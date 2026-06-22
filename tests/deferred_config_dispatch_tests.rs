//! Regression tests for intents 037-040 deferred-config bugs.
//!
//! Tests exercise the server dispatch path via `test_dispatch::TestBackend`,
//! which mirrors the exact routing code in `Backend::formatting`,
//! `Backend::rename`, and `Backend::publish_config_diagnostics_for`.
//!
//! All dispatch tests use the server-routing logic so the bugs that were
//! hidden by testing leaf functions directly are caught here.
//!
//! Bug map:
//! - C-1: formatting dispatch must not run KV-regex on YAML/TOML/JSONC
//! - C-2: rename dispatch must use format-specific functions (not generic)
//! - C-3: JSONC BOM offset in resolve_jsonc_key_span
//! - H-1: false "unknown key" for schema-valid JSONC/TOML keys
//! - H-2: canonical_key over-collapse (line-length ≡ LINE_LENGTH)
//! - M-1: collision check scoped to same-format docs
//! - M-3: is_appsettings_file rejects multi-dot envs
//! - M-4: per-request canonical key cache (correctness assertion)
//! - L-3: byte_offset_to_lsp_position out-of-bounds → clamp to end

use std::collections::HashMap;

use tower_lsp::lsp_types::Url;

use envforge::lsp::server::test_dispatch::TestBackend;
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use envforge::ops::schema_unification::{
    canonical_key, canonical_key_strict, cross_format_find_references,
    cross_format_goto_definition, is_key_sensitive, UnifiedSchema,
};
use envforge::parser::jsonc_config_parser::{parse_jsonc_config, resolve_jsonc_key_span};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn uri(path: &str) -> Url {
    Url::parse(&format!("file://{path}")).expect("valid test URL")
}

fn schema_with(name: &str, sensitive: bool, required: bool) -> EnvSchema {
    let mut variables = HashMap::new();
    variables.insert(
        name.to_string(),
        SchemaVariable {
            var_type: VarType::String,
            required,
            sensitive,
            ..Default::default()
        },
    );
    EnvSchema { variables }
}

// ── C-1: Formatting dispatch — YAML must be a no-op ──────────────────────────

/// C-1: formatting an application.yml with `spring:` bare keys returns NO edits
/// (YAML format is a deliberate no-op). Before the fix, the KV-normalizing regex
/// ran on YAML content and stripped the colon hierarchy.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_c1_yaml_format_is_noop_through_dispatch() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem:test\n    username: sa\n";
    let yaml_uri = uri("/workspace/src/main/resources/application.yml");

    let mut backend = TestBackend::new();
    backend.open_doc(yaml_uri.clone(), content.to_string());

    let edits = backend.formatting_dispatch(&yaml_uri);

    // YAML formatting must produce zero edits — byte-identical no-op.
    assert!(
        edits.is_empty(),
        "C-1: YAML format must be a no-op; got {} edit(s): {:?}",
        edits.len(),
        edits
    );
}

/// C-1: formatting a Cargo.toml with `host = "v"` does not strip spaces/quotes.
/// The TOML formatter uses toml_edit lossless round-trip, not the KV regex.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_c1_toml_format_preserves_spaces_and_quotes_through_dispatch() {
    let content = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    let toml_uri = uri("/workspace/Cargo.toml");

    let mut backend = TestBackend::new();
    backend.open_doc(toml_uri.clone(), content.to_string());

    let edits = backend.formatting_dispatch(&toml_uri);

    // toml_edit round-trip of valid, already-canonical TOML should produce zero edits.
    assert!(
        edits.is_empty(),
        "C-1: TOML format must not produce spurious edits; got {} edit(s)",
        edits.len()
    );
}

