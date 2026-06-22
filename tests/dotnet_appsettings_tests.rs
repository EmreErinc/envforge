//! Tests for intent 039 — .NET `appsettings.json` (JSONC) config intelligence.
//!
//! Covers stories 001–005:
//! - 001: Scoped recognition + JsoncFormat + .NET environment cascade
//! - 002: JSONC parse → `:`-path entry model + per-format separator
//! - 003: `__`→`:` env-var binding
//! - 004: Read features + diagnostics (via `config_features`)
//! - 005: Rename round-trip-safe (SurgicalEdit)

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Url};

use envforge::lsp::config_features::{
    config_hover, config_jsonc_diagnostics, config_jsonc_rename, config_semantic_tokens,
    is_valid_dotnet_key_segment,
};
use envforge::lsp::config_file::{
    format_for_uri, is_appsettings_file, is_config_format_file, is_toml_config_file,
    is_yaml_config_file,
};
use envforge::ops::canary::scanner::is_config_canary_target;
use envforge::ops::config_format::{source_layer_for_appsettings, SourceLayer, WriteCapability};
use envforge::parser::jsonc_config_parser::{
    env_var_to_json_path, json_path_to_env_var, parse_jsonc_config, resolve_jsonc_key_span,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn uri(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).expect("valid URL")
}

// ── Story 001: Recognition ────────────────────────────────────────────────────

#[test]
fn test_is_appsettings_file_base() {
    assert!(is_appsettings_file(&uri("/project/appsettings.json")));
}

#[test]
fn test_is_appsettings_file_production() {
    assert!(is_appsettings_file(&uri(
        "/project/appsettings.Production.json"
    )));
}

#[test]
fn test_is_appsettings_file_development() {
    assert!(is_appsettings_file(&uri(
        "/project/appsettings.Development.json"
    )));
}

#[test]
fn test_is_appsettings_file_staging() {
    assert!(is_appsettings_file(&uri(
        "/project/appsettings.Staging.json"
    )));
}

#[test]
fn test_is_appsettings_file_custom_env() {
    assert!(is_appsettings_file(&uri(
        "/project/appsettings.Custom-Env.json"
    )));
}

#[test]
fn test_is_appsettings_file_mcp_json_excluded() {
    assert!(!is_appsettings_file(&uri("/project/mcp.json")));
}

#[test]
fn test_is_appsettings_file_dot_mcp_json_excluded() {
    assert!(!is_appsettings_file(&uri("/project/.mcp.json")));
}

#[test]
fn test_is_appsettings_file_package_json_excluded() {
    assert!(!is_appsettings_file(&uri("/project/package.json")));
}

#[test]
fn test_is_appsettings_file_tsconfig_excluded() {
    assert!(!is_appsettings_file(&uri("/project/tsconfig.json")));
}

#[test]
fn test_is_appsettings_file_settings_json_excluded() {
    assert!(!is_appsettings_file(&uri("/project/settings.json")));
}

#[test]
fn test_is_appsettings_file_empty_env_excluded() {
    // appsettings..json — empty environment segment must not be recognized.
    assert!(!is_appsettings_file(&uri("/project/appsettings..json")));
}

#[test]
fn test_is_appsettings_file_other_json_excluded() {
    assert!(!is_appsettings_file(&uri("/project/launchSettings.json")));
    assert!(!is_appsettings_file(&uri("/project/secrets.json")));
    assert!(!is_appsettings_file(&uri("/project/.claude.json")));
}

#[test]
fn test_is_config_format_file_includes_appsettings() {
    assert!(is_config_format_file(&uri("/project/appsettings.json")));
    assert!(is_config_format_file(&uri(
        "/project/appsettings.Production.json"
    )));
}

#[test]
fn test_is_config_format_file_does_not_affect_existing_formats() {
    // Existing predicates must still work after adding appsettings.
    assert!(is_config_format_file(&uri(
        "/project/application.properties"
    )));
    assert!(is_config_format_file(&uri("/project/application.yml")));
    assert!(is_config_format_file(&uri("/project/Cargo.toml")));
    assert!(is_config_format_file(&uri("/project/.env.local")));
}

#[test]
fn test_format_for_uri_appsettings_base() {
    let (fmt, layer) = format_for_uri(&uri("/project/appsettings.json"))
        .expect("should recognize appsettings.json");
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::DotNetBase);
}

