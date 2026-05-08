use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, sort_secret_pairs, validate_provider_arg,
    validate_provider_response_label, validate_provider_response_value, validate_secret_name,
    SecretProvider, SecretsError,
};

/// `path` is a filesystem path to a SOPS-encrypted file. We must reject
/// values that would be interpreted as a flag by `sops` (leading `-`).
/// Other characters are allowed since legitimate file paths can contain
/// almost anything; the `--` separator is also used at every call site.
fn validate_sops_path(path: &str) -> Result<(), SecretsError> {
    if path.is_empty() {
        return Err(SecretsError::ProviderError {
            provider: "sops".to_string(),
            message: "path must be a SOPS-encrypted file path".to_string(),
        });
    }
    if path.starts_with('-') {
        return Err(SecretsError::ProviderError {
            provider: "sops".to_string(),
            message: "sops path cannot start with '-' (CLI flag injection)".to_string(),
        });
    }
    if path.contains('\0') || path.contains('\n') || path.contains('\r') {
        return Err(SecretsError::ProviderError {
            provider: "sops".to_string(),
            message: "sops path contains control character".to_string(),
        });
    }
    Ok(())
}

pub struct SopsProvider;

impl SecretProvider for SopsProvider {
    fn name(&self) -> &str {
        "sops"
    }

    fn display_name(&self) -> &str {
        "Mozilla SOPS"
    }

    fn binary_name(&self) -> &str {
        "sops"
    }

    fn install_hint(&self) -> &str {
        "https://github.com/getsops/sops/releases"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("3.8.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec!["key_file"]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["encryption_type"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(key_file) = credentials.get("key_file") {
            env.push(("SOPS_AGE_KEY_FILE", key_file.clone()));
        }
        env
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        validate_sops_path(path)?;

        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli(
            "sops",
            &["decrypt", "--output-type", "json", "--", path],
            &env_refs,
            "sops",
        )?;
        parse_sops_output(&output)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        validate_sops_path(path)?;
        for (k, _) in secrets {
            validate_secret_name(k)?;
        }

        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        // Step 1: Decrypt existing file
        let existing_output = run_cli(
            "sops",
            &["decrypt", "--output-type", "json", "--", path],
            &env_refs,
            "sops",
        );

        let mut existing: HashMap<String, String> = match existing_output {
            Ok(output) => serde_json::from_str(&output).unwrap_or_default(),
            Err(_) => HashMap::new(),
        };

        // Step 2: Merge new secrets
        for (key, value) in secrets {
            existing.insert(key.clone(), value.clone());
        }

        // Step 3: Write merged JSON to temp file and encrypt
        // Use a secure temp file with restrictive permissions to prevent
        // other users from reading decrypted secrets via /tmp
        let temp = tempfile::NamedTempFile::new().map_err(|e| SecretsError::IoError {
            path: std::path::PathBuf::from("/tmp"),
            source: e,
        })?;

        // Set restrictive permissions on the temp file (0600 on Unix)
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

        let json_content =
            serde_json::to_string_pretty(&existing).map_err(|e| SecretsError::ProviderError {
                provider: "sops".to_string(),
                message: format!("failed to serialize: {}", e),
            })?;

        std::fs::write(temp.path(), &json_content).map_err(|e| SecretsError::IoError {
            path: temp.path().to_path_buf(),
            source: e,
        })?;

        // Encrypt the temp file
        let temp_path_str = temp.path().to_string_lossy().to_string();

        let encryption_type = credentials
            .get("encryption_type")
            .map(|s| s.as_str())
            .unwrap_or("age");

        // Extract age public key from key file if using age encryption
        let public_key_str;
        if encryption_type == "age" {
            if let Some(kf) = credentials.get("key_file") {
                let key_content =
                    std::fs::read_to_string(kf).map_err(|e| SecretsError::IoError {
                        path: kf.into(),
                        source: e,
                    })?;
                public_key_str = extract_age_public_key(&key_content)?;
                // Defend against a malicious key_file dropping a value that
                // would be re-parsed as a CLI flag by `sops --age <value>`.
                validate_provider_arg(&public_key_str, "sops age public key")?;
            } else {
                return Err(SecretsError::CredentialError(
                    "age encryption requires 'key_file' credential".to_string(),
                ));
            }
        } else {
            public_key_str = String::new();
        }

        let mut encrypt_args = vec!["encrypt", "--in-place"];
        if encryption_type == "age" {
            encrypt_args.extend_from_slice(&["--age", &public_key_str]);
        }

        encrypt_args.push("--");
        encrypt_args.push(&temp_path_str);
        run_cli("sops", &encrypt_args, &env_refs, "sops")?;

        // Copy encrypted file to target path
        std::fs::copy(temp.path(), path).map_err(|e| SecretsError::IoError {
            path: path.into(),
            source: e,
        })?;

        // Set restrictive permissions on the destination file (0600 on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                std::path::Path::new(path),
                std::fs::Permissions::from_mode(0o600),
            );
        }

        // NamedTempFile auto-deletes on drop, but we clean up explicitly for clarity
        let _ = std::fs::remove_file(temp.path());

        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        validate_sops_path(path)?;
        validate_secret_name(key)?;
        // The key is interpolated into a JSON-path string `["{}"]`. Reject
        // quote characters and backslashes that could break out of the
        // quoted path expression.
        if key.contains(['"', '\\']) {
            return Err(SecretsError::ProviderError {
                provider: "sops".to_string(),
                message: "sops key cannot contain '\"' or '\\\\'".to_string(),
            });
        }

        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let extract_path = format!("[\"{}\"]", key);
        let output = run_cli(
            "sops",
            &["decrypt", "--extract", &extract_path, "--", path],
            &env_refs,
            "sops",
        )?;

        let value = output.trim();
        validate_provider_response_value("sops", value)?;
        Ok(value.to_string())
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

/// Parse decrypted SOPS JSON output into key-value pairs.
/// Filters out the `sops` metadata key if present.
pub fn parse_sops_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let map: HashMap<String, serde_json::Value> =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "sops".to_string(),
            message: e.to_string(),
        })?;

    let mut secrets: Vec<(String, String)> = Vec::new();
    for (k, v) in map {
        if k == "sops" {
            continue;
        }
        let value = match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        };
        validate_provider_response_label("sops", &k)?;
        validate_provider_response_value("sops", &value)?;
        secrets.push((k, value));
    }

    sort_secret_pairs(&mut secrets);
    Ok(secrets)
}

/// Build environment variables for SOPS (public for tests).
pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let provider = SopsProvider;
    provider.build_provider_env(credentials)
}

/// Extract the age public key from a key file content.
/// Reads lines starting with "# public key: " and returns the key string.
fn extract_age_public_key(key_content: &str) -> Result<String, SecretsError> {
    for line in key_content.lines() {
        if let Some(pubkey) = line.strip_prefix("# public key: ") {
            return Ok(pubkey.to_string());
        }
    }
    Err(SecretsError::ProviderError {
        provider: "sops".to_string(),
        message: "No public key found in age key file".to_string(),
    })
}
