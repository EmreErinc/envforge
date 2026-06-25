use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ─── Sync Encryption Policy ──────────────────────────────────

/// Encryption posture for cross-machine sync snapshots.
///
/// Replaces the old `require_encryption: bool` (which was a downgrade
/// attack surface — an attacker who controls the sync config file could
/// flip it to `false` and push plaintext snapshots).  The sum type makes
/// the intent explicit and compiler-verifiable.
///
/// **Backward compatibility:** Old boolean values (`true`/`false`) in
/// TOML configs are accepted via custom deserializer and mapped to the
/// equivalent variant.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SyncEncryptionPolicy {
    /// Encryption is mandatory — plaintext (unencrypted) snapshot payloads
    /// are rejected with [`SyncError::EncryptionRequired`].
    #[default]
    Mandatory,

    /// Encryption is required but temporarily relaxed for migration from
    /// pre-encryption snapshots.  The policy **auto-reverts to Mandatory**
    /// after the ISO-8601 datetime, preventing the "permanent bypass" bug.
    MigrationUntil(String),
}

impl serde::Serialize for SyncEncryptionPolicy {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Mandatory => serializer.serialize_str("mandatory"),
            Self::MigrationUntil(datetime) => {
                serializer.serialize_str(&format!("migration-until {}", datetime))
            }
        }
    }
}

impl<'de> Deserialize<'de> for SyncEncryptionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct PolicyVisitor;

        impl de::Visitor<'_> for PolicyVisitor {
            type Value = SyncEncryptionPolicy;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a boolean or a string (\"mandatory\" | \"migration-until\")")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                // Old format: require_encryption = true/false
                if v {
                    Ok(SyncEncryptionPolicy::Mandatory)
                } else {
                    // Legacy `require_encryption = false` previously mapped to a
                    // year-2099 window — an effectively permanent plaintext bypass that
                    // fails *open*, and which a relative window can't safely bound (no
                    // date anchor exists in the legacy bool). Treat the legacy opt-out
                    // as Mandatory (fail-safe). Operators needing a real, bounded
                    // migration window must declare it explicitly as
                    // `migration-until <RFC3339 date>`; the `--force-migration` flag
                    // remains for audited, explicit bypass.
                    log::warn!(
                        "sync: legacy 'require_encryption = false' is ignored and treated as \
                         mandatory encryption. Use encryption_policy = \
                         \"migration-until <RFC3339 date>\" for an explicit, bounded \
                         plaintext window."
                    );
                    Ok(SyncEncryptionPolicy::Mandatory)
                }
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match v {
                    "mandatory" => Ok(SyncEncryptionPolicy::Mandatory),
                    s if s.starts_with("migration-until") => {
                        // "migration-until <datetime>"
                        let datetime = s
                            .strip_prefix("migration-until")
                            .unwrap_or(s)
                            .trim()
                            .to_string();
                        Ok(SyncEncryptionPolicy::MigrationUntil(datetime))
                    }
                    _ => Err(de::Error::unknown_variant(
                        v,
                        &["mandatory", "migration-until"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(PolicyVisitor)
    }
}

impl SyncEncryptionPolicy {
    /// `true` when encryption is currently required.
    ///
    /// When `force_migration` is `true`, the operator has explicitly
    /// requested a bypass (via `--force-migration` CLI flag).  This
    /// should only be used during migration windows and is audited.
    pub fn is_required(&self) -> bool {
        match self {
            Self::Mandatory => true,
            Self::MigrationUntil(until) => {
                // If the datetime string can be parsed and is in the past,
                // encryption is now required.
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) {
                    dt <= chrono::Utc::now()
                } else {
                    // Unparseable datetime — treat as Mandatory (fail-safe).
                    true
                }
            }
        }
    }

    /// `true` when encryption is currently required, respecting an
    /// explicit `--force-migration` operator override.
    pub fn is_required_with_override(&self, force_migration: bool) -> bool {
        if force_migration {
            log::warn!(
                "sync encryption policy bypassed with --force-migration. \
                 Re-enable Mandatory as soon as migration is complete."
            );
            return false;
        }
        self.is_required()
    }
}

// ─── Error Types ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("git not found in PATH. Install git >= 2.28: https://git-scm.com")]
    GitNotFound,

    #[error("git version {found} is too old. Minimum required: {required}")]
    GitVersionTooOld { found: String, required: String },

    #[error("git command failed: {command}")]
    GitCommandFailed { command: String, stderr: String },

    #[error("push rejected: remote has changes. Run `envforge sync pull` first")]
    PushRejected,

    #[error("pull conflict: {files:?} have merge conflicts")]
    PullConflict { files: Vec<String> },

    #[error("authentication failed for remote. Check SSH keys or access token")]
    AuthFailed,

    #[error("network timeout after {seconds}s. Check connection and retry")]
    NetworkTimeout { seconds: u64 },

    #[error("sync not initialized. Run `envforge sync init` first")]
    RepoNotInitialized,

    #[error("sync already initialized at {path}. Use --force to reinitialize")]
    RepoAlreadyInitialized { path: PathBuf },

    #[error("failed to parse snapshot: {message}")]
    SnapshotParseError { message: String },

    #[error("failed to parse sync config: {message}")]
    ConfigParseError { message: String },

    #[error("I/O error: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "invalid machine ID '{id}': must contain only lowercase alphanumeric characters and dashes"
    )]
    InvalidMachineId { id: String },

    #[error("no keys marked for sync. Use `envforge sync mark` to select keys")]
    NoKeysMarked,

    #[error("nothing to sync — local state matches snapshot")]
    NothingToSync,

    #[error("key '{key}' not found in environment")]
    KeyNotFound { key: String },

    #[error("pattern '{pattern}' matched no keys")]
    PatternMatchesNothing { pattern: String },

    #[error("Cannot decrypt sync data. Key mismatch or corrupted.")]
    DecryptionFailed,

    #[error("encryption failed: {message}")]
    EncryptionFailed { message: String },

    #[error("sync.encryption_policy is Mandatory but received plaintext snapshot")]
    EncryptionRequired,
}

