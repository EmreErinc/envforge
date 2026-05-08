use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::config_dir;

use super::provider::SecretsError;

const SOURCES_FILE: &str = "secret-sources.toml";

/// Metadata about when a secret was last pulled or set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretAge {
    /// Provider it came from (or "local" if set manually)
    pub provider: String,
    /// Path in the provider
    pub path: String,
    /// When this secret was last pulled/set
    pub updated_at: String,
}

/// All tracked secret ages.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretSources {
    #[serde(default)]
    pub secrets: BTreeMap<String, SecretAge>,
}

/// Get the sources file path.
fn sources_path() -> Result<PathBuf, SecretsError> {
    let dir = config_dir().map_err(|e| SecretsError::CacheError(e.to_string()))?;
    Ok(dir.join(SOURCES_FILE))
}

/// Load secret sources from disk.
pub fn load_sources() -> Result<SecretSources, SecretsError> {
    let path = sources_path()?;
    if !path.exists() {
        return Ok(SecretSources::default());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| SecretsError::IoError {
        path: path.clone(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| SecretsError::CacheError(e.to_string()))
}

/// Save secret sources to disk.
///
/// Writes with mode 0600 on Unix at create time. The file records which
/// secrets came from which provider/path — operational metadata that
/// reveals the user's secret topology. Default umask leaves it
/// world-readable; this tightens it to owner-only. Mirrors the pattern
/// in `analytics/storage.rs`, `changelog.rs`, and `lifecycle/orchestrator.rs`.
pub fn save_sources(sources: &SecretSources) -> Result<(), SecretsError> {
    let path = sources_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SecretsError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let content =
        toml::to_string_pretty(sources).map_err(|e| SecretsError::CacheError(e.to_string()))?;

    #[cfg(not(unix))]
    {
        return std::fs::write(&path, content)
            .map_err(|e| SecretsError::IoError { path, source: e });
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| SecretsError::IoError {
                path: path.clone(),
                source: e,
            })?;
        file.write_all(content.as_bytes())
            .map_err(|e| SecretsError::IoError {
                path: path.clone(),
                source: e,
            })?;
        // Defensive post-write chmod for files inherited from older
        // envforge versions that wrote 0644.
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.permissions().mode() & 0o077 != 0 {
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
        Ok(())
    }
}

/// Record that secrets were pulled from a provider.
pub fn record_pull(keys: &[String], provider: &str, path: &str) -> Result<(), SecretsError> {
    let mut sources = load_sources()?;
    let now = Utc::now().to_rfc3339();
    for key in keys {
        sources.secrets.insert(
            key.clone(),
            SecretAge {
                provider: provider.to_string(),
                path: path.to_string(),
                updated_at: now.clone(),
            },
        );
    }
    save_sources(&sources)
}

/// Record a single secret set/update.
pub fn record_set(key: &str, provider: &str, path: &str) -> Result<(), SecretsError> {
    record_pull(&[key.to_string()], provider, path)
}

/// Age report entry for display.
#[derive(Debug, Clone)]
pub struct AgeEntry {
    pub key: String,
    pub provider: String,
    pub path: String,
    pub updated_at: String,
    pub age_days: i64,
    pub stale: bool,
}

/// Get age report for all tracked secrets.
pub fn get_age_report(stale_threshold_days: i64) -> Result<Vec<AgeEntry>, SecretsError> {
    let sources = load_sources()?;
    let now = Utc::now();
    let mut entries = Vec::new();

    for (key, age) in &sources.secrets {
        let age_days = if let Ok(dt) = DateTime::parse_from_rfc3339(&age.updated_at) {
            now.signed_duration_since(dt).num_days()
        } else {
            -1 // unknown
        };

        entries.push(AgeEntry {
            key: key.clone(),
            provider: age.provider.clone(),
            path: age.path.clone(),
            updated_at: age.updated_at.clone(),
            age_days,
            stale: age_days >= stale_threshold_days,
        });
    }

    entries.sort_by_key(|a| std::cmp::Reverse(a.age_days));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sources_roundtrip() {
        let mut sources = SecretSources::default();
        sources.secrets.insert(
            "DB_URL".to_string(),
            SecretAge {
                provider: "vault".to_string(),
                path: "secret/myapp".to_string(),
                updated_at: Utc::now().to_rfc3339(),
            },
        );
        let toml_str = toml::to_string_pretty(&sources).unwrap();
        let parsed: SecretSources = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.secrets.len(), 1);
        assert_eq!(parsed.secrets["DB_URL"].provider, "vault");
    }

    #[test]
    fn test_age_entry_stale() {
        let old_date = "2025-01-01T00:00:00+00:00";
        let dt = DateTime::parse_from_rfc3339(old_date).unwrap();
        let now = Utc::now();
        let days = now.signed_duration_since(dt).num_days();
        assert!(days > 90);
    }

    #[test]
    fn test_empty_sources() {
        let sources = SecretSources::default();
        assert!(sources.secrets.is_empty());
    }
}
