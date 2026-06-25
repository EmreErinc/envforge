use std::collections::HashMap;

use super::model::*;

/// Compute diff between local entries and snapshot entries.
///
/// `local_entries`: current ENV key-value pairs (only sync-marked ones).
/// `snapshot_entries`: entries from the last sync snapshot.
///
/// Returns: what changed locally compared to the snapshot.
pub fn compute_diff(
    local_entries: &[(String, String)],
    snapshot_entries: &[SyncEntry],
) -> SyncDiff {
    let local_map: HashMap<&str, &str> = local_entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let snapshot_map: HashMap<&str, &str> = snapshot_entries
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect();

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();

    for (key, value) in &local_map {
        if !snapshot_map.contains_key(key) {
            added.push(DiffEntry {
                key: (*key).to_string(),
                local_value: Some((*value).to_string()),
                remote_value: None,
            });
        }
    }

    for (key, local_val) in &local_map {
        if let Some(snapshot_val) = snapshot_map.get(key) {
            if local_val != snapshot_val {
                modified.push(DiffEntry {
                    key: (*key).to_string(),
                    local_value: Some((*local_val).to_string()),
                    remote_value: Some((*snapshot_val).to_string()),
                });
            }
        }
    }

    for (key, value) in &snapshot_map {
        if !local_map.contains_key(key) {
            removed.push(DiffEntry {
                key: (*key).to_string(),
                local_value: None,
                remote_value: Some((*value).to_string()),
            });
        }
    }

    // Sort for deterministic output
    added.sort_by(|a, b| a.key.cmp(&b.key));
    modified.sort_by(|a, b| a.key.cmp(&b.key));
    removed.sort_by(|a, b| a.key.cmp(&b.key));

    SyncDiff {
        added,
        modified,
        removed,
    }
}

/// Compute sync status by comparing local state vs snapshot.
pub fn compute_status(
    local_entries: &[(String, String)],
    snapshot_entries: &[SyncEntry],
) -> SyncStatus {
    let diff = compute_diff(local_entries, snapshot_entries);
    if diff.is_empty() {
        SyncStatus::InSync
    } else {
        SyncStatus::LocalAhead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_empty_both() {
        let diff = compute_diff(&[], &[]);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_diff_all_added() {
        let local = vec![
            ("KEY_A".to_string(), "val_a".to_string()),
            ("KEY_B".to_string(), "val_b".to_string()),
        ];
        let diff = compute_diff(&local, &[]);
        assert_eq!(diff.added.len(), 2);
        assert!(diff.modified.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.added[0].key, "KEY_A");
        assert_eq!(diff.added[1].key, "KEY_B");
    }

    #[test]
    fn test_diff_all_removed() {
        let snapshot = vec![
            SyncEntry {
                key: "KEY_A".to_string(),
                value: "val_a".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "KEY_B".to_string(),
                value: "val_b".to_string(),
                profile: None,
                group: None,
            },
        ];
        let diff = compute_diff(&[], &snapshot);
        assert!(diff.added.is_empty());
        assert!(diff.modified.is_empty());
        assert_eq!(diff.removed.len(), 2);
    }

    #[test]
    fn test_diff_modified() {
        let local = vec![("KEY_A".to_string(), "new_val".to_string())];
        let snapshot = vec![SyncEntry {
            key: "KEY_A".to_string(),
            value: "old_val".to_string(),
            profile: None,
            group: None,
        }];
        let diff = compute_diff(&local, &snapshot);
        assert!(diff.added.is_empty());
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].local_value, Some("new_val".to_string()));
        assert_eq!(diff.modified[0].remote_value, Some("old_val".to_string()));
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_mixed() {
        let local = vec![
            ("KEPT".to_string(), "same".to_string()),
            ("CHANGED".to_string(), "new".to_string()),
            ("NEW".to_string(), "added".to_string()),
        ];
        let snapshot = vec![
            SyncEntry {
                key: "KEPT".to_string(),
                value: "same".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "CHANGED".to_string(),
                value: "old".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "GONE".to_string(),
                value: "removed".to_string(),
                profile: None,
                group: None,
            },
        ];
        let diff = compute_diff(&local, &snapshot);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].key, "NEW");
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].key, "CHANGED");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].key, "GONE");
        assert_eq!(diff.total_changes(), 3);
    }

    #[test]
    fn test_diff_identical() {
        let local = vec![("KEY".to_string(), "val".to_string())];
        let snapshot = vec![SyncEntry {
            key: "KEY".to_string(),
            value: "val".to_string(),
            profile: None,
            group: None,
        }];
        let diff = compute_diff(&local, &snapshot);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_status_in_sync() {
        let local = vec![("K".to_string(), "V".to_string())];
        let snap = vec![SyncEntry {
            key: "K".to_string(),
            value: "V".to_string(),
            profile: None,
            group: None,
        }];
        assert_eq!(compute_status(&local, &snap), SyncStatus::InSync);
    }

    #[test]
    fn test_status_local_ahead() {
        let local = vec![("K".to_string(), "NEW".to_string())];
        let snap = vec![SyncEntry {
            key: "K".to_string(),
            value: "OLD".to_string(),
            profile: None,
            group: None,
        }];
        assert_eq!(compute_status(&local, &snap), SyncStatus::LocalAhead);
    }
}
