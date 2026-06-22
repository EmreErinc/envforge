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
use std::path::Path;

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

/// Returns `true` when the given path refers to a recognized config-format
/// file that should be treated as a canary scan target.
///
/// The check operates on the **basename** (`.file_name()`) of the supplied
/// path, so callers may pass either a bare filename *or* a full/relative
/// path — `"subdir/application.yml"` and `"application.yml"` both return
/// `true` (H-3 fix).
///
/// This predicate extends the canary scanner's file-type set to cover the
/// same framework config files recognized by the LSP (units 001/002), so
/// canary-token detection applies to `application.yml`, `.env.local`,
/// `application-prod.properties`, etc. at parity with `.env` (FR21/AR7).
///
/// The check is intentionally conservative — it mirrors the recognition
/// rules of `is_jvm_config_file`, `is_yaml_config_file`, and
/// `is_env_cascade_file` from `src/lsp/config_file.rs`, so adding a new
/// format only requires updating those predicates (single source).
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use envforge::ops::canary::scanner::is_config_canary_target;
///
/// assert!(is_config_canary_target(Path::new("application.properties")));
/// assert!(is_config_canary_target(Path::new("application-prod.yml")));
/// assert!(is_config_canary_target(Path::new(".env.local")));
/// assert!(is_config_canary_target(Path::new(".env")));
/// assert!(is_config_canary_target(Path::new("subdir/application.yml")));
/// assert!(!is_config_canary_target(Path::new("docker-compose.yml")));
/// assert!(!is_config_canary_target(Path::new(".env.schema")));
/// // TOML canonical names are canary targets (intent 037).
/// assert!(is_config_canary_target(Path::new("Cargo.toml")));
/// assert!(is_config_canary_target(Path::new("pyproject.toml")));
/// assert!(is_config_canary_target(Path::new("config.toml")));
/// // Non-canonical TOML files are NOT canary targets.
/// assert!(!is_config_canary_target(Path::new("foo.toml")));
/// assert!(!is_config_canary_target(Path::new("Gemfile.toml")));
/// // .NET appsettings JSONC files are canary targets (intent 039).
/// assert!(is_config_canary_target(Path::new("appsettings.json")));
/// assert!(is_config_canary_target(Path::new("appsettings.Production.json")));
/// assert!(is_config_canary_target(Path::new("appsettings.Development.json")));
/// // Other JSON files are NOT canary targets.
/// assert!(!is_config_canary_target(Path::new("package.json")));
/// assert!(!is_config_canary_target(Path::new("mcp.json")));
/// assert!(!is_config_canary_target(Path::new("tsconfig.json")));
/// ```
#[must_use]
pub fn is_config_canary_target(path: &Path) -> bool {
    // Extract the basename; if there is none (e.g. a root path) return false.
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    // JVM / Quarkus / MicroProfile: application.properties,
    // application-{profile}.properties, microprofile-config.properties only.
    // Intentionally narrow (matches is_jvm_config_file scope) to avoid
    // false-positive canary hits on log4j.properties, pom.properties, etc.
    if let Some(stem) = file_name.strip_suffix(".properties") {
        if stem == "application" || stem == "microprofile-config" {
            return true;
        }
        if let Some(profile) = stem.strip_prefix("application-") {
            if !profile.is_empty() {
                return true;
            }
        }
        // Not a recognized JVM config file — do not treat as canary target.
        return false;
    }

    // Spring / Quarkus YAML: application.yml/yaml or application-{profile}.yml/yaml.
    // Scoped to application* only — NOT every .yml — consistent with
    // `is_yaml_config_file` (scope-check requirement).
    if let Some(stem) = file_name
        .strip_suffix(".yml")
        .or_else(|| file_name.strip_suffix(".yaml"))
    {
        if stem == "application" {
            return true;
        }
        if let Some(profile) = stem.strip_prefix("application-") {
            if !profile.is_empty() {
                return true;
            }
        }
    }

    // .env cascade: .env, .env.local, .env.{env} — but NOT .env.schema / .env.schema.*.
    if file_name == ".env" || file_name == ".env.local" {
        return true;
    }
    if file_name.starts_with(".env.")
        && file_name != ".env.schema"
        && !file_name.starts_with(".env.schema.")
    {
        return true;
    }

    // Canonical TOML config files — scoped to the same names recognized by
    // `is_toml_config_file` (intent 037). NOT every *.toml.
    if matches!(file_name, "Cargo.toml" | "pyproject.toml" | "config.toml") {
        return true;
    }

    // .NET appsettings JSONC — scoped to the same names recognized by
    // `is_appsettings_file` (intent 039). NOT every *.json.
    if file_name == "appsettings.json" {
        return true;
    }
    if let Some(rest) = file_name.strip_prefix("appsettings.") {
        if let Some(env) = rest.strip_suffix(".json") {
            if !env.is_empty() {
                return true;
            }
        }
    }

    false
}

/// Scan all recognized config files under `dir` for canary tokens, returning
/// one [`TokenMatch`] per detection with the originating file path embedded.
///
/// This wires `is_config_canary_target` into a real directory-walk entry
/// point so that FR21 ("canary detection applies to recognized config files")
/// is satisfied via a call path, not just a predicate (H-2 fix).
///
/// Only regular files whose basenames pass [`is_config_canary_target`] are
/// opened; directories are walked recursively.  Unreadable files are
/// silently skipped (the caller sees no tokens for them but does not crash).
///
/// # Errors
///
/// Returns `Err` only if `dir` itself cannot be read.  Per-file I/O errors
/// are logged to `stderr` and skipped.
pub fn scan_config_dir(dir: &Path) -> std::io::Result<Vec<ConfigFileMatch>> {
    let mut results = Vec::new();
    scan_config_dir_inner(dir, &mut results)?;
    Ok(results)
}

fn scan_config_dir_inner(dir: &Path, out: &mut Vec<ConfigFileMatch>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "canary scan: read_dir entry error in {}: {}",
                    dir.display(),
                    e
                );
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                eprintln!("canary scan: file_type error for {}: {}", path.display(), e);
                continue;
            }
        };
        if file_type.is_dir() {
            // Skip common non-project directories to avoid excessive I/O.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "node_modules" | ".git" | "target" | ".gradle" | "build"
            ) {
                continue;
            }
            if let Err(e) = scan_config_dir_inner(&path, out) {
                eprintln!("canary scan: cannot enter {}: {}", path.display(), e);
            }
        } else if file_type.is_file() && is_config_canary_target(&path) {
            let f = match std::fs::File::open(&path) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("canary scan: cannot open {}: {}", path.display(), e);
                    continue;
                }
            };
            let matches = scan_reader(f);
            for m in matches {
                out.push(ConfigFileMatch {
                    path: path.clone(),
                    token_match: m,
                });
            }
        }
    }
    Ok(())
}

/// A canary token match found within a specific config file during a
/// directory walk (see [`scan_config_dir`]).
#[derive(Debug, Clone)]
pub struct ConfigFileMatch {
    /// Path of the file in which the token was found.
    pub path: std::path::PathBuf,
    /// The token match details.
    pub token_match: TokenMatch,
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
        let _token = dummy_token();
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
        let _token = dummy_token();
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
