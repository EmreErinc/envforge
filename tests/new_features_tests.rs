use std::collections::HashMap;
use std::collections::HashSet;

// ─── Schema Parser Tests ────────────────────────────────────

#[test]
fn test_schema_parse_basic() {
    use envforge::ops::schema::{parse_schema_content, VarType};

    let content = r#"
[PORT]
type = "number"
required = true
min = 1024
max = 65535
description = "HTTP server port"

[DEBUG]
type = "bool"
default = "false"
"#;

    let schema = parse_schema_content(content).unwrap();
    assert_eq!(schema.variables.len(), 2);

    let port = &schema.variables["PORT"];
    assert_eq!(port.var_type, VarType::Number);
    assert!(port.required);
    assert_eq!(port.min, Some(1024.0));
    assert_eq!(port.max, Some(65535.0));
    assert_eq!(port.description.as_deref(), Some("HTTP server port"));

    let debug = &schema.variables["DEBUG"];
    assert_eq!(debug.var_type, VarType::Bool);
    assert!(!debug.required);
    assert_eq!(debug.default.as_deref(), Some("false"));
}

#[test]
fn test_schema_parse_enum() {
    use envforge::ops::schema::parse_schema_content;

    let content = r#"
[NODE_ENV]
type = "enum"
required = true
values = ["development", "staging", "production"]
"#;

    let schema = parse_schema_content(content).unwrap();
    let var = &schema.variables["NODE_ENV"];
    assert_eq!(var.values.as_ref().unwrap().len(), 3);
    assert!(var
        .values
        .as_ref()
        .unwrap()
        .contains(&"production".to_string()));
}

#[test]
fn test_schema_parse_sensitive() {
    use envforge::ops::schema::parse_schema_content;

    let content = r#"
[API_KEY]
type = "string"
required = true
sensitive = true
pattern = "^sk-"
"#;

    let schema = parse_schema_content(content).unwrap();
    let var = &schema.variables["API_KEY"];
    assert!(var.sensitive);
    assert_eq!(var.pattern.as_deref(), Some("^sk-"));
}

#[test]
fn test_schema_parse_env_overrides() {
    use envforge::ops::schema::parse_schema_content;

    let content = r#"
[DATABASE_URL]
type = "url"
required = true

["DATABASE_URL.production"]
pattern = "^postgres://prod-"
"#;

    let schema = parse_schema_content(content).unwrap();
    let var = &schema.variables["DATABASE_URL"];
    assert!(var.env_overrides.contains_key("production"));
    assert_eq!(
        var.env_overrides["production"].pattern.as_deref(),
        Some("^postgres://prod-")
    );
}

#[test]
fn test_schema_parse_empty() {
    use envforge::ops::schema::parse_schema_content;

    let schema = parse_schema_content("").unwrap();
    assert!(schema.variables.is_empty());
}

#[test]
fn test_schema_parse_unknown_type() {
    use envforge::ops::schema::parse_schema_content;

    let content = r#"
[VAR]
type = "unknown_type"
"#;

    let result = parse_schema_content(content);
    assert!(result.is_err());
}

#[test]
fn test_schema_default_type_is_string() {
    use envforge::ops::schema::{parse_schema_content, VarType};

    let content = r#"
[SIMPLE]
description = "no type specified"
"#;

    let schema = parse_schema_content(content).unwrap();
    assert_eq!(schema.variables["SIMPLE"].var_type, VarType::String);
}

// ─── Schema Validation Tests ────────────────────────────────

#[test]
fn test_validate_number_valid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema =
        parse_schema_content("[PORT]\ntype = \"number\"\nmin = 1024\nmax = 65535\n").unwrap();
    let mut env = HashMap::new();
    env.insert("PORT".into(), "3000".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert!(errors.is_empty());
}

#[test]
fn test_validate_number_out_of_range() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema =
        parse_schema_content("[PORT]\ntype = \"number\"\nmin = 1024\nmax = 65535\n").unwrap();
    let mut env = HashMap::new();
    env.insert("PORT".into(), "80".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("below minimum"));
}

