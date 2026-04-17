use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

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

    fn credential_fields(&self) -> Vec<&str> {
        vec!["token", "project_id", "environment"]
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let mut args = vec!["export", "--format=json"];

        let env_str;
        if let Some(environment) = credentials.get("environment") {
            env_str = environment.clone();
            args.extend_from_slice(&["--env", &env_str]);
        }

        let project_str;
        if let Some(project_id) = credentials.get("project_id") {
            project_str = project_id.clone();
            args.extend_from_slice(&["--projectId", &project_str]);
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
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        // Infisical supports multiple KEY=VALUE pairs in a single set call
        let assignments: Vec<String> = secrets
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let mut args: Vec<&str> = vec!["secrets", "set"];
        for a in &assignments {
            args.push(a);
        }

        let env_str;
        if let Some(environment) = credentials.get("environment") {
            env_str = environment.clone();
            args.extend_from_slice(&["--env", &env_str]);
        }

        let project_str;
        if let Some(project_id) = credentials.get("project_id") {
            project_str = project_id.clone();
            args.extend_from_slice(&["--projectId", &project_str]);
        }

        run_cli("infisical", &args, &env_refs, "infisical")?;
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

fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(token) = credentials.get("token") {
        env.push(("INFISICAL_TOKEN", token.clone()));
    }
    env
}

pub fn parse_infisical_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "infisical".to_string(),
            message: e.to_string(),
        })?;

    let mut result: Vec<(String, String)> = items
        .iter()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?;
            let value = item.get("value")?.as_str()?;
            Some((key.to_string(), value.to_string()))
        })
        .collect();

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
