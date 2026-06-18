//! Regression tests for M7/FR28 — `export --format` redacts sensitive values
//! by default; `--reveal` emits cleartext.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

fn run(args: &[&str], home: &Path) -> std::process::Output {
    Command::new(envforge_bin())
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn envforge")
}

fn setup_home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".zshrc"),
        "export API_SECRET=\"topsecret-value-xyz\"\nexport APP_PORT=\"8080\"\n",
    )
    .unwrap();
    dir
}

#[test]
fn test_export_format_redacts_sensitive_by_default() {
    let home = setup_home();
    let out = run(&["export", "--format", "dotenv"], home.path());
    assert!(
        out.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("topsecret-value-xyz"),
        "default export leaked a secret: {stdout}"
    );
    assert!(
        stdout.contains("[REDACTED]"),
        "expected redaction: {stdout}"
    );
    // Non-sensitive value is preserved.
    assert!(
        stdout.contains("8080"),
        "non-sensitive value must remain: {stdout}"
    );
}

#[test]
fn test_export_format_reveal_emits_cleartext() {
    let home = setup_home();
    let out = run(&["export", "--format", "dotenv", "--reveal"], home.path());
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("topsecret-value-xyz"),
        "--reveal must emit cleartext: {stdout}"
    );
}
