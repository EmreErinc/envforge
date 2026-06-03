use std::collections::HashMap;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

    #[error("binary hash mismatch for '{provider}': binary at '{canonical_path}' has been modified or replaced. Stored hash: {stored_hash}. To accept the new binary, run: envforge secrets config {provider} --pin-hash")]
    BinaryHashMismatch {
        provider: String,
        canonical_path: PathBuf,
        stored_hash: String,
        current_hash: String,
    },

    #[error("credential not configured for '{provider}'. Run: envforge secrets config {provider} --token <value>")]
    CredentialNotFound { provider: String },

    #[error("credential error: {0}")]
    CredentialError(String),

    #[error("cache error: {0}")]
    CacheError(String),

    #[error("I/O error: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("JSON parse error from {provider}: {message}")]
    ParseError { provider: String, message: String },
}

// ─── Credential Encryption Policy ────────────────────────────

/// Mandatory-by-construction credential encryption posture.
///
/// Every provider MUST declare its encryption capability via
/// [`SecretProvider::encryption_mode()`].  There is **no** implicit
/// default — the trait method has no body, so a provider that forgets
/// to implement it will fail to compile.
///
/// `Reporting` (the old permissive variant) has been **deleted**.
/// The only escape hatch is [`NotSupported`] which requires an explicit
/// technical justification string and the name of the reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CredentialEncryptionPolicy {
    /// Encryption is mandatory.  Every call to `store_credential()`
    /// produces `ENC[age:…]` — the credential never touches disk as
    /// plaintext.
    Mandatory,

    /// Encryption is **not** supported by this provider (e.g. the
    /// provider's own storage is already encrypted, or the provider
    /// binary cannot be wrapped by an age layer).
    ///
    /// **Compensating controls are enforced at runtime:**
    /// - Volatile mode is forced ON for every pull.
    /// - Every access is audited at `Critical` severity.
    /// - AI fence blocks plaintext disclosure to agent tools.
    NotSupported {
        /// Technical justification (≥16 characters, must be substantive).
        reason: String,
        /// Name or handle of the security reviewer who approved this exception.
        reviewed_by: Option<String>,
        /// Unix timestamp after which this exception auto-expires and the
        /// provider MUST be re-evaluated for Mandatory support.  0 = no expiry.
        #[serde(default)]
        re_evaluate_after_secs: u64,
    },
}

