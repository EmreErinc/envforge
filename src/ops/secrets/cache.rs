use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::config::config_dir;

use super::provider::SecretsError;

/// Process-wide lock serializing cache read-resolve-write critical sections.
/// Defends against in-process TOCTOU between [`read_cache`] (miss) and
/// [`write_cache`] (store). Cross-process safety still relies on the
/// underlying tempfile + atomic-rename in [`write_cache`].
fn cache_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

const CACHE_DIR: &str = "secrets-cache";
const DEFAULT_TTL_SECS: u64 = 300; // 5 minutes

/// A single cached secret entry.
///
/// Wipes the decrypted `value` from memory on drop to limit how long the
/// plaintext lives in process memory / swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub value: String,
    pub fetched_at: String,
    pub ttl_secs: u64,
}

impl Drop for CacheEntry {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

/// A secret reference (pointer to a remote secret).
#[derive(Debug, Clone, PartialEq)]
pub struct SecretRef {
    pub provider: String,
    pub path: String,
    pub key: String,
}

const REF_PREFIX: &str = "ref:";

impl SecretRef {
    /// Parse a reference string: "ref:vault:secret/myapp/DB_URL"
    pub fn parse(value: &str) -> Option<Self> {
        if !value.starts_with(REF_PREFIX) {
            return None;
        }
        let rest = &value[REF_PREFIX.len()..];
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }
        // path may contain the key as the last segment
        let provider = parts[0].to_string();
        let full_path = parts[1].to_string();

        // Split path and key: "secret/myapp/DB_URL" → path="secret/myapp", key="DB_URL"
        if let Some(last_slash) = full_path.rfind('/') {
            Some(SecretRef {
                provider,
                path: full_path[..last_slash].to_string(),
                key: full_path[last_slash + 1..].to_string(),
            })
        } else {
            // No slash — entire thing is the key
            Some(SecretRef {
                provider,
                path: String::new(),
                key: full_path,
            })
        }
    }

    /// Format as reference string.
    pub fn to_ref_string(&self) -> String {
        if self.path.is_empty() {
            format!("{}{}:{}", REF_PREFIX, self.provider, self.key)
        } else {
            format!("{}{}:{}/{}", REF_PREFIX, self.provider, self.path, self.key)
        }
    }
}

/// Check if a value is a secret reference.
pub fn is_reference(value: &str) -> bool {
    value.starts_with(REF_PREFIX)
}

/// Get the cache directory path.
fn cache_dir() -> Result<PathBuf, SecretsError> {
    let dir = config_dir().map_err(|e| SecretsError::CacheError(e.to_string()))?;
    Ok(dir.join(CACHE_DIR))
}

/// Get the cache file path for a provider + key combination.
fn cache_file_path(provider: &str, key: &str) -> Result<PathBuf, SecretsError> {
    let dir = cache_dir()?;
    let provider_dir = dir.join(provider);
    // Use key name directly (sanitized for filesystem)
    let safe_key: String = key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(provider_dir.join(format!("{}.cache", safe_key)))
}

/// Read a cached value. Returns None if cache miss or expired.
pub fn read_cache(provider: &str, key: &str) -> Result<Option<String>, SecretsError> {
    let path = cache_file_path(provider, key)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| SecretsError::IoError {
        path: path.clone(),
        source: e,
    })?;

    let mut entry: CacheEntry = match toml::from_str(&content) {
        Ok(e) => e,
        Err(_) => {
            // Corrupt cache, remove it
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
    };

    // Check TTL
    if let Ok(fetched) = chrono::DateTime::parse_from_rfc3339(&entry.fetched_at) {
        let now = chrono::Utc::now();
        let elapsed = now.signed_duration_since(fetched).num_seconds() as u64;
        if elapsed <= entry.ttl_secs {
            // Take the value out of `entry` (we cannot move-destructure
            // because CacheEntry implements Drop). The husk will still
            // be zeroized, but its `value` is now an empty String.
            return Ok(Some(std::mem::take(&mut entry.value)));
        }
    }

    // Expired — return as stale (caller decides)
    Ok(None)
}

