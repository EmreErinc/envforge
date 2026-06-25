//! Coverage for `ops::scanner`: sensitive-entry filtering and on-disk secret
//! value detection (the pre-commit / repo scan that catches leaked secrets).

use envforge::model::{ExportStyle, QuoteStyle};
use envforge::ops::scanner::{filter_sensitive, scan_directory};
use envforge::ops::{EntryLocation, EnvEntry};
use std::path::PathBuf;

fn entry(key: &str, value: &str) -> EnvEntry {
    EnvEntry {
        key: key.to_string(),
        value: value.to_string(),
        source_file: PathBuf::from(".env"),
        line_number: 1,
        line_index: 0,
        location: EntryLocation::InFile,
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        is_dirty: false,
    }
}

// ---- filter_sensitive ------------------------------------------------------

#[test]
fn test_filter_sensitive_keeps_secret_keys_with_real_values() {
    let entries = vec![
        entry("API_KEY", "sk-supersecret"),
        entry("DB_PASSWORD", "hunter2pass"),
        entry("PORT", "8080"),              // not sensitive
        entry("KEYBOARD_LAYOUT", "dvorak"), // key but keyboard-excluded
    ];
    let kept = filter_sensitive(&entries);
    let keys: Vec<&str> = kept.iter().map(|e| e.key.as_str()).collect();
    assert!(keys.contains(&"API_KEY"));
    assert!(keys.contains(&"DB_PASSWORD"));
    assert!(!keys.contains(&"PORT"));
    assert!(!keys.contains(&"KEYBOARD_LAYOUT"));
}

#[test]
fn test_filter_sensitive_drops_short_and_empty_values() {
    let entries = vec![
        entry("SECRET_A", "abc"),  // < 4 chars → skipped
        entry("SECRET_B", ""),     // empty → skipped
        entry("SECRET_C", "abcd"), // 4 chars → kept
    ];
    let kept = filter_sensitive(&entries);
    let keys: Vec<&str> = kept.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, vec!["SECRET_C"]);
}

// ---- scan_directory --------------------------------------------------------

#[test]
fn test_scan_directory_finds_leaked_secret_value() {
    let dir = tempfile::tempdir().unwrap();
    let leaked = dir.path().join("config.yaml");
    std::fs::write(
        &leaked,
        "db:\n  url: postgres://u:supersecretvalue123@h/db\n",
    )
    .unwrap();

    let sensitive = vec![entry("DB_PASSWORD", "supersecretvalue123")];
    let matches = scan_directory(dir.path(), &sensitive).unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].matched_key, "DB_PASSWORD");
    // The reported match value is masked — the scanner never echoes the raw secret.
    assert!(!matches[0].matched_value.contains("supersecretvalue123"));
}

#[test]
fn test_scan_directory_no_match_when_value_absent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("clean.txt"), "nothing secret here\n").unwrap();
    let sensitive = vec![entry("API_KEY", "sk-never-appears-xyz")];
    assert!(scan_directory(dir.path(), &sensitive).unwrap().is_empty());
}
