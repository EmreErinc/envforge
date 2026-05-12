//! End-to-end CLI tests for `envforge mcp pin / verify / diff / trust / untrust / explain`.
//!
//! Tests invoke the `envforge` binary as a subprocess via
//! `CARGO_BIN_EXE_envforge`. Override `HOME` (and `XDG_CONFIG_HOME` on
//! Linux) to a tempdir so the user-trust override store and any other
//! per-user state stay isolated from the developer's real machine.

use std::path::{Path, PathBuf};
use std::process::Command;

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

fn run_in_tempdir(args: &[&str], home: &Path, extra_env: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(envforge_bin());
    cmd.args(args);
    cmd.env_clear();
    cmd.env("HOME", home);
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("PATH", std::env::var_os("PATH").unwrap_or_default());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn envforge")
}

fn overrides_path(home: &Path) -> PathBuf {
    // Mirror `dirs::config_dir()` per-platform semantics. The subprocess
    // sees `HOME` overridden to `home`, so dirs::config_dir() inside the
    // subprocess will produce one of these.
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/envforge/mcp-trust.json")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        home.join(".config/envforge/mcp-trust.json")
    }
    #[cfg(windows)]
    {
        home.join("AppData/Roaming/envforge/mcp-trust.json")
    }
}

fn read_overrides(home: &Path) -> Option<String> {
    std::fs::read_to_string(overrides_path(home)).ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Smoke: `mcp --help` lists all six new subcommands
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_help_lists_pin_verify_diff_trust_untrust_explain() {
    let out = Command::new(envforge_bin())
        .args(["mcp", "--help"])
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let blob = format!("{stdout}{stderr}");
    for cmd in ["pin", "verify", "diff", "trust", "untrust", "explain"] {
        assert!(
            blob.contains(cmd),
            "expected `mcp --help` output to mention `{cmd}`, got: {blob}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 004: mcp trust / mcp untrust
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_trust_records_override() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_in_tempdir(
        &["mcp", "trust", "community-server", "--reason", "audited"],
        dir.path(),
        &[],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let contents = read_overrides(dir.path()).expect("override file written");
    assert!(contents.contains("community-server"));
    assert!(contents.contains("audited"));
}

#[test]
fn test_mcp_trust_empty_reason_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_in_tempdir(&["mcp", "trust", "x", "--reason", ""], dir.path(), &[]);
    assert!(!out.status.success());
}

#[test]
fn test_mcp_trust_missing_reason_flag_errors() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_in_tempdir(&["mcp", "trust", "x"], dir.path(), &[]);
    assert!(!out.status.success());
}

#[test]
fn test_mcp_untrust_removes_existing_override() {
    let dir = tempfile::tempdir().unwrap();
    run_in_tempdir(
        &["mcp", "trust", "tobegone", "--reason", "test"],
        dir.path(),
        &[],
    );
    assert!(read_overrides(dir.path()).unwrap().contains("tobegone"));
    let out = run_in_tempdir(&["mcp", "untrust", "tobegone"], dir.path(), &[]);
    assert!(out.status.success());
    let after = read_overrides(dir.path()).unwrap();
    assert!(!after.contains("tobegone"));
}

#[test]
fn test_mcp_untrust_absent_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_in_tempdir(&["mcp", "untrust", "never-existed"], dir.path(), &[]);
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no override") || stderr.contains("no-op"));
}

#[test]
fn test_mcp_trust_replaces_existing_record() {
    let dir = tempfile::tempdir().unwrap();
    run_in_tempdir(
        &["mcp", "trust", "foo", "--reason", "first"],
        dir.path(),
        &[],
    );
    run_in_tempdir(
        &["mcp", "trust", "foo", "--reason", "second"],
        dir.path(),
        &[],
    );
    let contents = read_overrides(dir.path()).unwrap();
    assert!(contents.contains("\"reason\": \"second\""));
    assert!(!contents.contains("\"reason\": \"first\""));
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 001 / 002 / 005: pin / verify / explain — guarded paths
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_verify_without_lockfile_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    // Run in cwd without lockfile
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "verify"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_mcp_diff_without_lockfile_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "diff"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_mcp_explain_requires_lock_flag() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "explain"])
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 006: refresh / resolve-conflicts
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_pin_refresh_without_accept_or_yes_exits_2() {
    // First create a lockfile so --refresh has something to update.
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "pin", "--refresh"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_mcp_pin_resolve_conflicts_ours_picks_ours_side() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    let conflicted = "\
format_version = 1
pattern_set_version = \"2026-05-12\"
<<<<<<< HEAD
ours_field = 1
=======
theirs_field = 2
>>>>>>> branch
";
    std::fs::write(envforge_dir.join("mcp.lock"), conflicted).unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "pin", "--resolve-conflicts", "ours"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let resolved = std::fs::read_to_string(envforge_dir.join("mcp.lock")).unwrap();
    assert!(resolved.contains("ours_field"));
    assert!(!resolved.contains("theirs_field"));
    assert!(!resolved.contains("<<<<<<<"));
}

