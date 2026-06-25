//! Coverage for `ops::hardening` adversarial-input layers that deobfuscate tool
//! input before the secret scanner runs: control-char strip / homoglyph
//! normalization, base64 decode, split-string assembly, and the pipeline.

use envforge::ops::hardening::{
    detect_split_strings, find_and_decode_base64, strip_control_chars, HardenInput, HardenSource,
    HardeningConfig,
};

// ---- strip_control_chars ---------------------------------------------------

#[test]
fn test_strip_control_chars_removes_invisible_and_normalizes() {
    assert_eq!(strip_control_chars("a\u{200b}b"), "ab"); // zero-width space
    assert_eq!(strip_control_chars("x\0y"), "xy"); // null byte
    assert_eq!(strip_control_chars("a\u{202e}b"), "ab"); // BIDI override
                                                         // Cyrillic homoglyphs fold to ASCII so "аpi" (Cyrillic а) reads as "api".
    assert_eq!(strip_control_chars("\u{0430}pi_key"), "api_key");
}

#[test]
fn test_strip_control_chars_noop_on_clean_ascii() {
    assert_eq!(strip_control_chars("clean text"), "clean text");
}

// ---- find_and_decode_base64 ------------------------------------------------

#[test]
fn test_base64_decodes_embedded_payload() {
    // base64("AAAAAAAAAAAAAAAAAAAA") = 20 'A' bytes.
    let input = "prefix QUFBQUFBQUFBQUFBQUFBQUFBQUE= suffix";
    let decoded = find_and_decode_base64(input, 20);
    assert!(decoded.iter().any(|d| d == "AAAAAAAAAAAAAAAAAAAA"));
}

#[test]
fn test_base64_skips_below_min_length() {
    // "YWJj" decodes to "abc" but is shorter than min_length → ignored.
    assert!(find_and_decode_base64("YWJj", 20).is_empty());
}

// ---- detect_split_strings --------------------------------------------------

#[test]
fn test_detect_split_strings_concat() {
    let assembled = detect_split_strings("x = \"foo\" + \"bar\" + \"baz\"");
    assert!(assembled.iter().any(|s| s == "foobarbaz"));
}

#[test]
fn test_detect_split_strings_template_and_join() {
    assert!(detect_split_strings("value = ${API_KEY}")
        .iter()
        .any(|s| s == "${API_KEY}"));
    assert!(detect_split_strings("['a','b','c','d'].join('')")
        .iter()
        .any(|s| s == "abcd"));
}

// ---- HardenInput pipeline + config -----------------------------------------

#[test]
fn test_harden_pipeline_flags_control_chars() {
    let h = HardenInput::new(HardeningConfig::default());
    let results = h.harden("secret\u{200b}value");
    assert!(results
        .iter()
        .any(|r| r.source == HardenSource::ControlChars && r.text == "secretvalue"));
}

#[test]
fn test_hardening_config_defaults() {
    let c = HardeningConfig::default();
    assert!(c.control_chars);
    assert!(c.base64_decode);
    assert_eq!(c.base64_min_length, 20);
    assert!(!c.split_strings);
    assert!(c.encoding_chain);
    assert_eq!(c.encoding_chain_max_depth, 3);
}

#[test]
fn test_harden_source_display() {
    assert_eq!(HardenSource::ControlChars.to_string(), "control_char_strip");
    assert_eq!(HardenSource::Base64Decode.to_string(), "base64_decode");
    assert_eq!(HardenSource::SplitString.to_string(), "split_string");
    assert_eq!(HardenSource::EncodingChain.to_string(), "encoding_chain");
}
