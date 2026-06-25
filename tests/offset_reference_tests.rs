//! Coverage for `ops::offset` boundary cases and `ops::reference` move/source
//! plumbing not exercised by the in-module tests.

use envforge::model::{LineNode, ShellFile};
use envforge::ops::offset::{
    calculate_safe_zone, detect_protected_blocks, find_managed_zone, has_managed_zone,
};
use envforge::ops::{
    ensure_reference_file, ensure_source_directive, has_source_directive, move_to_reference,
};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn sf(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

// ─── offset ────────────────────────────────────────────────────────────────

#[test]
fn test_calculate_safe_zone_equal_nonzero_offsets_is_none() {
    // header == footer-derived end → zero-width zone → None.
    assert!(calculate_safe_zone(10, 5, 5).is_none());
}

#[test]
fn test_calculate_safe_zone_footer_exceeds_total_is_none() {
    assert!(calculate_safe_zone(5, 0, 10).is_none());
}

#[test]
fn test_detect_protected_blocks_unclosed_is_ignored() {
    // A conda start marker with no matching end marker yields no block.
    let s = sf("# >>> conda initialize >>>\nconda_stuff\nexport FOO=\"bar\"");
    assert!(detect_protected_blocks(&s).is_empty());
}

#[test]
fn test_find_managed_zone_present_and_inverted() {
    let present = sf("# >>> envforge >>>\nexport A=1\n# <<< envforge <<<");
    assert!(has_managed_zone(&present));
    let zone = find_managed_zone(&present).unwrap();
    assert!(zone.end_idx > zone.start_idx);

    // End marker before start marker → not a valid zone.
    let inverted = sf("# <<< envforge <<<\nexport A=1\n# >>> envforge >>>");
    assert!(find_managed_zone(&inverted).is_none());
}

// ─── reference ───────────────────────────────────────────────────────────────

#[test]
fn test_ensure_reference_file_creates_nested_with_header() {
    let tmp = tempfile::tempdir().unwrap();
    let ref_path = tmp.path().join("a/b/c/.env_managed");
    let out = ensure_reference_file(&ref_path).unwrap();
    assert_eq!(out, ref_path);
    assert!(ref_path.exists());
    let content = std::fs::read_to_string(&ref_path).unwrap();
    assert!(content.contains("EnvForge managed"));
}

#[test]
fn test_source_directive_absent_then_injected() {
    let ref_path = Path::new("/home/u/.env_managed");
    let mut primary = sf("export A=1\nexport B=2\nexport C=3\n");
    assert!(!has_source_directive(&primary, ref_path));

    ensure_source_directive(&mut primary, ref_path, 0, 0).unwrap();
    assert!(has_source_directive(&primary, ref_path));
}

#[test]
fn test_ensure_source_directive_no_safe_zone_errors() {
    let ref_path = Path::new("/home/u/.env_managed");
    let mut primary = sf("export A=1\nexport B=2");
    // header beyond the writable region → no safe zone.
    assert!(ensure_source_directive(&mut primary, ref_path, 5, 0).is_err());
}

#[test]
fn test_move_to_reference_tags_primary_and_appends_ref() {
    let ref_path = Path::new("/home/u/.env_managed");
    let mut primary = sf("export API_KEY=\"secret\"");
    let mut reference = sf("");

    move_to_reference(&mut primary, &mut reference, "API_KEY", ref_path).unwrap();

    // Primary line is now a managed (moved) comment, not a live export.
    assert!(matches!(primary.lines[0], LineNode::ManagedComment { .. }));
    // Reference file gained the export.
    assert!(reference
        .lines
        .iter()
        .any(|n| matches!(n, LineNode::EnvExport { key, .. } if key == "API_KEY")));
}

#[test]
fn test_move_to_reference_missing_key_errors() {
    let ref_path = Path::new("/home/u/.env_managed");
    let mut primary = sf("export A=1");
    let mut reference = sf("");
    assert!(move_to_reference(&mut primary, &mut reference, "NOPE", ref_path).is_err());
}
