use std::path::Path;

use envforge::model::*;
use envforge::ops::*;
use envforge::parser::*;

fn make_shell_file(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

#[test]
fn test_parse_envforge_start_marker() {
    let sf = make_shell_file("# >>> envforge >>>\nexport FOO=\"bar\"\n# <<< envforge <<<\n");
    assert!(matches!(sf.lines[0], LineNode::EnvforgeStart { .. }));
    assert!(matches!(sf.lines[2], LineNode::EnvforgeEnd { .. }));
}

#[test]
fn test_parse_envforge_markers_with_extra_whitespace() {
    let sf =
        make_shell_file("  # >>> envforge >>>  \nexport FOO=\"bar\"\n  # <<< envforge <<<  \n");
    assert!(matches!(sf.lines[0], LineNode::EnvforgeStart { .. }));
    assert!(matches!(sf.lines[2], LineNode::EnvforgeEnd { .. }));
}

#[test]
fn test_find_managed_zone() {
    let sf = make_shell_file(
        "# header\n# >>> envforge >>>\nexport FOO=\"bar\"\n# <<< envforge <<<\n# footer\n",
    );
    let zone = find_managed_zone(&sf).unwrap();
    assert_eq!(zone.start_idx, 1);
    assert_eq!(zone.end_idx, 3);
}

#[test]
fn test_find_managed_zone_missing() {
    let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=\"qux\"\n");
    assert!(find_managed_zone(&sf).is_none());
}

#[test]
fn test_has_managed_zone() {
    let sf = make_shell_file("# >>> envforge >>>\nexport FOO=\"bar\"\n# <<< envforge <<<\n");
    assert!(has_managed_zone(&sf));

    let sf2 = make_shell_file("export FOO=\"bar\"\n");
    assert!(!has_managed_zone(&sf2));
}

#[test]
fn test_add_entry_inserts_before_end_marker() {
    let mut sf = make_shell_file(
        "# header\n# >>> envforge >>>\nexport EXISTING=\"val\"\n# <<< envforge <<<\n# footer\n",
    );
    add_entry(
        &mut sf,
        "NEW_KEY",
        "new_val",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        1,
    )
    .unwrap();

    let new_idx = sf
        .lines
        .iter()
        .position(|n| matches!(n, LineNode::EnvExport { key, .. } if key == "NEW_KEY"))
        .unwrap();
    let end_idx = sf
        .lines
        .iter()
        .position(|n| matches!(n, LineNode::EnvforgeEnd { .. }))
        .unwrap();

    assert!(new_idx < end_idx, "new entry should be before end marker");
    assert!(new_idx > 1, "new entry should be after start marker");
}

#[test]
fn test_add_entry_without_markers_falls_back_to_offsets() {
    let mut sf = make_shell_file("# header\nexport EXISTING=\"val\"\n# footer\n");
    add_entry(
        &mut sf,
        "NEW_KEY",
        "new_val",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        1,
    )
    .unwrap();

    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.key == "NEW_KEY"));
}

#[test]
fn test_ensure_managed_zone_adds_markers() {
    let mut sf = make_shell_file("# header\nexport FOO=\"bar\"\n# footer\n");
    assert!(!has_managed_zone(&sf));

    ensure_managed_zone(&mut sf);
    assert!(has_managed_zone(&sf));

    let serialized = serialize_shell_file(&sf);
    assert!(serialized.contains("# >>> envforge >>>"));
    assert!(serialized.contains("# <<< envforge <<<"));
}

#[test]
fn test_ensure_managed_zone_idempotent() {
    let mut sf = make_shell_file("# >>> envforge >>>\nexport FOO=\"bar\"\n# <<< envforge <<<\n");
    assert!(has_managed_zone(&sf));

    ensure_managed_zone(&mut sf);
    let count_start = sf
        .lines
        .iter()
        .filter(|n| matches!(n, LineNode::EnvforgeStart { .. }))
        .count();
    let count_end = sf
        .lines
        .iter()
        .filter(|n| matches!(n, LineNode::EnvforgeEnd { .. }))
        .count();
    assert_eq!(count_start, 1, "should not add duplicate start markers");
    assert_eq!(count_end, 1, "should not add duplicate end markers");
}

#[test]
fn test_find_soft_deleted() {
    let mut sf = make_shell_file("export FOO=\"bar\"\n");
    soft_delete(&mut sf, "FOO").unwrap();

    assert!(find_soft_deleted(&sf, "FOO").is_some());
    assert!(find_soft_deleted(&sf, "NONEXISTENT").is_none());
}

#[test]
fn test_set_restores_soft_deleted_in_place() {
    let mut sf = make_shell_file(
        "# >>> envforge >>>\nexport FOO=\"old\"\nexport BAZ=\"keep\"\n# <<< envforge <<<\n",
    );

    soft_delete(&mut sf, "FOO").unwrap();
    assert!(find_soft_deleted(&sf, "FOO").is_some());

    undo_delete(&mut sf, "FOO").unwrap();
    edit_entry(&mut sf, "FOO", "new").unwrap();

    match &sf.lines[1] {
        LineNode::EnvExport { key, value, .. } => {
            assert_eq!(key, "FOO");
            assert_eq!(value, "new");
        }
        other => panic!("Expected EnvExport at index 1, got: {:?}", other),
    }

    match &sf.lines[2] {
        LineNode::EnvExport { key, .. } => {
            assert_eq!(key, "BAZ");
        }
        other => panic!("Expected EnvExport at index 2, got: {:?}", other),
    }
}

#[test]
fn test_add_entry_managed_zone_preserves_order() {
    let mut sf =
        make_shell_file("# >>> envforge >>>\nexport A=\"1\"\n# <<< envforge <<<\n# footer block\n");

    add_entry(
        &mut sf,
        "B",
        "2",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap();
    add_entry(
        &mut sf,
        "C",
        "3",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap();

    let serialized = serialize_shell_file(&sf);
    let a_pos = serialized.find("export A=").unwrap();
    let b_pos = serialized.find("export B=").unwrap();
    let c_pos = serialized.find("export C=").unwrap();
    let end_pos = serialized.find("# <<< envforge <<<").unwrap();

    assert!(a_pos < b_pos, "A should come before B");
    assert!(b_pos < c_pos, "B should come before C");
    assert!(c_pos < end_pos, "C should come before end marker");
}

#[test]
fn test_edit_entry_stays_in_place_within_zone() {
    let mut sf = make_shell_file(
        "# >>> envforge >>>\nexport FIRST=\"1\"\nexport SECOND=\"2\"\n# <<< envforge <<<\n",
    );

    edit_entry(&mut sf, "FIRST", "updated").unwrap();

    match &sf.lines[1] {
        LineNode::EnvExport { key, value, .. } => {
            assert_eq!(key, "FIRST");
            assert_eq!(value, "updated");
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }

    let serialized = serialize_shell_file(&sf);
    let first_pos = serialized.find("export FIRST=").unwrap();
    let second_pos = serialized.find("export SECOND=").unwrap();
    let end_pos = serialized.find("# <<< envforge <<<").unwrap();

    assert!(
        first_pos < second_pos,
        "FIRST should still come before SECOND"
    );
    assert!(
        second_pos < end_pos,
        "SECOND should still be before end marker"
    );
}

#[test]
fn test_round_trip_with_markers() {
    let content = "# >>> envforge >>>\nexport FOO=\"bar\"\n# <<< envforge <<<\n";
    let sf = make_shell_file(content);
    let serialized = serialize_shell_file(&sf);
    assert_eq!(serialized, content);
}
