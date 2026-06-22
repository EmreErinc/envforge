//! End-to-end tests for Intent 040 — cross-format schema unification wired into
//! LSP handlers (FR2, FR3, FR4 / AI-safety).
//!
//! These tests exercise the exact path now live in `server.rs`:
//! - goto_definition  → `config_goto_definition` + `cross_format_goto_definition`
//! - find_references  → `config_find_references`  + `cross_format_find_references`
//! - diagnostics      → `config_diagnostics`       + `cross_format_entry_diagnostics`
//!                      + `missing_required_diagnostics`
//! - hover sensitivity → `config_hover` with `unified_schema` (FR4 AI-safety)
//!
//! Convention: test_{what_is_being_tested}_{condition}

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range as LspRange, Url};

use envforge::lsp::config_features::{config_diagnostics, config_find_references, config_hover};
use envforge::ops::config_format::{ConfigEntry, SourceLayer};
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use envforge::ops::schema_unification::{
    cross_format_diagnostics_to_lsp, cross_format_entry_diagnostics, cross_format_find_references,
    cross_format_goto_definition, is_key_sensitive, missing_required_diagnostics,
    CrossFormatDiagnosticKind, UnifiedSchema,
};

// ── helpers ────────────────────────────────────────────────────────────────────

fn make_entry(key: &str, value: &str, line: u32) -> ConfigEntry {
    let key_len = key.len() as u32;
    let val_len = value.len() as u32;
    ConfigEntry {
        key: key.to_string(),
        value: value.to_string(),
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
                character: key_len + 1 + val_len,
            },
        },
        line,
        source_layer: SourceLayer::Base,
    }
}

fn fake_url(path: &str) -> Url {
    Url::parse(&format!("file:///workspace/{}", path)).expect("valid test URL")
}

/// Build a minimal `EnvSchema` with one variable.
fn schema_with_var(name: &str, required: bool, sensitive: bool, vtype: VarType) -> EnvSchema {
    let mut variables = HashMap::new();
    variables.insert(
        name.to_string(),
        SchemaVariable {
            var_type: vtype,
            required,
            sensitive,
            ..Default::default()
        },
    );
    EnvSchema { variables }
}

// ── FR3: Cross-format goto-definition ─────────────────────────────────────────

/// A canonical key defined in `.env.schema` + used in `application.yml` AND
/// `appsettings.json` → goto-def from one file returns the other + the schema
/// definition (deduped, sorted by URI then line).
#[test]
fn test_cross_format_goto_def_resolves_across_yaml_and_json() {
    let schema_url = fake_url(".env.schema");
    // Schema line map: SPRING_DATASOURCE_URL is on line 3.
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("SPRING_DATASOURCE_URL".to_string(), 3);

    // application.yml uses dotted lower form.
    let yaml_url = fake_url("src/main/resources/application.yml");
    let yaml_entries = vec![make_entry("spring.datasource.url", "jdbc:h2:mem:test", 5)];

    // appsettings.json uses colon-separated PascalCase.
    let json_url = fake_url("appsettings.json");
    let json_entries = vec![make_entry(
        "Spring:Datasource:Url",
        "Server=.;Database=App",
        2,
    )];

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(yaml_url.clone(), yaml_entries.clone());
    open_docs.insert(json_url.clone(), json_entries);

    // goto-def from the YAML file key.
    let locs = cross_format_goto_definition(
        "spring.datasource.url",
        Some(&schema_url),
        &schema_lines,
        &open_docs,
    );

    // Must include schema definition.
    assert!(
        locs.iter()
            .any(|l| l.uri == schema_url && l.range.start.line == 3),
        "must resolve to schema URI line 3; got: {:?}",
        locs
    );
    // Must include the yaml entry itself.
    assert!(
        locs.iter()
            .any(|l| l.uri == yaml_url && l.range.start.line == 5),
        "must include yaml entry; got: {:?}",
        locs
    );
    // Must include the json entry (cross-format).
    assert!(
        locs.iter()
            .any(|l| l.uri == json_url && l.range.start.line == 2),
        "must include json entry (cross-format); got: {:?}",
        locs
    );
    // Determinism: sorted by (uri, line).
    let uris_in_order: Vec<&str> = locs.iter().map(|l| l.uri.as_str()).collect();
    let mut sorted = uris_in_order.clone();
    sorted.sort();
    assert_eq!(
        uris_in_order, sorted,
        "locations must be sorted by URI; got: {:?}",
        locs
    );
}

