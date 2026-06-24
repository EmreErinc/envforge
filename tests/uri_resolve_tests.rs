//! Coverage for `ops::uri_resolve` parsing surface: provider URI validation,
//! `is_secret_uri` discrimination, and key=value content parsing edge cases.

use envforge::ops::uri_resolve::{is_secret_uri, parse_secret_uri, parse_uri_content};

// ---- parse_secret_uri ------------------------------------------------------

#[test]
fn test_parse_secret_uri_valid_providers() {
    let u = parse_secret_uri("vault://secret/db/password").unwrap();
    assert_eq!(u.provider, "vault");
    assert_eq!(u.path, "secret/db/password");

    let p = parse_secret_uri("1password://vault/item").unwrap();
    assert_eq!(p.provider, "1password");
}

#[test]
fn test_parse_secret_uri_strips_leading_slashes() {
    let u = parse_secret_uri("vault:///secret/key").unwrap();
    assert_eq!(u.path, "secret/key");
}

#[test]
fn test_parse_secret_uri_rejects_invalid() {
    assert!(parse_secret_uri("plainvalue").is_none()); // no scheme separator
    assert!(parse_secret_uri("unknown://x").is_none()); // provider not allowlisted
    assert!(parse_secret_uri("vault://").is_none()); // empty path
    assert!(parse_secret_uri("VAULT://x").is_none()); // case-sensitive provider
    assert!(parse_secret_uri("vault ://x").is_none()); // whitespace in provider
    assert!(parse_secret_uri("验证://x").is_none()); // non-ascii provider
}

// ---- is_secret_uri ---------------------------------------------------------

#[test]
fn test_is_secret_uri_discriminates() {
    assert!(is_secret_uri("vault://secret/db"));
    assert!(!is_secret_uri("https://example.com")); // valid URL, invalid provider
    assert!(!is_secret_uri("plainvalue"));
}

// ---- parse_uri_content -----------------------------------------------------

#[test]
fn test_parse_uri_content_basic_and_quotes() {
    let entries = parse_uri_content("A=1\nB=\"two\"\nC='three'\n");
    assert_eq!(
        entries,
        vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "two".to_string()),
            ("C".to_string(), "three".to_string()),
        ]
    );
}

#[test]
fn test_parse_uri_content_skips_comments_and_blanks() {
    let entries = parse_uri_content("# comment\n\n   \nKEY=val\n");
    assert_eq!(entries, vec![("KEY".to_string(), "val".to_string())]);
}

#[test]
fn test_parse_uri_content_edge_cases() {
    // First '=' splits; later '=' stay in the value.
    let multi = parse_uri_content("URL=a=b=c");
    assert_eq!(multi, vec![("URL".to_string(), "a=b=c".to_string())]);

    // Unbalanced quote is left intact (not stripped).
    let unclosed = parse_uri_content("K=\"unclosed");
    assert_eq!(unclosed, vec![("K".to_string(), "\"unclosed".to_string())]);

    // Empty key (line starts with '=') is dropped; line without '=' is dropped.
    assert!(parse_uri_content("=value").is_empty());
    assert!(parse_uri_content("noequals").is_empty());

    // Internal spaces in key are preserved (only ends are trimmed).
    let spaced = parse_uri_content("KEY WITH SPACE=val");
    assert_eq!(
        spaced,
        vec![("KEY WITH SPACE".to_string(), "val".to_string())]
    );
}