impl CredentialEncryptionPolicy {
    /// `true` when the provider encrypts all credentials at rest.
    pub fn is_encrypted(&self) -> bool {
        matches!(self, Self::Mandatory)
    }
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
                    field,
                    expired_at,
                    self.name(),
                    field
                )));
            }
        }

        self.check_binary()?;
        self.verify_version()?;
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

    /// Minimum version required for the CLI binary (returns None if unversioned).
    fn minimum_version(&self) -> Option<&str> {
        None
    }

    /// Verify CLI binary version meets minimum requirements.
    /// Returns the detected version string, or a warning if below minimum.
    fn verify_version(&self) -> Result<Option<String>, SecretsError> {
        let binary = self.binary_name();
        let binary_path = resolve_binary_path(binary, self.name())?;
        let output = Command::new(&binary_path)
            .arg("--version")
            .output()
            .map_err(|_| SecretsError::BinaryNotFound {
                binary: binary.to_string(),
                install_hint: self.install_hint().to_string(),
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let version_output = String::from_utf8_lossy(&output.stdout).to_string();
        let detected = version_output.lines().next().unwrap_or("").to_string();

        if let Some(min) = self.minimum_version() {
            if let Some(detected_ver) = extract_version_number(&detected) {
                if detected_ver.as_str() < min {
                    log::warn!(
                        "{}: version {} is below minimum {} — some features may not work",
                        binary,
                        detected_ver,
                        min
                    );
                }
            }
        }

        Ok(Some(detected))
    }

    /// Declare this provider's credential encryption posture.
    ///
    /// **No default implementation** — every provider MUST explicitly
    /// return a [`CredentialEncryptionPolicy`].  The compiler enforces this
    /// at every `impl SecretProvider for …` block.
    fn encryption_mode(&self) -> CredentialEncryptionPolicy;

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

    /// Build provider-specific environment variables from credentials.
    /// Override in provider implementation to set custom env vars.
    /// Default: returns empty vector.
    fn build_provider_env(
        &self,
        _credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
        vec![]
    }

    /// Binary names to verify SHA-256 hashes for.
    /// Default: `[self.binary_name()]`. Override for providers with
    /// dynamic binary selection (e.g., pass uses either "pass" or "gopass").
    fn hash_names(&self) -> Vec<&str> {
        vec![self.binary_name()]
    }

    /// Recommended credential rotation cadence for this provider.
    /// Returns `None` if the provider does not support or require
    /// automated rotation.  Override for providers whose credentials
    /// should be rotated on a schedule.
    fn rotation_policy(&self) -> Option<RotationPolicy> {
        None
    }

    /// GPG public key fingerprint used to verify provider binary
    /// signatures.  Return `None` if the provider does not publish
    /// signed binaries.
    ///
    /// Used by `envforge provider verify` to validate binary integrity
    /// at registration time.  Load-time verification uses SHA-256
    /// hash pinning (see `hash_names()`).
    fn gpg_fingerprint(&self) -> Option<&str> {
        None
    }

    /// URL where the provider's detached signature file (.asc or .sig)
    /// can be downloaded for the currently installed version.
    /// Return `None` if the provider does not publish signatures.
    fn signature_url(&self) -> Option<String> {
        None
    }
}

/// Recommended credential rotation policy for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    /// Recommended rotation interval in days (e.g. 90).
    pub interval_days: u32,
    /// Whether the rotation can be automated by EnvForge.
    pub automatable: bool,
    /// Human-readable instructions for manual rotation.
    #[serde(default)]
    pub instructions: Option<String>,
}

// ─── FD Isolation (T-001 mitigation) ─────────────────────────

/// Set `FD_CLOEXEC` on all non-stdio file descriptors before exec.
///
/// This closes the FD inheritance attack vector (see `.threatmodel/T-001.yaml`).
/// A co-resident attacker (same UID — AI agent, malware) that is the spawned child
/// cannot read EnvForge's open FDs because they are all marked close-on-exec.
///
/// Runs in the child process between fork() and exec(). Only libc calls are safe here
/// (async-signal-safe). fcntl is safe; memory allocation is NOT on Linux (fork),
/// but IS safe on macOS (posix_spawn).
///
/// We iterate FDs 3..max_fd because stdin=0, stdout=1, stderr=2 should remain open.
pub fn close_fds_before_exec() -> std::io::Result<()> {
    let max_fd = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    let max_fd: i32 = if max_fd <= 0 { 1024 } else { max_fd as i32 };

    for fd in 3..max_fd {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags != -1 {
                libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }
    }
    Ok(())
}

