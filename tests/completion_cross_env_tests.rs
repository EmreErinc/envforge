//! Tests for cross-environment completion (Epic 3 / FR11, FR12): the
//! `completions()` path enriched with the project [`EnvKeySet`]. Verifies key
//! completion surfaces keys from other environments and value completion
//! surfaces cross-env values — while sensitive keys never leak a raw cross-env
//! value (redaction parity, NFR4).

use std::path::Path;

use envforge::lsp::completion::completions;
use envforge::lsp::document::parse_env_document;
use envforge::lsp::server::ManagedVar;
use envforge::ops::env_keyset::{build_env_keyset_from_sources, EnvKeySet};
use tower_lsp::lsp_types::Position;

fn keyset(sources: &[(&str, &str, &str)]) -> EnvKeySet {
    let refs: Vec<(&str, &Path, &str)> = sources
        .iter()
        .map(|(n, f, c)| (*n, Path::new(*f), *c))
        .collect();
    build_env_keyset_from_sources(&refs)
}

#[test]
fn test_key_completion_includes_project_keys_from_other_envs() {
    // Current file defines only EXISTING; the project key-set also has
    // DATABASE_URL (from prod) and REDIS_URL (from dev).
    let content = "EXISTING=1\n";
    let entries = parse_env_document(content);
    let ks = keyset(&[
        ("prod", "/p/.env.prod", "EXISTING=1\nDATABASE_URL=prod-db\n"),
        ("dev", "/p/.env.dev", "REDIS_URL=localhost\n"),
    ]);

    // Cursor at the start of a fresh second line (key position).
    let pos = Position {
        line: 1,
        character: 0,
    };
    let items = completions(pos, content, &entries, None, &[], Some(&ks));
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"DATABASE_URL"), "labels: {labels:?}");
    assert!(labels.contains(&"REDIS_URL"), "labels: {labels:?}");
}

#[test]
fn test_value_completion_includes_cross_env_value_for_nonsensitive_key() {
    // Typing the value of LOG_LEVEL, which holds "info" in prod.
    let content = "LOG_LEVEL=";
    let entries = parse_env_document(content);
    let ks = keyset(&[("prod", "/p/.env.prod", "LOG_LEVEL=info\n")]);

    let pos = Position {
        line: 0,
        character: 10, // just after '='
    };
    let items = completions(pos, content, &entries, None, &[], Some(&ks));
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"info"), "labels: {labels:?}");
}

#[test]
fn test_value_completion_never_leaks_raw_value_for_sensitive_key() {
    // API_KEY is sensitive (key heuristic). Its prod value must NOT appear as a
    // completion; only a safe marker.
    let content = "API_KEY=";
    let entries = parse_env_document(content);
    let ks = keyset(&[("prod", "/p/.env.prod", "API_KEY=super-secret-123\n")]);

    let pos = Position {
        line: 0,
        character: 8, // just after '='
    };
    let items = completions(pos, content, &entries, None, &[], Some(&ks));
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        !labels.iter().any(|l| l.contains("super-secret-123")),
        "raw sensitive value leaked into completion: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l.contains("sensitive")),
        "expected a sensitive marker: {labels:?}"
    );
    // And no item carries the raw value as inserted text.
    for item in &items {
        if let Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(edit)) = &item.text_edit {
            assert!(
                !edit.new_text.contains("super-secret-123"),
                "raw sensitive value leaked into text_edit"
            );
        }
    }
}

#[test]
fn test_project_keys_rank_above_managed_shell_vars() {
    // In a project env file, the project's own keys must rank above the user's
    // global shell-managed vars — otherwise they get buried (the reported bug).
    let content = "EXISTING=1\n";
    let entries = parse_env_document(content);
    let ks = keyset(&[("prod", "/p/.env.prod", "PROJECT_KEY=x\n")]);
    let managed = vec![ManagedVar {
        key: "SHELL_VAR".to_string(),
        source_file: "/home/me/.zshrc".to_string(),
    }];

    let pos = Position {
        line: 1,
        character: 0,
    };
    let items = completions(pos, content, &entries, None, &managed, Some(&ks));
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    let pk = labels.iter().position(|l| *l == "PROJECT_KEY");
    let sv = labels.iter().position(|l| *l == "SHELL_VAR");
    assert!(pk.is_some() && sv.is_some(), "both present: {labels:?}");
    assert!(
        pk < sv,
        "project key must rank above managed shell var: {labels:?}"
    );
}

#[test]
fn test_no_keyset_preserves_baseline_behavior() {
    // With no EnvKeySet, completion offers nothing extra beyond the baseline
    // (no schema, no managed) — i.e. empty here.
    let content = "FOO=\n";
    let entries = parse_env_document(content);
    let pos = Position {
        line: 1,
        character: 0,
    };
    let items = completions(pos, content, &entries, None, &[], None);
    assert!(items.is_empty(), "expected no completions, got {items:?}");
}