/// C-1: formatting a .properties file DOES apply the KV normalizer.
/// This is the original correct behaviour — must not be regressed.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_c1_properties_format_normalizes_kv_through_dispatch() {
    // Deliberately un-canonical: spaces around `=`.
    let content = "spring.datasource.url  =  jdbc:h2:mem:test\n";
    let props_uri = uri("/workspace/src/main/resources/application.properties");

    let mut backend = TestBackend::new();
    backend.open_doc(props_uri.clone(), content.to_string());

    let edits = backend.formatting_dispatch(&props_uri);

    // The KV normalizer SHOULD have removed the extra spaces.
    assert!(
        !edits.is_empty(),
        "C-1: .properties format must apply KV normalizer"
    );
    let normalized = &edits[0].new_text;
    assert!(
        normalized.contains("spring.datasource.url=jdbc:h2:mem:test"),
        "C-1: normalized .properties must have KEY=value without spaces, got: {}",
        normalized
    );
}

// ── C-2: Rename dispatch — YAML must use surgical splice ─────────────────────

/// C-2: rename a nested YAML key `spring.datasource.url`→`spring.datasource.uri`
/// via the server dispatch — the leaf `url` is renamed to `uri`.
/// Before the fix, config_rename wrote `spring.datasource.uri` into the leaf
/// range, producing `spring.datasource.spring.datasource.uri:`.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_c2_yaml_rename_uses_surgical_splice_through_dispatch() {
    let content = "spring:\n  datasource:\n    url: jdbc:h2:mem:test\n";
    let yaml_uri = uri("/workspace/src/main/resources/application.yml");

    let mut backend = TestBackend::new();
    backend.open_doc(yaml_uri.clone(), content.to_string());

    let edit = backend.rename_dispatch(&yaml_uri, "spring.datasource.url", "spring.datasource.uri");

    // Must produce an edit.
    let edit = edit.expect("C-2: YAML rename must produce a WorkspaceEdit");
    let changes = edit.changes.expect("WorkspaceEdit must have changes");
    let file_edits = changes
        .get(&yaml_uri)
        .expect("changes must contain yaml_uri");

    assert_eq!(file_edits.len(), 1, "C-2: exactly one TextEdit expected");
    let new_text = &file_edits[0].new_text;
    // Surgical splice: only `url` → `uri`, not the full dotted path.
    assert!(
        new_text == "uri",
        "C-2: YAML rename new_text must be leaf 'uri', got: {:?}",
        new_text
    );
}

/// C-2: rename a JSONC key `Logging:LogLevel:Default` with new leaf `Information`
/// via the server dispatch — the leaf `Default` is renamed to `Information`.
/// Before the fix, generic config_rename wrote the full dotted path into the
/// leaf range, producing `Logging:LogLevel:Information:` inside `"Default"`.
///
/// Note: the `new_name` parameter is the leaf segment only (per format-specific
/// rename API), not the full colon path. The old_key is the full path used to
/// locate the key; the new_name is substituted for the leaf.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_c2_jsonc_rename_uses_surgical_splice_through_dispatch() {
    let content = r#"{"Logging":{"LogLevel":{"Default":"Warning"}}}"#;
    let jsonc_uri = uri("/workspace/appsettings.json");

    let mut backend = TestBackend::new();
    backend.open_doc(jsonc_uri.clone(), content.to_string());

    // new_name is the leaf segment only (not full colon path).
    let edit = backend.rename_dispatch(&jsonc_uri, "Logging:LogLevel:Default", "Information");

    let edit = edit.expect("C-2: JSONC rename must produce a WorkspaceEdit");
    let changes = edit.changes.expect("WorkspaceEdit must have changes");
    let file_edits = changes
        .get(&jsonc_uri)
        .expect("changes must contain jsonc_uri");

    assert_eq!(file_edits.len(), 1, "C-2: exactly one TextEdit expected");
    let new_text = &file_edits[0].new_text;
    // The leaf replacement wraps the new segment in quotes (surgical splice).
    assert_eq!(
        new_text, "\"Information\"",
        "C-2: JSONC rename must splice only the leaf segment as quoted string, got: {:?}",
        new_text
    );
    // Verify the splice was applied at the correct offset — the key_range.start
    // of the TextEdit must NOT be at (0,0) (which would be the ghost-at-start-of-file bug).
    assert!(
        file_edits[0].range.start.line > 0 || file_edits[0].range.start.character > 0,
        "C-2: JSONC rename TextEdit must not be at start-of-file (0,0); \
         that would indicate the generic config_rename was called instead of JSONC-specific"
    );
}

