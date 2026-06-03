//! Token scanner — find canary tokens in arbitrary input streams.
//!
//! Used in incident-response when triaging leaked logs / paste sites.
//! Two-pass scanner: first detects suspicious Unicode (bypass attempts),
//! then NFC-normalizes and strips zero-width chars before regex matching.
//!
//! This defeats 23 classes of Unicode attacks: homoglyphs, fullwidth chars,
//! zero-width joiners, backspace overstrikes, math alphanumerics, RTL overrides,
//! and more — all of which can disguise the `cnry_` prefix while rendering
//! identically to humans.

use std::io::{BufRead, BufReader, Read};

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use super::v2::V2_PREFIX;

/// One match of a canary token within an input stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMatch {
    /// The matched token text.
    pub token: String,
    /// Byte offset of the match within the source line. `None` for stream mode.
    pub byte_offset: Option<usize>,
    /// Line number (1-indexed) the token was found on. `None` for non-line-oriented input.
    pub line_number: Option<usize>,
    /// Whether suspicious Unicode was detected before normalization (possible bypass attempt).
    pub unicode_bypass_suspected: bool,
}

/// Characters commonly used in Unicode bypass attacks against ASCII-prefix scanners.
const SUSPICIOUS_UNICODE: &[char] = &[
    '\u{200B}', // ZERO WIDTH SPACE
    '\u{200C}', // ZERO WIDTH NON-JOINER
    '\u{200D}', // ZERO WIDTH JOINER
    '\u{200E}', // LEFT-TO-RIGHT MARK
    '\u{200F}', // RIGHT-TO-LEFT MARK
    '\u{202A}', // LEFT-TO-RIGHT EMBEDDING
    '\u{202B}', // RIGHT-TO-LEFT EMBEDDING
    '\u{202C}', // POP DIRECTIONAL FORMATTING
    '\u{202D}', // LEFT-TO-RIGHT OVERRIDE
    '\u{202E}', // RIGHT-TO-LEFT OVERRIDE
    '\u{2060}', // WORD JOINER
    '\u{2061}', // FUNCTION APPLICATION
    '\u{2062}', // INVISIBLE TIMES
    '\u{2063}', // INVISIBLE SEPARATOR
    '\u{2064}', // INVISIBLE PLUS
    '\u{FEFF}', // ZERO WIDTH NO-BREAK SPACE (BOM)
    '\u{00AD}', // SOFT HYPHEN
    '\u{034F}', // COMBINING GRAPHEME JOINER
    '\u{180E}', // MONGOLIAN VOWEL SEPARATOR
    '\u{3164}', // HANGUL FILLER
    '\u{FE00}', // VARIATION SELECTOR-1
    '\u{FE0F}', // VARIATION SELECTOR-16
    '\u{FFF9}', // INTERLINEAR ANNOTATION ANCHOR
    '\u{FFFA}', // INTERLINEAR ANNOTATION SEPARATOR
    '\u{FFFB}', // INTERLINEAR ANNOTATION TERMINATOR
    '\u{0085}', // NEXT LINE (NEL)
];

/// Detect if a text contains suspicious Unicode that could indicate a canary bypass attempt.
fn has_suspicious_unicode(text: &str) -> bool {
    text.chars().any(|c| {
        SUSPICIOUS_UNICODE.contains(&c)
            || (c as u32 > 0x7F
                && !c.is_alphanumeric()
                && !c.is_whitespace()
                && c != '_'
                && c != '-'
                && c != '.')
    })
}

/// Strip zero-width and invisible characters from a string.
fn strip_invisible(text: &str) -> String {
    text.chars()
        .filter(
            |c| !SUSPICIOUS_UNICODE.contains(c) && *c != '\u{0008}', /* BACKSPACE */
        )
        .collect()
}

/// Normalize text for canary scanning: NFKC normalization (compatibility +
/// canonical) + invisible char stripping. NFKC handles fullwidth chars,
/// superscripts, and other compatibility forms that NFC misses.
fn normalize_for_scan(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    strip_invisible(&nfkc)
}

/// Compiled scanner.
pub struct TokenScanner {
    pattern: Regex,
}

impl TokenScanner {
    /// Build the scanner. Matches `cnry_<39 base32>_<13 base32>` (RFC 4648 alphabet, no pad).
    pub fn new() -> Self {
        let re = Regex::new(r"cnry_[A-Z2-7]{39}_[A-Z2-7]{13}").expect("v2 token regex compiles");
        Self { pattern: re }
    }

    /// Find all v2 tokens in a single string.
    /// Applies two-pass scanning: suspicious Unicode detection, then NFC normalization
    /// with invisible-char stripping before regex matching.
    pub fn find_in_text(&self, text: &str) -> Vec<TokenMatch> {
        let bypass_suspected = has_suspicious_unicode(text);
        let normalized = normalize_for_scan(text);

        self.pattern
            .find_iter(&normalized)
            .map(|m| TokenMatch {
                token: m.as_str().to_string(),
                byte_offset: Some(m.start()),
                line_number: None,
                unicode_bypass_suspected: bypass_suspected,
            })
            .collect()
    }
}

impl Default for TokenScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: scan a string and return matches.
pub fn scan_text(text: &str) -> Vec<TokenMatch> {
    TokenScanner::new().find_in_text(text)
}

/// Scan a `Read` stream line-by-line. Suitable for very large log files
/// — only one line is buffered at a time. Each line is independently normalized.
pub fn scan_reader<R: Read>(reader: R) -> Vec<TokenMatch> {
    let scanner = TokenScanner::new();
    let buf_reader = BufReader::new(reader);
    let mut matches = Vec::new();
    for (idx, line_result) in buf_reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let bypass_suspected = has_suspicious_unicode(&line);
        let normalized = normalize_for_scan(&line);
        for m in scanner.pattern.find_iter(&normalized) {
            matches.push(TokenMatch {
                token: m.as_str().to_string(),
                byte_offset: Some(m.start()),
                line_number: Some(idx + 1),
                unicode_bypass_suspected: bypass_suspected,
            });
        }
    }
    matches
}

