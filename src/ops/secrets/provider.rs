use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

// ─── Error Types ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("provider '{name}' not found. Available: {available}")]
    ProviderNotFound { name: String, available: String },

    #[error("{binary} not found in PATH. Install: {install_hint}")]
    BinaryNotFound {
        binary: String,
        install_hint: String,
    },

    #[error("authentication failed for {provider}: {message}")]
    AuthFailed { provider: String, message: String },

    #[error("provider error ({provider}): {message}")]
    ProviderError { provider: String, message: String },

    #[error("credential not configured for '{provider}'. Run: envforge secrets config {provider} --token <value>")]
    CredentialNotFound { provider: String },

    #[error("credential error: {0}")]
    CredentialError(String),

    #[error("cache error: {0}")]
    CacheError(String),

    #[error("I/O error at '{path}': {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("JSON parse error from {provider}: {message}")]
    ParseError { provider: String, message: String },
}

// ─── Secret Provider Trait ───────────────────────────────────

/// Common trait for all secret manager providers.
pub trait SecretProvider: Send + Sync {
    /// Provider name (lowercase kebab-case, e.g., "vault", "aws-ssm").
    fn name(&self) -> &str;

    /// Display name for UI (e.g., "HashiCorp Vault").
    fn display_name(&self) -> &str;

    /// CLI binary name (e.g., "vault", "aws", "op").
    fn binary_name(&self) -> &str;

    /// Install hint URL or command.
    fn install_hint(&self) -> &str;

    /// Required credential field names (e.g., ["token"], ["access_key", "secret_key"]).
    fn credential_fields(&self) -> Vec<&str>;

    /// Optional credential field names (e.g., ["region", "profile"]).
    fn optional_credential_fields(&self) -> Vec<&str> {
        vec![]
    }

    /// Validate that all required credentials are present.
    fn validate_config(&self, credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        let missing: Vec<&str> = self
            .credential_fields()
            .into_iter()
            .filter(|f| !credentials.contains_key(*f))
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(SecretsError::CredentialError(format!(
                "missing required fields for '{}': {}. Run: {}",
                self.name(),
                missing.join(", "),
                missing
                    .iter()
                    .map(|f| format!(
                        "envforge secrets config {} --set {}=<value>",
                        self.name(),
                        f
                    ))
                    .collect::<Vec<_>>()
                    .join(" && ")
            )))
        }
    }

    /// Authenticate with the provider. Validates credentials, checks for expired tokens,
    /// and verifies binary availability.
    fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        self.validate_config(credentials)?;

        // Check for expired credentials
        for field in self.credential_fields() {
            if let Ok(Some(expired_at)) = super::credentials::check_expiry(self.name(), field) {
                return Err(SecretsError::CredentialError(format!(
                    "Token '{}' expired at {}. Run: envforge secrets config {} --set {}=<value>",
                    field, expired_at, self.name(), field
                )));
            }
        }

        self.check_binary()?;
        Ok(())
    }

    /// Check if the provider CLI binary is available.
    fn check_binary(&self) -> Result<String, SecretsError> {
        let output = Command::new("which")
            .arg(self.binary_name())
            .output()
            .map_err(|_| SecretsError::BinaryNotFound {
                binary: self.binary_name().to_string(),
                install_hint: self.install_hint().to_string(),
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(SecretsError::BinaryNotFound {
                binary: self.binary_name().to_string(),
                install_hint: self.install_hint().to_string(),
            })
        }
    }

    /// Pull secrets from the provider at the given path.
    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError>;

    /// Push secrets to the provider at the given path.
    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError>;

    /// Get a single secret value by key.
    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError>;

    /// List available secret keys at the given path.
    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError>;
}

// ─── Provider Registry ───────────────────────────────────────