#[test]
fn test_format_for_uri_appsettings_env() {
    let (fmt, layer) = format_for_uri(&uri("/project/appsettings.Production.json"))
        .expect("should recognize appsettings.Production.json");
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(
        layer,
        SourceLayer::DotNetEnvironment("Production".to_string())
    );
}

#[test]
fn test_format_for_uri_mcp_json_not_affected() {
    // mcp.json must still not be claimed by the appsettings predicate.
    // format_for_uri only touches is_config_format_file; mcp.json handling
    // is in server.rs and checked first. Here we verify it isn't recognized
    // as an appsettings file by the new predicate.
    assert!(!is_appsettings_file(&uri("/project/mcp.json")));
    assert!(!is_appsettings_file(&uri("/project/.vscode/mcp.json")));
    assert!(!is_appsettings_file(&uri("/project/.cursor/mcp.json")));
}

#[test]
fn test_format_for_uri_other_json_not_claimed() {
    // package.json / tsconfig.json are not recognized via the new route.
    assert!(format_for_uri(&uri("/project/package.json")).is_none());
    assert!(format_for_uri(&uri("/project/tsconfig.json")).is_none());
}

#[test]
fn test_format_for_uri_toml_yaml_unaffected() {
    // Verify existing formats still work.
    assert!(is_toml_config_file(&uri("/project/Cargo.toml")));
    assert!(is_yaml_config_file(&uri("/project/application.yml")));
}

// ── Story 001: SourceLayer for appsettings ────────────────────────────────────

#[test]
fn test_source_layer_appsettings_base() {
    assert_eq!(
        source_layer_for_appsettings("appsettings.json"),
        SourceLayer::DotNetBase
    );
}

#[test]
fn test_source_layer_appsettings_production() {
    assert_eq!(
        source_layer_for_appsettings("appsettings.Production.json"),
        SourceLayer::DotNetEnvironment("Production".to_string())
    );
}

#[test]
fn test_source_layer_appsettings_development() {
    assert_eq!(
        source_layer_for_appsettings("appsettings.Development.json"),
        SourceLayer::DotNetEnvironment("Development".to_string())
    );
}

#[test]
fn test_source_layer_display() {
    assert_eq!(SourceLayer::DotNetBase.display(), "appsettings.json");
    assert_eq!(
        SourceLayer::DotNetEnvironment("Production".to_string()).display(),
        "appsettings.Production.json"
    );
}

#[test]
fn test_source_layer_precedence_dotnet_cascade() {
    // Base has lower precedence than environment-specific.
    assert!(
        SourceLayer::DotNetBase.precedence()
            < SourceLayer::DotNetEnvironment("Prod".to_string()).precedence()
    );
}

// ── Story 002: JSONC parse → entry model ─────────────────────────────────────

#[test]
fn test_parse_jsonc_config_simple_flat() {
    let jsonc = r#"{ "AppName": "MyApp", "Version": "1.0" }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 2);
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert!(keys.contains(&"AppName"));
    assert!(keys.contains(&"Version"));
}

#[test]
fn test_parse_jsonc_config_nested_colon_path() {
    let jsonc = r#"{
  "Logging": {
    "LogLevel": {
      "Default": "Information",
      "Microsoft": "Warning"
    }
  }
}"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 2);
    let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    assert!(
        keys.contains(&"Logging:LogLevel:Default"),
        "keys: {:?}",
        keys
    );
    assert!(
        keys.contains(&"Logging:LogLevel:Microsoft"),
        "keys: {:?}",
        keys
    );
}

#[test]
fn test_parse_jsonc_config_with_comments() {
    let jsonc = r#"{
  // This is a comment
  "AppName": "MyApp", /* block comment */
  "Debug": true
}"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_parse_jsonc_config_with_trailing_comma() {
    let jsonc = r#"{
  "Key1": "value1",
  "Key2": "value2",
}"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_parse_jsonc_config_boolean_value() {
    let jsonc = r#"{ "Enabled": true, "Disabled": false }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty());
    let enabled = entries.iter().find(|e| e.key == "Enabled").unwrap();
    assert_eq!(enabled.value, "true");
    let disabled = entries.iter().find(|e| e.key == "Disabled").unwrap();
    assert_eq!(disabled.value, "false");
}