/// Attach FD isolation (`close_fds_before_exec`) to a `Command`.
///
/// Call this on every `Command` before `.output()` or `.spawn()` to ensure
/// the child process cannot inherit EnvForge's open file descriptors.
pub fn sandbox_command(cmd: &mut Command) {
    unsafe {
        cmd.pre_exec(close_fds_before_exec);
    }
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

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
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

/// Patterns that may contain credentials in CLI error output.
/// Used by `sanitize_error_output` to redact sensitive values.
const CREDENTIAL_PATTERNS: &[&str] = &[
    "token",
    "api_key",
    "api-key",
    "access_key",
    "access-key",
    "secret_key",
    "secret-key",
    "password",
    "apikey",
    "bearer",
    "authorization",
];

/// Sanitize CLI error output to prevent credential leakage in logs and UI.
/// Redacts lines containing credential-related keywords by replacing the value
/// portion after `=` or `:` with `[REDACTED]`.
pub fn sanitize_error_output(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if CREDENTIAL_PATTERNS.iter().any(|p| lower.contains(p)) {
                // Redact the value portion after = or : delimiters
                if let Some(pos) = line.find('=') {
                    format!("{}=[REDACTED]", &line[..pos])
                } else if let Some(pos) = line.find(':') {
                    format!("{}:[REDACTED]", &line[..pos])
                } else {
                    "[REDACTED]".to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn resolve_binary_path(binary: &str, provider_name: &str) -> Result<PathBuf, SecretsError> {
    if binary.is_empty() || binary.contains('\0') || binary.contains('/') || binary.contains('\\') {
        return Err(SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: format!("invalid binary name: '{}'", binary),
        });
    }

    let output =
        Command::new("which")
            .arg(binary)
            .output()
            .map_err(|_| SecretsError::BinaryNotFound {
                binary: binary.to_string(),
                install_hint: String::new(),
            })?;

    if !output.status.success() {
        return Err(SecretsError::BinaryNotFound {
            binary: binary.to_string(),
            install_hint: String::new(),
        });
    }

    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = Path::new(&path_str);

    if !path.is_file() {
        return Err(SecretsError::BinaryNotFound {
            binary: binary.to_string(),
            install_hint: format!("resolved path '{}' is not a file", path_str),
        });
    }

    let canonical = path
        .canonicalize()
        .map_err(|e| SecretsError::BinaryNotFound {
            binary: binary.to_string(),
            install_hint: format!("cannot canonicalize '{}': {}", path_str, e),
        })?;

    // Verify binary hash if a pinned hash exists for this provider+name.
    // Skipped if no hash stored — hash pinning is opt-in per provider.
    verify_binary_hash(&canonical, binary, provider_name)?;

    Ok(canonical)
}

/// Compute SHA-256 hash of the file at the given path.
/// Returns the hex-encoded hash.
pub fn compute_binary_hash(path: &Path) -> Result<String, SecretsError> {
    let bytes = std::fs::read(path).map_err(|e| SecretsError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash_bytes = hasher.finalize();
    Ok(hex::encode(hash_bytes))
}

/// Verify the binary at `canonical_path` matches the stored hash (if any).
/// Looks up stored hash from the credentials store via `{provider}._meta` →
/// `binary_{name}_sha256`. Returns `Ok(())` if no hash is stored (pin not yet
/// configured) or if the hash matches exactly.
fn verify_binary_hash(
    canonical_path: &Path,
    binary_name: &str,
    provider: &str,
) -> Result<(), SecretsError> {
    let stored = super::credentials::read_binary_hash(provider, binary_name)?;
    let Some(ref stored_hash) = stored else {
        return Ok(());
    };

    let current_hash = compute_binary_hash(canonical_path)?;

    if *stored_hash != current_hash {
        return Err(SecretsError::BinaryHashMismatch {
            provider: provider.to_string(),
            canonical_path: canonical_path.to_path_buf(),
            stored_hash: stored_hash.clone(),
            current_hash,
        });
    }

    Ok(())
}

/// Verify a provider binary against its GPG detached signature.
///
/// Uses the system `gpg` binary.  Falls back gracefully when `gpg` is
/// not installed or the provider does not publish signatures.
pub fn verify_gpg_signature(
    binary: &Path,
    sig_path: &Path,
    expected_fingerprint: &str,
) -> Result<(), SecretsError> {
    let output = Command::new("gpg")
        .args(["--verify", "--status-fd", "1", "--quiet"])
        .arg(sig_path)
        .arg(binary)
        .output()
        .map_err(|_| SecretsError::ProviderError {
            provider: "gpg".into(),
            message: "gpg not found in PATH. Install gnupg to verify provider signatures.".into(),
        })?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if !combined.contains("GOODSIG") {
        return Err(SecretsError::ProviderError {
            provider: "gpg".into(),
            message: format!("GPG signature verification failed for {}", binary.display()),
        });
    }

    if !combined.contains(expected_fingerprint) {
        return Err(SecretsError::ProviderError {
            provider: "gpg".into(),
            message: format!(
                "GPG signature valid but fingerprint mismatch for {}. Expected: {}",
                binary.display(),
                expected_fingerprint
            ),
        });
    }

    Ok(())
}

/// Verify the binary hash for a provider. Resolves the binary via `which`,
/// computes SHA-256, and checks against stored hash (if any).
/// Use this in custom `authenticate()` methods that spawn `Command` directly.
pub fn verify_provider_binary(binary: &str, provider: &str) -> Result<(), SecretsError> {
    let _ = resolve_binary_path(binary, provider)?;
    Ok(())
}

/// Run an external CLI command and return stdout.
fn clear_and_set_provider_env(
    cmd: &mut Command,
    env_vars: &[(&str, &str)],
) -> Result<(), SecretsError> {
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    #[cfg(unix)]
    if let Ok(user) = std::env::var("USER") {
        cmd.env("USER", user);
    }
    cmd.env("HISTFILE", "/dev/null");
    cmd.env("HISTFILESIZE", "0");
    cmd.env("HISTSIZE", "0");
    cmd.env("HISTCONTROL", "ignorespace");
    for (k, v) in env_vars {
        validate_env_pair(k, v)?;
        cmd.env(k, v);
    }
    Ok(())
}

/// Default timeout for provider CLI calls (30 seconds).
const PROVIDER_CLI_TIMEOUT_SECS: u64 = 30;

pub fn run_cli(
    binary: &str,
    args: &[&str],
    env_vars: &[(&str, &str)],
    provider_name: &str,
) -> Result<String, SecretsError> {
    let binary_path = resolve_binary_path(binary, provider_name)?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    clear_and_set_provider_env(&mut cmd, env_vars)?;
    sandbox_command(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| {
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

    let status = wait_timeout_or_kill(&mut child, PROVIDER_CLI_TIMEOUT_SECS, provider_name)?;

    let output = child
        .wait_with_output()
        .map_err(|e| SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: e.to_string(),
        })?;

    if status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = sanitize_error_output(&String::from_utf8_lossy(&output.stderr));
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

fn wait_timeout_or_kill(
    child: &mut std::process::Child,
    timeout_secs: u64,
    provider_name: &str,
) -> Result<std::process::ExitStatus, SecretsError> {
    let dur = std::time::Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if start.elapsed() >= dur {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(SecretsError::ProviderError {
                        provider: provider_name.to_string(),
                        message: format!(
                            "command timed out after {}s and was killed",
                            timeout_secs
                        ),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                return Err(SecretsError::ProviderError {
                    provider: provider_name.to_string(),
                    message: format!("failed to wait on child process: {}", e),
                });
            }
        }
    }
}

// ─── Secure CLI Runners (No /proc Leakage) ───────────────────

/// Run an external CLI with secret data piped via stdin.
/// Prevents /proc/PID/cmdline leakage of secret values by avoiding CLI arguments.
/// Same error handling as `run_cli()`: binary validation, auth-failed detection,
/// and credential sanitization in stderr.
pub fn run_cli_with_stdin(
    binary: &str,
    args: &[&str],
    stdin_data: &[u8],
    env_vars: &[(&str, &str)],
    provider_name: &str,
) -> Result<String, SecretsError> {
    let binary_path = resolve_binary_path(binary, provider_name)?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(args);
    clear_and_set_provider_env(&mut cmd, env_vars)?;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sandbox_command(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| {
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

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_data)
            .map_err(|e| SecretsError::ProviderError {
                provider: provider_name.to_string(),
                message: format!("failed to write to stdin: {}", e),
            })?;
    }

    let status = wait_timeout_or_kill(&mut child, PROVIDER_CLI_TIMEOUT_SECS, provider_name)?;

    let output = child
        .wait_with_output()
        .map_err(|e| SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: e.to_string(),
        })?;

    if status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = sanitize_error_output(&String::from_utf8_lossy(&output.stderr));
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

/// Run an external CLI with a secret value written to a temporary file.
/// Prevents /proc/PID/cmdline leakage by replacing secret values in CLI args
/// with a path to a 0600 tempfile. The caller uses `__TEMP__` as a placeholder
/// in the args array; this function replaces it with the actual tempfile path.
/// Tempfile is auto-deleted on drop (NamedTempFile behavior).
pub fn run_cli_with_tempfile(
    binary: &str,
    args: &[&str],
    secret_value: &str,
    env_vars: &[(&str, &str)],
    provider_name: &str,
) -> Result<String, SecretsError> {
    if binary.contains('\0') || binary.contains('/') || binary.contains('\\') {
        return Err(SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: format!("invalid binary name: '{}'", binary),
        });
    }

    #[allow(unused_mut)]
    let mut temp = tempfile::NamedTempFile::new().map_err(|e| SecretsError::IoError {
        path: PathBuf::from("/tmp"),
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

    temp.write_all(secret_value.as_bytes())
        .map_err(|e| SecretsError::IoError {
            path: temp.path().to_path_buf(),
            source: e,
        })?;
    temp.flush().map_err(|e| SecretsError::IoError {
        path: temp.path().to_path_buf(),
        source: e,
    })?;

    let temp_path = temp.path().to_string_lossy().to_string();
    let final_args: Vec<String> = args
        .iter()
        .map(|a| {
            if *a == "__TEMP__" {
                temp_path.clone()
            } else {
                (*a).to_string()
            }
        })
        .collect();
    let final_refs: Vec<&str> = final_args.iter().map(|s| s.as_str()).collect();

    let result = run_cli(binary, &final_refs, env_vars, provider_name);

    // Explicit cleanup (NamedTempFile does this on drop, but we ensure it here)
    let _ = std::fs::remove_file(temp.path());

    result
}

/// Write a batch of KEY=VALUE pairs to a 0600 tempfile, then call CLI.
/// Content format: one "KEY=VALUE" per line. Used by providers that batch
/// multiple secrets in a single CLI call (doppler, infisical).
/// Each key and value is validated before writing.
pub fn run_cli_with_tempfile_batch(
    binary: &str,
    args: &[&str],
    secrets: &[(String, String)],
    env_vars: &[(&str, &str)],
    provider_name: &str,
) -> Result<String, SecretsError> {
    if binary.contains('\0') || binary.contains('/') || binary.contains('\\') {
        return Err(SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: format!("invalid binary name: '{}'", binary),
        });
    }

    for (key, value) in secrets {
        validate_secret_name(key)?;
        validate_secret_value(value)?;
    }

    let mut content = String::new();
    for (key, value) in secrets {
        content.push_str(&format!("{}={}\n", key, value));
    }

    #[allow(unused_mut)]
    let mut temp = tempfile::NamedTempFile::new().map_err(|e| SecretsError::IoError {
        path: PathBuf::from("/tmp"),
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

    temp.write_all(content.as_bytes())
        .map_err(|e| SecretsError::IoError {
            path: temp.path().to_path_buf(),
            source: e,
        })?;
    temp.flush().map_err(|e| SecretsError::IoError {
        path: temp.path().to_path_buf(),
        source: e,
    })?;

    let temp_path = temp.path().to_string_lossy().to_string();
    let final_args: Vec<String> = args
        .iter()
        .map(|a| {
            if *a == "__TEMP__" {
                temp_path.clone()
            } else {
                (*a).to_string()
            }
        })
        .collect();
    let final_refs: Vec<&str> = final_args.iter().map(|s| s.as_str()).collect();

    let result = run_cli(binary, &final_refs, env_vars, provider_name);

    let _ = std::fs::remove_file(temp.path());

    result
}

// ─── Provider Helper Utilities ───────────────────────────────

/// Convert environment vector to reference vector for Command::env().
/// This helper eliminates repeated `.map(|(k, v)| (*k, v.as_str())).collect()` patterns
/// across all providers.
pub fn env_refs_from_env<'a>(env: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    env.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

/// Validate a single env-var pair before it is passed to `Command::env`.
/// Names must be a non-empty ASCII identifier, and neither name nor value
/// may contain a NUL byte (which would either panic on Unix or terminate
/// the value silently on some platforms). Newlines are also rejected to
/// prevent log/output smuggling through env-derived diagnostics.
pub fn validate_env_pair(name: &str, value: &str) -> Result<(), SecretsError> {
    if name.is_empty() {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: "env var name cannot be empty".to_string(),
        });
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => {
            return Err(SecretsError::ProviderError {
                provider: "input".to_string(),
                message: format!("invalid env var name: '{}'", name),
            });
        }
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(SecretsError::ProviderError {
                provider: "input".to_string(),
                message: format!("invalid env var name: '{}'", name),
            });
        }
    }
    if name.contains('\0') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: "env var name contains NUL byte".to_string(),
        });
    }
    if value.contains('\0') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("env var '{}' value contains NUL byte", name),
        });
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("env var '{}' value contains newline", name),
        });
    }
    Ok(())
}

