//! Coverage for `ops::fence` create/status/remove/apply lifecycle, including
//! the destructive `remove_fence` dry-run guarantee, idempotency, config-gated
//! skipping, and `apply_tool` error paths. Uses explicit `FenceConfig` so the
//! tests never depend on the developer's global config.

use envforge::config::FenceConfig;
use envforge::ops::fence::{
    apply_tool, check_fence_status_with, create_fence_for, create_fence_with, remove_fence,
    FenceCompleteness, FenceTarget,
};
use tempfile::TempDir;

#[test]
fn test_fence_target_all_ids_are_snake_case() {
    let all = FenceTarget::all();
    assert!(!all.is_empty());
    for t in all {
        let id = t.as_str();
        assert!(!id.is_empty());
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "non snake_case id: {id}"
        );
    }
}

#[test]
fn test_create_fence_with_default_writes_all_and_status_complete() {
    let tmp = TempDir::new().unwrap();
    let cfg = FenceConfig::default();
    let r = create_fence_with(tmp.path(), false, &cfg).unwrap();

    assert!(!r.files_created.is_empty());
    assert!(r.files_failed.is_empty());
    assert!(tmp.path().join(".envforgeignore").exists());

    let status = check_fence_status_with(tmp.path(), &cfg).unwrap();
    assert!(status.all_fenced);
    assert!(matches!(status.completeness, FenceCompleteness::Complete));
}

#[test]
fn test_check_fence_status_empty_dir_is_partial() {
    let tmp = TempDir::new().unwrap();
    let status = check_fence_status_with(tmp.path(), &FenceConfig::default()).unwrap();
    assert!(!status.all_fenced);
    assert!(matches!(status.completeness, FenceCompleteness::Partial(_)));
}

#[test]
fn test_create_fence_with_disabled_target_is_skipped_not_written() {
    let tmp = TempDir::new().unwrap();
    let mut cfg = FenceConfig::default();
    cfg.targets.set_enabled(FenceTarget::AmazonQ, false);

    let r = create_fence_with(tmp.path(), false, &cfg).unwrap();
    let amazonq = tmp.path().join(".amazonq/rules/envforge.md");
    assert!(!amazonq.exists(), "disabled target must not be written");
    assert!(r
        .files_skipped
        .iter()
        .any(|p| p.to_string_lossy().contains(".amazonq")));
}

#[test]
fn test_create_fence_for_subset_only_writes_specified() {
    let tmp = TempDir::new().unwrap();
    create_fence_for(tmp.path(), &[FenceTarget::Envforgeignore], false).unwrap();
    assert!(tmp.path().join(".envforgeignore").exists());
    assert!(!tmp.path().join(".cursorignore").exists());
}

#[test]
fn test_create_fence_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let cfg = FenceConfig::default();
    create_fence_with(tmp.path(), false, &cfg).unwrap();
    let second = create_fence_with(tmp.path(), false, &cfg).unwrap();
    assert!(
        second.files_created.is_empty(),
        "re-run must create nothing"
    );
    assert!(!second.files_skipped.is_empty());
}

#[test]
fn test_remove_fence_dry_run_keeps_files_then_real_removes() {
    let tmp = TempDir::new().unwrap();
    create_fence_with(tmp.path(), false, &FenceConfig::default()).unwrap();
    let ignore = tmp.path().join(".envforgeignore");
    assert!(ignore.exists());

    let dry = remove_fence(tmp.path(), true).unwrap();
    assert!(ignore.exists(), "dry-run must not delete anything");
    assert!(
        !dry.files_removed.is_empty() || !dry.files_updated.is_empty(),
        "dry-run still reports what it would change"
    );

    remove_fence(tmp.path(), false).unwrap();
    assert!(!ignore.exists(), "real remove deletes the owned file");
}

#[test]
fn test_apply_tool_error_paths_and_dry_run() {
    let tmp = TempDir::new().unwrap();

    // Unknown tool name is rejected.
    assert!(apply_tool(tmp.path(), "bogus_tool", false).is_err());
    // Known tool but no .envforgeignore source yet.
    assert!(apply_tool(tmp.path(), "cursor", false).is_err());

    // With a source present, dry-run returns the target path without creating it.
    create_fence_with(tmp.path(), false, &FenceConfig::default()).unwrap();
    let planned = apply_tool(tmp.path(), "continue", true).unwrap();
    assert_eq!(planned, tmp.path().join(".continueignore"));
    assert!(
        !tmp.path().join(".continueignore").exists(),
        "dry-run must not create the link"
    );

    // Real apply creates the propagated file/link.
    let made = apply_tool(tmp.path(), "continue", false).unwrap();
    assert!(std::fs::symlink_metadata(&made).is_ok());
}
