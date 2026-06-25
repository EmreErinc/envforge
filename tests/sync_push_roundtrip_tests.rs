//! Fixture-heavy IO coverage for `ops::sync` push: a real git repository plus
//! age-encrypted snapshots, exercised end-to-end without a remote.
//!
//! Serialized + env-isolated (`HOME`, `ENVFORGE_CONFIG_DIR`, age key auto-gen)
//! because push writes an age-encrypted snapshot, runs `git commit`, and the age
//! identity lives in the config dir.

use envforge::ops::sync::push::push;
use envforge::ops::sync::{
    init_fresh, read_config, read_snapshot, write_config, PushResult, SyncError, CONFIG_FILE,
    SNAPSHOT_FILE,
};
use serial_test::serial;
use std::path::Path;

fn isolate(dir: &Path) {
    std::env::set_var("HOME", dir);
    std::env::set_var("ENVFORGE_CONFIG_DIR", dir.join("cfg"));
    std::env::remove_var("ENVFORGE_AGE_KEY");
    std::env::remove_var("ENVFORGE_AGE_KEY_FILE");
    std::fs::create_dir_all(dir.join("cfg")).unwrap();
}

fn cleanup() {
    std::env::remove_var("HOME");
    std::env::remove_var("ENVFORGE_CONFIG_DIR");
}

fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// init_fresh a repo, then flip default_sync on so all keys count as synced.
fn init_repo(sync_path: &Path) {
    init_fresh(sync_path, "test-machine-aaaa").unwrap();
    let cfg_path = sync_path.join(CONFIG_FILE);
    let mut cfg = read_config(&cfg_path).unwrap();
    cfg.sync.default_sync = true;
    write_config(&cfg_path, &cfg).unwrap();
}

#[test]
#[serial]
fn test_push_commits_encrypted_snapshot_no_remote() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    let sync_path = dir.path().join("syncrepo");
    init_repo(&sync_path);

    let summary = push(
        &sync_path,
        &pairs(&[("API_KEY", "sk-123"), ("DB_URL", "postgres://x")]),
        Some("test push"),
        false,
    )
    .unwrap();
    assert_eq!(summary.keys_pushed, 2);
    assert!(matches!(summary.push_result, PushResult::NoRemote));
    assert!(summary.commit_hash.is_some());

    // Snapshot on disk is encrypted (no plaintext secret) yet decrypts back.
    let snap_path = sync_path.join(SNAPSHOT_FILE);
    let raw = std::fs::read_to_string(&snap_path).unwrap();
    assert!(!raw.contains("sk-123"), "snapshot leaked plaintext secret");

    let cfg = read_config(&sync_path.join(CONFIG_FILE)).unwrap();
    let snap = read_snapshot(&snap_path, &cfg.sync.encryption_policy, false).unwrap();
    assert_eq!(snap.entries.len(), 2);
    assert!(snap
        .entries
        .iter()
        .any(|e| e.key == "API_KEY" && e.value == "sk-123"));

    cleanup();
}

#[test]
#[serial]
fn test_push_dry_run_does_not_commit() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    let sync_path = dir.path().join("syncrepo");
    init_repo(&sync_path);

    let summary = push(&sync_path, &pairs(&[("K", "v")]), None, true).unwrap();
    assert!(matches!(summary.push_result, PushResult::NoRemote));
    assert!(summary.commit_hash.is_none(), "dry-run must not commit");
    assert!(summary.message.contains("dry-run"));

    cleanup();
}

#[test]
#[serial]
fn test_push_no_marked_keys_errors() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    let sync_path = dir.path().join("syncrepo");
    // Fresh init leaves default_sync=false → nothing is marked for sync.
    init_fresh(&sync_path, "test-machine-bbbb").unwrap();

    let err = push(&sync_path, &pairs(&[("K", "v")]), None, false).unwrap_err();
    assert!(matches!(err, SyncError::NoKeysMarked));

    cleanup();
}

#[test]
#[serial]
fn test_push_twice_second_is_nothing_to_sync() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    let sync_path = dir.path().join("syncrepo");
    init_repo(&sync_path);

    let entries = pairs(&[("K", "v")]);
    push(&sync_path, &entries, None, false).unwrap();
    let err = push(&sync_path, &entries, None, false).unwrap_err();
    assert!(matches!(err, SyncError::NothingToSync));

    cleanup();
}
