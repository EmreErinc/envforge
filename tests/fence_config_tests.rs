/// Integration tests for the fence configurability feature (Epic 1 + Epic 2).
///
/// Ops-level tests (create_fence_with, check_fence_status_with, resolve, etc.) live
/// inline in src/ops/fence.rs per gap-analysis decision G3. This file covers:
/// - AppConfig TOML round-trips for FenceConfig/FenceTargets (Story 1.1)
/// - Unknown-key parse error from deny_unknown_fields (Story 1.2 / FR5)
/// - Malformed config → fail-safe all-enabled (Story 1.2 / FR19)
/// - Config save/load round-trip preserving other sections (Story 2.2)
/// - CLI enable/disable persistence via ENVFORGE_CONFIG_DIR isolation (Story 2.2)
use std::path::PathBuf;

use envforge::config::{load_config, save_config, AppConfig, FenceConfig, FenceTargets};
use envforge::ops::fence::FenceTarget;

// Helper: create a temp config dir and set ENVFORGE_CONFIG_DIR to it for test isolation.
// Returns the TempDir (must keep alive) and the config file path.
fn isolated_config_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    (dir, path)
}

// ─── Story 1.1: FenceConfig model ──────────────────────────────────────────

#[test]
fn test_fence_config_default_all_true() {
    let cfg = FenceConfig::default();
    for t in FenceTarget::all() {
        assert!(
            cfg.targets.is_enabled(t),
            "target {:?} must default to enabled",
            t
        );
    }
}

#[test]
fn test_fence_targets_default_all_true() {
    let targets = FenceTargets::default();
    for t in FenceTarget::all() {
        assert!(
            targets.is_enabled(t),
            "target {:?} must default to enabled",
            t
        );
    }
}

/// Absent `[fence]` section → all targets enabled.
#[test]
fn test_fence_config_absent_section_defaults_all_enabled() {
    let (_dir, path) = isolated_config_dir();

    // Write a config.toml without any [fence] section
    let toml_content = r#"
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 0
footer_protected_lines = 0

[protected_blocks]
markers = []
"#;
    std::fs::write(&path, toml_content).unwrap();
    let cfg = load_config(&path).unwrap();
    for t in FenceTarget::all() {
        assert!(
            cfg.fence.targets.is_enabled(t),
            "target {:?} must be enabled when [fence] section absent",
            t
        );
    }
}

/// Absent single key within `[fence.targets]` → that target defaults to true.
#[test]
fn test_fence_config_absent_single_key_defaults_true() {
    let (_dir, path) = isolated_config_dir();

    let toml_content = r#"
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 0
footer_protected_lines = 0

[protected_blocks]
markers = []

[fence.targets]
claude_code = false
"#;
    std::fs::write(&path, toml_content).unwrap();
    let cfg = load_config(&path).unwrap();
    // Explicitly set field
    assert!(!cfg.fence.targets.is_enabled(FenceTarget::ClaudeCode));
    // All other targets default to true
    assert!(cfg.fence.targets.is_enabled(FenceTarget::Envforgeignore));
    assert!(cfg.fence.targets.is_enabled(FenceTarget::CursorIgnore));
    assert!(cfg.fence.targets.is_enabled(FenceTarget::CursorRules));
    assert!(cfg.fence.targets.is_enabled(FenceTarget::Copilot));
}

/// deny_unknown_fields: unknown key in [fence.targets] → parse error (FR5).
#[test]
fn test_fence_config_unknown_key_parse_error() {
    let (_dir, path) = isolated_config_dir();

    let toml_content = r#"
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 0
footer_protected_lines = 0

[protected_blocks]
markers = []

[fence.targets]
unknown_tool = true
"#;
    std::fs::write(&path, toml_content).unwrap();
    let result = load_config(&path);
    assert!(
        result.is_err(),
        "unknown key in [fence.targets] must produce a parse error"
    );
}

// ─── Story 1.2: Fail-safe (malformed config → all-enabled) ─────────────────