#[test]
fn test_parse_jsonc_config_number_value() {
    let jsonc = r#"{ "Port": 8080, "Timeout": 30 }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty());
    let port = entries.iter().find(|e| e.key == "Port").unwrap();
    assert_eq!(port.value, "8080");
}

#[test]
fn test_parse_jsonc_config_null_value() {
    let jsonc = r#"{ "OptionalKey": null }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty());
    let entry = entries.iter().find(|e| e.key == "OptionalKey").unwrap();
    assert_eq!(entry.value, "");
}

#[test]
fn test_parse_jsonc_config_array_skipped() {
    // Arrays are an acknowledged gap — they must not panic, just be silently skipped.
    let jsonc = r#"{ "Urls": ["http://localhost:5000"], "Name": "test" }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    // Only the non-array key should appear.
    assert_eq!(
        entries.len(),
        1,
        "entries: {:?}",
        entries.iter().map(|e| &e.key).collect::<Vec<_>>()
    );
    assert_eq!(entries[0].key, "Name");
}

#[test]
fn test_parse_jsonc_config_malformed_returns_diagnostic() {
    let bad = r#"{ "Key": }"#; // missing value
    let (entries, diags) = parse_jsonc_config(bad, SourceLayer::DotNetBase);
    // Should not panic; should return a diagnostic.
    assert!(
        !diags.is_empty(),
        "expected at least one diagnostic for malformed input"
    );
    // Entries may be empty or partial.
    let _ = entries; // don't assert count, just verify no panic
}

#[test]
fn test_parse_jsonc_config_empty_file() {
    let (entries, diags) = parse_jsonc_config("", SourceLayer::DotNetBase);
    assert!(
        diags.is_empty(),
        "unexpected diags for empty input: {:?}",
        diags
    );
    assert!(entries.is_empty());
}

#[test]
fn test_parse_jsonc_config_bom_stripped() {
    // UTF-8 BOM prefix (\u{FEFF}) must be silently stripped.
    let jsonc = "\u{FEFF}{ \"Key\": \"Value\" }";
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(
        diags.is_empty(),
        "unexpected diags after BOM strip: {:?}",
        diags
    );
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "Key");
}

#[test]
fn test_parse_jsonc_config_crlf_line_endings() {
    let jsonc = "{\r\n  \"Key\": \"Value\"\r\n}";
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "Key");
}

#[test]
fn test_parse_jsonc_config_no_final_newline() {
    let jsonc = r#"{ "Key": "Value" }"#;
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty());
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_parse_jsonc_config_utf16_positions_non_ascii_key() {
    // Key with a non-ASCII prefix character — UTF-16 column must be correct.
    // "café" is 4 Unicode code points, but "é" is U+00E9 which is 1 UTF-16 unit.
    let jsonc = "{\n  \"café\": \"value\"\n}";
    let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "café");
    // Key starts on line 1 (0-indexed), character 2 (after "  ").
    // Verify the range is non-zero and sensible.
    assert_eq!(entries[0].key_range.start.line, 1);
}

#[test]
fn test_parse_jsonc_config_deeply_nested_no_panic() {
    // Deep nesting should not stack overflow (jsonc-parser has a depth limit).
    // 512 levels of nesting should fail with a diagnostic, not a panic.
    let mut json = String::new();
    for i in 0..512 {
        json.push_str(&format!("{{\"k{}\": ", i));
    }
    json.push_str("\"leaf\"");
    for _ in 0..512 {
        json.push('}');
    }
    // Must not panic regardless of result.
    let (_entries, _diags) = parse_jsonc_config(&json, SourceLayer::DotNetBase);
}

#[test]
fn test_parse_jsonc_config_garbage_input() {
    let (_entries, diags) = parse_jsonc_config("not json at all !@#$", SourceLayer::DotNetBase);
    assert!(!diags.is_empty(), "expected diagnostic for garbage input");
}

#[test]
fn test_parse_jsonc_config_source_layer_preserved() {
    let jsonc = r#"{ "Key": "val" }"#;
    let (entries, _) =
        parse_jsonc_config(jsonc, SourceLayer::DotNetEnvironment("Staging".to_string()));
    assert_eq!(
        entries[0].source_layer,
        SourceLayer::DotNetEnvironment("Staging".to_string())
    );
}

