//! Coverage for `ops::crud` rename + soft-delete paths not exercised elsewhere:
//! `rename_entry_at` bounds/type errors, `rename_entry` missing-key + collision,
//! `find_soft_deleted`, and the `soft_delete`/`undo_delete` round-trip.

use envforge::model::{LineNode, ShellFile};
use envforge::ops::{find_soft_deleted, rename_entry, rename_entry_at, soft_delete, undo_delete};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn sf(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

fn key_at(shell: &ShellFile, idx: usize) -> Option<&str> {
    match &shell.lines[idx] {
        LineNode::EnvExport { key, .. } => Some(key.as_str()),
        _ => None,
    }
}

// ---- rename_entry_at -------------------------------------------------------

#[test]
fn test_rename_entry_at_success() {
    let mut s = sf("export OLD=1");
    rename_entry_at(&mut s, 0, "NEW").unwrap();
    assert_eq!(key_at(&s, 0), Some("NEW"));
}

#[test]
fn test_rename_entry_at_out_of_bounds_errors() {
    let mut s = sf("export A=1");
    assert!(rename_entry_at(&mut s, 99, "X").is_err());
}

#[test]
fn test_rename_entry_at_non_export_errors() {
    let mut s = sf("# just a comment");
    assert!(rename_entry_at(&mut s, 0, "X").is_err());
}

// ---- rename_entry ----------------------------------------------------------

#[test]
fn test_rename_entry_missing_key_errors() {
    let mut s = sf("export A=1");
    assert!(rename_entry(&mut s, "NOPE", "X").is_err());
}

#[test]
fn test_rename_entry_collision_creates_duplicate() {
    // rename_entry does not guard against the new key already existing; it will
    // produce two entries with the same key. Documents current behavior.
    let mut s = sf("export A=1\nexport B=2");
    rename_entry(&mut s, "A", "B").unwrap();
    assert_eq!(key_at(&s, 0), Some("B"));
    assert_eq!(key_at(&s, 1), Some("B"));
}

// ---- find_soft_deleted / soft_delete / undo_delete -------------------------

#[test]
fn test_find_soft_deleted_none_when_not_deleted() {
    let s = sf("export A=1");
    assert!(find_soft_deleted(&s, "A").is_none());
}

#[test]
fn test_soft_delete_then_find_and_undo_roundtrip() {
    let mut s = sf("export API_KEY=\"secret\"");
    soft_delete(&mut s, "API_KEY").unwrap();
    assert!(
        find_soft_deleted(&s, "API_KEY").is_some(),
        "soft-deleted key must be discoverable"
    );

    undo_delete(&mut s, "API_KEY").unwrap();
    assert!(
        find_soft_deleted(&s, "API_KEY").is_none(),
        "undo removes the deletion marker"
    );
    assert_eq!(key_at(&s, 0), Some("API_KEY"));
}

#[test]
fn test_undo_delete_missing_errors() {
    let mut s = sf("export A=1");
    assert!(undo_delete(&mut s, "NEVER_DELETED").is_err());
}
