//! Coverage for `ops::snapshot::diff_snapshot`: Added / Removed / Changed /
//! Same classification against the current environment.

use envforge::ops::snapshot::{diff_snapshot, DiffStatus, Snapshot, SnapshotMeta};
use std::collections::BTreeMap;

fn snapshot(entries: &[(&str, &str)]) -> Snapshot {
    Snapshot {
        metadata: SnapshotMeta {
            name: "test".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            profile: "default".to_string(),
            machine_id: "m".to_string(),
            var_count: entries.len(),
        },
        entries: entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn status_of<'a>(
    diff: &'a [envforge::ops::snapshot::SnapshotDiffEntry],
    key: &str,
) -> &'a DiffStatus {
    &diff.iter().find(|d| d.key == key).unwrap().status
}

#[test]
fn test_diff_snapshot_classifies_all_states() {
    // snapshot: A=1 B=2 D=4 ; current: A=1 B=9 C=3
    let snap = snapshot(&[("A", "1"), ("B", "2"), ("D", "4")]);
    let current = vec![
        ("A".to_string(), "1".to_string()),
        ("B".to_string(), "9".to_string()),
        ("C".to_string(), "3".to_string()),
    ];
    let diff = diff_snapshot(&snap, &current);

    assert!(matches!(status_of(&diff, "A"), DiffStatus::Same));
    assert!(matches!(status_of(&diff, "B"), DiffStatus::Changed));
    assert!(matches!(status_of(&diff, "C"), DiffStatus::Added));
    assert!(matches!(status_of(&diff, "D"), DiffStatus::Removed));
}

#[test]
fn test_diff_snapshot_carries_values() {
    let snap = snapshot(&[("B", "2")]);
    let current = vec![("B".to_string(), "9".to_string())];
    let diff = diff_snapshot(&snap, &current);
    let b = diff.iter().find(|d| d.key == "B").unwrap();
    assert_eq!(b.snapshot_value.as_deref(), Some("2"));
    assert_eq!(b.current_value.as_deref(), Some("9"));
}

#[test]
fn test_diff_snapshot_empty_current_all_removed() {
    let snap = snapshot(&[("A", "1"), ("B", "2")]);
    let diff = diff_snapshot(&snap, &[]);
    assert!(diff.iter().all(|d| matches!(d.status, DiffStatus::Removed)));
}
