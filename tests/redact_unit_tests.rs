//! Coverage for the shared `ops::redact` choke point: label preview is always
//! fully masked, and message redaction honors the 8-char floor and longest-first
//! ordering.

use envforge::ops::redact::{redact_for_label, redact_secrets_in_message};

#[test]
fn test_redact_for_label_always_masks() {
    assert_eq!(redact_for_label("anything", true), "***");
    assert_eq!(redact_for_label("", false), "***");
    assert_eq!(redact_for_label("🔑sk-proj-token", true), "***");
}

#[test]
fn test_redact_secrets_empty_list_returns_original() {
    assert_eq!(redact_secrets_in_message("hello world", &[]), "hello world");
}

#[test]
fn test_redact_secrets_masks_known_value() {
    let secrets = vec!["supersecretvalue".to_string()];
    let out = redact_secrets_in_message("token is supersecretvalue done", &secrets);
    assert_eq!(out, "token is *** done");
}

#[test]
fn test_redact_secrets_skips_short_secrets() {
    // 7 chars < 8-char floor: must NOT be redacted (too many false positives).
    let secrets = vec!["abc1234".to_string()];
    let msg = "value abc1234 here";
    assert_eq!(redact_secrets_in_message(msg, &secrets), msg);
}

#[test]
fn test_redact_secrets_eight_char_boundary_is_masked() {
    let secrets = vec!["abcd1234".to_string()]; // exactly 8 chars
    let out = redact_secrets_in_message("k=abcd1234", &secrets);
    assert_eq!(out, "k=***");
}

#[test]
fn test_redact_secrets_longest_first_no_residual() {
    let secrets = vec!["sk-proj".to_string(), "sk-proj-longkey".to_string()];
    let out = redact_secrets_in_message("key sk-proj-longkey end", &secrets);
    assert_eq!(out, "key *** end");
    assert!(!out.contains("sk-proj"));
}
