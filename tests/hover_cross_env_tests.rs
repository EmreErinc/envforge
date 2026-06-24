//! Tests for per-environment hover (Epic 4 / FR15): `hover_info` enriched with
//! the project [`EnvKeySet`]. Shows a key's value in each environment, with
//! sensitive values never rendered raw (NFR4).

use std::path::Path;

use envforge::lsp::document::parse_env_document;
use envforge::lsp::hover::hover_info;
use envforge::ops::env_keyset::{build_env_keyset_from_sources, EnvKeySet};
use tower_lsp::lsp_types::{Hover, HoverContents, Position};

fn keyset(sources: &[(&str, &str, &str)]) -> EnvKeySet {
    let refs: Vec<(&str, &Path, &str)> = sources
        .iter()
        .map(|(n, f, c)| (*n, Path::new(*f), *c))
        .collect();
    build_env_keyset_from_sources(&refs)
}

fn markdown(h: Hover) -> String {
    match h.contents {
        HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup hover"),
    }
}

#[test]
fn test_hover_shows_per_environment_values() {
    let content = "DATABASE_URL=local\n";
    let entries = parse_env_document(content);
    let ks = keyset(&[
        ("dev", "/p/.env.dev", "DATABASE_URL=local\n"),
        ("prod", "/p/.env.prod", "DATABASE_URL=prod-db\n"),
    ]);

    let pos = Position {
        line: 0,
        character: 0,
    };
    let hover = hover_info(pos, &entries, None, &[], Some(&ks)).expect("hover present");
    let md = markdown(hover);

    // Shows which environments set the key — but NOT the raw values (the LSP
    // never emits values in display surfaces).
    assert!(md.contains("Set in environments"), "md: {md}");
    assert!(md.contains("dev"), "md: {md}");
    assert!(md.contains("prod"), "md: {md}");
    assert!(!md.contains("prod-db"), "raw value must not appear: {md}");
    assert!(!md.contains("local"), "raw value must not appear: {md}");
}

#[test]
fn test_hover_redacts_sensitive_value_across_environments() {
    let content = "API_KEY=local-secret\n";
    let entries = parse_env_document(content);
    let ks = keyset(&[("prod", "/p/.env.prod", "API_KEY=prod-super-secret\n")]);

    let pos = Position {
        line: 0,
        character: 0,
    };
    let hover = hover_info(pos, &entries, None, &[], Some(&ks)).expect("hover present");
    let md = markdown(hover);

    assert!(md.contains("Set in environments"), "md: {md}");
    assert!(md.contains("(sensitive)"), "md: {md}");
    assert!(
        !md.contains("prod-super-secret"),
        "raw sensitive value leaked into hover: {md}"
    );
    assert!(
        !md.contains("local-secret"),
        "raw sensitive value leaked into hover: {md}"
    );
}

#[test]
fn test_hover_none_without_schema_managed_or_keyset() {
    // No schema, no managed, no keyset → no hover (baseline behavior).
    let content = "FOO=bar\n";
    let entries = parse_env_document(content);
    let pos = Position {
        line: 0,
        character: 0,
    };
    assert!(hover_info(pos, &entries, None, &[], None).is_none());
}
