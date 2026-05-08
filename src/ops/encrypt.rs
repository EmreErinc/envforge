use std::io::{Read, Write};
use std::path::PathBuf;

use age::secrecy::ExposeSecret;

use crate::config::config_dir;
use crate::model::ParseError;

const ENC_PREFIX: &str = "ENC[age:";
const ENC_SUFFIX_STR: &str = "]";

/// Get the age key file path.
pub fn age_key_path() -> Result<PathBuf, ParseError> {
    Ok(config_dir()?.join("age.key"))
}

/// Ensure an age keypair exists, generating one if needed.
pub fn ensure_age_key() -> Result<String, EncryptError> {
    let path =
        age_key_path().map_err(|_| EncryptError::KeyError("Cannot find config dir".into()))?;

    if path.exists() {
        // Verify and fix permissions on existing key file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                let mode = metadata.permissions().mode();
                if mode & 0o077 != 0 {
                    log::warn!(
                        "age key file {} has overly permissive permissions ({:o}), fixing to 0600",
                        path.display(),
                        mode & 0o777
                    );
                    let perms = std::fs::Permissions::from_mode(0o600);
                    let _ = std::fs::set_permissions(&path, perms);
                }
            }
        }

        std::fs::read_to_string(&path)
            .map_err(|e| EncryptError::KeyError(format!("Cannot read key: {}", e)))
    } else {
        let key = age::x25519::Identity::generate();
        let secret = key.to_string();
        let public = key.to_public().to_string();

        let content = format!(
            "# created by envforge\n# public key: {}\n{}\n",
            public,
            secret.expose_secret()
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| EncryptError::KeyError(format!("Cannot create dir: {}", e)))?;
        }

        // Write key file with restrictive permissions (0600 on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let tempfile = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(&path))
                .map_err(|e| EncryptError::KeyError(format!("Cannot create temp file: {}", e)))?;
            tempfile
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| EncryptError::KeyError(format!("Cannot set permissions: {}", e)))?;
            std::fs::write(tempfile.path(), &content)
                .map_err(|e| EncryptError::KeyError(format!("Cannot write key: {}", e)))?;
            tempfile.persist(&path).map_err(|e| {
                EncryptError::KeyError(format!("Cannot persist key file: {}", e.error))
            })?;
        }

        #[cfg(not(unix))]
        {
            // EnvForge does not currently support secure key storage on
            // non-unix targets (no portable way to set 0600 ACLs from
            // tempfile before persist). Refuse to write the key rather
            // than leak a world-readable private key on Windows.
            return Err(EncryptError::KeyError(
                "secure age key storage requires a unix-like OS".into(),
            ));
        }

        Ok(content)
    }
}

fn get_recipient(key_content: &str) -> Result<age::x25519::Recipient, EncryptError> {
    for line in key_content.lines() {
        if line.starts_with("# public key: ") {
            let pubkey_str = line.trim_start_matches("# public key: ");
            return pubkey_str
                .parse()
                .map_err(|_| EncryptError::KeyError("Invalid public key".into()));
        }
    }
    Err(EncryptError::KeyError(
        "No public key found in key file".into(),
    ))
}

fn get_identity(key_content: &str) -> Result<age::x25519::Identity, EncryptError> {
    for line in key_content.lines() {
        if line.starts_with("AGE-SECRET-KEY-") {
            return line
                .parse()
                .map_err(|_| EncryptError::KeyError("Invalid secret key".into()));
        }
    }
    Err(EncryptError::KeyError(
        "No secret key found in key file".into(),
    ))
}

/// Encrypt a plain text value.
pub fn encrypt_value(plain: &str) -> Result<String, EncryptError> {
    let key_content = ensure_age_key()?;
    let recipient = get_recipient(&key_content)?;

    let recipients: Vec<&dyn age::Recipient> = vec![&recipient];
    let encryptor = age::Encryptor::with_recipients(recipients.into_iter())
        .map_err(|_| EncryptError::EncryptFailed("No recipients".into()))?;

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| EncryptError::EncryptFailed(e.to_string()))?;
    writer
        .write_all(plain.as_bytes())
        .map_err(|e| EncryptError::EncryptFailed(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| EncryptError::EncryptFailed(e.to_string()))?;

    let encoded = base64_encode(&encrypted);
    Ok(format!("{}{}{}", ENC_PREFIX, encoded, ENC_SUFFIX_STR))
}

