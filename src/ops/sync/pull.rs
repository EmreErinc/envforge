use std::path::{Path, PathBuf};

use super::conflict::{auto_resolve, detect_conflicts};
use super::diff::compute_diff;
use super::git::GitOps;
use super::init::*;
use super::model::*;

/// Compute what would change if pull is applied.
///
/// Returns the diff and any detected conflicts.
pub fn compute_pull_changes(
    base_snapshot: &SyncSnapshot,
    remote_snapshot: &SyncSnapshot,
    local_entries: &[(String, String)],
) -> (SyncDiff, Vec<ConflictEntry>) {
    // Diff: what remote has vs what local has
    let diff = compute_diff(local_entries, &remote_snapshot.entries);

    // Conflicts: three-way comparison
    let conflicts = detect_conflicts(
        &base_snapshot.entries,
        local_entries,
        &remote_snapshot.entries,
    );

    (diff, conflicts)
}

/// Pull changes from remote and compute summary.
///
/// This does NOT apply changes — it computes what would change and returns
/// the summary with any conflicts. The caller (CLI) handles actual application.
pub fn pull(
    sync_path: &Path,
    local_entries: &[(String, String)],
    dry_run: bool,
) -> Result<PullSummary, SyncError> {
    let config_path = sync_path.join(CONFIG_FILE);
    let snapshot_path = sync_path.join(SNAPSHOT_FILE);

    let config = read_config(&config_path)?;

    // Read current snapshot as base (before pull)
    let base_snapshot = read_snapshot(&snapshot_path, &config.sync.encryption_policy, false)?;

    if !dry_run {
        // Backup current snapshot before any changes
        let backup = backup_snapshot(sync_path)?;

        // Git pull
        let git = super::git::GitCommandRunner::new(sync_path.to_path_buf());
        let pull_result = git.pull()?;

        match pull_result {
            PullResult::UpToDate => {
                return Ok(PullSummary {
                    keys_added: 0,
                    keys_modified: 0,
                    keys_removed: 0,
                    conflicts: vec![],
                    backup_path: Some(backup),
                });
            }
            PullResult::Conflict { files } => {
                return Err(SyncError::PullConflict { files });
            }
            PullResult::Updated => {
                // Continue with diff computation
            }
        }

        // Optional signed-commit verification: fail closed if the remote
        // tip is not signed by a trusted key. Configured via
        // `[sync] verify_signatures = true` in sync-config.toml.
        if config.sync.verify_signatures {
            git.verify_commit("HEAD")
                .map_err(|e| SyncError::GitCommandFailed {
                    command: "verify-commit HEAD".to_string(),
                    stderr: format!(
                        "signature verification failed for pulled commit ({}). \
                         Refusing to apply changes. Either sign upstream commits or \
                         disable [sync] verify_signatures.",
                        e
                    ),
                })?;
        }
    }

    // Read updated snapshot (after pull)
    let remote_snapshot = read_snapshot(&snapshot_path, &config.sync.encryption_policy, false)?;

    // Compute changes
    let (diff, conflicts) = compute_pull_changes(&base_snapshot, &remote_snapshot, local_entries);

    // Auto-resolve if strategy is not Ask
    let final_conflicts = if conflicts.is_empty() {
        vec![]
    } else {
        match config.sync.conflict_strategy {
            ConflictStrategy::Ask => conflicts,
            ref strategy => {
                auto_resolve(&conflicts, strategy);
                vec![] // All resolved
            }
        }
    };

    Ok(PullSummary {
        keys_added: diff.added.len(),
        keys_modified: diff.modified.len(),
        keys_removed: diff.removed.len(),
        conflicts: final_conflicts,
        backup_path: if dry_run { None } else { Some(PathBuf::new()) },
    })
}

/// Backup current snapshot before pull.
fn backup_snapshot(sync_path: &Path) -> Result<PathBuf, SyncError> {
    let snapshot_path = sync_path.join(SNAPSHOT_FILE);
    if !snapshot_path.exists() {
        return Ok(PathBuf::new());
    }

    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let backup_name = format!(".backup-{}.toml", timestamp);
    let backup_path = sync_path.join(backup_name);

    std::fs::copy(&snapshot_path, &backup_path).map_err(|e| SyncError::IoError {
        path: snapshot_path,
        source: e,
    })?;

    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, value: &str) -> SyncEntry {
        SyncEntry {
            key: key.to_string(),
            value: value.to_string(),
            profile: None,
            group: None,
        }
    }

    #[test]
    fn test_compute_pull_changes_no_conflicts() {
        let base = SyncSnapshot {
            metadata: SnapshotMeta {
                version: 1,
                created_at: "t".to_string(),
                created_by: "m".to_string(),
            },
            entries: vec![entry("K", "base")],
        };
        let remote = SyncSnapshot {
            metadata: base.metadata.clone(),
            entries: vec![entry("K", "remote_new")],
        };
        let local = vec![("K".to_string(), "base".to_string())]; // unchanged

        let (diff, conflicts) = compute_pull_changes(&base, &remote, &local);

        assert!(conflicts.is_empty()); // local didn't change, so no conflict
        assert_eq!(diff.modified.len(), 1);
    }

    #[test]
    fn test_compute_pull_changes_with_conflict() {
        let base = SyncSnapshot {
            metadata: SnapshotMeta {
                version: 1,
                created_at: "t".to_string(),
                created_by: "m".to_string(),
            },
            entries: vec![entry("K", "base")],
        };
        let remote = SyncSnapshot {
            metadata: base.metadata.clone(),
            entries: vec![entry("K", "remote_change")],
        };
        let local = vec![("K".to_string(), "local_change".to_string())]; // also changed!

        let (diff, conflicts) = compute_pull_changes(&base, &remote, &local);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(diff.modified.len(), 1);
    }

    #[test]
    fn test_compute_pull_changes_new_remote_key() {
        let base = SyncSnapshot {
            metadata: SnapshotMeta {
                version: 1,
                created_at: "t".to_string(),
                created_by: "m".to_string(),
            },
            entries: vec![],
        };
        let remote = SyncSnapshot {
            metadata: base.metadata.clone(),
            entries: vec![entry("NEW_KEY", "new_val")],
        };
        let local: Vec<(String, String)> = vec![];

        let (diff, conflicts) = compute_pull_changes(&base, &remote, &local);

        assert!(conflicts.is_empty());
        // NEW_KEY is in remote but not local → from diff perspective it's "removed" from local
        // Actually it's "added" from remote's perspective
        assert_eq!(diff.removed.len(), 1); // remote has it, local doesn't
    }

    #[test]
    fn test_backup_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        let backup = backup_snapshot(&sync_path).unwrap();
        assert!(backup.exists());
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".backup-"));
    }
}
