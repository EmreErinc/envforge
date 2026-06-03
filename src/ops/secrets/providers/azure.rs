use std::collections::HashMap;

use super::super::provider::{
    run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_provider_arg,
    validate_provider_response_label, validate_provider_response_value, validate_secret_name,
    validate_secret_value, CredentialEncryptionPolicy, SecretProvider, SecretsError,
};

/// Validate `vault_name` from credentials before it is interpolated into a
/// `--vault-name` flag. Refuses leading dash / control chars / `=` smuggling.
fn checked_vault(credentials: &HashMap<String, String>) -> Result<String, SecretsError> {
    let vault = credentials.get("vault_name").cloned().unwrap_or_default();
    if !vault.is_empty() {
        validate_provider_arg(&vault, "azure vault_name")?;
    }
    Ok(vault)
}

pub struct AzureKeyVaultProvider;

/// Convert env var name (with underscores) to Azure Key Vault compatible name (hyphens).
/// Azure Key Vault only allows `[a-zA-Z0-9-]` in secret names.
pub fn to_vault_name(env_key: &str) -> String {
    env_key.replace('_', "-")
}

/// Convert Azure Key Vault secret name (hyphens) back to env var style (underscores).
pub fn from_vault_name(vault_name: &str) -> String {
    vault_name.replace('-', "_")
}

impl SecretProvider for AzureKeyVaultProvider {
    fn name(&self) -> &str {
        "azure"
    }

    fn display_name(&self) -> &str {
        "Azure Key Vault"
    }

    fn binary_name(&self) -> &str {
        "az"
    }

    fn install_hint(&self) -> &str {
        "https://learn.microsoft.com/en-us/cli/azure/install-azure-cli"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("2.50.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["vault_name"]
    }

    fn encryption_mode(&self) -> CredentialEncryptionPolicy {
        CredentialEncryptionPolicy::Mandatory
    }

    fn build_provider_env(
        &self,
        _credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        // Azure CLI uses `az` command directly, no env vars needed
        Vec::new()
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let vault = checked_vault(credentials)?;

        let vault_keys = self.list_vault_names(credentials)?;
        let mut result = Vec::new();

        for vault_key in &vault_keys {
            let output = run_cli(
                "az",
                &[
                    "keyvault",
                    "secret",
                    "show",
                    "--name",
                    vault_key,
                    "--vault-name",
                    &vault,
                    "--output",
                    "json",
                ],
                &[],
                "azure",
            )?;

            let json: serde_json::Value =
                serde_json::from_str(&output).map_err(|e| SecretsError::ParseError {
                    provider: "azure".to_string(),
                    message: e.to_string(),
                })?;

            if let Some(value) = json.get("value").and_then(|v| v.as_str()) {
                validate_provider_response_value("azure", value)?;
                // Convert vault name (hyphens) back to env var style (underscores)
                let env_key = from_vault_name(vault_key);
                validate_provider_response_label("azure", &env_key)?;
                result.push((env_key, value.to_string()));
            }
        }

        sort_secret_pairs(&mut result);
        Ok(result)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let vault = checked_vault(credentials)?;

        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            let vault_name = to_vault_name(key);
            run_cli_with_tempfile(
                "az",
                &[
                    "keyvault",
                    "secret",
                    "set",
                    "--name",
                    &vault_name,
                    "--file",
                    "__TEMP__",
                    "--vault-name",
                    &vault,
                ],
                value,
                &[],
                "azure",
            )?;
        }

        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        validate_secret_name(key)?;
        let vault = checked_vault(credentials)?;
        let vault_key = to_vault_name(key);

        let output = run_cli(
            "az",
            &[
                "keyvault",
                "secret",
                "show",
                "--name",
                &vault_key,
                "--vault-name",
                &vault,
                "--output",
                "json",
            ],
            &[],
            "azure",
        )?;

        let json: serde_json::Value =
            serde_json::from_str(&output).map_err(|e| SecretsError::ParseError {
                provider: "azure".to_string(),
                message: e.to_string(),
            })?;

        let value =
            json.get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SecretsError::ParseError {
                    provider: "azure".to_string(),
                    message: "no value field in response".to_string(),
                })?;
        validate_provider_response_value("azure", value)?;
        Ok(value.to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let vault_names = self.list_vault_names(credentials)?;
        // Convert vault names (hyphens) to env var style (underscores)
        let mut names: Vec<String> = vault_names
            .into_iter()
            .map(|n| from_vault_name(&n))
            .collect();
        names.sort();
        Ok(names)
    }
}

impl AzureKeyVaultProvider {
    /// List raw vault secret names (with hyphens, not converted to underscores).
    fn list_vault_names(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Result<Vec<String>, SecretsError> {
        let vault = checked_vault(credentials)?;

        let output = run_cli(
            "az",
            &[
                "keyvault",
                "secret",
                "list",
                "--vault-name",
                &vault,
                "--output",
                "json",
            ],
            &[],
            "azure",
        )?;

        let items: Vec<serde_json::Value> =
            serde_json::from_str(&output).map_err(|e| SecretsError::ParseError {
                provider: "azure".to_string(),
                message: e.to_string(),
            })?;

        let mut names: Vec<String> = Vec::new();
        for i in &items {
            let id = match i.get("id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            // Format: "https://myvault.vault.azure.net/secrets/MY-SECRET"
            if let Some(short) = id.rsplit('/').next() {
                validate_provider_response_label("azure", short)?;
                names.push(short.to_string());
            }
        }

        names.sort();
        Ok(names)
    }
}
