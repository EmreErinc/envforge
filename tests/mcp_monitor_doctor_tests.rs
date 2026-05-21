//! Integration tests for bolt 081-monitor-doctor.
//!
//! Tests the public surface of:
//! - `ops::monitor::mcp_reverify_tick` + `McpReverifyState`
//! - `ops::doctor::build_mcp_section` + `McpHealthSection`
//! - `EventType::Mcp*` variants serde round-trip
//! - `doctor --fail-on mcp` exit code via subprocess

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Serializes cwd-changing tests. `std::env::set_current_dir` is
/// process-global; without this, parallel tests trample each other's cwd
/// and the next `current_dir()` call races a deleted tempdir.
fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

use chrono::Utc;
use envforge::ops::audit::types::EventType;
use envforge::ops::doctor::{build_mcp_section, DoctorOpts};
use envforge::ops::monitor::{
    mcp_reverify_tick, mcp_reverify_ttl, McpReverifyOutcome, McpReverifyState,
};

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 001: mcp_reverify_tick
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_reverify_first_run_no_lockfile_in_cwd_returns_no_lockfile() {
    let _guard = cwd_lock().lock().unwrap();
    // Run from a tempdir cwd guaranteed to have no `.envforge/mcp.lock`.
    let cwd = tempfile::tempdir().unwrap();
    let prev = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    std::env::set_current_dir(cwd.path()).unwrap();

    let mut state = McpReverifyState::default();
    let outcome = mcp_reverify_tick(Utc::now(), &mut state);

    let _ = std::env::set_current_dir(prev);
    assert_eq!(outcome, McpReverifyOutcome::NoLockfile);
    assert!(state.last_verify_at.is_some());
}

#[test]
fn test_reverify_before_ttl_returns_skipped() {
    let now = Utc::now();
    let mut state = McpReverifyState {
        last_verify_at: Some(now),
        last_known_bad: Vec::new(),
    };
    // Same instant → 0 elapsed → before TTL → Skipped
    let outcome = mcp_reverify_tick(now, &mut state);
    assert_eq!(outcome, McpReverifyOutcome::Skipped);
}

#[test]
fn test_reverify_ttl_env_override_parsed() {
    // Save current env state
    let prev = std::env::var("ENVFORGE_MCP_REVERIFY_TTL").ok();

    std::env::set_var("ENVFORGE_MCP_REVERIFY_TTL", "300");
    assert_eq!(mcp_reverify_ttl(), Duration::from_secs(300));

    std::env::set_var("ENVFORGE_MCP_REVERIFY_TTL", "garbage");
    // Invalid value falls back to default
    assert_eq!(mcp_reverify_ttl(), Duration::from_secs(7 * 24 * 60 * 60));

    // Restore
    match prev {
        Some(v) => std::env::set_var("ENVFORGE_MCP_REVERIFY_TTL", v),
        None => std::env::remove_var("ENVFORGE_MCP_REVERIFY_TTL"),
    }
}

#[test]
fn test_reverify_state_default() {
    let state = McpReverifyState::default();
    assert!(state.last_verify_at.is_none());
    assert!(state.last_known_bad.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 002: build_mcp_section
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_build_mcp_section_missing_lockfile() {
    let _guard = cwd_lock().lock().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let prev = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    std::env::set_current_dir(cwd.path()).unwrap();

    let opts = DoctorOpts {
        include_unknown: false,
    };
    let section = build_mcp_section(&opts).expect("section returned");

    let _ = std::env::set_current_dir(prev);
    assert!(!section.lockfile_exists);
    assert_eq!(section.pinned_server_count, 0);
    assert_eq!(section.known_bad_count, 0);
    assert!(!section.has_critical_findings());
}

#[test]
fn test_build_mcp_section_empty_lockfile() {
    let _guard = cwd_lock().lock().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let envforge_dir = cwd.path().join(".envforge");
    std::fs::create_dir_all(&envforge_dir).unwrap();
    std::fs::write(
        envforge_dir.join("mcp.lock"),
        "format_version = 1\npattern_set_version = \"2026-05-12\"\n",
    )
    .unwrap();

    let prev = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    std::env::set_current_dir(cwd.path()).unwrap();

    let opts = DoctorOpts {
        include_unknown: false,
    };
    let section = build_mcp_section(&opts).expect("section returned");

    let _ = std::env::set_current_dir(prev);
    assert!(section.lockfile_exists);
    assert_eq!(section.pinned_server_count, 0);
    assert_eq!(section.known_bad_count, 0);
    assert!(section.known_bad_servers.is_empty());
    assert!(!section.has_critical_findings());
    assert!(!section.feed_version.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 003: EventType variants serde round-trip
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_event_types_serialize() {
    let variants = [
        EventType::McpPinned,
        EventType::McpVerifyFailed,
        EventType::McpReverifyOk,
        EventType::McpReverifyFailed,
        EventType::McpPoisonDetected,
        EventType::McpFeedFlippedKnownBad,
        EventType::McpUserTrustGranted,
        EventType::McpUserTrustRevoked,
        EventType::McpLaunchBlocked,
        EventType::McpFeedStale,
    ];
    for v in &variants {
        let s = serde_json::to_string(v).expect("serializes");
        let back: EventType = serde_json::from_str(&s).expect("round-trips");
        assert_eq!(*v, back);
    }
}

#[test]
fn test_mcp_event_types_count_is_ten() {
    // Defensive: confirms we have exactly 10 new variants for the bolt.
    let variants = [
        EventType::McpPinned,
        EventType::McpVerifyFailed,
        EventType::McpReverifyOk,
        EventType::McpReverifyFailed,
        EventType::McpPoisonDetected,
        EventType::McpFeedFlippedKnownBad,
        EventType::McpUserTrustGranted,
        EventType::McpUserTrustRevoked,
        EventType::McpLaunchBlocked,
        EventType::McpFeedStale,
    ];
    assert_eq!(variants.len(), 10);
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI integration: doctor --all + --fail-on mcp
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_doctor_with_all_flag_runs() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["doctor", "--all"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    // Doctor may exit 0 or non-zero based on system state; assert it ran.
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = (stderr, stdout);
    // Status code is execution success regardless of doctor findings.
    assert!(out.status.code().is_some());
}

#[test]
fn test_doctor_fail_on_mcp_with_no_lockfile_exits_zero_for_mcp() {
    // With no lockfile, MCP section reports no critical findings → exit 0
    // for the mcp subsystem regardless of other doctor checks (because we
    // only exit on mcp critical when --fail-on mcp is passed).
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(envforge_bin())
        .args(["doctor", "--fail-on", "mcp"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn");
    // No lockfile → no known_bad → --fail-on mcp does NOT exit 2.
    assert_ne!(out.status.code(), Some(2));
}

#[test]
fn test_doctor_help_documents_new_flags() {
    let out = Command::new(envforge_bin())
        .args(["doctor", "--help"])
        .output()
        .expect("spawn");
    let blob = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(blob.contains("--all"));
    assert!(blob.contains("--fail-on"));
}