#[test]
fn test_mcp_pin_resolve_conflicts_theirs_picks_theirs_side() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    let conflicted = "\
format_version = 1
pattern_set_version = \"2026-05-12\"
<<<<<<< HEAD
ours_field = 1
=======
theirs_field = 2
>>>>>>> branch
";
    std::fs::write(envforge_dir.join("mcp.lock"), conflicted).unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "pin", "--resolve-conflicts", "theirs"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let resolved = std::fs::read_to_string(envforge_dir.join("mcp.lock")).unwrap();
    assert!(resolved.contains("theirs_field"));
    assert!(!resolved.contains("ours_field"));
}

#[test]
fn test_mcp_pin_resolve_conflicts_unknown_strategy_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(envforge_dir.join("mcp.lock"), "x\n").unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "pin", "--resolve-conflicts", "garbage"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON output
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_verify_json_with_empty_lockfile_emits_structured_report() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "verify", "--json"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["format_version"], 1);
    assert_eq!(parsed["pattern_set_version"], "2026-05-12");
    assert!(parsed["servers"].is_array());
    assert_eq!(parsed["exit_code"], 0);
}

#[test]
fn test_mcp_explain_lock_markdown_format() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "explain", "--lock", "--format", "markdown"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("MCP Lockfile"));
    assert!(stdout.contains("|"));
}

#[test]
fn test_mcp_explain_lock_text_format_default() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "explain", "--lock"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("mcp.lock"));
    assert!(stdout.contains("pattern_set_version"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 001 / 002: mcp launch — guard paths
// (Cannot fully test execvp replacement in a subprocess harness; we test
// the verify-and-refuse-to-exec paths.)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_launch_unknown_ide_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "launch", "nano"])
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown IDE"));
}

#[test]
fn test_mcp_launch_missing_lockfile_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "launch", "claude-code"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("lockfile missing"));
}

#[test]
fn test_mcp_launch_cursor_alias_recognized() {
    // Unknown IDE returns exit 2 with specific message; cursor is recognized
    // and proceeds to the lockfile-missing check (exit 2) — distinguishable
    // via stderr content (the unknown-IDE message must NOT appear).
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["mcp", "launch", "cursor"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("unknown IDE"));
    assert!(stderr.contains("lockfile missing"));
}

#[test]
fn test_mcp_launch_empty_lockfile_attempts_exec() {
    // With empty lockfile (no KnownBad), launch proceeds past verify to
    // attempt exec'ing the IDE binary. Since the IDE binary is unlikely
    // to exist in the test environment's PATH, exec fails with exit 127.
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let out = Command::new(envforge_bin())
        .args(["mcp", "launch", "claude-code"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        // Restrict PATH to a tempdir to ensure `claude` is not found.
        .env("PATH", "/nonexistent-test-path")
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("verify passed"));
    assert!(stderr.contains("launching"));
    // Exit code is OS-dependent on exec failure but should be non-zero.
    assert_ne!(out.status.code(), Some(0));
}
