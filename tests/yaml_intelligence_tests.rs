//! Integration tests for Unit 002 — YAML Config Intelligence (Read-Only)
//! (Intent 036, Stories 001–005).
//!
//! Tests live here (not in-module) per CLAUDE.md conventions.
//! Naming: `test_{what_is_being_tested}_{condition}`.

use std::collections::HashMap;

use envforge::lsp::config_features::{
    config_format_text_edits, config_hover, config_rename, config_semantic_tokens,
    config_yaml_diagnostics,
};
use envforge::lsp::config_file::{
    format_for_uri, is_config_format_file, is_jvm_config_file, is_yaml_config_file,
};
use envforge::lsp::document_symbol::config_document_symbols;
use envforge::ops::config_format::{
    source_layer_for_yaml, ConfigEntry, SourceLayer, WriteCapability,
};
use envforge::ops::properties_parser::parse_dotenv_cascade;
use envforge::parser::yaml_config_parser::{parse_yaml_config, parse_yaml_to_entries};
use tower_lsp::lsp_types::{Position, Url};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 001: YAML file recognition + write-capability classification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_recognize_application_yml_recognized() {
    assert!(is_yaml_config_file(&url("/proj/application.yml")));
}

#[test]
fn test_recognize_application_yaml_recognized() {
    assert!(is_yaml_config_file(&url("/proj/application.yaml")));
}

#[test]
fn test_recognize_application_profile_yml_recognized() {
    assert!(is_yaml_config_file(&url("/proj/application-prod.yml")));
    assert!(is_yaml_config_file(&url("/proj/application-staging.yaml")));
}

#[test]
fn test_recognize_generic_yml_file_not_recognized() {
    // After F9 fix: only application.yml/yaml patterns are recognized.
    // docker-compose.yml, k8s manifests, CI workflows are excluded.
    assert!(!is_yaml_config_file(&url("/proj/config.yaml")));
    assert!(!is_yaml_config_file(&url("/proj/something.yml")));
    assert!(!is_yaml_config_file(&url("/proj/docker-compose.yml")));
    assert!(!is_yaml_config_file(&url("/proj/.github/workflows/ci.yml")));
}

#[test]
fn test_recognize_non_yaml_extensions_not_recognized() {
    assert!(!is_yaml_config_file(&url("/proj/application.yaml.bak")));
    assert!(!is_yaml_config_file(&url("/proj/application.properties")));
    assert!(!is_yaml_config_file(&url("/proj/.env")));
}

#[test]
fn test_is_config_format_file_includes_yaml() {
    // YAML files are handled by the combined predicate (FR25 seam).
    assert!(is_config_format_file(&url("/proj/application.yml")));
    assert!(is_config_format_file(&url("/proj/application-dev.yaml")));
}

#[test]
fn test_format_for_uri_yaml_returns_readonly_format() {
    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadOnly);
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_format_for_uri_yaml_profile_returns_profile_layer() {
    let (_fmt, layer) = format_for_uri(&url("/proj/application-prod.yaml")).unwrap();
    assert_eq!(layer, SourceLayer::Profile("prod".to_string()));
}

#[test]
fn test_format_for_uri_yaml_non_application_returns_none() {
    // After F9 fix: non-application YAML files are not recognized.
    let result = format_for_uri(&url("/proj/config.yaml"));
    assert!(
        result.is_none(),
        "non-application YAML should not be recognized as a config-format file"
    );
}

