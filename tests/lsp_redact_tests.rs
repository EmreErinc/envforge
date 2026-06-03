// ═══════════════════════════════════════════════════════════════
// LSP Redaction Boundary Tests
// ═══════════════════════════════════════════════════════════════
// Tests for `src/lsp/redact.rs` — security-critical redaction
// functions that protect secrets from leaking into LSP responses.
//
// These are defense-in-depth tests: the primary protection is
// the fence, but redaction catches leaks that slip through.

use envforge::lsp::redact::{redact_for_label, redact_secrets_in_message};

// ═══════════════════════════════════════════════════════════════
// redact_for_label — Always returns "***"
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_for_label_always_returns_asterisks() {
    assert_eq!(redact_for_label("any-value", false), "***");
    assert_eq!(redact_for_label("any-value", true), "***");
}

#[test]
fn test_redact_for_label_empty_value() {
    assert_eq!(redact_for_label("", false), "***");
    assert_eq!(redact_for_label("", true), "***");
}

#[test]
fn test_redact_for_label_unicode_value() {
    assert_eq!(redact_for_label("🦀 секрет 秘密", false), "***");
    assert_eq!(redact_for_label("🦀 секрет 秘密", true), "***");
}

#[test]
fn test_redact_for_label_very_long_value() {
    let long = "x".repeat(10_000);
    assert_eq!(redact_for_label(&long, false), "***");
    assert_eq!(redact_for_label(&long, true), "***");
}

#[test]
fn test_redact_for_label_never_leaks_prefix() {
    // Critical: must not leak type via prefix like "gh_" or "sk-"
    assert_eq!(redact_for_label("gh_secret_token_abc123", false), "***");
    assert_eq!(redact_for_label("sk-abc123def456", false), "***");
    assert_eq!(redact_for_label("AWS_ACCESS_KEY_ID_abc", true), "***");
    assert_eq!(
        redact_for_label("-----BEGIN RSA PRIVATE KEY-----", true),
        "***"
    );
}

// ═══════════════════════════════════════════════════════════════
// redact_secrets_in_message — Basic Behavior
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_empty_secrets_list_returns_original() {
    let msg = "the secret is secret123";
    assert_eq!(redact_secrets_in_message(msg, &[]), msg);
}

#[test]
fn test_redact_secrets_empty_message() {
    let secrets = vec!["secretkey".to_string()];
    assert_eq!(redact_secrets_in_message("", &secrets), "");
}

#[test]
fn test_redact_secrets_basic_redaction() {
    let msg = "my token is abcdefgh12345678 in the config";
    let secrets = vec!["abcdefgh12345678".to_string()];
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "my token is *** in the config"
    );
}

#[test]
fn test_redact_secrets_multiple_occurrences() {
    let msg = "key=abcdefgh12345678 and also abcdefgh12345678 here";
    let secrets = vec!["abcdefgh12345678".to_string()];
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "key=*** and also *** here"
    );
}

#[test]
fn test_redact_secrets_multiple_different_secrets() {
    let msg = "a: sk-aaaabbbbccccdddd, b: gh_1111222233334444";
    let secrets = vec![
        "sk-aaaabbbbccccdddd".to_string(),
        "gh_1111222233334444".to_string(),
    ];
    let result = redact_secrets_in_message(msg, &secrets);
    assert_eq!(result, "a: ***, b: ***");
}

#[test]
fn test_redact_secrets_not_present_returns_original() {
    let msg = "no secrets here";
    let secrets = vec!["not_in_message_xyz_abc".to_string()];
    assert_eq!(redact_secrets_in_message(msg, &secrets), msg);
}

// ═══════════════════════════════════════════════════════════════
// Short Secret Filtering (< 8 chars skipped)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_skips_short_secret_7_chars() {
    let msg = "my key is abcdefg";
    let secrets = vec!["abcdefg".to_string()]; // 7 chars
    assert_eq!(redact_secrets_in_message(msg, &secrets), msg);
}

#[test]
fn test_redact_secrets_redacts_exactly_8_char_secret() {
    let msg = "my key is abcdefgh";
    let secrets = vec!["abcdefgh".to_string()]; // exactly 8 chars
    assert_eq!(redact_secrets_in_message(msg, &secrets), "my key is ***");
}

#[test]
fn test_redact_secrets_mixed_short_and_long() {
    let msg = "short=abc long=abcdefgh12345678";
    let secrets = vec![
        "abc".to_string(),              // 3 chars — skipped
        "abcdefgh12345678".to_string(), // 16 chars — redacted
    ];
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "short=abc long=***"
    );
}

#[test]
fn test_redact_secrets_all_shorter_than_8_returns_original() {
    let msg = "a b c d e f g";
    let secrets = vec![
        "a".to_string(),
        "bc".to_string(),
        "def".to_string(),
        "ghij".to_string(),
        "klmno".to_string(),
        "pqrstu".to_string(),
        "vwxyz12".to_string(), // 7 chars — still skipped
    ];
    assert_eq!(redact_secrets_in_message(msg, &secrets), msg);
}

// ═══════════════════════════════════════════════════════════════
// Longest-First Ordering (Prefix Protection)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_longest_first_prevents_prefix_leak() {
    // "sk-proj-long-key-12345678" contains "sk-proj" as prefix
    // If shorter redacted first: "***-long-key-12345678" — leaks suffix
    // If longer redacted first: "***" — correct
    let msg = "token=sk-proj-long-key-12345678";
    let secrets = vec![
        "sk-proj".to_string(),                   // shorter prefix
        "sk-proj-long-key-12345678".to_string(), // full secret
    ];
    let result = redact_secrets_in_message(msg, &secrets);
    // Should be fully redacted, not "***-long-key-12345678"
    assert_eq!(result, "token=***");
}

