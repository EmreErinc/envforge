// ─── Adversarial Input Hardening ──────────────────────────
//
// Pre-processing layers that decode/deobfuscate tool input before
// the secret scanner runs. Each layer operates on the original input
// independently and returns derived strings for scanning.

use base64::Engine;
use regex::Regex;
use std::sync::OnceLock;

// ─── Config ────────────────────────────────────────────────

/// Configuration for each hardening layer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HardeningConfig {
    pub control_chars: bool,
    pub base64_decode: bool,
    pub base64_min_length: usize,
    pub split_strings: bool,
    pub encoding_chain: bool,
    pub encoding_chain_max_depth: usize,
}

impl Default for HardeningConfig {
    fn default() -> Self {
        Self {
            control_chars: true,
            base64_decode: true,
            base64_min_length: 20,
            split_strings: false,
            encoding_chain: true,
            encoding_chain_max_depth: 3,
        }
    }
}

// ─── HardenInput Composition ───────────────────────────────

/// Composes all hardening layers into a single pipeline.
#[derive(Debug, Clone)]
pub struct HardenInput {
    config: HardeningConfig,
}

impl HardenInput {
    pub fn new(config: HardeningConfig) -> Self {
        Self { config }
    }

    /// Run all enabled layers on the input and return derived strings for scanning.
    ///
    /// The original input is never modified. Each layer independently produces
    /// additional strings that should also be scanned for secrets.
    pub fn harden(&self, input: &str) -> Vec<HardenedString> {
        let mut results = Vec::new();

        if self.config.control_chars {
            let cleaned = strip_control_chars(input);
            if cleaned != input {
                results.push(HardenedString {
                    text: cleaned,
                    source: HardenSource::ControlChars,
                });
            }
        }

        if self.config.base64_decode {
            for decoded in find_and_decode_base64(input, self.config.base64_min_length) {
                results.push(HardenedString {
                    text: decoded,
                    source: HardenSource::Base64Decode,
                });
            }
        }

        if self.config.split_strings {
            for assembled in detect_split_strings(input) {
                results.push(HardenedString {
                    text: assembled,
                    source: HardenSource::SplitString,
                });
            }
        }

        if self.config.encoding_chain {
            for decoded in decode_encoding_chain(input, self.config.encoding_chain_max_depth) {
                results.push(HardenedString {
                    text: decoded,
                    source: HardenSource::EncodingChain,
                });
            }
        }

        results
    }
}

/// A string derived from the original input by a hardening layer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HardenedString {
    pub text: String,
    pub source: HardenSource,
}

/// Which hardening layer produced this string.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardenSource {
    ControlChars,
    Base64Decode,
    SplitString,
    EncodingChain,
}

impl std::fmt::Display for HardenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardenSource::ControlChars => write!(f, "control_char_strip"),
            HardenSource::Base64Decode => write!(f, "base64_decode"),
            HardenSource::SplitString => write!(f, "split_string"),
            HardenSource::EncodingChain => write!(f, "encoding_chain"),
        }
    }
}

// ─── Layer 1: Control Character Strip ──────────────────────

/// Strip adversarial control characters and normalize homoglyphs.
pub fn strip_control_chars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        // Skip null bytes
        if ch == '\0' {
            continue;
        }
        // Skip BIDI override marks
        if matches!(ch, '\u{202e}' | '\u{202d}' | '\u{2066}'..='\u{2069}') {
            continue;
        }
        // Skip zero-width characters
        if matches!(ch, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}') {
            continue;
        }
        // Normalize common Cyrillic homoglyphs to ASCII
        result.push(normalize_homoglyph(ch));
    }
    result
}

fn normalize_homoglyph(ch: char) -> char {
    match ch {
        // Cyrillic lookalikes
        '\u{0430}' => 'a', // а (Cyrillic а)
        '\u{0435}' => 'e', // е (Cyrillic е)
        '\u{043e}' => 'o', // о (Cyrillic о)
        '\u{0440}' => 'p', // р (Cyrillic р)
        '\u{0441}' => 'c', // с (Cyrillic с)
        '\u{0445}' => 'x', // х (Cyrillic х)
        '\u{0456}' => 'i', // і (Cyrillic і)
        '\u{0458}' => 'j', // ј (Cyrillic ј)
        '\u{0410}' => 'A', // А (Cyrillic А)
        '\u{0415}' => 'E', // Е (Cyrillic Е)
        '\u{041e}' => 'O', // О (Cyrillic О)
        '\u{0420}' => 'P', // Р (Cyrillic Р)
        '\u{0421}' => 'C', // С (Cyrillic С)
        '\u{0425}' => 'X', // Х (Cyrillic Х)
        _ => ch,
    }
}

