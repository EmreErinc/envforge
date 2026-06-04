use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

/// Error types for atomic write operations.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("failed to write file: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("hash mismatch: file was modified externally")]
    HashMismatch { path: PathBuf },

    #[error("failed to create temp file: {source}")]
    TempFileError {
        dir: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to persist temp file: {source}")]
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
    let file_exists = path.exists();

    // Step 1: Create parent directories if needed (before lock acquisition)
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WriteError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // Step 2: Acquire advisory lock on the target file to prevent TOCTOU.
    // The lock spans hash verification → write → rename. Any concurrent
    // writer will block or fail, closing the TOCTOU window (T-004).
    let _lock = acquire_lock(path)?;

    // Step 3: Hash verification (with lock held).
    // Skip if file didn't exist — first-time writes have no prior content to verify.
    if let Some(expected) = expected_hash {
        if file_exists {
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

    // Step 4: Write to temp file in same directory with restrictive permissions.
    // Use fchmod on the raw fd BEFORE writing any data to eliminate the TOCTOU
    // window on macOS where NamedTempFile inherits umask (e.g., 0o644).
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut temp = NamedTempFile::new_in(dir).map_err(|e| WriteError::TempFileError {
        dir: dir.to_path_buf(),
        source: e,
    })?;

    #[cfg(unix)]
    {
        let fd = temp.as_file().as_raw_fd();
        let ret = unsafe { libc::fchmod(fd, 0o600) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            return Err(WriteError::IoError {
                path: path.to_path_buf(),
                source: err,
            });
        }
    }

    temp.write_all(content.as_bytes())
        .map_err(|e| WriteError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;

    temp.flush().map_err(|e| WriteError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    temp.as_file().sync_all().map_err(|e| WriteError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Step 5: Atomic rename (lock still held)
    temp.persist(path).map_err(|e| WriteError::PersistError {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Lock is released when lock_file (LockGuard) is dropped at end of scope
    Ok(())
}

/// Acquire an advisory flock(LOCK_EX) on the target file.
///
/// Returns a guard that releases the lock on drop. Used by `atomic_write`
/// to prevent TOCTOU between hash verification and atomic rename (T-004).
struct LockGuard {
    _file: std::fs::File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

fn acquire_lock(path: &Path) -> Result<LockGuard, WriteError> {
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| WriteError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;

    let ret = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        return Err(WriteError::IoError {
            path: path.to_path_buf(),
            source: err,
        });
    }

    Ok(LockGuard { _file: lock_file })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let data = b"hello world";
        let h1 = compute_hash(data);
        let h2 = compute_hash(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_different_inputs() {
        let h1 = compute_hash(b"hello");
        let h2 = compute_hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_atomic_write_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        atomic_write(&path, "hello", None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn test_atomic_write_hash_mismatch_aborts() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "original").unwrap();

        let wrong_hash = compute_hash(b"different content");
        let result = atomic_write(&path, "new content", Some(wrong_hash));
        assert!(matches!(result, Err(WriteError::HashMismatch { .. })));
        // Original content preserved
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn test_atomic_write_hash_match_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        let original = "original content";
        std::fs::write(&path, original).unwrap();

        let correct_hash = compute_hash(original.as_bytes());
        atomic_write(&path, "new content", Some(correct_hash)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new content");
    }

    #[test]
    fn test_atomic_write_none_hash_always_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "anything").unwrap();
        atomic_write(&path, "replaced", None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "replaced");
    }
}
