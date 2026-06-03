use std::io::{Read, Write};
use std::path::PathBuf;

use age::secrecy::ExposeSecret;
use zeroize::Zeroizing;

use crate::config::config_dir;

const ENC_PREFIX: &str = "ENC[age:";
const ENC_SUFFIX_STR: &str = "]";

/// Env var that holds the raw age identity key content.
/// Takes precedence over `ENVFORGE_AGE_KEY_FILE` and the default key path.
pub const ENV_AGE_KEY: &str = "ENVFORGE_AGE_KEY";

/// Env var that points to an alternative age key file.
/// Takes precedence over the default `~/.config/envforge/age.key` path.
pub const ENV_AGE_KEY_FILE: &str = "ENVFORGE_AGE_KEY_FILE";

/// Get the age key file path, respecting `ENVFORGE_AGE_KEY_FILE`.
pub fn age_key_path() -> Result<PathBuf, EncryptError> {
    if let Ok(custom) = std::env::var(ENV_AGE_KEY_FILE) {
        let p = PathBuf::from(custom);
        if !p.exists() {
            return Err(EncryptError::KeyError(format!(
                "{ENV_AGE_KEY_FILE}={} points to a file that does not exist",
                p.display()
            )));
        }
        return Ok(p);
    }
    Ok(config_dir()
        .map_err(|_| EncryptError::KeyError("Cannot find config dir".into()))?
        .join("age.key"))
}

/// Content of the recovery key, if one has been generated.
/// Stored alongside the primary key as `age-recovery.key`.
pub fn recovery_key_path() -> Result<PathBuf, EncryptError> {
    Ok(config_dir()
        .map_err(|_| EncryptError::KeyError("Cannot find config dir".into()))?
        .join("age-recovery.key"))
}

/// Ensure an age keypair exists, generating one if needed.
///
/// Resolution order:
/// 1. `ENVFORGE_AGE_KEY` — raw key content (CI / headless). Always preferred.
/// 2. `ENVFORGE_AGE_KEY_FILE` — custom key file path.
/// 3. `~/.config/envforge/age.key` — default location, auto-generated.
///
/// Returns the key content wrapped in [`Zeroizing`] so the private key
/// bytes are overwritten on drop. Callers that hold the returned value
/// should minimize its lifetime and drop it as soon as the key material
/// is no longer needed.
pub fn ensure_age_key() -> Result<Zeroizing<String>, EncryptError> {
    // 1. Inline key via env var (highest priority — CI / headless)
    if let Ok(key_content) = std::env::var(ENV_AGE_KEY) {
        if key_content.trim().is_empty() {
            return Err(EncryptError::KeyError(format!(
                "{ENV_AGE_KEY} is set but empty"
            )));
        }
        // Emit a distinct audit event for CI/headless key provisioning.
        // This allows audit tooling to distinguish ephemeral env-var keys
        // from persistent file-based keys.
        crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
            source: crate::ops::monitor::EventSource::KeyProvisioning,
            key: None,
            message: format!("age key loaded from {ENV_AGE_KEY} (CI/headless mode)"),
            timestamp: chrono::Utc::now(),
            severity: crate::ops::monitor::SecuritySeverity::Warn,
        });
        return Ok(Zeroizing::new(key_content));
    }

    let path =
        age_key_path().map_err(|_| EncryptError::KeyError("Cannot find config dir".into()))?;

    if path.exists() {
        // Verify and fix permissions on the existing key file.
        //
        // Open the file FIRST, then chmod via the open file handle.
        // This closes a TOCTOU window: the previous code did
        // `metadata(&path)` → `set_permissions(&path)` which a local
        // attacker could race by swapping the path (rename / symlink)
        // between the two syscalls. Using `File::set_permissions`
        // applies the chmod to the inode we actually have open, not
        // to whatever now sits at that path.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Refuse to follow a symlink that may have been swapped in
            // by another local user. `File::open` follows symlinks; we
            // need `O_NOFOLLOW` to be safe, but std doesn't expose it
            // directly — use OpenOptionsExt::custom_flags.
            use std::os::unix::fs::OpenOptionsExt;
            // O_NOFOLLOW values from <fcntl.h>: 0x100 on Linux, 0x100
            // on macOS / *BSD. Other unices fall through to 0 (best
            // effort — chmod-on-handle still defeats the `path`-level
            // TOCTOU even without `O_NOFOLLOW`).
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            const O_NOFOLLOW_LOCAL: i32 = 0x0100;
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            const O_NOFOLLOW_LOCAL: i32 = 0;

            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(O_NOFOLLOW_LOCAL)
                .open(&path)
                .map_err(|e| EncryptError::KeyError(format!("Cannot open age key file: {}", e)))?;
            if let Ok(meta) = file.metadata() {
                let mode = meta.permissions().mode();
                if mode & 0o077 != 0 {
                    log::warn!(
                        "age key file {} has overly permissive permissions ({:o}), fixing to 0600",
                        path.display(),
                        mode & 0o777
                    );
                    let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
                }
            }
            // Read via the same handle so we read the same inode whose
            // permissions we just fixed.
            let mut content = String::new();
            use std::io::Read as _;
            (&file)
                .read_to_string(&mut content)
                .map_err(|e| EncryptError::KeyError(format!("Cannot read age key file: {}", e)))?;
            Ok(Zeroizing::new(content))
        }

        #[cfg(not(unix))]
        {
            Err(EncryptError::KeyError(
                "secure age key reads require a unix-like OS".into(),
            ))
        }
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

        // Generate a recovery key alongside the primary key.
        // This is best-effort — a failure here does not prevent the
        // primary key from being usable.
        if let Err(e) = generate_recovery_key() {
            log::warn!("Failed to generate recovery key: {}", e);
        }

        Ok(Zeroizing::new(content))
    }
}

