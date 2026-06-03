use std::collections::HashMap;

use super::model::*;

/// Detect conflicts between local state and remote snapshot.
///
/// A conflict occurs when:
/// - A key was modified both locally AND remotely (compared to a common base)
/// - A key was deleted on one side but modified on the other
///
/// `base_entries`: the last known common snapshot (before divergence).
/// `local_entries`: current local ENV values.
/// `remote_entries`: entries from the pulled remote snapshot.
pub fn detect_conflicts(
    base_entries: &[SyncEntry],
    local_entries: &[(String, String)],
    remote_entries: &[SyncEntry],
) -> Vec<ConflictEntry> {
    let base_map: HashMap<&str, &str> = base_entries
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect();

    let local_map: HashMap<&str, &str> = local_entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let remote_map: HashMap<&str, &str> = remote_entries
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect();

    let mut conflicts = Vec::new();

    // Collect all unique keys across all three sets
    let mut all_keys: Vec<&str> = Vec::new();
    for k in base_map.keys() {
        if !all_keys.contains(k) {
            all_keys.push(k);
        }
    }
    for k in local_map.keys() {
        if !all_keys.contains(k) {
            all_keys.push(k);
        }
    }
    for k in remote_map.keys() {
        if !all_keys.contains(k) {
            all_keys.push(k);
        }
    }

    for key in &all_keys {
        let base_val = base_map.get(key).copied();
        let local_val = local_map.get(key).copied();
        let remote_val = remote_map.get(key).copied();

        let local_changed = local_val != base_val;
        let remote_changed = remote_val != base_val;

        // Only a conflict if both sides changed AND they differ from each other
        if local_changed && remote_changed && local_val != remote_val {
            conflicts.push(ConflictEntry {
                key: (*key).to_string(),
                local_value: local_val.map(String::from),
                remote_value: remote_val.map(String::from),
            });
        }
    }

    conflicts.sort_by(|a, b| a.key.cmp(&b.key));
    conflicts
}

/// Resolve a single conflict.
pub fn resolve_one(conflict: &ConflictEntry, resolution: Resolution) -> ResolvedEntry {
    let resolved_value = match &resolution {
        Resolution::KeepLocal => conflict.local_value.clone(),
        Resolution::KeepRemote => conflict.remote_value.clone(),
        Resolution::ManualEdit(val) => Some(val.clone()),
        Resolution::Delete => None,
    };
    ResolvedEntry {
        key: conflict.key.clone(),
        resolved_value,
        resolution,
    }
}

/// Auto-resolve all conflicts with a given strategy.
pub fn auto_resolve(
    conflicts: &[ConflictEntry],
    strategy: &ConflictStrategy,
) -> Vec<ResolvedEntry> {
    match strategy {
        ConflictStrategy::KeepLocal => conflicts
            .iter()
            .map(|c| resolve_one(c, Resolution::KeepLocal))
            .collect(),
        ConflictStrategy::KeepRemote => conflicts
            .iter()
            .map(|c| resolve_one(c, Resolution::KeepRemote))
            .collect(),
        ConflictStrategy::Ask => vec![], // Cannot auto-resolve with Ask
    }
}