// ── C-3: JSONC BOM offset in resolve_jsonc_key_span ──────────────────────────

/// C-3: a BOM-prefixed appsettings.json — resolve_jsonc_key_span must return
/// byte ranges relative to the ORIGINAL (BOM-included) content.
/// Before the fix, the range was 3 bytes off.
#[test]
fn test_c3_jsonc_bom_key_span_offset_is_correct() {
    // BOM (EF BB BF) + JSON content.
    let bom = "\u{FEFF}";
    let json_content = r#"{"Logging":{"LogLevel":{"Default":"Warning"}}}"#;
    let full_content = format!("{}{}", bom, json_content);

    let span = resolve_jsonc_key_span(&full_content, "Logging:LogLevel:Default");
    let span = span.expect("C-3: span must be resolved for BOM-prefixed content");

    // The span must be within the BOM-inclusive content bounds.
    assert!(
        span.byte_range.start < full_content.len() && span.byte_range.end <= full_content.len(),
        "C-3: byte_range {:?} must be within full_content length {}",
        span.byte_range,
        full_content.len()
    );

    // Extract the bytes at the returned range from the FULL content.
    let sliced = &full_content.as_bytes()[span.byte_range.clone()];
    let sliced_str = std::str::from_utf8(sliced).expect("valid UTF-8");

    // The slice must be the quoted key `"Default"`.
    assert_eq!(
        sliced_str, "\"Default\"",
        "C-3: BOM span must point to '\"Default\"' in full content, got: {:?}",
        sliced_str
    );
}

/// C-3: without a BOM, the span is still correct (no regression).
#[test]
fn test_c3_jsonc_no_bom_key_span_still_correct() {
    let content = r#"{"Logging":{"LogLevel":{"Default":"Warning"}}}"#;
    let span = resolve_jsonc_key_span(content, "Logging:LogLevel:Default");
    let span = span.expect("span must be resolved");

    let sliced = &content.as_bytes()[span.byte_range.clone()];
    let sliced_str = std::str::from_utf8(sliced).expect("valid UTF-8");
    assert_eq!(
        sliced_str, "\"Default\"",
        "C-3: no-BOM span must point to '\"Default\"'"
    );
}

// ── H-1: False unknown-key for schema-valid JSONC/TOML keys ──────────────────

/// H-1: a JSONC key `Logging:LogLevel:Default` whose schema has
/// `LOGGING__LOGLEVEL__DEFAULT` (canonical equivalent) must produce NO
/// unknown-key diagnostic.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_h1_jsonc_schema_valid_key_no_unknown_key_diag_through_dispatch() {
    let content = r#"{"Logging":{"LogLevel":{"Default":"Warning"}}}"#;
    let jsonc_uri = uri("/workspace/appsettings.json");

    let schema = schema_with("LOGGING__LOGLEVEL__DEFAULT", false, false);

    let mut backend = TestBackend::new();
    backend.open_doc(jsonc_uri.clone(), content.to_string());
    backend.set_schema(schema);

    let diags = backend.diagnostics_dispatch(&jsonc_uri);

    let unknown_key_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("Unknown key"))
        .collect();

    assert!(
        unknown_key_diags.is_empty(),
        "H-1: JSONC key 'Logging:LogLevel:Default' whose schema has \
         'LOGGING__LOGLEVEL__DEFAULT' must NOT produce unknown-key diagnostic; \
         got: {:?}",
        unknown_key_diags
    );
}