#[test]
fn test_validate_number_not_a_number() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[PORT]\ntype = \"number\"\n").unwrap();
    let mut env = HashMap::new();
    env.insert("PORT".into(), "banana".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("not a valid number"));
}

#[test]
fn test_validate_bool_valid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[DEBUG]\ntype = \"bool\"\n").unwrap();
    for val in &["true", "false", "1", "0", "yes", "no"] {
        let mut env = HashMap::new();
        env.insert("DEBUG".into(), val.to_string());
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(errors.is_empty(), "Failed for value: {}", val);
    }
}

#[test]
fn test_validate_bool_invalid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[DEBUG]\ntype = \"bool\"\n").unwrap();
    let mut env = HashMap::new();
    env.insert("DEBUG".into(), "maybe".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_validate_url_valid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[API]\ntype = \"url\"\n").unwrap();
    let mut env = HashMap::new();
    env.insert("API".into(), "https://example.com".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert!(errors.is_empty());
}

#[test]
fn test_validate_url_invalid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[API]\ntype = \"url\"\n").unwrap();
    let mut env = HashMap::new();
    env.insert("API".into(), "not-a-url".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_validate_enum_valid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let content = "[ENV]\ntype = \"enum\"\nvalues = [\"dev\", \"prod\"]\n";
    let schema = parse_schema_content(content).unwrap();
    let mut env = HashMap::new();
    env.insert("ENV".into(), "dev".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert!(errors.is_empty());
}

#[test]
fn test_validate_enum_invalid() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let content = "[ENV]\ntype = \"enum\"\nvalues = [\"dev\", \"prod\"]\n";
    let schema = parse_schema_content(content).unwrap();
    let mut env = HashMap::new();
    env.insert("ENV".into(), "staging".into());
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("not one of"));
}

#[test]
fn test_validate_required_missing() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[PORT]\ntype = \"number\"\nrequired = true\n").unwrap();
    let env = HashMap::new(); // empty — PORT is missing
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("required"));
}

#[test]
fn test_validate_required_with_default_not_error() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let content = "[PORT]\ntype = \"number\"\nrequired = true\ndefault = \"3000\"\n";
    let schema = parse_schema_content(content).unwrap();
    let env = HashMap::new();
    let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
    assert!(errors.is_empty()); // default exists, so not an error
}

#[test]
fn test_validate_port() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[P]\ntype = \"port\"\n").unwrap();

    let mut env = HashMap::new();
    env.insert("P".into(), "8080".into());
    assert!(validate_against_schema(&env, &schema, None, &HashMap::new()).is_empty());

    env.insert("P".into(), "banana".into());
    assert_eq!(
        validate_against_schema(&env, &schema, None, &HashMap::new()).len(),
        1
    );
}

#[test]
fn test_validate_email() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let schema = parse_schema_content("[E]\ntype = \"email\"\n").unwrap();

    let mut env = HashMap::new();
    env.insert("E".into(), "user@example.com".into());
    assert!(validate_against_schema(&env, &schema, None, &HashMap::new()).is_empty());

    env.insert("E".into(), "not-an-email".into());
    assert_eq!(
        validate_against_schema(&env, &schema, None, &HashMap::new()).len(),
        1
    );
}

#[test]
fn test_validate_regex() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let content = "[CODE]\ntype = \"regex\"\npattern = \"^[A-Z]{3}-\\\\d{4}$\"\n";
    let schema = parse_schema_content(content).unwrap();

    let mut env = HashMap::new();
    env.insert("CODE".into(), "ABC-1234".into());
    assert!(validate_against_schema(&env, &schema, None, &HashMap::new()).is_empty());

    env.insert("CODE".into(), "invalid".into());
    assert_eq!(
        validate_against_schema(&env, &schema, None, &HashMap::new()).len(),
        1
    );
}

#[test]
fn test_validate_env_override() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    let content = r#"
[DB_URL]
type = "url"
required = true

