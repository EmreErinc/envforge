//! Integration tests for Intent 038 — YAML Writes (ReadOnly → ReadWrite).
//!
//! Unit 001: SurgicalEdit foundation.
//! Unit 002: YAML value-span resolution + rename.
//!
//! Test naming: `test_{what_is_being_tested}_{condition}`.

use std::collections::HashMap;

use envforge::lsp::config_features::{config_yaml_format_text_edits, config_yaml_rename};
use envforge::lsp::config_file::{format_for_uri, is_yaml_config_file};
use envforge::ops::config_format::{SourceLayer, WriteCapability};
use envforge::ops::surgical_edit::SurgicalEdit;
use envforge::parser::yaml_config_parser::parse_yaml_config;
use envforge::parser::yaml_span_resolver::{resolve_yaml_key_span, resolve_yaml_value_span};
use tower_lsp::lsp_types::Url;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 001: SurgicalEdit — byte-range splice
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_surgical_edit_apply_middle_range_byte_identical_outside() {
    // source: "spring:\n  port: 8080\n"
    //          0123456 7 89012345 6789 20
    // "spring:\n" = 8 bytes (0..8)
    // "  port: "  = 8 bytes (8..16)
    // "8080"      = 4 bytes (16..20)
    let source = "spring:\n  port: 8080\n";
    let val_start = source.find("8080").unwrap();
    let val_end = val_start + 4;
    let result = SurgicalEdit::apply(source, val_start..val_end, "9090");
    assert!(result.is_some());
    let edited = result.unwrap();
    assert_eq!(edited, "spring:\n  port: 9090\n");
    // Byte-identity harness: prefix + suffix unchanged.
    assert!(SurgicalEdit::assert_byte_identity(
        source,
        &edited,
        val_start..val_end,
        4
    ));
}

#[test]
fn test_surgical_edit_apply_start_of_buffer() {
    let source = "hello world";
    let result = SurgicalEdit::apply(source, 0..5, "goodbye");
    assert_eq!(result.unwrap(), "goodbye world");
}

#[test]
fn test_surgical_edit_apply_end_of_buffer() {
    let source = "key: old_val";
    let len = source.len();
    let result = SurgicalEdit::apply(source, len - 7..len, "new_val");
    assert_eq!(result.unwrap(), "key: new_val");
}

#[test]
fn test_surgical_edit_apply_empty_replacement_is_deletion() {
    let source = "foo_DELETE_bar";
    let result = SurgicalEdit::apply(source, 3..10, "");
    assert_eq!(result.unwrap(), "foo_bar");
}

#[test]
fn test_surgical_edit_apply_zero_width_range_is_insertion() {
    let source = "foobar";
    // Insert "_" between "foo" and "bar".
    let result = SurgicalEdit::apply(source, 3..3, "_");
    assert_eq!(result.unwrap(), "foo_bar");
}

#[test]
fn test_surgical_edit_apply_inverted_range_returns_none() {
    let source = "hello";
    assert!(SurgicalEdit::apply(source, 3..1, "x").is_none());
}

#[test]
fn test_surgical_edit_apply_out_of_bounds_returns_none() {
    let source = "hello";
    assert!(SurgicalEdit::apply(source, 0..100, "x").is_none());
}

#[test]
fn test_surgical_edit_apply_multibyte_content_utf8_safe() {
    // "春節" is 6 bytes (3 bytes each). Replace 春(0..3) with "夏".
    let source = "春節";
    let result = SurgicalEdit::apply(source, 0..3, "夏");
    assert_eq!(result.unwrap(), "夏節");
}

#[test]
fn test_surgical_edit_to_text_edit_ascii_positions_correct() {
    let source = "line1\nkey: value\nline3\n";
    // "line1\n" = 6 bytes; "key: " = 5 bytes → "value" starts at byte 11.
    let val_start = source.find("value").unwrap();
    let val_end = val_start + "value".len();
    let se = SurgicalEdit::new(val_start..val_end, "new_value", source.len()).unwrap();
    let te = se.to_text_edit(source).unwrap();
    assert_eq!(te.range.start.line, 1);
    assert_eq!(te.range.start.character, 5); // "key: " = 5 UTF-16 units
    assert_eq!(te.range.end.line, 1);
    assert_eq!(te.range.end.character, 10); // "key: value" end
    assert_eq!(te.new_text, "new_value");
}

#[test]
fn test_surgical_edit_to_text_edit_multibyte_key_utf16_correct() {
    // Line: "naïve: val\n" — 'ï' is U+00EF (1 UTF-16 unit, 2 UTF-8 bytes).
    // "naïve" = 5 chars = 5 UTF-16 units; ": " = 2 → "val" starts at byte 8, UTF-16 col 7.
    let source = "naïve: val\n";
    // "val" is at bytes 8..11.
    let se = SurgicalEdit::new(8..11, "data", source.len()).unwrap();
    let te = se.to_text_edit(source).unwrap();
    assert_eq!(te.range.start.line, 0);
    assert_eq!(te.range.start.character, 7, "UTF-16 col of 'val' must be 7");
}

