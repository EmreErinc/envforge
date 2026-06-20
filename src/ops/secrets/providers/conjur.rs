use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_stdin, sandbox_command, sort_secret_pairs,
    validate_env_pair, validate_provider_arg, validate_provider_response_label,
    validate_provider_response_value, validate_secret_name, validate_secret_value,
    verify_provider_binary, CredentialEncryptionPolicy, SecretProvider, SecretsError,
};

pub struct ConjurProvider;

impl SecretProvider for ConjurProvider {
    fn name(&self) -> &str {
        "conjur"
    }

    fn display_name(&self) -> &str {
        "CyberArk Conjur"
    }

    fn binary_name(&self) -> &str {
        "conjur"
    }

    fn install_hint(&self) -> &str {
        "https://github.com/cyberark/conjur-cli-go/releases"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("1.0.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["url", "account", "login", "api_key"]
    }

    fn encryption_mode(&self) -> CredentialEncryptionPolicy {
        CredentialEncryptionPolicy::Mandatory
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(url) = credentials.get("url") {
            env.push(("CONJUR_APPLIANCE_URL", url.clone()));
        }
        if let Some(account) = credentials.get("account") {
            env.push(("CONJUR_ACCOUNT", account.clone()));
        }
        if let Some(login) = credentials.get("login") {
            env.push(("CONJUR_AUTHN_LOGIN", login.clone()));
        }
        if let Some(api_key) = credentials.get("api_key") {
            env.push(("CONJUR_AUTHN_API_KEY", api_key.clone()));
        }
        env
    }

    fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        self.validate_config(credentials)?;
        self.check_binary()?;

        Self::validate_appliance_url(credentials)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        // Initialize Conjur CLI connection
        let url = credentials.get("url").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'url' credential".to_string())
        })?;
        let account = credentials.get("account").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'account' credential".to_string())
        })?;
        run_cli(
            "conjur",
            &["init", "-u", url, "-a", account, "--force"],
            &env_refs,
            "conjur",
        )?;

        // Login using env vars — read credentials from CONJUR_AUTHN_LOGIN and CONJUR_AUTHN_API_KEY
        // instead of passing -p <api_key> as a CLI flag (which leaks via /proc/PID/cmdline)
        let login = credentials.get("login").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'login' credential".to_string())
        })?;
        let api_key = credentials.get("api_key").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'api_key' credential".to_string())
        })?;
        // Use stdin pipe to pass API key instead of -p flag to avoid /proc leakage
        verify_provider_binary("conjur", "conjur")?;

        let mut cmd = std::process::Command::new("conjur");
        cmd.env_clear();
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", home);
        }
        cmd.args(["login", "-i", login]);
        for (k, v) in &env_refs {
            validate_env_pair(k, v)?;
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        sandbox_command(&mut cmd);

        let mut child = cmd.spawn().map_err(|e| SecretsError::ProviderError {
            provider: "conjur".to_string(),
            message: e.to_string(),
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(api_key.as_bytes())
                .map_err(|e| SecretsError::ProviderError {
                    provider: "conjur".to_string(),
                    message: format!("failed to write to stdin: {}", e),
                })?;
            stdin
                .write_all(b"\n")
                .map_err(|e| SecretsError::ProviderError {
                    provider: "conjur".to_string(),
                    message: format!("failed to write to stdin: {}", e),
                })?;
        }

        let status = child.wait().map_err(|e| SecretsError::ProviderError {
            provider: "conjur".to_string(),
            message: e.to_string(),
        })?;

        if !status.success() {
            return Err(SecretsError::AuthFailed {
                provider: "conjur".to_string(),
                message: "conjur login failed".to_string(),
            });
        }

        Ok(())
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        if !path.is_empty() {
            validate_provider_arg(path, "conjur path")?;
        }
        Self::validate_appliance_url(credentials)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let account = credentials.get("account").map(|s| s.as_str()).unwrap_or("");

        // List all variables
        let list_output = run_cli("conjur", &["list", "-k", "variable"], &env_refs, "conjur")?;
        let variable_ids = parse_conjur_list(&list_output, account)?;

        // Filter by path prefix
        let filtered: Vec<&str> = if path.is_empty() {
            variable_ids.iter().map(|s| s.as_str()).collect()
        } else {
            let prefix = path.trim_start_matches('/');
            variable_ids
                .iter()
                .filter(|id| id.starts_with(prefix))
                .map(|s| s.as_str())
                .collect()
        };

        // Get each variable value
        let mut secrets = Vec::new();
        for var_path in &filtered {
            // var_path came from parse_conjur_list which validates labels.
            let output = run_cli(
                "conjur",
                &["variable", "get", "-i", "--", var_path],
                &env_refs,
                "conjur",
            )?;
            let key = extract_conjur_key(var_path);
            let value = output.trim();
            validate_provider_response_label("conjur", &key)?;
            validate_provider_response_value("conjur", value)?;
            secrets.push((key, value.to_string()));
        }

        sort_secret_pairs(&mut secrets);
        Ok(secrets)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        Self::validate_appliance_url(credentials)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut count = 0;
        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            // Use stdin pipe to pass value instead of -v flag to avoid /proc leakage
            run_cli_with_stdin(
                "conjur",
                &["variable", "set", "-i", "--", key],
                value.as_bytes(),
                &env_refs,
                "conjur",
            )?;
            count += 1;
        }

        Ok(count)
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        validate_secret_name(key)?;
        Self::validate_appliance_url(credentials)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli(
            "conjur",
            &["variable", "get", "-i", "--", key],
            &env_refs,
            "conjur",
        )?;
        let value = output.trim();
        validate_provider_response_value("conjur", value)?;
        Ok(value.to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        if !path.is_empty() {
            validate_provider_arg(path, "conjur path")?;
        }
        Self::validate_appliance_url(credentials)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let account = credentials.get("account").map(|s| s.as_str()).unwrap_or("");

        let output = run_cli("conjur", &["list", "-k", "variable"], &env_refs, "conjur")?;
        let mut variable_ids = parse_conjur_list(&output, account)?;

        // Filter by path prefix
        if !path.is_empty() {
            let prefix = path.trim_start_matches('/');
            variable_ids.retain(|id| id.starts_with(prefix));
        }

        let mut keys: Vec<String> = variable_ids
            .iter()
            .map(|id| extract_conjur_key(id))
            .collect();
        keys.sort();
        Ok(keys)
    }
}