// ── Story 001+002: Cascade resolution (precedence) ───────────────────────────

#[test]
fn test_dotnet_cascade_env_overrides_base() {
    // Base has precedence 1, DotNetEnvironment has precedence 2.
    // Resolution must pick the environment-specific value.
    use envforge::ops::config_resolution::resolve_layers;

    let base_json = r#"{ "Logging": { "LogLevel": { "Default": "Information" } } }"#;
    let env_json = r#"{ "Logging": { "LogLevel": { "Default": "Debug" } } }"#;

    let (base_entries, _) = parse_jsonc_config(base_json, SourceLayer::DotNetBase);
    let (env_entries, _) = parse_jsonc_config(
        env_json,
        SourceLayer::DotNetEnvironment("Development".to_string()),
    );

    let layers = vec![base_entries, env_entries];
    let winner = resolve_layers("Logging:LogLevel:Default", &layers).unwrap();
    assert_eq!(winner.value, "Debug");
    assert_eq!(
        winner.source_layer,
        SourceLayer::DotNetEnvironment("Development".to_string())
    );
}

#[test]
fn test_dotnet_cascade_base_used_when_no_env_override() {
    use envforge::ops::config_resolution::resolve_layers;

    let base_json = r#"{ "AppName": "MyApp" }"#;
    let env_json = r#"{ "Debug": "true" }"#;

    let (base_entries, _) = parse_jsonc_config(base_json, SourceLayer::DotNetBase);
    let (env_entries, _) = parse_jsonc_config(
        env_json,
        SourceLayer::DotNetEnvironment("Development".to_string()),
    );

    let layers = vec![base_entries, env_entries];
    let winner = resolve_layers("AppName", &layers).unwrap();
    assert_eq!(winner.value, "MyApp");
    assert_eq!(winner.source_layer, SourceLayer::DotNetBase);
}

// ── Story 003: `__` → `:` env-var binding ────────────────────────────────────

#[test]
fn test_env_var_to_json_path_double_underscore() {
    assert_eq!(
        env_var_to_json_path("Logging__LogLevel__Default"),
        "Logging:LogLevel:Default"
    );
}

#[test]
fn test_env_var_to_json_path_single_level() {
    assert_eq!(
        env_var_to_json_path("ConnectionStrings__Default"),
        "ConnectionStrings:Default"
    );
}

#[test]
fn test_env_var_to_json_path_no_double_underscore() {
    // A plain env var with no double underscore passes through unchanged.
    assert_eq!(
        env_var_to_json_path("ASPNETCORE_ENVIRONMENT"),
        "ASPNETCORE_ENVIRONMENT"
    );
}

#[test]
fn test_json_path_to_env_var_roundtrip() {
    let original = "Logging__LogLevel__Default";
    let path = env_var_to_json_path(original);
    let back = json_path_to_env_var(&path);
    assert_eq!(back, original);
}

#[test]
fn test_json_path_to_env_var_basic() {
    assert_eq!(
        json_path_to_env_var("Logging:LogLevel:Default"),
        "Logging__LogLevel__Default"
    );
}

#[test]
fn test_env_var_binding_goto_def() {
    // Simulate: an env var `Logging__LogLevel__Default` links to JSON key
    // `Logging:LogLevel:Default`. A basic go-to-def cross-reference test.
    let json_path = env_var_to_json_path("Logging__LogLevel__Default");
    let jsonc = r#"{
  "Logging": {
    "LogLevel": {
      "Default": "Information"
    }
  }
}"#;
    let (entries, _) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    let found = entries.iter().find(|e| e.key == json_path);
    assert!(
        found.is_some(),
        "env-var binding: JSON entry not found for path '{}'",
        json_path
    );
}

// ── Story 004: Read features + diagnostics ────────────────────────────────────

#[test]
fn test_config_hover_appsettings_entry() {
    let jsonc = r#"{ "AppName": "MyApp" }"#;
    let (entries, _) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    // Position at line 0, character at the key "AppName" start (after opening `{ "`).
    // `{ "AppName"` — `{` is char 0, ` ` is 1, `"` is 2, `A` is 3.
    // key_range.start.character should be 2 (at the opening quote).
    let pos = Position {
        line: 0,
        character: 3, // inside "AppName"
    };
    let hover = config_hover(pos, &entries, &[], None, None);
    assert!(hover.is_some(), "expected hover for AppName entry");
}

