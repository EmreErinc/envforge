use std::path::Path;

use envforge::model::*;
use envforge::ops::*;
use envforge::parser::*;

fn make_shell_file(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

fn make_shell_file_at(content: &str, path: &str) -> ShellFile {
    parse_shell_content(content, Path::new(path)).unwrap()
}

// ═══════════════════════════════════════════════════════════════
// Reference File Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_ensure_reference_file_creates() {
    let dir = tempfile::tempdir().unwrap();
    let ref_path = dir.path().join("env_managed");

    assert!(!ref_path.exists());
    ensure_reference_file(&ref_path).unwrap();
    assert!(ref_path.exists());

    let content = std::fs::read_to_string(&ref_path).unwrap();
    assert!(content.contains("EnvForge"));
}

#[test]
fn test_ensure_reference_file_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let ref_path = dir.path().join("env_managed");

    ensure_reference_file(&ref_path).unwrap();
    std::fs::write(&ref_path, "custom content\n").unwrap();
    ensure_reference_file(&ref_path).unwrap();

    // Should not overwrite existing content
    let content = std::fs::read_to_string(&ref_path).unwrap();
    assert_eq!(content, "custom content\n");
}

#[test]
fn test_has_source_directive_false() {
    let sf = make_shell_file("export FOO=\"bar\"\n");
    assert!(!has_source_directive(&sf, Path::new("~/.env_managed")));
}

#[test]
fn test_has_source_directive_true() {
    let sf = make_shell_file("source ~/.env_managed\n");
    assert!(has_source_directive(&sf, Path::new("~/.env_managed")));
}

#[test]
fn test_ensure_source_directive_injects() {
    let mut sf = make_shell_file("# header\nexport FOO=\"bar\"\n# footer\n");
    ensure_source_directive(&mut sf, Path::new("~/.env_managed"), 0, 1).unwrap();

    let serialized = serialize_shell_file(&sf);
    assert!(serialized.contains("envforge:source"));
    assert!(serialized.contains("source \"~/.env_managed\""));
}

#[test]
fn test_ensure_source_directive_idempotent() {
    let mut sf = make_shell_file(
        "# header\n# [envforge:source] Managed environment variables\n[ -f \"~/.env_managed\" ] && source \"~/.env_managed\"\nexport FOO=\"bar\"\n",
    );
    let lines_before = sf.lines.len();
    ensure_source_directive(&mut sf, Path::new("~/.env_managed"), 0, 0).unwrap();
    assert_eq!(sf.lines.len(), lines_before); // No new lines added
}

#[test]
fn test_move_to_reference() {
    let mut primary = make_shell_file("export API_KEY=\"secret\"\nexport OTHER=\"keep\"\n");
    let mut ref_file = make_shell_file_at("# Managed\n", "/test/.env_managed");

    move_to_reference(
        &mut primary,
        &mut ref_file,
        "API_KEY",
        Path::new("~/.env_managed"),
    )
    .unwrap();

    // Primary: API_KEY should be a managed comment
    assert!(matches!(primary.lines[0], LineNode::ManagedComment { .. }));
    let text = primary.lines[0].original_text();
    assert!(text.contains("#[envforge:moved:API_KEY"));
    assert!(text.contains("export API_KEY=\"secret\""));

    // Reference: should have the export
    let ref_entries = collect_entries(&ref_file);
    assert!(ref_entries.iter().any(|e| e.key == "API_KEY"));

    // OTHER should be untouched
    assert!(matches!(primary.lines[1], LineNode::EnvExport { .. }));
}

#[test]
fn test_restore_from_reference() {
    let mut primary = make_shell_file("export API_KEY=\"secret\"\nexport OTHER=\"keep\"\n");
    let mut ref_file = make_shell_file_at("# Managed\n", "/test/.env_managed");

    // Move first
    move_to_reference(
        &mut primary,
        &mut ref_file,
        "API_KEY",
        Path::new("~/.env_managed"),
    )
    .unwrap();

    // Then restore
    restore_from_reference(&mut primary, &mut ref_file, "API_KEY").unwrap();

    // Primary: API_KEY should be an EnvExport again
    match &primary.lines[0] {
        LineNode::EnvExport { key, value, .. } => {
            assert_eq!(key, "API_KEY");
            assert_eq!(value, "secret");
        }
        other => panic!("Expected EnvExport after restore, got: {:?}", other),
    }

    // Reference: API_KEY should be removed
    let ref_entries = collect_entries(&ref_file);
    assert!(!ref_entries.iter().any(|e| e.key == "API_KEY"));
}

