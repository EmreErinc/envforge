use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::git::{GitCommandRunner, GitOps};
use super::init::*;
use super::model::*;

/// Sync log file name (gitignored, local only).
const SYNC_LOG_FILE: &str = "sync-log.toml";

/// Maximum number of log entries to keep.
const MAX_LOG_ENTRIES: usize = 100;

// ─── History (Git Log) ──────────────────────────────────────

/// List sync snapshot history from git log.
pub fn list_history(sync_path: &Path, limit: usize) -> Result<Vec<GitCommitInfo>, SyncError> {
    let git = GitCommandRunner::new(sync_path.to_path_buf());
    git.log(limit)
}

// ─── Rollback ────────────────────────────────────────────────

/// Rollback to a specific snapshot commit.
///
/// 1. Backup current snapshot
/// 2. Read snapshot at target commit via `git show`
/// 3. Write it as current snapshot
/// 4. Create a new revert commit
///
/// Returns the backup path.
pub fn rollback_to(sync_path: &Path, commit_hash: &str) -> Result<PathBuf, SyncError> {
    let snapshot_path = sync_path.join(SNAPSHOT_FILE);

    // Backup current snapshot
    let backup_path = backup_current_snapshot(sync_path)?;

    // Read snapshot content at target commit (may be encrypted)
    let git = GitCommandRunner::new(sync_path.to_path_buf());
    let old_content = git.show(commit_hash, SNAPSHOT_FILE)?;

    // Decrypt if needed, then verify it's valid TOML before writing
    let toml_content = super::encryption::decrypt_snapshot(
        &old_content,
        &crate::ops::sync::model::SyncEncryptionPolicy::MigrationUntil(
            "2099-01-01T00:00:00Z".into(),
        ),
        false,
    )
    .map_err(|_| SyncError::SnapshotParseError {
        message: format!("snapshot at commit {} could not be decrypted", commit_hash),
    })?;

    let _: SyncSnapshot =
        toml::from_str(&toml_content).map_err(|e| SyncError::SnapshotParseError {
            message: format!("snapshot at commit {} is invalid: {}", commit_hash, e),
        })?;

    // Write restored snapshot (keep as-is — preserves encryption state from that commit)
    std::fs::write(&snapshot_path, &old_content).map_err(|e| SyncError::IoError {
        path: snapshot_path,
        source: e,
    })?;

    // Commit the rollback
    let short_hash = if commit_hash.len() > 7 {
        &commit_hash[..7]
    } else {
        commit_hash
    };
    git.add_all()?;
    git.commit(&format!("rollback: restored to {}", short_hash))?;

    Ok(backup_path)
}

/// Rollback to the previous snapshot (HEAD~1).
pub fn rollback_last(sync_path: &Path) -> Result<PathBuf, SyncError> {
    let git = GitCommandRunner::new(sync_path.to_path_buf());
    let log = git.log(2)?;

    if log.len() < 2 {
        return Err(SyncError::GitCommandFailed {
            command: "rollback --last".to_string(),
            stderr: "No previous snapshot to rollback to (only 1 commit exists)".to_string(),
        });
    }

    let target_hash = &log[1].hash;
    rollback_to(sync_path, target_hash)
}

/// Backup current snapshot before rollback.
fn backup_current_snapshot(sync_path: &Path) -> Result<PathBuf, SyncError> {
    let snapshot_path = sync_path.join(SNAPSHOT_FILE);
    if !snapshot_path.exists() {
        return Ok(PathBuf::new());
    }

    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let backup_name = format!(".rollback-backup-{}.toml", timestamp);
    let backup_path = sync_path.join(backup_name);

    std::fs::copy(&snapshot_path, &backup_path).map_err(|e| SyncError::IoError {
        path: snapshot_path,
        source: e,
    })?;

    Ok(backup_path)
}

// ─── Sync Log ────────────────────────────────────────────────

/// A single sync operation log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogEntry {
    pub timestamp: String,
    pub operation: String,
    pub summary: String,
}

/// The sync operation log.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncLog {
    #[serde(default)]
    pub entries: Vec<SyncLogEntry>,
}

/// Read the sync log.
pub fn read_sync_log(sync_path: &Path) -> Result<SyncLog, SyncError> {
    let log_path = sync_path.join(SYNC_LOG_FILE);
    if !log_path.exists() {
        return Ok(SyncLog::default());
    }

    let content = std::fs::read_to_string(&log_path).map_err(|e| SyncError::IoError {
        path: log_path,
        source: e,
    })?;

    match toml::from_str(&content) {
        Ok(log) => Ok(log),
        Err(_) => Ok(SyncLog::default()), // If corrupt, return empty (non-critical)
    }
}

