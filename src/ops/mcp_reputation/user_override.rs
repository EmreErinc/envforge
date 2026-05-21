//! Per-user trust override store.
//!
//! - On-disk JSON array at `~/.config/envforge/mcp-trust.json`
//! - Atomic save via tempfile + rename
//! - 0600 perms on Unix
//! - Security floor (ADR-017): write API refuses any tier other than UserTrusted

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::OverrideError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOverride {
    pub name: String,
    pub reason: String,
    pub granted_at: DateTime<Utc>,
    pub granted_by_machine: String,
}

pub trait UserOverrideRepository: Send + Sync {
    fn load(&self) -> Result<Vec<UserOverride>, OverrideError>;
    fn save(&self, overrides: &[UserOverride]) -> Result<(), OverrideError>;
}

/// Filesystem-backed override repository. Atomic save via tempfile + rename;
/// 0600 perms on Unix.
pub struct FsUserOverrideRepository {
    path: PathBuf,
}

impl FsUserOverrideRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Default location: `${config_dir}/envforge/mcp-trust.json`.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("envforge")
            .join("mcp-trust.json")
    }

    pub fn at_default() -> Self {
        Self::new(Self::default_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl UserOverrideRepository for FsUserOverrideRepository {
    fn load(&self) -> Result<Vec<UserOverride>, OverrideError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&self.path).map_err(|e| OverrideError::Io {
            path: self.path.clone(),
            source: e,
        })?;
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        let overrides: Vec<UserOverride> =
            serde_json::from_slice(&bytes).map_err(OverrideError::Corrupt)?;
        Ok(overrides)
    }

    fn save(&self, overrides: &[UserOverride]) -> Result<(), OverrideError> {
        let bytes = serde_json::to_vec_pretty(overrides).map_err(OverrideError::Corrupt)?;

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| OverrideError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| OverrideError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        use std::io::Write;
        tmp.write_all(&bytes).map_err(|e| OverrideError::Io {
            path: self.path.clone(),
            source: e,
        })?;
        tmp.flush().map_err(|e| OverrideError::Io {
            path: self.path.clone(),
            source: e,
        })?;
        let persisted = tmp.persist(&self.path).map_err(|e| OverrideError::Io {
            path: self.path.clone(),
            source: e.error,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        drop(persisted);
        Ok(())
    }
}

/// In-memory repository for tests. Behaves identically to FS-backed.
pub struct InMemoryUserOverrideRepository {
    data: Mutex<Vec<UserOverride>>,
}

impl Default for InMemoryUserOverrideRepository {
    fn default() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }
}

impl UserOverrideRepository for InMemoryUserOverrideRepository {
    fn load(&self) -> Result<Vec<UserOverride>, OverrideError> {
        Ok(self.data.lock().unwrap().clone())
    }

    fn save(&self, overrides: &[UserOverride]) -> Result<(), OverrideError> {
        *self.data.lock().unwrap() = overrides.to_vec();
        Ok(())
    }
}

/// Public store façade. Composes `UserOverrideRepository` with record/revoke
/// API enforcing the security floor (ADR-017).
pub struct UserOverrideStore {
    repo: Arc<dyn UserOverrideRepository>,
}

impl UserOverrideStore {
    pub fn new(repo: Arc<dyn UserOverrideRepository>) -> Self {
        Self { repo }
    }

    pub fn record_user_trust(&self, name: &str, reason: &str) -> Result<(), OverrideError> {
        if reason.trim().is_empty() {
            return Err(OverrideError::EmptyReason {
                name: name.to_string(),
            });
        }
        let mut overrides = self.repo.load()?;
        overrides.retain(|o| o.name != name);
        overrides.push(UserOverride {
            name: name.to_string(),
            reason: reason.to_string(),
            granted_at: Utc::now(),
            granted_by_machine: machine_id_hex(),
        });
        overrides.sort_by(|a, b| a.name.cmp(&b.name));
        self.repo.save(&overrides)
    }

    pub fn revoke_user_trust(&self, name: &str) -> Result<bool, OverrideError> {
        let mut overrides = self.repo.load()?;
        let before = overrides.len();
        overrides.retain(|o| o.name != name);
        let removed = overrides.len() != before;
        if removed {
            self.repo.save(&overrides)?;
        }
        Ok(removed)
    }

    pub fn find(&self, name: &str) -> Result<Option<UserOverride>, OverrideError> {
        let overrides = self.repo.load()?;
        Ok(overrides.into_iter().find(|o| o.name == name))
    }

    pub fn list(&self) -> Result<Vec<UserOverride>, OverrideError> {
        self.repo.load()
    }
}

/// SHA-256(hostname)[..8] hex — same algorithm as `mcp_pin::pinned_by_machine_id`.
/// Replicated locally to avoid cross-unit coupling.
fn machine_id_hex() -> String {
    use sha2::{Digest, Sha256};
    let host = hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_default();
    let h = Sha256::digest(host.as_bytes());
    hex::encode(&h[..8])
}