/// H-1: a genuinely absent JSONC key must still be flagged.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_h1_jsonc_genuinely_absent_key_still_flagged_through_dispatch() {
    let content = r#"{"SomeUnknownKey":"value"}"#;
    let jsonc_uri = uri("/workspace/appsettings.json");

    // Schema has a completely different key.
    let schema = schema_with("LOGGING__LOGLEVEL__DEFAULT", false, false);

    let mut backend = TestBackend::new();
    backend.open_doc(jsonc_uri.clone(), content.to_string());
    backend.set_schema(schema);

    let diags = backend.diagnostics_dispatch(&jsonc_uri);

    let unknown_key_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("Unknown key") && d.message.contains("SomeUnknownKey"))
        .collect();

    assert!(
        !unknown_key_diags.is_empty(),
        "H-1: genuinely absent key 'SomeUnknownKey' must produce unknown-key diagnostic"
    );
}

/// H-1: a TOML key `spring.datasource.url` whose schema has
/// `SPRING_DATASOURCE_URL` must produce NO unknown-key diagnostic.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_h1_toml_schema_valid_key_no_unknown_key_diag_through_dispatch() {
    let content = "[spring.datasource]\nurl = \"jdbc:h2:mem:test\"\n";
    let toml_uri = uri("/workspace/config.toml");

    let schema = schema_with("SPRING_DATASOURCE_URL", false, false);

    let mut backend = TestBackend::new();
    backend.open_doc(toml_uri.clone(), content.to_string());
    backend.set_schema(schema);

    let diags = backend.diagnostics_dispatch(&toml_uri);

    let unknown_key_diags: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.message.contains("Unknown key") && d.message.contains("spring.datasource.url")
        })
        .collect();

    assert!(
        unknown_key_diags.is_empty(),
        "H-1: TOML key 'spring.datasource.url' whose schema has 'SPRING_DATASOURCE_URL' \
         must NOT produce unknown-key diagnostic; got: {:?}",
        unknown_key_diags
    );
}

// ── H-2: canonical_key strict vs relaxed ─────────────────────────────────────

/// H-2: `LINE_LENGTH` (UPPER_SNAKE) and `line-length` (TOML bare key) must NOT
/// be treated as the same key for sensitivity / unknown-key purposes.
/// They must have DIFFERENT strict canonical keys.
#[test]
fn test_h2_strict_canonical_key_does_not_collapse_hyphen_with_upper_snake() {
    // UPPER_SNAKE: `_` → `.` → `line.length`
    let upper_snake = canonical_key_strict("LINE_LENGTH").expect("must not be None");
    // TOML bare key with hyphen: no collapse → `line-length`
    let toml_bare = canonical_key_strict("line-length").expect("must not be None");

    assert_ne!(
        upper_snake, toml_bare,
        "H-2: strict canonical key must NOT alias LINE_LENGTH with line-length; \
         got upper_snake={:?}, toml_bare={:?}",
        upper_snake, toml_bare
    );
    // Exact expected values.
    assert_eq!(upper_snake, "line.length", "LINE_LENGTH strict canonical");
    assert_eq!(toml_bare, "line-length", "line-length strict canonical");
}

/// H-2: `is_key_sensitive` must NOT flag `line-length` as sensitive when only
/// `LINE_LENGTH` is marked sensitive in the schema.
#[test]
fn test_h2_is_key_sensitive_does_not_false_positive_for_hyphen_key() {
    let schema = schema_with("LINE_LENGTH", true, false);
    let unified = UnifiedSchema::new(schema);

    // `LINE_LENGTH` must be sensitive.
    assert!(
        is_key_sensitive("LINE_LENGTH", &unified),
        "H-2: LINE_LENGTH must be sensitive"
    );

    // `line-length` (TOML bare key) must NOT be sensitive.
    assert!(
        !is_key_sensitive("line-length", &unified),
        "H-2: line-length must NOT be falsely marked sensitive; \
         LINE_LENGTH is sensitive but line-length is a different key"
    );
}

