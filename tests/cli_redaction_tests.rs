//! Integration regression tests for CLI secret-redaction defaults.
//!
//! Covers H6/FR28 (`get` masks sensitive values by default, `--reveal` shows
//! them), L1/FR4 (`set` does not disclose value length), and L2/FR3
//! (`set --dry-run` never echoes a sensitive cleartext value — old or new).
//!
//! Uses the compiled binary (`CARGO_BIN_EXE_envforge`) with `HOME` pointed at a
//! tempdir so the default primary file (`~/.zshrc`) and config are isolated.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

fn run(args: &[&str], home: &Path, stdin: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(envforge_bin());
    cmd.args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().expect("spawn envforge");
    if let Some(s) = stdin {
        child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    }
    child.wait_with_output().expect("wait envforge")
}

fn setup_home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    // Needs enough lines to clear the default header_protected_lines offset.
    std::fs::write(
        dir.path().join(".zshrc"),
        "# line1\n# line2\n# line3\n# line4\nexport EXISTING=1\n",
    )
    .unwrap();
    dir
}

const SECRET: &str = "supersecretvalue123";

#[test]
fn test_get_masks_sensitive_value_by_default() {
    let home = setup_home();
    let out = run(&["set", "API_SECRET", "--stdin"], home.path(), Some(SECRET));
    assert!(
        out.status.success(),
        "set failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run(&["get", "API_SECRET"], home.path(), None);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains(SECRET),
        "default `get` leaked cleartext: {stdout}"
    );
    assert!(
        stdout.contains("***"),
        "expected masked output, got: {stdout}"
    );
}

#[test]
fn test_get_reveal_shows_full_value() {
    let home = setup_home();
    assert!(
        run(&["set", "API_SECRET", "--stdin"], home.path(), Some(SECRET))
            .status
            .success()
    );

    let out = run(&["get", "API_SECRET", "--reveal"], home.path(), None);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(SECRET),
        "`get --reveal` should show full value: {stdout}"
    );
}

#[test]
fn test_set_success_does_not_disclose_length() {
    let home = setup_home();
    let out = run(&["set", "API_SECRET", "--stdin"], home.path(), Some(SECRET));
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("chars"),
        "`set` must not disclose value length: {stdout}"
    );
}

#[test]
fn test_dry_run_redacts_both_old_and_new_sensitive_values() {
    let home = setup_home();
    let old = SECRET;
    let new = "anothersecretvalue999";
    assert!(
        run(&["set", "API_SECRET", "--stdin"], home.path(), Some(old))
            .status
            .success()
    );

    let out = run(
        &["set", "API_SECRET", "--stdin", "--dry-run"],
        home.path(),
        Some(new),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "dry-run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !stdout.contains(old),
        "dry-run leaked OLD cleartext: {stdout}"
    );
    assert!(
        !stdout.contains(new),
        "dry-run leaked NEW cleartext: {stdout}"
    );
    assert!(
        stdout.contains("[REDACTED]"),
        "dry-run should redact sensitive values: {stdout}"
    );
}

#[test]
fn test_get_nonsensitive_value_not_masked() {
    let home = setup_home();
    assert!(
        run(&["set", "APP_PORT", "--stdin"], home.path(), Some("8080"))
            .status
            .success()
    );

    let out = run(&["get", "APP_PORT"], home.path(), None);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("8080"),
        "non-sensitive value should print as-is: {stdout}"
    );
}