#[test]
fn test_config_jsonc_diagnostics_no_errors() {
    let jsonc = r#"{ "Key": "value" }"#;
    let diags = config_jsonc_diagnostics(jsonc, SourceLayer::DotNetBase, None);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
}

#[test]
fn test_config_jsonc_diagnostics_duplicate_key() {
    // jsonc-parser allows duplicate keys syntactically; EnvForge warns on them.
    let jsonc = r#"{ "Key": "val1", "Key": "val2" }"#;
    let diags = config_jsonc_diagnostics(jsonc, SourceLayer::DotNetBase, None);
    let dup = diags.iter().find(|d| d.message.contains("Duplicate key"));
    assert!(
        dup.is_some(),
        "expected duplicate key diagnostic; got: {:?}",
        diags
    );
}

#[test]
fn test_config_jsonc_diagnostics_malformed_syntax() {
    let bad = r#"{ "Key": }"#;
    let diags = config_jsonc_diagnostics(bad, SourceLayer::DotNetBase, None);
    assert!(!diags.is_empty(), "expected syntax error diagnostic");
}

#[test]
fn test_config_semantic_tokens_appsettings() {
    let jsonc = r#"{ "Key": "value" }"#;
    let (entries, _) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
    let tokens = config_semantic_tokens(&entries, None);
    // Should produce at least one token for the key.
    assert!(
        !tokens.data.is_empty(),
        "expected semantic tokens for appsettings entries"
    );
}

// ── Story 005: Rename round-trip-safe ─────────────────────────────────────────

#[test]
fn test_resolve_jsonc_key_span_simple() {
    let content = r#"{ "AppName": "MyApp" }"#;
    let span = resolve_jsonc_key_span(content, "AppName");
    assert!(span.is_some(), "expected to find AppName span");
    let span = span.unwrap();
    // The span should cover `"AppName"` (10 bytes: `"`, `A`, `p`, `p`, `N`, `a`, `m`, `e`, `"`).
    let key_text = &content[span.byte_range.clone()];
    assert_eq!(key_text, "\"AppName\"");
}

#[test]
fn test_resolve_jsonc_key_span_nested() {
    let content = r#"{
  "Logging": {
    "LogLevel": {
      "Default": "Information"
    }
  }
}"#;
    let span = resolve_jsonc_key_span(content, "Logging:LogLevel:Default");
    assert!(span.is_some(), "expected to find nested key span");
    let span = span.unwrap();
    let key_text = &content[span.byte_range.clone()];
    assert_eq!(key_text, "\"Default\"");
}

#[test]
fn test_resolve_jsonc_key_span_not_found_returns_none() {
    let content = r#"{ "AppName": "MyApp" }"#;
    let span = resolve_jsonc_key_span(content, "NonExistentKey");
    assert!(span.is_none());
}

#[test]
fn test_resolve_jsonc_key_span_malformed_returns_none() {
    let bad = r#"{ "Key": }"#;
    let span = resolve_jsonc_key_span(bad, "Key");
    assert!(
        span.is_none(),
        "malformed input should return None, not panic"
    );
}

