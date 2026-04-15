use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::config_dir;

use super::provider::SecretsError;

const CACHE_DIR: &str = "secrets-cache";
const DEFAULT_TTL_SECS: u64 = 300; // 5 minutes

/// A single cached secret entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub value: String,
    pub fetched_at: String,
    pub ttl_secs: u64,
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

    let entry: CacheEntry = match toml::from_str(&content) {
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
            return Ok(Some(entry.value));
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

    let entry: CacheEntry = match toml::from_str(&content) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    Ok(Some(entry.value))
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

    std::fs::write(&path, content).map_err(|e| SecretsError::IoError { path, source: e })
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

/// Resolve a secret reference: try cache first, then fetch from provider.
pub fn resolve_reference(
    secret_ref: &SecretRef,
    provider: &dyn super::provider::SecretProvider,
    credentials: &HashMap<String, String>,
) -> Result<String, SecretsError> {
    // Try cache first
    if let Some(cached) = read_cache(&secret_ref.provider, &secret_ref.key)? {
        return Ok(cached);
    }

    // Fetch from provider
    let value = provider.get(credentials, &secret_ref.path, &secret_ref.key)?;

    // Cache the result
    write_cache(&secret_ref.provider, &secret_ref.key, &value, None)?;

    Ok(value)
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
