use std::collections::HashMap;

use super::super::provider::{
    run_cli, sort_secret_pairs, validate_provider_arg, validate_provider_response_label,
    validate_provider_response_value, validate_secret_name, CredentialEncryptionPolicy,
    SecretProvider, SecretsError,
};

/// Validate `project_id` from credentials before it is interpolated into a
/// `--project=` flag. Refuses leading dash / control chars / `=` smuggling.
fn checked_project(credentials: &HashMap<String, String>) -> Result<String, SecretsError> {
    let project = credentials.get("project_id").cloned().unwrap_or_default();
    if !project.is_empty() {
        validate_provider_arg(&project, "gcp project_id")?;
    }
    Ok(project)
}

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

    fn encryption_mode(&self) -> CredentialEncryptionPolicy {
        CredentialEncryptionPolicy::Mandatory
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

        let project = checked_project(credentials)?;

        for key in &keys {
            // Keys came from `list` which already validated; re-check is cheap.
            validate_secret_name(key)?;
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
            let trimmed = output.trim();
            validate_provider_response_value("gcp", trimmed)?;
            result.push((key.clone(), trimmed.to_string()));
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
        let project = checked_project(credentials)?;

        for (key, value) in secrets {
            validate_secret_name(key)?;
            // `value` is written to a 0600 tempfile and passed via
            // `--data-file=`; not interpreted as an argv value.
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

            // Add version — use temp file with restrictive permissions
            let temp = tempfile::NamedTempFile::new().map_err(|e| SecretsError::IoError {
                path: std::path::PathBuf::from("/tmp"),
                source: e,
            })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                temp.as_file()
                    .set_permissions(std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| SecretsError::IoError {
                        path: temp.path().to_path_buf(),
                        source: e,
                    })?;
            }

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

            // Check create error: only suppress "already exists", propagate others
            match create_result {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string().to_lowercase();
                    if !msg.contains("already exists") && !msg.contains("alreadyexist") {
                        return Err(e);
                    }
                }
            }
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
        let project = checked_project(credentials)?;

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

        let value = output.trim();
        validate_provider_response_value("gcp", value)?;
        Ok(value.to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let project = checked_project(credentials)?;

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

    let mut names: Vec<String> = Vec::new();
    for i in &items {
        let name = match i.get("name").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        // Format: "projects/123/secrets/MY_SECRET" → extract "MY_SECRET"
        if let Some(short) = name.rsplit('/').next() {
            validate_provider_response_label("gcp", short)?;
            names.push(short.to_string());
        }
    }

    names.sort();
    Ok(names)
}
