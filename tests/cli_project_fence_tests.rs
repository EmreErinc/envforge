//! CLI integration tests for `envforge project init` AI-fence integration.
//!
//! Runs the compiled binary with an isolated HOME so global config never
//! interferes.  All runs are non-interactive (no TTY attached to stdin).

use std::path::PathBuf;
use std::process::Command;

fn envforge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_envforge"))
}

/// Run envforge in `cwd` with an isolated HOME and the given args.
fn cmd(home: &std::path::Path, cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(envforge_bin())
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("spawn envforge")
}

// ─── --fence-targets tests ─────────────────────────────────────────────────

/// `project init --fence-targets cursor_ignore,copilot` writes `.cursorignore`
/// and `.github/copilot-instructions.md` but NOT a Windsurf or Cline file.
#[test]
fn test_project_init_fence_targets_writes_chosen_files() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let out = cmd(
        home.path(),
        cwd.path(),
        &[
            "project",
            "init",
            "--fence-targets",
            "cursor_ignore,copilot",
        ],
    );
    assert!(
        out.status.success(),
        "exit non-zero: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Chosen targets written
    assert!(
        cwd.path().join(".cursorignore").exists(),
        ".cursorignore must be written"
    );
    assert!(
        cwd.path().join(".github/copilot-instructions.md").exists(),
        "copilot-instructions.md must be written"
    );

    // Non-chosen target NOT written (windsurf: .codeiumignore)
    assert!(
        !cwd.path().join(".codeiumignore").exists(),
        ".codeiumignore must NOT be written when windsurf not selected"
    );
    // Non-chosen target NOT written (cline: .clineignore)
    assert!(
        !cwd.path().join(".clineignore").exists(),
        ".clineignore must NOT be written when cline not selected"
    );
}

// ─── --no-fence tests ──────────────────────────────────────────────────────

/// `project init --no-fence` writes no fence files at all.
#[test]
fn test_project_init_no_fence_writes_no_fence_files() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let out = cmd(home.path(), cwd.path(), &["project", "init", "--no-fence"]);
    assert!(
        out.status.success(),
        "exit non-zero: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // None of the canonical fence files should exist
    let fence_files = [
        ".envforgeignore",
        ".cursorignore",
        ".cursorrules",
        ".github/copilot-instructions.md",
        ".claude/settings.json",
        ".codeiumignore",
        ".clineignore",
        ".aiderignore",
        ".geminiignore",
        "GEMINI.md",
        "AGENTS.md",
        ".amazonq/rules/envforge.md",
    ];
    for file in &fence_files {
        assert!(
            !cwd.path().join(file).exists(),
            "--no-fence: {} must not exist",
            file
        );
    }
}

// ─── unknown id rejects with non-zero exit ─────────────────────────────────

/// `project init --fence-targets bogus_id` exits non-zero and prints an error.
#[test]
fn test_project_init_fence_targets_bogus_id_exits_nonzero() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let out = cmd(
        home.path(),
        cwd.path(),
        &["project", "init", "--fence-targets", "bogus_id"],
    );
    assert!(
        !out.status.success(),
        "bogus fence target id must cause non-zero exit"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bogus_id") || stderr.contains("unknown"),
        "stderr must mention the bad id; got: {}",
        stderr
    );
}

// ─── --fence (non-tty) uses detected set ──────────────────────────────────

/// `project init --fence` in a non-tty context uses detected set.
/// In an empty temp dir, only targets with empty detection hints (envforgeignore)
/// plus any present hints are included.  We assert no error and at least the
/// envforgeignore-always-applicable target is written.
#[test]
fn test_project_init_fence_flag_non_tty_uses_detected_set() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let out = cmd(
        home.path(),
        cwd.path(),
        &["project", "init", "--fence", "--non-interactive"],
    );
    assert!(
        out.status.success(),
        "exit non-zero: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The envforgeignore target (empty detection hints = always applicable)
    // must always be written.
    assert!(
        cwd.path().join(".envforgeignore").exists(),
        ".envforgeignore must be written (always-applicable target)"
    );
}

// ─── Unit tests for detect_installed_targets ───────────────────────────────

#[cfg(test)]
mod unit {
    use envforge::ops::fence::{detect_installed_targets, FenceTarget};

    /// A `.cursor/` dir causes cursor_ignore and cursor_rules to be detected.
    /// Absence of `.claude` means claude_code is NOT detected.
    /// envforgeignore is always detected (empty hints).
    #[test]
    fn test_detect_installed_targets_cursor_dir_detected() {
        let tmp = tempfile::tempdir().unwrap();

        // Create .cursor dir to trigger cursor detection hints
        std::fs::create_dir_all(tmp.path().join(".cursor")).unwrap();

        let detected = detect_installed_targets(tmp.path());

        assert!(
            detected.contains(&FenceTarget::Envforgeignore),
            "envforgeignore must always be detected"
        );
        assert!(
            detected.contains(&FenceTarget::CursorIgnore),
            "cursor_ignore detected via .cursor dir"
        );
        // .claude absent → claude_code NOT detected
        assert!(
            !detected.contains(&FenceTarget::ClaudeCode),
            "claude_code must not be detected when .claude is absent"
        );
    }

    /// Without any tool dirs, only the empty-hints (always-applicable) targets appear.
    #[test]
    fn test_detect_installed_targets_empty_dir_only_always_present() {
        let tmp = tempfile::tempdir().unwrap();
        let detected = detect_installed_targets(tmp.path());

        // Only targets with empty detection hints should appear
        use envforge::ops::fence::registry;
        let always_targets: Vec<FenceTarget> = registry::REGISTRY
            .iter()
            .filter(|s| s.detection.is_empty())
            .map(|s| s.target)
            .collect();

        for t in &always_targets {
            assert!(
                detected.contains(t),
                "always-applicable target {:?} must be detected in empty dir",
                t
            );
        }

        // All returned targets must be always-applicable (no tool hints satisfied)
        for t in &detected {
            assert!(
                always_targets.contains(t),
                "unexpected target {:?} detected in empty dir",
                t
            );
        }
    }

    /// `parse_fence_target_ids` equivalent: unknown ids must cause errors in CLI.
    /// Tested indirectly via the integration test above, but also verify the
    /// registry is_valid_id rejects unknown strings correctly.
    #[test]
    fn test_registry_is_valid_id_rejects_unknown() {
        use envforge::ops::fence::registry;
        assert!(!registry::is_valid_id("bogus_id"));
        assert!(!registry::is_valid_id("cursor")); // short name, not the id
        assert!(!registry::is_valid_id(""));
    }

    /// is_valid_id accepts all known registry ids.
    #[test]
    fn test_registry_is_valid_id_accepts_known() {
        use envforge::ops::fence::registry;
        for spec in registry::REGISTRY {
            assert!(
                registry::is_valid_id(spec.id),
                "is_valid_id must accept known id '{}'",
                spec.id
            );
        }
    }
}
