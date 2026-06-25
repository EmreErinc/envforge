//! Coverage for `ops::ci_trust::quarantine::apply` — the secret-scrubbing engine
//! that strips credential-shaped env vars before an untrusted CI step runs.

use envforge::ops::ci_trust::quarantine::{apply, DecisionSource, MaskedVia, QuarantineDecision};
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn decision(apply_it: bool, allow: &[&str]) -> QuarantineDecision {
    QuarantineDecision {
        apply: apply_it,
        allow_keys: allow.iter().map(|s| s.to_string()).collect(),
        source: DecisionSource::Auto,
    }
}

#[test]
fn test_apply_disabled_is_passthrough() {
    let e = env(&[("API_KEY", "secret")]);
    let (out, report) = apply(&e, &decision(false, &[]));
    assert_eq!(out, e);
    assert!(report.scrubbed_keys.is_empty());
}

#[test]
fn test_github_token_always_scrubbed_despite_prefix() {
    // GITHUB_ prefix auto-allows, but GITHUB_TOKEN is the explicit exception.
    let e = env(&[("GITHUB_TOKEN", "ghs_xxx"), ("GITHUB_SHA", "abc123")]);
    let (out, report) = apply(&e, &decision(true, &[]));
    assert!(!out.contains_key("GITHUB_TOKEN"), "token must be removed");
    assert!(
        out.contains_key("GITHUB_SHA"),
        "non-secret GITHUB_ var preserved"
    );
    assert!(report.scrubbed_keys.contains(&"GITHUB_TOKEN".to_string()));
    assert!(report.preserved_keys.contains(&"GITHUB_SHA".to_string()));
}

#[test]
fn test_scrub_by_key_name() {
    let e = env(&[("AWS_SECRET_KEY", "whatever"), ("PLAIN", "hello")]);
    let (out, report) = apply(&e, &decision(true, &[]));
    assert!(!out.contains_key("AWS_SECRET_KEY"));
    assert!(out.contains_key("PLAIN"));
    assert!(report
        .pattern_hits
        .iter()
        .any(|h| h.key == "AWS_SECRET_KEY" && h.via == MaskedVia::KeyName));
}

#[test]
fn test_scrub_by_value_shape() {
    // Key name is innocuous, but the value looks like a GitHub token.
    let e = env(&[("RANDOM", "ghp_0123456789abcdefghij")]);
    let (out, report) = apply(&e, &decision(true, &[]));
    assert!(!out.contains_key("RANDOM"));
    assert!(report
        .pattern_hits
        .iter()
        .any(|h| h.key == "RANDOM" && h.via == MaskedVia::ValueShape));
}

#[test]
fn test_allow_list_preserves_secret() {
    let e = env(&[("MY_TOKEN", "ghp_0123456789abcdefghij")]);
    let (out, report) = apply(&e, &decision(true, &["MY_TOKEN"]));
    assert!(
        out.contains_key("MY_TOKEN"),
        "allow-listed key is preserved"
    );
    assert!(report.preserved_keys.contains(&"MY_TOKEN".to_string()));
}

#[test]
fn test_plain_value_preserved() {
    let e = env(&[("LOG_LEVEL", "debug"), ("PORT", "8080")]);
    let (out, _) = apply(&e, &decision(true, &[]));
    assert!(out.contains_key("LOG_LEVEL"));
    assert!(out.contains_key("PORT"));
}
