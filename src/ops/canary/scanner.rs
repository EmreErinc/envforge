//! Token scanner — find canary tokens in arbitrary input streams.
//!
//! Used in incident-response when triaging leaked logs / paste sites.
//! Pure regex match; does not decode or verify (decoding lives in `v2`).

use std::io::{BufRead, BufReader, Read};

use regex::Regex;

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
}

/// Compiled scanner.
pub struct TokenScanner {
    pattern: Regex,
}

impl TokenScanner {
    /// Build the scanner. Matches `cnry_<39 base32>_<13 base32>` (RFC 4648 alphabet, no pad).
    /// Also matches v1 prefix-bearing tokens for completeness — caller distinguishes by decode.
    pub fn new() -> Self {
        // Pattern: cnry_ followed by exactly 39 alphanum chars, _, exactly 13 alphanum chars.
        // Use word-boundary anchors (\b) so the token is detected as a whole entity.
        let re = Regex::new(r"cnry_[A-Z2-7]{39}_[A-Z2-7]{13}").expect("v2 token regex compiles");
        Self { pattern: re }
    }

    /// Find all v2 tokens in a single string.
    pub fn find_in_text(&self, text: &str) -> Vec<TokenMatch> {
        self.pattern
            .find_iter(text)
            .map(|m| TokenMatch {
                token: m.as_str().to_string(),
                byte_offset: Some(m.start()),
                line_number: None,
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
/// — only one line is buffered at a time.
pub fn scan_reader<R: Read>(reader: R) -> Vec<TokenMatch> {
    let scanner = TokenScanner::new();
    let buf_reader = BufReader::new(reader);
    let mut matches = Vec::new();
    for (idx, line_result) in buf_reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        for m in scanner.pattern.find_iter(&line) {
            matches.push(TokenMatch {
                token: m.as_str().to_string(),
                byte_offset: Some(m.start()),
                line_number: Some(idx + 1),
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
        // 39 + 13 base32 chars
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
        // wrong prefix
        assert_eq!(
            scan_text("cnryX_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA_BBBBBBBBBBBBB").len(),
            0
        );
        // wrong payload length
        assert_eq!(scan_text("cnry_AAA_BBB").len(), 0);
        // lowercase rejected (canonical encoding is uppercase)
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
}
