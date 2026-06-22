//! Tests for intent 037 — TOML config intelligence.
//!
//! Stories covered:
//! - 001: recognition (`is_toml_config_file`, `is_config_format_file`, scoped predicates)
//! - 002: parse → entry model (dotted keys, UTF-16 positions, round-trip)
//! - 003: read features (hover, completion, goto, refs, semantic tokens)
//! - 004: diagnostics (duplicate, unknown-key-vs-schema, type-mismatch-vs-schema)
//! - 005: write features (format round-trip, rename)
//!
//! FR25 parity assertions: existing properties/YAML handlers are NOT modified.

use tower_lsp::lsp_types::Url;

use envforge::lsp::config_file::{
    format_for_uri, is_config_format_file, is_jvm_config_file, is_toml_config_file,
    is_yaml_config_file,
};
use envforge::ops::config_format::{SourceLayer, WriteCapability};
use envforge::parser::toml_config_parser::{parse_toml_config, toml_duplicate_key_diagnostics};

// ── Story 001: Recognition ────────────────────────────────────────────────────

fn url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).unwrap()
}

#[test]
fn test_is_toml_config_file_cargo_toml_recognized() {
    assert!(is_toml_config_file(&url("/workspace/Cargo.toml")));
}

#[test]
fn test_is_toml_config_file_pyproject_toml_recognized() {
    assert!(is_toml_config_file(&url("/workspace/pyproject.toml")));
}

#[test]
fn test_is_toml_config_file_config_toml_recognized() {
    assert!(is_toml_config_file(&url("/workspace/config.toml")));
}

#[test]
fn test_is_toml_config_file_cargo_dir_config_toml_recognized() {
    // .cargo/config.toml — recognized via the `config.toml` basename rule.
    assert!(is_toml_config_file(&url("/home/user/.cargo/config.toml")));
}

#[test]
fn test_is_toml_config_file_foo_toml_not_recognized() {
    assert!(!is_toml_config_file(&url("/workspace/foo.toml")));
}

#[test]
fn test_is_toml_config_file_gemfile_toml_not_recognized() {
    assert!(!is_toml_config_file(&url("/workspace/Gemfile.toml")));
}

#[test]
fn test_is_toml_config_file_cargo_lock_not_recognized() {
    assert!(!is_toml_config_file(&url("/workspace/Cargo.lock")));
}

#[test]
fn test_is_toml_config_file_env_schema_toml_not_recognized() {
    assert!(!is_toml_config_file(&url("/workspace/.env.schema.toml")));
}

#[test]
fn test_is_config_format_file_includes_toml() {
    assert!(is_config_format_file(&url("/workspace/Cargo.toml")));
    assert!(is_config_format_file(&url("/workspace/pyproject.toml")));
    assert!(is_config_format_file(&url("/workspace/config.toml")));
}

#[test]
fn test_is_config_format_file_excludes_non_canonical_toml() {
    assert!(!is_config_format_file(&url("/workspace/foo.toml")));
    assert!(!is_config_format_file(&url("/workspace/bar.toml")));
}

/// FR25 parity: adding TOML must not change recognition of existing file types.
#[test]
fn test_fr25_existing_predicates_unaffected_by_toml_addition() {
    // JVM properties still recognized.
    assert!(is_jvm_config_file(&url(
        "/workspace/application.properties"
    )));
    assert!(is_jvm_config_file(&url(
        "/workspace/application-prod.properties"
    )));
    assert!(is_jvm_config_file(&url(
        "/workspace/microprofile-config.properties"
    )));

    // YAML still recognized.
    assert!(is_yaml_config_file(&url("/workspace/application.yml")));
    assert!(is_yaml_config_file(&url("/workspace/application.yaml")));
    assert!(is_yaml_config_file(&url(
        "/workspace/application-staging.yml"
    )));

    // Plain .env stays on the existing handler (not in config_format_file).
    assert!(!is_config_format_file(&url("/workspace/.env")));

    // Non-canonical TOML excluded.
    assert!(!is_toml_config_file(&url("/workspace/random.toml")));
}

