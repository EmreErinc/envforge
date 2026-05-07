use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_secret_name,
    validate_secret_value, SecretProvider, SecretsError,
};

pub struct BitwardenProvider;

impl SecretProvider for BitwardenProvider {
    fn name(&self) -> &str {
        "bitwarden"
    }

    fn display_name(&self) -> &str {
        "Bitwarden Secrets Manager"
    }

    fn binary_name(&self) -> &str {
        "bws"
    }

    fn install_hint(&self) -> &str {
        "https://bitwarden.com/help/secrets-manager-cli/"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("0.10.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["access_token"]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["project_id"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(token) = credentials.get("access_token") {
            env.push(("BWS_ACCESS_TOKEN", token.clone()));
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

        let mut args = vec!["secret", "list"];

        let project_id_str;
        if let Some(project_id) = credentials.get("project_id") {
            project_id_str = project_id.clone();
            args.push(&project_id_str);
        }

        let output = run_cli("bws", &args, &env_refs, "bitwarden")?;
        parse_bitwarden_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let project_id = credentials.get("project_id").ok_or_else(|| {
            SecretsError::CredentialError(
                "project_id required for push. Run: envforge secrets config bitwarden --set project_id=<value>".to_string(),
            )
        })?;

        // List existing secrets to find IDs for updates
        let mut list_args = vec!["secret", "list"];
        let project_id_str = project_id.clone();
        list_args.push(&project_id_str);
        let list_output = run_cli("bws", &list_args, &env_refs, "bitwarden")?;
        let existing = parse_bitwarden_list_with_ids(&list_output)?;

        let mut count = 0;
        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            if let Some(id) = existing.get(key.as_str()) {
                let id_str = id.clone();
                let args = vec!["secret", "edit", &id_str, "--value", "__TEMP__"];
                run_cli_with_tempfile("bws", &args, value, &env_refs, "bitwarden")?;
            } else {
                let args = vec!["secret", "create", key, "__TEMP__", &project_id_str];
                run_cli_with_tempfile("bws", &args, value, &env_refs, "bitwarden")?;
            }
            count += 1;
        }

        Ok(count)
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let all = self.pull(credentials, path)?;
        all.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| SecretsError::ProviderError {
                provider: "bitwarden".to_string(),
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

/// Parse `bws secret list` JSON output into key-value pairs.
/// Format: `[{"id": "...", "key": "NAME", "value": "secret", ...}]`
pub fn parse_bitwarden_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "bitwarden".to_string(),
            message: e.to_string(),
        })?;

    let mut secrets: Vec<(String, String)> = arr
        .iter()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?;
            let value = item.get("value")?.as_str()?;
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect();

    sort_secret_pairs(&mut secrets);
    Ok(secrets)
}

/// Parse bws secret list output and return a map of key → secret ID.
fn parse_bitwarden_list_with_ids(output: &str) -> Result<HashMap<String, String>, SecretsError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "bitwarden".to_string(),
            message: e.to_string(),
        })?;

    let map = arr
        .iter()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?.to_string();
            let id = item.get("id")?.as_str()?.to_string();
            Some((key, id))
        })
        .collect();

    Ok(map)
}

/// Build environment variables for Bitwarden CLI (public for tests).
pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let provider = BitwardenProvider;
    provider.build_provider_env(credentials)
}