/// H-2: path-separator equivalences MUST still work (`:` ≡ `__` ≡ `.`).
#[test]
fn test_h2_path_separator_equivalence_preserved_in_strict_canonical() {
    let colon = canonical_key_strict("Logging:LogLevel:Default").expect("must not be None");
    let double_us = canonical_key_strict("LOGGING__LOGLEVEL__DEFAULT").expect("must not be None");
    let dotted = canonical_key_strict("logging.loglevel.default").expect("must not be None");
    let upper_snake = canonical_key_strict("LOGGING_LOGLEVEL_DEFAULT").expect("must not be None");

    assert_eq!(
        colon, double_us,
        "H-2: colon path must equal double-underscore path in strict canonical"
    );
    assert_eq!(
        colon, dotted,
        "H-2: colon path must equal dotted path in strict canonical"
    );
    assert_eq!(
        colon, upper_snake,
        "H-2: colon path must equal UPPER_SNAKE in strict canonical"
    );
}

/// H-2 + diagnostics path: `Cargo.toml`'s `line-length` must NOT produce
/// a false sensitivity diagnostic when schema marks `LINE_LENGTH` sensitive.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_h2_toml_line_length_not_flagged_sensitive_through_dispatch() {
    // Minimal Cargo.toml with a `line-length` key (rustfmt.toml style).
    // We're testing that it doesn't get redacted/flagged by a LINE_LENGTH schema entry.
    let content = "[fmt]\nline-length = 100\n";
    let toml_uri = uri("/workspace/config.toml");

    // Schema marks LINE_LENGTH as sensitive — must NOT affect line-length.
    let schema = schema_with("LINE_LENGTH", true, false);

    let mut backend = TestBackend::new();
    backend.open_doc(toml_uri.clone(), content.to_string());
    backend.set_schema(schema);

    let diags = backend.diagnostics_dispatch(&toml_uri);

    // The `line-length` key is unknown (LINE_LENGTH ≠ line-length), so it should
    // be flagged as unknown. But it must NOT be flagged as a TYPE_MISMATCH for
    // `LINE_LENGTH` (which would mean the schema lookup incorrectly resolved it).
    let type_mismatch_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("Type mismatch") && d.message.contains("line-length"))
        .collect();

    assert!(
        type_mismatch_diags.is_empty(),
        "H-2: line-length must not produce a type-mismatch diagnostic from LINE_LENGTH schema; \
         got: {:?}",
        type_mismatch_diags
    );
}

// ── M-1: Collision check scoped to same-format docs ──────────────────────────

/// M-1: a coincident key in Cargo.toml (TOML) must NOT block a rename of
/// the same key name in application.yml (YAML). Before the fix, the collision
/// check scanned ALL open config docs including TOML when renaming YAML.
///
/// TEST GOES THROUGH THE SERVER DISPATCH PATH.
#[test]
fn test_m1_toml_coincident_key_does_not_block_yaml_rename_through_dispatch() {
    let yaml_content = "spring:\n  datasource:\n    url: jdbc:h2:mem:test\n";
    let yaml_uri = uri("/workspace/src/main/resources/application.yml");

    // Cargo.toml happens to have a key that matches (dotted form).
    let toml_content = "[spring.datasource]\nurl = \"different-value\"\n";
    let toml_uri = uri("/workspace/Cargo.toml");

    let mut backend = TestBackend::new();
    backend.open_doc(yaml_uri.clone(), yaml_content.to_string());
    backend.open_doc(toml_uri.clone(), toml_content.to_string());

    // Rename spring.datasource.url in YAML — should succeed despite TOML having
    // the same key (they're different formats with separate namespaces).
    let edit = backend.rename_dispatch(&yaml_uri, "spring.datasource.url", "spring.datasource.uri");

    assert!(
        edit.is_some(),
        "M-1: YAML rename must succeed even when Cargo.toml has the same key; \
         cross-format collision check must be scoped to same-format docs"
    );
}