#[test]
fn test_format_for_uri_toml_returns_toml_format_readwrite() {
    let (fmt, layer) = format_for_uri(&url("/workspace/Cargo.toml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_format_for_uri_toml_pyproject() {
    let (fmt, layer) = format_for_uri(&url("/workspace/pyproject.toml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::Base);
}

// ── Story 002: Parse → entry model ───────────────────────────────────────────

#[test]
fn test_parse_toml_config_simple_table() {
    let toml = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
    let (entries, diags, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty(), "no diagnostics on valid TOML");
    assert!(
        entries.iter().any(|e| e.key == "package.name"),
        "package.name must be present"
    );
    assert!(
        entries.iter().any(|e| e.key == "package.version"),
        "package.version must be present"
    );
    // Round-trip byte-identity.
    assert_eq!(doc.to_string(), toml, "round-trip must be byte-identical");
}

#[test]
fn test_parse_toml_config_nested_tables() {
    let toml =
        "[profile.release]\npanic = \"abort\"\nopt-level = 3\n\n[profile.dev]\nopt-level = 0\n";
    let (entries, diags, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty());
    assert!(entries.iter().any(|e| e.key == "profile.release.panic"));
    assert!(entries.iter().any(|e| e.key == "profile.release.opt-level"));
    assert!(entries.iter().any(|e| e.key == "profile.dev.opt-level"));
    assert_eq!(doc.to_string(), toml);
}

#[test]
fn test_parse_toml_config_top_level_key() {
    let toml = "rust-version = \"1.75\"\n";
    let (entries, diags, _doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty());
    assert!(entries.iter().any(|e| e.key == "rust-version"));
}

#[test]
fn test_parse_toml_config_round_trip_with_comments() {
    // Rich fixture: comments, blank lines, nested tables, no-final-newline.
    let toml = r#"# This is a comment
[package]
name = "foo" # inline comment

[dependencies]
serde = "1.0"
# Another comment
tokio = { version = "1", features = ["rt"] }"#;
    let (entries, diags, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty());
    // Round-trip: toml_edit normalizes to add a trailing newline per TOML spec.
    // The doc content equals toml or toml + "\n" (toml_edit adds \n if missing).
    let roundtrip = doc.to_string();
    let normalized_input = if toml.ends_with('\n') {
        toml.to_string()
    } else {
        format!("{}\n", toml)
    };
    assert_eq!(
        roundtrip, normalized_input,
        "round-trip must be byte-identical (toml_edit normalizes trailing newline per TOML spec)"
    );
    // Entries are populated.
    assert!(entries.iter().any(|e| e.key == "package.name"));
    assert!(entries.iter().any(|e| e.key == "dependencies.serde"));
    assert!(entries.iter().any(|e| e.key == "dependencies.tokio"));
}

#[test]
fn test_parse_toml_config_round_trip_arrays_of_tables() {
    let toml = "[[bin]]\nname = \"envforge\"\npath = \"src/main.rs\"\n\n[[bin]]\nname = \"helper\"\npath = \"src/helper.rs\"\n";
    let (entries, diags, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty());
    // Both bin.name entries are present.
    let names: Vec<&str> = entries
        .iter()
        .filter(|e| e.key == "bin.name")
        .map(|e| e.value.as_str())
        .collect();
    assert_eq!(names.len(), 2, "two [[bin]] name entries");
    assert_eq!(doc.to_string(), toml);
}

#[test]
fn test_parse_toml_config_invalid_returns_error() {
    let bad_toml = "key = [unclosed\n";
    let result = parse_toml_config(bad_toml, SourceLayer::Base);
    assert!(result.is_err(), "malformed TOML must return Err");
    let err = result.unwrap_err();
    assert!(!err.message.is_empty());
}

#[test]
fn test_parse_toml_config_empty_content() {
    let (entries, diags, doc) = parse_toml_config("", SourceLayer::Base).unwrap();
    assert!(entries.is_empty());
    assert!(diags.is_empty());
    assert_eq!(doc.to_string(), "");
}

#[test]
fn test_parse_toml_config_utf8_value_positions_correct() {
    // Non-ASCII value: position should be UTF-16-correct.
    let toml = "[package]\nname = \"café\"\n";
    let (entries, diags, _doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(diags.is_empty());
    let name_entry = entries.iter().find(|e| e.key == "package.name").unwrap();
    // Value string should be the unquoted string (no surrounding quotes).
    assert_eq!(name_entry.value, "café");
    // The value range end character must be >= start (UTF-16 correctness).
    assert!(name_entry.value_range.end.character >= name_entry.value_range.start.character);
}

#[test]
fn test_parse_toml_config_round_trip_no_final_newline() {
    // toml_edit normalizes TOML files to end with a newline (TOML spec requirement).
    // Content without final newline gets one added on round-trip via doc.to_string().
    let toml = "[package]\nname = \"no-newline\"";
    let (entries, _diags, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(entries.iter().any(|e| e.key == "package.name"));
    // toml_edit adds trailing newline per TOML spec; content is otherwise preserved.
    let roundtrip = doc.to_string();
    assert!(
        roundtrip == toml || roundtrip == format!("{}\n", toml),
        "round-trip must match input or input + trailing newline: got {:?}",
        roundtrip
    );
}

#[test]
fn test_parse_toml_config_source_layer_propagated() {
    let toml = "[package]\nname = \"foo\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    for e in &entries {
        assert_eq!(e.source_layer, SourceLayer::Base);
    }
}

// ── Story 003: Read features (hover, semantic tokens) ────────────────────────

use envforge::lsp::config_features::{
    config_hover, config_semantic_tokens, config_toml_diagnostics,
};
use tower_lsp::lsp_types::Position;

#[test]
fn test_toml_hover_finds_entry_at_cursor() {
    let toml = "[package]\nname = \"my-crate\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    // Cursor on line 1 (name = "my-crate"), somewhere in the key range.
    let position = Position {
        line: 1,
        character: 2,
    };
    let hover = config_hover(position, &entries, &[], None, None);
    assert!(hover.is_some(), "hover must return Some for cursor on key");
    if let Some(h) = hover {
        let tower_lsp::lsp_types::HoverContents::Markup(mc) = h.contents else {
            panic!("expected Markup hover");
        };
        assert!(mc.value.contains("package.name"), "hover must show the key");
        assert!(mc.value.contains("my-crate"), "hover must show the value");
    }
}

#[test]
fn test_toml_hover_returns_none_for_empty_line() {
    let toml = "[package]\n\nname = \"foo\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let position = Position {
        line: 1,
        character: 0,
    };
    // Line 1 is blank — no entry at that position.
    let hover = config_hover(position, &entries, &[], None, None);
    assert!(hover.is_none());
}

#[test]
fn test_toml_semantic_tokens_non_empty_for_entries() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let tokens = config_semantic_tokens(&entries, None);
    // Should have at least one token for the non-empty entries.
    assert!(
        !tokens.data.is_empty(),
        "semantic tokens must be non-empty for parsed TOML entries"
    );
}

// ── Story 004: Diagnostics ────────────────────────────────────────────────────

use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_toml_diagnostics_valid_file_no_diagnostics() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let diags = config_toml_diagnostics(toml, SourceLayer::Base, None);
    assert!(
        diags.is_empty(),
        "valid TOML with no schema must emit no diagnostics"
    );
}

#[test]
fn test_toml_diagnostics_malformed_toml_error_diagnostic() {
    let bad = "key = [unclosed\n";
    let diags = config_toml_diagnostics(bad, SourceLayer::Base, None);
    assert!(
        !diags.is_empty(),
        "malformed TOML must produce at least one diagnostic"
    );
    assert_eq!(
        diags[0].severity,
        Some(DiagnosticSeverity::ERROR),
        "malformed TOML diagnostic must be ERROR severity"
    );
    assert!(
        diags[0].message.contains("TOML syntax error"),
        "diagnostic message must mention TOML syntax error"
    );
}

#[test]
fn test_toml_diagnostics_no_panic_on_empty_input() {
    // Must not panic; may produce zero or one diagnostic.
    let _ = config_toml_diagnostics("", SourceLayer::Base, None);
}

#[test]
fn test_toml_duplicate_key_diagnostic_detected() {
    // toml_edit rejects duplicate keys at parse time, so we test the
    // secondary duplicate-key pass on the entry model directly.
    use envforge::ops::config_format::ConfigEntry;
    use tower_lsp::lsp_types::{Position, Range as LspRange};

    let make_entry = |key: &str, line: u32| -> ConfigEntry {
        let pos = Position { line, character: 0 };
        ConfigEntry {
            key: key.to_string(),
            value: "v".to_string(),
            key_range: LspRange {
                start: pos,
                end: pos,
            },
            value_range: LspRange {
                start: pos,
                end: pos,
            },
            line,
            source_layer: SourceLayer::Base,
        }
    };

    let entries = vec![
        make_entry("package.name", 1),
        make_entry("package.name", 3), // duplicate
    ];
    let diags = toml_duplicate_key_diagnostics(&entries);
    assert_eq!(diags.len(), 1, "one duplicate-key diagnostic expected");
    assert!(diags[0].message.contains("Duplicate key"));
}

#[test]
fn test_toml_diagnostics_different_table_keys_not_duplicate() {
    // [a].x and [b].x have the same leaf but different dotted paths — not duplicate.
    let toml = "[a]\nx = 1\n\n[b]\nx = 2\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let diags = toml_duplicate_key_diagnostics(&entries);
    assert!(
        diags.is_empty(),
        "[a].x and [b].x are not duplicates: different dotted paths"
    );
}

#[test]
fn test_toml_diagnostics_arrays_of_tables_not_duplicate() {
    // [[bin]] repeated is legal — each element has the same path but is a distinct entry.
    let toml = "[[bin]]\nname = \"foo\"\n\n[[bin]]\nname = \"bar\"\n";
    let (entries, diags, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    assert!(
        diags.is_empty(),
        "[[bin]] repeated must not error at parse time"
    );
    // The entry model has two bin.name entries (different values); dup check
    // fires because they share the same dotted key. This is acceptable since
    // arrays-of-tables produce same-key entries by design.
    let _ = toml_duplicate_key_diagnostics(&entries); // must not panic
}

// ── Story 005: Write features (format round-trip, rename) ────────────────────

use envforge::lsp::config_features::{
    config_toml_format_text_edits, config_toml_rename, is_valid_toml_key_segment,
};
use std::collections::HashMap;

#[test]
fn test_toml_format_text_edits_valid_toml_is_idempotent() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let edits = config_toml_format_text_edits(toml);
    // toml_edit round-trip is byte-identical for well-formed TOML.
    assert!(edits.is_empty(), "format must be idempotent on valid TOML");
}

#[test]
fn test_toml_format_text_edits_malformed_returns_empty() {
    let bad = "key = [unclosed\n";
    let edits = config_toml_format_text_edits(bad);
    // Malformed TOML → leave unchanged (no edits, no panic).
    assert!(
        edits.is_empty(),
        "malformed TOML must produce no format edits (leave unchanged)"
    );
}

#[test]
fn test_toml_format_text_edits_rich_fixture_idempotent() {
    // Rich fixture: comments, blank lines, nested tables, arrays-of-tables,
    // inline tables, quoted keys, WITH final newline (so toml_edit doesn't mutate it).
    let toml = "# Top comment\n[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n\n# Dependencies section\n[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\ntokio = \"1.0\"\n\n[[bin]]\nname = \"main\"\npath = \"src/main.rs\"\n";
    let edits = config_toml_format_text_edits(toml);
    assert!(
        edits.is_empty(),
        "format must be idempotent on a rich valid TOML fixture with final newline"
    );
}

#[test]
fn test_toml_rename_rejects_readonly_capability() {
    let result = config_toml_rename(
        "old",
        "new",
        WriteCapability::ReadOnly,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(
        result.is_none(),
        "rename must be rejected for ReadOnly capability"
    );
}

#[test]
fn test_toml_rename_rejects_same_name() {
    let result = config_toml_rename(
        "name",
        "name",
        WriteCapability::ReadWrite,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(result.is_none(), "rename to same name must be rejected");
}

#[test]
fn test_toml_rename_rejects_invalid_key_segment() {
    let result = config_toml_rename(
        "name",
        "has.dot",
        WriteCapability::ReadWrite,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(
        result.is_none(),
        "rename with dot in segment must be rejected by is_valid_toml_key_segment"
    );
}

#[test]
fn test_toml_rename_produces_workspace_edit_with_content() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();

    let uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), toml.to_string());

    let result = config_toml_rename(
        "package.name",
        "pkg-name",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_some(),
        "valid rename must produce Some(WorkspaceEdit)"
    );
    let edit = result.unwrap();
    let changes = edit.changes.unwrap();
    assert!(changes.contains_key(&uri));
    let file_edits = &changes[&uri];
    assert!(!file_edits.is_empty());
    let new_text = &file_edits[0].new_text;
    assert!(
        new_text.contains("pkg-name"),
        "renamed document must contain the new key name"
    );
    // Byte-except-for-change: comments and structure must be preserved.
    assert!(
        new_text.contains("\"foo\""),
        "rename must preserve the value"
    );
}

#[test]
fn test_toml_rename_rejects_collision_with_existing_key() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();

    let uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);

    // Try to rename "package.name" to "package.version" — collision!
    let result = config_toml_rename(
        "package.name",
        "package.version",
        WriteCapability::ReadWrite,
        &open_docs,
        &HashMap::new(),
    );
    assert!(
        result.is_none(),
        "rename must be rejected when new name collides with existing key"
    );
}

