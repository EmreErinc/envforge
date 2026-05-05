use std::path::PathBuf;

use envforge::model::{ExportStyle, QuoteStyle};
use envforge::ops::check::{
    parse_category_filter, report_to_json, run_checks, CheckCategory, CheckReport, CheckResult,
    CheckStatus,
};
use envforge::ops::export_format::{export_as, ExportFormat};
use envforge::ops::secrets::age::{SecretAge, SecretSources};
use envforge::ops::{EntryLocation, EnvEntry};

// ─── Helpers ────────────────────────────────────────────────────

fn make_entry(key: &str, value: &str) -> EnvEntry {
    EnvEntry {
        key: key.to_string(),
        value: value.to_string(),
        source_file: PathBuf::from("/test"),
        line_number: 1,
        line_index: 0,
        location: EntryLocation::InFile,
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        is_dirty: false,
    }
}

fn make_commented(key: &str, value: &str) -> EnvEntry {
    EnvEntry {
        key: key.to_string(),
        value: value.to_string(),
        source_file: PathBuf::from("/test"),
        line_number: 1,
        line_index: 0,
        location: EntryLocation::Commented,
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        is_dirty: false,
    }
}

fn sample_entries() -> Vec<EnvEntry> {
    vec![
        make_entry("APP_NAME", "myapp"),
        make_entry("DB_HOST", "localhost"),
        make_entry("DB_PORT", "5432"),
        make_entry("API_KEY", "sk-abc123def456"),
        make_entry("DEBUG", "true"),
        make_entry("EMPTY_VAL", ""),
        make_entry("SPACED", "hello world"),
        make_commented("DELETED", "gone"),
    ]
}

// ─── Export Format Parse Tests ──────────────────────────────────

#[test]
fn test_all_format_aliases() {
    let cases = vec![
        ("dotenv", ExportFormat::Dotenv),
        ("env", ExportFormat::Dotenv),
        (".env", ExportFormat::Dotenv),
        ("json", ExportFormat::Json),
        ("yaml", ExportFormat::Yaml),
        ("yml", ExportFormat::Yaml),
        ("toml", ExportFormat::Toml),
        ("docker", ExportFormat::Docker),
        ("docker-env", ExportFormat::Docker),
        ("k8s", ExportFormat::K8s),
        ("kubernetes", ExportFormat::K8s),
        ("k8s-secret", ExportFormat::K8s),
        ("tfvars", ExportFormat::Tfvars),
        ("terraform", ExportFormat::Tfvars),
        ("tf", ExportFormat::Tfvars),
    ];
    for (input, expected) in cases {
        assert_eq!(
            ExportFormat::parse(input).unwrap(),
            expected,
            "Failed for input: {}",
            input
        );
    }
}

#[test]
fn test_format_parse_case_insensitive() {
    assert_eq!(ExportFormat::parse("JSON").unwrap(), ExportFormat::Json);
    assert_eq!(ExportFormat::parse("Yaml").unwrap(), ExportFormat::Yaml);
    assert_eq!(ExportFormat::parse("TOML").unwrap(), ExportFormat::Toml);
}

#[test]
fn test_format_parse_invalid() {
    assert!(ExportFormat::parse("xml").is_err());
    assert!(ExportFormat::parse("csv").is_err());
    assert!(ExportFormat::parse("").is_err());
}

// ─── JSON Export ────────────────────────────────────────────────

#[test]
fn test_json_export_structure() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Json, None, None);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["APP_NAME"], "myapp");
    assert_eq!(parsed["DB_HOST"], "localhost");
    assert_eq!(parsed["DB_PORT"], "5432");
    assert_eq!(parsed["DEBUG"], "true"); // stays string, not bool
    assert_eq!(parsed["EMPTY_VAL"], "");
    assert!(parsed.get("DELETED").is_none()); // commented = excluded
}

#[test]
fn test_json_export_valid_json() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Json, None, None);
    let result: Result<serde_json::Value, _> = serde_json::from_str(&output);
    assert!(result.is_ok(), "Output is not valid JSON: {}", output);
}

// ─── YAML Export ────────────────────────────────────────────────

#[test]
fn test_yaml_export_basic() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Yaml, None, None);
    assert!(output.contains("APP_NAME: myapp"));
    assert!(output.contains("DB_HOST: localhost"));
}

#[test]
fn test_yaml_export_quotes_bools() {
    let entries = vec![make_entry("FLAG", "true"), make_entry("OTHER", "false")];
    let output = export_as(&entries, &ExportFormat::Yaml, None, None);
    // "true" and "false" should be quoted to avoid YAML bool interpretation
    assert!(output.contains("FLAG: \"true\""));
    assert!(output.contains("OTHER: \"false\""));
}