/// Maximum decoded ciphertext size accepted by [`decrypt_value`].
/// Legitimate ENV values are well under 1 MiB; bigger inputs are
/// rejected to defend against decompression-bomb-style age files that
/// would balloon during `read_to_string`.
const MAX_CIPHERTEXT_BYTES: usize = 1024 * 1024;

/// Maximum decrypted plaintext size read from an age stream. Bounds
/// memory growth even if the decryptor itself surfaces an unexpectedly
/// large stream from a crafted ciphertext.
const MAX_PLAINTEXT_BYTES: u64 = 1024 * 1024;

/// Decrypt an encrypted value.
pub fn decrypt_value(encrypted: &str) -> Result<String, EncryptError> {
    let key_content = ensure_age_key()?;
    let identity = get_identity(&key_content)?;

    let data = extract_encrypted_data(encrypted)?;
    let decoded = base64_decode(&data)?;
    if decoded.len() > MAX_CIPHERTEXT_BYTES {
        return Err(EncryptError::DecryptFailed(format!(
            "ciphertext too large ({} bytes, max {})",
            decoded.len(),
            MAX_CIPHERTEXT_BYTES
        )));
    }

    let decryptor = age::Decryptor::new(&decoded[..])
        .map_err(|e| EncryptError::DecryptFailed(e.to_string()))?;

    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| EncryptError::DecryptFailed(e.to_string()))?;

    let mut limited = std::io::Read::take(reader, MAX_PLAINTEXT_BYTES);
    let mut decrypted = String::new();
    limited
        .read_to_string(&mut decrypted)
        .map_err(|e| EncryptError::DecryptFailed(e.to_string()))?;

    Ok(decrypted)
}

/// Check if a value is encrypted.
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(ENC_PREFIX) && value.ends_with(ENC_SUFFIX_STR)
}

fn extract_encrypted_data(value: &str) -> Result<String, EncryptError> {
    if !is_encrypted(value) {
        return Err(EncryptError::DecryptFailed("Not an encrypted value".into()));
    }
    let data = &value[ENC_PREFIX.len()..value.len() - ENC_SUFFIX_STR.len()];
    Ok(data.to_string())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, EncryptError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| EncryptError::DecryptFailed(format!("Base64 decode failed: {}", e)))
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("key error: {0}")]
    KeyError(String),

    #[error("encryption failed: {0}")]
    EncryptFailed(String),

    #[error("decryption failed: {0}")]
    DecryptFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_encrypted_true() {
        assert!(is_encrypted("ENC[age:base64data]"));
    }

    #[test]
    fn test_is_encrypted_false() {
        assert!(!is_encrypted("plaintext"));
        assert!(!is_encrypted("ENC[other:data]"));
        assert!(!is_encrypted(""));
    }

    #[test]
    fn test_extract_encrypted_data_valid() {
        let data = extract_encrypted_data("ENC[age:someb64data]").unwrap();
        assert_eq!(data, "someb64data");
    }

    #[test]
    fn test_extract_encrypted_data_not_encrypted() {
        let result = extract_encrypted_data("plaintext");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_recipient_valid() {
        let key_content = "# created by envforge\n# public key: age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p\nAGE-SECRET-KEY-FAKE\n";
        let result = get_recipient(key_content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_recipient_missing_pubkey() {
        let key_content = "# no public key here\nAGE-SECRET-KEY-FAKE\n";
        let result = get_recipient(key_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_identity_valid() {
        // Generate a real key to get a valid secret key string
        use age::secrecy::ExposeSecret;
        let key = age::x25519::Identity::generate();
        let secret = key.to_string();
        let content = format!("# comment\n{}\n", secret.expose_secret());
        let result = get_identity(&content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_identity_missing_secret() {
        let content = "# public key: age1xyz\n# no secret key\n";
        let result = get_identity(content);
        assert!(result.is_err());
    }
}