// ── is_valid_toml_key_segment ─────────────────────────────────────────────────

#[test]
fn test_is_valid_toml_key_segment_valid_names() {
    assert!(is_valid_toml_key_segment("name"));
    assert!(is_valid_toml_key_segment("opt-level"));
    assert!(is_valid_toml_key_segment("rust_version"));
    assert!(is_valid_toml_key_segment("abc123"));
    assert!(is_valid_toml_key_segment("123")); // TOML allows digit-start
}

#[test]
fn test_is_valid_toml_key_segment_rejects_empty() {
    assert!(!is_valid_toml_key_segment(""));
}

#[test]
fn test_is_valid_toml_key_segment_rejects_dots() {
    // Dots indicate a path, not a segment.
    assert!(!is_valid_toml_key_segment("a.b"));
}

#[test]
fn test_is_valid_toml_key_segment_rejects_spaces() {
    assert!(!is_valid_toml_key_segment("key name"));
}

// ── BUG regression tests ─────────────────────────────────────────────────────

// BUG-1: CRLF line endings must be preserved by format.
#[test]
fn test_bug1_format_preserves_crlf_line_endings() {
    let toml = "[package]\r\nname = \"foo\"\r\nversion = \"0.1.0\"\r\n";
    let edits = config_toml_format_text_edits(toml);
    // Already-canonical CRLF file must produce zero edits.
    assert!(
        edits.is_empty(),
        "format must produce no edits for already-canonical CRLF file; got: {:?}",
        edits
    );
}

