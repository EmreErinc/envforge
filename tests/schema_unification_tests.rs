//! Tests for Intent 040 — Cross-Format Schema Unification (Stories 001–004).
//!
//! All tests live in `tests/` per project conventions. No in-module `#[cfg(test)]`.
//!
//! Coverage:
//! - Story 001: `canonical_key` normalization rules, `UnifiedSchema` lookup.
//! - Story 004: Spring relaxed-binding equivalences folded into `canonical_key`.
//! - Story 002: Cross-format entry diagnostics (unknown-key, type-mismatch,
//!   missing-required).
//! - Story 003: Cross-format go-to-definition, find-references (determinism,
//!   cache reuse, multi-format).

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range as LspRange, Url};

use envforge::ops::config_format::{ConfigEntry, SourceLayer};
use envforge::ops::schema::{parse_schema_content, EnvSchema, SchemaVariable, VarType};
use envforge::ops::schema_unification::{
    canonical_key, cross_format_diagnostics_to_lsp, cross_format_entry_diagnostics,
    cross_format_find_references, cross_format_goto_definition, is_key_sensitive,
    missing_required_diagnostics, CrossFormatDiagnosticKind, UnifiedSchema,
};

// ── helpers ───────────────────────────────────────────────────────────────────

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

fn schema_with_variable(name: &str, var: SchemaVariable) -> EnvSchema {
    let mut variables = HashMap::new();
    variables.insert(name.to_string(), var);
    EnvSchema { variables }
}

fn string_var(required: bool, sensitive: bool) -> SchemaVariable {
    SchemaVariable {
        var_type: VarType::String,
        required,
        sensitive,
        ..Default::default()
    }
}

fn number_var() -> SchemaVariable {
    SchemaVariable {
        var_type: VarType::Number,
        ..Default::default()
    }
}

fn bool_var() -> SchemaVariable {
    SchemaVariable {
        var_type: VarType::Bool,
        ..Default::default()
    }
}

fn file_url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).expect("valid test URL")
}

// ── Story 001 + 004: canonical_key normalization ──────────────────────────────

#[test]
fn test_canonical_key_empty_returns_none() {
    assert!(canonical_key("").is_none());
}

#[test]
fn test_canonical_key_whitespace_only_returns_none() {
    assert!(canonical_key("   ").is_none());
}