#[test]
fn test_move_not_found() {
    let mut primary = make_shell_file("export FOO=\"bar\"\n");
    let mut ref_file = make_shell_file_at("# Managed\n", "/test/.env_managed");

    let result = move_to_reference(
        &mut primary,
        &mut ref_file,
        "MISSING",
        Path::new("~/.env_managed"),
    );
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════
// Offset / Protected Block Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_detect_conda_block() {
    let sf = make_shell_file(
        "\
export MY_VAR=\"hello\"
# >>> conda initialize >>>
__conda_setup=\"something\"
eval \"$__conda_setup\"
# <<< conda initialize <<<
export OTHER=\"world\"
",
    );
    let blocks = detect_protected_blocks(&sf);
    assert!(
        blocks.iter().any(|b| b.name == "conda"),
        "Conda block not detected"
    );
}

#[test]
fn test_detect_amazon_q_block() {
    let sf = make_shell_file(
        "\
export MY_VAR=\"hello\"
# Q pre block. Keep at the top.
[[ -f something ]] && builtin source something
export OTHER=\"world\"
# Q post block. Keep at the bottom.
[[ -f something ]] && builtin source something
",
    );
    let blocks = detect_protected_blocks(&sf);
    assert!(
        blocks.iter().any(|b| b.name.contains("Amazon Q")),
        "Amazon Q block not detected"
    );
}

#[test]
fn test_detect_no_blocks() {
    let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=\"qux\"\n");
    let blocks = detect_protected_blocks(&sf);
    assert!(blocks.is_empty());
}

#[test]
fn test_calculate_safe_zone() {
    let zone = calculate_safe_zone(20, 0, 5).unwrap();
    assert_eq!(zone.start, 0);
    assert_eq!(zone.end, 15);
    assert_eq!(zone.size(), 15);
    assert!(zone.contains(0));
    assert!(zone.contains(14));
    assert!(!zone.contains(15));
}

#[test]
fn test_calculate_safe_zone_with_header() {
    let zone = calculate_safe_zone(20, 3, 5).unwrap();
    assert_eq!(zone.start, 3);
    assert_eq!(zone.end, 15);
    assert!(!zone.contains(2));
    assert!(zone.contains(3));
}

#[test]
fn test_calculate_safe_zone_none_when_full() {
    let zone = calculate_safe_zone(10, 5, 5);
    assert!(zone.is_none());
}

#[test]
fn test_calculate_safe_zone_none_when_over() {
    let zone = calculate_safe_zone(5, 3, 3);
    assert!(zone.is_none());
}

#[test]
fn test_suggest_offsets_no_blocks() {
    let sf = make_shell_file("export A=1\nexport B=2\n");
    let (header, footer) = suggest_offsets(&sf);
    assert_eq!(header, 0);
    assert_eq!(footer, 0);
}

// ═══════════════════════════════════════════════════════════════
// Conflict / Diff Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_check_conflict_no_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.txt");
    let content = "export FOO=\"bar\"\n";
    std::fs::write(&path, content).unwrap();

    let sf = parse_shell_file(&path).unwrap();
    assert!(check_conflict(&sf).is_none());
}

#[test]
fn test_check_conflict_detects_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.txt");
    std::fs::write(&path, "export FOO=\"bar\"\n").unwrap();

    let sf = parse_shell_file(&path).unwrap();

    // Modify file externally
    std::fs::write(&path, "export FOO=\"changed\"\n").unwrap();

    let conflict = check_conflict(&sf);
    assert!(conflict.is_some());
    assert_ne!(conflict.unwrap().stored_hash, [0u8; 32]);
}

#[test]
fn test_generate_diff_from_strings_no_change() {
    let diff = generate_diff_from_strings("same\n", "same\n", "test.txt");
    assert!(diff.is_empty());
}

#[test]
fn test_generate_diff_from_strings_with_change() {
    let diff = generate_diff_from_strings(
        "export FOO=\"old\"\nexport BAR=\"keep\"\n",
        "export FOO=\"new\"\nexport BAR=\"keep\"\n",
        "test.txt",
    );
    assert!(diff.contains("---"));
    assert!(diff.contains("+++"));
    assert!(diff.contains("-export FOO=\"old\""));
    assert!(diff.contains("+export FOO=\"new\""));
}

#[test]
fn test_generate_diff_from_strings_added_line() {
    let diff = generate_diff_from_strings("export A=1\n", "export A=1\nexport B=2\n", "test.txt");
    assert!(diff.contains("+export B=2"));
}

#[test]
fn test_generate_diff_from_strings_removed_line() {
    let diff = generate_diff_from_strings("export A=1\nexport B=2\n", "export A=1\n", "test.txt");
    assert!(diff.contains("-export B=2"));
}
