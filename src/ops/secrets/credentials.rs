use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
}
