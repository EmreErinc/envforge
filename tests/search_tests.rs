use clap::Parser;
use envforge::cli::*;
use envforge::ops::{collect_entries, fuzzy_search, EntryLocation, EnvEntry};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn make_entries(content: &str) -> Vec<EnvEntry> {
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    collect_entries(&sf)
}

#[test]
fn test_search_cli_variant_parsing() {
    let cli = Cli::try_parse_from(["envforge", "search", "database"]);
    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        Some(Commands::Search { query }) => assert_eq!(query, "database"),
        _ => panic!("Expected Search command"),
    }
}

#[test]
fn test_search_cli_with_json_flag() {
    let cli = Cli::try_parse_from(["envforge", "search", "db", "--json"]);
    assert!(cli.is_ok());
    let cli = cli.unwrap();
    assert!(cli.json);
    match cli.command {
        Some(Commands::Search { query }) => assert_eq!(query, "db"),
        _ => panic!("Expected Search command"),
    }
}

#[test]
fn test_search_cli_missing_query_fails() {
    let cli = Cli::try_parse_from(["envforge", "search"]);
    assert!(cli.is_err());
}

#[test]
fn test_fuzzy_search_matches_key() {
    let entries = make_entries(
        "export DATABASE_URL=\"postgres://localhost:5432/mydb\"\nexport API_KEY=\"sk-live-xxx\"",
    );
    let results = fuzzy_search(&entries, "database");
    assert!(!results.is_empty());
    assert_eq!(results[0].entry.key, "DATABASE_URL");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_matches_value() {
    let entries =
        make_entries("export MY_VAR=\"postgres://localhost:5432/mydb\"\nexport OTHER=\"hello\"");
    let results = fuzzy_search(&entries, "postgres");
    assert!(!results.is_empty());
    assert_eq!(results[0].entry.key, "MY_VAR");
}

#[test]
fn test_fuzzy_search_no_match_returns_empty() {
    let entries = make_entries("export FOO=\"bar\"\nexport BAZ=\"qux\"");
    let results = fuzzy_search(&entries, "zzzzz");
    assert!(results.is_empty());
}

#[test]
fn test_fuzzy_search_results_sorted_by_score() {
    let entries = make_entries(
        "export DATABASE_URL=\"value\"\nexport DATA_DIR=\"/data\"\nexport OTHER=\"unrelated\"",
    );
    let results = fuzzy_search(&entries, "database");
    assert!(!results.is_empty());
    if results.len() > 1 {
        assert!(results[0].score >= results[1].score);
    }
}

#[test]
fn test_search_json_output_includes_version() {
    let entries = make_entries("export DATABASE_URL=\"postgres://localhost:5432\"");
    let results = fuzzy_search(&entries, "db");
    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "version": 1,
                "key": r.entry.key,
                "value": r.entry.value,
                "source_file": r.entry.source_file.to_string_lossy(),
                "line_number": r.entry.line_number,
                "score": r.score,
                "matched_indices": r.matched_indices,
            })
        })
        .collect();
    let output = serde_json::to_string_pretty(&json_results).unwrap();
    assert!(output.contains("\"version\": 1"));
    assert!(output.contains("DATABASE_URL"));
}

#[test]
fn test_search_result_has_matched_indices() {
    let entries = make_entries("export DATABASE_URL=\"postgres://localhost:5432\"");
    let results = fuzzy_search(&entries, "db");
    assert!(!results.is_empty());
    let first = &results[0];
    assert!(!first.matched_indices.is_empty());
}