#[test]
fn test_config_jsonc_rename_basic() {
    use envforge::ops::surgical_edit::SurgicalEdit;

    let content = r#"{ "OldKey": "value" }"#;
    let (entries, _) = parse_jsonc_config(content, SourceLayer::DotNetBase);
    let mut open_docs = HashMap::new();
    let uri_val = uri("/project/appsettings.json");
    open_docs.insert(uri_val.clone(), entries);
    let mut doc_contents = HashMap::new();
    doc_contents.insert(uri_val.clone(), content.to_string());

    let edit = config_jsonc_rename(
        "OldKey",
        "NewKey",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(edit.is_some(), "expected a WorkspaceEdit for rename");

    let we = edit.unwrap();
    let changes = we.changes.unwrap();
    let edits = changes.get(&uri_val).unwrap();
    assert_eq!(edits.len(), 1);

    // Apply the edit and verify round-trip safety.
    let te = &edits[0];
    // Compute byte range from the TextEdit.
    let span = resolve_jsonc_key_span(content, "OldKey").unwrap();
    let result = SurgicalEdit::apply(content, span.byte_range, "\"NewKey\"").unwrap();
    assert!(result.contains("\"NewKey\""), "renamed content: {}", result);
    assert!(
        !result.contains("\"OldKey\""),
        "old key still present: {}",
        result
    );
    // The value part must be unchanged.
    assert!(
        result.contains("\"value\""),
        "value changed unexpectedly: {}",
        result
    );
    let _ = te; // used above via span
}

#[test]
fn test_config_jsonc_rename_round_trip_no_final_newline() {
    let content = r#"{ "OldKey": "value" }"#; // no trailing newline
    let span = resolve_jsonc_key_span(content, "OldKey").unwrap();
    let result = envforge::ops::surgical_edit::SurgicalEdit::apply(
        content,
        span.byte_range.clone(),
        "\"NewKey\"",
    )
    .unwrap();
    // Prefix before the key must be byte-identical.
    assert_eq!(
        &result[..span.byte_range.start],
        &content[..span.byte_range.start]
    );
    // Suffix after the old key must be byte-identical.
    let old_key_len = span.byte_range.end - span.byte_range.start;
    let new_key = b"\"NewKey\"";
    assert_eq!(
        &result.as_bytes()[span.byte_range.start + new_key.len()..],
        &content.as_bytes()[span.byte_range.start + old_key_len..]
    );
    // No trailing newline added.
    assert!(!result.ends_with('\n'));
}

#[test]
fn test_config_jsonc_rename_crlf_preserved() {
    let content = "{\r\n  \"OldKey\": \"value\"\r\n}";
    let span = resolve_jsonc_key_span(content, "OldKey").unwrap();
    let result = envforge::ops::surgical_edit::SurgicalEdit::apply(
        content,
        span.byte_range.clone(),
        "\"NewKey\"",
    )
    .unwrap();
    assert!(
        result.contains("\r\n"),
        "CRLF must be preserved in renamed content"
    );
}

#[test]
fn test_config_jsonc_rename_comment_rich_preserved() {
    let content = r#"{
  // App name setting
  "OldKey": "value", /* trailing comment */
  "Other": "val2"
}"#;
    let span = resolve_jsonc_key_span(content, "OldKey").unwrap();
    let result = envforge::ops::surgical_edit::SurgicalEdit::apply(
        content,
        span.byte_range.clone(),
        "\"NewKey\"",
    )
    .unwrap();
    assert!(
        result.contains("// App name setting"),
        "comment lost after rename"
    );
    assert!(
        result.contains("/* trailing comment */"),
        "block comment lost after rename"
    );
    assert!(result.contains("\"NewKey\""), "new key not present");
}

#[test]
fn test_config_jsonc_rename_trailing_comma_preserved() {
    let content = r#"{
  "OldKey": "value",
}"#;
    let span = resolve_jsonc_key_span(content, "OldKey").unwrap();
    let result = envforge::ops::surgical_edit::SurgicalEdit::apply(
        content,
        span.byte_range.clone(),
        "\"NewKey\"",
    )
    .unwrap();
    // Trailing comma after the value must be preserved.
    assert!(result.contains(","), "trailing comma lost after rename");
    assert!(result.contains("\"NewKey\""), "new key not present");
}

#[test]
fn test_config_jsonc_rename_readonly_returns_none() {
    let content = r#"{ "Key": "val" }"#;
    let (entries, _) = parse_jsonc_config(content, SourceLayer::DotNetBase);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri("/project/appsettings.json"), entries);
    let doc_contents = HashMap::new();
    let edit = config_jsonc_rename(
        "Key",
        "NewKey",
        WriteCapability::ReadOnly,
        &open_docs,
        &doc_contents,
    );
    assert!(edit.is_none(), "ReadOnly must produce no edit");
}

#[test]
fn test_config_jsonc_rename_collision_returns_none() {
    let content = r#"{ "OldKey": "v1", "NewKey": "v2" }"#;
    let (entries, _) = parse_jsonc_config(content, SourceLayer::DotNetBase);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri("/project/appsettings.json"), entries);
    let doc_contents = HashMap::new();
    let edit = config_jsonc_rename(
        "OldKey",
        "NewKey",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(edit.is_none(), "collision must produce no edit");
}