// ─── Layer 2: Base64 Decode ────────────────────────────────

/// Find Base64-like substrings and return decoded content.
pub fn find_and_decode_base64(input: &str, min_length: usize) -> Vec<String> {
    let re = base64_regex();
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for mat in re.find_iter(input) {
        let candidate = mat.as_str();
        if candidate.len() < min_length {
            continue;
        }
        // Try standard Base64 first
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(candidate) {
            if let Ok(text) = String::from_utf8(decoded) {
                if text.len() >= 4 && !seen.contains(&text) {
                    seen.insert(text.clone());
                    results.push(text);
                }
            }
        }
        // Try URL-safe Base64
        else if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE.decode(candidate) {
            if let Ok(text) = String::from_utf8(decoded) {
                if text.len() >= 4 && !seen.contains(&text) {
                    seen.insert(text.clone());
                    results.push(text);
                }
            }
        }
    }

    results
}

fn base64_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-z0-9+/]{20,}={0,2}|[A-Za-z0-9_-]{20,}").unwrap())
}

// ─── Layer 3: Split String Detection ───────────────────────

/// Detect string concatenation / template patterns and return assembled strings.
pub fn detect_split_strings(input: &str) -> Vec<String> {
    let mut results = Vec::new();

    // Pattern: "part1" + "part2" + "part3"
    let re_concat = concat_regex();
    for caps in re_concat.captures_iter(input) {
        let mut assembled = String::new();
        for i in 1..=5 {
            if let Some(m) = caps.get(i) {
                assembled.push_str(m.as_str());
            } else {
                break;
            }
        }
        if assembled.len() >= 4 {
            results.push(assembled);
        }
    }

    // Pattern: ${VAR}suffix or prefix${VAR}
    let re_template = template_regex();
    for caps in re_template.captures_iter(input) {
        if let Some(m) = caps.get(1) {
            let var_name = m.as_str();
            // We can't resolve the env var here, but we can flag the pattern
            // by returning a placeholder that the secret scanner might match
            // if the variable name itself is a known secret key
            let placeholder = format!("${{{}}}", var_name);
            results.push(placeholder);
        }
    }

    // Pattern: ['a','b','c'].join('')
    let re_join = join_regex();
    for caps in re_join.captures_iter(input) {
        if let Some(m) = caps.get(1) {
            let chars: String = m
                .as_str()
                .split(',')
                .filter_map(|s| {
                    let trimmed = s.trim().trim_matches('\'').trim_matches('"');
                    if trimmed.len() == 1 {
                        trimmed.chars().next()
                    } else {
                        None
                    }
                })
                .collect();
            if chars.len() >= 4 {
                results.push(chars);
            }
        }
    }

    results
}

fn concat_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Match up to 5 quoted strings joined by +
        Regex::new(
            r#""([^"]{1,50})"\s*\+\s*"([^"]{1,50})"(?:\s*\+\s*"([^"]{1,50})")?(?:\s*\+\s*"([^"]{1,50})")?(?:\s*\+\s*"([^"]{1,50})")?"#,
        )
        .unwrap()
    })
}

fn template_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap())
}

fn join_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"\[\s*((?:['"][^'"]{1,10}['"]\s*,?\s*)+)\]\s*\.\s*join\s*\(\s*['"]\s*['"]\s*\)"#,
        )
        .unwrap()
    })
}

// ─── Layer 4: Encoding Chain Decode ────────────────────────

/// Recursively decode common encoding chains up to max_depth.
pub fn decode_encoding_chain(input: &str, max_depth: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for candidate in find_base64_candidates(input) {
        decode_chain_recursive(&candidate, 0, max_depth, &mut results, &mut seen);
    }

    results
}

fn find_base64_candidates(input: &str) -> Vec<String> {
    let re = base64_regex();
    re.find_iter(input)
        .map(|m| m.as_str().to_string())
        .collect()
}

