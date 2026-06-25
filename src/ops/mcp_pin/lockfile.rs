//! Lockfile schema, serde, and persistence for `.envforge/mcp.lock`.
//!
//! Schema versioning uses a top-level `format_version: u32` plus
//! explicit `migrate_v{n}_to_v{n+1}` chain functions (none needed at v1).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::{PackageManager, PinMethod, Platform, Transport};

/// Current on-disk lockfile format version. Bump on schema change.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

// ──────────────────────────────────────────────────────────────────────────────
// Error
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum LockfileError {
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("TOML parse error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("unsupported lockfile format_version: found {found}, supported max {supported_max}")]
    UnsupportedFormatVersion { found: u32, supported_max: u32 },

    #[error("duplicate server entry: '{name}'")]
    DuplicateServer { name: String },

    #[error("invalid server '{name}': {reason}")]
    InvalidServer { name: String, reason: String },

    #[error("lockfile contains git merge-conflict markers at line {line}; run `mcp pin --resolve-conflicts`")]
    MergeConflictMarkers { line: usize },
}

// ──────────────────────────────────────────────────────────────────────────────
// BinaryHash (entity)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryHash {
    pub platform: Platform,
    /// Hex-encoded SHA-256 (lowercase, no `0x` prefix).
    pub sha256: String,
    pub realpath: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<PathBuf>,
}

impl BinaryHash {
    pub fn from_bytes(platform: Platform, sha: [u8; 32], realpath: PathBuf) -> Self {
        Self {
            platform,
            sha256: hex::encode(sha),
            realpath,
            symlink_target: None,
        }
    }

