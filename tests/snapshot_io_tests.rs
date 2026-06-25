//! IO coverage for `ops::snapshot` create/list/load/delete/prune round-trip.
//!
//! Serialized because `snapshots_dir()` is derived from the home directory; the
//! tests point `HOME` at a tempdir to isolate on-disk snapshot state.

use envforge::ops::snapshot::{
    create_snapshot, delete_snapshot, list_snapshots, load_snapshot, prune_snapshots,
};
use serial_test::serial;
use std::path::Path;

fn isolate(dir: &Path) {
    std::env::set_var("HOME", dir);
}

fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
#[serial]
fn test_snapshot_create_list_load_delete() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    let path = create_snapshot("snap1", &pairs(&[("A", "1"), ("B", "2")]), "default").unwrap();
    assert!(path.exists());

    let metas = list_snapshots().unwrap();
    let m = metas
        .iter()
        .find(|m| m.name == "snap1")
        .expect("snap1 listed");
    assert_eq!(m.var_count, 2);

    let loaded = load_snapshot("snap1").unwrap();
    assert_eq!(loaded.entries.get("A").map(String::as_str), Some("1"));
    assert_eq!(loaded.entries.get("B").map(String::as_str), Some("2"));

    delete_snapshot("snap1").unwrap();
    assert!(!list_snapshots().unwrap().iter().any(|m| m.name == "snap1"));

    std::env::remove_var("HOME");
}

#[test]
#[serial]
fn test_snapshot_load_last_alias() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    create_snapshot("only", &pairs(&[("K", "v")]), "default").unwrap();
    let last = load_snapshot("last").unwrap();
    assert_eq!(last.entries.get("K").map(String::as_str), Some("v"));

    std::env::remove_var("HOME");
}

#[test]
#[serial]
fn test_snapshot_prune_keeps_max_count() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    for n in ["a", "b", "c"] {
        create_snapshot(n, &pairs(&[("K", "v")]), "default").unwrap();
    }
    let before = list_snapshots().unwrap().len();
    assert!(before >= 3);

    let pruned = prune_snapshots(1).unwrap();
    assert_eq!(pruned, before - 1);
    assert_eq!(list_snapshots().unwrap().len(), 1);

    std::env::remove_var("HOME");
}