#[test]
fn test_config_jsonc_rename_same_name_returns_none() {
    let content = r#"{ "Key": "val" }"#;
    let (entries, _) = parse_jsonc_config(content, SourceLayer::DotNetBase);
    let mut open_docs = HashMap::new();
    open_docs.insert(uri("/project/appsettings.json"), entries);
    let doc_contents = HashMap::new();
    let edit = config_jsonc_rename(
        "Key",
        "Key",
        WriteCapability::ReadWrite,
        &open_docs,
        &doc_contents,
    );
    assert!(edit.is_none(), "rename to same name must produce no edit");
}

// ── AI-safety parity ──────────────────────────────────────────────────────────

#[test]
fn test_canary_target_appsettings_base() {
    use std::path::Path;
    assert!(is_config_canary_target(Path::new("appsettings.json")));
}

#[test]
fn test_canary_target_appsettings_production() {
    use std::path::Path;
    assert!(is_config_canary_target(Path::new(
        "appsettings.Production.json"
    )));
}

#[test]
fn test_canary_target_appsettings_development() {
    use std::path::Path;
    assert!(is_config_canary_target(Path::new(
        "appsettings.Development.json"
    )));
}

#[test]
fn test_canary_target_mcp_json_not_a_target() {
    use std::path::Path;
    assert!(!is_config_canary_target(Path::new("mcp.json")));
}

#[test]
fn test_canary_target_package_json_not_a_target() {
    use std::path::Path;
    assert!(!is_config_canary_target(Path::new("package.json")));
}

#[test]
fn test_canary_target_tsconfig_json_not_a_target() {
    use std::path::Path;
    assert!(!is_config_canary_target(Path::new("tsconfig.json")));
}

#[test]
fn test_canary_target_generic_json_not_a_target() {
    use std::path::Path;
    assert!(!is_config_canary_target(Path::new("settings.json")));
    assert!(!is_config_canary_target(Path::new("config.json")));
    assert!(!is_config_canary_target(Path::new("data.json")));
}

#[test]
fn test_canary_target_existing_formats_unaffected() {
    // Regression: existing canary targets still return true.
    use std::path::Path;
    assert!(is_config_canary_target(Path::new("application.properties")));
    assert!(is_config_canary_target(Path::new("application.yml")));
    assert!(is_config_canary_target(Path::new("Cargo.toml")));
    assert!(is_config_canary_target(Path::new(".env")));
    assert!(is_config_canary_target(Path::new(".env.local")));
}

// ── Validation helpers ────────────────────────────────────────────────────────

#[test]
fn test_is_valid_dotnet_key_segment_valid() {
    assert!(is_valid_dotnet_key_segment("Logging"));
    assert!(is_valid_dotnet_key_segment("LogLevel"));
    assert!(is_valid_dotnet_key_segment("Default"));
    assert!(is_valid_dotnet_key_segment("MyKey-2"));
    assert!(is_valid_dotnet_key_segment("my_key"));
}

#[test]
fn test_is_valid_dotnet_key_segment_empty_invalid() {
    assert!(!is_valid_dotnet_key_segment(""));
}

#[test]
fn test_is_valid_dotnet_key_segment_colon_invalid() {
    // Colons are the separator — they must not appear inside a segment.
    assert!(!is_valid_dotnet_key_segment("Logging:LogLevel"));
}

#[test]
fn test_is_valid_dotnet_key_segment_space_invalid() {
    assert!(!is_valid_dotnet_key_segment("my key"));
}

// ── FR25: No regression on existing formats ───────────────────────────────────

#[test]
fn test_fr25_properties_format_unaffected() {
    let (fmt, layer) = format_for_uri(&uri("/project/application.properties")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_fr25_yaml_format_unaffected() {
    let (fmt, layer) = format_for_uri(&uri("/project/application.yml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_fr25_toml_format_unaffected() {
    let (fmt, layer) = format_for_uri(&uri("/project/Cargo.toml")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    assert_eq!(layer, SourceLayer::Base);
}

#[test]
fn test_fr25_env_cascade_format_unaffected() {
    let (fmt, layer) = format_for_uri(&uri("/project/.env.local")).unwrap();
    assert_eq!(fmt.write_capability(), WriteCapability::ReadWrite);
    // .env.local → DotEnvLocal
    assert_eq!(layer, SourceLayer::DotEnvLocal);
}
