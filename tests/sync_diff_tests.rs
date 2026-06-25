//! Coverage for `ops::sync::diff`: local-vs-snapshot diff classification
//! (added / modified / removed) and derived sync status.

use envforge::ops::sync::diff::{compute_diff, compute_status};
use envforge::ops::sync::{SyncEntry, SyncStatus};

fn entry(key: &str, value: &str) -> SyncEntry {
    SyncEntry {
        key: key.to_string(),
        value: value.to_string(),
        profile: None,
        group: None,
    }
}

fn local(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn test_compute_diff_modified_only() {
    let diff = compute_diff(&local(&[("A", "2")]), &[entry("A", "1")]);
    assert_eq!(diff.modified.len(), 1);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert_eq!(diff.modified[0].local_value.as_deref(), Some("2"));
    assert_eq!(diff.modified[0].remote_value.as_deref(), Some("1"));
}

#[test]
fn test_compute_diff_mixed_added_modified_removed() {
    let snapshot = vec![entry("A", "1"), entry("B", "2"), entry("D", "4")];
    let diff = compute_diff(&local(&[("A", "1"), ("B", "9"), ("C", "3")]), &snapshot);
    assert_eq!(
        diff.added
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        vec!["C"]
    );
    assert_eq!(
        diff.modified
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        vec!["B"]
    );
    assert_eq!(
        diff.removed
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        vec!["D"]
    );
}

#[test]
fn test_compute_status_in_sync_vs_ahead() {
    let snapshot = vec![entry("A", "1")];
    assert!(matches!(
        compute_status(&local(&[("A", "1")]), &snapshot),
        SyncStatus::InSync
    ));
    assert!(matches!(
        compute_status(&local(&[("A", "2")]), &snapshot),
        SyncStatus::LocalAhead
    ));
}
