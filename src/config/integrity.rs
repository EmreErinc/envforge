use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Integrity cache mapping file paths to their last known SHA-256 hashes.
///
/// Stored at `.envforge/integrity.toml`. Updated on every atomic write.
/// Verified on every parse. Closes the decorative-SHA-256 gap (T-003) and
/// provides the foundation for drift detection (G-7).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrityCache {
    /// File path (relative to project root) → hex-encoded SHA-256 hash.
    pub files: HashMap<String, String>,
}

impl IntegrityCache {
    pub fn load(cache_path: &Path) -> Result<Self, std::io::Error> {
        if !cache_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(cache_path)?;
        let cache: Self = toml::from_str(&content).unwrap_or_default();
        Ok(cache)
    }

    pub fn save(&self, cache_path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::other(format!("TOML serialize: {}", e)))?;
        std::fs::write(cache_path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(cache_path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// Store a hash for a file key.
    pub fn set(&mut self, file_key: &str, hash_hex: &str) {
        self.files
            .insert(file_key.to_string(), hash_hex.to_string());
    }

    /// Get the stored hash for a file key.
    pub fn get(&self, file_key: &str) -> Option<&str> {
        self.files.get(file_key).map(|s| s.as_str())
    }

    /// Determine the cache path for a given project.
    pub fn cache_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".envforge").join("integrity.toml")
    }
}

/// Convert a byte hash to hex string.
#[allow(dead_code)]
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    hash.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
        s
    })
}
