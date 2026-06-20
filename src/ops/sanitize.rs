use std::path::Path;

use super::OpError;

/// Borrow the first `n` characters of `s` without splitting a UTF-8 codepoint.
///
/// Shared char-boundary-safe primitive for every redaction/truncation sink
/// (TUI render, CLI `mask_value`, table truncation). Replaces raw `&s[..n]`
/// byte-slicing, which panics when byte `n` falls mid-codepoint (M5/M8/FR9).
pub fn char_prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Borrow the last `n` characters of `s` without splitting a UTF-8 codepoint.
pub fn char_suffix(s: &str, n: usize) -> &str {
    let total = s.chars().count();
    if n >= total {
        return s;
    }
    match s.char_indices().nth(total - n) {
        Some((idx, _)) => &s[idx..],
        None => s,
    }
}

/// Heuristic: does `value` look like a secret that must not appear on argv?
///
/// Catches long values, URLs, known token prefixes, AND — new in M1 — short,
/// prefix-less, high-entropy strings (random API keys / passwords) that the
/// length+prefix checks alone slipped onto the command line (visible in `ps`,
/// `/proc/<pid>/cmdline`, shell history). The entropy path is deliberately
/// conservative (mixed character classes + high per-char entropy) so ordinary
/// words/identifiers ("production", "us-east-1") are not flagged.
pub fn value_looks_like_secret(value: &str) -> bool {
    if value.len() > 16 {
        return true;
    }
    if value.contains("://") {
        return true;
    }
    // Prefix list preserved byte-for-byte from the original inline copies (incl.
    // the mixed-case "eyJ"/"AKIA"/"BEGIN " entries) so existing detection
    // contracts are unchanged; M1 only ADDS the entropy path below. (Those
    // mixed-case entries don't match a lowercased value, but the long tokens
    // they'd catch are already flagged by the >16-char length rule.)
    let lower = value.to_lowercase();
    const PREFIXES: &[&str] = &[
        "sk-", "ak-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "xoxb-", "xoxp-", "xapp-", "glpat-",
        "gldt-", "glft-", "glsoat-", "key-", "pk.", "sk.", "whsec_", "eyJ", "AKIA", "ssh-",
        "BEGIN ", "s3cr3t", "passw", "token", "api_key", "secret",
    ];
    if PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    // Entropy path (M1): catch a short (12–16 char), prefix-less secret that the
    // length/prefix checks miss — e.g. a generated password/API key like
    // "x7Kp9mQ2nL4w". Deliberately narrow to a CONTIGUOUS alphanumeric run with
    // BOTH letters and digits and high per-char entropy, so hyphenated/dotted
    // identifiers ("this-is-16-chars", "us-east-1", "v1.2.3", "2024-01-01") and
    // single-class words ("production") are NOT flagged.
    let alnum_only = !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric());
    let has_letter = value.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    value.len() >= 12
        && alnum_only
        && has_letter
        && has_digit
        && shannon_entropy_per_char(value) >= 3.2
}

fn shannon_entropy_per_char(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut total = 0u32;
    for c in s.chars() {
        *counts.entry(c).or_insert(0u32) += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    let total = f64::from(total);
    -counts
        .values()
        .map(|&c| {
            let p = f64::from(c) / total;
            p * p.log2()
        })
        .sum::<f64>()
}

/// Redact the value of every `KEY=VALUE` assignment whose key is sensitive,
/// line by line. Safe to run over unified-diff text: a leading `+`/`-`/space
/// marker (and `+++`/`---` headers) are preserved. Redacts BOTH old and new
/// values, so a diff can never leak a secret on either side (H2/L2/FR1/FR3).
pub fn redact_sensitive_assignments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (content, nl) = match line.strip_suffix('\n') {
            Some(c) => (c, "\n"),
            None => (line, ""),
        };
        out.push_str(&redact_assignment_line(content));
        out.push_str(nl);
    }
    out
}

fn redact_assignment_line(line: &str) -> String {
    // Split off an optional diff marker; leave file headers (+++/---) untouched.
    let (marker, rest) = match line.chars().next() {
        Some('+' | '-' | ' ') => (&line[..1], &line[1..]),
        _ => ("", line),
    };
    if rest.starts_with("++") || rest.starts_with("--") {
        return line.to_string();
    }
    // Preserve indentation and an optional `export ` prefix.
    let trimmed = rest.trim_start();
    let indent = &rest[..rest.len() - trimmed.len()];
    let (exp, kv) = match trimmed.strip_prefix("export ") {
        Some(b) => ("export ", b),
        None => ("", trimmed),
    };
    if let Some(eq) = kv.find('=') {
        let key = &kv[..eq];
        if crate::ops::dotenv::is_sensitive_key(key.trim()) {
            return format!("{}{}{}{}=[REDACTED]", marker, indent, exp, key);
        }
    }
    line.to_string()
}