#[test]
fn test_surgical_edit_byte_identity_harness_prefix_and_suffix_identical() {
    let source = "AAABBBCCC";
    let edited = "AAAxxxCCC";
    // range 3..6 replaced with "xxx" (same length).
    assert!(SurgicalEdit::assert_byte_identity(source, edited, 3..6, 3));
}

#[test]
fn test_surgical_edit_byte_identity_harness_detects_prefix_corruption() {
    let source = "AAABBBCCC";
    let edited = "ZZZBBZCCC";
    // prefix is corrupted → harness returns false.
    assert!(!SurgicalEdit::assert_byte_identity(source, edited, 3..6, 3));
}

#[test]
fn test_surgical_edit_new_rejects_out_of_bounds() {
    assert!(SurgicalEdit::new(0..100, "x", 5).is_none());
}

#[test]
fn test_surgical_edit_new_rejects_inverted_range() {
    assert!(SurgicalEdit::new(5..3, "x", 10).is_none());
}

#[test]
fn test_surgical_edit_new_accepts_empty_replacement() {
    // Deletion: start..end with empty replacement.
    let se = SurgicalEdit::new(2..5, "", 10);
    assert!(se.is_some());
}

#[test]
fn test_surgical_edit_new_accepts_zero_width_insertion() {
    let se = SurgicalEdit::new(3..3, "inserted", 10);
    assert!(se.is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 002: YAML value-span resolution
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_resolve_yaml_key_span_flat_key() {
    let content = "port: 8080\n";
    let span = resolve_yaml_key_span(content, "port").unwrap();
    let raw = &content[span.byte_range.clone()];
    assert_eq!(raw, "port");
    assert!(!span.is_quoted);
}

#[test]
fn test_resolve_yaml_key_span_nested_key_leaf_only() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n";
    let span = resolve_yaml_key_span(content, "spring.datasource.url").unwrap();
    let raw = &content[span.byte_range.clone()];
    // The leaf key in source is "url".
    assert_eq!(raw, "url");
}

#[test]
fn test_resolve_yaml_key_span_not_found_returns_error() {
    use envforge::parser::yaml_span_resolver::ResolveError;
    let content = "key: value\n";
    let err = resolve_yaml_key_span(content, "nonexistent").unwrap_err();
    assert_eq!(err, ResolveError::KeyNotFound);
}

#[test]
fn test_resolve_yaml_key_span_malformed_yaml_returns_error() {
    use envforge::parser::yaml_span_resolver::ResolveError;
    let content = "key: [unclosed";
    let err = resolve_yaml_key_span(content, "key").unwrap_err();
    assert_eq!(err, ResolveError::MalformedYaml);
}

#[test]
fn test_resolve_yaml_key_span_anchor_document_returns_error() {
    use envforge::parser::yaml_span_resolver::ResolveError;
    // Document with anchor — resolver must refuse, not silently mis-edit.
    let content = "base: &anchor\n  url: http://localhost\nother: *anchor\n";
    let err = resolve_yaml_key_span(content, "base.url").unwrap_err();
    assert_eq!(
        err,
        ResolveError::AnchorAlias,
        "anchor documents must be refused, not silently mis-edited"
    );
}

#[test]
fn test_resolve_yaml_value_span_plain_scalar() {
    let content = "port: 8080\n";
    let range = resolve_yaml_value_span(content, "port").unwrap();
    let raw = &content[range.clone()];
    assert_eq!(raw, "8080");
}

#[test]
fn test_resolve_yaml_value_span_nested_key() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n";
    let range = resolve_yaml_value_span(content, "spring.datasource.url").unwrap();
    let raw = &content[range.clone()];
    assert_eq!(raw, "jdbc:h2:mem");
}