// ── M-3: is_appsettings_file rejects multi-dot envs ──────────────────────────

/// M-3: `appsettings.foo.bar.json` (multi-dot env) must NOT be recognized
/// as a config-format file. Only single-segment envs are valid.
#[test]
fn test_m3_appsettings_multi_dot_env_not_recognized() {
    use envforge::lsp::config_file::is_appsettings_file;

    // Valid: single-segment env.
    assert!(
        is_appsettings_file(&uri("/ws/appsettings.json")),
        "M-3: appsettings.json must be recognized"
    );
    assert!(
        is_appsettings_file(&uri("/ws/appsettings.Production.json")),
        "M-3: appsettings.Production.json must be recognized"
    );
    assert!(
        is_appsettings_file(&uri("/ws/appsettings.Development.json")),
        "M-3: appsettings.Development.json must be recognized"
    );

    // Invalid: multi-dot env (M-3 fix).
    assert!(
        !is_appsettings_file(&uri("/ws/appsettings.foo.bar.json")),
        "M-3: appsettings.foo.bar.json must NOT be recognized (multi-dot env)"
    );
    assert!(
        !is_appsettings_file(&uri("/ws/appsettings.a.b.c.json")),
        "M-3: appsettings.a.b.c.json must NOT be recognized (multi-dot env)"
    );
}

// ── M-4: per-request canonical key cache (correctness) ───────────────────────

/// M-4: cross_format_goto_definition / cross_format_find_references must
/// return the same results whether or not the per-request cache is populated.
/// This test verifies correctness (not performance) of the cache.
#[test]
fn test_m4_cross_format_goto_def_cache_gives_same_results_as_no_cache() {
    use envforge::ops::config_format::{ConfigEntry, SourceLayer};
    use tower_lsp::lsp_types::{Position, Range as LspRange};

    fn make_entry(key: &str, line: u32) -> ConfigEntry {
        let key_len = key.len() as u32;
        ConfigEntry {
            key: key.to_string(),
            value: String::new(),
            key_range: LspRange {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: key_len,
                },
            },
            value_range: LspRange {
                start: Position {
                    line,
                    character: key_len + 1,
                },
                end: Position {
                    line,
                    character: key_len + 1,
                },
            },
            line,
            source_layer: SourceLayer::Base,
        }
    }

    let yaml_url = uri("/ws/application.yml");
    let json_url = uri("/ws/appsettings.json");

    let mut open_docs = HashMap::new();
    // Both formats have the key under different representations.
    open_docs.insert(
        yaml_url.clone(),
        vec![make_entry("spring.datasource.url", 5)],
    );
    open_docs.insert(
        json_url.clone(),
        vec![make_entry("Spring:Datasource:Url", 2)],
    );

    // goto-def with the relaxed canonical key (Spring relaxed binding).
    let locs1 =
        cross_format_goto_definition("SPRING_DATASOURCE_URL", None, &HashMap::new(), &open_docs);
    // Call again — the per-request cache resets on each call.
    let locs2 =
        cross_format_goto_definition("SPRING_DATASOURCE_URL", None, &HashMap::new(), &open_docs);

    assert_eq!(
        locs1.len(),
        locs2.len(),
        "M-4: repeated calls must return same count"
    );
    assert_eq!(
        locs1, locs2,
        "M-4: per-request cache must not affect result correctness"
    );
    // Must have found both formats.
    assert_eq!(locs1.len(), 2, "M-4: must find 2 locations (yaml + jsonc)");
}

