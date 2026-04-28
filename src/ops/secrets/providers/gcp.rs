use std::collections::HashMap;

use super::super::provider::{run_cli, sort_secret_pairs, SecretProvider, SecretsError};

pub struct GcpSecretManagerProvider;

impl SecretProvider for GcpSecretManagerProvider {
    fn name(&self) -> &str {
        "gcp"
    }

    fn display_name(&self) -> &str {
        "GCP Secret Manager"
    }

    fn binary_name(&self) -> &str {
        "gcloud"
    }

    fn install_hint(&self) -> &str {
        "https://cloud.google.com/sdk/docs/install"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("450.0.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["project_id"]
    }

    fn build_provider_env(
        &self,
        _credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        // GCP uses gcloud CLI directly, no env vars needed
        Vec::new()
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let keys = self.list(credentials, "")?;
        let mut result = Vec::new();

        let project = credentials.get("project_id").cloned().unwrap_or_default();

        for key in &keys {
            let output = run_cli(
                "gcloud",
                &[
                    "secrets",
                    "versions",
                    "access",
                    "latest",
                    &format!("--secret={}", key),
                    &format!("--project={}", project),
                ],
                &[],
                "gcp",
            )?;
            result.push((key.clone(), output.trim().to_string()));
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
        let project = credentials.get("project_id").cloned().unwrap_or_default();

        for (key, value) in secrets {
            // Try to create; if exists, add a new version
            let create_result = run_cli(
                "gcloud",
                &[
                    "secrets",
                    "create",
                    key,
                    &format!("--project={}", project),
                    "--replication-policy=automatic",
                ],
                &[],
                "gcp",
            );

            // Add version — use temp file since piping is complex with Command
            let temp = tempfile::NamedTempFile::new().map_err(|e| SecretsError::IoError {
                path: std::path::PathBuf::from("/tmp"),
                source: e,
            })?;
            std::fs::write(temp.path(), value).map_err(|e| SecretsError::IoError {
                path: temp.path().to_path_buf(),
                source: e,
            })?;

            run_cli(
                "gcloud",
                &[
                    "secrets",
                    "versions",
                    "add",
                    key,
                    &format!("--project={}", project),
                    &format!("--data-file={}", temp.path().display()),
                ],
                &[],
                "gcp",
            )?;

            // Suppress create error (secret may already exist)
            let _ = create_result;
        }

        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let project = credentials.get("project_id").cloned().unwrap_or_default();

        let output = run_cli(
            "gcloud",
            &[
                "secrets",
                "versions",
                "access",
                "latest",
                &format!("--secret={}", key),
                &format!("--project={}", project),
            ],
            &[],
            "gcp",
        )?;

        Ok(output.trim().to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let project = credentials.get("project_id").cloned().unwrap_or_default();

        let output = run_cli(
            "gcloud",
            &[
                "secrets",
                "list",
                &format!("--project={}", project),
                "--format=json",
            ],
            &[],
            "gcp",
        )?;

        parse_gcp_list_output(&output)
    }
}

/// Parse `gcloud secrets list --format=json` output.
/// Handles empty string output (some gcloud versions) and extracts secret names
/// from full resource paths like `projects/123/secrets/MY_SECRET`.
pub fn parse_gcp_list_output(output: &str) -> Result<Vec<String>, SecretsError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let items: Vec<serde_json::Value> =
        serde_json::from_str(trimmed).map_err(|e| SecretsError::ParseError {
            provider: "gcp".to_string(),
            message: e.to_string(),
        })?;

    let mut names: Vec<String> = items
        .iter()
        .filter_map(|i| {
            let name = i.get("name")?.as_str()?;
            // Format: "projects/123/secrets/MY_SECRET" → extract "MY_SECRET"
            name.rsplit('/').next().map(String::from)
        })
        .collect();

    names.sort();
    Ok(names)
}
