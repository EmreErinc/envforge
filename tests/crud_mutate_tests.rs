//! Coverage for `ops::crud` mutation paths: `add_entry` (success / duplicate /
//! no-safe-zone), `edit_entry` (success / missing / ambiguous), and
//! `ensure_managed_zone`.

use envforge::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};
use envforge::ops::{add_entry, edit_entry, ensure_managed_zone, has_managed_zone, OpsError};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn sf(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

fn has_key(shell: &ShellFile, key: &str) -> bool {
    shell
        .lines
        .iter()
        .any(|n| matches!(n, LineNode::EnvExport { key: k, .. } if k == key))
}

fn value_of(shell: &ShellFile, key: &str) -> Option<String> {
    shell.lines.iter().find_map(|n| match n {
        LineNode::EnvExport { key: k, value, .. } if k == key => Some(value.clone()),
        _ => None,
    })
}

// ---- add_entry -------------------------------------------------------------

#[test]
fn test_add_entry_inserts_new_key() {
    let mut s = sf("export A=1\nexport B=2\n");
    add_entry(
        &mut s,
        "C",
        "3",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap();
    assert!(has_key(&s, "C"));
    assert_eq!(value_of(&s, "C").as_deref(), Some("3"));
}

#[test]
fn test_add_entry_duplicate_errors() {
    let mut s = sf("export A=1");
    let err = add_entry(
        &mut s,
        "A",
        "9",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap_err();
    assert!(matches!(err, OpsError::KeyAlreadyExists { .. }));
}

#[test]
fn test_add_entry_no_safe_zone_errors() {
    // header offset past the writable region, no managed zone → NoSafeZone.
    let mut s = sf("export A=1");
    let err = add_entry(
        &mut s,
        "B",
        "2",
        ExportStyle::Export,
        QuoteStyle::Double,
        5,
        0,
    )
    .unwrap_err();
    assert!(matches!(err, OpsError::NoSafeZone { .. }));
}

// ---- edit_entry ------------------------------------------------------------

#[test]
fn test_edit_entry_updates_value_and_text() {
    let mut s = sf("export A=\"old\"");
    edit_entry(&mut s, "A", "new").unwrap();
    assert_eq!(value_of(&s, "A").as_deref(), Some("new"));
    match s
        .lines
        .iter()
        .find(|n| matches!(n, LineNode::EnvExport { key, .. } if key == "A"))
    {
        Some(LineNode::EnvExport { original_text, .. }) => {
            assert!(original_text.contains("new"));
            assert!(!original_text.contains("old"));
        }
        _ => panic!("expected EnvExport A"),
    }
}

#[test]
fn test_edit_entry_missing_errors() {
    let mut s = sf("export A=1");
    assert!(matches!(
        edit_entry(&mut s, "NOPE", "x").unwrap_err(),
        OpsError::KeyNotFound { .. }
    ));
}

#[test]
fn test_edit_entry_ambiguous_errors() {
    let mut s = sf("export A=1\nexport A=2");
    assert!(matches!(
        edit_entry(&mut s, "A", "3").unwrap_err(),
        OpsError::AmbiguousKey { .. }
    ));
}

// ---- ensure_managed_zone ---------------------------------------------------

#[test]
fn test_ensure_managed_zone_creates_then_idempotent() {
    let mut s = sf("export A=1");
    assert!(!has_managed_zone(&s));

    assert!(ensure_managed_zone(&mut s));
    assert!(has_managed_zone(&s), "zone markers must be inserted");

    // Second call is a no-op that still reports the zone present.
    assert!(ensure_managed_zone(&mut s));
    assert!(has_managed_zone(&s));
}
