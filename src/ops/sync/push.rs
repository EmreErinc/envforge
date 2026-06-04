use std::path::Path;

use super::diff::compute_diff;
use super::git::{GitCommandRunner, GitOps};
use super::init::*;
use super::marking::get_key_status;
use super::model::*;

/// Push local ENV state to sync repository.
pub fn push(
    sync_path: &Path,
    entries: &[(String, String)],
    message: Option<&str>,
    dry_run: bool,
) -> Result<PushSummary, SyncError> {
    let config_path = sync_path.join(CONFIG_FILE);
    let snapshot_path = sync_path.join(SNAPSHOT_FILE);
    let config = read_config(&config_path)?;

    // Filter to sync-marked keys only
    let sync_entries = filter_sync_entries(entries, &config);

    if sync_entries.is_empty() {
        return Err(SyncError::NoKeysMarked);
    }

    // Read current snapshot
    let current_snapshot = read_snapshot(&snapshot_path, &config.sync.encryption_policy, false)?;

    // Compute diff
    let diff = compute_diff(&sync_entries, &current_snapshot.entries);

    if diff.is_empty() {
        return Err(SyncError::NothingToSync);
    }

    let keys_pushed = sync_entries.len();
    let commit_msg = message.unwrap_or("").to_string();
    let auto_msg = format!(
        "sync: {} keys updated from {}",
        diff.total_changes(),
        config.sync.machine_id
    );
    let final_msg = if commit_msg.is_empty() {
        &auto_msg
    } else {
        &commit_msg
    };

    if dry_run {
        return Ok(PushSummary {
            keys_pushed,
            commit_hash: None,
            push_result: PushResult::NoRemote,
            message: format!("[dry-run] would push {} changes", diff.total_changes()),
        });
    }

    // Build new snapshot
    let new_snapshot = export_to_snapshot(&sync_entries, &config.sync.machine_id);

    // Write snapshot atomically (encrypted if config says so)
    write_snapshot_encrypted(&snapshot_path, &new_snapshot)?;

    // Git operations
    let git = GitCommandRunner::new(sync_path.to_path_buf());
    git.add_all()?;
    let commit_hash = git.commit(final_msg)?;
    let push_result = git.push()?;

    Ok(PushSummary {
        keys_pushed,
        commit_hash: Some(commit_hash),
        push_result,
        message: final_msg.clone(),
    })
}

/// Export entries to a SyncSnapshot.
pub fn export_to_snapshot(entries: &[(String, String)], machine_id: &str) -> SyncSnapshot {
    let sync_entries: Vec<SyncEntry> = entries
        .iter()
        .map(|(k, v)| SyncEntry {
            key: k.clone(),
            value: v.clone(),
            profile: None,
            group: None,
        })
        .collect();

    SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: machine_id.to_string(),
        },
        entries: sync_entries,
    }
}

/// Filter entries to only those marked for sync.
fn filter_sync_entries(entries: &[(String, String)], config: &SyncConfig) -> Vec<(String, String)> {
    entries
        .iter()
        .filter(|(k, _)| get_key_status(k, config) == KeyStatus::Synced)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::sync::SyncEncryptionPolicy;

    #[test]
    fn test_export_to_snapshot() {
        let entries = vec![
            ("KEY_A".to_string(), "val_a".to_string()),
            ("KEY_B".to_string(), "val_b".to_string()),
        ];

        let snapshot = export_to_snapshot(&entries, "test-machine");
        assert_eq!(snapshot.metadata.version, 1);
        assert_eq!(snapshot.metadata.created_by, "test-machine");
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].key, "KEY_A");
        assert_eq!(snapshot.entries[1].key, "KEY_B");
    }

    #[test]
    fn test_filter_sync_entries() {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: false,
                auto_push: false,
                conflict_strategy: ConflictStrategy::Ask,
                encrypted: true,
                encryption_policy: SyncEncryptionPolicy::MigrationUntil(
                    "2099-01-01T00:00:00Z".into(),
                ),
                verify_signatures: false,
                enforce_ssh: false,
            },
            manifest: ManifestConfig {
                sync_keys: vec!["SYNC_KEY".to_string()],
                local_keys: vec!["LOCAL_KEY".to_string()],
                patterns: vec![],
            },
        };

        let entries = vec![
            ("SYNC_KEY".to_string(), "val1".to_string()),
            ("LOCAL_KEY".to_string(), "val2".to_string()),
            ("UNSET_KEY".to_string(), "val3".to_string()),
        ];

        let filtered = filter_sync_entries(&entries, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "SYNC_KEY");
    }

    #[test]
    fn test_push_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        // Mark a key
        let config_path = sync_path.join(CONFIG_FILE);
        let mut config = read_config(&config_path).unwrap();
        config.manifest.sync_keys = vec!["MY_KEY".to_string()];
        write_config(&config_path, &config).unwrap();

        let entries = vec![("MY_KEY".to_string(), "my_value".to_string())];
        let result = push(&sync_path, &entries, None, true).unwrap();

        assert!(result.commit_hash.is_none());
        assert!(result.message.contains("dry-run"));
    }

    #[test]
    fn test_push_no_keys_marked() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        let entries = vec![("MY_KEY".to_string(), "val".to_string())];
        let result = push(&sync_path, &entries, None, false);

        assert!(matches!(result, Err(SyncError::NoKeysMarked)));
    }

    #[test]
    fn test_push_creates_commit() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        // Mark a key
        let config_path = sync_path.join(CONFIG_FILE);
        let mut config = read_config(&config_path).unwrap();
        config.manifest.sync_keys = vec!["DB_URL".to_string()];
        write_config(&config_path, &config).unwrap();

        let entries = vec![("DB_URL".to_string(), "postgres://localhost".to_string())];
        let result = push(&sync_path, &entries, Some("test push"), false).unwrap();

        assert!(result.commit_hash.is_some());
        assert_eq!(result.push_result, PushResult::NoRemote);
        assert_eq!(result.message, "test push");

        // Verify snapshot was written
        let snapshot = read_snapshot(
            &sync_path.join(SNAPSHOT_FILE),
            &SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
            false,
        )
        .unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].key, "DB_URL");
        assert_eq!(snapshot.entries[0].value, "postgres://localhost");
    }

    #[test]
    fn test_push_nothing_to_sync() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        // Push some data first
        let config_path = sync_path.join(CONFIG_FILE);
        let mut config = read_config(&config_path).unwrap();
        config.manifest.sync_keys = vec!["KEY".to_string()];
        write_config(&config_path, &config).unwrap();

        let entries = vec![("KEY".to_string(), "val".to_string())];
        push(&sync_path, &entries, None, false).unwrap();

        // Push same data again
        let result = push(&sync_path, &entries, None, false);
        assert!(matches!(result, Err(SyncError::NothingToSync)));
    }
}
