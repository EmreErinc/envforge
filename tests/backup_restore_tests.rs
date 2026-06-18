//! Regression tests for L4 — `backup restore` must be confined to the backups
//! directory and refuse paths outside it (path-traversal / symlink hardening).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

fn run(args: &[&str], config_dir: &Path) -> std::process::Output {
    Command::new(envforge_bin())
        .args(args)
        .env("ENVFORGE_CONFIG_DIR", config_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn envforge")
}

#[test]
fn test_backup_restore_inside_backups_dir_succeeds() {
    let cfg = tempfile::tempdir().unwrap();
    let backups = cfg.path().join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    let backup = backups.join(".zshrc.20260101000000.bak");
    std::fs::write(&backup, "export EXISTING=1\n").unwrap();

    let out = run(&["backup", "restore", backup.to_str().unwrap()], cfg.path());
    assert!(
        out.status.success(),
        "in-dir restore should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn test_backup_restore_outside_backups_dir_is_rejected() {
    let cfg = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cfg.path().join("backups")).unwrap();

    // A real file that exists but lives OUTSIDE the backups directory.
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), "secret outside content\n").unwrap();

    let out = run(
        &["backup", "restore", outside.path().to_str().unwrap()],
        cfg.path(),
    );
    assert!(
        !out.status.success(),
        "restore from outside the backups dir must be rejected"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("outside") || stderr.contains("refusing"),
        "error should explain the confinement, got: {stderr}"
    );
}
