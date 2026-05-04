use std::collections::HashMap;
use envforge::ops::{validate_value, validate_entries, EnvEntry, EntryLocation};
use envforge::model::{ExportStyle, QuoteStyle};
use std::path::PathBuf;

#[test]
fn test_validate_value_nonempty() {
    assert!(validate_value("hello", "nonempty").is_none());
    assert!(validate_value("", "nonempty").is_some());
    assert!(validate_value("  ", "nonempty").is_some());
}

#[test]
fn test_validate_value_number() {
    assert!(validate_value("123", "number").is_none());
    assert!(validate_value("123.45", "number").is_none());
    assert!(validate_value("-123", "number").is_none());
    assert!(validate_value("abc", "number").is_some());
}

#[test]
fn test_validate_value_bool() {
    for val in &["true", "false", "1", "0", "yes", "no", "TRUE", "False"] {
        assert!(validate_value(val, "bool").is_none(), "Failed for {}", val);
    }
    assert!(validate_value("maybe", "bool").is_some());
}

#[test]
fn test_validate_value_url() {
    assert!(validate_value("https://google.com", "url").is_none());
    assert!(validate_value("postgres://user@localhost", "url").is_none());
    assert!(validate_value("s3://bucket/key", "url").is_none());
    assert!(validate_value("ftp://mysite.com", "url").is_some());
    assert!(validate_value("http://", "url").is_some()); // too short/incomplete
    assert!(validate_value("not-a-url", "url").is_some());
}

#[test]
fn test_validate_value_email() {
    assert!(validate_value("test@example.com", "email").is_none());
    assert!(validate_value("test.name+extra@sub.domain.co.uk", "email").is_none());
    assert!(validate_value("test.example.com", "email").is_some());
    assert!(validate_value("test@", "email").is_some());
    assert!(validate_value("@example.com", "email").is_some());
}

#[test]
fn test_validate_value_regex() {
    assert!(validate_value("v1.2.3", "regex:^v\\d+\\.\\d+\\.\\d+$").is_none());
    assert!(validate_value("1.2.3", "regex:^v\\d+\\.\\d+\\.\\d+$").is_some());
}

#[test]
fn test_validate_value_unknown_rule() {
    // Unknown rules should pass (for forward compatibility/graceful degradation)
    assert!(validate_value("anything", "unknown_rule").is_none());
}

#[test]
fn test_validate_entries() {
    let entries = vec![
        EnvEntry {
            key: "PORT".to_string(),
            value: "8080".to_string(),
            source_file: PathBuf::from(".env"),
            line_number: 1,
            line_index: 0,
            location: EntryLocation::InFile,
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            is_dirty: false,
        },
        EnvEntry {
            key: "DEBUG".to_string(),
            value: "not-a-bool".to_string(),
            source_file: PathBuf::from(".env"),
            line_number: 2,
            line_index: 0,
            location: EntryLocation::InFile,
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            is_dirty: false,
        },
    ];
    
    let mut rules = HashMap::new();
    rules.insert("PORT".to_string(), "number".to_string());
    rules.insert("DEBUG".to_string(), "bool".to_string());
    
    let errors = validate_entries(&entries, &rules);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].key, "DEBUG");
    assert!(errors[0].message.contains("Expected bool"));
}
