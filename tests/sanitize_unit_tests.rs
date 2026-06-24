//! Edge-case coverage for `ops::sanitize`: UTF-8-safe slicing, the
//! `value_looks_like_secret` heuristic, diff-aware assignment redaction, and
//! `sanitize_content`/`sanitize_file` boundaries.

use envforge::ops::sanitize::{
    char_prefix, char_suffix, redact_sensitive_assignments, sanitize_content, sanitize_file,
    value_looks_like_secret,
};

// ---- char_prefix / char_suffix (UTF-8 boundary safety) ---------------------

#[test]
fn test_char_prefix_zero_and_overflow() {
    assert_eq!(char_prefix("abc", 0), "");
    assert_eq!(char_prefix("abc", 10), "abc"); // n > len returns whole string
}

#[test]
fn test_char_prefix_multibyte_no_panic() {
    // "héllo": é is 2 bytes; prefix of 2 chars must not split the codepoint.
    assert_eq!(char_prefix("héllo", 2), "hé");
    assert_eq!(char_prefix("🔑key", 1), "🔑");
}

#[test]
fn test_char_suffix_multibyte_and_overflow() {
    assert_eq!(char_suffix("héllo", 2), "lo");
    assert_eq!(char_suffix("abc", 5), "abc"); // n >= len returns whole string
                                              // Documents current behavior: n == 0 falls through to the None arm and
                                              // returns the whole string rather than "".
    assert_eq!(char_suffix("abc", 0), "abc");
}

// ---- value_looks_like_secret ----------------------------------------------

#[test]
fn test_value_looks_like_secret_true_cases() {
    assert!(value_looks_like_secret("this_is_a_very_long_value_xyz")); // > 16 chars
    assert!(value_looks_like_secret("sk-abc123")); // known prefix
    assert!(value_looks_like_secret("https://u:p@host/db")); // contains ://
    assert!(value_looks_like_secret("x7Kp9mQ2nL4w")); // 12-char high-entropy alnum
}

#[test]
fn test_value_looks_like_secret_false_cases() {
    assert!(!value_looks_like_secret("")); // empty is never a secret
    assert!(!value_looks_like_secret("production")); // single-class word, short
    assert!(!value_looks_like_secret("us-east-1")); // hyphenated identifier
    assert!(!value_looks_like_secret("v1.2.3")); // dotted version
}

// ---- redact_sensitive_assignments (diff-aware) -----------------------------

#[test]
fn test_redact_assignment_sensitive_key_value_masked() {
    let out = redact_sensitive_assignments("export API_KEY=supersecret\n");
    assert_eq!(out, "export API_KEY=[REDACTED]\n");
}

#[test]
fn test_redact_assignment_non_sensitive_key_untouched() {
    assert_eq!(redact_sensitive_assignments("PORT=8080"), "PORT=8080");
}

#[test]
fn test_redact_assignment_preserves_diff_markers() {
    let diff = "+export DB_PASSWORD=hunter2\n-export DB_PASSWORD=old\n";
    let out = redact_sensitive_assignments(diff);
    assert_eq!(
        out,
        "+export DB_PASSWORD=[REDACTED]\n-export DB_PASSWORD=[REDACTED]\n"
    );
}

#[test]
fn test_redact_assignment_leaves_file_headers() {
    // `+++`/`---` headers must not be treated as assignments.
    let header = "+++ b/.env\n--- a/.env\n";
    assert_eq!(redact_sensitive_assignments(header), header);
}

#[test]
fn test_redact_assignment_empty_input() {
    assert_eq!(redact_sensitive_assignments(""), "");
}

// ---- sanitize_content / sanitize_file --------------------------------------

#[test]
fn test_sanitize_content_empty_inputs() {
    assert_eq!(sanitize_content("", &[]), (String::new(), 0));
    let secrets = vec![("K".to_string(), "v".to_string())];
    assert_eq!(sanitize_content("", &secrets), (String::new(), 0));
}

#[test]
fn test_sanitize_file_missing_input_errors() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does_not_exist.env");
    let res = sanitize_file(&missing, None, &[]);
    assert!(res.is_err(), "reading a missing input file must error");
}
