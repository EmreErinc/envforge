use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile_batch, sort_secret_pairs,
    validate_provider_arg, validate_provider_response_label, validate_provider_response_value,
    SecretProvider, SecretsError,
};

fn checked_env_project(
    credentials: &HashMap<String, String>,
) -> Result<(Option<String>, Option<String>), SecretsError> {
    let env = match credentials.get("environment") {
        Some(e) => {
            validate_provider_arg(e, "infisical environment")?;
            Some(e.clone())
        }
        None => None,
    };
    let project = match credentials.get("project_id") {
        Some(p) => {
            validate_provider_arg(p, "infisical project_id")?;
            Some(p.clone())
        }
        None => None,
    };
    Ok((env, project))
}

pub struct InfisicalProvider;

impl SecretProvider for InfisicalProvider {
    fn name(&self) -> &str {
        "infisical"
    }

    fn display_name(&self) -> &str {
        "Infisical"
    }

    fn binary_name(&self) -> &str {
        "infisical"
    }

    fn install_hint(&self) -> &str {
        "https://infisical.com/docs/cli/overview"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("0.14.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["token", "project_id", "environment"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(token) = credentials.get("token") {
            env.push(("INFISICAL_TOKEN", token.clone()));
        }
        env
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut args = vec!["export", "--format=json"];

        let (env_str, project_str) = checked_env_project(credentials)?;
        if let Some(e) = env_str.as_deref() {
            args.extend_from_slice(&["--env", e]);
        }
        if let Some(p) = project_str.as_deref() {
            args.extend_from_slice(&["--projectId", p]);
        }

        let output = run_cli("infisical", &args, &env_refs, "infisical")?;
        parse_infisical_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        // Infisical supports --file flag for batch secret upload
        let mut args: Vec<&str> = vec!["secrets", "set", "--file", "__TEMP__"];

        let (env_str, project_str) = checked_env_project(credentials)?;
        if let Some(e) = env_str.as_deref() {
            args.extend_from_slice(&["--env", e]);
        }
        if let Some(p) = project_str.as_deref() {
            args.extend_from_slice(&["--projectId", p]);
        }

        run_cli_with_tempfile_batch("infisical", &args, secrets, &env_refs, "infisical")?;
        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let all = self.pull(credentials, "")?;
        all.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| SecretsError::ProviderError {
                provider: "infisical".to_string(),
                message: format!("key '{}' not found", key),
            })
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let secrets = self.pull(credentials, path)?;
        Ok(secrets.into_iter().map(|(k, _)| k).collect())
    }
}

pub fn parse_infisical_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "infisical".to_string(),
            message: e.to_string(),
        })?;

    let mut result: Vec<(String, String)> = Vec::new();
    for item in &items {
        let key = match item.get("key").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let value = match item.get("value").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        validate_provider_response_label("infisical", key)?;
        validate_provider_response_value("infisical", value)?;
        result.push((key.to_string(), value.to_string()));
    }

    sort_secret_pairs(&mut result);
    Ok(result)
}
