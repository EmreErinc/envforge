use std::io::{Read, Write};

use super::model::SyncError;

/// Magic header that identifies an encrypted snapshot file.
const ENCRYPTED_HEADER: &str = "ENVFORGE-ENCRYPTED\n";

/// Encrypt a TOML string with age.
///
/// Returns content prefixed with "ENVFORGE-ENCRYPTED\n" followed by
/// base64-encoded age-encrypted data.
pub fn encrypt_snapshot(toml_content: &str) -> Result<String, SyncError> {
    let key_content =
        crate::ops::encrypt::ensure_age_key().map_err(|e| SyncError::EncryptionFailed {
            message: e.to_string(),
        })?;

    let recipient = get_recipient(&key_content)?;

    let recipients: Vec<&dyn age::Recipient> = vec![&recipient];
    let encryptor = age::Encryptor::with_recipients(recipients.into_iter()).map_err(|_| {
        SyncError::EncryptionFailed {
            message: "No recipients".into(),
        }
    })?;

    let mut encrypted = vec![];
    let mut writer =
        encryptor
            .wrap_output(&mut encrypted)
            .map_err(|e| SyncError::EncryptionFailed {
                message: e.to_string(),
            })?;
    writer
        .write_all(toml_content.as_bytes())
        .map_err(|e| SyncError::EncryptionFailed {
            message: e.to_string(),
        })?;
    writer.finish().map_err(|e| SyncError::EncryptionFailed {
        message: e.to_string(),
    })?;

    let encoded = base64_encode(&encrypted);
    Ok(format!("{}{}", ENCRYPTED_HEADER, encoded))
}

/// Maximum decoded ciphertext size accepted by [`decrypt_snapshot`].
/// Snapshots are TOML files containing tracked ENV entries; legitimate
/// content fits comfortably in single-digit MiB. The cap defends against
/// decompression-bomb-style age files in a malicious sync remote
/// causing OOM during `envforge sync pull`. Mirrors the cap in
/// `src/ops/encrypt.rs` (0.7.5 O7).
const MAX_SNAPSHOT_CIPHERTEXT_BYTES: usize = 8 * 1024 * 1024;

/// Maximum decrypted plaintext size read from the age stream. Bounds
/// memory growth even if the decryptor surfaces an unexpectedly large
/// stream from a crafted ciphertext.
const MAX_SNAPSHOT_PLAINTEXT_BYTES: u64 = 8 * 1024 * 1024;

/// Decrypt an encrypted snapshot back to TOML string.
///
/// Auto-detects encrypted vs plaintext. If the content does not start with the
/// magic header, it is returned as-is (plaintext passthrough for backward compat).
pub fn decrypt_snapshot(content: &str) -> Result<String, SyncError> {
    if !is_encrypted_snapshot(content) {
        return Ok(content.to_string());
    }

    let encoded = &content[ENCRYPTED_HEADER.len()..];
    let decoded = base64_decode(encoded)?;
    if decoded.len() > MAX_SNAPSHOT_CIPHERTEXT_BYTES {
        return Err(SyncError::EncryptionFailed {
            message: format!(
                "encrypted snapshot too large ({} bytes, max {})",
                decoded.len(),
                MAX_SNAPSHOT_CIPHERTEXT_BYTES
            ),
        });
    }

    let key_content =
        crate::ops::encrypt::ensure_age_key().map_err(|e| SyncError::EncryptionFailed {
            message: e.to_string(),
        })?;
    let identity = get_identity(&key_content)?;

    let decryptor = age::Decryptor::new(&decoded[..]).map_err(|_| SyncError::DecryptionFailed)?;

    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| SyncError::DecryptionFailed)?;

    let mut limited = std::io::Read::take(reader, MAX_SNAPSHOT_PLAINTEXT_BYTES);
    let mut decrypted = String::new();
    limited
        .read_to_string(&mut decrypted)
        .map_err(|_| SyncError::DecryptionFailed)?;

    Ok(decrypted)
}

/// Check if content is an encrypted snapshot (has the magic header).
pub fn is_encrypted_snapshot(content: &str) -> bool {
    content.starts_with(ENCRYPTED_HEADER)
}

// ─── Internal helpers (same pattern as crate::ops::encrypt) ─────

fn get_recipient(key_content: &str) -> Result<age::x25519::Recipient, SyncError> {
    for line in key_content.lines() {
        if line.starts_with("# public key: ") {
            let pubkey_str = line.trim_start_matches("# public key: ");
            return pubkey_str.parse().map_err(|_| SyncError::EncryptionFailed {
                message: "Invalid public key in age key file".into(),
            });
        }
    }
    Err(SyncError::EncryptionFailed {
        message: "No public key found in age key file".into(),
    })
}

fn get_identity(key_content: &str) -> Result<age::x25519::Identity, SyncError> {
    for line in key_content.lines() {
        if line.starts_with("AGE-SECRET-KEY-") {
            return line.parse().map_err(|_| SyncError::DecryptionFailed);
        }
    }
    Err(SyncError::DecryptionFailed)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, SyncError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| SyncError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let toml_content = r#"[metadata]
version = 1
created_at = "2026-04-15T10:00:00Z"
created_by = "test-machine"

[[entries]]
key = "DATABASE_URL"
value = "postgres://localhost:5432/db"
"#;

        let encrypted = encrypt_snapshot(toml_content).unwrap();
        assert!(is_encrypted_snapshot(&encrypted));
        assert!(encrypted.starts_with("ENVFORGE-ENCRYPTED\n"));

        let decrypted = decrypt_snapshot(&encrypted).unwrap();
        assert_eq!(decrypted, toml_content);
    }

    #[test]
    fn test_is_encrypted_snapshot_detection() {
        assert!(is_encrypted_snapshot("ENVFORGE-ENCRYPTED\nSomeBase64Data"));
        assert!(!is_encrypted_snapshot("[metadata]\nversion = 1\n"));
        assert!(!is_encrypted_snapshot(""));
        assert!(!is_encrypted_snapshot("ENVFORGE-ENCRYPTED")); // no newline
    }

    #[test]
    fn test_plaintext_passthrough() {
        let toml_content = "[metadata]\nversion = 1\n";
        let result = decrypt_snapshot(toml_content).unwrap();
        assert_eq!(result, toml_content);
    }

    #[test]
    fn test_encrypt_small_content() {
        let small = "key = \"value\"\n";
        let encrypted = encrypt_snapshot(small).unwrap();
        let decrypted = decrypt_snapshot(&encrypted).unwrap();
        assert_eq!(decrypted, small);
    }

    #[test]
    fn test_encrypt_multiline_content() {
        let content = "[metadata]\nversion = 1\n\n[[entries]]\nkey = \"A\"\nvalue = \"B\"\n";
        let encrypted = encrypt_snapshot(content).unwrap();
        let decrypted = decrypt_snapshot(&encrypted).unwrap();
        assert_eq!(decrypted, content);
    }

    #[test]
    fn test_corrupted_encrypted_data() {
        let corrupted = "ENVFORGE-ENCRYPTED\nNotValidBase64!!!@@@";
        let result = decrypt_snapshot(corrupted);
        assert!(result.is_err());
    }
}