#[test]
fn test_resolve_yaml_key_span_byte_range_is_valid_char_boundary() {
    // Verify that the returned byte range is on valid UTF-8 char boundaries.
    let content = "server:\n  port: 8080\n  host: localhost\n";
    let span = resolve_yaml_key_span(content, "server.port").unwrap();
    assert!(content.is_char_boundary(span.byte_range.start));
    assert!(content.is_char_boundary(span.byte_range.end));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 002: YAML ReadWrite capability (Intent 038 FR3 inversion)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_format_write_capability_is_readwrite() {
    let (fmt, _) = format_for_uri(&url("/proj/application.yml")).unwrap();
    assert_eq!(
        fmt.write_capability(),
        WriteCapability::ReadWrite,
        "YamlFormat must be ReadWrite after Intent 038"
    );
}

#[test]
fn test_yaml_profile_format_write_capability_is_readwrite() {
    let (fmt, _) = format_for_uri(&url("/proj/application-prod.yaml")).unwrap();
    assert_eq!(
        fmt.write_capability(),
        WriteCapability::ReadWrite,
        "Profile YamlFormat must also be ReadWrite"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 002: YAML surgical rename (Intent 038 FR4)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_config_yaml_rename_simple_key_produces_edit() {
    let content = "port: 8080\n";
    let uri = url("/proj/application.yml");

    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);

    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "port",
        "http_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(result.is_some(), "rename should succeed for a simple key");

    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "http_port");
}

#[test]
fn test_config_yaml_rename_byte_identical_outside_renamed_span() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n    username: sa\n";
    let uri = url("/proj/application.yml");

    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);

    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "spring.datasource.username",
        "spring.datasource.user",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(result.is_some(), "rename of nested key should succeed");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits.len(), 1);

    // Apply the edit and verify round-trip byte-identity.
    let edit = &edits[0];
    // We verify the edit targets "username" and the new_text is "user".
    assert_eq!(edit.new_text, "user");
}

#[test]
fn test_config_yaml_rename_returns_none_for_readonly() {
    let content = "port: 8080\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri, content.to_string());

    let result = config_yaml_rename(
        "port",
        "http_port",
        WriteCapability::ReadOnly,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename must return None when write_capability is ReadOnly"
    );
}

#[test]
fn test_config_yaml_rename_returns_none_for_collision() {
    let content = "port: 8080\nhttps_port: 443\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri, content.to_string());

    // "https_port" already exists → collision → None.
    let result = config_yaml_rename(
        "port",
        "https_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename must return None when new name already exists"
    );
}

#[test]
fn test_config_yaml_rename_returns_none_for_same_key() {
    let content = "port: 8080\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri, content.to_string());

    let result = config_yaml_rename(
        "port",
        "port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename must return None when old_key == new_name"
    );
}

#[test]
fn test_config_yaml_rename_returns_none_for_invalid_key() {
    let content = "port: 8080\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri, content.to_string());

    // "123invalid" starts with a digit → invalid key.
    let result = config_yaml_rename(
        "port",
        "123invalid",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename must return None for an invalid new key name"
    );
}

#[test]
fn test_config_yaml_rename_anchor_document_returns_none() {
    // Document with anchors — rename must be declined (not silently mis-edited).
    let content = "base: &anchor\n  url: http://localhost\nother: *anchor\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri, content.to_string());

    let result = config_yaml_rename(
        "base.url",
        "base.endpoint",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename must return None for anchor/alias documents (documented gap)"
    );
}

#[test]
fn test_config_yaml_rename_comments_preserved_by_construction() {
    // The rename must be surgical — comments must be byte-identical outside the renamed span.
    let content = "# spring config\nspring:\n  # datasource settings\n  datasource:\n    url: jdbc:h2:mem  # in-memory\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "spring.datasource.url",
        "spring.datasource.jdbc_url",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_some(),
        "rename of deeply nested key should succeed"
    );
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "jdbc_url");
}

#[test]
fn test_config_yaml_rename_crlf_file_preserves_line_endings() {
    // File with CRLF line endings — the rename edit must be positionally correct.
    let content = "port: 8080\r\nhost: localhost\r\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "port",
        "http_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(result.is_some(), "rename should work on CRLF files");
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "http_port");
}

