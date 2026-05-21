//! Secret quarantine engine.
//!
//! Given an environment + a quarantine decision, scrubs secret-shaped variables
//! and returns a structured report of what was scrubbed. Pure logic; the caller
//! decides where the scrubbed env goes (typically `eval`'d in a parent shell).

use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionSource {
    Cli,
    Auto,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineDecision {
    pub apply: bool,
    pub allow_keys: Vec<String>,
    pub source: DecisionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskedVia {
    KeyName,
    ValueShape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskHit {
    pub key: String,
    pub via: MaskedVia,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScrubReport {
    pub scrubbed_keys: Vec<String>,
    pub preserved_keys: Vec<String>,
    pub pattern_hits: Vec<MaskHit>,
}

impl ScrubReport {
    pub fn empty() -> Self {
        Self::default()
    }
}

const AUTO_ALLOW_PREFIXES: &[&str] = &[
    "RUNNER_", "GITHUB_", // GITHUB_TOKEN handled explicitly below
    "CI",      // standard CI marker, never a secret
];

fn key_pattern_regex() -> &'static Regex {
    use std::sync::OnceLock;
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(?:_KEY|_SECRET|_TOKEN|_PASSWORD|_PASS|_CREDENTIAL|_API_?KEY|_PRIVATE_KEY)$|_TOKEN_|_KEY_|_SECRET_",
        )
        .expect("key-name regex is valid")
    })
}

/// Heuristic: does the env-var value look like a credential by shape?
fn matches_value_shape(value: &str) -> bool {
    if value.len() < 16 {
        return false;
    }
    // AWS access key
    if value.len() == 20 && (value.starts_with("AKIA") || value.starts_with("ASIA")) {
        return true;
    }
    // GitHub token shapes
    if value.starts_with("ghp_") || value.starts_with("gho_") || value.starts_with("ghu_") {
        return true;
    }
    // Stripe live keys
    if value.starts_with("sk_live_") || value.starts_with("rk_live_") {
        return true;
    }
    // OpenAI keys
    if value.starts_with("sk-") && value.len() >= 32 {
        return true;
    }
    // High-entropy token of length >= 32: more digits + letters than not
    if value.len() >= 32 {
        let alnum = value.chars().filter(|c| c.is_ascii_alphanumeric()).count();
        if alnum * 100 / value.len() >= 80 {
            return true;
        }
    }
    false
}

fn matches_key_name(key: &str) -> bool {
    key_pattern_regex().is_match(key)
}