fn decode_chain_recursive(
    input: &str,
    depth: usize,
    max_depth: usize,
    results: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
) {
    if depth >= max_depth {
        return;
    }

    // Try Base64 decode
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(input) {
        if let Ok(text) = String::from_utf8(decoded) {
            if text.len() >= 4 && !seen.contains(&text) {
                seen.insert(text.clone());
                results.push(text.clone());
                decode_chain_recursive(&text, depth + 1, max_depth, results, seen);
            }
        }
    }

    // Try hex decode
    if input.len() >= 4 && input.len() % 2 == 0 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(decoded) = hex::decode(input) {
            if let Ok(text) = String::from_utf8(decoded) {
                if text.len() >= 4 && !seen.contains(&text) {
                    seen.insert(text.clone());
                    results.push(text.clone());
                    decode_chain_recursive(&text, depth + 1, max_depth, results, seen);
                }
            }
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Control Chars ─────────────────────────────────────

    #[test]
    fn test_strip_null_bytes() {
        let input = "he\0llo\0world";
        assert_eq!(strip_control_chars(input), "helloworld");
    }

    #[test]
    fn test_strip_bidi_marks() {
        let input = "hello\u{202e}world\u{2066}";
        assert_eq!(strip_control_chars(input), "helloworld");
    }

    #[test]
    fn test_strip_zero_width() {
        let input = "se\u{200b}cr\u{200c}et\u{200d}";
        assert_eq!(strip_control_chars(input), "secret");
    }

    #[test]
    fn test_normalize_homoglyphs() {
        // Cyrillic 'а' (U+0430) looks like Latin 'a'
        let input = "p\u{0430}ssword"; // p + Cyrillic a + ssword
        assert_eq!(strip_control_chars(input), "password");
    }

    #[test]
    fn test_control_chars_no_change_for_clean_input() {
        let input = "just normal text";
        assert_eq!(strip_control_chars(input), input);
    }

    // ─── Base64 Decode ─────────────────────────────────────

    #[test]
    fn test_find_and_decode_base64_secret() {
        let secret = "my-secret-value-12345";
        let encoded = base64::engine::general_purpose::STANDARD.encode(secret);
        let input = format!("echo '{}'", encoded);
        let results = find_and_decode_base64(&input, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], secret);
    }

    #[test]
    fn test_base64_min_length_filter() {
        let short = base64::engine::general_purpose::STANDARD.encode("hi");
        let results = find_and_decode_base64(&short, 20);
        assert!(results.is_empty());
    }

    #[test]
    fn test_base64_no_false_positives() {
        let input = "just some normal text without any base64";
        let results = find_and_decode_base64(input, 20);
        assert!(results.is_empty());
    }

    // ─── Split Strings ─────────────────────────────────────

    #[test]
    fn test_detect_concatenation() {
        let input = r#""sec" + "ret" + "value""#;
        let results = detect_split_strings(input);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "secretvalue");
    }

    #[test]
    fn test_detect_template() {
        let input = "${API_KEY}suffix";
        let results = detect_split_strings(input);
        assert!(results.iter().any(|s| s.contains("API_KEY")));
    }

    #[test]
    fn test_detect_array_join() {
        let input = "['s','e','c','r','e','t'].join('')";
        let results = detect_split_strings(input);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "secret");
    }

    // ─── Encoding Chain ────────────────────────────────────

    #[test]
    fn test_decode_encoding_chain_base64() {
        let secret = "my-secret-value-12345";
        let encoded = base64::engine::general_purpose::STANDARD.encode(secret);
        let results = decode_encoding_chain(&encoded, 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], secret);
    }

    #[test]
    fn test_decode_encoding_chain_hex() {
        let hex_str = hex::encode("secret_value_here");
        let results = decode_encoding_chain(&hex_str, 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "secret_value_here");
    }

    // ─── Composition ───────────────────────────────────────

    #[test]
    fn test_harden_input_all_layers() {
        let config = HardeningConfig {
            control_chars: true,
            base64_decode: true,
            base64_min_length: 20,
            split_strings: true,
            encoding_chain: true,
            encoding_chain_max_depth: 3,
        };
        let hardener = HardenInput::new(config);

        let secret = "my-secret-value-12345";
        let b64 = base64::engine::general_purpose::STANDARD.encode(secret);
        let input = format!("echo '{}' + \"{}\"", b64, "suffix");
        let results = hardener.harden(&input);

        // Should find: base64 decoded secret + assembled string
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.text == secret));
    }

    #[test]
    fn test_harden_input_disabled_layers() {
        let config = HardeningConfig {
            control_chars: false,
            base64_decode: false,
            base64_min_length: 20,
            split_strings: false,
            encoding_chain: false,
            encoding_chain_max_depth: 3,
        };
        let hardener = HardenInput::new(config);
        let results = hardener.harden("some input");
        assert!(results.is_empty());
    }

    #[test]
    fn test_harden_source_display() {
        assert_eq!(HardenSource::ControlChars.to_string(), "control_char_strip");
        assert_eq!(HardenSource::Base64Decode.to_string(), "base64_decode");
    }
}
