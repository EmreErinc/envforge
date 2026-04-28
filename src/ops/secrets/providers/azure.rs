use std::collections::HashMap;

use super::super::provider::{run_cli, sort_secret_pairs, SecretProvider, SecretsError};

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
        let vault = credentials.get("vault_name").cloned().unwrap_or_default();

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
                // Convert vault name (hyphens) back to env var style (underscores)
                let env_key = from_vault_name(vault_key);
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
        let vault = credentials.get("vault_name").cloned().unwrap_or_default();

        for (key, value) in secrets {
            // Convert underscore env var names to hyphenated vault names
            let vault_name = to_vault_name(key);
            run_cli(
                "az",
                &[
                    "keyvault",
                    "secret",
                    "set",
                    "--name",
                    &vault_name,
                    "--value",
                    value,
                    "--vault-name",
                    &vault,
                ],
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
        let vault = credentials.get("vault_name").cloned().unwrap_or_default();
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

        json.get("value")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| SecretsError::ParseError {
                provider: "azure".to_string(),
                message: "no value field in response".to_string(),
            })
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
        let vault = credentials.get("vault_name").cloned().unwrap_or_default();

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

        let mut names: Vec<String> = items
            .iter()
            .filter_map(|i| {
                let id = i.get("id")?.as_str()?;
                // Format: "https://myvault.vault.azure.net/secrets/MY-SECRET"
                id.rsplit('/').next().map(String::from)
            })
            .collect();

        names.sort();
        Ok(names)
    }
}