["DB_URL.production"]
pattern = "^https://prod-"
"#;
    let schema = parse_schema_content(content).unwrap();

    // Without environment — any URL is fine
    let mut env = HashMap::new();
    env.insert("DB_URL".into(), "http://localhost".into());
    assert!(validate_against_schema(&env, &schema, None, &HashMap::new()).is_empty());

    // With production environment — must match pattern
    let errors = validate_against_schema(&env, &schema, Some("production"), &HashMap::new());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("does not match pattern"));

    // Correct production URL
    env.insert("DB_URL".into(), "https://prod-db.example.com".into());
    assert!(validate_against_schema(&env, &schema, Some("production"), &HashMap::new()).is_empty());
}

#[test]
fn test_validate_merges_config_rules() {
    use envforge::ops::schema::{parse_schema_content, validate_against_schema};

    // Schema has PORT, config.toml has EXTRA
    let schema = parse_schema_content("[PORT]\ntype = \"number\"\n").unwrap();
    let mut config_rules = HashMap::new();
    config_rules.insert("EXTRA".into(), "nonempty".into());

    let mut env = HashMap::new();
    env.insert("PORT".into(), "3000".into());
    env.insert("EXTRA".into(), "".into()); // empty, but rule says nonempty

    let errors = validate_against_schema(&env, &schema, None, &config_rules);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].key, "EXTRA");
}

// ─── Schema Generation Tests ────────────────────────────────

#[test]
fn test_schema_generate() {
    use envforge::ops::schema::generate_schema;

    let mut env = HashMap::new();
    env.insert("PORT".into(), "3000".into());
    env.insert("DEBUG".into(), "true".into());
    env.insert("API_URL".into(), "https://example.com".into());
    env.insert("API_KEY".into(), "sk-12345".into());

    let output = generate_schema(&env);
    assert!(output.contains("[API_KEY]"));
    assert!(output.contains("sensitive = true")); // KEY pattern
    assert!(output.contains("[PORT]"));
    assert!(output.contains("[DEBUG]"));
    assert!(output.contains("[API_URL]"));
}

// ─── Schema Docs Generation Tests ───────────────────────────

#[test]
fn test_schema_docs() {
    use envforge::ops::schema::{generate_docs, parse_schema_content};

    let content = r#"
[PORT]
type = "number"
required = true
default = "3000"
description = "HTTP server port"

[API_KEY]
type = "string"
required = true
sensitive = true
"#;

    let schema = parse_schema_content(content).unwrap();
    let docs = generate_docs(&schema);
    assert!(docs.contains("| PORT |"));
    assert!(docs.contains("| API_KEY [sensitive] |"));
    assert!(docs.contains("| Yes |")); // required
    assert!(docs.contains("HTTP server port"));
}

// ─── Drift Detection Tests ──────────────────────────────────

#[test]
fn test_drift_detect_same() {
    use envforge::ops::schema::{detect_drift, DriftStatus};

    let env1: HashMap<String, String> = [("A".into(), "1".into())].into();
    let env2: HashMap<String, String> = [("A".into(), "1".into())].into();

    let drift = detect_drift(&[("env1".into(), env1), ("env2".into(), env2)], None);
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].status, DriftStatus::Same);
}

#[test]
fn test_drift_detect_differs() {
    use envforge::ops::schema::{detect_drift, DriftStatus};

    let env1: HashMap<String, String> = [("A".into(), "1".into())].into();
    let env2: HashMap<String, String> = [("A".into(), "2".into())].into();

    let drift = detect_drift(&[("env1".into(), env1), ("env2".into(), env2)], None);
    assert_eq!(drift[0].status, DriftStatus::Differs);
}

#[test]
fn test_drift_detect_missing() {
    use envforge::ops::schema::{detect_drift, DriftStatus};

    let env1: HashMap<String, String> = [("A".into(), "1".into()), ("B".into(), "2".into())].into();
    let env2: HashMap<String, String> = [("A".into(), "1".into())].into();

    let drift = detect_drift(&[("env1".into(), env1), ("env2".into(), env2)], None);
    let b_entry = drift.iter().find(|d| d.key == "B").unwrap();
    assert_eq!(b_entry.status, DriftStatus::Missing);
}

// ─── Safe Export Tests ──────────────────────────────────────