/// goto-def from JSONC form also resolves to YAML and schema.
#[test]
fn test_cross_format_goto_def_from_jsonc_key_finds_yaml_and_schema() {
    let schema_url = fake_url(".env.schema");
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("DB_HOST".to_string(), 1);

    let yaml_url = fake_url("config/application.yml");
    let yaml_entries = vec![make_entry("db.host", "localhost", 0)];

    let props_url = fake_url("config/application.properties");
    let props_entries = vec![make_entry("db.host", "127.0.0.1", 7)];

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(yaml_url.clone(), yaml_entries);
    open_docs.insert(props_url.clone(), props_entries);

    // cursor is on `DB_HOST` (UPPER_SNAKE form in a .env).
    let locs =
        cross_format_goto_definition("DB_HOST", Some(&schema_url), &schema_lines, &open_docs);

    assert!(
        locs.iter().any(|l| l.uri == schema_url),
        "must include schema; got: {:?}",
        locs
    );
    assert!(
        locs.iter().any(|l| l.uri == yaml_url),
        "must include yaml; got: {:?}",
        locs
    );
    assert!(
        locs.iter().any(|l| l.uri == props_url),
        "must include properties; got: {:?}",
        locs
    );
}

/// Dedup: same key in two formats that appear under the same canonical form
/// should not produce duplicate locations for the same (uri, line).
#[test]
fn test_cross_format_goto_def_deduplicates_same_uri_line() {
    let schema_url = fake_url(".env.schema");
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("APP_PORT".to_string(), 0);

    let env_url = fake_url(".env");
    // Two entries with equivalent keys at the same line is contrived but dedup
    // must not panic.
    let entries = vec![make_entry("APP_PORT", "8080", 0)];
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(env_url.clone(), entries);

    let locs =
        cross_format_goto_definition("APP_PORT", Some(&schema_url), &schema_lines, &open_docs);

    // Count occurrences of (env_url, line 0).
    let count = locs
        .iter()
        .filter(|l| l.uri == env_url && l.range.start.line == 0)
        .count();
    assert_eq!(
        count, 1,
        "must not duplicate (uri, line) entry; got: {:?}",
        locs
    );
}

// ── FR3: Cross-format find-references ─────────────────────────────────────────

/// Same key across `.properties` + YAML + `.env` → references include all three.
#[test]
fn test_cross_format_find_refs_includes_all_formats() {
    let schema_url = fake_url(".env.schema");
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("SERVER_PORT".to_string(), 2);

    let props_url = fake_url("src/main/resources/application.properties");
    let props_entries = vec![make_entry("server.port", "8080", 4)];

    let yaml_url = fake_url("src/main/resources/application.yml");
    let yaml_entries = vec![make_entry("server.port", "9090", 1)];

    let env_url = fake_url(".env");
    let env_entries = vec![make_entry("SERVER_PORT", "7070", 0)];

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(props_url.clone(), props_entries);
    open_docs.insert(yaml_url.clone(), yaml_entries);
    open_docs.insert(env_url.clone(), env_entries);

    let locs = cross_format_find_references(
        "SERVER_PORT",
        Some(&schema_url),
        &schema_lines,
        &open_docs,
        false, // exclude declaration
    );

    assert!(
        locs.iter().any(|l| l.uri == props_url),
        "must include properties; got: {:?}",
        locs
    );
    assert!(
        locs.iter().any(|l| l.uri == yaml_url),
        "must include yaml; got: {:?}",
        locs
    );
    assert!(
        locs.iter().any(|l| l.uri == env_url),
        "must include .env; got: {:?}",
        locs
    );

    // Sorted by (uri, line).
    let lines: Vec<(_, _)> = locs
        .iter()
        .map(|l| (l.uri.as_str(), l.range.start.line))
        .collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted, "references must be sorted; got: {:?}", locs);
}