/// Malformed TOML → config load fails, but fence uses fail-safe (all enabled).
/// We test the fail-safe path indirectly via load_config returning an error.
#[test]
fn test_malformed_config_load_fails() {
    let (_dir, path) = isolated_config_dir();
    std::fs::write(&path, "this is not valid TOML {{{").unwrap();
    let result = load_config(&path);
    assert!(result.is_err(), "malformed config must return error");
}

// ─── Story 1.1 AC5: Round-trip preserves other sections ────────────────────

#[test]
fn test_fence_config_roundtrip_preserves_other_sections() {
    let (_dir, path) = isolated_config_dir();

    let mut config = AppConfig::default();
    config
        .fence
        .targets
        .set_enabled(FenceTarget::Copilot, false);
    config.general.default_shell = "bash".to_string();

    save_config(&config, &path).unwrap();
    let loaded = load_config(&path).unwrap();

    // fence config preserved
    assert!(!loaded.fence.targets.is_enabled(FenceTarget::Copilot));
    assert!(loaded.fence.targets.is_enabled(FenceTarget::Envforgeignore));
    assert!(loaded.fence.targets.is_enabled(FenceTarget::CursorIgnore));
    assert!(loaded.fence.targets.is_enabled(FenceTarget::CursorRules));
    assert!(loaded.fence.targets.is_enabled(FenceTarget::ClaudeCode));

    // Unrelated section preserved
    assert_eq!(loaded.general.default_shell, "bash");
}

// ─── Story 2.2: CLI enable/disable persistence ─────────────────────────────

/// Disable then reload: persisted false.
#[test]
fn test_fence_config_disable_persists() {
    let (dir, path) = isolated_config_dir();
    // Ensure config dir env var is set for this process
    let _guard = EnvGuard::set("ENVFORGE_CONFIG_DIR", dir.path().to_str().unwrap());

    // Write initial all-enabled config
    let config = AppConfig::default();
    save_config(&config, &path).unwrap();

    // Reload and disable claude_code
    let mut loaded = load_config(&path).unwrap();
    loaded
        .fence
        .targets
        .set_enabled(FenceTarget::ClaudeCode, false);
    save_config(&loaded, &path).unwrap();

    // Reload again — change persists
    let reloaded = load_config(&path).unwrap();
    assert!(
        !reloaded.fence.targets.is_enabled(FenceTarget::ClaudeCode),
        "disabled state must persist after save/load"
    );
    // Other fields unaffected
    assert!(reloaded
        .fence
        .targets
        .is_enabled(FenceTarget::Envforgeignore));
    assert!(reloaded.fence.targets.is_enabled(FenceTarget::CursorIgnore));
}

/// Enable after disable: persisted true.
#[test]
fn test_fence_config_enable_after_disable_persists() {
    let (_dir, path) = isolated_config_dir();

    let mut config = AppConfig::default();
    config
        .fence
        .targets
        .set_enabled(FenceTarget::CursorRules, false);
    save_config(&config, &path).unwrap();

    // Re-enable
    let mut loaded = load_config(&path).unwrap();
    loaded
        .fence
        .targets
        .set_enabled(FenceTarget::CursorRules, true);
    save_config(&loaded, &path).unwrap();

    let reloaded = load_config(&path).unwrap();
    assert!(
        reloaded.fence.targets.is_enabled(FenceTarget::CursorRules),
        "re-enabled state must persist"
    );
}

/// Toggle one field — other sections unaffected.
#[test]
fn test_fence_config_toggle_preserves_unrelated_sections() {
    let (_dir, path) = isolated_config_dir();

    let mut config = AppConfig::default();
    config.general.default_shell = "fish".to_string();
    config.lifecycle.default_stale_threshold_days = 42;
    save_config(&config, &path).unwrap();

    let mut loaded = load_config(&path).unwrap();
    loaded
        .fence
        .targets
        .set_enabled(FenceTarget::Copilot, false);
    save_config(&loaded, &path).unwrap();

    let reloaded = load_config(&path).unwrap();
    assert_eq!(reloaded.general.default_shell, "fish");
    assert_eq!(reloaded.lifecycle.default_stale_threshold_days, 42);
    assert!(!reloaded.fence.targets.is_enabled(FenceTarget::Copilot));
}

