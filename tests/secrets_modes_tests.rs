//! Coverage for `ops::secrets::modes` pure helpers: the `glob_match` wildcard
//! matcher, `VolatileMode` accessors, and `zeroize_secrets`.

use envforge::ops::secrets::modes::{glob_match, zeroize_secrets, VolatileMode};

// ---- glob_match ------------------------------------------------------------

#[test]
fn test_glob_match_exact_and_star() {
    assert!(glob_match("abc", "abc"));
    assert!(!glob_match("abc", "abd"));
    assert!(glob_match("*", "anything"));
    assert!(glob_match("*", ""));
}

#[test]
fn test_glob_match_prefix_suffix_contains() {
    assert!(glob_match("DB_*", "DB_HOST"));
    assert!(!glob_match("DB_*", "REDIS_HOST"));
    assert!(glob_match("*_KEY", "API_KEY"));
    assert!(!glob_match("*_KEY", "API_VALUE"));
    assert!(glob_match("*abc*", "xx_abc_yy"));
    assert!(glob_match("a*b*c", "axxbyyc"));
}

#[test]
fn test_glob_match_question_mark_and_empty() {
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("a?c", "ac")); // ? requires exactly one char
    assert!(glob_match("", ""));
    assert!(!glob_match("", "x"));
}

// ---- VolatileMode ----------------------------------------------------------

#[test]
fn test_volatile_mode_default_is_on_300() {
    let m = VolatileMode::default();
    assert!(m.is_enabled());
    assert_eq!(m.ttl_seconds(), 300);
    assert!(!m.requires_reauth());
}

#[test]
fn test_volatile_mode_off() {
    let m = VolatileMode::Off;
    assert!(!m.is_enabled());
    assert_eq!(m.ttl_seconds(), 0);
    assert!(!m.requires_reauth());
}

#[test]
fn test_volatile_mode_on_and_strict() {
    let on = VolatileMode::On { ttl_seconds: 60 };
    assert!(on.is_enabled());
    assert_eq!(on.ttl_seconds(), 60);
    assert!(!on.requires_reauth());

    let strict = VolatileMode::Strict {
        ttl_seconds: 120,
        reauth: true,
    };
    assert!(strict.is_enabled());
    assert_eq!(strict.ttl_seconds(), 120);
    assert!(strict.requires_reauth());

    let strict_no_reauth = VolatileMode::Strict {
        ttl_seconds: 90,
        reauth: false,
    };
    assert!(!strict_no_reauth.requires_reauth());
}

// ---- zeroize_secrets -------------------------------------------------------

#[test]
fn test_zeroize_secrets_clears_vec() {
    let mut secrets = vec![
        ("API_KEY".to_string(), "sk-secret".to_string()),
        ("DB_PASS".to_string(), "hunter2".to_string()),
    ];
    zeroize_secrets(&mut secrets);
    assert!(secrets.is_empty());
}
