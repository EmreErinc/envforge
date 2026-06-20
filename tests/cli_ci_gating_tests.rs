//! CLI exit-code tests for CI gating (Story 5.1 / FR9, FR24).
//!
//! Runs the compiled binary (`CARGO_BIN_EXE_envforge`) with an isolated HOME so
//! global config never interferes. Verifies the deterministic exit codes a CI
//! job relies on: `fence --status` and `mcp status` exit non-zero (2) when
//! coverage is incomplete / a hardcoded credential is present, and 0 otherwise.

use std::path::PathBuf;
use std::process::Command;

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

/// Build a command with an isolated, clean environment rooted at `home`/`cwd`.
fn cmd(home: &std::path::Path, cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(envforge_bin())
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn envforge")
}

#[test]
fn test_fence_status_unfenced_exits_2() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    // Nothing fenced → .envforgeignore (always-applicable) is unfenced → not
    // protected → exit 2 (CI gate fails).
    let out = cmd(home.path(), cwd.path(), &["fence", "--status"]);
    assert_eq!(out.status.code(), Some(2), "unfenced dir must exit 2");
}

#[test]
fn test_fence_then_status_exits_0() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    // Fence the project (writes targets into cwd), then status must be clean.
    let fence = cmd(home.path(), cwd.path(), &["fence"]);
    assert_eq!(fence.status.code(), Some(0), "fence should succeed");
    let out = cmd(home.path(), cwd.path(), &["fence", "--status"]);
    assert_eq!(out.status.code(), Some(0), "fully fenced dir must exit 0");
}

#[test]
fn test_mcp_status_clean_exits_0() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = cmd(home.path(), cwd.path(), &["mcp", "status"]);
    assert_eq!(out.status.code(), Some(0), "no MCP creds → exit 0");
}

#[test]
fn test_mcp_status_hardcoded_credential_exits_2() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    // Plant an MCP config with a hardcoded AWS access key id.
    std::fs::write(
        cwd.path().join(".mcp.json"),
        r#"{"mcpServers":{"db":{"command":"x","env":{"AWS_ACCESS_KEY_ID":"AKIAIOSFODNN7EXAMPLE"}}}}"#,
    )
    .unwrap();
    let out = cmd(home.path(), cwd.path(), &["mcp", "status"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "hardcoded credential in MCP config must exit 2"
    );
}