// BUG-1 + BUG-2: CRLF file without trailing newline must roundtrip exactly.
#[test]
fn test_bug1_bug2_crlf_no_final_newline_format_is_no_op() {
    let toml = "[package]\r\nname = \"no-newline\"";
    let edits = config_toml_format_text_edits(toml);
    assert!(
        edits.is_empty(),
        "CRLF no-final-newline file must produce no format edits; got: {:?}",
        edits
    );
}

// BUG-2: file without trailing newline must produce no format edits.
#[test]
fn test_bug2_no_final_newline_format_produces_no_edits() {
    let toml = "[package]\nname = \"no-newline\"";
    let edits = config_toml_format_text_edits(toml);
    assert!(
        edits.is_empty(),
        "no-final-newline file must produce no format edits; got: {:?}",
        edits
    );
}

// BUG-3: rename on a no-final-newline file must not add a trailing newline.
#[test]
fn test_bug3_rename_preserves_no_final_newline() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\""; // no trailing newline
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();

    let uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), toml.to_string());

    let result = config_toml_rename(
        "package.name",
        "pkg-name",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    let edit = result.expect("rename must succeed");
    let changes = edit.changes.unwrap();
    let new_text = &changes[&uri][0].new_text;
    assert!(
        !new_text.ends_with('\n'),
        "rename must not add trailing newline when original had none; got: {:?}",
        new_text
    );
    assert!(
        new_text.contains("pkg-name"),
        "rename must contain new key name"
    );
}

