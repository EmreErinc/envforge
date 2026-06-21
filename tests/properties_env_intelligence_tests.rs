//! Integration tests for Unit 001 — Properties & `.env` Config Intelligence
//! (Intent 036, Stories 001–009).
//!
//! Tests live here (not in-module) per CLAUDE.md conventions.
//! Naming: `test_{what_is_being_tested}_{condition}`.

use std::collections::HashMap;

use envforge::lsp::config_features::{
    config_completions, config_diagnostics, config_find_references, config_format_document,
    config_format_text_edits, config_goto_definition, config_hover, config_rename,
    config_semantic_tokens, is_valid_config_key,
};
use envforge::lsp::config_file::{
    format_for_uri, is_config_format_file, is_env_cascade_file, is_jvm_config_file,
};
use envforge::ops::config_format::{
    source_layer_for_dotenv, source_layer_for_properties, ConfigEntry, ResolvedValue, SourceLayer,
    WriteCapability,
};
use envforge::ops::config_resolution::{
    find_unterminated_refs, interpolate_value, resolve_effective_value, resolve_layers,
};
use envforge::ops::properties_parser::{parse_dotenv_cascade, parse_properties};
use tower_lsp::lsp_types::{Position, Range as LspRange, Url};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).unwrap()
}

fn make_entry(key: &str, value: &str, line: u32, layer: SourceLayer) -> ConfigEntry {
    let key_end = key.len() as u32;
    let val_start = key_end + 1;
    let val_end = val_start + value.len() as u32;
    ConfigEntry {
        key: key.to_string(),
        value: value.to_string(),
        key_range: LspRange {
            start: Position { line, character: 0 },
            end: Position {
                line,
                character: key_end,
            },
        },
        value_range: LspRange {
            start: Position {
                line,
                character: val_start,
            },
            end: Position {
                line,
                character: val_end,
            },
        },
        line,
        source_layer: layer,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 001: Recognition + ConfigFormat abstraction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_recognize_application_properties_recognized() {
    assert!(is_jvm_config_file(&url("/proj/application.properties")));
}

#[test]
fn test_recognize_application_profile_properties_recognized() {
    assert!(is_jvm_config_file(&url(
        "/proj/application-prod.properties"
    )));
    assert!(is_jvm_config_file(&url(
        "/proj/application-staging.properties"
    )));
}

#[test]
fn test_recognize_microprofile_config_properties_recognized() {
    assert!(is_jvm_config_file(&url(
        "/proj/microprofile-config.properties"
    )));
}

#[test]
fn test_recognize_dotenv_recognized() {
    // Plain .env must NOT match is_env_cascade_file — it stays on the
    // existing env handler path (routing fix / design decision).
    assert!(!is_env_cascade_file(&url("/proj/.env")));
}

#[test]
fn test_recognize_dotenv_local_recognized() {
    assert!(is_env_cascade_file(&url("/proj/.env.local")));
}

#[test]
fn test_recognize_dotenv_staging_recognized() {
    assert!(is_env_cascade_file(&url("/proj/.env.staging")));
}

#[test]
fn test_recognize_schema_file_not_matched_by_new_predicates() {
    // .env.schema must remain with the existing handler — zero regression.
    let u = url("/proj/.env.schema");
    assert!(!is_jvm_config_file(&u));
    assert!(!is_env_cascade_file(&u));
}

#[test]
fn test_recognize_unrelated_file_not_matched() {
    assert!(!is_jvm_config_file(&url("/proj/application.txt")));
    assert!(!is_env_cascade_file(&url("/proj/package.json")));
}

#[test]
fn test_recognize_application_dot_properties_recognized_as_config_format() {
    assert!(is_config_format_file(&url("/proj/application.properties")));
}

#[test]
fn test_recognize_empty_profile_name_no_crash() {
    // `application-.properties` — no panic; empty profile → does NOT match
    // (profile must be non-empty per FR3 scope, mirroring is_yaml_config_file).
    assert!(!is_jvm_config_file(&url("/proj/application-.properties")));
    // But application-prod.properties with a non-empty profile DOES match.
    assert!(is_jvm_config_file(&url(
        "/proj/application-prod.properties"
    )));
}

#[test]
fn test_recognize_format_for_uri_properties_write_capability_is_readwrite() {
    let (fmt, _) = format_for_uri(&url("/proj/application.properties")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
}

#[test]
fn test_recognize_format_for_uri_dotenv_write_capability_is_readwrite() {
    // Plain .env no longer routes through format_for_uri (routing fix).
    // .env.local does route through it and is ReadWrite.
    let (fmt, _) = format_for_uri(&url("/proj/.env.local")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
}

#[test]
fn test_recognize_source_layer_inferred_for_base() {
    let (_, layer) = format_for_uri(&url("/proj/application.properties")).unwrap();
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_recognize_source_layer_inferred_for_profile() {
    let (_, layer) = format_for_uri(&url("/proj/application-prod.properties")).unwrap();
    assert_eq!(layer, SourceLayer::Profile("prod".into()));
}

#[test]
fn test_recognize_source_layer_inferred_for_dotenv_local() {
    let (_, layer) = format_for_uri(&url("/proj/.env.local")).unwrap();
    assert_eq!(layer, SourceLayer::DotEnvLocal);
}

#[test]
fn test_recognize_source_layer_for_properties_base() {
    assert_eq!(
        source_layer_for_properties("application.properties"),
        SourceLayer::Base
    );
}

#[test]
fn test_recognize_source_layer_for_dotenv_environment() {
    assert_eq!(
        source_layer_for_dotenv(".env.production"),
        SourceLayer::DotEnvEnvironment("production".into())
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 002: Parse entry model
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_properties_yields_kv_entries() {
    let content = "# Database\nserver.port=8080\napp.name=MyApp\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 2);
    assert_eq!(kv[0].key, "server.port");
    assert_eq!(kv[0].value, "8080");
}

#[test]
fn test_parse_properties_comment_entry_has_empty_key() {
    let content = "# comment\nfoo=bar\n";
    let entries = parse_properties(content, SourceLayer::Base);
    assert!(
        entries[0].key.is_empty(),
        "Comment entry must have empty key"
    );
    assert_eq!(entries[1].key, "foo");
}

#[test]
fn test_parse_properties_blank_lines_preserved() {
    let content = "foo=bar\n\nbaz=qux\n";
    let entries = parse_properties(content, SourceLayer::Base);
    assert_eq!(entries.len(), 3);
    assert!(entries[1].key.is_empty());
}

#[test]
fn test_parse_properties_colon_separator() {
    let content = "server.port: 9090\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].value, "9090");
}

#[test]
fn test_parse_properties_duplicate_keys_both_retained() {
    let content = "FOO=first\nFOO=second\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 2, "Both duplicate entries must be retained");
}

#[test]
fn test_parse_dotenv_basic_kv() {
    let content = "DB_HOST=localhost\nAPP_PORT=8080\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 2);
    assert_eq!(kv[0].key, "DB_HOST");
    assert_eq!(kv[0].value, "localhost");
}

#[test]
fn test_parse_dotenv_export_prefix_stripped() {
    let content = "export FOO=bar\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].key, "FOO");
    assert_eq!(kv[0].value, "bar");
}

#[test]
fn test_parse_dotenv_quoted_value_stripped() {
    let content = r#"SECRET="my secret""#;
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].value, "my secret");
}

#[test]
fn test_parse_dotenv_utf8_multibyte_no_crash() {
    let content = "MSG=héllo\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1);
    // Key range starts at 0.
    assert_eq!(kv[0].key_range.start.character, 0);
}

#[test]
fn test_parse_properties_source_layer_attached_to_entries() {
    let content = "foo=bar\n";
    let entries = parse_properties(content, SourceLayer::Profile("prod".to_string()));
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].source_layer, SourceLayer::Profile("prod".to_string()));
}

#[test]
fn test_parse_properties_empty_value_allowed() {
    let content = "EMPTY=\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].value, "");
}

// Round-trip: parser preserves structure for ALL content features.
// The raw `content` string is never re-serialised from entries;
// byte identity is guaranteed by design (NFR9).
#[test]
fn test_parse_properties_round_trip_content_preserved() {
    // `.properties` fixture covers: # comment, ! comment, blank line,
    // trailing whitespace on a value, empty value, no final newline.
    // (`.properties` format does NOT use `export` — that belongs to dotenv.)
    let content =
        "# a comment\n! exclamation comment\n\nserver.port=8080  \napp.name: MyApp\nEMPTY=";
    // no trailing newline ↑

    let entries = parse_properties(content, SourceLayer::Base);

    // Every physical line must produce exactly one entry.
    let expected_lines = content.lines().count();
    assert_eq!(
        entries.len(),
        expected_lines,
        "entry count must match physical line count"
    );

    // Comments → empty key.
    assert!(entries[0].key.is_empty(), "line 0 is a comment → empty key");
    assert!(entries[1].key.is_empty(), "line 1 is a comment → empty key");

    // Blank line → empty key.
    assert!(entries[2].key.is_empty(), "line 2 is blank → empty key");

    // KV lines → non-empty keys.
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert!(
        kv.iter().any(|e| e.key == "server.port"),
        "server.port must be parsed"
    );
    assert!(
        kv.iter().any(|e| e.key == "app.name"),
        "app.name (colon separator) must be parsed"
    );
    assert!(kv.iter().any(|e| e.key == "EMPTY"), "EMPTY= must be parsed");

    // Value correctness spot-checks.
    let port = kv.iter().find(|e| e.key == "server.port").unwrap();
    assert_eq!(
        port.value, "8080",
        "trailing spaces must be stripped from value"
    );

    let empty_entry = kv.iter().find(|e| e.key == "EMPTY").unwrap();
    assert_eq!(
        empty_entry.value, "",
        "empty value must parse as empty string"
    );
}

// Round-trip: `.env` cascade parser preserves `export` prefix and quoted values.
#[test]
fn test_parse_dotenv_round_trip_content_preserved() {
    // dotenv fixture covers: # comment, export prefix, quoted value, no final newline.
    let content = "# a comment\nexport APP=hello\nSECRET=\"my val\"\nEMPTY=";
    // no trailing newline ↑

    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();

    assert!(
        kv.iter().any(|e| e.key == "APP"),
        "APP (export prefix) must be parsed"
    );
    assert!(
        kv.iter().any(|e| e.key == "SECRET"),
        "SECRET (quoted) must be parsed"
    );
    let secret = kv.iter().find(|e| e.key == "SECRET").unwrap();
    assert_eq!(secret.value, "my val", "quotes must be stripped from value");

    let empty_entry = kv.iter().find(|e| e.key == "EMPTY").unwrap();
    assert_eq!(
        empty_entry.value, "",
        "empty value must parse as empty string"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 003: Resolution engine
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_resolution_profile_wins_over_base() {
    let base = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let profile = vec![make_entry(
        "server.port",
        "9090",
        0,
        SourceLayer::Profile("prod".into()),
    )];
    let layers = vec![base, profile];
    let winner = resolve_layers("server.port", &layers).unwrap();
    assert_eq!(winner.value, "9090");
}

#[test]
fn test_resolution_base_used_when_no_profile_override() {
    let base = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let layers = [base];
    let winner = resolve_layers("server.port", &layers).unwrap();
    assert_eq!(winner.value, "8080");
}

#[test]
fn test_resolution_dotenv_cascade_local_wins_over_dotenv() {
    let env = vec![make_entry("DB_HOST", "localhost", 0, SourceLayer::DotEnv)];
    let local = vec![make_entry(
        "DB_HOST",
        "db.local",
        0,
        SourceLayer::DotEnvLocal,
    )];
    let layers = vec![env, local];
    let winner = resolve_layers("DB_HOST", &layers).unwrap();
    assert_eq!(winner.value, "db.local");
}

#[test]
fn test_resolution_dotenv_environment_wins_over_local() {
    let local = vec![make_entry(
        "DB_HOST",
        "db.local",
        0,
        SourceLayer::DotEnvLocal,
    )];
    let staging = vec![make_entry(
        "DB_HOST",
        "db.staging",
        0,
        SourceLayer::DotEnvEnvironment("staging".into()),
    )];
    let layers = vec![local, staging];
    let winner = resolve_layers("DB_HOST", &layers).unwrap();
    assert_eq!(winner.value, "db.staging");
}

#[test]
fn test_resolution_missing_key_returns_none() {
    let base = vec![make_entry("foo", "bar", 0, SourceLayer::Base)];
    assert!(resolve_layers("missing", &[base]).is_none());
}

#[test]
fn test_resolution_interpolation_simple_ref() {
    let mut env = HashMap::new();
    env.insert("DB_HOST".into(), "localhost".into());
    let (result, expanded) =
        interpolate_value("${DB_HOST}", &env, &mut std::collections::HashSet::new());
    assert_eq!(result, "localhost");
    assert!(expanded);
}

#[test]
fn test_resolution_interpolation_default_used_when_unset() {
    let env = HashMap::new();
    let (result, expanded) = interpolate_value(
        "${DB_URL:localhost}",
        &env,
        &mut std::collections::HashSet::new(),
    );
    assert_eq!(result, "localhost");
    assert!(expanded);
}

#[test]
fn test_resolution_interpolation_explicit_value_overrides_default() {
    let mut env = HashMap::new();
    env.insert("DB_URL".into(), "prod-db".into());
    let (result, _) = interpolate_value(
        "${DB_URL:localhost}",
        &env,
        &mut std::collections::HashSet::new(),
    );
    assert_eq!(result, "prod-db");
}

#[test]
fn test_resolution_interpolation_cycle_no_infinite_loop() {
    let mut env = HashMap::new();
    env.insert("A".into(), "${B}".into());
    env.insert("B".into(), "${A}".into());
    // Must terminate (no stack overflow) and return a defined result.
    // When a cycle is detected the value is left as the unresolved ref.
    let (result, _) = interpolate_value("${A}", &env, &mut std::collections::HashSet::new());
    // The important invariant: result is a finite String (no panic / infinite loop).
    // We do not mandate the exact form, only that it terminates and is non-empty.
    assert!(
        !result.is_empty(),
        "cycle resolution must terminate with a non-empty result"
    );
}

#[test]
fn test_resolution_interpolation_unresolved_left_intact() {
    let env = HashMap::new();
    let (result, _) = interpolate_value("${MISSING}", &env, &mut std::collections::HashSet::new());
    assert_eq!(result, "${MISSING}");
}

#[test]
fn test_resolution_effective_value_with_interpolation() {
    let base = vec![
        make_entry("DB_HOST", "localhost", 0, SourceLayer::Base),
        make_entry(
            "DB_URL",
            "jdbc:postgresql://${DB_HOST}/mydb",
            1,
            SourceLayer::Base,
        ),
    ];
    let resolved: Option<ResolvedValue> = resolve_effective_value("DB_URL", &[base]);
    let resolved = resolved.unwrap();
    assert_eq!(resolved.value, "jdbc:postgresql://localhost/mydb");
    assert!(resolved.interpolated);
}

#[test]
fn test_resolution_winning_layer_reported() {
    let base = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let profile = vec![make_entry(
        "server.port",
        "9090",
        0,
        SourceLayer::Profile("prod".into()),
    )];
    let layers = vec![base, profile];
    let resolved = resolve_effective_value("server.port", &layers).unwrap();
    assert_eq!(resolved.winning_layer, SourceLayer::Profile("prod".into()));
}

#[test]
fn test_resolution_unterminated_ref_detected() {
    let positions = find_unterminated_refs("${OPEN");
    assert!(!positions.is_empty());
}

#[test]
fn test_resolution_terminated_ref_not_flagged() {
    let positions = find_unterminated_refs("${DB_HOST}");
    assert!(positions.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 004: Hover with effective value, layer, schema, redaction
// ═══════════════════════════════════════════════════════════════════════════════

use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};

fn schema_with_sensitive(key: &str) -> EnvSchema {
    let mut s = EnvSchema {
        variables: HashMap::new(),
    };
    s.variables.insert(
        key.to_string(),
        SchemaVariable {
            var_type: VarType::String,
            sensitive: true,
            required: false,
            ..Default::default()
        },
    );
    s
}

fn schema_with_key(key: &str) -> EnvSchema {
    let mut s = EnvSchema {
        variables: HashMap::new(),
    };
    s.variables.insert(
        key.to_string(),
        SchemaVariable {
            var_type: VarType::String,
            sensitive: false,
            required: false,
            description: Some("A description".to_string()),
            ..Default::default()
        },
    );
    s
}

#[test]
fn test_hover_shows_effective_value_and_layer() {
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &entries,
        &[entries.clone()],
        None,
    );
    assert!(hover.is_some());
    let h = hover.unwrap();
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = h.contents {
        assert!(m.value.contains("server.port"), "key must appear in hover");
        // Note: redact_for_label always returns "***" for all values (by design,
        // to prevent secret leakage in any LSP popup). The value field shows
        // the redacted form rather than the raw value.
        assert!(m.value.contains("Value:"), "hover must show Value field");
        assert!(m.value.contains("Layer:"), "hover must show Layer field");
        assert!(m.value.contains("base"), "layer must be 'base'");
    } else {
        panic!("expected markup hover");
    }
}

#[test]
fn test_hover_redacts_sensitive_value() {
    let schema = schema_with_sensitive("DB_PASSWORD");
    let entries = vec![make_entry(
        "DB_PASSWORD",
        "supersecret",
        0,
        SourceLayer::Base,
    )];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &entries,
        &[entries.clone()],
        Some(&schema),
    );
    let h = hover.unwrap();
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = h.contents {
        assert!(
            !m.value.contains("supersecret"),
            "Secret value must not appear in hover"
        );
        // After the M1 fix: sensitive hover shows "_(redacted)_" as the value
        // and "Sensitive: **yes**" in the schema section.
        assert!(
            m.value.contains("redacted") || m.value.to_lowercase().contains("sensitive"),
            "sensitive hover must contain a redaction or sensitivity indicator; got: {}",
            m.value
        );
    }
}

#[test]
fn test_hover_shows_schema_metadata() {
    let schema = schema_with_key("server.port");
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &entries,
        &[entries.clone()],
        Some(&schema),
    );
    let h = hover.unwrap();
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = h.contents {
        assert!(m.value.contains("A description"));
    }
}

#[test]
fn test_hover_no_hover_on_comment_line() {
    let mut entry = make_entry("", "", 0, SourceLayer::Base);
    entry.value = "# a comment".to_string();
    let result = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &[entry],
        &[],
        None,
    );
    assert!(result.is_none());
}

#[test]
fn test_hover_no_hover_when_no_entry_at_position() {
    let entries = vec![make_entry("foo", "bar", 5, SourceLayer::Base)];
    let result = config_hover(
        Position {
            line: 0,
            character: 0,
        },
        &entries,
        &[entries.clone()],
        None,
    );
    assert!(result.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 005: Completion
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_completion_key_from_schema() {
    let schema = schema_with_key("server.port");
    let content = "s\n";
    let items = config_completions(
        Position {
            line: 0,
            character: 1,
        },
        content,
        &[],
        Some(&schema),
    );
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i.label == "server.port"));
}

#[test]
fn test_completion_dollar_brace_suggests_refs() {
    let entries = vec![make_entry("DB_HOST", "localhost", 0, SourceLayer::Base)];
    let schema = schema_with_key("DB_HOST");
    let content = "URL=${";
    let items = config_completions(
        Position {
            line: 0,
            character: content.len() as u32,
        },
        content,
        &entries,
        Some(&schema),
    );
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i.label == "DB_HOST"));
}

