use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

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

    fn credential_fields(&self) -> Vec<&str> {
        vec!["addr"]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["token", "role_id", "secret_id", "auth_method"]
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
                // Login with AppRole to get a token
                let env_vars = vec![(
                    "VAULT_ADDR",
                    credentials.get("addr").map(|s| s.as_str()).unwrap_or(""),
                )];
                let _output = run_cli(
                    "vault",
                    &[
                        "write",
                        "-format=json",
                        "auth/approle/login",
                        &format!("role_id={}", role_id),
                        &format!("secret_id={}", secret_id),
                    ],
                    &env_vars,
                    "vault",
                )?;
                // If we get here, auth succeeded
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
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let output = run_cli(
            "vault",
            &["kv", "get", "-format=json", path],
            &env_refs,
            "vault",
        )?;

        parse_kv_get_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

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
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

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
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let output = run_cli(
            "vault",
            &["kv", "list", "-format=json", path],
            &env_refs,
            "vault",
        )?;

        parse_kv_list_output(&output)
    }
}

pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(addr) = credentials.get("addr") {
        env.push(("VAULT_ADDR", addr.clone()));
    }
    if let Some(token) = credentials.get("token") {
        env.push(("VAULT_TOKEN", token.clone()));
    }
    env
}

pub fn parse_kv_get_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "vault".to_string(),
            message: e.to_string(),
        })?;

    let data = json
        .get("data")
        .and_then(|d| d.get("data"))
        .or_else(|| json.get("data"))
        .ok_or_else(|| SecretsError::ParseError {
            provider: "vault".to_string(),
            message: "unexpected JSON structure".to_string(),
        })?;

    let map = data.as_object().ok_or_else(|| SecretsError::ParseError {
        provider: "vault".to_string(),
        message: "data is not an object".to_string(),
    })?;

    let mut result: Vec<(String, String)> = map
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
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