/// Sanitize a file's content by replacing known secret values with ${KEY} placeholders.
///
/// Returns the sanitized content and the number of replacements made.
pub fn sanitize_content(content: &str, secrets: &[(String, String)]) -> (String, usize) {
    let mut result = content.to_string();
    let mut count = 0;

    // Sort by value length descending (replace longest first to avoid partial matches).
    // Skip only empty values; a previous `>= 4` cutoff allowed 3-char tokens
    // (some API keys / OTPs / pins) to leak through redaction unchanged.
    let mut sorted: Vec<_> = secrets.iter().filter(|(_, v)| !v.is_empty()).collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.1.len()));

    for (key, value) in sorted {
        if result.contains(value.as_str()) {
            result = result.replace(value.as_str(), &format!("${{{}}}", key));
            count += 1;
        }
    }

    (result, count)
}

/// Sanitize a file on disk, replacing secret values with placeholders.
///
/// Reads the input file, replaces secrets, and writes to output (or returns the content).
pub fn sanitize_file(
    input_path: &Path,
    output_path: Option<&Path>,
    secrets: &[(String, String)],
) -> Result<usize, OpError> {
    let content = std::fs::read_to_string(input_path)?;
    let (sanitized, count) = sanitize_content(&content, secrets);

    if let Some(out) = output_path {
        std::fs::write(out, &sanitized)?;
    } else {
        print!("{}", sanitized);
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_replaces_values() {
        let content =
            "DATABASE_URL=postgres://user:supersecretpass@localhost/db\nAPI_KEY=sk-abcdef123456";
        let secrets = vec![
            ("DB_PASSWORD".to_string(), "supersecretpass".to_string()),
            ("API_KEY".to_string(), "sk-abcdef123456".to_string()),
        ];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 2);
        assert!(result.contains("${DB_PASSWORD}"));
        assert!(result.contains("${API_KEY}"));
        assert!(!result.contains("supersecretpass"));
        assert!(!result.contains("sk-abcdef123456"));
    }

    #[test]
    fn test_sanitize_skips_only_empty_values() {
        // Short tokens (3 chars) used to be silently skipped; M5 removed
        // that gap. Empty values are still skipped (replacing "" loops).
        let content = "PORT=80 and TOKEN=abc and EMPTY=";
        let secrets = vec![
            ("PORT".to_string(), "80".to_string()),
            ("TOKEN".to_string(), "abc".to_string()),
            ("EMPTY".to_string(), "".to_string()),
        ];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 2);
        assert!(result.contains("${PORT}"));
        assert!(result.contains("${TOKEN}"));
        assert!(!result.contains("=80"));
        assert!(!result.contains("=abc "));
    }

    #[test]
    fn test_sanitize_longest_first() {
        // If API_KEY_FULL="sk-abcdef123456" and API_KEY="sk-abcdef",
        // replacing longest first avoids partial match issues.
        let content = "token: sk-abcdef123456";
        let secrets = vec![
            ("API_KEY".to_string(), "sk-abcdef".to_string()),
            ("API_KEY_FULL".to_string(), "sk-abcdef123456".to_string()),
        ];

        let (result, count) = sanitize_content(content, &secrets);

        // Should replace the longest match first
        assert_eq!(count, 1);
        assert!(result.contains("${API_KEY_FULL}"));
        assert!(!result.contains("${API_KEY}")); // partial should NOT have matched
    }

    #[test]
    fn test_sanitize_no_secrets() {
        let content = "just some text without secrets";
        let secrets: Vec<(String, String)> = vec![];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 0);
        assert_eq!(result, content);
    }

    #[test]
    fn test_sanitize_multiple_occurrences() {
        let content = "first: mypassword123, second: mypassword123";
        let secrets = vec![("PASSWORD".to_string(), "mypassword123".to_string())];

        let (result, count) = sanitize_content(content, &secrets);

        assert_eq!(count, 1); // one key replaced (even though multiple occurrences)
        assert_eq!(result, "first: ${PASSWORD}, second: ${PASSWORD}");
    }

    #[test]
    fn test_sanitize_file_to_output() {
        let tmp = tempfile::TempDir::new().unwrap();
        let input = tmp.path().join("input.txt");
        let output = tmp.path().join("output.txt");

        std::fs::write(&input, "secret=mysupersecretvalue").unwrap();

        let secrets = vec![("SECRET".to_string(), "mysupersecretvalue".to_string())];
        let count = sanitize_file(&input, Some(&output), &secrets).unwrap();

        assert_eq!(count, 1);
        let result = std::fs::read_to_string(&output).unwrap();
        assert!(result.contains("${SECRET}"));
        assert!(!result.contains("mysupersecretvalue"));
    }
}
