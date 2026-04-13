use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

/// Error types for atomic write operations.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("failed to write to '{path}': {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("hash mismatch for '{path}': file was modified externally")]
    HashMismatch { path: PathBuf },

    #[error("failed to create temp file in '{dir}': {source}")]
    TempFileError {
        dir: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to persist temp file to '{path}': {source}")]
    PersistError {
        path: PathBuf,
        source: tempfile::PersistError,
    },
}

/// Write content to a file atomically.
///
/// 1. If `expected_hash` is provided, reads the current file and verifies its hash.
///    Aborts with `HashMismatch` if the hash doesn't match.
/// 2. Writes content to a temp file in the same directory as the target.
/// 3. Renames the temp file to the target path (atomic on same filesystem).
///
/// On any failure, the original file is left untouched.
pub fn atomic_write(
    path: &Path,
    content: &str,
    expected_hash: Option<[u8; 32]>,
) -> Result<(), WriteError> {
    // Step 1: Hash verification
    if let Some(expected) = expected_hash {
        if path.exists() {
            let current_content = std::fs::read(path).map_err(|e| WriteError::IoError {
                path: path.to_path_buf(),
                source: e,
            })?;
            let current_hash = compute_hash(&current_content);
            if current_hash != expected {
                return Err(WriteError::HashMismatch {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    // Step 2: Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WriteError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // Step 3: Write to temp file in same directory
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut temp = NamedTempFile::new_in(dir).map_err(|e| WriteError::TempFileError {
        dir: dir.to_path_buf(),
        source: e,
    })?;

    temp.write_all(content.as_bytes())
        .map_err(|e| WriteError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;

    temp.flush().map_err(|e| WriteError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Step 4: Atomic rename
    temp.persist(path).map_err(|e| WriteError::PersistError {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Compute SHA-256 hash of data.
pub fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Write content to a file atomically with backup.
///
/// Convenience function that:
/// 1. Creates a backup of the existing file (if it exists)
/// 2. Verifies hash if provided
/// 3. Writes atomically
pub fn safe_write(
    path: &Path,
    content: &str,
    expected_hash: Option<[u8; 32]>,
) -> Result<(), WriteError> {
    // Create backup if file exists
    if path.exists() {
        use super::backup::{cleanup_backups, create_backup, MAX_BACKUPS};

        create_backup(path).map_err(|e| WriteError::IoError {
            path: path.to_path_buf(),
            source: std::io::Error::other(e.to_string()),
        })?;

        let _ = cleanup_backups(path, MAX_BACKUPS);
    }

    atomic_write(path, content, expected_hash)
}