#[test]
fn test_is_sensitive_key() {
    use envforge::ops::dotenv::is_sensitive_key;

    assert!(is_sensitive_key("API_KEY"));
    assert!(is_sensitive_key("SECRET_VALUE"));
    assert!(is_sensitive_key("AUTH_TOKEN"));
    assert!(is_sensitive_key("DB_PASSWORD"));
    assert!(is_sensitive_key("aws_credential"));
    assert!(!is_sensitive_key("PORT"));
    assert!(!is_sensitive_key("DEBUG"));
    assert!(!is_sensitive_key("DATABASE_URL"));
    assert!(!is_sensitive_key("KEYBOARD_LAYOUT")); // contains "key" but also "keyboard"
}

#[test]
fn test_safe_export_redacts_sensitive() {
    use envforge::model::{ExportStyle, QuoteStyle};
    use envforge::ops::dotenv::export_safe;
    use envforge::ops::EnvEntry;

    let entries = vec![
        EnvEntry {
            key: "PORT".into(),
            value: "3000".into(),
            source_file: std::path::PathBuf::from("test"),
            line_number: 1,
            line_index: 0,
            location: envforge::ops::EntryLocation::InFile,
            export_style: ExportStyle::Bare,
            quote_style: QuoteStyle::None,
            is_dirty: false,
        },
        EnvEntry {
            key: "API_KEY".into(),
            value: "sk-secret-123".into(),
            source_file: std::path::PathBuf::from("test"),
            line_number: 2,
            line_index: 0,
            location: envforge::ops::EntryLocation::InFile,
            export_style: ExportStyle::Bare,
            quote_style: QuoteStyle::None,
            is_dirty: false,
        },
    ];

    let schema_sensitive = HashSet::new();
    let output = export_safe(&entries, &schema_sensitive);
    assert!(output.contains("PORT=3000"));
    assert!(output.contains("API_KEY=[REDACTED]"));
    assert!(!output.contains("sk-secret-123"));
}

#[test]
fn test_safe_export_respects_schema_sensitive() {
    use envforge::model::{ExportStyle, QuoteStyle};
    use envforge::ops::dotenv::export_safe;
    use envforge::ops::EnvEntry;

    let entries = vec![EnvEntry {
        key: "CUSTOM_FIELD".into(),
        value: "should-be-hidden".into(),
        source_file: std::path::PathBuf::from("test"),
        line_number: 1,
        line_index: 0,
        location: envforge::ops::EntryLocation::InFile,
        export_style: ExportStyle::Bare,
        quote_style: QuoteStyle::None,
        is_dirty: false,
    }];

    let mut schema_sensitive = HashSet::new();
    schema_sensitive.insert("CUSTOM_FIELD".into());
    let output = export_safe(&entries, &schema_sensitive);
    assert!(output.contains("CUSTOM_FIELD=[REDACTED]"));
}

#[test]
fn test_safe_export_skips_commented() {
    use envforge::model::{ExportStyle, QuoteStyle};
    use envforge::ops::dotenv::export_safe;
    use envforge::ops::EnvEntry;

    let entries = vec![EnvEntry {
        key: "DELETED".into(),
        value: "old-value".into(),
        source_file: std::path::PathBuf::from("test"),
        line_number: 1,
        line_index: 0,
        location: envforge::ops::EntryLocation::Commented,
        export_style: ExportStyle::Bare,
        quote_style: QuoteStyle::None,
        is_dirty: false,
    }];

    let output = export_safe(&entries, &HashSet::new());
    assert!(!output.contains("DELETED"));
}

// ─── Env Example Generation Tests ───────────────────────────

#[test]
fn test_env_example_generation() {
    use envforge::ops::dotenv::export_env_example;
    use envforge::ops::schema::parse_schema_content;

    let content = r#"
[PORT]
type = "port"
required = true
default = "3000"
description = "HTTP server port"

[API_KEY]
type = "string"
sensitive = true

[NODE_ENV]
type = "enum"
values = ["dev", "staging", "prod"]
"#;

    let schema = parse_schema_content(content).unwrap();
    let output = export_env_example(&schema);

    assert!(output.contains("PORT=3000")); // uses default
    assert!(output.contains("API_KEY=<your-api-key>")); // sensitive placeholder
    assert!(output.contains("NODE_ENV=dev")); // first enum value
    assert!(output.contains("# HTTP server port")); // description as comment
}