/// Check if a conflict is trivial (both sides have the same value).
pub fn is_trivial_conflict(conflict: &ConflictEntry) -> bool {
    conflict.local_value == conflict.remote_value
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
    fn test_no_conflicts_when_only_one_side_changed() {
        let base = vec![entry("K", "base")];
        let local = vec![("K".to_string(), "changed".to_string())];
        let remote = vec![entry("K", "base")]; // unchanged

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_when_both_sides_changed_differently() {
        let base = vec![entry("K", "base")];
        let local = vec![("K".to_string(), "local_change".to_string())];
        let remote = vec![entry("K", "remote_change")];

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "K");
        assert_eq!(conflicts[0].local_value, Some("local_change".to_string()));
        assert_eq!(conflicts[0].remote_value, Some("remote_change".to_string()));
    }

    #[test]
    fn test_no_conflict_when_both_sides_changed_same() {
        let base = vec![entry("K", "base")];
        let local = vec![("K".to_string(), "same_change".to_string())];
        let remote = vec![entry("K", "same_change")];

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_local_deleted_remote_changed() {
        let base = vec![entry("K", "base")];
        let local: Vec<(String, String)> = vec![]; // deleted locally
        let remote = vec![entry("K", "changed")]; // changed remotely

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].local_value, None);
        assert_eq!(conflicts[0].remote_value, Some("changed".to_string()));
    }

    #[test]
    fn test_conflict_local_changed_remote_deleted() {
        let base = vec![entry("K", "base")];
        let local = vec![("K".to_string(), "changed".to_string())];
        let remote: Vec<SyncEntry> = vec![]; // deleted remotely

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].local_value, Some("changed".to_string()));
        assert_eq!(conflicts[0].remote_value, None);
    }

    #[test]
    fn test_no_conflict_new_key_only_on_one_side() {
        let base: Vec<SyncEntry> = vec![];
        let local = vec![("NEW_LOCAL".to_string(), "val".to_string())];
        let remote = vec![entry("NEW_REMOTE", "val")];

        let conflicts = detect_conflicts(&base, &local, &remote);
        // New keys on different sides are not conflicts
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_new_key_added_on_both_sides_differently() {
        let base: Vec<SyncEntry> = vec![];
        let local = vec![("KEY".to_string(), "local_val".to_string())];
        let remote = vec![entry("KEY", "remote_val")];

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_multiple_conflicts() {
        let base = vec![entry("A", "base_a"), entry("B", "base_b")];
        let local = vec![
            ("A".to_string(), "local_a".to_string()),
            ("B".to_string(), "local_b".to_string()),
        ];
        let remote = vec![entry("A", "remote_a"), entry("B", "remote_b")];

        let conflicts = detect_conflicts(&base, &local, &remote);
        assert_eq!(conflicts.len(), 2);
    }

    #[test]
    fn test_resolve_keep_local() {
        let conflict = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("local".to_string()),
            remote_value: Some("remote".to_string()),
        };
        let resolved = resolve_one(&conflict, Resolution::KeepLocal);
        assert_eq!(resolved.resolved_value, Some("local".to_string()));
    }

    #[test]
    fn test_resolve_keep_remote() {
        let conflict = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("local".to_string()),
            remote_value: Some("remote".to_string()),
        };
        let resolved = resolve_one(&conflict, Resolution::KeepRemote);
        assert_eq!(resolved.resolved_value, Some("remote".to_string()));
    }

    #[test]
    fn test_resolve_manual_edit() {
        let conflict = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("local".to_string()),
            remote_value: Some("remote".to_string()),
        };
        let resolved = resolve_one(&conflict, Resolution::ManualEdit("custom".to_string()));
        assert_eq!(resolved.resolved_value, Some("custom".to_string()));
    }

    #[test]
    fn test_resolve_delete() {
        let conflict = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("local".to_string()),
            remote_value: None,
        };
        let resolved = resolve_one(&conflict, Resolution::Delete);
        assert_eq!(resolved.resolved_value, None);
    }

    #[test]
    fn test_auto_resolve_keep_local() {
        let conflicts = vec![
            ConflictEntry {
                key: "A".to_string(),
                local_value: Some("la".to_string()),
                remote_value: Some("ra".to_string()),
            },
            ConflictEntry {
                key: "B".to_string(),
                local_value: Some("lb".to_string()),
                remote_value: Some("rb".to_string()),
            },
        ];

        let resolved = auto_resolve(&conflicts, &ConflictStrategy::KeepLocal);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].resolved_value, Some("la".to_string()));
        assert_eq!(resolved[1].resolved_value, Some("lb".to_string()));
    }

    #[test]
    fn test_auto_resolve_ask_returns_empty() {
        let conflicts = vec![ConflictEntry {
            key: "A".to_string(),
            local_value: Some("la".to_string()),
            remote_value: Some("ra".to_string()),
        }];

        let resolved = auto_resolve(&conflicts, &ConflictStrategy::Ask);
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_is_trivial_conflict() {
        let trivial = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("same".to_string()),
            remote_value: Some("same".to_string()),
        };
        assert!(is_trivial_conflict(&trivial));

        let real = ConflictEntry {
            key: "K".to_string(),
            local_value: Some("a".to_string()),
            remote_value: Some("b".to_string()),
        };
        assert!(!is_trivial_conflict(&real));
    }
}