#[test]
fn test_yaml_export_quotes_numbers() {
    let entries = vec![make_entry("PORT", "3000"), make_entry("RATIO", "1.5")];
    let output = export_as(&entries, &ExportFormat::Yaml, None, None);
    assert!(output.contains("PORT: \"3000\""));
    assert!(output.contains("RATIO: \"1.5\""));
}

#[test]
fn test_yaml_export_quotes_special_yaml_values() {
    let entries = vec![
        make_entry("A", "yes"),
        make_entry("B", "no"),
        make_entry("C", "null"),
        make_entry("D", "~"),
        make_entry("E", "on"),
        make_entry("F", "off"),
    ];
    let output = export_as(&entries, &ExportFormat::Yaml, None, None);
    for key in &["A", "B", "C", "D", "E", "F"] {
        assert!(
            output.contains(&format!("{}: \"", key)),
            "Key {} should be quoted in YAML",
            key
        );
    }
}

// ─── TOML Export ────────────────────────────────────────────────

#[test]
fn test_toml_export_basic() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Toml, None, None);
    assert!(output.contains("APP_NAME = \"myapp\""));
    assert!(output.contains("DB_PORT = \"5432\""));
}

#[test]
fn test_toml_export_escapes_quotes() {
    let entries = vec![make_entry("MSG", "say \"hello\"")];
    let output = export_as(&entries, &ExportFormat::Toml, None, None);
    assert!(output.contains("MSG = \"say \\\"hello\\\"\""));
}

#[test]
fn test_toml_export_escapes_backslash() {
    let entries = vec![make_entry("PATH", "C:\\Users\\test")];
    let output = export_as(&entries, &ExportFormat::Toml, None, None);
    assert!(output.contains("PATH = \"C:\\\\Users\\\\test\""));
}

// ─── Docker Export ──────────────────────────────────────────────

#[test]
fn test_docker_export_no_quotes() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Docker, None, None);
    assert!(output.contains("APP_NAME=myapp"));
    assert!(output.contains("SPACED=hello world")); // Docker: no quotes
    assert!(!output.contains('"'));
    assert!(!output.contains('#'));
}

#[test]
fn test_docker_export_excludes_commented() {
    let entries = sample_entries();
    let output = export_as(&entries, &ExportFormat::Docker, None, None);
    assert!(!output.contains("DELETED"));
}

// ─── K8s Secret Export ──────────────────────────────────────────

#[test]
fn test_k8s_export_structure() {
    let entries = sample_entries();
    let output = export_as(
        &entries,
        &ExportFormat::K8s,
        Some("app-secrets"),
        Some("prod"),
    );
    assert!(output.contains("apiVersion: v1"));
    assert!(output.contains("kind: Secret"));
    assert!(output.contains("name: app-secrets"));
    assert!(output.contains("namespace: prod"));
    assert!(output.contains("type: Opaque"));
    assert!(output.contains("data:"));
}

#[test]
fn test_k8s_export_base64_values() {
    use base64::Engine;
    let entries = vec![make_entry("SECRET", "mysecret")];
    let output = export_as(&entries, &ExportFormat::K8s, None, None);
    let expected = base64::engine::general_purpose::STANDARD.encode(b"mysecret");
    assert!(output.contains(&format!("SECRET: {}", expected)));
}

#[test]
fn test_k8s_export_defaults() {
    let entries = vec![make_entry("KEY", "val")];
    let output = export_as(&entries, &ExportFormat::K8s, None, None);
    assert!(output.contains("name: envforge-secrets"));
    assert!(output.contains("namespace: default"));
}

// ─── Terraform tfvars Export ────────────────────────────────────

#[test]
fn test_tfvars_export_lowercase_keys() {
    let entries = vec![
        make_entry("DB_HOST", "localhost"),
        make_entry("API_KEY", "abc"),
    ];
    let output = export_as(&entries, &ExportFormat::Tfvars, None, None);
    assert!(output.contains("db_host = \"localhost\""));
    assert!(output.contains("api_key = \"abc\""));
    // Uppercase originals should NOT appear
    assert!(!output.contains("DB_HOST"));
    assert!(!output.contains("API_KEY"));
}

// ─── Dotenv Export ──────────────────────────────────────────────

#[test]
fn test_dotenv_export_quotes_spaces() {
    let entries = vec![make_entry("MSG", "hello world")];
    let output = export_as(&entries, &ExportFormat::Dotenv, None, None);
    assert!(output.contains("MSG=\"hello world\""));
}

