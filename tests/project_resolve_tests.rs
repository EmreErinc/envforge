//! Tests for [`envforge::ops::project::resolve`] — Epic 1: Project Manifest
//! Foundation. Validates resolving a `ProjectConfig`'s declared environments
//! into the concrete recognized env-file set, with root-containment safety.

use std::path::{Path, PathBuf};

use envforge::ops::project::{
    resolve_env_set, AiGuardConfig, ProjectConfig, ProjectEnvironment, ProjectMeta, WizardState,
};

/// Build a `ProjectConfig` with the given (name, env_file) environments and
/// active environment.
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
fn test_resolve_env_set_resolves_all_environments() {
    let root = Path::new("/proj");
    let config = config_with(
        "development",
        &[
            ("base", ".env"),
            ("development", ".env.development"),
            ("stage", ".env.stage"),
            ("production", ".env.production"),
        ],
    );

    let set = resolve_env_set(&config, root);

    assert_eq!(set.root, PathBuf::from("/proj"));
    assert_eq!(set.envs.len(), 4);
    assert!(set.recognizes(Path::new("/proj/.env.development")));
    assert!(set.recognizes(Path::new("/proj/.env.production")));
    assert!(set.recognizes(Path::new("/proj/.env")));
}

#[test]
fn test_resolve_env_set_marks_active_environment() {
    let root = Path::new("/proj");
    let config = config_with(
        "production",
        &[
            ("development", ".env.development"),
            ("production", ".env.production"),
        ],
    );

    let set = resolve_env_set(&config, root);

    let active: Vec<&str> = set
        .envs
        .iter()
        .filter(|e| e.is_active)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(active, vec!["production"]);
}

#[test]
fn test_resolve_env_set_resolves_paths_relative_to_root() {
    let root = Path::new("/home/me/svc");
    let config = config_with("dev", &[("dev", "config/.env.dev")]);

    let set = resolve_env_set(&config, root);

    assert_eq!(
        set.envs[0].path,
        PathBuf::from("/home/me/svc/config/.env.dev")
    );
    assert!(set.recognizes(Path::new("/home/me/svc/config/.env.dev")));
}

#[test]
fn test_resolve_env_set_drops_parent_traversal_escape() {
    let root = Path::new("/proj");
    let config = config_with("dev", &[("dev", "../secrets/.env")]);

    let set = resolve_env_set(&config, root);

    // Path escapes the project root — must be dropped, never recognized.
    assert!(set.is_empty());
    assert!(!set.recognizes(Path::new("/secrets/.env")));
}

#[test]
fn test_resolve_env_set_drops_absolute_env_file() {
    let root = Path::new("/proj");
    let config = config_with("dev", &[("dev", "/etc/passwd")]);

    let set = resolve_env_set(&config, root);

    assert!(set.is_empty());
    assert!(!set.recognizes(Path::new("/etc/passwd")));
}

#[test]
fn test_resolve_env_set_keeps_inner_dotdot_within_root() {
    let root = Path::new("/proj");
    // Normalizes to /proj/.env — stays within root, so it is kept.
    let config = config_with("dev", &[("dev", "sub/../.env")]);

    let set = resolve_env_set(&config, root);

    assert_eq!(set.envs.len(), 1);
    assert!(set.recognizes(Path::new("/proj/.env")));
}

#[test]
fn test_resolve_env_set_dedups_by_resolved_path() {
    let root = Path::new("/proj");
    let config = config_with("a", &[("a", ".env"), ("b", "./.env")]);

    let set = resolve_env_set(&config, root);

    // Both resolve to /proj/.env — collapsed to one, first declaration wins.
    assert_eq!(set.envs.len(), 1);
    assert_eq!(set.envs[0].name, "a");
}

#[test]
fn test_resolve_env_set_empty_when_no_environments() {
    let root = Path::new("/proj");
    let config = config_with("dev", &[]);

    let set = resolve_env_set(&config, root);

    assert!(set.is_empty());
    assert!(!set.recognizes(Path::new("/proj/.env")));
}

#[test]
fn test_env_name_for_maps_file_to_environment() {
    let root = Path::new("/proj");
    let config = config_with(
        "dev",
        &[("dev", ".env.development"), ("prod", ".env.production")],
    );

    let set = resolve_env_set(&config, root);

    assert_eq!(
        set.env_name_for(Path::new("/proj/.env.development")),
        Some("dev")
    );
    assert_eq!(
        set.env_name_for(Path::new("/proj/.env.production")),
        Some("prod")
    );
    assert_eq!(set.env_name_for(Path::new("/proj/.env.other")), None);
}

#[test]
fn test_recognizes_normalizes_query_path() {
    let root = Path::new("/proj");
    let config = config_with("dev", &[("dev", ".env.dev")]);

    let set = resolve_env_set(&config, root);

    // Query path with a redundant `.` component still matches.
    assert!(set.recognizes(Path::new("/proj/./.env.dev")));
    assert!(!set.recognizes(Path::new("/proj/.env.other")));
}