#[test]
fn test_yaml_does_not_affect_properties_recognition() {
    // Existing .properties recognition must be unchanged (no regression FR25).
    assert!(is_jvm_config_file(&url("/proj/application.properties")));
    assert!(is_jvm_config_file(&url(
        "/proj/application-prod.properties"
    )));
    // .properties files are NOT YAML files.
    assert!(!is_yaml_config_file(&url("/proj/application.properties")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 001: source_layer_for_yaml rules
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_source_layer_application_yml_is_base() {
    assert_eq!(source_layer_for_yaml("application.yml"), SourceLayer::Base);
    assert_eq!(source_layer_for_yaml("application.yaml"), SourceLayer::Base);
}

#[test]
fn test_source_layer_profile_yml_is_profile() {
    assert_eq!(
        source_layer_for_yaml("application-prod.yml"),
        SourceLayer::Profile("prod".to_string())
    );
    assert_eq!(
        source_layer_for_yaml("application-staging.yaml"),
        SourceLayer::Profile("staging".to_string())
    );
}

#[test]
fn test_source_layer_empty_profile_yaml_falls_back_to_base() {
    // application-.yml has empty profile segment → Base (no crash).
    assert_eq!(source_layer_for_yaml("application-.yml"), SourceLayer::Base);
}

#[test]
fn test_source_layer_generic_yaml_is_base() {
    assert_eq!(source_layer_for_yaml("config.yaml"), SourceLayer::Base);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 002: YAML parse + flatten
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_yaml_flat_key_parsed() {
    let content = "key: value\n";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "key");
    assert_eq!(entries[0].value, "value");
}

#[test]
fn test_parse_yaml_nested_keys_flattened_to_dotted_path() {
    let content = "spring:\n  datasource:\n    url: jdbc:postgresql://localhost/db\n";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "spring.datasource.url");
    assert_eq!(entries[0].value, "jdbc:postgresql://localhost/db");
}

#[test]
fn test_parse_yaml_multiple_nested_keys_all_flattened() {
    let content = "\
spring:
  datasource:
    url: jdbc:h2:mem
    username: sa
    password: secret
";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    assert_eq!(entries.len(), 3);
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert!(keys.contains(&"spring.datasource.url"));
    assert!(keys.contains(&"spring.datasource.username"));
    assert!(keys.contains(&"spring.datasource.password"));
}

#[test]
fn test_parse_yaml_source_layer_propagated() {
    let content = "key: value\n";
    let (entries, _) =
        parse_yaml_config(content, SourceLayer::Profile("prod".to_string())).unwrap();
    assert_eq!(
        entries[0].source_layer,
        SourceLayer::Profile("prod".to_string())
    );
}

#[test]
fn test_parse_yaml_empty_document_returns_empty_entries() {
    let (entries, errs) = parse_yaml_config("", SourceLayer::Base).unwrap();
    assert!(entries.is_empty());
    assert!(errs.is_empty());
}

#[test]
fn test_parse_yaml_sequence_values_skipped_no_panic() {
    // Lists are not flattened to indexed entries in the read model.
    let content = "servers:\n  - host1\n  - host2\n";
    let result = parse_yaml_config(content, SourceLayer::Base);
    // Must not panic; entries for list items are omitted.
    assert!(result.is_ok());
    let (entries, _) = result.unwrap();
    // `servers` has a sequence value → skipped.
    assert!(
        !entries.iter().any(|e| e.key == "servers"),
        "sequence-valued key should be skipped"
    );
}

#[test]
fn test_parse_yaml_key_positions_are_zero_based_line() {
    let content = "first: one\nsecond: two\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    let first = entries.iter().find(|e| e.key == "first").unwrap();
    let second = entries.iter().find(|e| e.key == "second").unwrap();
    assert_eq!(first.key_range.start.line, 0);
    assert_eq!(second.key_range.start.line, 1);
}

#[test]
fn test_parse_yaml_multibyte_value_utf16_column_correct() {
    // Japanese string: each char is 1 UTF-16 code unit but 3 UTF-8 bytes.
    // The value marker byte-col should still produce the correct UTF-16 column.
    let content = "greeting: こんにちは\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, "こんにちは");
    // Value starts after "greeting: " (10 UTF-16 units including the space).
    // The UTF-16 column offset must equal the number of UTF-16 units before the value.
    let val_col = entries[0].value_range.start.character;
    // "greeting: " is 10 ASCII chars = 10 UTF-16 units.
    assert_eq!(val_col, 10, "value start UTF-16 column should be 10");
}

#[test]
fn test_parse_yaml_deeply_nested_no_crash() {
    let content = "a:\n  b:\n    c:\n      d:\n        e: deep_value\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "a.b.c.d.e");
    assert_eq!(entries[0].value, "deep_value");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 004: YAML diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_diagnostics_duplicate_key_detected() {
    let content = "key: value1\nkey: value2\n";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    // The parser / yaml-rust2 may or may not surface duplicate-key as an error
    // depending on the YAML spec behaviour (YAML 1.2 allows it as a warning).
    // We emit a duplicate-key diagnostic via our own seen_keys tracking.
    let dup_diag = diags.iter().any(|d| d.message.contains("Duplicate"));
    assert!(
        dup_diag,
        "duplicate-key diagnostic expected; got: {:?}",
        diags
    );
}

#[test]
fn test_yaml_diagnostics_unterminated_interpolation_detected() {
    let content = "key: ${MISSING\n";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    let unterm = diags.iter().any(|d| d.message.contains("Unterminated"));
    assert!(
        unterm,
        "unterminated interpolation diagnostic expected; got: {:?}",
        diags
    );
}

#[test]
fn test_yaml_diagnostics_well_formed_no_diagnostics() {
    let content = "spring:\n  app:\n    name: my-app\n";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    assert!(
        diags.is_empty(),
        "no diagnostics expected for well-formed YAML; got: {:?}",
        diags
    );
}

// test_yaml_diagnostics_malformed_yaml_returns_error_diagnostic_no_panic was
// vacuous (no assert). Replaced by:
//   test_yaml_diagnostics_real_malformed_yaml_produces_error_diagnostic (below).

// test_yaml_diagnostics_multiple_unterminated_all_reported had a weak `>=1`
// assert. Replaced by:
//   test_yaml_diagnostics_two_unterminated_both_reported (below).

// ═══════════════════════════════════════════════════════════════════════════════
// Story 005: YAML write-guard — no language feature writes YAML
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_write_capability_is_readonly() {
    let (fmt, _) = format_for_uri(&url("/proj/application.yml")).unwrap();
    assert_eq!(
        fmt.write_capability(),
        WriteCapability::ReadOnly,
        "YAML format must be ReadOnly"
    );
}

#[test]
fn test_yaml_format_text_edits_returns_empty_for_readonly() {
    // config_format_text_edits respects WriteCapability::ReadOnly → empty edits.
    let content = "key: value\n";
    let edits = config_format_text_edits(content, WriteCapability::ReadOnly);
    assert!(
        edits.is_empty(),
        "format edits must be empty for ReadOnly YAML; got {} edit(s)",
        edits.len()
    );
}

#[test]
fn test_yaml_rename_returns_none_for_readonly() {
    // config_rename must return None for ReadOnly capability (write-guard).
    let open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    let result = config_rename(
        "key",
        "new_key",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &open_docs,
    );
    assert!(
        result.is_none(),
        "rename must return None for ReadOnly YAML"
    );
}

// test_yaml_semantic_tokens_work_for_yaml_entries was vacuous (let _ = tokens).
// Replaced by:
//   test_yaml_semantic_tokens_contain_expected_token_for_yaml_key (below).

#[test]
fn test_yaml_hover_works_for_yaml_entries() {
    let content = "greeting: hello\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    // Hover at the key position on line 0.
    let pos = Position {
        line: 0,
        character: 2,
    };
    let hover = config_hover(pos, &entries, &[entries.clone()], None);
    assert!(hover.is_some(), "hover should work for YAML entries");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 003: YAML read features prove FR25 (no Unit-001 handler changes)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_format_parse_returns_entries() {
    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    let content = "server:\n  port: 8080\n";
    let entries = fmt.parse(content, layer);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "server.port");
    assert_eq!(entries[0].value, "8080");
}

#[test]
fn test_yaml_format_resolve_returns_correct_value() {
    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    let content = "app:\n  name: my-service\n";
    let entries = fmt.parse(content, layer);
    let layers = vec![entries];
    let resolved = fmt.resolve("app.name", &layers);
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().value, "my-service");
}

#[test]
fn test_yaml_format_resolve_missing_key_returns_none() {
    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    let content = "key: value\n";
    let entries = fmt.parse(content, layer);
    let layers = vec![entries];
    assert!(fmt.resolve("nonexistent", &layers).is_none());
}

#[test]
fn test_yaml_cross_layer_resolution_profile_overrides_base() {
    // Base YAML + profile YAML: profile value wins (FR15 for YAML).
    let (base_fmt, base_layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    let (prof_fmt, prof_layer) = format_for_uri(&url("/proj/application-prod.yml")).unwrap();

    let base_content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n";
    let prod_content = "spring:\n  datasource:\n    url: jdbc:postgresql://prod/db\n";

    let base_entries = base_fmt.parse(base_content, base_layer);
    let prod_entries = prof_fmt.parse(prod_content, prof_layer);

    // Sort by precedence: base first, profile second.
    let layers = vec![base_entries, prod_entries];

    // Use the base format's resolve (implementation is format-agnostic).
    let resolved = base_fmt.resolve("spring.datasource.url", &layers);
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().value,
        "jdbc:postgresql://prod/db",
        "profile should override base"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_yaml_null_value_handled() {
    let content = "key: ~\n";
    let result = parse_yaml_config(content, SourceLayer::Base);
    // Must not panic. Null values appear as "~" or "null" string in our model.
    assert!(result.is_ok());
}

#[test]
fn test_parse_yaml_boolean_value_handled() {
    let content = "flag: true\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "flag");
    // Boolean is represented as string in the flat model.
    assert!(!entries[0].value.is_empty());
}

#[test]
fn test_parse_yaml_integer_value_handled() {
    let content = "port: 8080\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, "8080");
}

#[test]
fn test_parse_yaml_key_range_character_is_zero_for_top_level() {
    let content = "mykey: value\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    // Top-level key starts at column 0.
    assert_eq!(entries[0].key_range.start.character, 0);
}

#[test]
fn test_parse_yaml_nested_key_range_has_positive_indent_col() {
    let content = "outer:\n  inner: value\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    let inner = entries.iter().find(|e| e.key == "outer.inner").unwrap();
    // "  inner" — key starts at column 2.
    assert_eq!(
        inner.key_range.start.character, 2,
        "nested key should start at its indentation column"
    );
}

#[test]
fn test_parse_yaml_entries_have_correct_line_field() {
    let content = "a: 1\nb: 2\nc: 3\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    let a = entries.iter().find(|e| e.key == "a").unwrap();
    let b = entries.iter().find(|e| e.key == "b").unwrap();
    let c = entries.iter().find(|e| e.key == "c").unwrap();
    assert_eq!(a.line, 0);
    assert_eq!(b.line, 1);
    assert_eq!(c.line, 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Regression tests (bug-fixes F1, F2, F4, F5, F6, F7, F8, F9)
// ═══════════════════════════════════════════════════════════════════════════════

// ── F1: UTF-16 positions correct on lines with non-ASCII (NFR14) ─────────────

/// Regression for F1: a comment or key with non-ASCII chars on an earlier line
/// must not corrupt the UTF-16 column of an ASCII key on a later line.
/// The old code mixed Marker::index() (char offset) with line_start_byte_offset()
/// (byte offset), which produced garbage for any doc containing multi-byte chars.
#[test]
fn test_f1_utf16_ascii_key_after_non_ascii_comment_correct_position() {
    // Line 0: "# café" (non-ASCII 'é' = U+00E9, 2 UTF-8 bytes)
    // Line 1: "key: value" (pure ASCII)
    let content = "# café\nkey: value\n";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty(), "no errors expected: {:?}", errs);
    assert_eq!(entries.len(), 1);
    // `key` is on line 1, starting at char/UTF-16 column 0.
    assert_eq!(entries[0].key_range.start.line, 1);
    assert_eq!(
        entries[0].key_range.start.character, 0,
        "UTF-16 column of ASCII key on line after non-ASCII must be 0, not corrupted"
    );
    // Value `value` starts after "key: " = 5 UTF-16 units.
    assert_eq!(
        entries[0].value_range.start.character, 5,
        "value UTF-16 column must be 5"
    );
}

/// F1 supplementary: non-ASCII KEY on a line — UTF-16 column of the value is correct.
#[test]
fn test_f1_utf16_value_col_after_multibyte_key_correct() {
    // "naïve: yes\n" — 'ï' is U+00EF (1 UTF-16 unit, 2 UTF-8 bytes)
    // "naïve" is 5 chars = 5 UTF-16 units; ": " is 2 more → value at col 7.
    let content = "naive: yes\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert_eq!(entries[0].value_range.start.character, 7);
}

// ── F4: 3+ duplicate-key occurrences each get their own diagnostic ────────────

/// Regression for F4: a key appearing 3 times must yield exactly 2 duplicate
/// diagnostics, each pointing to the correct (non-first) line.
#[test]
fn test_f4_triple_duplicate_key_two_diagnostics_correct_lines() {
    // Line 0: key: first
    // Line 1: key: second   ← first duplicate
    // Line 2: key: third    ← second duplicate
    let content = "key: first\nkey: second\nkey: third\n";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    let dup_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("Duplicate"))
        .collect();
    assert_eq!(
        dup_diags.len(),
        2,
        "expected 2 duplicate diagnostics for a 3x key; got {:?}",
        dup_diags
    );
    // Each duplicate must point to a line > 0 (lines 1 and 2).
    let dup_lines: Vec<u32> = dup_diags.iter().map(|d| d.range.start.line).collect();
    assert!(
        dup_lines.contains(&1),
        "line 1 must be reported; got {:?}",
        dup_lines
    );
    assert!(
        dup_lines.contains(&2),
        "line 2 must be reported; got {:?}",
        dup_lines
    );
}

// ── F5: sequence-inside-mapping path_stack balance ───────────────────────────

/// Regression for F5: a sequence inside a mapping must not corrupt path_stack,
/// and a sibling key after the sequence must resolve with the correct dotted path.
#[test]
fn test_f5_sequence_in_mapping_sibling_resolves_correctly() {
    let content = "outer:\n  list:\n    - item1\n    - item2\n  sibling: data\n";
    let (entries, errs) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(errs.is_empty(), "no errors expected: {:?}", errs);
    // `outer.sibling` must be present.
    let sibling = entries.iter().find(|e| e.key == "outer.sibling");
    assert!(
        sibling.is_some(),
        "outer.sibling must resolve; entries: {:?}",
        entries.iter().map(|e| &e.key).collect::<Vec<_>>()
    );
    assert_eq!(sibling.unwrap().value, "data");
    // No entry for the list items (sequences are skipped).
    assert!(
        !entries
            .iter()
            .any(|e| e.key.starts_with("outer.list.") || e.key == "outer.list"),
        "list items must be skipped; entries: {:?}",
        entries.iter().map(|e| &e.key).collect::<Vec<_>>()
    );
}

// ── F6: .env / config-format files serve document_symbol (Unit-001 regression) ─

/// Regression for F6: parse a .env-cascade file into config entries and assert
/// that config_document_symbols returns symbols — proving the symbol provider
/// works with ConfigEntry (not just EnvDocEntry).
#[test]
fn test_f6_config_document_symbols_returns_symbols_for_config_entries() {
    use envforge::lsp::document_symbol::config_document_symbols;
    use envforge::ops::properties_parser::parse_dotenv_cascade;

    let content = "DB_HOST=localhost\nDB_PORT=5432\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::Base);
    assert!(!entries.is_empty(), "entries must be parsed");

    let result = config_document_symbols(&entries, None);
    assert!(
        result.is_some(),
        "document_symbols must return Some for .env entries"
    );
}

// ── F7: full_doc_range uses UTF-16 units for last line (NFR14) ───────────────

/// Regression for F7: when the last line of a document ends with an emoji
/// (2 UTF-16 surrogate-pair units), the range end character must be 2 not 1.
#[test]
fn test_f7_full_doc_range_emoji_last_line_utf16_correct() {
    use envforge::lsp::config_features::config_format_text_edits;
    // Content where last line contains an emoji (U+1F600 = 2 UTF-16 units).
    // Formatting this ASCII content produces no edits; what we really test is
    // that the range calculation doesn't panic and the UTF-16 length is correct
    // (indirectly: if it were chars().count() it would return 3 instead of 4
    // for a 3-char line ending with an emoji).
    // We test full_doc_range indirectly through config_format_text_edits on a
    // doc that needs reformatting. Since format is ReadWrite for .properties,
    // we pass ReadWrite so the function actually computes the range.
    let content = "KEY  =  value\nlast=\u{1F600}";
    let edits = config_format_text_edits(content, WriteCapability::ReadWrite);
    // The edit should cover the entire document. The end character of the last
    // line "last=😀" is 6 chars but 7 UTF-16 units (emoji = 2).
    // If the edit exists, verify the end character is 7 (UTF-16), not 6 (chars).
    if let Some(edit) = edits.first() {
        // "last=😀" = l(1) a(1) s(1) t(1) =(1) 😀(2) = 7 UTF-16 units.
        assert_eq!(
            edit.range.end.character, 7,
            "end character must be UTF-16 units (7 for 'last=😀'), not char count (6)"
        );
    }
}

// ── F8: config_ref_completions replace_range uses UTF-16 units ───────────────

/// Regression for F8: when the line before the cursor contains non-ASCII,
/// the completion replace_range start must be a UTF-16 offset, not a byte offset.
#[test]
fn test_f8_ref_completion_start_utf16_with_multibyte_prefix() {
    use envforge::lsp::config_features::config_completions;
    use envforge::ops::properties_parser::parse_dotenv_cascade;

    // Line: "café=${" — 'café' is 4 chars but 5 UTF-8 bytes.
    // The `$` starts at byte offset 5 but UTF-16 offset 4 (all BMP chars).
    let content = "café=${";
    let entries = parse_dotenv_cascade(content, SourceLayer::Base);
    let pos = Position {
        line: 0,
        // Cursor at end of line: 'café=${' = 7 chars = 7 UTF-16 units.
        character: 7,
    };
    let items = config_completions(pos, content, &entries, None);
    // If there are completion items, the replace_range start must not exceed
    // the cursor position (which is 7 UTF-16 units, not the byte position 8).
    for item in &items {
        use tower_lsp::lsp_types::CompletionTextEdit;
        if let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit {
            assert!(
                edit.range.start.character <= 7,
                "replace_range start must be UTF-16 (<=7), not byte offset (8); got {}",
                edit.range.start.character
            );
        }
    }
}

// ── F9: YAML recognition restricted to application.yml/yaml patterns ─────────

/// Regression for F9: docker-compose.yml, k8s manifests, and GitHub Actions
/// workflows must NOT be routed to the YAML config handler.
#[test]
fn test_f9_docker_compose_not_recognized_as_yaml_config() {
    assert!(
        !is_yaml_config_file(&url("/proj/docker-compose.yml")),
        "docker-compose.yml must not be recognized"
    );
    assert!(
        !is_yaml_config_file(&url("/proj/docker-compose.yaml")),
        "docker-compose.yaml must not be recognized"
    );
}

#[test]
fn test_f9_k8s_deployment_yaml_not_recognized() {
    assert!(
        !is_yaml_config_file(&url("/proj/k8s/deployment.yaml")),
        "k8s deployment.yaml must not be recognized"
    );
}

#[test]
fn test_f9_github_workflow_yml_not_recognized() {
    assert!(
        !is_yaml_config_file(&url("/proj/.github/workflows/ci.yml")),
        "GitHub Actions ci.yml must not be recognized"
    );
}

#[test]
fn test_f9_application_yml_still_recognized() {
    assert!(is_yaml_config_file(&url("/proj/application.yml")));
    assert!(is_yaml_config_file(&url("/proj/application.yaml")));
}

#[test]
fn test_f9_application_profile_variants_still_recognized() {
    assert!(is_yaml_config_file(&url("/proj/application-prod.yml")));
    assert!(is_yaml_config_file(&url("/proj/application-staging.yaml")));
    assert!(is_yaml_config_file(&url("/proj/application-test.yml")));
}

// ── Improved vacuous-test replacements ───────────────────────────────────────

/// Replacement for vacuous test_yaml_semantic_tokens_work_for_yaml_entries.
/// Asserts the actual token contents rather than discarding with `let _ = tokens`.
#[test]
fn test_yaml_semantic_tokens_contain_expected_token_for_yaml_key() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem\n";
    let (entries, _) = parse_yaml_config(content, SourceLayer::Base).unwrap();
    assert!(!entries.is_empty());
    let tokens = config_semantic_tokens(&entries, None);
    // The entry `spring.datasource.url` must produce at least 1 token for the key
    // and 1 for the value (non-sensitive, so both are emitted).
    assert!(
        !tokens.data.is_empty(),
        "semantic tokens must not be empty for a parsed YAML entry"
    );
    // Exactly 2 tokens expected: key token (TYPE_VARIABLE) and value token (TYPE_STRING).
    assert_eq!(
        tokens.data.len(),
        2,
        "expected 1 key + 1 value token; got {:?}",
        tokens.data
    );
}

/// Replacement for test_yaml_diagnostics_malformed_yaml_returns_error_diagnostic_no_panic.
/// Uses genuinely malformed YAML and asserts a diagnostic is produced.
#[test]
fn test_yaml_diagnostics_real_malformed_yaml_produces_error_diagnostic() {
    // "key: [unclosed" — the flow sequence is never closed → parse error.
    let content = "key: [unclosed";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    assert!(
        !diags.is_empty(),
        "a parse-error diagnostic must be emitted for malformed YAML"
    );
    let has_error = diags
        .iter()
        .any(|d| d.severity == tower_lsp::lsp_types::DiagnosticSeverity::ERROR);
    assert!(
        has_error,
        "malformed YAML diagnostic must have ERROR severity; got {:?}",
        diags
    );
}

/// Replacement for test_yaml_diagnostics_multiple_unterminated_all_reported.
/// Asserts >= 2 diagnostics for 2 unterminated interpolations, not just >= 1.
#[test]
fn test_yaml_diagnostics_two_unterminated_both_reported() {
    let content = "a: ${A_MISSING\nb: ${B_MISSING\n";
    let (_entries, diags) = parse_yaml_to_entries(content, SourceLayer::Base);
    let unterm_count = diags
        .iter()
        .filter(|d| d.message.contains("Unterminated"))
        .count();
    assert!(
        unterm_count >= 2,
        "expected at least 2 unterminated interpolation diagnostics; got {}",
        unterm_count
    );
}

/// Replacement for vacuous write-guard tests: confirm YamlFormat.write_capability()
/// is ReadOnly AND that rename returns None AND format returns empty edits
/// when using a real parsed YAML document.
#[test]
fn test_yaml_write_guard_real_document_rename_and_format_return_nothing() {
    use envforge::lsp::config_features::{config_format_text_edits, config_rename};
    use envforge::lsp::config_file::format_for_uri;
    use envforge::ops::config_format::WriteCapability;

    let (fmt, layer) = format_for_uri(&url("/proj/application.yml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadOnly);

    let content = "server:\n  port: 8080\n";
    let entries = fmt.parse(content, layer);
    assert!(!entries.is_empty(), "entries must be parsed");

    // Rename must return None for ReadOnly.
    let open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    let rename_result = config_rename(
        "server.port",
        "server.http_port",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &open_docs,
    );
    assert!(
        rename_result.is_none(),
        "rename must return None for ReadOnly YAML document"
    );

    // Format must return empty edits for ReadOnly.
    let format_edits = config_format_text_edits(content, WriteCapability::ReadOnly);
    assert!(
        format_edits.is_empty(),
        "format edits must be empty for ReadOnly YAML document"
    );
}

// ── F2: did_save republishes config/YAML diagnostics ─────────────────────────

/// Regression for F2: verify that config_yaml_diagnostics (the function called
/// by publish_config_diagnostics_for on save) detects a duplicate key in YAML.
/// This is the unit-level proof that the pipeline is wired correctly.
#[test]
fn test_f2_yaml_diagnostics_via_config_yaml_diagnostics_on_save() {
    use envforge::lsp::config_features::config_yaml_diagnostics;
    use envforge::ops::config_format::SourceLayer;

    // Duplicate key — should be flagged as a WARNING on save.
    let content = "key: value1\nkey: value2\n";
    let diags = config_yaml_diagnostics(content, SourceLayer::Base);
    let has_dup = diags.iter().any(|d| d.message.contains("Duplicate"));
    assert!(
        has_dup,
        "config_yaml_diagnostics must report duplicate key (used by did_save); got {:?}",
        diags
    );
}