    pub fn sha256_bytes(&self) -> Option<[u8; 32]> {
        let v = hex::decode(&self.sha256).ok()?;
        if v.len() != 32 {
            return None;
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&v);
        Some(out)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ServerPin
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPin {
    pub name: String,
    #[serde(default)]
    pub pin_method: PinMethod,
    pub pinned_at: DateTime<Utc>,
    pub pinned_by_machine: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub transport: Transport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<PackageManager>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_integrity: Option<String>,

    /// Hex-encoded SHA-256 of canonical-JSON form of this server's config section.
    pub config_hash: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_list_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_list_captured_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub dynamic_tools: bool,
    #[serde(default)]
    pub volatile: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spki_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialize_response_hash: Option<String>,

    #[serde(default, rename = "binary_hash", skip_serializing_if = "Vec::is_empty")]
    pub binary_hashes: Vec<BinaryHash>,
}

impl ServerPin {
    /// Enforce all invariants from the static model.
    pub fn validate(&self) -> Result<(), LockfileError> {
        if self.name.is_empty() {
            return Err(LockfileError::InvalidServer {
                name: self.name.clone(),
                reason: "name must be non-empty".into(),
            });
        }

        let has_command = self.command.is_some();
        let has_remote = matches!(self.transport, Transport::Sse | Transport::Http);

        match (has_command, has_remote, self.url.is_some()) {
            (true, false, false) => {}
            (false, true, true) => {}
            _ => {
                return Err(LockfileError::InvalidServer {
                    name: self.name.clone(),
                    reason: "must specify exactly one of {command+args} or {transport=sse|http with url}".into(),
                });
            }
        }

        if self.volatile {
            if !self.binary_hashes.is_empty() {
                return Err(LockfileError::InvalidServer {
                    name: self.name.clone(),
                    reason: "volatile servers must have empty binary_hashes".into(),
                });
            }
            if self.package_integrity.is_none() && self.spki_sha256.is_none() {
                return Err(LockfileError::InvalidServer {
                    name: self.name.clone(),
                    reason: "volatile servers require package_integrity or spki_sha256 anchor"
                        .into(),
                });
            }
        }

        // Platform uniqueness within binary_hashes.
        for i in 0..self.binary_hashes.len() {
            for j in (i + 1)..self.binary_hashes.len() {
                if self.binary_hashes[i].platform == self.binary_hashes[j].platform {
                    return Err(LockfileError::InvalidServer {
                        name: self.name.clone(),
                        reason: format!(
                            "duplicate platform '{}' in binary_hashes",
                            self.binary_hashes[i].platform
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn binary_hash_for(&self, platform: &Platform) -> Option<&BinaryHash> {
        self.binary_hashes.iter().find(|b| &b.platform == platform)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Lockfile (aggregate root)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub format_version: u32,
    pub pattern_set_version: String,
    #[serde(default, rename = "server", skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<ServerPin>,
}

impl Lockfile {
    pub fn new(pattern_set_version: impl Into<String>) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            pattern_set_version: pattern_set_version.into(),
            servers: Vec::new(),
        }
    }

    /// Insert or replace a server by name. Validates invariants.
    pub fn upsert_server(&mut self, pin: ServerPin) -> Result<(), LockfileError> {
        pin.validate()?;
        match self.servers.iter().position(|s| s.name == pin.name) {
            Some(i) => self.servers[i] = pin,
            None => self.servers.push(pin),
        }
        self.sort_servers();
        Ok(())
    }

    /// Add a platform entry to an existing server. Errors if server absent.
    pub fn add_platform(&mut self, name: &str, binary: BinaryHash) -> Result<(), LockfileError> {
        let pin = self
            .servers
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| LockfileError::InvalidServer {
                name: name.to_string(),
                reason: "server not found".into(),
            })?;
        // Replace existing platform entry or push new.
        match pin
            .binary_hashes
            .iter()
            .position(|b| b.platform == binary.platform)
        {
            Some(i) => pin.binary_hashes[i] = binary,
            None => pin.binary_hashes.push(binary),
        }
        pin.binary_hashes
            .sort_by(|a, b| a.platform.cmp(&b.platform));
        Ok(())
    }

    fn sort_servers(&mut self) {
        self.servers.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn find(&self, name: &str) -> Option<&ServerPin> {
        self.servers.iter().find(|s| s.name == name)
    }

    pub fn validate(&self) -> Result<(), LockfileError> {
        // format_version supported?
        if self.format_version > CURRENT_FORMAT_VERSION {
            return Err(LockfileError::UnsupportedFormatVersion {
                found: self.format_version,
                supported_max: CURRENT_FORMAT_VERSION,
            });
        }
        // Name uniqueness.
        for i in 0..self.servers.len() {
            for j in (i + 1)..self.servers.len() {
                if self.servers[i].name == self.servers[j].name {
                    return Err(LockfileError::DuplicateServer {
                        name: self.servers[i].name.clone(),
                    });
                }
            }
        }
        // Per-server invariants.
        for s in &self.servers {
            s.validate()?;
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LockfileSerde
// ──────────────────────────────────────────────────────────────────────────────

pub struct LockfileSerde;

impl LockfileSerde {
    /// Parse a lockfile from raw TOML bytes.
    ///
    /// Pre-scans for git merge-conflict markers and rejects them with a
    /// structured error so the caller can route to `mcp pin --resolve-conflicts`.
    pub fn parse(input: &[u8]) -> Result<Lockfile, LockfileError> {
        // Merge-conflict marker pre-scan.
        let text = std::str::from_utf8(input).map_err(|_| LockfileError::InvalidServer {
            name: String::new(),
            reason: "lockfile is not valid UTF-8".into(),
        })?;
        for (idx, line) in text.lines().enumerate() {
            if line.starts_with("<<<<<<<")
                || line.starts_with("=======")
                || line.starts_with(">>>>>>>")
            {
                return Err(LockfileError::MergeConflictMarkers { line: idx + 1 });
            }
        }

        let lockfile: Lockfile = toml::from_str(text)?;

        if lockfile.format_version > CURRENT_FORMAT_VERSION {
            return Err(LockfileError::UnsupportedFormatVersion {
                found: lockfile.format_version,
                supported_max: CURRENT_FORMAT_VERSION,
            });
        }
        // Future migrations dispatch here.
        lockfile.validate()?;
        Ok(lockfile)
    }

    /// Serialize to byte-stable TOML.
    pub fn write(lockfile: &Lockfile) -> Result<Vec<u8>, LockfileError> {
        lockfile.validate()?;
        let mut out = toml::to_string(lockfile)?;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        Ok(out.into_bytes())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LockfileRepository
// ──────────────────────────────────────────────────────────────────────────────

pub trait LockfileRepository {
    fn load(&self, path: &Path) -> Result<Lockfile, LockfileError>;
    fn save(&self, path: &Path, lockfile: &Lockfile) -> Result<(), LockfileError>;
    fn exists(&self, path: &Path) -> bool;
}

/// Filesystem-backed lockfile repo. Atomic save via tempfile + rename.
pub struct FsLockfileRepository;

impl LockfileRepository for FsLockfileRepository {
    fn load(&self, path: &Path) -> Result<Lockfile, LockfileError> {
        let bytes = std::fs::read(path).map_err(|e| LockfileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        LockfileSerde::parse(&bytes)
    }

    fn save(&self, path: &Path, lockfile: &Lockfile) -> Result<(), LockfileError> {
        let bytes = LockfileSerde::write(lockfile)?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| LockfileError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| LockfileError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        use std::io::Write;
        tmp.write_all(&bytes).map_err(|e| LockfileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        tmp.flush().map_err(|e| LockfileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        tmp.persist(path).map_err(|e| LockfileError::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Machine identity (mirrors `canary::v2` algorithm to avoid coupling)
// ──────────────────────────────────────────────────────────────────────────────

/// Hex-encoded SHA-256(hostname)[..8] — same algorithm as `canary::v2::stable_machine_id`.
///
/// Lockfile field `pinned_by_machine` is a human-readable hex string (16 chars).
pub fn pinned_by_machine_id() -> String {
    use sha2::{Digest, Sha256};
    let host = hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_default();
    let h = Sha256::digest(host.as_bytes());
    hex::encode(&h[..8])
}