/// Extract the first semver-like version number from a version string.
/// E.g., "vault v1.15.4" → Some("1.15.4"), "bws 2024.1.0" → Some("2024.1.0")
pub fn extract_version_number(output: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
    re.captures(output)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Validate that a secret name is safe for use in CLI arguments.
/// Rejects names containing null bytes, newlines, or other characters that
/// could cause unexpected behavior when passed to external CLI binaries.
pub fn validate_secret_name(name: &str) -> Result<(), SecretsError> {
    if name.is_empty() {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: "secret name cannot be empty".to_string(),
        });
    }
    if name.len() > 512 {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("secret name too long ({} chars, max 512)", name.len()),
        });
    }
    // Block leading dash to prevent the value from being parsed by a
    // downstream provider CLI as an option flag (argv smuggling).
    if name.starts_with('-') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: "secret name cannot start with '-' (would be parsed as a CLI flag)"
                .to_string(),
        });
    }
    for ch in name.chars() {
        match ch {
            '\0' => {
                return Err(SecretsError::ProviderError {
                    provider: "input".to_string(),
                    message: "secret name contains null byte".to_string(),
                });
            }
            '\n' | '\r' => {
                return Err(SecretsError::ProviderError {
                    provider: "input".to_string(),
                    message: "secret name contains newline character".to_string(),
                });
            }
            // Reject other ASCII control characters (tab, BEL, ESC, DEL, ...).
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                return Err(SecretsError::ProviderError {
                    provider: "input".to_string(),
                    message: "secret name contains control character".to_string(),
                });
            }
            // `=` would let a key like `foo=bar` poison `--field=label=KEY`
            // style flags by introducing extra `=` segments.
            '=' => {
                return Err(SecretsError::ProviderError {
                    provider: "input".to_string(),
                    message: "secret name cannot contain '='".to_string(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Maximum byte length for a single secret value parsed from a provider's
/// CLI output. Anything bigger is almost certainly garbage / hostile.
pub const MAX_PROVIDER_VALUE_LEN: usize = 64 * 1024;

/// Maximum byte length for a label/key parsed from a provider's CLI output.
pub const MAX_PROVIDER_LABEL_LEN: usize = 512;

/// Reject obviously hostile or malformed values returned from a provider
/// CLI's JSON output before they propagate into ENV exports / shell.
///
/// Returns `Err` if the value exceeds [`MAX_PROVIDER_VALUE_LEN`] bytes
/// or contains a NUL byte. Other ASCII control characters are allowed
/// (multi-line tokens are legitimate) but callers may strip them if they
/// will be exported to the shell.
pub fn validate_provider_response_value(provider: &str, value: &str) -> Result<(), SecretsError> {
    if value.len() > MAX_PROVIDER_VALUE_LEN {
        return Err(SecretsError::ParseError {
            provider: provider.to_string(),
            message: format!(
                "secret value exceeds {} bytes; refusing to import",
                MAX_PROVIDER_VALUE_LEN
            ),
        });
    }
    if value.contains('\0') {
        return Err(SecretsError::ParseError {
            provider: provider.to_string(),
            message: "secret value contains NUL byte".to_string(),
        });
    }
    Ok(())
}

/// Reject malformed labels/keys parsed from a provider's CLI output.
pub fn validate_provider_response_label(provider: &str, label: &str) -> Result<(), SecretsError> {
    if label.is_empty() {
        return Err(SecretsError::ParseError {
            provider: provider.to_string(),
            message: "empty secret label in provider response".to_string(),
        });
    }
    if label.len() > MAX_PROVIDER_LABEL_LEN {
        return Err(SecretsError::ParseError {
            provider: provider.to_string(),
            message: format!("secret label exceeds {} bytes", MAX_PROVIDER_LABEL_LEN),
        });
    }
    for ch in label.chars() {
        if ch == '\0' || ch == '\n' || ch == '\r' || (ch as u32) < 0x20 || ch as u32 == 0x7f {
            return Err(SecretsError::ParseError {
                provider: provider.to_string(),
                message: "secret label contains control character".to_string(),
            });
        }
    }
    Ok(())
}

/// Validate a value used as a positional argument to a provider CLI.
/// Same hardening as `validate_secret_name`, applied to paths/items/etc.
pub fn validate_provider_arg(arg: &str, what: &str) -> Result<(), SecretsError> {
    if arg.is_empty() {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("{} cannot be empty", what),
        });
    }
    if arg.len() > 1024 {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("{} too long ({} chars, max 1024)", what, arg.len()),
        });
    }
    if arg.starts_with('-') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: format!("{} cannot start with '-' (CLI flag injection)", what),
        });
    }
    for ch in arg.chars() {
        if ch == '\0' || ch == '\n' || ch == '\r' || (ch as u32) < 0x20 || ch as u32 == 0x7f {
            return Err(SecretsError::ProviderError {
                provider: "input".to_string(),
                message: format!("{} contains control character", what),
            });
        }
    }
    Ok(())
}

