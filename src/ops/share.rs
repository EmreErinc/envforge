use std::collections::BTreeMap;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::ops::encrypt::{ensure_age_key, EncryptError};

#[derive(Serialize, Deserialize, Debug)]
pub struct SharePackage {
    pub metadata: ShareMeta,
    pub entries: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ShareMeta {
    pub created_at: String,
    /// Hostname of the sender at encryption time.
    ///
    /// **Not authenticated.** This field is set by the sender from
    /// `hostname::get()` and is preserved as-is through age encryption,
    /// but age does not bind it to the sender's identity. Treat this
    /// value as informational metadata, NOT proof of origin. If you
    /// need verifiable sender identity, sign the share contents with
    /// the sender's private age key out-of-band and verify on receipt.
    pub created_by: String,
    pub key_count: usize,
    pub expires_at: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("encryption failed: {0}")]
    EncryptFailed(String),
    #[error("decryption failed: {0}")]
    DecryptFailed(String),
    #[error("invalid share format: {0}")]
    InvalidFormat(String),
    #[error("share expired at {0}")]
    Expired(String),
    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<EncryptError> for ShareError {
    fn from(e: EncryptError) -> Self {
        ShareError::DecryptFailed(e.to_string())
    }
}

/// Create an encrypted share file.
/// Encrypts with recipient's age public key.
pub fn create_share(
    entries: &[(String, String)],
    recipient_pubkey: &str,
    expire_hours: Option<u64>,
) -> Result<Vec<u8>, ShareError> {
    let now = chrono::Local::now();
    let created_at = now.format("%Y-%m-%dT%H:%M:%S%z").to_string();

    let created_by = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let expires_at = expire_hours.map(|hours| {
        let expiry = now + chrono::Duration::hours(hours as i64);
        expiry.format("%Y-%m-%dT%H:%M:%S%z").to_string()
    });

    let mut entry_map = BTreeMap::new();
    for (k, v) in entries {
        entry_map.insert(k.clone(), v.clone());
    }

    let package = SharePackage {
        metadata: ShareMeta {
            created_at,
            created_by,
            key_count: entries.len(),
            expires_at,
        },
        entries: entry_map,
    };

    let toml_str = toml::to_string_pretty(&package)
        .map_err(|e| ShareError::EncryptFailed(format!("TOML serialization failed: {}", e)))?;

    let recipient: age::x25519::Recipient = recipient_pubkey.parse().map_err(|_| {
        ShareError::EncryptFailed(format!(
            "Invalid recipient public key: {}",
            recipient_pubkey
        ))
    })?;

    let recipients: Vec<&dyn age::Recipient> = vec![&recipient];
    let encryptor = age::Encryptor::with_recipients(recipients.into_iter())
        .map_err(|_| ShareError::EncryptFailed("No recipients".into()))?;

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| ShareError::EncryptFailed(e.to_string()))?;
    writer
        .write_all(toml_str.as_bytes())
        .map_err(|e| ShareError::EncryptFailed(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| ShareError::EncryptFailed(e.to_string()))?;

    Ok(encrypted)
}

/// Receive (decrypt) a share file using local age private key.
///
/// By default, expired shares are rejected (`ShareError::Expired`).
/// Pass `allow_expired = true` to bypass — typically for recovery /
/// inspection where the user has already accepted that the TTL passed.
pub fn receive_share(encrypted_data: &[u8]) -> Result<SharePackage, ShareError> {
    receive_share_with(encrypted_data, false)
}

/// Variant of [`receive_share`] that lets the caller opt into accepting
/// expired shares.
pub fn receive_share_with(
    encrypted_data: &[u8],
    allow_expired: bool,
) -> Result<SharePackage, ShareError> {
    // Cap input size — same reasoning as encrypt::decrypt_value: defend
    // against decompression-bomb-style age files.
    const MAX_SHARE_BYTES: usize = 8 * 1024 * 1024;
    const MAX_SHARE_PLAINTEXT_BYTES: u64 = 8 * 1024 * 1024;
    if encrypted_data.len() > MAX_SHARE_BYTES {
        return Err(ShareError::DecryptFailed(format!(
            "share blob too large ({} bytes, max {})",
            encrypted_data.len(),
            MAX_SHARE_BYTES
        )));
    }

    let key_content = ensure_age_key()?;
    let identity = get_identity(&key_content)?;

    let decryptor = age::Decryptor::new(encrypted_data)
        .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

    let mut limited = std::io::Read::take(reader, MAX_SHARE_PLAINTEXT_BYTES);
    let mut decrypted = String::new();
    limited
        .read_to_string(&mut decrypted)
        .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

    let package: SharePackage = toml::from_str(&decrypted)
        .map_err(|e| ShareError::InvalidFormat(format!("TOML parse failed: {}", e)))?;

    // Hard-block expired shares unless caller explicitly opts in.
    // The 0.7.5 behaviour ("warn but don't block") was advisory only,
    // which made the TTL a comment rather than a control.
    if let Some(ref expires_at) = package.metadata.expires_at {
        if let Ok(expiry) = chrono::DateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S%z") {
            if chrono::Local::now() > expiry {
                if allow_expired {
                    eprintln!(
                        "Warning: share expired at {} (--allow-expired set, proceeding).",
                        expires_at
                    );
                } else {
                    return Err(ShareError::Expired(expires_at.clone()));
                }
            }
        }
    }

