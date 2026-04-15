use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

pub struct AzureKeyVaultProvider;

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

    fn credential_fields(&self) -> Vec<&str> {
        vec!["vault_name"]
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let vault = credentials.get("vault_name").cloned().unwrap_or_default();

        let keys = self.list(credentials, "")?;
        let mut result = Vec::new();

        for key in &keys {
            let output = run_cli(
                "az",
                &[
                    "keyvault",
                    "secret",
                    "show",
                    "--name",
                    key,
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
                result.push((key.clone(), value.to_string()));
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
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
            run_cli(
                "az",
                &[
                    "keyvault",
                    "secret",
                    "set",
                    "--name",
                    key,
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

        let output = run_cli(
            "az",
            &[
                "keyvault",
                "secret",
                "show",
                "--name",
                key,
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