/// Runtime registry of available secret providers.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn SecretProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn SecretProvider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// Get a provider by name. Suggests similar names if not found.
    pub fn get(&self, name: &str) -> Result<&dyn SecretProvider, SecretsError> {
        self.providers.get(name).map(|p| p.as_ref()).ok_or_else(|| {
            let suggestion = self.suggest_similar(name);
            let available = match suggestion {
                Some(s) => format!(
                    "did you mean '{}'? Available: {}",
                    s,
                    self.list_names().join(", ")
                ),
                None => format!("Available: {}", self.list_names().join(", ")),
            };
            SecretsError::ProviderNotFound {
                name: name.to_string(),
                available,
            }
        })
    }

    /// Find a similar provider name (simple Levenshtein-like matching).
    fn suggest_similar(&self, name: &str) -> Option<String> {
        self.providers
            .keys()
            .filter(|k| {
                // Simple heuristic: shared prefix >= 2 chars or contains
                k.contains(name)
                    || name.contains(k.as_str())
                    || (k.len() >= 2 && name.len() >= 2 && k[..2] == name[..2])
            })
            .min_by_key(|k| {
                // Prefer shorter edit distance (approximate with length diff)
                (k.len() as i32 - name.len() as i32).unsigned_abs()
            })
            .cloned()
    }

    /// List all registered provider names.
    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        names.sort();
        names
    }

    /// List providers with their status.
    pub fn list_with_status(&self) -> Vec<ProviderStatus> {
        let mut statuses: Vec<ProviderStatus> = self
            .providers
            .values()
            .map(|p| {
                let binary_found = p.check_binary().is_ok();
                ProviderStatus {
                    name: p.name().to_string(),
                    display_name: p.display_name().to_string(),
                    binary_name: p.binary_name().to_string(),
                    binary_found,
                }
            })
            .collect();
        statuses.sort_by(|a, b| a.name.cmp(&b.name));
        statuses
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a single provider.
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub display_name: String,
    pub binary_name: String,
    pub binary_found: bool,
}

// ─── Helper: Run CLI Command ─────────────────────────────────

/// Run an external CLI command and return stdout.
pub fn run_cli(
    binary: &str,
    args: &[&str],
    env_vars: &[(&str, &str)],
    provider_name: &str,
) -> Result<String, SecretsError> {
    let mut cmd = Command::new(binary);
    cmd.args(args);
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SecretsError::BinaryNotFound {
                binary: binary.to_string(),
                install_hint: String::new(),
            }
        } else {
            SecretsError::ProviderError {
                provider: provider_name.to_string(),
                message: e.to_string(),
            }
        }
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("permission")
            || stderr.contains("denied")
            || stderr.contains("unauthorized")
            || stderr.contains("403")
            || stderr.contains("401")
        {
            Err(SecretsError::AuthFailed {
                provider: provider_name.to_string(),
                message: stderr,
            })
        } else {
            Err(SecretsError::ProviderError {
                provider: provider_name.to_string(),
                message: stderr,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;

    impl SecretProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn display_name(&self) -> &str {
            "Mock Provider"
        }
        fn binary_name(&self) -> &str {
            "echo"
        }
        fn install_hint(&self) -> &str {
            "builtin"
        }
        fn credential_fields(&self) -> Vec<&str> {
            vec!["token"]
        }
        fn pull(
            &self,
            _creds: &HashMap<String, String>,
            _path: &str,
        ) -> Result<Vec<(String, String)>, SecretsError> {
            Ok(vec![("KEY".to_string(), "VALUE".to_string())])
        }
        fn push(
            &self,
            _creds: &HashMap<String, String>,
            _path: &str,
            secrets: &[(String, String)],
        ) -> Result<usize, SecretsError> {
            Ok(secrets.len())
        }
        fn get(
            &self,
            _creds: &HashMap<String, String>,
            _path: &str,
            _key: &str,
        ) -> Result<String, SecretsError> {
            Ok("VALUE".to_string())
        }
        fn list(
            &self,
            _creds: &HashMap<String, String>,
            _path: &str,
        ) -> Result<Vec<String>, SecretsError> {
            Ok(vec!["KEY".to_string()])
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider));

        let provider = registry.get("mock").unwrap();
        assert_eq!(provider.name(), "mock");
        assert_eq!(provider.display_name(), "Mock Provider");
    }

    #[test]
    fn test_registry_not_found() {
        let registry = ProviderRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_err());
        match result {
            Err(SecretsError::ProviderNotFound { name, .. }) => assert_eq!(name, "nonexistent"),
            _ => panic!("expected ProviderNotFound"),
        }
    }

    #[test]
    fn test_registry_list_names() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider));
        assert_eq!(registry.list_names(), vec!["mock"]);
    }

    #[test]
    fn test_mock_provider_pull() {
        let provider = MockProvider;
        let creds = HashMap::new();
        let result = provider.pull(&creds, "path").unwrap();
        assert_eq!(result, vec![("KEY".to_string(), "VALUE".to_string())]);
    }

    #[test]
    fn test_mock_provider_push() {
        let provider = MockProvider;
        let creds = HashMap::new();
        let secrets = vec![("A".to_string(), "B".to_string())];
        let count = provider.push(&creds, "path", &secrets).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_check_binary_echo() {
        let provider = MockProvider;
        let result = provider.check_binary();
        assert!(result.is_ok()); // echo should be available everywhere
    }
}