// ─── Doctor Tests ───────────────────────────────────────────

#[test]
fn test_doctor_runs_without_panic() {
    use envforge::ops::doctor::run_doctor;
    let report = run_doctor();
    assert!(report.checks.len() >= 9); // at least 9 checks (10 with AI safety)
    assert_eq!(
        report.ok_count() + report.warning_count() + report.error_count(),
        report.checks.len()
    );
}

#[test]
fn test_doctor_check_status_counts() {
    use envforge::ops::doctor::{CheckStatus, HealthCheck, HealthReport};

    let report = HealthReport {
        checks: vec![
            HealthCheck {
                name: "a".into(),
                status: CheckStatus::Ok,
                message: "ok".into(),
                details: vec![],
                hint: None,
            },
            HealthCheck {
                name: "b".into(),
                status: CheckStatus::Warning,
                message: "warn".into(),
                details: vec![],
                hint: Some("fix it".into()),
            },
            HealthCheck {
                name: "c".into(),
                status: CheckStatus::Error,
                message: "err".into(),
                details: vec![],
                hint: Some("fix it now".into()),
            },
        ],
    };

    assert_eq!(report.ok_count(), 1);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(report.error_count(), 1);
}

// ─── Run Config Tests ───────────────────────────────────────

#[test]
fn test_run_collect_env_includes_shell_vars() {
    use envforge::ops::run::{collect_env, RunConfig};

    std::env::set_var("ENVFORGE_TEST_VAR", "test_value_12345");

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![],
        overrides: vec![],
        redact: false,
        no_project: false,
    };

    let env = collect_env(&config).unwrap();
    assert_eq!(
        env.get("ENVFORGE_TEST_VAR").map(|s| s.as_str()),
        Some("test_value_12345")
    );

    std::env::remove_var("ENVFORGE_TEST_VAR");
}

#[test]
fn test_run_overrides_take_priority() {
    use envforge::ops::run::{collect_env, RunConfig};

    std::env::set_var("ENVFORGE_OVERRIDE_TEST", "original");

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![],
        overrides: vec![("ENVFORGE_OVERRIDE_TEST".into(), "overridden".into())],
        redact: false,
        no_project: false,
    };

    let env = collect_env(&config).unwrap();
    assert_eq!(
        env.get("ENVFORGE_OVERRIDE_TEST").map(|s| s.as_str()),
        Some("overridden")
    );

    std::env::remove_var("ENVFORGE_OVERRIDE_TEST");
}

#[test]
fn test_run_invalid_profile_error() {
    use envforge::ops::run::{collect_env, RunConfig};

    let config = RunConfig {
        profile: Some("nonexistent_profile_xyz".into()),
        profiles: vec![],
        resolve: false,
        env_files: vec![],
        overrides: vec![],
        redact: false,
        no_project: false,
    };

    let result = collect_env(&config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[test]
fn test_run_missing_env_file_error() {
    use envforge::ops::run::{collect_env, RunConfig};

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![std::path::PathBuf::from("/tmp/nonexistent_file_xyz.env")],
        overrides: vec![],
        redact: false,
        no_project: false,
    };

    let result = collect_env(&config);
    assert!(result.is_err());
}

#[test]
fn test_spawn_command_not_found() {
    use envforge::ops::run::spawn_process;

    let result = spawn_process("nonexistent_command_xyz_123", &[], &HashMap::new());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[test]
fn test_spawn_exit_code_passthrough() {
    use envforge::ops::run::spawn_process;

    let result = spawn_process("true", &[], &HashMap::new()).unwrap();
    assert_eq!(result.exit_code, 0);

    let result = spawn_process("false", &[], &HashMap::new()).unwrap();
    assert_eq!(result.exit_code, 1);
}

#[test]
fn test_spawn_with_env() {
    use envforge::ops::run::spawn_process;

    let mut env = HashMap::new();
    env.insert("PATH".into(), std::env::var("PATH").unwrap_or_default());
    env.insert("TEST_INJECTED".into(), "hello_from_envforge".into());

    let result = spawn_process("printenv", &["TEST_INJECTED".into()], &env).unwrap();
    assert_eq!(result.exit_code, 0);
}

// ─── Redaction Tests ───────────────────────────────────────

#[test]
fn test_redact_secrets_basic() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("API_KEY".to_string(), "sk-abc123".to_string())];
    let text = "Connecting with key sk-abc123 to server";
    let result = redact_secrets(text, &secrets);
    assert_eq!(result, "Connecting with key [REDACTED:API_KEY] to server");
}

