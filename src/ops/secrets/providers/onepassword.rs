use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_provider_arg,
    validate_provider_response_label, validate_provider_response_value, validate_secret_name,
    validate_secret_value, SecretProvider, SecretsError,
};

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

    fn minimum_version(&self) -> Option<&str> {
        Some("2.25.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["service_account_token"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(token) = credentials.get("service_account_token") {
            env.push(("OP_SERVICE_ACCOUNT_TOKEN", token.clone()));
        }
        env
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        validate_provider_arg(path, "1password item path")?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli(
            "op",
            &["item", "get", "--format=json", "--", path],
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
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            let assignment = format!("{}={}", key, value);
            // 1Password CLI supports --template (file-based) for bulk edits.
            // For single key updates, use stdin piped through `op item edit`.
            run_cli_with_tempfile(
                "op",
                &["item", "edit", path, "__TEMP__"],
                &assignment,
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
        validate_provider_arg(path, "1password item path")?;
        validate_secret_name(key)?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        // `key` cannot start with `-`, contain `=`, or contain control chars
        // (enforced above). Build the flag and place positional args after `--`.
        let field = format!("--fields=label={}", key);
        let output = run_cli(
            "op",
            &["item", "get", &field, "--", path],
            &env_refs,
            "1password",
        )?;

        Ok(output.trim().to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        validate_provider_arg(path, "1password vault")?;
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

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
    let provider = OnePasswordProvider;
    provider.build_provider_env(credentials)
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

    let mut result: Vec<(String, String)> = Vec::new();
    for f in fields {
        let label = match f.get("label").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let value = f.get("value").and_then(|v| v.as_str()).unwrap_or("");
        if label.is_empty() || value.is_empty() {
            continue;
        }
        // Defend against compromised / hostile CLI output: cap sizes,
        // reject control chars in labels, reject NUL in values.
        validate_provider_response_label("1password", label)?;
        validate_provider_response_value("1password", value)?;
        result.push((label.to_string(), value.to_string()));
    }

    sort_secret_pairs(&mut result);
    Ok(result)
}