#[test]
fn test_canonical_key_upper_snake_to_dotted() {
    assert_eq!(
        canonical_key("SPRING_DATASOURCE_URL").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_already_dotted_lower() {
    assert_eq!(
        canonical_key("spring.datasource.url").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_colon_separated_pascal() {
    // JSONC / .NET style
    assert_eq!(
        canonical_key("Spring:Datasource:Url").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_hyphen_separated() {
    assert_eq!(
        canonical_key("spring-datasource-url").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_camel_case() {
    assert_eq!(
        canonical_key("springDatasourceUrl").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_pascal_case() {
    // PascalCase without separator
    assert_eq!(
        canonical_key("SpringDatasourceUrl").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_double_underscore_spring() {
    // Spring double-underscore binding
    assert_eq!(
        canonical_key("spring__datasource__url").as_deref(),
        Some("spring.datasource.url")
    );
}

#[test]
fn test_canonical_key_mixed_separators_deterministic() {
    // Mixed: `spring.datasource_url` — underscore is NOT double-underscore,
    // it is treated as a separator (via UPPER_SNAKE check failing and
    // then `-` → `.` path).
    // Since the key has a mix of upper and lower we split at camelCase boundary
    // then replace `-` with `.`. The `_` in `datasource_url` → `datasource.url`.
    // But `spring.datasource_url` has a dot already, making it NOT UPPER_SNAKE,
    // so the `_` stays until the `replace('-', '.')` step. The key here is that
    // the test validates the documented behaviour (deterministic, no panic).
    let result = canonical_key("spring.datasource_url");
    assert!(result.is_some());
    // Should not panic, should be lowercase.
    assert_eq!(
        result.as_deref().unwrap(),
        result.as_deref().unwrap().to_lowercase().as_str()
    );
}

#[test]
fn test_canonical_key_spring_relaxed_binding_all_variants_match() {
    // Story 004 AC: all variants must collapse to one canonical key.
    let variants = [
        "spring.datasource.url",
        "SPRING_DATASOURCE_URL",
        "spring-datasource-url",
        "spring__datasource__url",
        "Spring:Datasource:Url",
    ];
    let canonicals: Vec<Option<String>> = variants.iter().map(|k| canonical_key(k)).collect();
    // All must be Some and identical.
    let first = canonicals[0]
        .as_deref()
        .expect("first variant must normalize");
    for (i, ck) in canonicals.iter().enumerate() {
        assert_eq!(
            ck.as_deref(),
            Some(first),
            "variant '{}' produced different canonical: {:?}",
            variants[i],
            ck
        );
    }
}

#[test]
fn test_canonical_key_single_word_upper() {
    assert_eq!(canonical_key("PORT").as_deref(), Some("port"));
}

#[test]
fn test_canonical_key_single_word_lower() {
    assert_eq!(canonical_key("port").as_deref(), Some("port"));
}

#[test]
fn test_canonical_key_deeply_nested() {
    assert_eq!(
        canonical_key("SPRING_SECURITY_OAUTH2_RESOURCE_SERVER_JWT_ISSUER_URI").as_deref(),
        Some("spring.security.oauth2.resource.server.jwt.issuer.uri")
    );
}

#[test]
fn test_canonical_key_numeric_component() {
    // Keys with numbers should not panic and produce a valid canonical.
    let result = canonical_key("list0item");
    assert!(result.is_some());
}

#[test]
fn test_canonical_key_non_spring_dots_not_collapsed() {
    // A dotted key that is not Spring-specific should still normalize cleanly.
    assert_eq!(canonical_key("my.key").as_deref(), Some("my.key"));
    assert_eq!(canonical_key("MY_KEY").as_deref(), Some("my.key"));
}

// ── Story 001: UnifiedSchema ──────────────────────────────────────────────────

#[test]
fn test_unified_schema_lookup_by_upper_snake() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // Exact UPPER_SNAKE key.
    assert!(us.lookup("SPRING_DATASOURCE_URL").is_some());
}

#[test]
fn test_unified_schema_lookup_by_dotted() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // Dotted properties style.
    assert!(us.lookup("spring.datasource.url").is_some());
}

#[test]
fn test_unified_schema_lookup_by_colon_path() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // .NET JSONC style.
    assert!(us.lookup("Spring:Datasource:Url").is_some());
}

/// H-2 fix: `lookup` uses strict canonical key so `spring-datasource-url`
/// (hyphenated) does NOT alias `SPRING_DATASOURCE_URL` (UPPER_SNAKE) in the
/// strict sense used for sensitivity and unknown-key diagnostics.
///
/// Use `lookup_relaxed` for goto-def / refs where Spring relaxed binding applies.
#[test]
fn test_unified_schema_lookup_by_hyphen() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // Strict lookup: hyphen is NOT a path separator, so this should NOT match.
    // (H-2 fix: prevents false sensitivity / unknown-key for unrelated keys)
    assert!(
        us.lookup("spring-datasource-url").is_none(),
        "strict lookup: hyphen key must NOT match UPPER_SNAKE (H-2 fix)"
    );
    // Relaxed lookup: Spring relaxed binding collapses hyphen, so this SHOULD match.
    assert!(
        us.lookup_relaxed("spring-datasource-url").is_some(),
        "relaxed lookup: hyphen key MUST match UPPER_SNAKE for goto-def/refs"
    );
}

#[test]
fn test_unified_schema_lookup_unknown_returns_none() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    assert!(us.lookup("COMPLETELY_UNKNOWN_KEY").is_none());
}

#[test]
fn test_unified_schema_empty_key_returns_none() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    assert!(us.lookup("").is_none());
}

#[test]
fn test_unified_schema_from_env_schema_trait() {
    let schema = schema_with_variable("DB_HOST", string_var(false, false));
    let us: UnifiedSchema = schema.into();
    assert!(us.lookup("DB_HOST").is_some());
    assert!(us.lookup("db.host").is_some());
}

#[test]
fn test_unified_schema_schema_key_for() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(false, false));
    let us = UnifiedSchema::new(schema);
    let k = us.schema_key_for("spring.datasource.url");
    assert_eq!(k, Some("SPRING_DATASOURCE_URL"));
}

#[test]
fn test_unified_schema_is_empty_true_for_empty() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    assert!(us.is_empty());
}

#[test]
fn test_unified_schema_sensitivity_preserved_across_formats() {
    // AI-safety: sensitive key in schema → sensitive in all formats.
    let schema = schema_with_variable("SECRET_KEY", string_var(false, true));
    let us = UnifiedSchema::new(schema);
    assert!(is_key_sensitive("SECRET_KEY", &us));
    // Via dotted (YAML / properties format)
    assert!(is_key_sensitive("secret.key", &us));
}

// ── Story 002: Cross-format diagnostics ──────────────────────────────────────

#[test]
fn test_cross_format_entry_diagnostics_unknown_key() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    let entries = vec![make_entry("some.unknown.key", "value", 0)];
    let diags = cross_format_entry_diagnostics(&entries, &us);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].kind, CrossFormatDiagnosticKind::UnknownKey);
    assert_eq!(diags[0].concrete_key, "some.unknown.key");
}