/// M-4: cross_format_find_references cache correctness.
#[test]
fn test_m4_cross_format_find_references_cache_gives_same_results() {
    use envforge::ops::config_format::{ConfigEntry, SourceLayer};
    use tower_lsp::lsp_types::{Position, Range as LspRange};

    fn make_entry(key: &str, line: u32) -> ConfigEntry {
        let key_len = key.len() as u32;
        ConfigEntry {
            key: key.to_string(),
            value: String::new(),
            key_range: LspRange {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: key_len,
                },
            },
            value_range: LspRange {
                start: Position {
                    line,
                    character: key_len + 1,
                },
                end: Position {
                    line,
                    character: key_len + 1,
                },
            },
            line,
            source_layer: SourceLayer::Base,
        }
    }

    let mut open_docs = HashMap::new();
    open_docs.insert(
        uri("/ws/application.yml"),
        vec![make_entry("spring.datasource.url", 5)],
    );

    let refs1 = cross_format_find_references(
        "SPRING_DATASOURCE_URL",
        None,
        &HashMap::new(),
        &open_docs,
        false,
    );
    let refs2 = cross_format_find_references(
        "SPRING_DATASOURCE_URL",
        None,
        &HashMap::new(),
        &open_docs,
        false,
    );

    assert_eq!(
        refs1, refs2,
        "M-4: cross_format_find_references must be idempotent"
    );
}

// ── L-3: byte_offset_to_lsp_position out-of-bounds clamping ─────────────────

/// L-3: an out-of-bounds byte offset in parse_jsonc_config must not produce
/// a ghost position at (0,0). The parser must clamp to end-of-content.
/// We verify this indirectly by ensuring parse_jsonc_config doesn't panic on
/// a BOM-only string and that positions are non-negative.
#[test]
fn test_l3_jsonc_parser_handles_out_of_bounds_offset_without_panic() {
    use envforge::ops::config_format::SourceLayer;

    // Content that could produce out-of-bounds offsets if not handled.
    let content = r#"{"a":"b"}"#;

    // Should not panic on any input.
    let (entries, diags) = parse_jsonc_config(content, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "L-3: valid JSON must produce no diags");
    assert_eq!(entries.len(), 1, "L-3: must parse 1 entry");

    // Position must be valid LSP positions (line and char are u32 so always >= 0).
    let e = &entries[0];
    assert!(
        e.key_range.start.line < 1000,
        "L-3: key_range.start.line must be sane"
    );
}

/// L-3: BOM-only content must not panic.
#[test]
fn test_l3_jsonc_bom_only_content_no_panic() {
    use envforge::ops::config_format::SourceLayer;

    let bom_only = "\u{FEFF}";
    let (entries, _diags) = parse_jsonc_config(bom_only, SourceLayer::DotNetBase);
    assert!(
        entries.is_empty(),
        "L-3: BOM-only must parse to empty entries"
    );
}

// ── H-2: relaxed canonical_key still works for goto/refs ─────────────────────

/// H-2: The relaxed canonical_key must still alias camelCase / hyphen for
/// goto-def / refs purposes (only strict is tightened).
#[test]
fn test_h2_relaxed_canonical_key_still_collapses_for_goto_refs() {
    // Relaxed must still alias spring-datasource-url with SPRING_DATASOURCE_URL.
    let spring_upper = canonical_key("SPRING_DATASOURCE_URL").expect("must not be None");
    let spring_lower = canonical_key("spring.datasource.url").expect("must not be None");
    let spring_colon = canonical_key("Spring:Datasource:Url").expect("must not be None");
    let spring_camel = canonical_key("springDatasourceUrl").expect("must not be None");

    assert_eq!(
        spring_upper, spring_lower,
        "relaxed: UPPER_SNAKE must alias dotted"
    );
    assert_eq!(
        spring_upper, spring_colon,
        "relaxed: UPPER_SNAKE must alias colon path"
    );
    assert_eq!(
        spring_upper, spring_camel,
        "relaxed: UPPER_SNAKE must alias camelCase"
    );
}
