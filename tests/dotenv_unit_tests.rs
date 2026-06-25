//! Coverage for `ops::dotenv` pure helpers: `parse_dotenv_content`,
//! `is_sensitive_key`, and `export_safe` redaction.

use envforge::model::{ExportStyle, QuoteStyle};
use envforge::ops::dotenv::{export_safe, is_sensitive_key, parse_dotenv_content};
use envforge::ops::{EntryLocation, EnvEntry};
use std::collections::HashSet;
use std::path::PathBuf;

// ---- parse_dotenv_content --------------------------------------------------

#[test]
fn test_parse_dotenv_content_basic_quotes_and_skips() {
    let content = "A=1\n# comment\n\nB=\"two\"\nC='three'\n=nokey\nURL=a=b=c\n";
    let entries = parse_dotenv_content(content);
    let map: Vec<(&str, &str)> = entries
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect();
    assert_eq!(
        map,
        vec![
            ("A", "1"),
            ("B", "two"),     // double quotes stripped
            ("C", "three"),   // single quotes stripped
            ("URL", "a=b=c"), // only first '=' splits
        ]
    );
}

// ---- is_sensitive_key ------------------------------------------------------

#[test]
fn test_is_sensitive_key_matches_keywords() {
    for k in [
        "API_KEY",
        "DB_PASSWORD",
        "AUTH_TOKEN",
        "MY_SECRET",
        "AWS_CREDENTIAL",
    ] {
        assert!(is_sensitive_key(k), "expected sensitive: {k}");
    }
}

#[test]
fn test_is_sensitive_key_non_sensitive_and_keyboard_exception() {
    assert!(!is_sensitive_key("PORT"));
    assert!(!is_sensitive_key("LOG_LEVEL"));
    // "keyboard" contains "key" but is explicitly excluded.
    assert!(!is_sensitive_key("KEYBOARD_LAYOUT"));
}

// ---- export_safe -----------------------------------------------------------

fn entry(key: &str, value: &str, location: EntryLocation) -> EnvEntry {
    EnvEntry {
        key: key.to_string(),
        value: value.to_string(),
        source_file: PathBuf::from(".env"),
        line_number: 1,
        line_index: 0,
        location,
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        is_dirty: false,
    }
}

#[test]
fn test_export_safe_redacts_sensitive_and_skips_commented() {
    let entries = vec![
        entry("API_KEY", "sk-secret", EntryLocation::InFile),
        entry("PORT", "8080", EntryLocation::InFile),
        entry("OLD", "x", EntryLocation::Commented),
    ];
    let out = export_safe(&entries, &HashSet::new());
    assert!(out.contains("API_KEY=[REDACTED]"));
    assert!(out.contains("PORT=8080"));
    assert!(!out.contains("sk-secret"));
    assert!(!out.contains("OLD="), "commented entries are skipped");
}

#[test]
fn test_export_safe_honors_schema_sensitive_set() {
    // REGION is not a keyword-sensitive key, but the schema marks it sensitive.
    let entries = vec![entry("REGION", "us-east-1", EntryLocation::InFile)];
    let mut schema_sensitive = HashSet::new();
    schema_sensitive.insert("REGION".to_string());
    let out = export_safe(&entries, &schema_sensitive);
    assert!(out.contains("REGION=[REDACTED]"));
    assert!(!out.contains("us-east-1"));
}