/// Validate that a secret value is safe for use in CLI arguments.
/// Rejects null bytes which could truncate the value unexpectedly.
pub fn validate_secret_value(value: &str) -> Result<(), SecretsError> {
    if value.contains('\0') {
        return Err(SecretsError::ProviderError {
            provider: "input".to_string(),
            message: "secret value contains null byte".to_string(),
        });
    }
    Ok(())
}

/// Sort secrets by key for consistent output across providers.
/// All providers use identical sorting logic; this helper ensures consistency.
pub fn sort_secret_pairs(secrets: &mut [(String, String)]) {
    secrets.sort_by(|a, b| a.0.cmp(&b.0));
}

/// Extract and parse JSON object into a HashMap<String, String>.
/// Filters out system keys (those starting with underscore) and handles nested structures.
/// Provider-specific JSON extraction logic should use this utility.
pub fn extract_json_object(
    json: &serde_json::Value,
    path: &[&str],
    provider: &str,
) -> Result<Vec<(String, String)>, SecretsError> {
    let mut current = json;
    for key in path {
        current = &current[key];
        if current.is_null() {
            return Err(SecretsError::ParseError {
                provider: provider.to_string(),
                message: format!("missing key in path: {}", key),
            });
        }
    }

    let obj = current
        .as_object()
        .ok_or_else(|| SecretsError::ParseError {
            provider: provider.to_string(),
            message: "expected object at path".to_string(),
        })?;

    let mut secrets: Vec<(String, String)> = obj
        .iter()
        .filter(|(k, _)| !k.starts_with('_')) // Filter system keys
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    sort_secret_pairs(&mut secrets);
    Ok(secrets)
}

/// Parse JSON response and return sorted secrets.
/// This is the standard parsing pipeline used by all providers.
pub fn parse_json_secrets(
    output: &str,
    path: &[&str],
    provider: &str,
) -> Result<Vec<(String, String)>, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: provider.to_string(),
            message: format!("JSON parse error: {}", e),
        })?;

    extract_json_object(&json, path, provider)
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
        fn encryption_mode(&self) -> CredentialEncryptionPolicy {
            CredentialEncryptionPolicy::Mandatory
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
        let Err(SecretsError::ProviderNotFound { name, .. }) = result else {
            panic!("expected ProviderNotFound");
        };
        assert_eq!(name, "nonexistent");
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