/// Append an entry to the sync log.
pub fn append_sync_log(sync_path: &Path, operation: &str, summary: &str) -> Result<(), SyncError> {
    let log_path = sync_path.join(SYNC_LOG_FILE);
    let mut log = read_sync_log(sync_path)?;

    log.entries.push(SyncLogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        operation: operation.to_string(),
        summary: summary.to_string(),
    });

    // Rotate: keep only last MAX_LOG_ENTRIES
    if log.entries.len() > MAX_LOG_ENTRIES {
        let start = log.entries.len() - MAX_LOG_ENTRIES;
        log.entries = log.entries[start..].to_vec();
    }

    let content = toml::to_string_pretty(&log).map_err(|e| SyncError::ConfigParseError {
        message: e.to_string(),
    })?;

    std::fs::write(&log_path, content).map_err(|e| SyncError::IoError {
        path: log_path,
        source: e,
    })
}

/// Get the last N sync log entries.
pub fn get_sync_log(sync_path: &Path, limit: usize) -> Result<Vec<SyncLogEntry>, SyncError> {
    let log = read_sync_log(sync_path)?;
    let start = if log.entries.len() > limit {
        log.entries.len() - limit
    } else {
        0
    };
    Ok(log.entries[start..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::sync::init::init_fresh;
    use crate::ops::sync::push::push;
    use crate::ops::sync::SyncEncryptionPolicy;

    fn setup_sync_with_data() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        // Mark keys and push initial data
        let config_path = sync_path.join(CONFIG_FILE);
        let mut config = read_config(&config_path).unwrap();
        config.manifest.sync_keys = vec!["KEY_A".to_string(), "KEY_B".to_string()];
        write_config(&config_path, &config).unwrap();

        let entries = vec![
            ("KEY_A".to_string(), "value_a".to_string()),
            ("KEY_B".to_string(), "value_b".to_string()),
        ];
        push(&sync_path, &entries, Some("first push"), false).unwrap();

        (dir, sync_path)
    }

    #[test]
    fn test_list_history() {
        let (_dir, sync_path) = setup_sync_with_data();
        let history = list_history(&sync_path, 10).unwrap();
        assert!(history.len() >= 2); // init commit + push commit
    }

    #[test]
    fn test_sync_log_append_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        append_sync_log(&sync_path, "push", "5 keys updated").unwrap();
        append_sync_log(&sync_path, "pull", "+2 added, ~1 modified").unwrap();

        let entries = get_sync_log(&sync_path, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].operation, "push");
        assert_eq!(entries[1].operation, "pull");
    }

    #[test]
    fn test_sync_log_empty() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        let entries = get_sync_log(&sync_path, 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sync_log_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        for i in 0..110 {
            append_sync_log(&sync_path, "push", &format!("entry {}", i)).unwrap();
        }

        let log = read_sync_log(&sync_path).unwrap();
        assert_eq!(log.entries.len(), MAX_LOG_ENTRIES);
        // First entry should be #10 (0-9 rotated out)
        assert!(log.entries[0].summary.contains("10"));
    }

    #[test]
    fn test_rollback_to_previous() {
        let (_dir, sync_path) = setup_sync_with_data();

        // Push updated data
        let entries = vec![
            ("KEY_A".to_string(), "updated_a".to_string()),
            ("KEY_B".to_string(), "updated_b".to_string()),
        ];
        push(&sync_path, &entries, Some("second push"), false).unwrap();

        // Verify current state is updated
        let snapshot = read_snapshot(
            &sync_path.join(SNAPSHOT_FILE),
            &SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
            false,
        )
        .unwrap();
        assert_eq!(snapshot.entries[0].value, "updated_a");

        // Rollback to previous
        let backup = rollback_last(&sync_path).unwrap();
        assert!(backup.exists());

        // Verify rollback restored original values
        let restored = read_snapshot(
            &sync_path.join(SNAPSHOT_FILE),
            &SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
            false,
        )
        .unwrap();
        assert_eq!(restored.entries[0].value, "value_a");
        assert_eq!(restored.entries[1].value, "value_b");
    }

    #[test]
    fn test_rollback_creates_new_commit() {
        let (_dir, sync_path) = setup_sync_with_data();

        // Push update
        let entries = vec![
            ("KEY_A".to_string(), "v2".to_string()),
            ("KEY_B".to_string(), "v2".to_string()),
        ];
        push(&sync_path, &entries, Some("update"), false).unwrap();

        let before_count = list_history(&sync_path, 100).unwrap().len();
        rollback_last(&sync_path).unwrap();
        let after_count = list_history(&sync_path, 100).unwrap().len();

        assert_eq!(after_count, before_count + 1); // rollback created a new commit
    }

    #[test]
    fn test_rollback_last_on_single_commit_fails() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        let result = rollback_last(&sync_path);
        assert!(result.is_err());
    }
}