#[test]
fn test_cross_format_entry_diagnostics_no_diag_for_known_key() {
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(false, false));
    let us = UnifiedSchema::new(schema);
    // Match via dotted form (YAML/properties file would use this).
    let entries = vec![make_entry(
        "spring.datasource.url",
        "jdbc:postgresql://localhost/db",
        0,
    )];
    let diags = cross_format_entry_diagnostics(&entries, &us);
    // No unknown-key diagnostic because the canonical keys match.
    let unknown_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == CrossFormatDiagnosticKind::UnknownKey)
        .collect();
    assert!(
        unknown_diags.is_empty(),
        "unexpected unknown-key diags: {:?}",
        unknown_diags
    );
}

#[test]
fn test_cross_format_entry_diagnostics_type_mismatch_number() {
    let schema = schema_with_variable("SERVER_PORT", number_var());
    let us = UnifiedSchema::new(schema);
    // "not_a_number" for a number field → type mismatch.
    let entries = vec![make_entry("SERVER_PORT", "not_a_number", 0)];
    let diags = cross_format_entry_diagnostics(&entries, &us);
    let type_diags: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.kind, CrossFormatDiagnosticKind::TypeMismatch { .. }))
        .collect();
    assert_eq!(type_diags.len(), 1);
}

#[test]
fn test_cross_format_entry_diagnostics_type_mismatch_bool() {
    let schema = schema_with_variable("DEBUG", bool_var());
    let us = UnifiedSchema::new(schema);
    let entries = vec![make_entry("DEBUG", "maybe", 0)];
    let diags = cross_format_entry_diagnostics(&entries, &us);
    let type_diags: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.kind, CrossFormatDiagnosticKind::TypeMismatch { .. }))
        .collect();
    assert_eq!(type_diags.len(), 1);
}

#[test]
fn test_cross_format_entry_diagnostics_no_panic_on_empty_entries() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    let diags = cross_format_entry_diagnostics(&[], &us);
    assert!(diags.is_empty());
}

#[test]
fn test_cross_format_entry_diagnostics_skips_blank_keys() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    let mut blank = make_entry("", "", 0);
    blank.key = String::new();
    let diags = cross_format_entry_diagnostics(&[blank], &us);
    assert!(diags.is_empty());
}

#[test]
fn test_cross_format_entry_diagnostics_cross_format_match() {
    // Story 002 AC: YAML dotted key matches schema UPPER_SNAKE entry.
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(false, false));
    let us = UnifiedSchema::new(schema);
    // Entries from application.yml (dotted keys) and appsettings.json (colon keys).
    let entries_yaml = vec![make_entry(
        "spring.datasource.url",
        "jdbc:pg://localhost/db",
        0,
    )];
    let entries_jsonc = vec![make_entry(
        "Spring:Datasource:Url",
        "Server=.;Database=mydb",
        0,
    )];
    let diags_yaml = cross_format_entry_diagnostics(&entries_yaml, &us);
    let diags_jsonc = cross_format_entry_diagnostics(&entries_jsonc, &us);
    // Neither should produce an unknown-key diagnostic.
    assert!(
        diags_yaml
            .iter()
            .all(|d| d.kind != CrossFormatDiagnosticKind::UnknownKey),
        "YAML entry should not produce unknown-key: {:?}",
        diags_yaml
    );
    assert!(
        diags_jsonc
            .iter()
            .all(|d| d.kind != CrossFormatDiagnosticKind::UnknownKey),
        "JSONC entry should not produce unknown-key: {:?}",
        diags_jsonc
    );
}

#[test]
fn test_missing_required_diagnostic_when_absent() {
    let schema = schema_with_variable("API_KEY", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // No entries anywhere → missing-required.
    let diags = missing_required_diagnostics(&[], &us);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].kind, CrossFormatDiagnosticKind::MissingRequired);
    assert_eq!(diags[0].concrete_key, "API_KEY");
}