impl ConjurProvider {
    /// Validate the Conjur appliance URL (if present): must be http(s), have a
    /// host, and carry no control characters. Conservative SSRF/scheme guard
    /// (L7) — the URL flows into `CONJUR_APPLIANCE_URL` and `conjur init -u`.
    fn validate_appliance_url(credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        let Some(url) = credentials.get("url") else {
            return Ok(());
        };
        let trimmed = url.trim();
        let rest = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .ok_or_else(|| {
                SecretsError::CredentialError(
                    "conjur: 'url' must start with https:// or http://".to_string(),
                )
            })?;
        if rest.split('/').next().unwrap_or("").is_empty() {
            return Err(SecretsError::CredentialError(
                "conjur: 'url' has no host".to_string(),
            ));
        }
        if trimmed.chars().any(|c| c.is_control()) {
            return Err(SecretsError::CredentialError(
                "conjur: 'url' contains control characters".to_string(),
            ));
        }
        Ok(())
    }
}

/// Parse `conjur list -k variable` JSON output.
/// Input: `["account:variable:path/name", ...]`
/// Returns: stripped variable paths (without `account:variable:` prefix).
pub fn parse_conjur_list(output: &str, account: &str) -> Result<Vec<String>, SecretsError> {
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let arr: Vec<String> = serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
        provider: "conjur".to_string(),
        message: e.to_string(),
    })?;

    let prefix = format!("{}:variable:", account);

    let names: Vec<String> = arr
        .into_iter()
        .filter_map(|id| {
            if id.starts_with(&prefix) {
                Some(id[prefix.len()..].to_string())
            } else if id.contains(":variable:") {
                // Handle case where account doesn't match — strip generically
                let parts: Vec<&str> = id.splitn(3, ':').collect();
                if parts.len() == 3 && parts[1] == "variable" {
                    Some(parts[2].to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(names)
}

/// Extract key name from Conjur variable path.
/// "app/db/HOST" → "HOST" (last segment)
pub fn extract_conjur_key(var_path: &str) -> String {
    var_path.rsplit('/').next().unwrap_or(var_path).to_string()
}