// ─── Snapshot Types ──────────────────────────────────────────

/// Portable ENV state export — the core sync data structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncSnapshot {
    pub metadata: SnapshotMeta,
    #[serde(default)]
    pub entries: Vec<SyncEntry>,
}

/// Snapshot metadata for versioning and provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotMeta {
    pub version: u32,
    pub created_at: String,
    pub created_by: String,
}

/// A single synced environment variable entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncEntry {
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

// ─── Config Types ────────────────────────────────────────────

/// Sync configuration stored in sync-config.toml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncConfig {
    pub sync: SyncSettings,
    #[serde(default)]
    pub manifest: ManifestConfig,
}

/// Core sync settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncSettings {
    pub machine_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub default_sync: bool,
    #[serde(default)]
    pub auto_push: bool,
    #[serde(default = "default_conflict_strategy")]
    pub conflict_strategy: ConflictStrategy,
    #[serde(default = "default_encrypted")]
    pub encrypted: bool,

    /// Encryption policy for sync snapshots.
    ///
    /// **Default:** `Mandatory` — plaintext snapshots are rejected.
    /// The old `require_encryption: true/false` bool is accepted via
    /// serde alias for backward compatibility.
    ///
    /// `MigrationUntil("2026-07-01T00:00:00Z")` allows plaintext
    /// snapshots only until the given UTC datetime, after which
    /// Mandatory enforcement auto-activates.  This prevents the
    /// "permanent bypass" bug where a migration flag is never re-enabled.
    #[serde(default, alias = "require_encryption")]
    pub encryption_policy: SyncEncryptionPolicy,

    /// If true, every pulled commit must carry a verifiable git signature
    /// (`git verify-commit HEAD`). Pulls fail closed if the check fails.
    /// Default false for backwards compatibility; recommended on for
    /// untrusted-remote scenarios.
    #[serde(default)]
    pub verify_signatures: bool,

    /// If true, only SSH-based remote URLs are accepted. HTTP/HTTPS URLs
    /// are rejected. Closes the MITM-on-HTTP transport vector (T-005).
    /// Default false for backwards compatibility.
    #[serde(default)]
    pub enforce_ssh: bool,
}

fn default_encrypted() -> bool {
    true
}

/// Strategy for resolving sync conflicts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictStrategy {
    Ask,
    KeepLocal,
    KeepRemote,
}

fn default_conflict_strategy() -> ConflictStrategy {
    ConflictStrategy::Ask
}

/// Manifest tracking which keys are synced vs local-only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ManifestConfig {
    #[serde(default)]
    pub sync_keys: Vec<String>,
    #[serde(default)]
    pub local_keys: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<GlobPattern>,
}

/// A glob pattern for bulk sync/local marking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobPattern {
    pub pattern: String,
    pub sync: bool,
}

// ─── Git Types ───────────────────────────────────────────────

/// Parsed git version.
#[derive(Debug, Clone, PartialEq)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Minimum required git version for EnvForge sync.
    pub const MINIMUM: GitVersion = GitVersion {
        major: 2,
        minor: 28,
        patch: 0,
    };

    pub fn meets_minimum(&self) -> bool {
        (self.major, self.minor, self.patch)
            >= (
                Self::MINIMUM.major,
                Self::MINIMUM.minor,
                Self::MINIMUM.patch,
            )
    }
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A single git commit entry from log.
#[derive(Debug, Clone, PartialEq)]
pub struct GitCommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub date: String,
    pub message: String,
    pub author: String,
}

/// Result of a git pull operation.
#[derive(Debug, Clone, PartialEq)]
pub enum PullResult {
    UpToDate,
    Updated,
    Conflict { files: Vec<String> },
}

/// Result of a git push operation.
#[derive(Debug, Clone, PartialEq)]
pub enum PushResult {
    Success,
    Rejected,
    NoRemote,
}

