//! Coverage for `ops::sync` pure parsers/validators: git status/log parsing,
//! remote-URL validation (incl. SSH enforcement + injection guards),
//! `export_to_snapshot`, and three-way `compute_pull_changes`.

use envforge::ops::sync::pull::compute_pull_changes;
use envforge::ops::sync::push::export_to_snapshot;
use envforge::ops::sync::{
    parse_git_log, parse_git_status, validate_remote_url, validate_remote_url_enforce_ssh,
    FileStatusKind, SyncSnapshot,
};

fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn snap(entries: &[(&str, &str)], machine: &str) -> SyncSnapshot {
    export_to_snapshot(&pairs(entries), machine)
}

// ---- parse_git_status ------------------------------------------------------

#[test]
fn test_parse_git_status_kinds() {
    let out = " M modified.txt\n?? untracked.txt\nA  added.txt\nD  deleted.txt\n";
    let parsed = parse_git_status(out);
    assert_eq!(parsed.len(), 4);
    assert!(matches!(parsed[0].status, FileStatusKind::Modified));
    assert_eq!(parsed[0].path, "modified.txt");
    assert!(matches!(parsed[1].status, FileStatusKind::Untracked));
    assert!(matches!(parsed[2].status, FileStatusKind::Added));
    assert!(matches!(parsed[3].status, FileStatusKind::Deleted));
}

// ---- parse_git_log ---------------------------------------------------------

#[test]
fn test_parse_git_log_well_formed_and_skips_malformed() {
    let out = "h1|s1|2026-01-01|first commit|alice\nGARBAGE LINE\nh2|s2|2026-01-02|second|bob\n";
    let log = parse_git_log(out);
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].hash, "h1");
    assert_eq!(log[0].short_hash, "s1");
    assert_eq!(log[0].message, "first commit");
    assert_eq!(log[0].author, "alice");
    assert_eq!(log[1].author, "bob");
}

// ---- validate_remote_url ---------------------------------------------------

#[test]
fn test_validate_remote_url_accepts_safe_forms() {
    assert!(validate_remote_url("https://github.com/u/repo.git").is_ok());
    assert!(validate_remote_url("ssh://git@host/repo.git").is_ok());
    assert!(validate_remote_url("git@github.com:u/repo.git").is_ok()); // scp-like
}

#[test]
fn test_validate_remote_url_rejects_dangerous() {
    assert!(validate_remote_url("").is_err());
    assert!(validate_remote_url("-oProxyCommand=evil").is_err()); // leading dash
    assert!(validate_remote_url("file:///etc/passwd").is_err()); // disallowed scheme
    assert!(validate_remote_url("ext::sh -c touch/x").is_err()); // git ext exploit
    assert!(validate_remote_url("https://h/r\nrm -rf").is_err()); // control char
}

#[test]
fn test_validate_remote_url_enforce_ssh_rejects_http() {
    assert!(validate_remote_url_enforce_ssh("https://github.com/u/r.git", true).is_err());
    assert!(validate_remote_url_enforce_ssh("ssh://git@host/r.git", true).is_ok());
    assert!(validate_remote_url_enforce_ssh("git@host:u/r.git", true).is_ok());
}

// ---- export_to_snapshot ----------------------------------------------------

#[test]
fn test_export_to_snapshot_carries_machine_and_entries() {
    let s = export_to_snapshot(&pairs(&[("A", "1"), ("B", "2")]), "machine-x");
    assert_eq!(s.metadata.created_by, "machine-x");
    assert_eq!(s.metadata.version, 1);
    assert!(!s.metadata.created_at.is_empty());
    assert_eq!(s.entries.len(), 2);
    assert!(s.entries.iter().any(|e| e.key == "A" && e.value == "1"));
}

// ---- compute_pull_changes (three-way) --------------------------------------

#[test]
fn test_compute_pull_changes_fast_forward_no_conflict() {
    // local unchanged from base, remote moved ahead → no conflict.
    let base = snap(&[("A", "1")], "m");
    let remote = snap(&[("A", "2")], "m");
    let (diff, conflicts) = compute_pull_changes(&base, &remote, &pairs(&[("A", "1")]));
    assert!(conflicts.is_empty());
    assert_eq!(diff.modified.len(), 1);
}

#[test]
fn test_compute_pull_changes_divergent_edit_is_conflict() {
    // base A=1; local edited to 2; remote edited to 3 → divergent conflict.
    let base = snap(&[("A", "1")], "m");
    let remote = snap(&[("A", "3")], "m");
    let (_diff, conflicts) = compute_pull_changes(&base, &remote, &pairs(&[("A", "2")]));
    let c = conflicts
        .iter()
        .find(|c| c.key == "A")
        .expect("A conflicts");
    assert_eq!(c.local_value.as_deref(), Some("2"));
    assert_eq!(c.remote_value.as_deref(), Some("3"));
}