// BUG-3: rename on a CRLF file must preserve CRLF.
#[test]
fn test_bug3_rename_preserves_crlf_line_endings() {
    let toml = "[package]\r\nname = \"foo\"\r\nversion = \"0.1.0\"\r\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();

    let uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), toml.to_string());

    let result = config_toml_rename(
        "package.name",
        "pkg-name",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    let edit = result.expect("rename must succeed on CRLF file");
    let changes = edit.changes.unwrap();
    let new_text = &changes[&uri][0].new_text;
    assert!(
        new_text.contains("\r\n"),
        "rename must preserve CRLF line endings; got: {:?}",
        new_text
    );
    assert!(
        !new_text.contains("\r\r\n"),
        "rename must not double-convert CRLF; got: {:?}",
        new_text
    );
}

// BUG-4: duplicate-position bug — entries sharing leaf+different tables.
// `[a]\nx=1\n[b]\nx=1` must give b.x line 4 (0-based: line 3), not line 1.
#[test]
fn test_bug4_duplicate_leaf_different_tables_correct_position() {
    let toml = "[a]\nx = 1\n\n[b]\nx = 1\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let b_x = entries
        .iter()
        .find(|e| e.key == "b.x")
        .expect("b.x must be present");
    // [b] is on line 3 (0-based), x = 1 is on line 4 (0-based).
    assert_eq!(
        b_x.line, 4,
        "b.x must be on line 4 (0-based); got line {}",
        b_x.line
    );
}

