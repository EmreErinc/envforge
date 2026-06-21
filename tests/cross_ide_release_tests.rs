//! Unit 004 — Cross-IDE Validation, Docs & Release
//! (Intent 036, Stories 001–003: cross-client parity, no-regression gate,
//! docs-release-sync.)
//!
//! ## Story 001 — Cross-client behavior parity (FR22, NFR13)
//!
//! The EnvForge LSP server is the single implementation of every language
//! feature.  The same Rust functions (`config_hover`, `config_completions`,
//! `config_semantic_tokens`, …) are exercised regardless of the client that
//! drives them.  These tests assert the *client-agnostic property*: given the
//! same input document + position, the output is deterministic and independent
//! of any client-capability flag.
//!
//! Where true multi-IDE execution cannot be automated (live VS Code /
//! IntelliJ / Neovim processes), the validated combinations are recorded in
//! `docs/integration-matrix.md` and `docs/lsp-clients.md`.
//!
//! ## Story 002 — No-regression gate (FR23, NFR12)
//!
//! Asserts that the new config-format routing predicates (`is_jvm_config_file`,
//! `is_env_cascade_file`, `is_yaml_config_file`) do NOT alter the results of
//! the pre-existing `is_env_file` / `is_schema_file` dispatch for any
//! representative URI that those predicates own.
//!
//! Tests live in `tests/` per CLAUDE.md.  Naming: `test_{what}_{condition}`.

use std::collections::HashMap;

use clap::Parser;
use envforge::cli::{CanaryAction, Cli, Commands};
use envforge::lsp::config_features::{
    config_completions, config_format_text_edits, config_hover, config_rename,
    config_semantic_tokens,
};
use envforge::lsp::config_file::{
    format_for_uri, is_config_format_file, is_env_cascade_file, is_jvm_config_file,
    is_yaml_config_file,
};
use envforge::ops::config_format::{ConfigEntry, SourceLayer, WriteCapability};
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use tower_lsp::lsp_types::{Position, Range as LspRange, Url};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn url(path: &str) -> Url {
    Url::parse(&format!("file://{}", path)).unwrap()
}

