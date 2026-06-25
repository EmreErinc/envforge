//! Fixture-heavy IO coverage for `ops::sync` against a real bare git remote:
//! push reaches the remote (verified by an independent clone), and pull picks up
//! another machine's change. Real git + age-encrypted snapshots.
//!
//! Serialized + env-isolated (`HOME`, `ENVFORGE_CONFIG_DIR`, age key auto-gen) —
//! both repos share one age identity so the encrypted snapshot round-trips.

use envforge::ops::sync::pull::pull;
use envforge::ops::sync::push::push;
use envforge::ops::sync::{
    init_fresh, read_config, read_snapshot, write_config, PushResult, CONFIG_FILE, SNAPSHOT_FILE,
};
use serial_test::serial;
use std::path::Path;
use std::process::Command;

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

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn enable_sync(sync_path: &Path) {
    let cfg_path = sync_path.join(CONFIG_FILE);
    let mut cfg = read_config(&cfg_path).unwrap();
    cfg.sync.default_sync = true;
    write_config(&cfg_path, &cfg).unwrap();
}

#[test]
#[serial]
fn test_push_reaches_bare_remote() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    // Bare remote + working repo wired to it.
    git(dir.path(), &["init", "--bare", "-b", "main", "remote.git"]);
    let bare = dir.path().join("remote.git");
    let repo_a = dir.path().join("A");
    init_fresh(&repo_a, "machine-a").unwrap();
    git(
        &repo_a,
        &["remote", "add", "origin", bare.to_str().unwrap()],
    );
    enable_sync(&repo_a);

    let summary = push(
        &repo_a,
        &pairs(&[("API_KEY", "sk-remote")]),
        Some("v1"),
        false,
    )
    .unwrap();
    assert!(matches!(summary.push_result, PushResult::Success));

    // Independent clone proves the encrypted snapshot landed on the remote.
    git(dir.path(), &["clone", bare.to_str().unwrap(), "C"]);
    let repo_c = dir.path().join("C");
    let snap_path = repo_c.join(SNAPSHOT_FILE);
    assert!(snap_path.exists(), "remote is missing the snapshot");
    let raw = std::fs::read_to_string(&snap_path).unwrap();
    assert!(
        !raw.contains("sk-remote"),
        "remote snapshot leaked plaintext"
    );

    let cfg = read_config(&repo_c.join(CONFIG_FILE)).unwrap();
    let snap = read_snapshot(&snap_path, &cfg.sync.encryption_policy, false).unwrap();
    assert!(snap
        .entries
        .iter()
        .any(|e| e.key == "API_KEY" && e.value == "sk-remote"));

    cleanup();
}

#[test]
#[serial]
fn test_pull_picks_up_remote_change() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    git(dir.path(), &["init", "--bare", "-b", "main", "remote.git"]);
    let bare = dir.path().join("remote.git");

    // Machine A publishes v1 (A=1).
    let repo_a = dir.path().join("A");
    init_fresh(&repo_a, "machine-a").unwrap();
    git(
        &repo_a,
        &["remote", "add", "origin", bare.to_str().unwrap()],
    );
    enable_sync(&repo_a);
    push(&repo_a, &pairs(&[("A", "1")]), Some("v1"), false).unwrap();

    // Machine B clones the published state.
    git(dir.path(), &["clone", bare.to_str().unwrap(), "B"]);
    let repo_b = dir.path().join("B");
    git(&repo_b, &["config", "user.email", "b@test"]);
    git(&repo_b, &["config", "user.name", "machine-b"]);

    // Machine A publishes v2 (A=2).
    push(&repo_a, &pairs(&[("A", "2")]), Some("v2"), false).unwrap();

    // Machine B pulls; B still holds local A=1 → remote's A=2 is a modification.
    let summary = pull(&repo_b, &pairs(&[("A", "1")]), false).unwrap();
    assert_eq!(summary.keys_modified, 1);
    assert!(
        summary.conflicts.is_empty(),
        "fast-forward should not conflict"
    );

    cleanup();
}
