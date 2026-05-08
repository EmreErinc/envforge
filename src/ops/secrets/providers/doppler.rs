use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile_batch, sort_secret_pairs,
    validate_provider_arg, validate_provider_response_label, validate_provider_response_value,
    SecretProvider, SecretsError,
};

/// Validate `project` and `config` credential fields before they are
/// passed positionally to `doppler` as flag values.
fn checked_project_config(
    credentials: &HashMap<String, String>,
) -> Result<(Option<String>, Option<String>), SecretsError> {
    let project = match credentials.get("project") {
        Some(p) => {
            validate_provider_arg(p, "doppler project")?;
            Some(p.clone())
        }
        None => None,
    };
    let config = match credentials.get("config") {
        Some(c) => {
            validate_provider_arg(c, "doppler config")?;
            Some(c.clone())
        }
        None => None,
    };
    Ok((project, config))
}

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

    fn minimum_version(&self) -> Option<&str> {
        Some("3.50.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["token", "project", "config"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(token) = credentials.get("token") {
            env.push(("DOPPLER_TOKEN", token.clone()));
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

        let mut args = vec!["secrets", "download", "--format=json", "--no-file"];

        let (project_str, config_str) = checked_project_config(credentials)?;
        if let Some(p) = project_str.as_deref() {
            args.extend_from_slice(&["--project", p]);
        }
        if let Some(c) = config_str.as_deref() {
            args.extend_from_slice(&["--config", c]);
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
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        // Doppler supports --file flag for batch secret upload (KEY=VALUE per line)
        let mut args: Vec<&str> = vec!["secrets", "set", "--file", "__TEMP__"];

        let (project_str, config_str) = checked_project_config(credentials)?;
        if let Some(p) = project_str.as_deref() {
            args.extend_from_slice(&["--project", p]);
        }
        if let Some(c) = config_str.as_deref() {
            args.extend_from_slice(&["--config", c]);
        }

        run_cli_with_tempfile_batch("doppler", &args, secrets, &env_refs, "doppler")?;
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

/// Doppler system keys injected into download output that are not user secrets.
const DOPPLER_SYSTEM_KEYS: &[&str] = &["DOPPLER_PROJECT", "DOPPLER_CONFIG", "DOPPLER_ENVIRONMENT"];

pub fn parse_doppler_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let map: HashMap<String, String> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "doppler".to_string(),
            message: e.to_string(),
        })?;

    let mut result: Vec<(String, String)> = Vec::new();
    for (k, v) in map {
        if DOPPLER_SYSTEM_KEYS.contains(&k.as_str()) {
            continue;
        }
        validate_provider_response_label("doppler", &k)?;
        validate_provider_response_value("doppler", &v)?;
        result.push((k, v));
    }
    sort_secret_pairs(&mut result);
    Ok(result)
}