/// Status of a single file in git working tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStatus {
    pub path: String,
    pub status: FileStatusKind,
}

/// Kind of file status change.
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatusKind {
    Added,
    Modified,
    Deleted,
    Untracked,
}

// ─── Factory Functions ───────────────────────────────────────

impl SyncSnapshot {
    /// Create an empty snapshot for a new repo.
    pub fn empty(machine_id: &str) -> Self {
        Self {
            metadata: SnapshotMeta {
                version: 1,
                created_at: chrono::Utc::now().to_rfc3339(),
                created_by: machine_id.to_string(),
            },
            entries: vec![],
        }
    }
}

impl SyncConfig {
    /// Create default config for a new sync repo.
    pub fn new(machine_id: &str, remote_url: Option<&str>) -> Self {
        Self {
            sync: SyncSettings {
                machine_id: machine_id.to_string(),
                remote_url: remote_url.map(String::from),
                default_sync: false,
                auto_push: false,
                conflict_strategy: ConflictStrategy::Ask,
                encrypted: true,
                encryption_policy: SyncEncryptionPolicy::Mandatory,
                verify_signatures: false,
                enforce_ssh: false,
            },
            manifest: ManifestConfig::default(),
        }
    }
}

// ─── Diff Types ──────────────────────────────────────────────

/// Difference between local ENV state and sync snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncDiff {
    pub added: Vec<DiffEntry>,
    pub modified: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
}

impl SyncDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.removed.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len() + self.modified.len() + self.removed.len()
    }
}

/// A single key difference.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    pub key: String,
    pub local_value: Option<String>,
    pub remote_value: Option<String>,
}

// ─── Conflict Types ──────────────────────────────────────────

/// A key with conflicting local and remote values.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictEntry {
    pub key: String,
    pub local_value: Option<String>,
    pub remote_value: Option<String>,
}

/// How a conflict was resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    KeepLocal,
    KeepRemote,
    ManualEdit(String),
    Delete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_version_meets_minimum_exact() {
        let v = GitVersion {
            major: 2,
            minor: 28,
            patch: 0,
        };
        assert!(v.meets_minimum());
    }

    #[test]
    fn test_git_version_below_minimum() {
        let v = GitVersion {
            major: 2,
            minor: 27,
            patch: 9,
        };
        assert!(!v.meets_minimum());
    }

    #[test]
    fn test_git_version_above_minimum() {
        let v = GitVersion {
            major: 2,
            minor: 40,
            patch: 0,
        };
        assert!(v.meets_minimum());
    }

    #[test]
    fn test_sync_diff_empty_and_total() {
        let empty_diff = SyncDiff {
            added: vec![],
            modified: vec![],
            removed: vec![],
        };
        assert!(empty_diff.is_empty());
        assert_eq!(empty_diff.total_changes(), 0);

        let non_empty = SyncDiff {
            added: vec![DiffEntry {
                key: "A".to_string(),
                local_value: Some("1".to_string()),
                remote_value: None,
            }],
            modified: vec![],
            removed: vec![DiffEntry {
                key: "B".to_string(),
                local_value: None,
                remote_value: Some("2".to_string()),
            }],
        };
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.total_changes(), 2);
    }

    #[test]
    fn test_sync_config_new_defaults() {
        let config = SyncConfig::new("my-machine", Some("git@host:repo.git"));
        assert_eq!(config.sync.machine_id, "my-machine");
        assert_eq!(
            config.sync.remote_url,
            Some("git@host:repo.git".to_string())
        );
        assert!(!config.sync.auto_push);
        assert!(!config.sync.default_sync);
        assert_eq!(config.sync.conflict_strategy, ConflictStrategy::Ask);
        assert!(config.sync.encrypted);
    }
}

/// A conflict after resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEntry {
    pub key: String,
    pub resolved_value: Option<String>,
    pub resolution: Resolution,
}

// ─── Status Types ────────────────────────────────────────────

/// Overall sync status.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    InSync,
    LocalAhead,
    NotInitialized,
}

/// Per-key sync status.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyStatus {
    Synced,
    LocalOnly,
    Unset,
}

// ─── Summary Types ───────────────────────────────────────────

/// Result of a push operation.
#[derive(Debug, Clone)]
pub struct PushSummary {
    pub keys_pushed: usize,
    pub commit_hash: Option<String>,
    pub push_result: PushResult,
    pub message: String,
}

/// Result of a pull operation.
#[derive(Debug, Clone)]
pub struct PullSummary {
    pub keys_added: usize,
    pub keys_modified: usize,
    pub keys_removed: usize,
    pub conflicts: Vec<ConflictEntry>,
    pub backup_path: Option<PathBuf>,
}

/// Result of a mark operation.
#[derive(Debug, Clone)]
pub struct MarkResult {
    pub marked_keys: Vec<String>,
    pub warnings: Vec<String>,
}