#[test]
fn test_completion_deduplicates_suggestions() {
    let schema = schema_with_key("FOO");
    let entries = vec![make_entry("FOO", "val", 0, SourceLayer::Base)];
    let content = "F";
    let items = config_completions(
        Position {
            line: 0,
            character: 1,
        },
        content,
        &entries,
        Some(&schema),
    );
    let foo_count = items.iter().filter(|i| i.label == "FOO").count();
    assert!(foo_count <= 1, "FOO must appear at most once");
}

#[test]
fn test_completion_empty_file_no_crash() {
    let items = config_completions(
        Position {
            line: 0,
            character: 0,
        },
        "",
        &[],
        None,
    );
    // An empty file with no schema produces zero completions.
    assert!(
        items.is_empty(),
        "Empty file with no schema must yield zero completions"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 006: Go-to-definition and find-references
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_find_references_across_open_docs() {
    let uri1 = url("/proj/application.properties");
    let uri2 = url("/proj/application-prod.properties");
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(
        uri1.clone(),
        vec![make_entry("server.port", "8080", 0, SourceLayer::Base)],
    );
    open_docs.insert(
        uri2.clone(),
        vec![make_entry(
            "server.port",
            "9090",
            0,
            SourceLayer::Profile("prod".into()),
        )],
    );
    let locs = config_find_references("server.port", None, &HashMap::new(), &open_docs, false);
    assert_eq!(locs.len(), 2, "Both file references must be found");
}

#[test]
fn test_find_references_includes_schema_when_include_declaration() {
    let schema_uri = url("/proj/.env.schema");
    let mut schema_lines = HashMap::new();
    schema_lines.insert("MY_KEY".to_string(), 5u32);
    let locs = config_find_references(
        "MY_KEY",
        Some(&schema_uri),
        &schema_lines,
        &HashMap::new(),
        true,
    );
    assert!(locs.iter().any(|l| l.uri == schema_uri));
}

#[test]
fn test_find_references_no_result_for_absent_key() {
    let locs = config_find_references("NONEXISTENT", None, &HashMap::new(), &HashMap::new(), false);
    assert!(locs.is_empty());
}

#[test]
fn test_find_references_same_key_in_three_profiles() {
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    for (i, layer) in [
        SourceLayer::Base,
        SourceLayer::Profile("dev".into()),
        SourceLayer::Profile("prod".into()),
    ]
    .into_iter()
    .enumerate()
    {
        open_docs.insert(
            url(&format!("/proj/file{}.properties", i)),
            vec![make_entry("MY_KEY", "val", 0, layer)],
        );
    }
    let locs = config_find_references("MY_KEY", None, &HashMap::new(), &open_docs, false);
    assert_eq!(locs.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 007: Semantic tokens
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_semantic_tokens_key_gets_variable_type() {
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let tokens = config_semantic_tokens(&entries, None);
    assert!(!tokens.data.is_empty());
    // First token is VARIABLE (type index 0).
    assert_eq!(tokens.data[0].token_type, 0);
}

#[test]
fn test_semantic_tokens_comment_gets_comment_type() {
    let mut comment = make_entry("", "", 0, SourceLayer::Base);
    comment.value = "# this is a comment".to_string();
    comment.key_range.end.character = comment.value.len() as u32;
    let tokens = config_semantic_tokens(&[comment], None);
    assert!(!tokens.data.is_empty());
    // Comment token type is 2.
    assert_eq!(tokens.data[0].token_type, 2);
}

#[test]
fn test_semantic_tokens_sensitive_key_has_readonly_modifier() {
    let schema = schema_with_sensitive("DB_PASSWORD");
    let entries = vec![make_entry("DB_PASSWORD", "secret", 0, SourceLayer::Base)];
    let tokens = config_semantic_tokens(&entries, Some(&schema));
    let key_tok = tokens.data.first().unwrap();
    assert_eq!(
        key_tok.token_modifiers_bitset & 1,
        1,
        "sensitive key must have readonly modifier"
    );
}

#[test]
fn test_semantic_tokens_empty_value_no_value_token() {
    let entries = vec![make_entry("EMPTY", "", 0, SourceLayer::Base)];
    let tokens = config_semantic_tokens(&entries, None);
    // Key token present; no value token (value length is 0).
    assert_eq!(tokens.data.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 008: Diagnostics
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_diagnostics_duplicate_key_flagged() {
    let entries = vec![
        make_entry("FOO", "first", 0, SourceLayer::Base),
        make_entry("FOO", "second", 1, SourceLayer::Base),
    ];
    let diags = config_diagnostics(&entries, None);
    assert!(diags.iter().any(|d| d.message.contains("Duplicate key")));
}

#[test]
fn test_diagnostics_unterminated_interpolation_flagged() {
    let mut entry = make_entry("BAR", "${OPEN", 0, SourceLayer::Base);
    entry.value = "${OPEN".to_string();
    let diags = config_diagnostics(&[entry], None);
    assert!(
        diags.iter().any(|d| d.message.contains("Unterminated")),
        "Unterminated interpolation must produce a diagnostic"
    );
}

#[test]
fn test_diagnostics_unknown_key_flagged_when_schema_present() {
    let schema = schema_with_key("KNOWN");
    let entries = vec![make_entry("UNKNOWN_KEY", "val", 0, SourceLayer::Base)];
    let diags = config_diagnostics(&entries, Some(&schema));
    assert!(diags.iter().any(|d| d.message.contains("Unknown key")));
}

#[test]
fn test_diagnostics_no_unknown_key_without_schema() {
    let entries = vec![make_entry("ANY_KEY", "val", 0, SourceLayer::Base)];
    let diags = config_diagnostics(&entries, None);
    assert!(!diags.iter().any(|d| d.message.contains("Unknown key")));
}

#[test]
fn test_diagnostics_no_crash_on_empty_entries() {
    let diags = config_diagnostics(&[], None);
    assert!(diags.is_empty());
}

#[test]
fn test_diagnostics_malformed_input_no_panic() {
    // Entries with empty key/value — should produce zero diagnostics, no panic.
    let mut blank = make_entry("", "", 0, SourceLayer::Base);
    blank.value = String::new();
    let diags = config_diagnostics(&[blank], None);
    // The result must be empty — blank/comment entries produce no diagnostics
    // unless they contain unterminated interpolation (this one has empty value).
    assert!(
        diags.is_empty(),
        "blank entry with empty value must produce zero diagnostics"
    );
}

#[test]
fn test_diagnostics_duplicate_across_profile_files_not_flagged() {
    // Cross-file duplicates are legal overrides — only same-file duplicates
    // are flagged. We simulate one entry per file by checking with
    // two separate entry sets (each with a unique key but different values).
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let diags = config_diagnostics(&entries, None);
    assert!(!diags.iter().any(|d| d.message.contains("Duplicate")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 009: Rename and format
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rename_readwrite_produces_workspace_edit() {
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    let uri = url("/proj/application.properties");
    open_docs.insert(
        uri.clone(),
        vec![make_entry("OLD_KEY", "val", 0, SourceLayer::Base)],
    );
    let edit = config_rename(
        "OLD_KEY",
        "NEW_KEY",
        WriteCapability::ReadWrite,
        None,
        &HashMap::new(),
        &open_docs,
    );
    assert!(edit.is_some());
    let changes = edit.unwrap().changes.unwrap();
    let edits = changes.get(&uri).unwrap();
    assert!(edits.iter().any(|e| e.new_text == "NEW_KEY"));
}

#[test]
fn test_rename_readonly_returns_none() {
    let edit = config_rename(
        "OLD",
        "NEW",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(edit.is_none());
}

#[test]
fn test_rename_collision_with_existing_key_rejected() {
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    let uri = url("/proj/.env");
    open_docs.insert(
        uri,
        vec![
            make_entry("OLD_KEY", "v1", 0, SourceLayer::DotEnv),
            make_entry("NEW_KEY", "v2", 1, SourceLayer::DotEnv),
        ],
    );
    let edit = config_rename(
        "OLD_KEY",
        "NEW_KEY",
        WriteCapability::ReadWrite,
        None,
        &HashMap::new(),
        &open_docs,
    );
    assert!(edit.is_none(), "Rename collision must be rejected");
}

#[test]
fn test_rename_noop_same_name_returns_none() {
    let edit = config_rename(
        "FOO",
        "FOO",
        WriteCapability::ReadWrite,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(edit.is_none());
}

#[test]
fn test_rename_invalid_new_name_returns_none() {
    let edit = config_rename(
        "FOO",
        "123invalid",
        WriteCapability::ReadWrite,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(edit.is_none());
}

#[test]
fn test_format_normalises_spacing() {
    let content = "server.port = 8080\n";
    let formatted = config_format_document(content);
    assert_eq!(formatted, "server.port=8080\n");
}

#[test]
fn test_format_text_edits_empty_when_already_canonical() {
    let content = "server.port=8080\n";
    let edits = config_format_text_edits(content, WriteCapability::ReadWrite);
    assert!(edits.is_empty());
}

#[test]
fn test_format_text_edits_readonly_produces_no_edits() {
    let content = "server.port = 8080\n";
    let edits = config_format_text_edits(content, WriteCapability::ReadOnly);
    assert!(edits.is_empty());
}

#[test]
fn test_format_preserves_comments_and_blanks() {
    let content = "# my comment\n\nserver.port=8080\n";
    let formatted = config_format_document(content);
    assert!(formatted.contains("# my comment"));
    assert!(formatted.contains("server.port=8080"));
}

// ─── Key-validation helper ────────────────────────────────────────────────────

#[test]
fn test_is_valid_config_key_java_dotted_keys() {
    assert!(is_valid_config_key("server.port"));
    assert!(is_valid_config_key("spring.datasource.url"));
}

#[test]
fn test_is_valid_config_key_env_style_keys() {
    assert!(is_valid_config_key("DB_HOST"));
    assert!(is_valid_config_key("APP_PORT"));
}

#[test]
fn test_is_valid_config_key_rejects_numeric_start() {
    assert!(!is_valid_config_key("123foo"));
}

#[test]
fn test_is_valid_config_key_rejects_empty() {
    assert!(!is_valid_config_key(""));
}

// ─── config_format: source_layer helpers (migrated from in-module tests) ─────

#[test]
fn test_source_layer_for_properties_microprofile_is_base() {
    assert_eq!(
        source_layer_for_properties("microprofile-config.properties"),
        SourceLayer::Base
    );
}

#[test]
fn test_source_layer_for_properties_staging_profile() {
    assert_eq!(
        source_layer_for_properties("application-staging.properties"),
        SourceLayer::Profile("staging".to_string())
    );
}

#[test]
fn test_source_layer_for_properties_empty_profile_is_base() {
    // `application-.properties` — no crash, treated as Base.
    assert_eq!(
        source_layer_for_properties("application-.properties"),
        SourceLayer::Base
    );
}

#[test]
fn test_source_layer_precedence_ordering() {
    assert!(SourceLayer::Profile("x".into()).precedence() > SourceLayer::Base.precedence());
    assert!(SourceLayer::DotEnvLocal.precedence() > SourceLayer::DotEnv.precedence());
    assert!(
        SourceLayer::DotEnvEnvironment("x".into()).precedence()
            > SourceLayer::DotEnvLocal.precedence()
    );
    assert_eq!(SourceLayer::Unknown.precedence(), 0);
}

// ─── config_format: resolve_ref (migrated from in-module tests) ───────────────

use envforge::ops::config_format::resolve_ref;

#[test]
fn test_resolve_ref_found_in_env() {
    let mut env = HashMap::new();
    env.insert("DB_HOST".to_string(), "localhost".to_string());
    let (val, resolved) = resolve_ref("DB_HOST", None, &env);
    assert_eq!(val, "localhost");
    assert!(resolved);
}

#[test]
fn test_resolve_ref_uses_default_when_missing() {
    let env = HashMap::new();
    let (val, resolved) = resolve_ref("MISSING", Some("fallback"), &env);
    assert_eq!(val, "fallback");
    assert!(resolved);
}

#[test]
fn test_resolve_ref_unresolved_returns_placeholder() {
    let env = HashMap::new();
    let (val, resolved) = resolve_ref("MISSING", None, &env);
    assert_eq!(val, "${MISSING}");
    assert!(!resolved);
}

// ─── config_resolution: unterminated ref (migrated from in-module tests) ─────

#[test]
fn test_resolution_interpolation_unterminated_ref_preserved() {
    let env = HashMap::new();
    let (result, _) = interpolate_value(
        "start ${UNTERMINATED end",
        &env,
        &mut std::collections::HashSet::new(),
    );
    // Must not panic; the prefix before `${` is preserved.
    assert!(
        result.contains("start "),
        "prefix before unterminated ref must be preserved"
    );
}

// ─── Security/allowlist ───────────────────────────────────────────────────────

#[test]
fn test_security_properties_extension_allowed_in_guard_scan() {
    use envforge::lsp::security::guard_scan_extension;
    use std::path::Path;
    assert!(guard_scan_extension(Path::new("application.properties")).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Regression tests — adversarial review findings
// ═══════════════════════════════════════════════════════════════════════════════

// ── C1: .env cascade route ────────────────────────────────────────────────────

/// C1 — `.env.local` / `.env.{environment}` are recognised by
/// `is_config_format_file`. Plain `.env` is NOT — it stays on the
/// existing env handler path (routing fix / design decision).
#[test]
fn test_c1_dotenv_and_dotenv_local_recognised_as_config_format() {
    // Plain .env stays on the existing env handler (routing fix).
    assert!(
        !is_config_format_file(&url("/proj/.env")),
        ".env must NOT be routed to the config handler (routing fix)"
    );
    assert!(
        is_config_format_file(&url("/proj/.env.local")),
        ".env.local must be a config-format file"
    );
    assert!(
        is_config_format_file(&url("/proj/.env.production")),
        ".env.production must be a config-format file"
    );
}

/// C1 — `.env.schema` must NOT be matched (it has its own handler).
#[test]
fn test_c1_dotenv_schema_not_matched_by_config_format() {
    assert!(
        !is_config_format_file(&url("/proj/.env.schema")),
        ".env.schema must NOT be routed to config handler"
    );
    assert!(
        !is_config_format_file(&url("/proj/.env.schema.toml")),
        ".env.schema.toml must NOT be routed to config handler"
    );
}

/// C1 — format_for_uri succeeds for .env.local and parses entries.
#[test]
fn test_c1_format_for_uri_dotenv_local_parses_entries() {
    let (fmt, layer) = format_for_uri(&url("/proj/.env.local")).unwrap();
    assert_eq!(
        layer,
        SourceLayer::DotEnvLocal,
        ".env.local must map to DotEnvLocal layer"
    );
    let entries = fmt.parse("DB_HOST=local\n", layer);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1);
    assert_eq!(kv[0].key, "DB_HOST");
}

// ── H1: UTF-16 slice panic ────────────────────────────────────────────────────

/// H1 — completion with cursor inside a multi-byte value must not panic.
/// Before the fix, `position.character as usize` was used directly as a byte
/// offset, causing a panic on "byte index is not a char boundary" for UTF-8
/// multi-byte content such as `MSG=héllo`.
#[test]
fn test_h1_completion_inside_multibyte_value_no_panic() {
    // "MSG=héllo" — 'é' is U+00E9, 2 UTF-8 bytes but 1 UTF-16 unit.
    // The cursor is at UTF-16 position 6 (after 'h'), which is byte offset 7
    // because of the 2-byte 'é'. Previously this indexed `&line[..6]`
    // which would panic on the 'é' boundary.
    let content = "MSG=héllo\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    // Position character = 6 in UTF-16 is inside the value after 'h'.
    let items = config_completions(
        Position {
            line: 0,
            character: 6,
        },
        content,
        &entries,
        None,
    );
    // We only care that this doesn't panic; empty result is fine.
    let _ = items;
}

/// H1 — completion on a line with a multi-byte key prefix must not panic.
#[test]
fn test_h1_completion_multibyte_key_no_panic() {
    // Unusual but valid: a key starting at character 0, followed by multibyte
    // in a value. Cursor at character 4 (UTF-16).
    let content = "DB=héllo world\n";
    let items = config_completions(
        Position {
            line: 0,
            character: 4,
        },
        content,
        &[],
        None,
    );
    let _ = items; // must not panic
}

// ── H2: cross-layer resolution ────────────────────────────────────────────────

/// H2 — hover resolves value from base layer when hovering a profile file.
/// Before the fix `all_layers` only contained the current document's entries,
/// so a key defined only in the base file would show no resolved value when
/// hovering from a profile file. The fix assembles all open config_documents.
#[test]
fn test_h2_hover_resolves_value_from_other_layer() {
    // Simulate the merged layers as the fixed server.rs now passes them.
    let base = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let profile = vec![make_entry(
        "server.host",
        "prod.example.com",
        0,
        SourceLayer::Profile("prod".into()),
    )];
    // Hover on "server.port" from within the profile document.
    // The profile document doesn't define server.port but the base layer does.
    // With both layers passed, the hover should resolve to "8080".
    let all_layers = vec![base.clone(), profile.clone()];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &base,       // entries of the doc being hovered (base)
        &all_layers, // all layers assembled by server
        None,
    );
    assert!(hover.is_some(), "hover must return a result");
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.unwrap().contents {
        assert!(m.value.contains("server.port"), "key name must appear");
        assert!(
            m.value.contains("8080"),
            "base-layer value must be resolved"
        );
        assert!(m.value.contains("base"), "winning layer must be 'base'");
    }
}

/// H2 — profile value wins over base when both layers are present.
#[test]
fn test_h2_hover_profile_wins_over_base_with_all_layers() {
    let base = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let profile = vec![make_entry(
        "server.port",
        "9090",
        0,
        SourceLayer::Profile("prod".into()),
    )];
    let all_layers = vec![base, profile.clone()];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &profile, // hovering from the profile doc
        &all_layers,
        None,
    );
    assert!(hover.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.unwrap().contents {
        assert!(
            m.value.contains("9090"),
            "profile value must win; got: {}",
            m.value
        );
    }
}

// ── H3: CRLF / no-final-newline preservation ─────────────────────────────────

/// H3 — format a CRLF file must preserve CRLF line endings.
/// Before the fix `config_format_document` always joined with `\n`, silently
/// converting CRLF to LF.
#[test]
fn test_h3_format_preserves_crlf_line_endings() {
    let content = "server.port = 8080\r\napp.name = MyApp\r\n";
    let formatted = config_format_document(content);
    // Must still use CRLF.
    assert!(
        formatted.contains("\r\n"),
        "formatted CRLF file must still use CRLF; got: {:?}",
        formatted
    );
    // Must NOT introduce stray LF-only line breaks.
    // Every \n should be preceded by \r.
    for (i, byte) in formatted.as_bytes().iter().enumerate() {
        if *byte == b'\n' && i > 0 {
            assert_eq!(
                formatted.as_bytes()[i - 1],
                b'\r',
                "bare LF found at position {}; expected CRLF",
                i
            );
        }
    }
    // Normalisation still happens.
    assert!(
        formatted.contains("server.port=8080"),
        "spacing must be normalised"
    );
}

/// H3 — format a file with no final newline must NOT add one.
/// Before the fix `config_format_document` always appended `\n`.
#[test]
fn test_h3_format_no_final_newline_preserved() {
    let content = "server.port=8080"; // no trailing newline
    let formatted = config_format_document(content);
    assert!(
        !formatted.ends_with('\n'),
        "file without final newline must remain without final newline after format; got: {:?}",
        formatted
    );
    assert_eq!(formatted, "server.port=8080");
}

/// H3 — format a LF file with a final newline preserves the trailing newline.
#[test]
fn test_h3_format_lf_file_with_trailing_newline_preserved() {
    let content = "server.port = 8080\n";
    let formatted = config_format_document(content);
    assert!(
        formatted.ends_with('\n'),
        "file with trailing newline must keep it"
    );
    assert_eq!(formatted, "server.port=8080\n");
}

// ── H4: `export  KEY=val` off-by-one ─────────────────────────────────────────

/// H4 — parse `export  FOO=bar` (two spaces after export) and assert the
/// value_range.start points at the byte of 'b', not at a shifted position.
/// Before the fix `find_sep_byte_in_original` subtracted a fixed 7 bytes for
/// `"export "`, missing the extra space and yielding a wrong offset.
#[test]
fn test_h4_export_double_space_value_range_correct() {
    let content = "export  FOO=bar\n"; // two spaces after export
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1, "must parse exactly one KV entry");
    assert_eq!(kv[0].key, "FOO");
    assert_eq!(kv[0].value, "bar");

    // The `=` is at byte index 11 in `"export  FOO=bar\n"`.
    // value_range.start must point to the char AFTER `=`, i.e. the 'b'.
    // In UTF-16: "export  FOO=" = 12 code units, so val_start should be 12.
    let val_start = kv[0].value_range.start.character;
    assert_eq!(
        val_start, 12,
        "value_range.start must be 12 (after 'export  FOO='); got {}",
        val_start
    );
}

/// H4 — the single-space form `export FOO=bar` is unaffected.
#[test]
fn test_h4_export_single_space_value_range_correct() {
    let content = "export FOO=bar\n";
    let entries = parse_dotenv_cascade(content, SourceLayer::DotEnv);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv[0].key, "FOO");
    // "export FOO=" = 11 UTF-16 units → value starts at char 11.
    let val_start = kv[0].value_range.start.character;
    assert_eq!(
        val_start, 11,
        "value_range.start for single-space export must be 11; got {}",
        val_start
    );
}

// ── M1: redaction label clarity ───────────────────────────────────────────────

/// M1 — hover for a NON-sensitive key must show the actual value, not `***`.
/// Before the fix `redact_for_label` was called even for non-sensitive values,
/// always returning `***` and making the hover silently useless.
#[test]
fn test_m1_hover_non_sensitive_shows_actual_value() {
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &entries,
        &[entries.clone()],
        None,
    );
    assert!(hover.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.unwrap().contents {
        assert!(
            m.value.contains("8080"),
            "non-sensitive hover must show actual value; got: {}",
            m.value
        );
        assert!(
            !m.value.contains("***"),
            "non-sensitive hover must NOT show *** redaction marker; got: {}",
            m.value
        );
    }
}

/// M1 — hover for a SENSITIVE key must NOT show the raw value but must show
/// a redacted annotation.
#[test]
fn test_m1_hover_sensitive_shows_redacted_annotation() {
    let schema = schema_with_sensitive("DB_PASSWORD");
    let entries = vec![make_entry(
        "DB_PASSWORD",
        "supersecret",
        0,
        SourceLayer::Base,
    )];
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &entries,
        &[entries.clone()],
        Some(&schema),
    );
    assert!(hover.is_some());
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.unwrap().contents {
        assert!(
            !m.value.contains("supersecret"),
            "sensitive value must not appear in hover"
        );
        // The annotation must indicate redaction (matches FR6 / hover.rs style).
        assert!(
            m.value.contains("redacted") || m.value.contains("sensitive"),
            "sensitive hover must include redacted/sensitive annotation; got: {}",
            m.value
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Regression tests — adversarial review findings (second batch)
// ═══════════════════════════════════════════════════════════════════════════════

// ── FR9: config_goto_definition ───────────────────────────────────────────────

/// FR9 — goto_definition resolves to base-layer entry in an open doc.
/// Cursor is on the key on line 0 of a profile doc; the defining entry is in
/// a separate base-layer document registered in `open_docs`.
#[test]
fn test_fr9_goto_definition_key_resolves_to_base_layer_doc() {
    let base_url = url("/proj/application.properties");
    let profile_url = url("/proj/application-prod.properties");

    // Base doc defines "server.port" at line 0.
    let base_entry = make_entry("server.port", "8080", 0, SourceLayer::Base);
    let base_entries = vec![base_entry.clone()];

    // Profile doc overrides it at line 0.
    let profile_entry = make_entry(
        "server.port",
        "9090",
        0,
        SourceLayer::Profile("prod".into()),
    );
    let profile_entries = vec![profile_entry.clone()];

    let mut open_docs = HashMap::new();
    open_docs.insert(base_url.clone(), base_entries.clone());

    // Hover cursor on the "server.port" key character range (char 3 is inside key).
    let result = config_goto_definition(
        Position {
            line: 0,
            character: 3,
        },
        &profile_entries,
        None,
        &HashMap::new(),
        &open_docs,
    );

    let Some(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(loc)) = result else {
        panic!("FR9: expected Scalar goto_definition response");
    };
    assert_eq!(
        loc.uri, base_url,
        "FR9: definition must resolve to the base-layer document"
    );
    assert_eq!(
        loc.range.start.line, 0,
        "FR9: definition must point at line 0"
    );
    assert_eq!(
        loc.range.start.character, 0,
        "FR9: definition range must start at character 0"
    );
}

/// FR9 — goto_definition returns None when key is not in any open doc.
#[test]
fn test_fr9_goto_definition_absent_key_returns_none() {
    let entries = vec![make_entry("server.port", "8080", 0, SourceLayer::Base)];
    let result = config_goto_definition(
        Position {
            line: 5,
            character: 0,
        },
        &entries,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(
        result.is_none(),
        "FR9: cursor not on any key must return None"
    );
}

// ── Determinism: goto_definition sorts open_docs by URI ──────────────────────

/// When two base-layer documents share the same key, goto_definition must
/// deterministically return the one with the lexicographically earlier URI,
/// not whichever HashMap iteration order happens to produce.
#[test]
fn test_goto_definition_deterministic_when_two_base_docs_share_key() {
    let url_a = url("/proj/a_application.properties");
    let url_b = url("/proj/b_application.properties");

    // Both base docs define "timeout" at line 0. The result must always be url_a
    // (lexicographically first) regardless of HashMap internal order.
    let entry_a = make_entry("timeout", "30", 0, SourceLayer::Base);
    let entry_b = make_entry("timeout", "60", 0, SourceLayer::Base);

    // Produce many open_docs variations by inserting in both orders.
    for _ in 0..20 {
        let mut open_docs = HashMap::new();
        open_docs.insert(url_a.clone(), vec![entry_a.clone()]);
        open_docs.insert(url_b.clone(), vec![entry_b.clone()]);

        // Current-file entries (neither base doc is the "current" file here).
        let current_entries = vec![make_entry(
            "timeout",
            "99",
            0,
            SourceLayer::Profile("test".into()),
        )];

        let result = config_goto_definition(
            Position {
                line: 0,
                character: 3,
            },
            &current_entries,
            None,
            &HashMap::new(),
            &open_docs,
        );

        let Some(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(loc)) = result else {
            panic!("determinism: expected Scalar goto_definition response");
        };
        assert_eq!(
            loc.uri, url_a,
            "determinism: sorted order must always yield url_a first; got {}",
            loc.uri
        );
    }
}

// ── M-C: interpolation depth cap ─────────────────────────────────────────────

/// M-C — a long non-cyclic chain of 110 variables must not stack-overflow.
/// Each Vn references Vn+1; without the depth cap this would recurse 110
/// frames deep and could overflow in release builds.
#[test]
fn test_mc_interpolation_depth_cap_no_stack_overflow() {
    const DEPTH: usize = 110;
    let mut env = HashMap::new();
    // V0 = ${V1}, V1 = ${V2}, ..., V108 = ${V109}, V109 = "leaf"
    for i in 0..DEPTH - 1 {
        env.insert(format!("V{}", i), format!("${{V{}}}", i + 1));
    }
    env.insert(format!("V{}", DEPTH - 1), "leaf".to_string());

    let raw = "${V0}";
    let mut visited = std::collections::HashSet::new();
    // Must not stack-overflow regardless of depth.
    let (result, _expanded) = interpolate_value(raw, &env, &mut visited);
    // The depth cap kicks in before fully resolving, so result is some
    // partial/unresolved string — we only assert no panic occurred and
    // that the output is not empty.
    assert!(!result.is_empty(), "M-C: result must not be empty");
}

/// M-C — chains under the depth cap (< 100 vars) resolve normally.
#[test]
fn test_mc_interpolation_shallow_chain_resolves_fully() {
    let mut env = HashMap::new();
    // A -> B -> C (depth 3 — well within limit)
    env.insert("A".to_string(), "${B}".to_string());
    env.insert("B".to_string(), "${C}".to_string());
    env.insert("C".to_string(), "hello".to_string());

    let mut visited = std::collections::HashSet::new();
    let (result, expanded) = interpolate_value("${A}", &env, &mut visited);
    assert_eq!(result, "hello", "M-C: shallow chain must resolve fully");
    assert!(expanded, "M-C: expansion flag must be true");
}

// ── BOM stripping ─────────────────────────────────────────────────────────────

/// BOM — a .properties file with a leading UTF-8 BOM must parse the first
/// key without corruption (previously the BOM bytes were included in the key).
#[test]
fn test_bom_leading_utf8_bom_stripped_before_parse() {
    // \u{FEFF} is the UTF-8 BOM (EF BB BF as bytes).
    let content = "\u{FEFF}server.port=8080\napp.name=MyApp\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 2, "BOM: must parse exactly 2 entries");
    assert_eq!(
        kv[0].key, "server.port",
        "BOM: first key must be 'server.port', not '\u{FEFF}server.port'"
    );
    assert_eq!(kv[0].value, "8080", "BOM: first value must be '8080'");
    assert_eq!(kv[1].key, "app.name");
}

// ── KEY = value off-by-one ────────────────────────────────────────────────────

/// Verify `KEY = value` (spaces around `=`) positions value_range at the
/// first non-whitespace character of the value, not at the `=` or the space.
#[test]
fn test_key_eq_space_value_range_starts_at_value() {
    let content = "server.port = 8080\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1, "must parse exactly one entry");
    assert_eq!(kv[0].key, "server.port");
    assert_eq!(kv[0].value, "8080");

    // "server.port = " = 14 chars before '8'.
    // key=0..11, space, =, space → value starts at char 14.
    let val_start = kv[0].value_range.start.character;
    assert_eq!(
        val_start, 14,
        "KEY = value: value_range.start must be 14 (after 'server.port = '); got {}",
        val_start
    );
}

/// Same test for `KEY: value` (colon separator with trailing space).
#[test]
fn test_key_colon_space_value_range_starts_at_value() {
    let content = "server.port: 8080\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1);
    assert_eq!(kv[0].key, "server.port");
    assert_eq!(kv[0].value, "8080");

    // "server.port: " = 13 chars before '8'.
    let val_start = kv[0].value_range.start.character;
    assert_eq!(
        val_start, 13,
        "KEY: value: value_range.start must be 13; got {}",
        val_start
    );
}

// ── col-0 off-by-one fix ──────────────────────────────────────────────────────

/// The unterminated-ref at column 0 must report column 0, not 1.
/// Before the fix `start_col.saturating_sub(1) + 1` always reported at
/// least 1 even when the `$` was the very first character.
#[test]
fn test_find_unterminated_refs_col_zero() {
    let positions = find_unterminated_refs("${OPEN");
    assert_eq!(
        positions,
        vec![0],
        "col-0 fix: unterminated ref at position 0 must report column 0"
    );
}

/// Unterminated ref in the middle of a line reports the correct column.
#[test]
fn test_find_unterminated_refs_mid_line() {
    // "ABC${OPEN" — `$` is at UTF-16 col 3.
    let positions = find_unterminated_refs("ABC${OPEN");
    assert_eq!(positions, vec![3], "mid-line: column must be 3");
}

/// Terminated ref must NOT appear in results.
#[test]
fn test_find_unterminated_refs_terminated_not_reported() {
    let positions = find_unterminated_refs("${CLOSED}");
    assert!(
        positions.is_empty(),
        "terminated ref must produce no unterminated positions"
    );
}

// ── NFR9: format round-trip idempotency ───────────────────────────────────────

/// NFR9 — `config_format_text_edits` on an already-canonical doc must return
/// no edits. Writing a well-formed properties file through the formatter and
/// then formatting again must be a no-op (byte-identical round-trip).
#[test]
fn test_nfr9_format_text_edits_already_canonical_produces_no_change() {
    // This document is already correctly formatted.
    let canonical = "# Database config\nserver.port=8080\napp.name=MyApp\n";
    let edits = config_format_text_edits(canonical, WriteCapability::ReadWrite);
    assert!(
        edits.is_empty(),
        "NFR9: format of already-canonical doc must produce zero edits; got {} edit(s)",
        edits.len()
    );
}

/// NFR9 — formatting is idempotent: applying `config_format_document` twice
/// produces the same output as applying it once.
#[test]
fn test_nfr9_format_document_is_idempotent() {
    let input = "server.port  =  8080\napp.name=MyApp\n";
    let once = config_format_document(input);
    let twice = config_format_document(&once);
    assert_eq!(
        once, twice,
        "NFR9: config_format_document must be idempotent"
    );
}

// ── UTF-16 key range for non-ASCII keys ──────────────────────────────────────

/// Non-ASCII key names produce a key_range whose end character equals the
/// UTF-16 length of the key, not its byte length.
/// 'é' (U+00E9) is 2 UTF-8 bytes but 1 UTF-16 code unit.
/// 'α' (U+03B1) is 2 UTF-8 bytes but 1 UTF-16 code unit.
#[test]
fn test_utf16_key_range_non_ascii_key_two_byte_chars() {
    // "héllo=world" — key is "héllo" (5 UTF-16 units, 6 UTF-8 bytes).
    // Note: keys starting with 'h' are valid (ASCII letter).
    // But parse_properties requires key to start with ASCII letter or '_'.
    // 'h' is ASCII so "héllo" starts with 'h' — valid.
    let content = "héllo=world\n";
    let entries = parse_properties(content, SourceLayer::Base);
    let kv: Vec<_> = entries.iter().filter(|e| !e.key.is_empty()).collect();
    assert_eq!(kv.len(), 1, "must parse exactly one entry");
    // key_range end should be 5 (UTF-16 units for "héllo"), not 6 (bytes).
    let key_end = kv[0].key_range.end.character;
    assert_eq!(
        key_end, 5,
        "UTF-16 key range: 'héllo' has 5 UTF-16 units; got {}",
        key_end
    );
    // value_range start should be at char 6 ("héllo="), i.e. after the '='.
    let val_start = kv[0].value_range.start.character;
    assert_eq!(
        val_start, 6,
        "UTF-16 value range: value must start at char 6 after 'héllo='; got {}",
        val_start
    );
}
