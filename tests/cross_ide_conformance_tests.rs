//! Cross-client conformance (Epic 6 / FR22-24). All env intelligence is
//! computed server-side and is deterministic, so every client that attaches to
//! the resolved file set renders identical results. These tests pin the
//! server-side guarantees the four first-party clients rely on:
//!
//! - one manifest resolves to the same env-file set every time (determinism),
//! - recognition covers declared **non-`.env*`** names (so a client that
//!   attaches by filename gets features there too),
//! - the unified key-set is deterministically ordered (same input ⇒ same
//!   output, FR24 / NFR11).

use std::path::{Path, PathBuf};

use envforge::ops::env_keyset::build_env_keyset_from_sources;
use envforge::ops::project::{
    resolve_env_set, AiGuardConfig, ProjectConfig, ProjectEnvironment, ProjectMeta, WizardState,
};

fn config_with(active: &str, envs: &[(&str, &str)]) -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            name: "demo".to_string(),
            schema_path: PathBuf::from(".env.schema"),
            active_environment: active.to_string(),
        },
        wizard: WizardState::default(),
        environments: envs
            .iter()
            .map(|(name, file)| ProjectEnvironment {
                name: name.to_string(),
                env_file: PathBuf::from(file),
                description: None,
            })
            .collect(),
        ai_guard: AiGuardConfig::default(),
    }
}

#[test]
fn test_recognition_covers_non_conventional_declared_file() {
    // A custom filename that does NOT match the conventional `.env*` rule.
    let root = Path::new("/proj");
    let config = config_with("dev", &[("dev", "config/settings.dev")]);

    let set = resolve_env_set(&config, root);

    // Server-side recognition covers it — any client attaching to this path
    // gets the same features. (Client-side attach for such names is a
    // documented Growth limitation; the server guarantee is unconditional.)
    assert!(set.recognizes(Path::new("/proj/config/settings.dev")));
}

#[test]
fn test_resolution_is_deterministic() {
    let root = Path::new("/proj");
    let config = config_with(
        "dev",
        &[
            ("dev", ".env.development"),
            ("stage", ".env.stage"),
            ("prod", ".env.production"),
        ],
    );

    let a = resolve_env_set(&config, root);
    let b = resolve_env_set(&config, root);

    // Same manifest ⇒ byte-identical resolved set, so every client agrees.
    assert_eq!(a, b);
}

#[test]
fn test_keyset_ordering_is_deterministic() {
    // Keys declared in a non-sorted order across files must surface in a
    // stable, sorted order regardless of input order (BTreeMap, NFR11).
    let sources_a: Vec<(&str, &Path, &str)> = vec![
        ("dev", Path::new("/p/.env.dev"), "ZED=1\nALPHA=2\n"),
        ("prod", Path::new("/p/.env.prod"), "MIKE=3\n"),
    ];
    let sources_b: Vec<(&str, &Path, &str)> = vec![
        ("prod", Path::new("/p/.env.prod"), "MIKE=3\n"),
        ("dev", Path::new("/p/.env.dev"), "ALPHA=2\nZED=1\n"),
    ];

    let ka = build_env_keyset_from_sources(&sources_a);
    let kb = build_env_keyset_from_sources(&sources_b);

    let keys_a: Vec<&str> = ka.key_names().collect();
    let keys_b: Vec<&str> = kb.key_names().collect();

    assert_eq!(keys_a, vec!["ALPHA", "MIKE", "ZED"]);
    assert_eq!(
        keys_a, keys_b,
        "key ordering must be input-order independent"
    );
}
