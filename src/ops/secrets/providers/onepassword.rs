use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

pub struct OnePasswordProvider;

impl SecretProvider for OnePasswordProvider {
    fn name(&self) -> &str {
        "1password"
    }

    fn display_name(&self) -> &str {
        "1Password"
    }

    fn binary_name(&self) -> &str {
        "op"
    }

    fn install_hint(&self) -> &str {
        "https://developer.1password.com/docs/cli/get-started/"
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["service_account_token"]
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let output = run_cli(
            "op",
            &["item", "get", path, "--format=json"],
            &env_refs,
            "1password",
        )?;
        parse_item_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        for (key, value) in secrets {
            let assignment = format!("{}={}", key, value);
            run_cli(
                "op",
                &["item", "edit", path, &assignment],
                &env_refs,
                "1password",
            )?;
        }

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

        let field = format!("--fields=label={}", key);
        let output = run_cli("op", &["item", "get", path, &field], &env_refs, "1password")?;

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
            "op",
            &["item", "list", "--vault", path, "--format=json"],
            &env_refs,
            "1password",
        )?;

        let items: Vec<serde_json::Value> =
            serde_json::from_str(&output).map_err(|e| SecretsError::ParseError {
                provider: "1password".to_string(),
                message: e.to_string(),
            })?;

        Ok(items
            .iter()
            .filter_map(|i| i.get("title").and_then(|t| t.as_str()).map(String::from))
            .collect())
    }
}

pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(token) = credentials.get("service_account_token") {
        env.push(("OP_SERVICE_ACCOUNT_TOKEN", token.clone()));
    }
    env
}

pub fn parse_item_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "1password".to_string(),
            message: e.to_string(),
        })?;

    let fields = json
        .get("fields")
        .and_then(|f| f.as_array())
        .ok_or_else(|| SecretsError::ParseError {
            provider: "1password".to_string(),
            message: "no fields array in item".to_string(),
        })?;

    let mut result: Vec<(String, String)> = fields
        .iter()
        .filter_map(|f| {
            let label = f.get("label")?.as_str()?;
            let value = f.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if label.is_empty() || value.is_empty() {
                None
            } else {
                Some((label.to_string(), value.to_string()))
            }
        })
        .collect();

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