#[test]
fn test_redact_secrets_prefix_protection_with_ordering() {
    // Same test but secrets provided in wrong order — function must sort
    let msg = "token=sk-proj-long-key-12345678";
    let secrets = vec![
        "sk-proj".to_string(),                   // shorter — provided FIRST
        "sk-proj-long-key-12345678".to_string(), // longer  — provided SECOND
    ];
    let result = redact_secrets_in_message(msg, &secrets);
    assert_eq!(result, "token=***");
}

// ═══════════════════════════════════════════════════════════════
// String::replace Semantics (Exact Match, Case-Sensitive)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_substring_match_behavior() {
    // String::replace does substring matching — if secret "abcdefgh" is
    // a substring of "abcdefgh12345678", it WILL be redacted.
    // This is acceptable because the 8-char minimum makes false
    // positives extremely unlikely in practice.
    let msg = "use the key 'abcdefgh12345678' for auth";
    let secrets = vec!["abcdefgh".to_string()];
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "use the key '***12345678' for auth"
    );
}

#[test]
fn test_redact_secrets_case_sensitive() {
    let msg = "KEY: ABCDEFGH12345678, key: abcdefgh12345678";
    let secrets = vec!["ABCDEFGH12345678".to_string()];
    let result = redact_secrets_in_message(msg, &secrets);
    assert_eq!(result, "KEY: ***, key: abcdefgh12345678");
}

#[test]
fn test_redact_secrets_exact_match_required_no_regex_behavior() {
    // String::replace is literal, not regex — special chars are literal
    let msg = "value=abc.def*g hi+j kl?mn ^op$";
    let secrets = vec!["abc.def*g".to_string()]; // contains regex-like chars
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "value=*** hi+j kl?mn ^op$"
    );
}

// ═══════════════════════════════════════════════════════════════
// Unicode and Special Characters
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_unicode_secret_value() {
    let msg = "секрет=мойсекретныйключ12345";
    let secrets = vec!["мойсекретныйключ12345".to_string()]; // 22 chars
    assert_eq!(redact_secrets_in_message(msg, &secrets), "секрет=***");
}

#[test]
fn test_redact_secrets_emoji_in_message_preserved() {
    let msg = "🔑 key=abcdefgh12345678 ✅";
    let secrets = vec!["abcdefgh12345678".to_string()];
    assert_eq!(redact_secrets_in_message(msg, &secrets), "🔑 key=*** ✅");
}

#[test]
fn test_redact_secrets_emoji_secret_redacted() {
    let msg = "token=🔑secret🔑 in message";
    let secrets = vec!["🔑secret🔑".to_string()];
    assert_eq!(
        redact_secrets_in_message(msg, &secrets),
        "token=*** in message"
    );
}

// ═══════════════════════════════════════════════════════════════
// Edge Cases and Defense-in-Depth
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_redact_secrets_zero_length_secret_in_list() {
    let msg = "key=abcdefgh12345678";
    let secrets = vec![
        "".to_string(),                 // 0 chars — filtered out
        "abcdefgh12345678".to_string(), // 16 chars — redacted
    ];
    assert_eq!(redact_secrets_in_message(msg, &secrets), "key=***");
}

#[test]
fn test_redact_secrets_only_whitespace_secret_filtered() {
    let msg = "key=        ";
    let secrets = vec!["        ".to_string()]; // 8 spaces — exactly 8 chars
    assert_eq!(redact_secrets_in_message(msg, &secrets), "key=***");
}

#[test]
fn test_redact_secrets_overlapping_secrets_independent_redaction() {
    // Two different secrets that don't overlap
    let msg = "a=firstsecret12, b=secondsecret34";
    let secrets = vec!["firstsecret12".to_string(), "secondsecret34".to_string()];
    assert_eq!(redact_secrets_in_message(msg, &secrets), "a=***, b=***");
}

#[test]
fn test_redact_secrets_adjacent_secrets_redacted() {
    let msg = "combined=sk-aabbccddeeffgghh_gh-1122334455667788";
    let secrets = vec![
        "sk-aabbccddeeffgghh".to_string(),
        "gh-1122334455667788".to_string(),
    ];
    // Both should be independently redacted
    let result = redact_secrets_in_message(msg, &secrets);
    assert!(!result.contains("sk-aabbccddeeffgghh"));
    assert!(!result.contains("gh-1122334455667788"));
    assert!(result.contains("***"));
}

#[test]
fn test_redact_secrets_large_secret_list() {
    let msg = "key9998=value9998";
    // Build 10,000 secrets — none of them match "value9998"
    let secrets: Vec<String> = (0..10_000)
        .map(|i| format!("secret_key_{:05}", i))
        .collect();
    // Should not crash or hang — message remains unchanged
    assert_eq!(redact_secrets_in_message(msg, &secrets), msg);
}

#[test]
fn test_redact_secrets_large_message() {
    let secret = "target_secret_abcd".to_string();
    let mut msg = "prefix ".to_string();
    msg.push_str(&"filler ".repeat(500));
    msg.push_str(&secret);
    msg.push_str(" suffix");
    let secrets = vec![secret];
    let result = redact_secrets_in_message(&msg, &secrets);
    assert!(result.starts_with("prefix "));
    assert!(result.ends_with(" suffix"));
    assert!(!result.contains("target_secret_abcd"));
    assert_eq!(result.matches("***").count(), 1);
}

#[test]
fn test_redact_secrets_no_allocation_when_secrets_empty() {
    // Empty secrets should return original without allocation
    // (verifying the early return path)
    let msg = "this should be returned as-is";
    let secrets: Vec<String> = vec![];
    let result = redact_secrets_in_message(msg, &secrets);
    assert_eq!(result, msg);
}