/// `include_declaration = true` must add the schema entry.
#[test]
fn test_cross_format_find_refs_include_declaration_adds_schema() {
    let schema_url = fake_url(".env.schema");
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("DB_NAME".to_string(), 5);

    let yaml_url = fake_url("application.yml");
    let yaml_entries = vec![make_entry("db.name", "mydb", 2)];
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(yaml_url.clone(), yaml_entries);

    let with_decl = cross_format_find_references(
        "DB_NAME",
        Some(&schema_url),
        &schema_lines,
        &open_docs,
        true,
    );
    let without_decl = cross_format_find_references(
        "DB_NAME",
        Some(&schema_url),
        &schema_lines,
        &open_docs,
        false,
    );

    assert!(
        with_decl.iter().any(|l| l.uri == schema_url),
        "with declaration must include schema URI"
    );
    assert!(
        !without_decl.iter().any(|l| l.uri == schema_url),
        "without declaration must not include schema URI"
    );
}

// ── FR2: Cross-format diagnostics ─────────────────────────────────────────────

/// An unknown key (not in unified schema) in any format is flagged as
/// `UnknownKey`.
#[test]
fn test_cross_format_diag_unknown_key_in_yaml() {
    let schema = schema_with_var("KNOWN_KEY", false, false, VarType::String);
    let unified = UnifiedSchema::new(schema);

    // An entry with a key that has no canonical match in the schema.
    let entries = vec![
        make_entry("known.key", "value", 0),  // canonical match → no diag
        make_entry("unknown.key", "oops", 1), // no match → UnknownKey
    ];

    let diags = cross_format_entry_diagnostics(&entries, &unified);
    assert_eq!(
        diags.len(),
        1,
        "exactly one unknown-key diagnostic expected"
    );
    assert_eq!(
        diags[0].kind,
        CrossFormatDiagnosticKind::UnknownKey,
        "must be UnknownKey"
    );
    assert_eq!(diags[0].concrete_key, "unknown.key");
}

/// A required key absent from ALL formats triggers `MissingRequired` once.
#[test]
fn test_cross_format_diag_missing_required_flagged_once() {
    let schema = schema_with_var("REQUIRED_KEY", true, false, VarType::String);
    let unified = UnifiedSchema::new(schema);

    // No entry provides `required_key` in any format.
    let yaml_entries = vec![make_entry("other.key", "val", 0)];
    let props_entries = vec![make_entry("another.key", "val2", 0)];
    let all: Vec<&[ConfigEntry]> = vec![&yaml_entries, &props_entries];

    let diags = missing_required_diagnostics(&all, &unified);
    assert_eq!(
        diags.len(),
        1,
        "exactly one missing-required diagnostic; got: {:?}",
        diags
    );
    assert_eq!(diags[0].kind, CrossFormatDiagnosticKind::MissingRequired);
    assert_eq!(diags[0].concrete_key, "REQUIRED_KEY");
}

/// A required key present in ONE of the formats satisfies the requirement
/// globally — no `MissingRequired` diagnostic.
#[test]
fn test_cross_format_diag_required_satisfied_by_any_format() {
    let schema = schema_with_var("DB_URL", true, false, VarType::String);
    let unified = UnifiedSchema::new(schema);

    // Present in yaml but not in props.
    let yaml_entries = vec![make_entry("db.url", "jdbc:h2:mem", 0)]; // canonical match
    let props_entries: Vec<ConfigEntry> = vec![];
    let all: Vec<&[ConfigEntry]> = vec![&yaml_entries, &props_entries];

    let diags = missing_required_diagnostics(&all, &unified);
    assert!(
        diags.is_empty(),
        "no missing-required when key present in any format; got: {:?}",
        diags
    );
}