#[test]
fn test_redact_secrets_skips_short_values() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("API_KEY".to_string(), "abc".to_string())];
    let text = "Value is abc here";
    let result = redact_secrets(text, &secrets);
    // Short values (< 4 chars) should NOT be redacted
    assert_eq!(result, "Value is abc here");
}

#[test]
fn test_redact_secrets_skips_non_sensitive_keys() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("PORT".to_string(), "3000".to_string())];
    let text = "Running on port 3000";
    let result = redact_secrets(text, &secrets);
    // PORT is not a sensitive key, so value should not be redacted
    assert_eq!(result, "Running on port 3000");
}

#[test]
fn test_redact_secrets_multiple() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![
        ("API_KEY".to_string(), "sk-abc123".to_string()),
        ("DB_PASSWORD".to_string(), "super-secret".to_string()),
    ];
    let text = "key=sk-abc123 pass=super-secret done";
    let result = redact_secrets(text, &secrets);
    assert!(result.contains("[REDACTED:API_KEY]"));
    assert!(result.contains("[REDACTED:DB_PASSWORD]"));
    assert!(!result.contains("sk-abc123"));
    assert!(!result.contains("super-secret"));
}

#[test]
fn test_redact_secrets_no_match() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("API_KEY".to_string(), "sk-abc123".to_string())];
    let text = "No secrets here at all";
    let result = redact_secrets(text, &secrets);
    assert_eq!(result, "No secrets here at all");
}

#[test]
fn test_redact_secrets_empty_text() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("API_KEY".to_string(), "sk-abc123".to_string())];
    let result = redact_secrets("", &secrets);
    assert_eq!(result, "");
}

#[test]
fn test_redact_secrets_empty_secrets_list() {
    use envforge::ops::run::redact_secrets;

    let result = redact_secrets("some text with secrets", &[]);
    assert_eq!(result, "some text with secrets");
}

#[test]
fn test_redact_secrets_value_appears_twice() {
    use envforge::ops::run::redact_secrets;

    let secrets = vec![("API_KEY".to_string(), "sk-abc123".to_string())];
    let text = "first sk-abc123 then sk-abc123 again";
    let result = redact_secrets(text, &secrets);
    assert!(!result.contains("sk-abc123"));
    assert_eq!(result.matches("[REDACTED:API_KEY]").count(), 2);
}

#[test]
fn test_run_config_no_project_flag() {
    use envforge::ops::run::{collect_env, RunConfig};

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![],
        overrides: vec![],
        redact: false,
        no_project: true,
    };

    // Should succeed even if project config exists somewhere
    let env = collect_env(&config).unwrap();
    assert!(!env.is_empty()); // At least has PATH etc.
}

#[test]
fn test_run_multiple_overrides_last_wins() {
    use envforge::ops::run::{collect_env, RunConfig};

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![],
        overrides: vec![
            ("ENVFORGE_MULTI_TEST".into(), "first".into()),
            ("ENVFORGE_MULTI_TEST".into(), "second".into()),
        ],
        redact: false,
        no_project: true,
    };

    let env = collect_env(&config).unwrap();
    assert_eq!(
        env.get("ENVFORGE_MULTI_TEST").map(|s| s.as_str()),
        Some("second")
    );
}