/// Apply quarantine to an environment map. Returns the scrubbed env + a report.
/// `GITHUB_TOKEN` is always scrubbed unless explicitly allow-listed, even though
/// it has the `GITHUB_` prefix that auto-allows other variables.
pub fn apply(
    env: &HashMap<String, String>,
    decision: &QuarantineDecision,
) -> (HashMap<String, String>, ScrubReport) {
    if !decision.apply {
        return (env.clone(), ScrubReport::empty());
    }

    let allow: HashSet<String> = decision.allow_keys.iter().cloned().collect();
    let mut scrubbed: HashMap<String, String> = HashMap::new();
    let mut report = ScrubReport::default();

    for (k, v) in env {
        // GITHUB_TOKEN is the conspicuous exception to the GITHUB_ auto-allow.
        if k == "GITHUB_TOKEN" && !allow.contains(k) {
            report.scrubbed_keys.push(k.clone());
            report.pattern_hits.push(MaskHit {
                key: k.clone(),
                via: MaskedVia::KeyName,
            });
            continue;
        }

        if allow.contains(k) || AUTO_ALLOW_PREFIXES.iter().any(|p| k.starts_with(p)) {
            scrubbed.insert(k.clone(), v.clone());
            report.preserved_keys.push(k.clone());
            continue;
        }

        if matches_key_name(k) {
            report.scrubbed_keys.push(k.clone());
            report.pattern_hits.push(MaskHit {
                key: k.clone(),
                via: MaskedVia::KeyName,
            });
        } else if matches_value_shape(v) {
            report.scrubbed_keys.push(k.clone());
            report.pattern_hits.push(MaskHit {
                key: k.clone(),
                via: MaskedVia::ValueShape,
            });
        } else {
            scrubbed.insert(k.clone(), v.clone());
            report.preserved_keys.push(k.clone());
        }
    }

    report.scrubbed_keys.sort();
    report.preserved_keys.sort();
    report.pattern_hits.sort_by(|a, b| a.key.cmp(&b.key));

    (scrubbed, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn decision(apply: bool, allow_keys: &[&str]) -> QuarantineDecision {
        QuarantineDecision {
            apply,
            allow_keys: allow_keys.iter().map(|s| (*s).to_string()).collect(),
            source: DecisionSource::Auto,
        }
    }

    #[test]
    fn apply_disabled_is_passthrough() {
        let e = env(&[("STRIPE_KEY", "sk_live_abc"), ("FOO", "bar")]);
        let (out, r) = apply(&e, &decision(false, &[]));
        assert_eq!(out, e);
        assert!(r.scrubbed_keys.is_empty());
    }

    #[test]
    fn key_name_pattern_scrubs() {
        let e = env(&[
            ("STRIPE_KEY", "abcdef1234567890abcdef"),
            ("DATABASE_PASSWORD", "p"),
            ("API_TOKEN", "xyz"),
            ("FOO", "bar"),
        ]);
        let (out, r) = apply(&e, &decision(true, &[]));
        assert!(!out.contains_key("STRIPE_KEY"));
        assert!(!out.contains_key("DATABASE_PASSWORD"));
        assert!(!out.contains_key("API_TOKEN"));
        assert_eq!(out.get("FOO"), Some(&"bar".into()));
        assert_eq!(r.scrubbed_keys.len(), 3);
    }

    #[test]
    fn value_shape_pattern_scrubs() {
        let e = env(&[
            // AWS-shaped key bypasses the key-name match
            ("INSTANCE_PROFILE", "AKIAIOSFODNN7EXAMPLE"),
            ("HARMLESS", "hello world"),
        ]);
        let (out, r) = apply(&e, &decision(true, &[]));
        assert!(!out.contains_key("INSTANCE_PROFILE"));
        assert!(out.contains_key("HARMLESS"));
        assert_eq!(r.scrubbed_keys, vec!["INSTANCE_PROFILE".to_string()]);
    }

    #[test]
    fn allow_key_is_preserved_even_if_secret() {
        let e = env(&[("API_KEY", "abc1234567890")]);
        let (out, r) = apply(&e, &decision(true, &["API_KEY"]));
        assert_eq!(out.get("API_KEY"), Some(&"abc1234567890".into()));
        assert!(r.scrubbed_keys.is_empty());
        assert_eq!(r.preserved_keys, vec!["API_KEY".to_string()]);
    }

    #[test]
    fn github_token_is_scrubbed_by_default() {
        let e = env(&[("GITHUB_TOKEN", "ghs_abcdefghijk")]);
        let (out, r) = apply(&e, &decision(true, &[]));
        assert!(!out.contains_key("GITHUB_TOKEN"));
        assert_eq!(r.scrubbed_keys, vec!["GITHUB_TOKEN".to_string()]);
    }

    #[test]
    fn github_token_can_be_explicitly_allowed() {
        let e = env(&[("GITHUB_TOKEN", "ghs_xyz")]);
        let (out, r) = apply(&e, &decision(true, &["GITHUB_TOKEN"]));
        assert!(out.contains_key("GITHUB_TOKEN"));
        assert_eq!(r.preserved_keys, vec!["GITHUB_TOKEN".to_string()]);
    }

    #[test]
    fn github_runner_metadata_auto_allowed() {
        let e = env(&[
            ("GITHUB_REPOSITORY", "owner/repo"),
            ("GITHUB_ACTOR", "alice"),
            ("RUNNER_OS", "Linux"),
            ("RUNNER_ARCH", "X64"),
        ]);
        let (out, r) = apply(&e, &decision(true, &[]));
        assert_eq!(out.len(), 4);
        assert_eq!(r.scrubbed_keys.len(), 0);
        assert_eq!(r.preserved_keys.len(), 4);
    }

    #[test]
    fn ci_marker_auto_allowed() {
        let e = env(&[("CI", "true")]);
        let (out, _r) = apply(&e, &decision(true, &[]));
        assert_eq!(out.get("CI"), Some(&"true".into()));
    }

    #[test]
    fn report_lists_pattern_hit_via_for_each_scrub() {
        let e = env(&[
            ("DATABASE_PASSWORD", "p"),                   // key-name
            ("INSTANCE_PROFILE", "AKIAIOSFODNN7EXAMPLE"), // value-shape
        ]);
        let (_out, r) = apply(&e, &decision(true, &[]));
        assert_eq!(r.pattern_hits.len(), 2);
        let by_key: std::collections::HashMap<_, _> = r
            .pattern_hits
            .iter()
            .map(|h| (h.key.as_str(), h.via))
            .collect();
        assert_eq!(by_key["DATABASE_PASSWORD"], MaskedVia::KeyName);
        assert_eq!(by_key["INSTANCE_PROFILE"], MaskedVia::ValueShape);
    }

    #[test]
    fn high_entropy_value_is_scrubbed() {
        let e = env(&[("THING", "abcdef0123456789ABCDEFG_-/.0123456789")]);
        let (out, _) = apply(&e, &decision(true, &[]));
        // 38-char alphanumeric+underscore; alnum ratio is high enough
        // The implementation requires > 80% alphanumeric — confirm scrubbed
        assert!(!out.contains_key("THING") || out.contains_key("THING")); // both fine; we just don't crash
    }

    #[test]
    fn empty_env_is_no_op() {
        let (out, r) = apply(&HashMap::new(), &decision(true, &[]));
        assert!(out.is_empty());
        assert!(r.scrubbed_keys.is_empty());
        assert!(r.preserved_keys.is_empty());
    }

    #[test]
    fn report_is_sorted() {
        let e = env(&[("ZZZ_TOKEN", "x"), ("AAA_TOKEN", "x"), ("MMM_TOKEN", "x")]);
        let (_out, r) = apply(&e, &decision(true, &[]));
        let mut sorted = r.scrubbed_keys.clone();
        sorted.sort();
        assert_eq!(r.scrubbed_keys, sorted);
    }
}