#[test]
fn test_missing_required_diagnostic_satisfied_by_any_format() {
    // Story 002 AC: required in schema, present in .env (not YAML) → not missing.
    let schema = schema_with_variable("API_KEY", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    // Present in .env as UPPER_SNAKE.
    let dotenv_entries = vec![make_entry("API_KEY", "sk-abc", 0)];
    let diags = missing_required_diagnostics(&[dotenv_entries.as_slice()], &us);
    assert!(
        diags.is_empty(),
        "required key present in .env should not generate missing-required: {:?}",
        diags
    );
}

#[test]
fn test_missing_required_satisfied_by_yaml_dotted_format() {
    // Required key in schema as UPPER_SNAKE, satisfied by YAML dotted key.
    let schema = schema_with_variable("SPRING_DATASOURCE_URL", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    let yaml_entries = vec![make_entry(
        "spring.datasource.url",
        "jdbc:postgresql://db",
        0,
    )];
    let diags = missing_required_diagnostics(&[yaml_entries.as_slice()], &us);
    assert!(
        diags.is_empty(),
        "required key present in YAML format should not be flagged missing: {:?}",
        diags
    );
}

#[test]
fn test_missing_required_not_required_no_diagnostic() {
    let schema = schema_with_variable("OPTIONAL_KEY", string_var(false, false));
    let us = UnifiedSchema::new(schema);
    let diags = missing_required_diagnostics(&[], &us);
    assert!(diags.is_empty());
}

#[test]
fn test_cross_format_diagnostics_to_lsp_converts_correctly() {
    let schema = schema_with_variable("EXISTING_KEY", string_var(false, false));
    let us = UnifiedSchema::new(schema);
    let entries = vec![make_entry("UNKNOWN_KEY", "value", 5)];
    let cross_diags = cross_format_entry_diagnostics(&entries, &us);
    let lsp_diags = cross_format_diagnostics_to_lsp(&cross_diags);
    assert_eq!(lsp_diags.len(), 1);
    assert!(lsp_diags[0].message.contains("UNKNOWN_KEY"));
    assert!(lsp_diags[0].message.contains("not in schema"));
    assert_eq!(lsp_diags[0].source.as_deref(), Some("envforge"));
}

// ── Story 003: Cross-format go-to-definition ─────────────────────────────────

#[test]
fn test_cross_format_goto_empty_key_returns_empty() {
    let locs = cross_format_goto_definition("", None, &HashMap::new(), &HashMap::new());
    assert!(locs.is_empty());
}

#[test]
fn test_cross_format_goto_schema_entry_first() {
    let schema_url = file_url("/project/.env.schema");
    let mut schema_line_map = HashMap::new();
    schema_line_map.insert("SPRING_DATASOURCE_URL".to_string(), 5u32);

    let locs = cross_format_goto_definition(
        "spring.datasource.url",
        Some(&schema_url),
        &schema_line_map,
        &HashMap::new(),
    );
    // Should find the schema entry.
    assert!(!locs.is_empty(), "expected at least one location");
    assert_eq!(locs[0].uri, schema_url);
    assert_eq!(locs[0].range.start.line, 5);
}

#[test]
fn test_cross_format_goto_schema_entry_by_upper_snake() {
    // Exact UPPER_SNAKE lookup (from .env file).
    let schema_url = file_url("/project/.env.schema");
    let mut schema_line_map = HashMap::new();
    schema_line_map.insert("SPRING_DATASOURCE_URL".to_string(), 5u32);

    let locs = cross_format_goto_definition(
        "SPRING_DATASOURCE_URL",
        Some(&schema_url),
        &schema_line_map,
        &HashMap::new(),
    );
    assert!(!locs.is_empty());
    assert_eq!(locs[0].range.start.line, 5);
}

#[test]
fn test_cross_format_goto_concrete_defs_across_formats() {
    let schema_url = file_url("/project/.env.schema");
    let mut schema_line_map = HashMap::new();
    schema_line_map.insert("SPRING_DATASOURCE_URL".to_string(), 5u32);

    // Two open docs: one .properties, one .env.local
    let props_url = file_url("/project/application.properties");
    let dotenv_url = file_url("/project/.env.local");

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(
        props_url.clone(),
        vec![make_entry("spring.datasource.url", "jdbc:pg://db", 3)],
    );
    open_docs.insert(
        dotenv_url.clone(),
        vec![make_entry("SPRING_DATASOURCE_URL", "jdbc:pg://local", 7)],
    );

    let locs = cross_format_goto_definition(
        "spring.datasource.url",
        Some(&schema_url),
        &schema_line_map,
        &open_docs,
    );

    // Expect: schema entry + 2 concrete defs = 3 total.
    assert_eq!(locs.len(), 3, "locs: {:?}", locs);

    // Schema entry should come first (sorted by URI, schema URI < /project/...).
    // All should reference the same logical key.
    let uris: Vec<&str> = locs.iter().map(|l| l.uri.as_str()).collect();
    assert!(uris.contains(&schema_url.as_str()));
    assert!(uris.contains(&props_url.as_str()));
    assert!(uris.contains(&dotenv_url.as_str()));
}

#[test]
fn test_cross_format_goto_sorted_by_uri_then_line() {
    // No schema, two docs each with one matching entry.
    let url_a = file_url("/project/a.properties");
    let url_b = file_url("/project/b.properties");
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(url_a.clone(), vec![make_entry("my.key", "v1", 10)]);
    open_docs.insert(url_b.clone(), vec![make_entry("MY_KEY", "v2", 2)]);

    let locs = cross_format_goto_definition("my.key", None, &HashMap::new(), &open_docs);
    assert_eq!(locs.len(), 2);
    // a comes before b lexicographically.
    assert_eq!(locs[0].uri, url_a);
    assert_eq!(locs[1].uri, url_b);
}

#[test]
fn test_cross_format_goto_no_docs_returns_schema_only() {
    let schema_url = file_url("/project/.env.schema");
    let mut schema_line_map = HashMap::new();
    schema_line_map.insert("DB_HOST".to_string(), 2u32);

    let locs = cross_format_goto_definition(
        "db.host",
        Some(&schema_url),
        &schema_line_map,
        &HashMap::new(),
    );
    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].uri, schema_url);
}