// BUG-5: collision check must compare sibling scope, not full dotted key.
// Renaming `package.name` → `version` when `package.version` exists must be rejected.
#[test]
fn test_bug5_rename_collision_sibling_scope_rejected() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();

    let uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let mut open_docs = HashMap::new();
    open_docs.insert(uri.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri.clone(), toml.to_string());

    // Rename package.name → version: should collide with package.version.
    let result = config_toml_rename(
        "package.name",
        "version",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(
        result.is_none(),
        "rename package.name→version must be rejected: package.version already exists"
    );
}

// BUG-7: dotted top-level key assignment `a.b = 1` must produce a non-zero range.
#[test]
fn test_bug7_dotted_top_level_key_gets_nonzero_range() {
    let toml = "a.b = 1\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let ab = entries
        .iter()
        .find(|e| e.key == "a.b")
        .expect("a.b must be parsed");
    assert!(
        ab.key_range.end.character > ab.key_range.start.character,
        "a.b key range must be non-zero; got {:?}",
        ab.key_range
    );
}

// BUG-7: `workspace.resolver = "2"` must produce a non-zero key range.
#[test]
fn test_bug7_workspace_resolver_dotted_key_gets_nonzero_range() {
    let toml = "workspace.resolver = \"2\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let entry = entries
        .iter()
        .find(|e| e.key == "workspace.resolver")
        .expect("workspace.resolver must be parsed");
    assert!(
        entry.key_range.end.character > entry.key_range.start.character,
        "workspace.resolver key range must be non-zero; got {:?}",
        entry.key_range
    );
}

// BUG-8: two [[bin]] sections with same keys must emit zero duplicate diagnostics.
#[test]
fn test_bug8_arrays_of_tables_two_bin_zero_duplicate_diagnostics() {
    let toml =
        "[[bin]]\nname = \"foo\"\npath = \"src/foo.rs\"\n\n[[bin]]\nname = \"bar\"\npath = \"src/bar.rs\"\n";
    let diags = config_toml_diagnostics(toml, SourceLayer::Base, None);
    let dup_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("Duplicate"))
        .collect();
    assert!(
        dup_diags.is_empty(),
        "two [[bin]] sections must produce zero duplicate diagnostics; got: {:?}",
        dup_diags
    );
}

// BUG-9: parse-error range end must be a real column, not u32::MAX.
#[test]
fn test_bug9_parse_error_range_end_is_real_column_not_max() {
    let bad = "key = [unclosed\n";
    let diags = config_toml_diagnostics(bad, SourceLayer::Base, None);
    assert!(!diags.is_empty(), "must have at least one diagnostic");
    for d in &diags {
        assert!(
            d.range.end.character != u32::MAX,
            "parse-error range end must not be u32::MAX; got {:?}",
            d.range
        );
    }
}

