use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ─── Error Types ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("git not found in PATH. Install git >= 2.28: https://git-scm.com")]
    GitNotFound,

    #[error("git version {found} is too old. Minimum required: {required}")]
    GitVersionTooOld { found: String, required: String },

    #[error("git command failed: {command}\n{stderr}")]
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

    #[error("I/O error at '{path}': {source}")]
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