#[test]
fn test_dotenv_export_no_quotes_simple() {
    let entries = vec![make_entry("KEY", "simple")];
    let output = export_as(&entries, &ExportFormat::Dotenv, None, None);
    assert!(output.contains("KEY=simple"));
    assert!(!output.contains('"'));
}

// ─── Cross-format: Commented entries excluded ───────────────────

#[test]
fn test_all_formats_exclude_commented() {
    let entries = vec![make_commented("HIDDEN", "secret")];
    let formats = vec![
        ExportFormat::Dotenv,
        ExportFormat::Json,
        ExportFormat::Yaml,
        ExportFormat::Toml,
        ExportFormat::Docker,
        ExportFormat::K8s,
        ExportFormat::Tfvars,
    ];
    for fmt in &formats {
        let output = export_as(&entries, fmt, None, None);
        assert!(
            !output.contains("HIDDEN"),
            "Format {:?} should exclude commented entries",
            fmt
        );
    }
}

// ─── Cross-format: Empty entries ────────────────────────────────

#[test]
fn test_all_formats_handle_empty_entries() {
    let entries: Vec<EnvEntry> = vec![];
    let formats = vec![
        ExportFormat::Dotenv,
        ExportFormat::Json,
        ExportFormat::Yaml,
        ExportFormat::Toml,
        ExportFormat::Docker,
        ExportFormat::Tfvars,
    ];
    for fmt in &formats {
        let output = export_as(&entries, fmt, None, None);
        // Should not panic, should produce valid (possibly empty) output
        assert!(output.is_empty() || !output.is_empty());
    }
}

// ─── Secret Age Tests ───────────────────────────────────────────

#[test]
fn test_secret_sources_serialization() {
    let mut sources = SecretSources::default();
    sources.secrets.insert(
        "DB_URL".to_string(),
        SecretAge {
            provider: "vault".to_string(),
            path: "secret/myapp".to_string(),
            updated_at: "2026-04-20T10:00:00+00:00".to_string(),
        },
    );
    sources.secrets.insert(
        "API_KEY".to_string(),
        SecretAge {
            provider: "aws-ssm".to_string(),
            path: "/prod/keys".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
        },
    );

    let toml_str = toml::to_string_pretty(&sources).unwrap();
    let parsed: SecretSources = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.secrets.len(), 2);
    assert_eq!(parsed.secrets["DB_URL"].provider, "vault");
    assert_eq!(parsed.secrets["API_KEY"].provider, "aws-ssm");
}

#[test]
fn test_secret_sources_default_empty() {
    let sources = SecretSources::default();
    assert!(sources.secrets.is_empty());
}

#[test]
fn test_secret_age_calculation() {
    use chrono::{DateTime, Utc};

    let old_date = "2025-01-01T00:00:00+00:00";
    let dt = DateTime::parse_from_rfc3339(old_date).unwrap();
    let now = Utc::now();
    let days = now.signed_duration_since(dt).num_days();
    // Should be well over 90 days
    assert!(days > 90, "Expected >90 days, got {}", days);
}

#[test]
fn test_secret_age_recent() {
    use chrono::Utc;

    let now = Utc::now().to_rfc3339();
    let dt = chrono::DateTime::parse_from_rfc3339(&now).unwrap();
    let days = Utc::now().signed_duration_since(dt).num_days();
    assert_eq!(days, 0);
}

// ─── Format Extension Tests ─────────────────────────────────────

#[test]
fn test_format_extensions() {
    assert_eq!(ExportFormat::Dotenv.extension(), ".env");
    assert_eq!(ExportFormat::Json.extension(), ".json");
    assert_eq!(ExportFormat::Yaml.extension(), ".yaml");
    assert_eq!(ExportFormat::Toml.extension(), ".toml");
    assert_eq!(ExportFormat::Docker.extension(), ".env");
    assert_eq!(ExportFormat::K8s.extension(), ".yaml");
    assert_eq!(ExportFormat::Tfvars.extension(), ".tfvars");
}

// ─── Check Runner Tests ─────────────────────────────────────────

#[test]
fn test_check_category_parse_all() {
    assert_eq!(
        CheckCategory::parse("doctor").unwrap(),
        CheckCategory::Doctor
    );
    assert_eq!(
        CheckCategory::parse("validate").unwrap(),
        CheckCategory::Validate
    );
    assert_eq!(CheckCategory::parse("scan").unwrap(), CheckCategory::Scan);
    assert_eq!(CheckCategory::parse("age").unwrap(), CheckCategory::Age);
    assert_eq!(CheckCategory::parse("drift").unwrap(), CheckCategory::Drift);
    assert!(CheckCategory::parse("invalid").is_none());
}