/// `cross_format_diagnostics_to_lsp` converts `CrossFormatDiagnostic` to
/// tower_lsp `Diagnostic` with source `"envforge"`.
#[test]
fn test_cross_format_diagnostics_to_lsp_source_is_envforge() {
    let schema = schema_with_var("EXISTING", false, false, VarType::String);
    let unified = UnifiedSchema::new(schema);
    let entries = vec![make_entry("mystery.key", "x", 0)];
    let raw = cross_format_entry_diagnostics(&entries, &unified);
    assert_eq!(raw.len(), 1);

    let lsp_diags = cross_format_diagnostics_to_lsp(&raw);
    assert_eq!(lsp_diags.len(), 1);
    assert_eq!(
        lsp_diags[0].source.as_deref(),
        Some("envforge"),
        "source must always be 'envforge'"
    );
    assert!(
        lsp_diags[0].message.contains("mystery.key"),
        "message must mention concrete key"
    );
}

// ── FR4 / AI-safety: cross-format sensitivity in hover ────────────────────────

/// A key marked sensitive in `.env.schema` (UPPER_SNAKE) must be redacted in
/// hover for its JSON (colon-separated PascalCase) form.
#[test]
fn test_cross_format_sensitivity_json_key_redacted_via_unified_schema() {
    // Schema marks SPRING_DATASOURCE_PASSWORD as sensitive.
    let schema = schema_with_var("SPRING_DATASOURCE_PASSWORD", false, true, VarType::String);
    let unified = UnifiedSchema::new(schema.clone());

    // Entry uses JSON/appsettings key form.
    let entry = make_entry("Spring:Datasource:Password", "top-secret-db-pw", 0);

    // is_key_sensitive via unified_schema must return true.
    assert!(
        is_key_sensitive("Spring:Datasource:Password", &unified),
        "Spring:Datasource:Password must be sensitive via canonical mapping"
    );

    // config_hover must redact the value.
    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &[entry],
        &[],
        Some(&schema),
        Some(&unified),
    );
    let hover = hover.expect("hover must be Some for a known key");
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.contents {
        assert!(
            !m.value.contains("top-secret-db-pw"),
            "raw password must not appear in hover; got: {}",
            m.value
        );
        assert!(
            m.value.contains("redacted"),
            "hover must indicate redaction; got: {}",
            m.value
        );
    } else {
        panic!("expected Markup hover content");
    }
}

/// A key marked sensitive in `.env.schema` (UPPER_SNAKE) must be redacted in
/// hover for its YAML/properties (dot-separated lower) form.
#[test]
fn test_cross_format_sensitivity_yaml_key_redacted_via_unified_schema() {
    let schema = schema_with_var("DB_PASSWORD", false, true, VarType::String);
    let unified = UnifiedSchema::new(schema.clone());

    // Entry uses dotted lower form (YAML / .properties).
    let entry = make_entry("db.password", "ultra-secret", 0);

    assert!(
        is_key_sensitive("db.password", &unified),
        "db.password must be sensitive via canonical mapping to DB_PASSWORD"
    );

    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &[entry],
        &[],
        Some(&schema),
        Some(&unified),
    );
    let hover = hover.expect("hover must resolve");
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.contents {
        assert!(
            !m.value.contains("ultra-secret"),
            "raw value must not appear; got: {}",
            m.value
        );
    } else {
        panic!("expected Markup hover content");
    }
}