    Ok(package)
}

/// Extract the age identity from key file content.
fn get_identity(key_content: &str) -> Result<age::x25519::Identity, ShareError> {
    for line in key_content.lines() {
        if line.starts_with("AGE-SECRET-KEY-") {
            return line
                .parse()
                .map_err(|_| ShareError::DecryptFailed("Invalid secret key in age.key".into()));
        }
    }
    Err(ShareError::DecryptFailed(
        "No secret key found in age.key file".into(),
    ))
}

/// Check if a share package is expired.
pub fn is_expired(package: &SharePackage) -> bool {
    if let Some(ref expires_at) = package.metadata.expires_at {
        if let Ok(expiry) = chrono::DateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S%z") {
            return chrono::Local::now() > expiry;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Decrypt a share using an explicit identity (for CI-safe tests).
    fn receive_share_with_identity(
        encrypted_data: &[u8],
        identity: &age::x25519::Identity,
    ) -> Result<SharePackage, ShareError> {
        let decryptor = age::Decryptor::new(encrypted_data)
            .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

        let mut reader = decryptor
            .decrypt(std::iter::once(identity as &dyn age::Identity))
            .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

        let mut decrypted = String::new();
        reader
            .read_to_string(&mut decrypted)
            .map_err(|e| ShareError::DecryptFailed(e.to_string()))?;

        let package: SharePackage = toml::from_str(&decrypted)
            .map_err(|e| ShareError::InvalidFormat(format!("TOML parse failed: {e}")))?;

        Ok(package)
    }

    #[test]
    fn test_create_receive_roundtrip() {
        // Generate an ephemeral key pair — no filesystem dependency
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();

        let entries = vec![
            ("API_KEY".to_string(), "sk-test-123".to_string()),
            ("DB_HOST".to_string(), "localhost".to_string()),
        ];

        let encrypted = create_share(&entries, &pubkey, None).expect("create_share");
        assert!(!encrypted.is_empty());

        let package = receive_share_with_identity(&encrypted, &identity).expect("receive_share");
        assert_eq!(package.metadata.key_count, 2);
        assert_eq!(package.entries.get("API_KEY").unwrap(), "sk-test-123");
        assert_eq!(package.entries.get("DB_HOST").unwrap(), "localhost");
        assert!(package.metadata.expires_at.is_none());
    }

    #[test]
    fn test_share_metadata_serialization() {
        let meta = ShareMeta {
            created_at: "2026-04-20T12:00:00+0000".to_string(),
            created_by: "test-host".to_string(),
            key_count: 3,
            expires_at: Some("2026-04-21T12:00:00+0000".to_string()),
        };

        let mut entries = BTreeMap::new();
        entries.insert("KEY1".to_string(), "val1".to_string());
        entries.insert("KEY2".to_string(), "val2".to_string());
        entries.insert("KEY3".to_string(), "val3".to_string());

        let package = SharePackage {
            metadata: meta,
            entries,
        };

        let toml_str = toml::to_string_pretty(&package).expect("serialize");
        let parsed: SharePackage = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(parsed.metadata.key_count, 3);
        assert_eq!(parsed.metadata.created_by, "test-host");
        assert_eq!(parsed.entries.len(), 3);
        assert_eq!(parsed.entries.get("KEY1").unwrap(), "val1");
    }

    #[test]
    fn test_expired_share_warning() {
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();
        let entries = vec![("SECRET".to_string(), "value".to_string())];

        // Create a share that expires in 0 hours (already expired)
        let encrypted = create_share(&entries, &pubkey, Some(0)).expect("create_share");

        // receive_share should still succeed (warn but not block)
        let package = receive_share_with_identity(&encrypted, &identity).expect("receive_share");
        assert_eq!(package.entries.get("SECRET").unwrap(), "value");

        // is_expired should return true
        assert!(is_expired(&package));
    }

    #[test]
    fn test_create_share_invalid_pubkey() {
        let entries = vec![("K".to_string(), "V".to_string())];
        let result = create_share(&entries, "not-a-valid-key", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid recipient public key"));
    }

    #[test]
    fn test_receive_share_invalid_data() {
        let result = receive_share(b"this is not encrypted data");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_share_with_expiry() {
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();
        let entries = vec![("TOKEN".to_string(), "abc123".to_string())];

        let encrypted = create_share(&entries, &pubkey, Some(24)).expect("create_share");
        let package = receive_share_with_identity(&encrypted, &identity).expect("receive_share");

        assert!(package.metadata.expires_at.is_some());
        // Should NOT be expired (24 hours from now)
        assert!(!is_expired(&package));
    }
}
