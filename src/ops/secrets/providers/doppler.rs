use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

pub struct DopplerProvider;

impl SecretProvider for DopplerProvider {
    fn name(&self) -> &str {
        "doppler"
    }

    fn display_name(&self) -> &str {
        "Doppler"
    }

    fn binary_name(&self) -> &str {
        "doppler"
    }

    fn install_hint(&self) -> &str {
        "https://docs.doppler.com/docs/install-cli"
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["token", "project", "config"]
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let mut args = vec!["secrets", "download", "--format=json", "--no-file"];

        let project_str;
        if let Some(project) = credentials.get("project") {
            project_str = project.clone();
            args.extend_from_slice(&["--project", &project_str]);
        }

        let config_str;
        if let Some(config) = credentials.get("config") {
            config_str = config.clone();
            args.extend_from_slice(&["--config", &config_str]);
        }

        let output = run_cli("doppler", &args, &env_refs, "doppler")?;
        parse_doppler_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        // Batch all KEY=VALUE pairs into a single doppler secrets set call
        let assignments: Vec<String> = secrets
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let mut args: Vec<&str> = vec!["secrets", "set"];
        for a in &assignments {
            args.push(a);
        }

        let project_str;
        if let Some(project) = credentials.get("project") {
            project_str = project.clone();
            args.extend_from_slice(&["--project", &project_str]);
        }

        let config_str;
        if let Some(config) = credentials.get("config") {
            config_str = config.clone();
            args.extend_from_slice(&["--config", &config_str]);
        }

        run_cli("doppler", &args, &env_refs, "doppler")?;
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
                provider: "doppler".to_string(),
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
        env.push(("DOPPLER_TOKEN", token.clone()));
    }
    env
}

/// Doppler system keys injected into download output that are not user secrets.
const DOPPLER_SYSTEM_KEYS: &[&str] = &["DOPPLER_PROJECT", "DOPPLER_CONFIG", "DOPPLER_ENVIRONMENT"];

pub fn parse_doppler_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let map: HashMap<String, String> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "doppler".to_string(),
            message: e.to_string(),
        })?;

    let mut result: Vec<(String, String)> = map
        .into_iter()
        .filter(|(k, _)| !DOPPLER_SYSTEM_KEYS.contains(&k.as_str()))
        .collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