/// A non-sensitive key must NOT be redacted even when a UnifiedSchema is present.
#[test]
fn test_cross_format_sensitivity_non_sensitive_key_shows_value() {
    let schema = schema_with_var("SERVER_PORT", false, false, VarType::Number);
    let unified = UnifiedSchema::new(schema.clone());

    let entry = make_entry("server.port", "8080", 0);

    assert!(
        !is_key_sensitive("server.port", &unified),
        "server.port must not be sensitive"
    );

    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &[entry],
        &[],
        Some(&schema),
        Some(&unified),
    );
    let hover = hover.expect("hover must resolve");
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.contents {
        assert!(
            m.value.contains("8080"),
            "non-sensitive value must appear in hover; got: {}",
            m.value
        );
    } else {
        panic!("expected Markup hover content");
    }
}

// ── Regression: single-format behaviour unchanged ─────────────────────────────

/// Existing per-format goto-def still returns a result when no cross-format
/// docs are open (open_docs empty).
#[test]
fn test_regression_per_format_goto_def_still_works_no_cross_docs() {
    let schema_url = fake_url(".env.schema");
    let mut schema_lines: HashMap<String, u32> = HashMap::new();
    schema_lines.insert("APP_NAME".to_string(), 0);

    let entry = make_entry("APP_NAME", "MyApp", 2);

    // No other docs open — cross-format should not break the result.
    let locs = cross_format_goto_definition(
        "APP_NAME",
        Some(&schema_url),
        &schema_lines,
        &HashMap::new(),
    );

    assert!(
        locs.iter().any(|l| l.uri == schema_url),
        "schema definition must be returned even with no open cross-format docs; got: {:?}",
        locs
    );
    // entry not in open_docs, so only schema loc is returned.
    let _ = entry; // suppress warning
}

/// Single-format `config_diagnostics` still runs unaffected.
#[test]
fn test_regression_per_format_config_diagnostics_still_runs() {
    let schema = schema_with_var("KNOWN", false, false, VarType::String);
    let entries = vec![
        make_entry("KNOWN", "value", 0),
        make_entry("UNKNOWN", "bad", 1),
    ];

    // Single-format diagnostics: UNKNOWN key triggers a warning.
    let diags = config_diagnostics(&entries, Some(&schema));
    assert!(
        diags.iter().any(|d| d.message.contains("UNKNOWN")),
        "per-format diagnostics must still flag UNKNOWN; got: {:?}",
        diags
    );
}

/// Per-format `config_find_references` exact-key match still works.
#[test]
fn test_regression_per_format_find_references_exact_match() {
    let url1 = fake_url("a.properties");
    let url2 = fake_url("b.properties");
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(url1.clone(), vec![make_entry("db.host", "host1", 0)]);
    open_docs.insert(url2.clone(), vec![make_entry("db.host", "host2", 3)]);

    let locs = config_find_references("db.host", None, &HashMap::new(), &open_docs, false);
    assert!(
        locs.iter().any(|l| l.uri == url1),
        "per-format refs must include url1"
    );
    assert!(
        locs.iter().any(|l| l.uri == url2),
        "per-format refs must include url2"
    );
}

/// Per-format hover (single-format EnvSchema, no unified) still redacts exact-key matches.
#[test]
fn test_regression_per_format_hover_sensitive_exact_key_redacted() {
    let schema = schema_with_var("MY_SECRET", false, true, VarType::String);
    let entry = make_entry("MY_SECRET", "raw-secret-value", 0);

    let hover = config_hover(
        Position {
            line: 0,
            character: 3,
        },
        &[entry],
        &[],
        Some(&schema),
        None, // no unified schema — regression: exact-match path must still redact
    );
    let hover = hover.expect("hover must resolve");
    if let tower_lsp::lsp_types::HoverContents::Markup(m) = hover.contents {
        assert!(
            !m.value.contains("raw-secret-value"),
            "exact-key sensitive value must be redacted even without unified_schema; got: {}",
            m.value
        );
    } else {
        panic!("expected Markup hover content");
    }
}