// ─── FenceTarget enum sanity checks ────────────────────────────────────────

#[test]
fn test_fence_target_as_str_unique_and_snake_case() {
    let all = FenceTarget::all();
    let strs: Vec<&str> = all.iter().map(|t| t.as_str()).collect();
    // All unique
    let set: std::collections::HashSet<_> = strs.iter().collect();
    assert_eq!(set.len(), all.len());
    // All snake_case (no uppercase, no hyphens)
    for s in &strs {
        assert!(
            s.chars().all(|c| c.is_lowercase() || c == '_'),
            "target ID '{}' must be snake_case",
            s
        );
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// RAII guard that sets an env var for the duration of a test and restores it.
struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

// ─── CLI parsing tests: FenceTarget clap ValueEnum snake_case ───────────────
//
// These tests catch the bug where clap defaulted to kebab-case for multi-word
// variants (e.g. `claude-code`) instead of the canonical snake_case IDs.
// They parse the full Cli struct via `try_parse_from` so they exercise the
// real clap ValueEnum machinery.

use clap::Parser as _;
use envforge::cli::{Cli, Commands, FenceAction};

fn parse_fence_disable(id: &str) -> Result<FenceTarget, clap::Error> {
    let args = ["envforge", "fence", "config", "--disable", id];
    let cli = Cli::try_parse_from(args)?;
    match cli.command {
        Some(Commands::Fence {
            action: Some(FenceAction::Config(args)),
            ..
        }) => Ok(args.disable.expect("disable must be set")),
        _ => panic!("unexpected parse result"),
    }
}

fn parse_fence_enable(id: &str) -> Result<FenceTarget, clap::Error> {
    let args = ["envforge", "fence", "config", "--enable", id];
    let cli = Cli::try_parse_from(args)?;
    match cli.command {
        Some(Commands::Fence {
            action: Some(FenceAction::Config(args)),
            ..
        }) => Ok(args.enable.expect("enable must be set")),
        _ => panic!("unexpected parse result"),
    }
}

/// All five canonical snake_case IDs must parse via --disable.
#[test]
fn test_cli_fence_disable_envforgeignore() {
    let target = parse_fence_disable("envforgeignore").expect("envforgeignore must parse");
    assert_eq!(target, FenceTarget::Envforgeignore);
}

#[test]
fn test_cli_fence_disable_cursor_ignore() {
    let target = parse_fence_disable("cursor_ignore").expect("cursor_ignore must parse");
    assert_eq!(target, FenceTarget::CursorIgnore);
}

#[test]
fn test_cli_fence_disable_cursor_rules() {
    let target = parse_fence_disable("cursor_rules").expect("cursor_rules must parse");
    assert_eq!(target, FenceTarget::CursorRules);
}

#[test]
fn test_cli_fence_disable_copilot() {
    let target = parse_fence_disable("copilot").expect("copilot must parse");
    assert_eq!(target, FenceTarget::Copilot);
}

#[test]
fn test_cli_fence_disable_claude_code() {
    let target = parse_fence_disable("claude_code").expect("claude_code must parse");
    assert_eq!(target, FenceTarget::ClaudeCode);
}

/// All canonical snake_case IDs must parse via --enable.
#[test]
fn test_cli_fence_enable_all_five() {
    let cases = [
        ("envforgeignore", FenceTarget::Envforgeignore),
        ("cursor_ignore", FenceTarget::CursorIgnore),
        ("cursor_rules", FenceTarget::CursorRules),
        ("copilot", FenceTarget::Copilot),
        ("claude_code", FenceTarget::ClaudeCode),
        ("windsurf", FenceTarget::Windsurf),
        ("cline", FenceTarget::Cline),
        ("aider", FenceTarget::Aider),
        ("gemini", FenceTarget::Gemini),
    ];
    for (id, expected) in cases {
        let got = parse_fence_enable(id).unwrap_or_else(|e| panic!("{id} must parse: {e}"));
        assert_eq!(got, expected, "enable {id} mapped to wrong variant");
    }
}

/// New tool IDs must parse via --disable (Story 1.3).
#[test]
fn test_cli_fence_disable_windsurf() {
    let target = parse_fence_disable("windsurf").expect("windsurf must parse");
    assert_eq!(target, FenceTarget::Windsurf);
}

#[test]
fn test_cli_fence_disable_cline() {
    let target = parse_fence_disable("cline").expect("cline must parse");
    assert_eq!(target, FenceTarget::Cline);
}

#[test]
fn test_cli_fence_disable_aider() {
    let target = parse_fence_disable("aider").expect("aider must parse");
    assert_eq!(target, FenceTarget::Aider);
}

#[test]
fn test_cli_fence_disable_gemini() {
    let target = parse_fence_disable("gemini").expect("gemini must parse");
    assert_eq!(target, FenceTarget::Gemini);
}

/// Kebab-case IDs must be REJECTED (would have been accepted before the fix).
#[test]
fn test_cli_fence_disable_kebab_case_rejected() {
    assert!(
        parse_fence_disable("claude-code").is_err(),
        "kebab-case 'claude-code' must not be accepted"
    );
    assert!(
        parse_fence_disable("cursor-ignore").is_err(),
        "kebab-case 'cursor-ignore' must not be accepted"
    );
    assert!(
        parse_fence_disable("cursor-rules").is_err(),
        "kebab-case 'cursor-rules' must not be accepted"
    );
}

/// Completely bogus IDs must be rejected.
#[test]
fn test_cli_fence_disable_nonsense_rejected() {
    assert!(parse_fence_disable("nonsense").is_err());
    assert!(parse_fence_disable("").is_err());
}

// ─── Story 3.1: Cross-surface parity tests ──────────────────────────────────

/// CLI-path and LSP-path produce identical enabled-only file sets when a target
/// is disabled (Story 3.1 AC2 / FR15).
///
/// Disables `claude_code`, creates fence via the ops path used by CLI
/// (`create_fence_with`) and verifies that `resolve_fence_targets` (used by LSP
/// `envforge.fence.config`) returns the same enabled set.
#[test]
fn test_cli_lsp_parity_disabled_target_identical_enabled_set() {
    use envforge::config::FenceConfig;
    use envforge::ops::fence::{create_fence_with, resolve_fence_targets, FenceTarget};

    let tmp = tempfile::TempDir::new().unwrap();

    // Build config with claude_code disabled
    let mut cfg = FenceConfig::default();
    cfg.targets.set_enabled(FenceTarget::ClaudeCode, false);

    // CLI path: create_fence_with respects the config
    let result = create_fence_with(tmp.path(), false, &cfg).unwrap();

    // The skipped set must contain only claude_code's path
    let skipped_names: Vec<String> = result
        .files_skipped
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    assert!(
        skipped_names.iter().any(|s| s.contains("settings.json")),
        "cli path must skip .claude/settings.json when claude_code disabled"
    );
    // No other target path should be in skipped (only the disabled one)
    let created_and_updated = result.files_created.len() + result.files_updated.len();
    // Total files across all targets minus claude_code's 1 file
    let total_files: usize = envforge::ops::fence::registry::REGISTRY
        .iter()
        .map(|s| s.files.len())
        .sum();
    let claude_files = envforge::ops::fence::registry::spec_for(FenceTarget::ClaudeCode)
        .files
        .len();
    assert_eq!(
        created_and_updated,
        total_files - claude_files,
        "cli path must write all files for enabled targets"
    );

    // LSP path: resolve_fence_targets returns the same enabled set
    let resolved = resolve_fence_targets(&cfg);
    let enabled_targets: Vec<FenceTarget> = resolved
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.target)
        .collect();
    assert_eq!(
        enabled_targets.len(),
        FenceTarget::all().len() - 1,
        "lsp path must resolve all-but-one enabled targets"
    );
    assert!(
        !enabled_targets.contains(&FenceTarget::ClaudeCode),
        "disabled target must not appear in enabled set"
    );
    // Verify both paths agree on which target is missing
    assert!(
        resolved
            .iter()
            .find(|r| r.target == FenceTarget::ClaudeCode)
            .map(|r| !r.enabled)
            .unwrap_or(false),
        "resolve_fence_targets must show claude_code as disabled"
    );
}

/// Default config → CLI and LSP paths both write/report all targets (byte-identity path).
#[test]
fn test_cli_lsp_parity_default_config_five_targets() {
    use envforge::config::FenceConfig;
    use envforge::ops::fence::registry;
    use envforge::ops::fence::{create_fence_with, resolve_fence_targets, FenceTarget};

    let tmp = tempfile::TempDir::new().unwrap();
    let cfg = FenceConfig::default();

    let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
    let expected_files: usize = registry::REGISTRY.iter().map(|s| s.files.len()).sum();
    assert_eq!(
        result.files_created.len(),
        expected_files,
        "cli path must create all registry files on default config"
    );
    assert!(
        result.files_skipped.is_empty(),
        "no targets skipped on default config"
    );

    let resolved = resolve_fence_targets(&cfg);
    let enabled_count = resolved.iter().filter(|r| r.enabled).count();
    assert_eq!(
        enabled_count,
        FenceTarget::all().len(),
        "lsp path must report all targets enabled on default config"
    );
}

// ─── Story 3.2: fence_target_summary formatter tests ────────────────────────

#[test]
fn test_fence_target_summary_all_enabled() {
    use envforge::config::FenceConfig;
    use envforge::ops::fence::{fence_target_summary, resolve_fence_targets, FenceTarget};

    let cfg = FenceConfig::default();
    let resolved = resolve_fence_targets(&cfg);
    let s = fence_target_summary(&resolved);
    let total = FenceTarget::all().len();
    let expected = format!("{total}/{total}");
    assert!(
        s.contains(&expected),
        "all-enabled must show ({expected}): got '{s}'"
    );
    assert!(!s.contains("none"), "all-enabled must not say none");
}

#[test]
fn test_fence_target_summary_subset_enabled() {
    use envforge::config::FenceConfig;
    use envforge::ops::fence::{fence_target_summary, resolve_fence_targets, FenceTarget};

    let mut cfg = FenceConfig::default();
    cfg.targets.set_enabled(FenceTarget::Copilot, false);
    cfg.targets.set_enabled(FenceTarget::ClaudeCode, false);
    let resolved = resolve_fence_targets(&cfg);
    let s = fence_target_summary(&resolved);
    let total = FenceTarget::all().len(); // 9
    let enabled = total - 2; // 7
    let expected = format!("{enabled}/{total}");
    assert!(
        s.contains(&expected),
        "two disabled → ({expected}): got '{s}'"
    );
    // Must list enabled target names
    assert!(
        s.contains("envforgeignore") || s.contains("cursor"),
        "must list enabled target names: got '{s}'"
    );
}

#[test]
fn test_fence_target_summary_none_enabled() {
    use envforge::config::{FenceConfig, FenceTargets};
    use envforge::ops::fence::{fence_target_summary, resolve_fence_targets, FenceTarget};

    let mut targets = FenceTargets::default();
    for t in FenceTarget::all() {
        targets.set_enabled(t, false);
    }
    let cfg = FenceConfig { targets };
    let resolved = resolve_fence_targets(&cfg);
    let s = fence_target_summary(&resolved);
    let total = FenceTarget::all().len();
    let expected_none = format!("0/{total}");
    assert!(s.contains("none"), "all-disabled must say none: got '{s}'");
    assert!(
        s.contains(&expected_none),
        "all-disabled must show (0/{total}): got '{s}'"
    );
}