// TEST HONESTY FIX: round-trip no-final-newline must assert byte-identity.
#[test]
fn test_toml_round_trip_no_final_newline_honest() {
    let toml = "[package]\nname = \"no-newline\"";
    let (_, _, doc) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    // toml_edit.to_string() adds '\n'; that is the raw parser round-trip.
    // But config_toml_format_text_edits must produce NO edit (BUG-2 fix).
    let edits = config_toml_format_text_edits(toml);
    assert!(
        edits.is_empty(),
        "format must produce no edits for no-final-newline file (BUG-2 regression)"
    );
    // The raw doc.to_string() may add '\n' — that is acceptable toml_edit
    // behavior, but the LSP format layer must strip it back.
    let raw = doc.to_string();
    // Acceptable: raw equals toml with '\n' appended.
    assert!(
        raw == toml || raw == format!("{}\n", toml),
        "raw round-trip must be input or input+newline"
    );
}

// TEST HONESTY FIX: arrays-of-tables duplicate check must assert ZERO, not `let _ =`.
#[test]
fn test_toml_arrays_of_tables_not_duplicate_honest() {
    use envforge::parser::toml_config_parser::toml_duplicate_key_diagnostics_aot;
    let toml = "[[bin]]\nname = \"foo\"\n\n[[bin]]\nname = \"bar\"\n";
    let (entries, aot_flags, _, _) =
        envforge::parser::toml_config_parser::parse_toml_config_with_aot_flags(
            toml,
            SourceLayer::Base,
        )
        .unwrap();
    let diags = toml_duplicate_key_diagnostics_aot(&entries, &aot_flags);
    assert!(
        diags.is_empty(),
        "[[bin]] repeated must emit ZERO duplicate diagnostics with AoT-aware check; got: {:?}",
        diags
    );
}

// TEST HONESTY FIX: UTF-8 value positions must assert exact UTF-16 columns.
#[test]
fn test_toml_utf8_value_positions_exact_utf16_columns() {
    // "café" = 4 chars, but 'é' is U+00E9 (1 UTF-16 unit), so UTF-16 len = 4.
    let toml = "[package]\nname = \"café\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let entry = entries
        .iter()
        .find(|e| e.key == "package.name")
        .expect("package.name must be present");
    assert_eq!(entry.value, "café");
    // Value starts after `"` at column 8 (0-based): `name = "` = 8 chars (all ASCII).
    // UTF-16 start = 8, UTF-16 end = 8 + 4 = 12.
    assert_eq!(
        entry.value_range.start.character, 8,
        "value start must be column 8 (after opening quote)"
    );
    assert_eq!(
        entry.value_range.end.character, 12,
        "value end must be column 12 (start + 4 UTF-16 units for 'café')"
    );
}

// TEST HONESTY FIX: semantic tokens on non-empty TOML must assert real content.
#[test]
fn test_toml_semantic_tokens_assert_real_content() {
    let toml = "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n";
    let (entries, _, _) = parse_toml_config(toml, SourceLayer::Base).unwrap();
    let tokens = config_semantic_tokens(&entries, None);
    // Must have at least 2 tokens: one for each scalar key (name, version).
    assert!(
        tokens.data.len() >= 2,
        "must have at least 2 semantic tokens for 2 keys; got {}",
        tokens.data.len()
    );
    // All token lengths must be non-zero.
    for tok in &tokens.data {
        assert!(
            tok.length > 0,
            "all semantic token lengths must be > 0; got zero-length token: {:?}",
            tok
        );
    }
}

// TEST HONESTY FIX: empty input diagnostics must produce zero diagnostics (not just not-panic).
#[test]
fn test_toml_diagnostics_empty_input_produces_zero_diagnostics() {
    let diags = config_toml_diagnostics("", SourceLayer::Base, None);
    assert!(
        diags.is_empty(),
        "empty TOML input must produce zero diagnostics; got: {:?}",
        diags
    );
}
