use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_provider_arg,
    validate_secret_name, validate_secret_value, CredentialEncryptionPolicy, SecretProvider,
    SecretsError,
};

/// Pull the `profile` credential, if present, after validating it cannot
/// inject a CLI flag (leading `-`, control chars, etc.). Returns
/// `Ok(None)` when the field is unset; `Err` when a hostile value is
/// present in the credential store.
fn checked_profile(credentials: &HashMap<String, String>) -> Result<Option<String>, SecretsError> {
    match credentials.get("profile") {
        Some(p) => {
            validate_provider_arg(p, "keeper profile")?;
            Ok(Some(p.clone()))
        }
        None => Ok(None),
    }
}

pub struct KeeperProvider;

impl SecretProvider for KeeperProvider {
    fn name(&self) -> &str {
        "keeper"
    }

    fn display_name(&self) -> &str {
        "Keeper Secrets Manager"
    }

    fn binary_name(&self) -> &str {
        "ksm"
    }

    fn install_hint(&self) -> &str {
        "pip3 install keeper-secrets-manager-cli"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("1.0.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec![]
    }

    fn encryption_mode(&self) -> CredentialEncryptionPolicy {
        CredentialEncryptionPolicy::Mandatory
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["profile"]
    }

    fn build_provider_env(
        &self,
        _credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        // ksm uses its own device config, not env vars
        Vec::new()
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut list_args = vec!["secret", "list", "--json"];
        let profile_str = checked_profile(credentials)?;
        if let Some(p) = profile_str.as_deref() {
            list_args.extend_from_slice(&["--profile", p]);
        }

        let list_output = run_cli("ksm", &list_args, &env_refs, "keeper")?;
        let records = parse_keeper_list(&list_output)?;

        let mut secrets = Vec::new();
        for (uid, title) in &records {
            let mut get_args = vec!["secret", "get", "--uid", uid, "--json"];
            if let Some(p) = profile_str.as_deref() {
                get_args.extend_from_slice(&["--profile", p]);
            }

            if let Ok(output) = run_cli("ksm", &get_args, &env_refs, "keeper") {
                if let Ok(value) = parse_keeper_record(&output) {
                    secrets.push((title.clone(), value));
                }
            }
        }

        sort_secret_pairs(&mut secrets);
        Ok(secrets)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut list_args = vec!["secret", "list", "--json"];
        let profile_str = checked_profile(credentials)?;
        if let Some(p) = profile_str.as_deref() {
            list_args.extend_from_slice(&["--profile", p]);
        }

        let list_output = run_cli("ksm", &list_args, &env_refs, "keeper")?;
        let existing = parse_keeper_list(&list_output)?;
        let title_to_uid: HashMap<&str, &str> = existing
            .iter()
            .map(|(uid, title)| (title.as_str(), uid.as_str()))
            .collect();

        let mut count = 0;
        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            if let Some(uid) = title_to_uid.get(key.as_str()) {
                let field_value = format!("password={}", value);
                let mut update_args = vec!["secret", "update", "--uid", uid, "--field", "__TEMP__"];

                if let Some(p) = profile_str.as_deref() {
                    update_args.extend_from_slice(&["--profile", p]);
                }

                run_cli_with_tempfile("ksm", &update_args, &field_value, &env_refs, "keeper")?;
                count += 1;
            } else {
                return Err(SecretsError::ProviderError {
                    provider: "keeper".to_string(),
                    message: format!(
                        "cannot create new secret '{}'. Keeper push only supports updating existing records.",
                        key
                    ),
                });
            }
        }

        Ok(count)
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        // Find the record by title match via pull
        let all = self.pull(credentials, path)?;
        all.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| SecretsError::ProviderError {
                provider: "keeper".to_string(),
                message: format!("key '{}' not found", key),
            })
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut args = vec!["secret", "list", "--json"];
        let profile_str = checked_profile(credentials)?;
        if let Some(p) = profile_str.as_deref() {
            args.extend_from_slice(&["--profile", p]);
        }

        let output = run_cli("ksm", &args, &env_refs, "keeper")?;
        let records = parse_keeper_list(&output)?;
        let mut titles: Vec<String> = records.into_iter().map(|(_, title)| title).collect();
        titles.sort();
        Ok(titles)
    }
}

/// Parse `ksm secret list --json` output.
/// Returns: Vec of (uid, title) tuples.
pub fn parse_keeper_list(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let arr: Vec<serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "keeper".to_string(),
            message: e.to_string(),
        })?;

    let mut records: Vec<(String, String)> = Vec::new();
    for item in &arr {
        let uid = match item.get("uid").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let title = match item.get("title").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        // Cap label length / reject control chars before propagating.
        if super::super::provider::validate_provider_response_label("keeper", uid).is_err()
            || super::super::provider::validate_provider_response_label("keeper", title).is_err()
        {
            continue;
        }
        records.push((uid.to_string(), title.to_string()));
    }

    Ok(records)
}

/// Parse `ksm secret get --uid <UID> --json` output.
/// Extracts the password field value from the complex nested record.
pub fn parse_keeper_record(output: &str) -> Result<String, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "keeper".to_string(),
            message: e.to_string(),
        })?;

    let fields = json
        .get("fields")
        .and_then(|f| f.as_array())
        .ok_or_else(|| SecretsError::ParseError {
            provider: "keeper".to_string(),
            message: "missing 'fields' array in record".to_string(),
        })?;

    // Look for password field first, then login, then any field with a value
    for field_type in &["password", "login", "secret", "text"] {
        for field in fields {
            if let Some(ft) = field.get("type").and_then(|t| t.as_str()) {
                if ft == *field_type {
                    if let Some(values) = field.get("value").and_then(|v| v.as_array()) {
                        if let Some(first) = values.first() {
                            if let Some(s) = first.as_str() {
                                if !s.is_empty() {
                                    super::super::provider::validate_provider_response_value(
                                        "keeper", s,
                                    )?;
                                    return Ok(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(SecretsError::ParseError {
        provider: "keeper".to_string(),
        message: "no extractable value found in record fields".to_string(),
    })
}