/// Read a cached value even if expired (for offline fallback).
pub fn read_cache_stale(provider: &str, key: &str) -> Result<Option<String>, SecretsError> {
    let path = cache_file_path(provider, key)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| SecretsError::IoError {
        path: path.clone(),
        source: e,
    })?;

    let mut entry: CacheEntry = match toml::from_str(&content) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    Ok(Some(std::mem::take(&mut entry.value)))
}

/// Write a value to cache.
pub fn write_cache(
    provider: &str,
    key: &str,
    value: &str,
    ttl_secs: Option<u64>,
) -> Result<(), SecretsError> {
    let path = cache_file_path(provider, key)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SecretsError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let entry = CacheEntry {
        value: value.to_string(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
        ttl_secs: ttl_secs.unwrap_or(DEFAULT_TTL_SECS),
    };

    let content =
        toml::to_string_pretty(&entry).map_err(|e| SecretsError::CacheError(e.to_string()))?;

    // Write cache file with restrictive permissions (0600 on Unix)
    // Cache contains decrypted secret values and must not be world-readable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let tempfile =
            tempfile::NamedTempFile::new_in(path.parent().unwrap_or(&path)).map_err(|e| {
                SecretsError::IoError {
                    path: path.clone(),
                    source: e,
                }
            })?;
        tempfile
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| SecretsError::IoError {
                path: path.clone(),
                source: e,
            })?;
        std::fs::write(tempfile.path(), &content).map_err(|e| SecretsError::IoError {
            path: path.clone(),
            source: e,
        })?;
        tempfile.persist(&path).map_err(|e| SecretsError::IoError {
            path: path.clone(),
            source: e.error,
        })?;
    }

    #[cfg(not(unix))]
    {
        // Refuse to write decrypted secret cache on non-unix targets
        // where 0600 perms cannot be reliably enforced.
        let _ = content;
        return Err(SecretsError::CacheError(
            "secret cache requires a unix-like OS for secure (0600) writes".to_string(),
        ));
    }

    #[allow(unreachable_code)]
    Ok(())
}

/// Invalidate (remove) a cached value.
pub fn invalidate_cache(provider: &str, key: &str) -> Result<(), SecretsError> {
    let path = cache_file_path(provider, key)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| SecretsError::IoError { path, source: e })?;
    }
    Ok(())
}

/// Invalidate all cache for a provider.
pub fn invalidate_provider_cache(provider: &str) -> Result<(), SecretsError> {
    let dir = cache_dir()?.join(provider);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| SecretsError::IoError {
            path: dir,
            source: e,
        })?;
    }
    Ok(())
}

/// Information about a single cached entry (for listing).
#[derive(Debug, Clone, Serialize)]
pub struct CachedEntryInfo {
    pub provider: String,
    pub key: String,
    pub fetched_at: String,
    pub ttl_secs: u64,
    pub expired: bool,
}

/// List all cached entries across all providers.
pub fn list_all_cached() -> Result<Vec<CachedEntryInfo>, SecretsError> {
    let dir = cache_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let providers = std::fs::read_dir(&dir).map_err(|e| SecretsError::IoError {
        path: dir,
        source: e,
    })?;

    for provider_entry in providers {
        let provider_entry = provider_entry.map_err(|e| SecretsError::CacheError(e.to_string()))?;
        let provider_path = provider_entry.path();
        if !provider_path.is_dir() {
            continue;
        }
        let provider_name = provider_entry.file_name().to_string_lossy().to_string();

        let files = std::fs::read_dir(&provider_path).map_err(|e| SecretsError::IoError {
            path: provider_path.clone(),
            source: e,
        })?;

        for file_entry in files {
            let file_entry = file_entry.map_err(|e| SecretsError::CacheError(e.to_string()))?;
            let file_path = file_entry.path();
            let file_name = file_entry.file_name().to_string_lossy().to_string();
            if !file_name.ends_with(".cache") {
                continue;
            }

            let key = file_name.trim_end_matches(".cache").to_string();

            let content = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut cache_entry: CacheEntry = match toml::from_str(&content) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let expired = if let Ok(fetched) =
                chrono::DateTime::parse_from_rfc3339(&cache_entry.fetched_at)
            {
                let now = chrono::Utc::now();
                let elapsed = now.signed_duration_since(fetched).num_seconds() as u64;
                elapsed > cache_entry.ttl_secs
            } else {
                true
            };

            // Cannot move fields out of `CacheEntry` (it implements Drop).
            entries.push(CachedEntryInfo {
                provider: provider_name.clone(),
                key,
                fetched_at: std::mem::take(&mut cache_entry.fetched_at),
                ttl_secs: cache_entry.ttl_secs,
                expired,
            });
        }
    }

    entries.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.key.cmp(&b.key)));
    Ok(entries)
}

