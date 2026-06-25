//! Edge-case coverage for `ops::validation_utils` primitive validators.
//! Complements the small inline unit tests by exercising boundaries, unicode,
//! scheme handling, and malformed input.

use envforge::ops::validation_utils::{is_valid_bool, is_valid_email, is_valid_port, is_valid_url};

// ---- is_valid_email --------------------------------------------------------

#[test]
fn test_is_valid_email_plus_and_subdomain() {
    assert!(is_valid_email("user+tag@example.com"));
    assert!(is_valid_email("user@sub.example.co.uk"));
    assert!(is_valid_email("first.last%mid@a-b.io"));
}

#[test]
fn test_is_valid_email_rejects_empty_and_whitespace() {
    assert!(!is_valid_email(""));
    assert!(!is_valid_email("user name@example.com"));
    assert!(!is_valid_email(" user@example.com"));
}

#[test]
fn test_is_valid_email_rejects_double_at_and_unicode() {
    assert!(!is_valid_email("user@@example.com"));
    assert!(!is_valid_email("üser@example.com"));
    assert!(!is_valid_email("user@example.c")); // TLD must be >= 2 chars
}

// ---- is_valid_url ----------------------------------------------------------

#[test]
fn test_is_valid_url_supported_schemes() {
    assert!(is_valid_url("https://example.com"));
    assert!(is_valid_url("postgres://user:pass@localhost:5432/db"));
    assert!(is_valid_url("redis://localhost"));
    assert!(is_valid_url("s3://bucket/key"));
}

#[test]
fn test_is_valid_url_rejects_unsupported_and_empty() {
    assert!(!is_valid_url(""));
    assert!(!is_valid_url("ftp://site.com"));
    assert!(!is_valid_url("git+https://host/repo")); // not in scheme allowlist
}

#[test]
fn test_is_valid_url_scheme_is_case_sensitive_and_needs_host() {
    assert!(!is_valid_url("HTTPS://example.com")); // allowlist is lowercase
    assert!(!is_valid_url("https://")); // no host after scheme
}

// ---- is_valid_port ---------------------------------------------------------

#[test]
fn test_is_valid_port_boundaries() {
    assert!(is_valid_port("1"));
    assert!(is_valid_port("8080"));
    assert!(is_valid_port("65535"));
    assert!(!is_valid_port("0")); // must be > 0
    assert!(!is_valid_port("65536")); // overflows u16
}

#[test]
fn test_is_valid_port_rejects_malformed() {
    assert!(!is_valid_port(""));
    assert!(!is_valid_port("-1"));
    assert!(!is_valid_port("8080 "));
    assert!(!is_valid_port("8o80"));
    // Leading zeros are accepted by integer parsing (documents current behavior).
    assert!(is_valid_port("08080"));
}

// ---- is_valid_bool ---------------------------------------------------------

#[test]
fn test_is_valid_bool_accepts_variants_case_insensitive() {
    for v in &[
        "true", "false", "1", "0", "yes", "no", "on", "off", "TRUE", "Off", "YeS",
    ] {
        assert!(is_valid_bool(v), "expected valid: {v}");
    }
}

#[test]
fn test_is_valid_bool_rejects_unknown_and_padded() {
    for v in &["", "2", "t", "enabled", " true", "true "] {
        assert!(!is_valid_bool(v), "expected invalid: {v}");
    }
}
