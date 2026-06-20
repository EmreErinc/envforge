//! Regression test (2026-06-20 hardening): a config backup may carry plaintext
//! secrets, so it must be created 0600 — never momentarily world-readable via a
//! copy-then-chmod window.
//!
//! Single test in its own binary so the process-global `ENVFORGE_CONFIG_DIR`
//! override cannot race other tests. Unix-only (permission bits).

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

#[test]
fn test_config_backup_is_created_0600() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ENVFORGE_CONFIG_DIR", tmp.path());

    let src = tmp.path().join("config.toml");
    std::fs::write(&src, "token = \"supersecret\"\n").unwrap();

    let backup = envforge::config::create_backup(&src).expect("create backup");
    let mode = std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "backup must be 0600, got {mode:o}");
}
