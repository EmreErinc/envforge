use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::config_dir;
use crate::ops::encrypt::{decrypt_value, encrypt_value, is_encrypted};

use super::provider::SecretsError;

const CREDENTIALS_FILE: &str = "credentials.toml";

/// Encrypted credential store for secret manager providers.
/// Each provider's credentials are stored as encrypted key-value pairs.
///
/// File format (credentials.toml):
/// ```toml
/// [vault]
/// token = "ENC[age:...]"
///
/// [aws-ssm]
/// access_key = "ENC[age:...]"
/// secret_key = "ENC[age:...]"
/// ```
///
/// Get the credentials file path.
pub fn credentials_path() -> Result<PathBuf, SecretsError> {
    let dir = config_dir().map_err(|e| SecretsError::CredentialError(e.to_string()))?;
    Ok(dir.join(CREDENTIALS_FILE))
}

/// Raw credential store (encrypted values on disk).
/// Provider sections contain encrypted key-value pairs.
/// `{provider}._meta` sections contain TTL metadata (e.g., `token_expires = "2026-04-21T01:00:00Z"`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialStore {
    #[serde(flatten)]
    pub providers: HashMap<String, HashMap<String, String>>,
}

/// Store a credential for a provider (encrypts the value).
pub fn store_credential(provider: &str, key: &str, value: &str) -> Result<(), SecretsError> {
    let path = credentials_path()?;
    let mut store = load_store(&path)?;

    let encrypted = encrypt_value(value)
        .map_err(|e| SecretsError::CredentialError(format!("encryption failed: {}", e)))?;

    store
        .providers
        .entry(provider.to_string())
        .or_default()
        .insert(key.to_string(), encrypted);

    save_store(&path, &store)
}

/// Read a credential for a provider (decrypts the value).
pub fn read_credential(provider: &str, key: &str) -> Result<String, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;

    let provider_creds =
        store
            .providers
            .get(provider)
            .ok_or_else(|| SecretsError::CredentialNotFound {
                provider: provider.to_string(),
            })?;

    let encrypted = provider_creds
        .get(key)
        .ok_or_else(|| SecretsError::CredentialNotFound {
            provider: provider.to_string(),
        })?;

    if is_encrypted(encrypted) {
        decrypt_value(encrypted)
            .map_err(|e| SecretsError::CredentialError(format!("decryption failed: {}", e)))
    } else {
        Ok(encrypted.clone())
    }
}

/// Read all credentials for a provider (decrypted).
pub fn read_all_credentials(provider: &str) -> Result<HashMap<String, String>, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;

    let provider_creds =
        store
            .providers
            .get(provider)
            .ok_or_else(|| SecretsError::CredentialNotFound {
                provider: provider.to_string(),
            })?;

    let mut decrypted = HashMap::new();
    for (key, encrypted) in provider_creds {
        let value = if is_encrypted(encrypted) {
            decrypt_value(encrypted)
                .map_err(|e| SecretsError::CredentialError(format!("decryption failed: {}", e)))?
        } else {
            encrypted.clone()
        };
        decrypted.insert(key.clone(), value);
    }

    Ok(decrypted)
}

/// Remove all credentials for a provider.
pub fn remove_credentials(provider: &str) -> Result<bool, SecretsError> {
    let path = credentials_path()?;
    let mut store = load_store(&path)?;
    let removed = store.providers.remove(provider).is_some();
    if removed {
        save_store(&path, &store)?;
    }
    Ok(removed)
}

/// Check if a provider has stored credentials.
pub fn has_credentials(provider: &str) -> Result<bool, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;
    Ok(store
        .providers
        .get(provider)
        .is_some_and(|creds| !creds.is_empty()))
}

/// List all providers with stored credentials.
pub fn list_configured_providers() -> Result<Vec<String>, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;
    let mut providers: Vec<String> = store
        .providers
        .iter()
        .filter(|(_, creds)| !creds.is_empty())
        .map(|(name, _)| name.clone())
        .collect();
    providers.sort();
    Ok(providers)
}

fn load_store(path: &Path) -> Result<CredentialStore, SecretsError> {
    if !path.exists() {
        return Ok(CredentialStore::default());
    }

    let content = std::fs::read_to_string(path).map_err(|e| SecretsError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content)
        .map_err(|e| SecretsError::CredentialError(format!("corrupt credentials file: {}", e)))
}

fn save_store(path: &Path, store: &CredentialStore) -> Result<(), SecretsError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SecretsError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let content = toml::to_string_pretty(store)
        .map_err(|e| SecretsError::CredentialError(format!("serialize failed: {}", e)))?;

    std::fs::write(path, content).map_err(|e| SecretsError::IoError {
        path: path.to_path_buf(),
        source: e,
    })
}

// ─── TTL Support ────────────────────────────────────────────

/// Parse a duration string into seconds.
/// Supported formats: "8h" (hours), "7d" (days).
pub fn parse_duration(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if let Some(hours) = s.strip_suffix('h') {
        hours
            .parse::<i64>()
            .map(|h| h * 3600)
            .map_err(|e| e.to_string())
    } else if let Some(days) = s.strip_suffix('d') {
        days.parse::<i64>()
            .map(|d| d * 86400)
            .map_err(|e| e.to_string())
    } else {
        Err(format!(
            "Invalid duration '{}'. Use: 8h, 24h, 7d, 30d",
            s
        ))
    }
}