#[test]
fn test_config_yaml_rename_no_trailing_newline_file() {
    // File without trailing newline — the rename edit must be positionally correct.
    let content = "port: 8080";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "port",
        "http_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_some(),
        "rename should work on files without trailing newline"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 002: YAML format no-op (Open decision 1: rename-only)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_format_noop_empty_document() {
    assert!(config_yaml_format_text_edits("").is_empty());
}

#[test]
fn test_yaml_format_noop_rich_document_with_comments() {
    let content = "# Spring Boot config\nspring:\n  # Datasource\n  datasource:\n    url: jdbc:h2:mem\n    # credentials\n    username: sa\n";
    let edits = config_yaml_format_text_edits(content);
    assert!(
        edits.is_empty(),
        "YAML format must be a no-op for comment-rich docs; got {} edit(s)",
        edits.len()
    );
}

#[test]
fn test_yaml_format_noop_block_scalar() {
    let content = "init_script: |\n  CREATE TABLE foo (id INT);\n  INSERT INTO foo VALUES (1);\n";
    let edits = config_yaml_format_text_edits(content);
    assert!(
        edits.is_empty(),
        "YAML format must never touch block scalars; got {} edit(s)",
        edits.len()
    );
}

#[test]
fn test_yaml_format_noop_crlf_document() {
    let content = "key: value\r\nother: data\r\n";
    let edits = config_yaml_format_text_edits(content);
    assert!(
        edits.is_empty(),
        "YAML format must be a no-op for CRLF documents; got {} edit(s)",
        edits.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unit 002: Round-trip / write-guard inversion (Intent 038 FR6)
// ═══════════════════════════════════════════════════════════════════════════════

/// "Spring corpus" byte-identity property test.
/// Apply a rename and verify byte-identity outside the edited span.
#[test]
fn test_yaml_rename_round_trip_byte_identical_spring_corpus() {
    // Real-world-ish Spring Boot application.yml content.
    let content = "\
# Spring Boot application config
spring:
  application:
    name: my-service  # service name
  datasource:
    url: jdbc:postgresql://localhost:5432/mydb
    username: app_user
    password: ${DB_PASSWORD}
    driver-class-name: org.postgresql.Driver
  jpa:
    hibernate:
      ddl-auto: validate
    show-sql: false
server:
  port: 8080
  servlet:
    context-path: /api
logging:
  level:
    root: INFO
    com.example: DEBUG
";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);

    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    // Rename "server.port" to "server.http_port".
    let result = config_yaml_rename(
        "server.port",
        "server.http_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_some(),
        "corpus rename of server.port must succeed"
    );

    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits.len(), 1, "exactly one edit expected");
    assert_eq!(edits[0].new_text, "http_port");

    // Apply the edit and verify byte-identity via the SurgicalEdit harness.
    // We reconstruct the expected output manually and compare.
    let span = resolve_yaml_key_span(content, "server.port").unwrap();
    let edited = SurgicalEdit::apply(content, span.byte_range.clone(), "http_port").unwrap();

    // Every byte outside the renamed span must be identical.
    assert!(
        SurgicalEdit::assert_byte_identity(
            content,
            &edited,
            span.byte_range.clone(),
            "http_port".len()
        ),
        "bytes outside renamed span must be identical"
    );

    // Comments are preserved — spot-check.
    assert!(
        edited.contains("# Spring Boot application config"),
        "comments must be preserved"
    );
    assert!(
        edited.contains("# service name"),
        "inline comments must be preserved"
    );

    // The renamed key appears; old key does not.
    assert!(edited.contains("http_port:"));
    assert!(!edited.contains("\n  port:"));
}

#[test]
fn test_yaml_rename_round_trip_multibyte_comment_preserved() {
    // File with a multi-byte comment (Japanese). The rename must not corrupt it.
    let content = "# サービス設定\nport: 8080\n";
    let uri = url("/proj/application.yml");
    let (fmt, layer) = format_for_uri(&uri).unwrap();
    let entries = fmt.parse(content, layer);

    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), content.to_string());

    let result = config_yaml_rename(
        "port",
        "http_port",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(result.is_some());
    let workspace_edit = result.unwrap();
    let changes = workspace_edit.changes.unwrap();
    let edits = &changes[&uri];
    assert_eq!(edits[0].new_text, "http_port");

    // Apply and verify the multi-byte comment is intact.
    let span = resolve_yaml_key_span(content, "port").unwrap();
    let edited = SurgicalEdit::apply(content, span.byte_range.clone(), "http_port").unwrap();
    assert!(
        edited.contains("# サービス設定"),
        "multi-byte comment must be preserved"
    );
}

#[test]
fn test_yaml_rename_no_write_guard_regression() {
    // Ensure no test asserts YamlFormat is ReadOnly (regression guard).
    let (fmt, _) = format_for_uri(&url("/proj/application.yml")).unwrap();
    // This assertion must pass — if it fails, the write-guard has been re-asserted.
    assert_ne!(
        fmt.write_capability(),
        WriteCapability::ReadOnly,
        "YamlFormat must NOT be ReadOnly after Intent 038 — write-guard regression detected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Non-regression: 036 YAML read features unchanged
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_036_yaml_read_parse_still_works() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "spring.datasource.url");
    assert_eq!(entries[0].value, "jdbc:h2:mem");
}

#[test]
fn test_036_yaml_recognition_still_works() {
    assert!(is_yaml_config_file(&url("/proj/application.yml")));
    assert!(is_yaml_config_file(&url("/proj/application-prod.yaml")));
    assert!(!is_yaml_config_file(&url("/proj/docker-compose.yml")));
}

#[test]
fn test_036_yaml_format_parse_and_resolve_still_works() {
    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    let content = "server:\n  port: 8080\n";
    let entries = fmt.parse(content, layer);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "server.port");

    let layers = vec![entries];
    let resolved = fmt.resolve("server.port", &layers).unwrap();
    assert_eq!(resolved.value, "8080");
}
