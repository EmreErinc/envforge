use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, sort_secret_pairs, SecretProvider, SecretsError,
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

    fn credential_fields(&self) -> Vec<&str> {
        vec!["url", "account", "login", "api_key"]
    }

    fn build_provider_env(
        &self,
        _credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        // Conjur uses conjur init + login, not env vars
        Vec::new()
    }

    fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        self.validate_config(credentials)?;
        self.check_binary()?;

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

        // Login with API key
        let login = credentials.get("login").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'login' credential".to_string())
        })?;
        let api_key = credentials.get("api_key").ok_or_else(|| {
            SecretsError::CredentialError("conjur: missing 'api_key' credential".to_string())
        })?;
        run_cli(
            "conjur",
            &["login", "-i", login, "-p", api_key],
            &env_refs,
            "conjur",
        )?;

        Ok(())
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
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
            let output = run_cli(
                "conjur",
                &["variable", "get", "-i", var_path],
                &env_refs,
                "conjur",
            )?;
            let key = extract_conjur_key(var_path);
            secrets.push((key, output.trim().to_string()));
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
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut count = 0;
        for (key, value) in secrets {
            run_cli(
                "conjur",
                &["variable", "set", "-i", key, "-v", value],
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
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli(
            "conjur",
            &["variable", "get", "-i", key],
            &env_refs,
            "conjur",
        )?;
        Ok(output.trim().to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
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