// ── Story 003: Cross-format find-references ───────────────────────────────────

#[test]
fn test_cross_format_find_references_includes_all_formats() {
    let props_url = file_url("/project/application.properties");
    let yaml_url = file_url("/project/application.yml");
    let toml_url = file_url("/project/config.toml");
    let dotenv_url = file_url("/project/.env.local");

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(props_url.clone(), vec![make_entry("my.key", "v1", 0)]);
    open_docs.insert(yaml_url.clone(), vec![make_entry("my.key", "v2", 1)]);
    open_docs.insert(toml_url.clone(), vec![make_entry("my.key", "v3", 2)]);
    open_docs.insert(dotenv_url.clone(), vec![make_entry("MY_KEY", "v4", 3)]);

    let locs = cross_format_find_references("my.key", None, &HashMap::new(), &open_docs, false);
    // All four docs contain the logical key "my.key".
    assert_eq!(locs.len(), 4, "expected 4 references, got: {:?}", locs);
}

#[test]
fn test_cross_format_find_references_include_declaration() {
    let schema_url = file_url("/project/.env.schema");
    let mut schema_line_map = HashMap::new();
    schema_line_map.insert("MY_KEY".to_string(), 0u32);

    let doc_url = file_url("/project/application.properties");
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(doc_url.clone(), vec![make_entry("my.key", "v1", 0)]);

    // With include_declaration = true.
    let locs_with = cross_format_find_references(
        "my.key",
        Some(&schema_url),
        &schema_line_map,
        &open_docs,
        true,
    );
    // With include_declaration = false.
    let locs_without = cross_format_find_references(
        "my.key",
        Some(&schema_url),
        &schema_line_map,
        &open_docs,
        false,
    );

    assert_eq!(locs_with.len(), locs_without.len() + 1);
    assert!(locs_with.iter().any(|l| l.uri == schema_url));
    assert!(!locs_without.iter().any(|l| l.uri == schema_url));
}

#[test]
fn test_cross_format_find_references_sorted_deterministic() {
    // Same logical key in two docs; results must be sorted by (uri, line).
    let url_a = file_url("/project/a.properties");
    let url_b = file_url("/project/b.properties");

    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(url_a.clone(), vec![make_entry("my.key", "v1", 5)]);
    open_docs.insert(url_b.clone(), vec![make_entry("my.key", "v2", 1)]);

    let locs = cross_format_find_references("my.key", None, &HashMap::new(), &open_docs, false);
    assert_eq!(locs.len(), 2);
    // Sorted by URI: /project/a < /project/b.
    assert_eq!(locs[0].uri, url_a);
    assert_eq!(locs[1].uri, url_b);
}

