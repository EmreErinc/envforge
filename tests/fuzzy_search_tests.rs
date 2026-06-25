//! Coverage for `ops::fuzzy::fuzzy_search`: empty-query passthrough, key vs
//! value matching, no-match emptiness, and score ordering.

use envforge::ops::{collect_entries, fuzzy_search};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn entries(content: &str) -> Vec<envforge::ops::EnvEntry> {
    collect_entries(&parse_shell_content(content, Path::new("/test/.zshrc")).unwrap())
}

#[test]
fn test_fuzzy_empty_query_returns_all_zero_score() {
    let e = entries("export A=1\nexport B=2\n");
    let res = fuzzy_search(&e, "");
    assert_eq!(res.len(), e.len());
    assert!(res.iter().all(|m| m.score == 0));
}

#[test]
fn test_fuzzy_matches_key() {
    let e = entries("export DATABASE_URL=x\nexport PORT=1\n");
    let res = fuzzy_search(&e, "DATA");
    assert!(res.iter().any(|m| m.entry.key == "DATABASE_URL"));
}

#[test]
fn test_fuzzy_matches_value_without_key_highlights() {
    let e = entries("export X=postgres\n");
    let res = fuzzy_search(&e, "postgres");
    let hit = res.iter().find(|m| m.entry.key == "X").unwrap();
    // Matched on value, so there are no key-character highlight indices.
    assert!(hit.matched_indices.is_empty());
}

#[test]
fn test_fuzzy_no_match_is_empty() {
    let e = entries("export A=1\nexport B=2\n");
    assert!(fuzzy_search(&e, "ZZZQQQ").is_empty());
}

#[test]
fn test_fuzzy_results_sorted_by_score_desc() {
    let e = entries("export DB_HOST=h\nexport DB_HOST_BACKUP=h2\nexport UNRELATED=u\n");
    let res = fuzzy_search(&e, "dbhost");
    assert!(res.windows(2).all(|w| w[0].score >= w[1].score));
}