/// Clear all cached entries.
pub fn clear_all_cache() -> Result<(), SecretsError> {
    let dir = cache_dir()?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| SecretsError::IoError {
            path: dir,
            source: e,
        })?;
    }
    Ok(())
}

/// Resolve a secret reference: try cache first, then fetch from provider.
/// Falls back to stale cache if the provider is unreachable.
pub fn resolve_reference(
    secret_ref: &SecretRef,
    provider: &dyn super::provider::SecretProvider,
    credentials: &HashMap<String, String>,
) -> Result<String, SecretsError> {
    // Serialize cache miss → fetch → store across threads in this process
    // so two simultaneous resolves of the same ref don't both call the
    // remote provider and race to write the cache file.
    let _guard = cache_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Try fresh cache first (re-check under lock).
    if let Some(cached) = read_cache(&secret_ref.provider, &secret_ref.key)? {
        return Ok(cached);
    }

    // Try provider
    match provider.get(credentials, &secret_ref.path, &secret_ref.key) {
        Ok(value) => {
            write_cache(&secret_ref.provider, &secret_ref.key, &value, None)?;
            Ok(value)
        }
        Err(e) => {
            // Fallback to stale cache
            if let Some(stale) = read_cache_stale(&secret_ref.provider, &secret_ref.key)? {
                eprintln!(
                    "warning: using cached value for {} (provider unreachable: {})",
                    secret_ref.key, e
                );
                Ok(stale)
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_ref_parse() {
        let r = SecretRef::parse("ref:vault:secret/myapp/DB_URL").unwrap();
        assert_eq!(r.provider, "vault");
        assert_eq!(r.path, "secret/myapp");
        assert_eq!(r.key, "DB_URL");
    }

    #[test]
    fn test_secret_ref_parse_no_path() {
        let r = SecretRef::parse("ref:doppler:DB_URL").unwrap();
        assert_eq!(r.provider, "doppler");
        assert_eq!(r.path, "");
        assert_eq!(r.key, "DB_URL");
    }

    #[test]
    fn test_secret_ref_parse_invalid() {
        assert!(SecretRef::parse("not-a-ref").is_none());
        assert!(SecretRef::parse("ref:").is_none());
    }

    #[test]
    fn test_secret_ref_roundtrip() {
        let r = SecretRef {
            provider: "vault".to_string(),
            path: "secret/myapp".to_string(),
            key: "DB_URL".to_string(),
        };
        let s = r.to_ref_string();
        assert_eq!(s, "ref:vault:secret/myapp/DB_URL");
        let parsed = SecretRef::parse(&s).unwrap();
        assert_eq!(parsed, r);
    }

    #[test]
    fn test_is_reference() {
        assert!(is_reference("ref:vault:secret/myapp/KEY"));
        assert!(!is_reference("plain_value"));
        assert!(!is_reference("ENC[age:xxx]"));
    }

    #[test]
    fn test_cache_write_and_read() {
        // Use a temp dir for cache by testing the entry serialization
        let entry = CacheEntry {
            value: "secret_val".to_string(),
            fetched_at: chrono::Utc::now().to_rfc3339(),
            ttl_secs: 300,
        };
        let toml_str = toml::to_string_pretty(&entry).unwrap();
        let deserialized: CacheEntry = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.value, "secret_val");
        assert_eq!(deserialized.ttl_secs, 300);
    }
}
