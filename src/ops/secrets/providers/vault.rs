use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, parse_json_secrets, run_cli, SecretProvider, SecretsError,
};

pub struct VaultProvider;

impl SecretProvider for VaultProvider {
    fn name(&self) -> &str {
        "vault"
    }

    fn display_name(&self) -> &str {
        "HashiCorp Vault"
    }

    fn binary_name(&self) -> &str {
        "vault"
    }

    fn install_hint(&self) -> &str {
        "https://developer.hashicorp.com/vault/install"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("1.15.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["addr"]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["token", "role_id", "secret_id", "auth_method"]
    }

    /// Build provider-specific environment variables for Vault CLI.
    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(addr) = credentials.get("addr") {
            env.push(("VAULT_ADDR", addr.clone()));
        }
        if let Some(token) = credentials.get("token") {
            env.push(("VAULT_TOKEN", token.clone()));
        }
        env
    }

    fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        self.check_binary()?;

        let auth_method = credentials
            .get("auth_method")
            .map(|s| s.as_str())
            .unwrap_or("token");

        match auth_method {
            "approle" => {
                let role_id = credentials.get("role_id").ok_or_else(|| {
                    SecretsError::CredentialError(
                        "AppRole auth requires 'role_id'. Run: envforge secrets config vault --set role_id=<value>".into(),
                    )
                })?;
                let secret_id = credentials.get("secret_id").ok_or_else(|| {
                    SecretsError::CredentialError(
                        "AppRole auth requires 'secret_id'. Run: envforge secrets config vault --set secret_id=<value>".into(),
                    )
                })?;
                // Login with AppRole using stdin for credentials
                // to avoid leaking role_id/secret_id via /proc/PID/cmdline
                let env_vars = [(
                    "VAULT_ADDR",
                    credentials.get("addr").map(|s| s.as_str()).unwrap_or(""),
                )];
                let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, *v)).collect();

                let mut cmd = std::process::Command::new("vault");
                cmd.args(["write", "-format=json", "auth/approle/login", "-"]);
                for (k, v) in &env_refs {
                    cmd.env(k, v);
                }
                cmd.stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let mut child = cmd.spawn().map_err(|e| SecretsError::ProviderError {
                    provider: "vault".to_string(),
                    message: e.to_string(),
                })?;

                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let payload = format!("role_id={}\nsecret_id={}\n", role_id, secret_id);
                    stdin.write_all(payload.as_bytes()).map_err(|e| SecretsError::ProviderError {
                        provider: "vault".to_string(),
                        message: format!("failed to write to stdin: {}", e),
                    })?;
                }

                let status = child.wait().map_err(|e| SecretsError::ProviderError {
                    provider: "vault".to_string(),
                    message: e.to_string(),
                })?;

                if !status.success() {
                    return Err(SecretsError::AuthFailed {
                        provider: "vault".to_string(),
                        message: "AppRole login failed".to_string(),
                    });
                }

                Ok(())
            }
            _ => {
                // Token auth — just verify token exists
                if !credentials.contains_key("token") {
                    return Err(SecretsError::CredentialError(
                        "Token auth requires 'token'. Run: envforge secrets config vault --set token=<value>".into(),
                    ));
                }
                Ok(())
            }
        }
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli(
            "vault",
            &["kv", "get", "-format=json", path],
            &env_refs,
            "vault",
        )?;

        parse_json_secrets(&output, &["data", "data"], "vault")
            .or_else(|_| parse_json_secrets(&output, &["data"], "vault"))
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let kv_args: Vec<String> = secrets
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let mut args = vec!["kv", "put", path];
        let kv_strs: Vec<&str> = kv_args.iter().map(|s| s.as_str()).collect();
        args.extend(kv_strs);

        run_cli("vault", &args, &env_refs, "vault")?;
        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let field_flag = format!("-field={}", key);
        let output = run_cli(
            "vault",
            &["kv", "get", &field_flag, path],
            &env_refs,
            "vault",
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

        let output = run_cli(
            "vault",
            &["kv", "list", "-format=json", path],
            &env_refs,
            "vault",
        )?;

        parse_kv_list_output(&output)
    }
}

/// Legacy parse function for Vault KV GET output.
/// Wrapper around parse_json_secrets for backward compatibility with tests.
pub fn parse_kv_get_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    use super::super::provider::parse_json_secrets;
    parse_json_secrets(output, &["data", "data"], "vault")
        .or_else(|_| parse_json_secrets(output, &["data"], "vault"))
}

/// Legacy environment builder for Vault.
/// Wrapper around build_provider_env for backward compatibility with tests.
pub fn build_env(
    credentials: &std::collections::HashMap<String, String>,
) -> Vec<(&'static str, String)> {
    let provider = VaultProvider;
    provider.build_provider_env(credentials)
}

/// Parse `vault kv list -format=json` output.
/// Output structure: `{"data": {"keys": ["key1", "key2/", ...]}}`
pub fn parse_kv_list_output(output: &str) -> Result<Vec<String>, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "vault".to_string(),
            message: e.to_string(),
        })?;

    let keys = json
        .get("data")
        .and_then(|d| d.get("keys"))
        .and_then(|k| k.as_array())
        .ok_or_else(|| SecretsError::ParseError {
            provider: "vault".to_string(),
            message: "expected data.keys array in list output".to_string(),
        })?;

    Ok(keys
        .iter()
        .filter_map(|k| k.as_str().map(String::from))
        .collect())
}