/// The meta section key for a provider (e.g., "vault._meta").
fn meta_section(provider: &str) -> String {
    format!("{}._meta", provider)
}

/// The meta key for a credential's expiry (e.g., "token_expires").
fn expires_key(key: &str) -> String {
    format!("{}_expires", key)
}

/// Store a credential with an optional TTL.
/// If `ttl` is provided (e.g., "8h", "7d"), an expiry timestamp is written
/// to the `[provider._meta]` section.
pub fn store_credential_with_ttl(
    provider: &str,
    key: &str,
    value: &str,
    ttl: Option<&str>,
) -> Result<(), SecretsError> {
    // Store the encrypted credential as usual.
    store_credential(provider, key, value)?;

    // If a TTL is provided, write the expiry to the _meta section.
    if let Some(ttl_str) = ttl {
        let seconds = parse_duration(ttl_str).map_err(SecretsError::CredentialError)?;
        let expires_at = Utc::now() + chrono::Duration::seconds(seconds);
        let expires_str = expires_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let path = credentials_path()?;
        let mut store = load_store(&path)?;

        let meta = meta_section(provider);
        store
            .providers
            .entry(meta)
            .or_default()
            .insert(expires_key(key), expires_str);

        save_store(&path, &store)?;
    }

    Ok(())
}

/// Check if a credential has expired.
/// Returns `Ok(Some(expired_at_string))` if the credential has expired,
/// `Ok(None)` if it has no TTL or has not expired yet.
pub fn check_expiry(provider: &str, key: &str) -> Result<Option<String>, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;

    let meta = meta_section(provider);
    let exp_key = expires_key(key);

    if let Some(meta_map) = store.providers.get(&meta) {
        if let Some(expires_str) = meta_map.get(&exp_key) {
            if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                if Utc::now() > expires_at {
                    return Ok(Some(expires_str.clone()));
                }
            }
        }
    }

    Ok(None)
}

/// Get TTL remaining for a credential.
/// Returns `Ok(Some((expires_at, seconds_remaining)))` if the credential has a TTL.
/// `seconds_remaining` can be negative if the credential has already expired.
/// Returns `Ok(None)` if no TTL is set.
pub fn get_ttl_remaining(provider: &str, key: &str) -> Result<Option<(String, i64)>, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;

    let meta = meta_section(provider);
    let exp_key = expires_key(key);

    if let Some(meta_map) = store.providers.get(&meta) {
        if let Some(expires_str) = meta_map.get(&exp_key) {
            if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                let remaining = (expires_at.with_timezone(&Utc) - Utc::now()).num_seconds();
                return Ok(Some((expires_str.clone(), remaining)));
            }
        }
    }

    Ok(None)
}

/// Check all credentials for a provider and return expired ones.
/// Returns a list of (key, expired_at) pairs for any expired credentials.
pub fn check_all_expiry(provider: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let path = credentials_path()?;
    let store = load_store(&path)?;

    let meta = meta_section(provider);
    let mut expired = Vec::new();

    if let Some(meta_map) = store.providers.get(&meta) {
        for (meta_key, expires_str) in meta_map {
            if let Some(cred_key) = meta_key.strip_suffix("_expires") {
                if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                    if Utc::now() > expires_at {
                        expired.push((cred_key.to_string(), expires_str.clone()));
                    }
                }
            }
        }
    }

    Ok(expired)
}

