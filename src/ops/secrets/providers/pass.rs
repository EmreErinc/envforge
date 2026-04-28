use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::super::provider::{
    env_refs_from_env, run_cli, sort_secret_pairs, SecretProvider, SecretsError,
};

pub struct PassProvider;

impl PassProvider {
    /// Detect which binary to use: prefer gopass, fallback to pass.
    fn detect_binary(credentials: &HashMap<String, String>) -> &'static str {
        if let Some(binary) = credentials.get("binary") {
            if binary == "gopass" {
                return "gopass";
            } else if binary == "pass" {
                return "pass";
            }
        }
        // Auto-detect: check gopass first
        if std::process::Command::new("which")
            .arg("gopass")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "gopass"
        } else {
            "pass"
        }
    }

    /// Get the password store directory.
    fn store_dir(credentials: &HashMap<String, String>) -> PathBuf {
        if let Some(path) = credentials.get("store_path") {
            PathBuf::from(path)
        } else if let Ok(dir) = std::env::var("PASSWORD_STORE_DIR") {
            PathBuf::from(dir)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".password-store")
        }
    }
}

impl SecretProvider for PassProvider {
    fn name(&self) -> &str {
        "pass"
    }

    fn display_name(&self) -> &str {
        "pass/gopass"
    }

    fn binary_name(&self) -> &str {
        "pass"
    }

    fn install_hint(&self) -> &str {
        "https://www.passwordstore.org/ or https://github.com/gopasspw/gopass"
    }

    fn minimum_version(&self) -> Option<&str> {
        Some("1.7.0")
    }

    fn credential_fields(&self) -> Vec<&str> {
        vec![]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["store_path", "binary"]
    }

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(store_path) = credentials.get("store_path") {
            env.push(("PASSWORD_STORE_DIR", store_path.clone()));
        }
        env
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let binary = Self::detect_binary(credentials);
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);
        let store = Self::store_dir(credentials);

        let entries = scan_password_store(&store, path)?;

        let mut secrets = Vec::new();
        for entry in &entries {
            let show_args = if binary == "gopass" {
                vec!["show", "-o", entry.as_str()]
            } else {
                vec!["show", entry.as_str()]
            };

            if let Ok(output) = run_cli(binary, &show_args, &env_refs, "pass") {
                // First line is the secret value
                let value = output.lines().next().unwrap_or("").to_string();
                if !value.is_empty() {
                    secrets.push((entry.clone(), value));
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
        let binary = Self::detect_binary(credentials);
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let mut count = 0;
        for (key, value) in secrets {
            // Use echo pipe approach: echo "value" | pass insert -e key
            let insert_args = if binary == "gopass" {
                // gopass insert -f key (reads from stdin)
                vec!["insert", "-f", key.as_str()]
            } else {
                // pass insert -e key (echo mode, no confirmation)
                vec!["insert", "-e", key.as_str()]
            };

            // Write value to stdin via a temp file + pipe approach using run_cli
            // Since run_cli doesn't support stdin, use Command directly
            let mut cmd = std::process::Command::new(binary);
            cmd.args(&insert_args);
            for (k, v) in &env_refs {
                cmd.env(k, v);
            }
            cmd.stdin(std::process::Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| SecretsError::ProviderError {
                provider: "pass".to_string(),
                message: e.to_string(),
            })?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(value.as_bytes())
                    .map_err(|e| SecretsError::ProviderError {
                        provider: "pass".to_string(),
                        message: format!("failed to write to stdin: {}", e),
                    })?;
                stdin
                    .write_all(b"\n")
                    .map_err(|e| SecretsError::ProviderError {
                        provider: "pass".to_string(),
                        message: format!("failed to write to stdin: {}", e),
                    })?;
            }

            let status = child.wait().map_err(|e| SecretsError::ProviderError {
                provider: "pass".to_string(),
                message: e.to_string(),
            })?;

            if !status.success() {
                return Err(SecretsError::ProviderError {
                    provider: "pass".to_string(),
                    message: format!("failed to insert key '{}'", key),
                });
            }
            count += 1;
        }

        Ok(count)
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        _path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let binary = Self::detect_binary(credentials);
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let show_args = if binary == "gopass" {
            vec!["show", "-o", key]
        } else {
            vec!["show", key]
        };

        let output = run_cli(binary, &show_args, &env_refs, "pass")?;
        Ok(output.lines().next().unwrap_or("").to_string())
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let store = Self::store_dir(credentials);
        scan_password_store(&store, path)
    }
}

/// Scan password store directory for `.gpg` files.
/// Returns relative paths with `.gpg` extension stripped.
pub fn scan_password_store(store_dir: &Path, prefix: &str) -> Result<Vec<String>, SecretsError> {
    if !store_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    scan_dir_recursive(store_dir, store_dir, prefix, &mut entries)?;
    entries.sort();
    Ok(entries)
}

fn scan_dir_recursive(
    base: &Path,
    current: &Path,
    prefix: &str,
    entries: &mut Vec<String>,
) -> Result<(), SecretsError> {
    let read_dir = std::fs::read_dir(current).map_err(|e| SecretsError::IoError {
        path: current.to_path_buf(),
        source: e,
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| SecretsError::IoError {
            path: current.to_path_buf(),
            source: e,
        })?;

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dirs and files
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            scan_dir_recursive(base, &path, prefix, entries)?;
        } else if name.ends_with(".gpg") {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            // Strip .gpg extension
            let key = rel.trim_end_matches(".gpg").to_string();

            // Apply prefix filter
            if prefix.is_empty() || key.starts_with(prefix) {
                entries.push(key);
            }
        }
    }

    Ok(())
}

/// Build environment variables (public for tests).
pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let provider = PassProvider;
    provider.build_provider_env(credentials)
}