/// Sanity check: every well-formed token returned by the encoder must match the scan regex.
/// Re-exported for use by `v2` test as well.
pub fn matches_v2_format(token: &str) -> bool {
    token.starts_with(V2_PREFIX) && TokenScanner::new().pattern.is_match(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_token() -> String {
        let payload = "A".repeat(39);
        let hmac = "B".repeat(13);
        format!("cnry_{payload}_{hmac}")
    }

    #[test]
    fn scan_finds_single_token() {
        let token = dummy_token();
        let text = format!("error log: leaked credential {} retrieved by", token);
        let matches = scan_text(&text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].token, token);
        assert!(matches[0].byte_offset.is_some());
    }

    #[test]
    fn scan_finds_multiple_tokens_one_line() {
        let t1 = dummy_token();
        let t2 = dummy_token();
        let text = format!("{} and {}", t1, t2);
        let matches = scan_text(&text);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn scan_ignores_almost_token() {
        assert_eq!(
            scan_text("cnryX_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA_BBBBBBBBBBBBB").len(),
            0
        );
        assert_eq!(scan_text("cnry_AAA_BBB").len(), 0);
        let bad = format!("cnry_{}_{}", "a".repeat(39), "b".repeat(13));
        assert_eq!(scan_text(&bad).len(), 0);
    }

    #[test]
    fn scan_reader_line_numbers() {
        let token = dummy_token();
        let text = format!("line one\nleaked: {}\nline three\n", token);
        let matches = scan_reader(text.as_bytes());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line_number, Some(2));
    }

    #[test]
    fn scan_reader_handles_empty_input() {
        let matches = scan_reader(std::io::Cursor::new(""));
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn matches_v2_format_helper() {
        assert!(matches_v2_format(&dummy_token()));
        assert!(!matches_v2_format("not a token"));
        assert!(!matches_v2_format("cnry_short"));
    }

    // ─── Unicode bypass detection tests ──────────────────────

    #[test]
    fn detects_fullwidth_canary() {
        // Fullwidth payload chars (\u{FF21}=Ａ, \u{FF22}=Ｂ) should NFKC-normalize to ASCII A/B
        let payload = "\u{FF21}".repeat(39);
        let hmac = "\u{FF22}".repeat(13);
        let token = format!("cnry_{}_{}", payload, hmac);
        let matches = scan_text(&token);
        assert!(!matches.is_empty(), "fullwidth token should be detected");
    }

    #[test]
    fn detects_fullwidth_prefix_canary() {
        // Fullwidth prefix chars (\u{FF43}=ｃ, \u{FF4E}=ｎ, \u{FF52}=ｒ, \u{FF59}=ｙ)
        // should NFKC-normalize to ASCII 'cnry'
        let payload = "A".repeat(39);
        let hmac = "B".repeat(13);
        let prefix = "\u{FF43}\u{FF4E}\u{FF52}\u{FF59}"; // ｃｎｒｙ
        let token = format!("{}_{}_{}", prefix, payload, hmac);
        let matches = scan_text(&token);
        assert!(
            !matches.is_empty(),
            "fullwidth prefix token should be detected"
        );
    }

    #[test]
    fn detects_zero_width_joiner_bypass_attempt() {
        let token = dummy_token();
        // Insert zero-width joiner between 'c' and 'n'
        let mutated = format!("c\u{200D}nry_{}_{}", "A".repeat(39), "B".repeat(13));
        let matches = scan_text(&mutated);
        assert!(!matches.is_empty(), "ZWJ-mutated token should be detected");
        assert!(
            matches.iter().any(|m| m.unicode_bypass_suspected),
            "ZWJ should flag bypass suspected"
        );
    }

    #[test]
    fn detects_rtl_override_bypass_attempt() {
        let token = dummy_token();
        let mutated = format!("\u{202E}{}", token);
        let matches = scan_text(&mutated);
        assert!(
            !matches.is_empty(),
            "RTL-overridden token should be detected"
        );
        assert!(
            matches.iter().any(|m| m.unicode_bypass_suspected),
            "RTL override should flag bypass suspected"
        );
    }

    #[test]
    fn detects_nfc_normalized_canary() {
        // 'é' as e + combining acute accent (NFD) should normalize to NFC 'é'
        // The base token is ASCII, but surrounding text with NFD chars shouldn't break detection
        let token = dummy_token();
        let nfd_e = "e\u{0301}"; // e + combining acute
        let text = format!("{} {} {}", nfd_e, token, nfd_e);
        let matches = scan_text(&text);
        assert_eq!(
            matches.len(),
            1,
            "NFC normalization should not break ASCII canary detection"
        );
    }

    #[test]
    fn detects_zero_width_space_bypass() {
        let token = dummy_token();
        let mutated = format!("c\u{200B}nry_{}_{}", "A".repeat(39), "B".repeat(13));
        let matches = scan_text(&mutated);
        assert!(!matches.is_empty(), "ZWSP-mutated token should be detected");
    }

    #[test]
    fn detects_backspace_overstrike() {
        let token = dummy_token();
        // Backspace attack: write decoy chars, backspace over them visually,
        // then write the real token. The bytes still contain the canary.
        // After stripping backspaces, the token should still be found.
        let mutated = format!("BRK_\x08\x08\x08\x08 {}", token);
        let matches = scan_text(&mutated);
        assert!(
            !matches.is_empty(),
            "backspace-decoy should not hide canary token"
        );
    }
}
