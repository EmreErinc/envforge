use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_secret_name,
    validate_secret_value, SecretProvider, SecretsError,
};

pub struct AkeylessProvider;

impl SecretProvider for AkeylessProvider {
    fn name(&self) -> &str {
        "akeyless"
    }

    fn display_name(&self) -> &str {
        "Akeyless Vault"
    }

    fn binary_name(&self) -> &str {
        "akeyless"
    }

    fn install_hint(&self) -> &str {
        "https://docs.akeyless.io/docs/cli"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("1.50.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["access_id", "access_key"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(id) = credentials.get("access_id") {
            env.push(("AKEYLESS_ACCESS_ID", id.clone()));
        }
        if let Some(key) = credentials.get("access_key") {
            env.push(("AKEYLESS_ACCESS_KEY", key.clone()));
        }
        env
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let path_str = if path.is_empty() { "/" } else { path };

        let list_args = vec!["list-items", "--path", path_str, "--json"];
        let list_output = run_cli("akeyless", &list_args, &env_refs, "akeyless")?;
        let items = parse_akeyless_list(&list_output)?;

        let mut secrets = Vec::new();
        for item_name in &items {
            let get_args = vec!["get-secret-value", "-n", item_name, "--json"];
            let value_output = run_cli("akeyless", &get_args, &env_refs, "akeyless")?;
            if let Ok(value) = parse_akeyless_value(&value_output, item_name) {
                let key_name = extract_key_name(item_name);
                secrets.push((key_name, value));
            }
        }

        sort_secret_pairs(&mut secrets);
        Ok(secrets)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let path_prefix = if path.is_empty() || path == "/" {
            "/".to_string()
        } else {
            let p = path.trim_end_matches('/');
            format!("{}/", p)
        };

        let mut count = 0;
        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            let full_name = format!("{}{}", path_prefix, key);

            let args = vec![
                "update-secret-val",
                "--name",
                &full_name,
                "--value",
                "__TEMP__",
            ];

            if run_cli_with_tempfile("akeyless", &args, value, &env_refs, "akeyless").is_err() {
                let create_args =
                    vec!["create-secret", "--name", &full_name, "--value", "__TEMP__"];
                run_cli_with_tempfile("akeyless", &create_args, value, &env_refs, "akeyless")?;
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
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let full_name = if path.is_empty() || path == "/" {
            format!("/{}", key)
        } else {
            format!("{}/{}", path.trim_end_matches('/'), key)
        };

        let args = vec!["get-secret-value", "-n", &full_name, "--json"];
        let output = run_cli("akeyless", &args, &env_refs, "akeyless")?;
        parse_akeyless_value(&output, &full_name)
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let path_str = if path.is_empty() { "/" } else { path };

        let args = vec!["list-items", "--path", path_str, "--json"];
        let output = run_cli("akeyless", &args, &env_refs, "akeyless")?;
        let items = parse_akeyless_list(&output)?;
        let mut keys: Vec<String> = items.iter().map(|name| extract_key_name(name)).collect();
        keys.sort();
        Ok(keys)
    }
}

/// Parse `akeyless list-items --json` output. Returns item names of STATIC_SECRET type.
pub fn parse_akeyless_list(output: &str) -> Result<Vec<String>, SecretsError> {
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let arr: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "akeyless".to_string(),
            message: e.to_string(),
        })?;

    let mut names: Vec<String> = arr
        .iter()
        .filter_map(|item| {
            let item_type = item.get("item_type")?.as_str()?;
            if item_type != "STATIC_SECRET" {
                return None;
            }
            let name = item.get("item_name")?.as_str()?;
            Some(name.to_string())
        })
        .collect();

    names.sort();
    Ok(names)
}

/// Parse `akeyless get-secret-value --json` output.
/// Format: `{"/path/name": "value"}`
pub fn parse_akeyless_value(output: &str, item_name: &str) -> Result<String, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "akeyless".to_string(),
            message: e.to_string(),
        })?;

    // Try exact name match first
    if let Some(val) = json.get(item_name).and_then(|v| v.as_str()) {
        return Ok(val.to_string());
    }

    // If single key in object, use that
    if let Some(obj) = json.as_object() {
        if obj.len() == 1 {
            if let Some((_, val)) = obj.iter().next() {
                if let Some(s) = val.as_str() {
                    return Ok(s.to_string());
                }
            }
        }
    }

    Err(SecretsError::ParseError {
        provider: "akeyless".to_string(),
        message: format!("could not extract value for '{}'", item_name),
    })
}

/// Extract key name from full Akeyless path. Last segment after final `/`.
pub fn extract_key_name(full_path: &str) -> String {
    full_path
        .rsplit('/')
        .next()
        .unwrap_or(full_path)
        .to_string()
}

/// Build environment variables (public for tests).
pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let provider = AkeylessProvider;
    provider.build_provider_env(credentials)
}