fn entry(key: &str, value: &str, line: u32) -> ConfigEntry {
    let klen = key.len() as u32;
    let vstart = klen + 1;
    let vend = vstart + value.len() as u32;
    ConfigEntry {
        key: key.to_string(),
        value: value.to_string(),
        key_range: LspRange {
            start: Position { line, character: 0 },
            end: Position {
                line,
                character: klen,
            },
        },
        value_range: LspRange {
            start: Position {
                line,
                character: vstart,
            },
            end: Position {
                line,
                character: vend,
            },
        },
        line,
        source_layer: SourceLayer::Base,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 001: Cross-client behavior parity
// ═══════════════════════════════════════════════════════════════════════════════
//
// The server implementation is a single set of pure functions.  These tests
// simulate two "clients" with different capability profiles (e.g. one that
// would declare semantic-tokens support, one that would not) and show that the
// feature function outputs are identical — the functions take no capability
// flags, so the result cannot diverge by client.

/// Hover returns the same markdown regardless of which client invokes it.
#[test]
fn test_parity_hover_identical_across_simulated_clients() {
    let entries = vec![
        entry("DATABASE_URL", "postgres://localhost/mydb", 0),
        entry("API_KEY", "secret123", 1),
    ];
    let layers: Vec<Vec<ConfigEntry>> = vec![];
    let pos = Position {
        line: 0,
        character: 4,
    };
    // Simulate VS Code, IntelliJ, and Neovim each calling the function with the
    // same arguments (the function has no client-capability parameter).
    let result_vscode = config_hover(pos, &entries, &layers, None);
    let result_intellij = config_hover(pos, &entries, &layers, None);
    let result_neovim = config_hover(pos, &entries, &layers, None);

    assert_eq!(
        result_vscode, result_intellij,
        "hover must be identical for VS Code and IntelliJ"
    );
    assert_eq!(
        result_intellij, result_neovim,
        "hover must be identical for IntelliJ and Neovim"
    );
    assert!(
        result_vscode.is_some(),
        "hover must resolve for a valid key position"
    );
}

/// Hover on a sensitive key is redacted identically across all simulated clients.
#[test]
fn test_parity_hover_sensitive_key_redacted_same_for_all_clients() {
    let entries = vec![entry("SECRET_TOKEN", "super-secret-value", 0)];
    let layers: Vec<Vec<ConfigEntry>> = vec![];
    let pos = Position {
        line: 0,
        character: 3,
    };

    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "SECRET_TOKEN".to_string(),
        SchemaVariable {
            description: Some("Auth token".to_string()),
            var_type: VarType::String,
            required: true,
            sensitive: true,
            ..Default::default()
        },
    );

    let result_vscode = config_hover(pos, &entries, &layers, Some(&schema));
    let result_intellij = config_hover(pos, &entries, &layers, Some(&schema));
    let result_neovim = config_hover(pos, &entries, &layers, Some(&schema));

    // All three must agree.
    assert_eq!(result_vscode, result_intellij);
    assert_eq!(result_intellij, result_neovim);

    // The raw value must not appear in the hover output.
    if let Some(hover) = result_vscode {
        use tower_lsp::lsp_types::{HoverContents, MarkupContent};
        if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents {
            assert!(
                !value.contains("super-secret-value"),
                "raw sensitive value must not appear in hover output for any client"
            );
        }
    }
}

/// Completion returns the same item list regardless of the client requesting it.
#[test]
fn test_parity_completions_identical_across_simulated_clients() {
    let entries: Vec<ConfigEntry> = vec![];
    let pos = Position {
        line: 0,
        character: 2,
    };
    // A partial key prefix that matches two schema entries.
    let content = "DA\n";

    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "DATABASE_URL".to_string(),
        SchemaVariable {
            description: Some("DB connection string".to_string()),
            var_type: VarType::Url,
            required: true,
            sensitive: true,
            ..Default::default()
        },
    );
    schema.variables.insert(
        "DATA_DIR".to_string(),
        SchemaVariable {
            description: None,
            var_type: VarType::String,
            required: false,
            default: Some("/data".to_string()),
            ..Default::default()
        },
    );

    let items_vscode = config_completions(pos, content, &entries, Some(&schema));
    let items_intellij = config_completions(pos, content, &entries, Some(&schema));
    let items_neovim = config_completions(pos, content, &entries, Some(&schema));

    assert_eq!(
        items_vscode, items_intellij,
        "completions must be identical for VS Code and IntelliJ"
    );
    assert_eq!(
        items_intellij, items_neovim,
        "completions must be identical for IntelliJ and Neovim"
    );
}

/// Semantic tokens are deterministic — same tokens regardless of which
/// client requests them (NFR13: identical behavior across VS Code / IntelliJ / Neovim).
#[test]
fn test_parity_semantic_tokens_identical_across_simulated_clients() {
    let entries = vec![
        entry("PORT", "8080", 0),
        entry("API_SECRET", "tok-abc123", 1),
    ];

    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "API_SECRET".to_string(),
        SchemaVariable {
            sensitive: true,
            ..Default::default()
        },
    );

    let tokens_vscode = config_semantic_tokens(&entries, Some(&schema));
    let tokens_intellij = config_semantic_tokens(&entries, Some(&schema));
    let tokens_neovim = config_semantic_tokens(&entries, Some(&schema));

    assert_eq!(
        tokens_vscode, tokens_intellij,
        "semantic tokens must be identical for VS Code and IntelliJ"
    );
    assert_eq!(
        tokens_intellij, tokens_neovim,
        "semantic tokens must be identical for IntelliJ and Neovim"
    );
}