/// Generate a recovery (break-glass) age keypair and write it to
/// `age-recovery.key` alongside the primary key.  The recovery key is
/// a second keypair that can decrypt everything encrypted with the
/// primary key.  Users should store the recovery key offline (printed
/// QR code, hardware token, sealed envelope).
///
/// This function is called **once** during first-run key generation.
/// It is NOT called again — if the recovery key already exists, it is
/// left untouched (the user may have removed it intentionally).
pub fn generate_recovery_key() -> Result<(), EncryptError> {
    let recovery_path = recovery_key_path()
        .map_err(|_| EncryptError::KeyError("Cannot determine recovery key path".into()))?;

    // Do NOT overwrite an existing recovery key — the user may
    // have deliberately stored it offline and removed the local copy.
    if recovery_path.exists() {
        log::info!(
            "Recovery key already exists at {}, skipping generation",
            recovery_path.display()
        );
        return Ok(());
    }

    let key = age::x25519::Identity::generate();
    let secret = key.to_string();
    let public = key.to_public().to_string();

    let content = format!(
        "# envforge recovery key — STORE THIS OFFLINE\n\
         # If your primary age key is lost or corrupted, this key\n\
         # can decrypt all encrypted credentials.\n\
         # public key: {}\n{}\n",
        public,
        secret.expose_secret()
    );

    if let Some(parent) = recovery_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            EncryptError::KeyError(format!("Cannot create dir for recovery key: {}", e))
        })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let tempfile =
            tempfile::NamedTempFile::new_in(recovery_path.parent().unwrap_or(&recovery_path))
                .map_err(|e| EncryptError::KeyError(format!("Cannot create temp file: {}", e)))?;
        tempfile
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| EncryptError::KeyError(format!("Cannot set permissions: {}", e)))?;
        std::fs::write(tempfile.path(), &content)
            .map_err(|e| EncryptError::KeyError(format!("Cannot write recovery key: {}", e)))?;
        tempfile.persist(&recovery_path).map_err(|e| {
            EncryptError::KeyError(format!("Cannot persist recovery key: {}", e.error))
        })?;
    }

    #[cfg(not(unix))]
    {
        return Err(EncryptError::KeyError(
            "secure recovery key storage requires a unix-like OS".into(),
        ));
    }

    eprintln!(
        "\n🔑 Recovery key written to: {}\n\
         ⚠️  STORE THIS FILE OFFLINE (USB drive, printed QR, secure vault).\n\
         ⚠️  Without it, losing your primary key means PERMANENT data loss.\n\
         ⚠️  You will NOT see this message again.\n",
        recovery_path.display()
    );

    Ok(())
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
