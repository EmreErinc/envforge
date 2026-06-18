//! Regression tests for the shared char-boundary-safe truncation primitives
//! and the secret-masking paths that consume them (M5 / M8 / FR9).
//!
//! Before this hardening, every `mask_value` copy and the TUI table truncation
//! byte-sliced UTF-8 (`&s[..n]`), which panics when byte `n` falls mid-codepoint
//! — a guaranteed crash (and, with no panic hook, a stranded secret on screen)
//! on any multibyte value. These tests pin the no-panic / correct-boundary
//! behavior of the canonical helpers and a public `mask_value` consumer.

use envforge::ops::sanitize::{char_prefix, char_suffix};

#[test]
fn test_char_prefix_ascii() {
    assert_eq!(char_prefix("abcdef", 3), "abc");
}

#[test]
fn test_char_prefix_n_exceeds_len_returns_whole() {
    assert_eq!(char_prefix("ab", 5), "ab");
    assert_eq!(char_prefix("", 3), "");
}

#[test]
fn test_char_suffix_ascii() {
    assert_eq!(char_suffix("abcdef", 3), "def");
}

#[test]
fn test_char_suffix_n_exceeds_len_returns_whole() {
    assert_eq!(char_suffix("ab", 5), "ab");
}

#[test]
fn test_char_prefix_multibyte_does_not_split_codepoint() {
    // "☕" is 3 bytes; a raw `&s[..2]` would panic mid-codepoint.
    let s = "☕abc☕";
    let p = char_prefix(s, 2);
    assert_eq!(p, "☕a");
    assert!(s.starts_with(p)); // valid UTF-8 boundary
}

#[test]
fn test_char_suffix_multibyte_does_not_split_codepoint() {
    let s = "☕abc☕";
    let p = char_suffix(s, 2);
    assert_eq!(p, "c☕");
    assert!(s.ends_with(p));
}

#[test]
fn test_char_helpers_no_panic_on_emoji_cjk_accents() {
    for s in ["🔑🔒🔥", "日本語のキー", "café-señor-naïve", "a̐éあ💥"] {
        for n in 0..10 {
            // Must never panic regardless of cut point.
            let _ = char_prefix(s, n);
            let _ = char_suffix(s, n);
        }
    }
}

#[test]
fn test_mask_value_no_panic_on_multibyte_boundary() {
    // rotate::mask_value previously did `&value[..2]` / `&value[len-2..]`.
    // "☕abc☕" has a multibyte char straddling byte 2 — the old code panicked.
    let masked = envforge::ops::rotate::mask_value("☕abc☕");
    assert!(masked.contains("****"));

    // mcp_scan::mask_value (already char-based) must also stay panic-free.
    let masked2 = envforge::ops::mcp_scan::mask_value("日本語トークンの値");
    assert!(!masked2.is_empty());
}