#[test]
fn test_cross_format_find_references_empty_key_returns_empty() {
    let locs = cross_format_find_references("", None, &HashMap::new(), &HashMap::new(), false);
    assert!(locs.is_empty());
}

#[test]
fn test_cross_format_find_references_no_match_returns_empty() {
    let url = file_url("/project/application.properties");
    let mut open_docs: HashMap<Url, Vec<ConfigEntry>> = HashMap::new();
    open_docs.insert(url, vec![make_entry("other.key", "v1", 0)]);

    let locs = cross_format_find_references(
        "completely.different.key",
        None,
        &HashMap::new(),
        &open_docs,
        false,
    );
    assert!(locs.is_empty());
}

// ── Sensitive flag propagation across formats ─────────────────────────────────

#[test]
fn test_is_key_sensitive_from_schema_via_dotted_format() {
    // Schema has UPPER_SNAKE sensitive key; checked via YAML dotted key.
    let schema = schema_with_variable("SECRET_API_KEY", string_var(false, true));
    let us = UnifiedSchema::new(schema);
    assert!(is_key_sensitive("secret.api.key", &us));
}

#[test]
fn test_is_key_sensitive_from_schema_via_colon_format() {
    let schema = schema_with_variable("SECRET_API_KEY", string_var(false, true));
    let us = UnifiedSchema::new(schema);
    assert!(is_key_sensitive("Secret:Api:Key", &us));
}

#[test]
fn test_is_key_sensitive_heuristic_even_without_schema() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    // dotenv::is_sensitive_key should catch "PASSWORD".
    assert!(is_key_sensitive("PASSWORD", &us));
}

#[test]
fn test_is_key_sensitive_false_for_nonsensitive() {
    let us = UnifiedSchema::new(EnvSchema {
        variables: HashMap::new(),
    });
    assert!(!is_key_sensitive("LOG_LEVEL", &us));
}

// ── Round-trip: parse_schema_content + UnifiedSchema ─────────────────────────

#[test]
fn test_unified_schema_from_parse_schema_content() {
    let content = r#"
[SPRING_DATASOURCE_URL]
type = "url"
required = true
description = "Database JDBC URL"

[SERVER_PORT]
type = "number"
required = false
default = "8080"
"#;
    let schema = parse_schema_content(content).expect("valid schema");
    let us = UnifiedSchema::new(schema);

    // Lookup via UPPER_SNAKE (native).
    assert!(us.lookup("SPRING_DATASOURCE_URL").is_some());
    // Lookup via dotted (Spring / YAML / properties convention).
    assert!(us.lookup("spring.datasource.url").is_some());
    // Lookup via JSONC colon path.
    assert!(us.lookup("Spring:Datasource:Url").is_some());
    // Lookup number via dotted.
    assert!(us.lookup("server.port").is_some());
    // Unknown → None.
    assert!(us.lookup("UNKNOWN").is_none());
}

#[test]
fn test_missing_required_diagnostics_sorted_by_key() {
    // Two required keys missing → diagnostics sorted by concrete_key.
    let content = r#"
[ALPHA_KEY]
type = "string"
required = true

[BETA_KEY]
type = "string"
required = true
"#;
    let schema = parse_schema_content(content).expect("valid schema");
    let us = UnifiedSchema::new(schema);
    let diags = missing_required_diagnostics(&[], &us);
    assert_eq!(diags.len(), 2);
    // Sorted alphabetically.
    assert!(diags[0].concrete_key <= diags[1].concrete_key);
}

// ── Regression: existing single-format schema behaviour unchanged ─────────────

#[test]
fn test_regression_schema_exact_match_still_works() {
    // Existing callers that pass UPPER_SNAKE key directly must continue to get
    // the schema entry (no regression to 036/037/039 code paths).
    let schema = schema_with_variable("DB_HOST", string_var(true, false));
    let us = UnifiedSchema::new(schema);
    let var = us.lookup("DB_HOST").expect("exact match must work");
    assert!(var.required);
}

#[test]
fn test_regression_canonical_index_built_for_all_keys() {
    let content = r#"
[API_KEY]
type = "string"
sensitive = true

[DEBUG]
type = "bool"
"#;
    let schema = parse_schema_content(content).expect("valid schema");
    let us = UnifiedSchema::new(schema);
    // Both keys accessible via canonical form.
    assert!(us.lookup("api.key").is_some());
    assert!(us.lookup("debug").is_some());
}
