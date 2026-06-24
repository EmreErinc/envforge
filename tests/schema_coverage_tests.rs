//! Coverage for `ops::schema` gaps not exercised by the in-module tests:
//! `generate_docs`, numeric min/max boundaries, per-environment override
//! resolution, url/port/regex/enum validation, drift for schema-only keys, and
//! generation type inference.

use envforge::ops::schema::{
    detect_drift, generate_docs, generate_schema, parse_schema_content, validate_against_schema,
    DriftStatus, EnvSchema, SchemaVariable, VarType,
};
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

// ---- generate_docs (entirely untested before) ------------------------------

#[test]
fn test_generate_docs_table_required_first_and_sensitive_marker() {
    let mut variables = HashMap::new();
    variables.insert(
        "DB_HOST".to_string(),
        SchemaVariable {
            var_type: VarType::String,
            required: true,
            description: Some("Database hostname".to_string()),
            ..Default::default()
        },
    );
    variables.insert(
        "API_KEY".to_string(),
        SchemaVariable {
            var_type: VarType::String,
            required: false,
            sensitive: true,
            ..Default::default()
        },
    );
    let docs = generate_docs(&EnvSchema { variables });

    assert!(docs.contains("# Environment Variables"));
    assert!(docs.contains("| Variable | Type | Required | Default | Description |"));
    assert!(docs.contains("Database hostname"));
    assert!(docs.contains("API_KEY [sensitive]"));
    // Required variables are listed before optional ones.
    let host_at = docs.find("DB_HOST").unwrap();
    let key_at = docs.find("API_KEY").unwrap();
    assert!(host_at < key_at, "required rows must precede optional rows");
    // Missing default renders as the em-dash placeholder.
    assert!(docs.contains(" — "));
}

// ---- validate_against_schema: numeric boundaries ---------------------------

#[test]
fn test_validate_number_min_max_boundaries() {
    let schema = parse_schema_content(
        r#"
[PORT]
type = "number"
min = 1
max = 65535
"#,
    )
    .unwrap();
    let rules = HashMap::new();

    assert!(
        !validate_against_schema(&env(&[("PORT", "8080")]), &schema, None, &rules)
            .iter()
            .any(|e| e.key == "PORT")
    );
    assert!(
        validate_against_schema(&env(&[("PORT", "0")]), &schema, None, &rules)
            .iter()
            .any(|e| e.message.contains("below minimum"))
    );
    assert!(
        validate_against_schema(&env(&[("PORT", "70000")]), &schema, None, &rules)
            .iter()
            .any(|e| e.message.contains("exceeds maximum"))
    );
}

// ---- validate_against_schema: url / port / regex / enum-no-values ----------

#[test]
fn test_validate_url_port_regex_types() {
    let schema = parse_schema_content(
        r#"
[API_URL]
type = "url"
[LISTEN]
type = "port"
[CODE]
type = "regex"
pattern = "[unclosed"
"#,
    )
    .unwrap();
    let rules = HashMap::new();

    assert!(
        validate_against_schema(&env(&[("API_URL", "not a url")]), &schema, None, &rules)
            .iter()
            .any(|e| e.key == "API_URL")
    );
    assert!(
        validate_against_schema(&env(&[("API_URL", "https://x.com")]), &schema, None, &rules)
            .iter()
            .all(|e| e.key != "API_URL")
    );
    assert!(
        validate_against_schema(&env(&[("LISTEN", "99999")]), &schema, None, &rules)
            .iter()
            .any(|e| e.key == "LISTEN")
    );
    // An invalid regex pattern surfaces as a validation error, not a panic.
    let errs = validate_against_schema(&env(&[("CODE", "abc")]), &schema, None, &rules);
    assert!(errs
        .iter()
        .any(|e| e.key == "CODE" && e.message.contains("invalid regex pattern")));
}