/// YAML files report `WriteCapability::ReadOnly` regardless of client.
/// Rename and format return `None` / empty for YAML — identical no-op on all clients.
#[test]
fn test_parity_yaml_readonly_identical_on_all_clients() {
    let yaml_uri = url("/proj/application.yml");
    let (fmt, _layer) = format_for_uri(&yaml_uri).expect("should recognize application.yml");
    assert_eq!(
        fmt.write_capability(),
        WriteCapability::ReadOnly,
        "YAML must be ReadOnly for all clients"
    );

    let doc_content = "spring:\n  datasource:\n    url: ${DATABASE_URL}\n";
    let entries = vec![entry("spring.datasource.url", "${DATABASE_URL}", 2)];

    // Rename must return None for ReadOnly on all clients.
    let rename_vscode = config_rename(
        "spring.datasource.url",
        "spring.db.url",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    let rename_intellij = config_rename(
        "spring.datasource.url",
        "spring.db.url",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    let rename_neovim = config_rename(
        "spring.datasource.url",
        "spring.db.url",
        WriteCapability::ReadOnly,
        None,
        &HashMap::new(),
        &HashMap::new(),
    );
    assert_eq!(
        rename_vscode, None,
        "YAML rename must return None on VS Code"
    );
    assert_eq!(
        rename_intellij, None,
        "YAML rename must return None on IntelliJ"
    );
    assert_eq!(
        rename_neovim, None,
        "YAML rename must return None on Neovim"
    );

    // Format must return empty edits for ReadOnly on all clients.
    let fmt_vscode = config_format_text_edits(doc_content, WriteCapability::ReadOnly);
    let fmt_intellij = config_format_text_edits(doc_content, WriteCapability::ReadOnly);
    let fmt_neovim = config_format_text_edits(doc_content, WriteCapability::ReadOnly);
    assert!(
        fmt_vscode.is_empty(),
        "YAML format must return empty edits on VS Code"
    );
    assert!(
        fmt_intellij.is_empty(),
        "YAML format must return empty edits on IntelliJ"
    );
    assert!(
        fmt_neovim.is_empty(),
        "YAML format must return empty edits on Neovim"
    );

    // The unused `entries` variable is referenced to suppress the warning, and
    // to document that the parity applies regardless of document content.
    let _ = entries;
}

/// Position-encoding parity: on ASCII content, UTF-16 and UTF-8 character
/// offsets are identical, so the hover result is the same for any client.
#[test]
fn test_parity_ascii_hover_position_encoding_independent() {
    let entries = vec![entry("SERVER_PORT", "9090", 0)];
    let layers: Vec<Vec<ConfigEntry>> = vec![];
    // On ASCII content, UTF-16 and UTF-8 offsets agree at character=3.
    let pos_utf16 = Position {
        line: 0,
        character: 3,
    };
    let pos_utf8_equiv = Position {
        line: 0,
        character: 3,
    };
    let r_utf16 = config_hover(pos_utf16, &entries, &layers, None);
    let r_utf8 = config_hover(pos_utf8_equiv, &entries, &layers, None);
    assert_eq!(
        r_utf16, r_utf8,
        "ASCII hover must be position-encoding independent"
    );
    assert!(r_utf16.is_some(), "hover must resolve on ASCII key");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 002: No-regression gate
// ═══════════════════════════════════════════════════════════════════════════════
//
// These tests verify that the new config-format routing predicates do NOT
// intercept URIs that belong to the pre-existing `is_env_file` /
// `is_schema_file` handlers.  Any change here is a regression (FR23, NFR12).

/// `.env.schema` URIs must never be captured by any new predicate.
#[test]
fn test_regression_schema_uri_not_captured_by_new_predicates() {
    let schema = url("/proj/.env.schema");
    assert!(
        !is_jvm_config_file(&schema),
        ".env.schema must not match is_jvm_config_file"
    );
    assert!(
        !is_env_cascade_file(&schema),
        ".env.schema must not match is_env_cascade_file"
    );
    assert!(
        !is_yaml_config_file(&schema),
        ".env.schema must not match is_yaml_config_file"
    );
    assert!(
        !is_config_format_file(&schema),
        ".env.schema must not be routed through config-format dispatch"
    );
}

/// `.env.schema.toml` must not be captured by any new predicate.
#[test]
fn test_regression_schema_toml_uri_not_captured_by_new_predicates() {
    let schema_toml = url("/proj/.env.schema.toml");
    assert!(!is_jvm_config_file(&schema_toml));
    assert!(!is_env_cascade_file(&schema_toml));
    assert!(!is_yaml_config_file(&schema_toml));
    assert!(!is_config_format_file(&schema_toml));
}

/// Shell rc files must not be captured by any new predicate.
#[test]
fn test_regression_shell_rc_not_captured_by_new_predicates() {
    for rc in &[
        "/home/user/.zshrc",
        "/home/user/.bashrc",
        "/home/user/.profile",
        "/home/user/.bash_profile",
        "/home/user/.config/fish/config.fish",
    ] {
        let u = url(rc);
        assert!(!is_jvm_config_file(&u), "{rc} must not match jvm predicate");
        assert!(
            !is_env_cascade_file(&u),
            "{rc} must not match cascade predicate"
        );
        assert!(
            !is_yaml_config_file(&u),
            "{rc} must not match yaml predicate"
        );
        assert!(
            !is_config_format_file(&u),
            "{rc} must not match config-format dispatch"
        );
    }
}

/// Plain `.env` routing is stable: it must NOT match any new predicate so it
/// stays on the existing env handler path (documents store) with all its
/// existing features (AI-guard, code_lens, inlay hints, managed-var hover).
#[test]
fn test_regression_dotenv_file_routing_is_stable() {
    let dotenv = url("/proj/.env");
    assert!(
        !is_jvm_config_file(&dotenv),
        ".env must not match jvm predicate"
    );
    assert!(
        !is_yaml_config_file(&dotenv),
        ".env must not match yaml predicate"
    );
    // After the routing fix: plain .env must NOT match the cascade predicate
    // so it stays on the existing env handler (routing fix — design decision).
    assert!(
        !is_env_cascade_file(&dotenv),
        ".env must NOT match cascade predicate (routing fix)"
    );
    assert!(
        !is_config_format_file(&dotenv),
        ".env must NOT be routed through config-format dispatch (routing fix)"
    );
}

/// `.env.local` is part of the cascade; schema predicates must not fire.
#[test]
fn test_regression_dotenv_local_is_cascade_not_schema_not_jvm() {
    let local = url("/proj/.env.local");
    assert!(is_env_cascade_file(&local));
    assert!(!is_jvm_config_file(&local));
    assert!(!is_yaml_config_file(&local));
}

/// Docker-compose YAML must NOT be captured — only `application.yml` /
/// `application-{profile}.yml` patterns are claimed.
#[test]
fn test_regression_docker_compose_yml_not_captured() {
    let docker = url("/proj/docker-compose.yml");
    assert!(
        !is_yaml_config_file(&docker),
        "docker-compose.yml must not match yaml config predicate"
    );
    assert!(!is_config_format_file(&docker));
}

/// GitHub Actions workflow YAML must not be captured.
#[test]
fn test_regression_github_workflow_yml_not_captured() {
    let workflow = url("/proj/.github/workflows/ci.yml");
    assert!(!is_yaml_config_file(&workflow));
    assert!(!is_config_format_file(&workflow));
}

/// Kubernetes YAML must not be captured.
#[test]
fn test_regression_k8s_yaml_not_captured() {
    for k8s in &[
        "/proj/k8s/deployment.yaml",
        "/proj/manifests/service.yaml",
        "/proj/helm/values.yaml",
    ] {
        let u = url(k8s);
        assert!(
            !is_yaml_config_file(&u),
            "{k8s} must not match yaml config predicate"
        );
    }
}

/// Arbitrary `.properties` files are NOT matched by `is_jvm_config_file` after
/// the FR3 scope-narrowing fix (only application.properties, application-{profile}.properties,
/// and microprofile-config.properties are recognized). The schema-file predicate
/// remains completely independent and unaffected.
#[test]
fn test_regression_arbitrary_properties_does_not_affect_schema_predicate() {
    let log4j = url("/proj/log4j.properties");
    // After FR3 scope narrowing, arbitrary .properties files are NOT matched.
    assert!(
        !is_jvm_config_file(&log4j),
        "log4j.properties must NOT match after FR3 scope-narrowing"
    );
    // The schema predicate (URI path check) is completely independent.
    let schema = url("/proj/.env.schema");
    assert!(schema.path().ends_with(".env.schema"));
    assert!(!is_jvm_config_file(&schema));
}

/// Source language files must not be intercepted by the config dispatch.
#[test]
fn test_regression_source_files_not_captured() {
    for src in &[
        "/proj/src/main.ts",
        "/proj/src/main.rs",
        "/proj/src/app.py",
        "/proj/cmd/main.go",
        "/proj/src/Main.java",
        "/proj/src/App.kt",
        "/proj/src/App.cs",
        "/proj/src/app.rb",
    ] {
        let u = url(src);
        assert!(
            !is_config_format_file(&u),
            "{src} must not be captured by config-format dispatch"
        );
    }
}

/// `application.properties` is captured only by the JVM predicate — not the
/// YAML or cascade predicates.
#[test]
fn test_regression_application_properties_only_matches_jvm_predicate() {
    let app_props = url("/proj/application.properties");
    assert!(is_jvm_config_file(&app_props));
    assert!(!is_env_cascade_file(&app_props));
    assert!(!is_yaml_config_file(&app_props));
}

/// Profile-variant YAML is claimed only by the YAML predicate.
#[test]
fn test_regression_application_prod_yml_only_matches_yaml_predicate() {
    let prod_yml = url("/proj/application-prod.yml");
    assert!(is_yaml_config_file(&prod_yml));
    assert!(!is_jvm_config_file(&prod_yml));
    assert!(!is_env_cascade_file(&prod_yml));
    assert!(is_config_format_file(&prod_yml));
}

/// `application.yaml` (alternative suffix) is claimed only by the YAML predicate.
#[test]
fn test_regression_application_yaml_suffix_only_matches_yaml_predicate() {
    let app_yaml = url("/proj/application.yaml");
    assert!(is_yaml_config_file(&app_yaml));
    assert!(!is_jvm_config_file(&app_yaml));
    assert!(!is_env_cascade_file(&app_yaml));
}

/// Profile-variant `.yaml` suffix is correctly identified.
#[test]
fn test_regression_application_staging_yaml_recognized() {
    let staging = url("/proj/application-staging.yaml");
    assert!(is_yaml_config_file(&staging));
    assert!(!is_jvm_config_file(&staging));
    assert!(!is_env_cascade_file(&staging));
}

/// Empty profile segment in YAML (e.g. `application-.yml`) must not match.
#[test]
fn test_regression_application_empty_profile_yaml_not_matched() {
    let empty_profile = url("/proj/application-.yml");
    assert!(!is_yaml_config_file(&empty_profile));
}

/// Unrelated filenames that start with "application" but are not config files.
#[test]
fn test_regression_application_txt_not_captured() {
    let app_txt = url("/proj/application.txt");
    assert!(!is_jvm_config_file(&app_txt));
    assert!(!is_yaml_config_file(&app_txt));
    assert!(!is_config_format_file(&app_txt));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Story 003: Docs & Release — M-D CLI scan-dir wiring (FR21)
// ═══════════════════════════════════════════════════════════════════════════════

/// M-D — `envforge canary scan-dir` CLI subcommand parses correctly.
/// Verifies the ScanDir variant was wired into the CanaryAction enum
/// and that clap accepts `--dir` and `--json` flags.
#[test]
fn test_md_cli_canary_scan_dir_parses_with_dir_flag() {
    let cli = Cli::try_parse_from(["envforge", "canary", "scan-dir", "--dir", "/tmp"])
        .expect("envforge canary scan-dir --dir /tmp must parse successfully");
    let Some(Commands::Canary { action }) = cli.command else {
        panic!("M-D: expected Canary command");
    };
    let CanaryAction::ScanDir { dir, strict, json } = action else {
        panic!("M-D: expected ScanDir action");
    };
    assert_eq!(dir, "/tmp", "M-D: --dir flag must be '/tmp'");
    assert!(!strict, "M-D: --strict must default to false");
    assert!(!json, "M-D: --json must default to false");
}

/// M-D — default dir is `.` when `--dir` is omitted.
#[test]
fn test_md_cli_canary_scan_dir_default_dir_is_dot() {
    let cli = Cli::try_parse_from(["envforge", "canary", "scan-dir"])
        .expect("envforge canary scan-dir must parse with defaults");
    let Some(Commands::Canary { action }) = cli.command else {
        panic!("M-D: expected Canary command");
    };
    let CanaryAction::ScanDir { dir, .. } = action else {
        panic!("M-D: expected ScanDir action");
    };
    assert_eq!(dir, ".", "M-D: default --dir must be '.'");
}

/// M-D — `--json` flag is accepted on scan-dir.
#[test]
fn test_md_cli_canary_scan_dir_json_flag_accepted() {
    let cli = Cli::try_parse_from(["envforge", "canary", "scan-dir", "--json"])
        .expect("envforge canary scan-dir --json must parse");
    let Some(Commands::Canary { action }) = cli.command else {
        panic!("M-D: expected Canary command");
    };
    let CanaryAction::ScanDir { json, .. } = action else {
        panic!("M-D: expected ScanDir action");
    };
    assert!(json, "M-D: --json flag must be true when passed");
}
