//! Canonicalization pipeline shared between the poisoning scanner and a
//! future ENV-value scanner.
//!
//! 7 steps: NFKC, lowercase, strip-zero-width, line-sep-normalize,
//! leetspeak-fold, whitespace-collapse, non-allowed-strip.

use unicode_normalization::UnicodeNormalization;

use crate::ops::mcp_poison::error::ScannerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalText(String);

impl CanonicalText {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

pub struct Canonicalizer;

impl Canonicalizer {
    pub const MAX_INPUT_BYTES: usize = 64 * 1024;

    pub fn canonicalize(text: &str) -> Result<CanonicalText, ScannerError> {
        if text.len() > Self::MAX_INPUT_BYTES {
            return Err(ScannerError::InputTooLarge {
                size: text.len(),
                limit: Self::MAX_INPUT_BYTES,
            });
        }

        // 1. NFKC
        let nfkc: String = text.nfkc().collect();
        // 2. Lowercase (Unicode-aware)
        let lower: String = nfkc.to_lowercase();
        // 3. Strip zero-width
        let stripped = strip_zero_width(&lower);
        // 4. Normalize line separators
        let line_normalized = stripped
            .replace(['\u{2028}', '\u{2029}'], "\n")
            .replace("\r\n", "\n");
        // 5. Leetspeak fold
        let leet = fold_leetspeak(&line_normalized);
        // 6. Collapse whitespace
        let ws = collapse_whitespace(&leet);
        // 7. Strip non-allowed
        let cleaned = strip_non_allowed(&ws);
        Ok(CanonicalText(cleaned))
    }
}

fn strip_zero_width(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(
                *c,
                '\u{200B}'
                    | '\u{200C}'
                    | '\u{200D}'
                    | '\u{200E}'
                    | '\u{200F}'
                    | '\u{FEFF}'
                    | '\u{2060}'
            )
        })
        .collect()
}

fn fold_leetspeak(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'i',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            '8' => 'b',
            '@' => 'a',
            '$' => 's',
            other => other,
        })
        .collect()
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_was_space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            if !prev_was_space {
                out.push(' ');
                prev_was_space = true;
            }
        } else {
            out.push(c);
            prev_was_space = false;
        }
    }
    out
}

fn strip_non_allowed(s: &str) -> String {
    s.chars()
        .filter(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(*c, ' ' | '\n' | ':' | '<' | '>' | '/' | '-')
        })
        .collect()
}