#[test]
fn test_validate_enum_without_values_passes() {
    let schema = parse_schema_content("[MODE]\ntype = \"enum\"\n").unwrap();
    let errs = validate_against_schema(
        &env(&[("MODE", "anything")]),
        &schema,
        None,
        &HashMap::new(),
    );
    assert!(errs.iter().all(|e| e.key != "MODE"));
}

// ---- per-environment override resolution -----------------------------------

#[test]
fn test_parse_schema_populates_nested_env_overrides() {
    // Idiomatic `[VAR.environment]` parses as a TOML sub-table of VAR.
    // `parse_schema_content` must lift those sub-tables into `env_overrides`.
    let schema = parse_schema_content(
        r#"
[LOG_LEVEL]
type = "enum"
values = ["info", "warn"]

[LOG_LEVEL.production]
values = ["error"]
"#,
    )
    .unwrap();
    let var = schema.variables.get("LOG_LEVEL").unwrap();
    let ov = var.env_overrides.get("production").unwrap();
    assert_eq!(ov.values.as_deref(), Some(&["error".to_string()][..]));
}

#[test]
fn test_parse_schema_supports_quoted_dotted_override_key() {
    // Backward-compat: the quoted literal dotted-key form still works.
    let schema = parse_schema_content(
        r#"
[PORT]
type = "number"

["PORT.production"]
min = 1024
"#,
    )
    .unwrap();
    let ov = schema
        .variables
        .get("PORT")
        .unwrap()
        .env_overrides
        .get("production")
        .unwrap();
    assert_eq!(ov.min, Some(1024.0));
}

#[test]
fn test_validate_applies_environment_overrides() {
    // End-to-end: idiomatic nested overrides flow through parse -> resolve -> validate.
    let schema = parse_schema_content(
        r#"
[LOG_LEVEL]
type = "enum"
values = ["info", "warn"]

[LOG_LEVEL.production]
values = ["error"]

[DEBUG]
type = "bool"
required = false

[DEBUG.production]
required = true
"#,
    )
    .unwrap();
    let rules = HashMap::new();

    // Base: "error" is not in [info, warn]; production override permits it.
    assert!(
        validate_against_schema(&env(&[("LOG_LEVEL", "error")]), &schema, None, &rules)
            .iter()
            .any(|e| e.key == "LOG_LEVEL")
    );
    assert!(validate_against_schema(
        &env(&[("LOG_LEVEL", "error")]),
        &schema,
        Some("production"),
        &rules
    )
    .iter()
    .all(|e| e.key != "LOG_LEVEL"));

    // DEBUG is optional at base but required in production.
    assert!(validate_against_schema(&env(&[]), &schema, None, &rules)
        .iter()
        .all(|e| e.key != "DEBUG"));
    assert!(
        validate_against_schema(&env(&[]), &schema, Some("production"), &rules)
            .iter()
            .any(|e| e.key == "DEBUG" && e.message.contains("required"))
    );
}

// ---- detect_drift: schema-only key -----------------------------------------

#[test]
fn test_detect_drift_schema_only_key_is_missing() {
    let schema = parse_schema_content("[ONLY_IN_SCHEMA]\ntype = \"string\"\n").unwrap();
    let files = vec![("dev".to_string(), env(&[("A", "1")]))];
    let drift = detect_drift(&files, Some(&schema));
    let entry = drift.iter().find(|d| d.key == "ONLY_IN_SCHEMA").unwrap();
    assert_eq!(entry.status, DriftStatus::Missing);
}

// ---- generate_schema: type inference & sensitivity -------------------------

#[test]
fn test_generate_schema_infers_email_string_and_sensitive() {
    let e = env(&[
        ("CONTACT", "a@b.com"),
        ("APP_NAME", "hello world"),
        ("DB_CREDENTIAL", "x"),
    ]);
    let out = generate_schema(&e);
    assert!(out.contains("[CONTACT]"));
    assert!(out.contains("type = \"email\""));
    assert!(out.contains("type = \"string\"")); // APP_NAME falls back to string
    assert!(out.contains("sensitive = true")); // DB_CREDENTIAL matches CREDENTIAL
}