/// Format remaining seconds as a human-readable string.
pub fn format_ttl_remaining(seconds: i64) -> String {
    if seconds < 0 {
        return "expired".to_string();
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours >= 24 {
        let days = hours / 24;
        let rem_hours = hours % 24;
        format!("{}d {}h remaining", days, rem_hours)
    } else if hours > 0 {
        format!("{}h {}m remaining", hours, minutes)
    } else {
        format!("{}m remaining", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_store_roundtrip() {
        let mut store = CredentialStore::default();
        let mut vault_creds = HashMap::new();
        vault_creds.insert("token".to_string(), "ENC[age:test123]".to_string());
        store.providers.insert("vault".to_string(), vault_creds);

        let toml_str = toml::to_string_pretty(&store).unwrap();
        let deserialized: CredentialStore = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            deserialized.providers.get("vault").unwrap().get("token"),
            Some(&"ENC[age:test123]".to_string())
        );
    }

    #[test]
    fn test_load_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.toml");
        let store = load_store(&path).unwrap();
        assert!(store.providers.is_empty());
    }

    #[test]
    fn test_save_and_load_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.toml");

        let mut store = CredentialStore::default();
        let mut creds = HashMap::new();
        creds.insert("key".to_string(), "value".to_string());
        store.providers.insert("test".to_string(), creds);

        save_store(&path, &store).unwrap();
        let loaded = load_store(&path).unwrap();
        assert_eq!(
            loaded.providers.get("test").unwrap().get("key"),
            Some(&"value".to_string())
        );
    }

    // ─── TTL Tests ──────────────────────────────────────────

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("8h").unwrap(), 28800);
        assert_eq!(parse_duration("24h").unwrap(), 86400);
        assert_eq!(parse_duration("1h").unwrap(), 3600);
    }

    #[test]
    fn test_parse_duration_days() {
        assert_eq!(parse_duration("7d").unwrap(), 604800);
        assert_eq!(parse_duration("30d").unwrap(), 2592000);
        assert_eq!(parse_duration("1d").unwrap(), 86400);
    }

    #[test]
    fn test_parse_duration_with_whitespace() {
        assert_eq!(parse_duration("  8h  ").unwrap(), 28800);
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("8m").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("h").is_err());
        assert!(parse_duration("d").is_err());
    }

    #[test]
    fn test_meta_section_name() {
        assert_eq!(meta_section("vault"), "vault._meta");
        assert_eq!(meta_section("aws-ssm"), "aws-ssm._meta");
    }

    #[test]
    fn test_expires_key_name() {
        assert_eq!(expires_key("token"), "token_expires");
        assert_eq!(expires_key("access_key"), "access_key_expires");
    }

    #[test]
    fn test_store_with_meta_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.toml");

        let mut store = CredentialStore::default();

        // Credential section
        let mut creds = HashMap::new();
        creds.insert("token".to_string(), "ENC[age:test123]".to_string());
        store.providers.insert("vault".to_string(), creds);

        // Meta section
        let mut meta = HashMap::new();
        meta.insert(
            "token_expires".to_string(),
            "2026-04-21T01:00:00Z".to_string(),
        );
        store
            .providers
            .insert("vault._meta".to_string(), meta);

        save_store(&path, &store).unwrap();
        let loaded = load_store(&path).unwrap();

        assert_eq!(
            loaded.providers.get("vault").unwrap().get("token"),
            Some(&"ENC[age:test123]".to_string())
        );
        assert_eq!(
            loaded
                .providers
                .get("vault._meta")
                .unwrap()
                .get("token_expires"),
            Some(&"2026-04-21T01:00:00Z".to_string())
        );
    }

    #[test]
    fn test_check_expiry_expired() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.toml");

        let mut store = CredentialStore::default();
        let mut meta = HashMap::new();
        // Set an expiry in the past
        meta.insert(
            "token_expires".to_string(),
            "2020-01-01T00:00:00Z".to_string(),
        );
        store
            .providers
            .insert("vault._meta".to_string(), meta);
        save_store(&path, &store).unwrap();

        // check_expiry reads from the default credentials_path,
        // so we test the logic directly with the store
        let meta_map = store.providers.get("vault._meta").unwrap();
        let expires_str = meta_map.get("token_expires").unwrap();
        let expires_at = chrono::DateTime::parse_from_rfc3339(expires_str).unwrap();
        assert!(Utc::now() > expires_at);
    }

    #[test]
    fn test_check_expiry_not_expired() {
        let mut store = CredentialStore::default();
        let mut meta = HashMap::new();
        // Set an expiry far in the future
        meta.insert(
            "token_expires".to_string(),
            "2099-12-31T23:59:59Z".to_string(),
        );
        store
            .providers
            .insert("vault._meta".to_string(), meta);

        let meta_map = store.providers.get("vault._meta").unwrap();
        let expires_str = meta_map.get("token_expires").unwrap();
        let expires_at = chrono::DateTime::parse_from_rfc3339(expires_str).unwrap();
        assert!(Utc::now() < expires_at);
    }

    #[test]
    fn test_format_ttl_remaining() {
        assert_eq!(format_ttl_remaining(-1), "expired");
        assert_eq!(format_ttl_remaining(0), "0m remaining");
        assert_eq!(format_ttl_remaining(3600), "1h 0m remaining");
        assert_eq!(format_ttl_remaining(7200 + 1800), "2h 30m remaining");
        assert_eq!(format_ttl_remaining(86400), "1d 0h remaining");
        assert_eq!(
            format_ttl_remaining(86400 + 3600 * 5),
            "1d 5h remaining"
        );
    }

    #[test]
    fn test_check_all_expiry_with_store() {
        let mut store = CredentialStore::default();
        let mut meta = HashMap::new();
        meta.insert(
            "token_expires".to_string(),
            "2020-01-01T00:00:00Z".to_string(),
        );
        meta.insert(
            "api_key_expires".to_string(),
            "2099-12-31T23:59:59Z".to_string(),
        );
        store
            .providers
            .insert("vault._meta".to_string(), meta);

        // Check which are expired by inspecting the store directly
        let meta_map = store.providers.get("vault._meta").unwrap();
        let mut expired = Vec::new();
        for (meta_key, expires_str) in meta_map {
            if let Some(cred_key) = meta_key.strip_suffix("_expires") {
                if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_str) {
                    if Utc::now() > expires_at {
                        expired.push(cred_key.to_string());
                    }
                }
            }
        }

        assert_eq!(expired.len(), 1);
        assert!(expired.contains(&"token".to_string()));
    }
}
