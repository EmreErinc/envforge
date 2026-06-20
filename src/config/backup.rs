use std::path::{Path, PathBuf};

use chrono::Local;

use super::app_config::{backups_dir, ConfigError};

/// Create a backup of the given file before writing.
///
/// Copies the file to `~/.config/envforge/backups/{filename}.{timestamp}.bak`.
/// Creates the backups directory if it doesn't exist.
pub fn create_backup(file_path: &Path) -> Result<PathBuf, ConfigError> {
    let backup_dir = backups_dir().map_err(|_| ConfigError::HomeDirNotFound)?;

    std::fs::create_dir_all(&backup_dir).map_err(|e| ConfigError::IoError {
        path: backup_dir.clone(),
        source: e,
    })?;

    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

    let timestamp = Local::now().format("%Y%m%dT%H%M%S");
    let backup_name = format!("{}.{}.bak", file_name, timestamp);
    let backup_path = backup_dir.join(&backup_name);

    // Handle collision (same timestamp)
    let final_path = if backup_path.exists() {
        let mut suffix = 1;
        loop {
            let alt_name = format!("{}.{}_{}.bak", file_name, timestamp, suffix);
            let alt_path = backup_dir.join(&alt_name);
            if !alt_path.exists() {
                break alt_path;
            }
            suffix += 1;
        }
    } else {
        backup_path
    };

    // Backups of `~/.config/envforge/config.toml` may carry secrets / API
    // keys. `std::fs::copy` follows the source mode but applies the umask to
    // the destination, so a 0600 source can land at 0644 — and even a
    // copy-then-chmod leaves a window in which the secret backup is
    // world-readable. On Unix, create the destination 0600 *at creation time*
    // (`final_path` is guaranteed not to exist by the collision loop above) and
    // stream the bytes in, so the backup is never momentarily over-permissioned.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let io_err = |e: std::io::Error, path: &Path| ConfigError::IoError {
            path: path.to_path_buf(),
            source: e,
        };
        let bytes = std::fs::read(file_path).map_err(|e| io_err(e, file_path))?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&final_path)
            .map_err(|e| io_err(e, &final_path))?;
        f.write_all(&bytes).map_err(|e| io_err(e, &final_path))?;
        f.sync_all().map_err(|e| io_err(e, &final_path))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(file_path, &final_path).map_err(|e| ConfigError::IoError {
            path: file_path.to_path_buf(),
            source: e,
        })?;
    }

    Ok(final_path)
}

/// List all backups for a given file, sorted oldest first.
pub fn list_backups(file_path: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let backup_dir = backups_dir().map_err(|_| ConfigError::HomeDirNotFound)?;

    if !backup_dir.exists() {
        return Ok(vec![]);
    }

    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

    let prefix = format!("{}.", file_name);

    let mut backups: Vec<PathBuf> = std::fs::read_dir(&backup_dir)
        .map_err(|e| ConfigError::IoError {
            path: backup_dir.clone(),
            source: e,
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with(&prefix)
                && path.extension().is_some_and(|ext| ext == "bak")
        })
        .collect();

    backups.sort();
    Ok(backups)
}

/// Remove old backups, keeping only the most recent `max_count`.
///
/// Returns the number of backups removed.
pub fn cleanup_backups(file_path: &Path, max_count: usize) -> Result<usize, ConfigError> {
    let backups = list_backups(file_path)?;

    if backups.len() <= max_count {
        return Ok(0);
    }

    let to_remove = backups.len() - max_count;
    let mut removed = 0;

    for path in backups.iter().take(to_remove) {
        if std::fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

/// The maximum number of backups to retain per file.
pub const MAX_BACKUPS: usize = 10;