#[test]
fn test_run_env_file_loading() {
    use envforge::ops::run::{collect_env, RunConfig};

    let temp = std::env::temp_dir().join("envforge-test-run-envfile");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let env_file = temp.join(".env.test");
    std::fs::write(&env_file, "ENVFORGE_RUN_FILE_TEST=loaded\n").unwrap();

    let config = RunConfig {
        profile: None,
        profiles: vec![],
        resolve: false,
        env_files: vec![env_file],
        overrides: vec![],
        redact: false,
        no_project: true,
    };

    let env = collect_env(&config).unwrap();
    assert_eq!(
        env.get("ENVFORGE_RUN_FILE_TEST").map(|s| s.as_str()),
        Some("loaded")
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_spawn_process_echo() {
    use envforge::ops::run::spawn_process;

    let mut env = HashMap::new();
    env.insert("PATH".into(), std::env::var("PATH").unwrap_or_default());

    let result = spawn_process("echo", &["hello".into()], &env).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_run_error_display_variants() {
    use envforge::ops::run::RunError;

    let err = RunError::Config("bad config".into());
    assert!(err.to_string().contains("bad config"));

    let err = RunError::ProfileNotFound("staging".into(), "dev, prod".into());
    let msg = err.to_string();
    assert!(msg.contains("staging"));
    assert!(msg.contains("dev, prod"));

    let err = RunError::EnvFileNotFound("/tmp/missing.env".into());
    assert!(err.to_string().contains("missing.env"));

    let err = RunError::DecryptFailed {
        key: "SECRET".into(),
        message: "bad key".into(),
    };
    assert!(err.to_string().contains("SECRET"));

    let err = RunError::ResolveFailed {
        key: "REF_KEY".into(),
        message: "provider down".into(),
    };
    assert!(err.to_string().contains("REF_KEY"));

    let err = RunError::CommandNotFound("nonexistent".into());
    assert!(err.to_string().contains("nonexistent"));

    let err = RunError::SpawnFailed("permission denied".into());
    assert!(err.to_string().contains("permission denied"));
}

// ─── OpError Tests ─────────────────────────────────────────

#[test]
fn test_op_error_from_string() {
    use envforge::ops::OpError;

    let err: OpError = "something went wrong".to_string().into();
    assert!(err.to_string().contains("something went wrong"));
}

#[test]
fn test_op_error_from_str() {
    use envforge::ops::OpError;

    let err: OpError = "static error".into();
    assert!(err.to_string().contains("static error"));
}

#[test]
fn test_op_error_from_io_error() {
    use envforge::ops::OpError;

    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err: OpError = io_err.into();
    assert!(err.to_string().contains("file missing"));
}

#[test]
fn test_op_error_from_json_error() {
    use envforge::ops::OpError;

    let json_err = serde_json::from_str::<serde_json::Value>("{bad").unwrap_err();
    let err: OpError = json_err.into();
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_op_error_debug_format() {
    use envforge::ops::OpError;

    let err = OpError::Other("debug test".into());
    let debug = format!("{:?}", err);
    assert!(debug.contains("debug test"));
}

// ─── ProjectError Tests ────────────────────────────────────

#[test]
fn test_project_error_io_error() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::IoError {
        path: "/test/path".into(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/test/path"));
}

#[test]
fn test_project_error_config_not_found() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::ConfigNotFound;
    let msg = err.to_string();
    assert!(msg.contains("project init"));
}

#[test]
fn test_project_error_already_initialized() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::AlreadyInitialized {
        path: "/project/.envforge.project.toml".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("already initialized"));
    assert!(msg.contains("--force"));
}

#[test]
fn test_project_error_invalid_env_name() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::InvalidEnvironmentName {
        name: "BAD_NAME".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("BAD_NAME"));
    assert!(msg.contains("lowercase"));
}

#[test]
fn test_project_error_env_not_found_lists_available() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::EnvironmentNotFound {
        name: "staging".into(),
        available: "dev, prod".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("staging"));
    assert!(msg.contains("dev, prod"));
}

#[test]
fn test_project_error_env_exists() {
    use envforge::ops::project::ProjectError;

    let err = ProjectError::EnvironmentExists { name: "dev".into() };
    assert!(err.to_string().contains("dev"));
    assert!(err.to_string().contains("already exists"));
}