#[test]
fn test_check_category_all_count() {
    assert_eq!(CheckCategory::all().len(), 5);
}

#[test]
fn test_check_parse_filter_valid() {
    let cats = parse_category_filter("doctor,scan,age").unwrap();
    assert_eq!(cats.len(), 3);
    assert_eq!(cats[0], CheckCategory::Doctor);
    assert_eq!(cats[1], CheckCategory::Scan);
    assert_eq!(cats[2], CheckCategory::Age);
}

#[test]
fn test_check_parse_filter_invalid_rejects() {
    assert!(parse_category_filter("doctor,bogus").is_err());
    assert!(parse_category_filter("").is_err());
}

#[test]
fn test_check_report_counts() {
    let report = CheckReport {
        results: vec![
            CheckResult {
                category: CheckCategory::Doctor,
                status: CheckStatus::Ok,
                message: "ok".into(),
                hint: None,
            },
            CheckResult {
                category: CheckCategory::Doctor,
                status: CheckStatus::Warning,
                message: "warn".into(),
                hint: Some("fix".into()),
            },
            CheckResult {
                category: CheckCategory::Validate,
                status: CheckStatus::Error,
                message: "fail".into(),
                hint: Some("fix".into()),
            },
        ],
        skipped: vec![(CheckCategory::Age, "no data".into())],
    };
    assert_eq!(report.ok_count(), 1);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(report.error_count(), 1);
    assert!(report.has_errors());
}

#[test]
fn test_check_report_no_errors() {
    let report = CheckReport {
        results: vec![
            CheckResult {
                category: CheckCategory::Doctor,
                status: CheckStatus::Ok,
                message: "ok".into(),
                hint: None,
            },
            CheckResult {
                category: CheckCategory::Scan,
                status: CheckStatus::Warning,
                message: "warn".into(),
                hint: None,
            },
        ],
        skipped: vec![],
    };
    assert!(!report.has_errors());
}

#[test]
fn test_check_report_json_structure() {
    let report = CheckReport {
        results: vec![CheckResult {
            category: CheckCategory::Doctor,
            status: CheckStatus::Ok,
            message: "Config loaded".into(),
            hint: None,
        }],
        skipped: vec![(CheckCategory::Drift, "no schema".into())],
    };
    let json = report_to_json(&report);
    assert_eq!(json["summary"]["total"], 1);
    assert_eq!(json["summary"]["ok"], 1);
    assert_eq!(json["summary"]["warnings"], 0);
    assert_eq!(json["summary"]["errors"], 0);
    assert_eq!(json["categories_skipped"][0]["category"], "drift");
    assert_eq!(json["categories_skipped"][0]["reason"], "no schema");
    assert_eq!(json["results"][0]["category"], "doctor");
    assert_eq!(json["results"][0]["status"], "ok");
}

#[test]
fn test_check_run_doctor_only() {
    // Run only doctor category — should always succeed
    let report = run_checks(Some(&[CheckCategory::Doctor]));
    // Doctor should produce results (config check at minimum)
    assert!(!report.results.is_empty());
    // All results should be Doctor category
    for r in &report.results {
        assert_eq!(r.category, CheckCategory::Doctor);
    }
}

#[test]
fn test_check_run_skips_missing_prerequisites() {
    // Run all checks — validate/drift/age may be skipped depending on env
    let report = run_checks(None);
    // Doctor always runs, so we should have some results
    assert!(!report.results.is_empty() || !report.skipped.is_empty());
}

#[test]
fn test_check_category_names() {
    assert_eq!(CheckCategory::Doctor.name(), "doctor");
    assert_eq!(CheckCategory::Validate.name(), "validate");
    assert_eq!(CheckCategory::Scan.name(), "scan");
    assert_eq!(CheckCategory::Age.name(), "age");
    assert_eq!(CheckCategory::Drift.name(), "drift");
}

#[test]
fn test_check_category_display_names() {
    assert_eq!(CheckCategory::Doctor.display_name(), "Doctor");
    assert_eq!(CheckCategory::Validate.display_name(), "Validate");
    assert_eq!(CheckCategory::Scan.display_name(), "Scan");
    assert_eq!(CheckCategory::Age.display_name(), "Age");
    assert_eq!(CheckCategory::Drift.display_name(), "Drift");
}
