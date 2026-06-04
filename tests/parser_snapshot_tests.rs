mod common;

use std::path::Path;

use envforge::parser::*;

// ══════════════════════════════════════════════════════════════
// Parser Round-Trip Snapshot Tests
// ══════════════════════════════════════════════════════════════

/// Parse and serialize should produce byte-identical output for simple assignments.
#[test]
fn test_roundtrip_simple_export() {
    let content = "export API_KEY=\"abc123\"\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with single-quoted values preserves quote style.
#[test]
fn test_roundtrip_single_quotes() {
    let content = "export DB_HOST='localhost'\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with inline comments preserves them.
#[test]
fn test_roundtrip_inline_comment() {
    let content = "export PORT=\"8080\" # web server port\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip multiple assignments preserves ordering.
#[test]
fn test_roundtrip_multiple_exports() {
    let content = "export FOO=\"bar\"\nexport BAZ=\"qux\"\nexport XYZ=\"123\"\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip mixed content (comments, blank lines, exports).
#[test]
fn test_roundtrip_mixed_content() {
    let content = "# Configuration file\nexport FOO=\"bar\"\n\n# Database settings\nexport DB_URL=\"postgres://localhost\"\n\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with source directive.
#[test]
fn test_roundtrip_source_directive() {
    let content = "source ~/.secrets\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with managed zone markers.
#[test]
fn test_roundtrip_managed_zone() {
    let content = "# >>> envforge >>>\nexport KEY=\"value\"\n# <<< envforge <<<\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with bare assignment (no export keyword).
#[test]
fn test_roundtrip_bare_assignment() {
    let content = "VERSION=\"1.0.0\"\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with unquoted value.
#[test]
fn test_roundtrip_unquoted_value() {
    let content = "export PORT=3000\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with empty file.
#[test]
fn test_roundtrip_empty_file() {
    let sf = parse_shell_content("", Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with only blank lines.
#[test]
fn test_roundtrip_blank_lines() {
    let content = "\n\n\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with only comments.
#[test]
fn test_roundtrip_comments_only() {
    let content = "# only comments\n# no exports\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Round-trip with envforge tags.
#[test]
fn test_roundtrip_envforge_tags() {
    let content = "#[envforge:managed]\nFOO=\"bar\"\n# end managed\n";
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let output = serialize_shell_file(&sf);
    insta::assert_snapshot!(output);
}

/// Verify double round-trip produces identical output.
#[test]
fn test_double_roundtrip_is_stable() {
    let content = "export A=\"1\"\nexport B=\"2\"\n# comment\nexport C='3'\n";
    let sf1 = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let serialized1 = serialize_shell_file(&sf1);
    let sf2 = parse_shell_content(&serialized1, Path::new("/test/.zshrc")).unwrap();
    let serialized2 = serialize_shell_file(&sf2);
    assert_eq!(serialized1, serialized2, "double round-trip must be stable");
}
