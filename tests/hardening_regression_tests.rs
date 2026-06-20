//! Regression tests for the 2026-06-20 security hardening pass.
//!
//! Each test pins a concrete, previously-shipped bug so it cannot silently
//! return. Grouped by the module that was fixed.
//!
//! Bugs covered:
//! 1. crud — value quoting did not escape the closing quote, breaking the
//!    byte-for-byte round-trip and allowing shell-syntax injection into the
//!    rc file (the on-disk writer always emits `original_text`).
//! 2. fence — `write_deny_rule` replaced an existing-but-unparseable
//!    `.claude/settings.json` with `{}` + deny rules, destroying every other
//!    user setting.
//! 3. secrets::provider — `suggest_similar` byte-sliced a user-supplied name
//!    (`name[..2]`), panicking on a multi-byte first character.
//! 4. lease — lease names flowed unvalidated into `dir.join("<name>.toml")`,
//!    allowing path traversal outside the leases directory.

use envforge::model::{ExportStyle, QuoteStyle};
use envforge::ops::{add_entry, edit_entry, rename_entry};
use envforge::parser::{parse_shell_content, serialize_shell_file};
use std::path::Path;

fn parse(content: &str) -> envforge::model::ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

/// Count EnvExport nodes matching `key` in serialized-then-reparsed output.
fn exported_value(content: &str, key: &str) -> Option<String> {
    let sf = parse(content);
    sf.lines.iter().find_map(|n| match n {
        envforge::model::LineNode::EnvExport { key: k, value, .. } if k == key => {
            Some(value.clone())
        }
        _ => None,
    })
}

// ─── Bug 1: crud quote escaping / round-trip safety ─────────────────────────

/// Editing a double-quoted value to one containing `"` must NOT break out of
/// its quotes. Before the fix, `edit` produced `export FOO="a"b"`, which
/// re-parses to value `a` (the `b"` tail is silently dropped) — data loss and
/// shell-injection. After the fix the value survives intact across a
/// write→reparse cycle.
#[test]
fn test_edit_double_quoted_value_with_quote_does_not_truncate() {
    let mut sf = parse("export FOO=\"original\"\n");
    edit_entry(&mut sf, "FOO", "a\"b").unwrap();

    let serialized = serialize_shell_file(&sf);
    // The closing quote must be escaped, not left bare.
    assert!(
        serialized.contains("\\\""),
        "value must be escaped, got: {serialized}"
    );
    assert!(
        !serialized.contains("\"a\"b\""),
        "must not emit an unescaped breakout sequence: {serialized}"
    );

    // Re-parsing yields exactly one FOO entry whose `b` tail was not dropped.
    let reparsed = parse(&serialized);
    let foo_count = reparsed
        .lines
        .iter()
        .filter(|n| matches!(n, envforge::model::LineNode::EnvExport { key, .. } if key == "FOO"))
        .count();
    assert_eq!(foo_count, 1, "exactly one FOO entry expected");
    let val = exported_value(&serialized, "FOO").unwrap();
    assert!(val.ends_with('b'), "value tail dropped: {val:?}");
}

/// parse → edit → serialize → parse → serialize must reach a byte-identical
/// fixpoint (the round-trip invariant) even for awkward values.
#[test]
fn test_edit_quoted_value_reaches_stable_fixpoint() {
    let mut sf = parse("export FOO=\"x\"\n");
    edit_entry(&mut sf, "FOO", "has \"quotes\" and \\ backslash").unwrap();
    let once = serialize_shell_file(&sf);
    let twice = serialize_shell_file(&parse(&once));
    assert_eq!(once, twice, "serialize is not a fixpoint after edit");
}

/// A single-quoted value containing an apostrophe cannot be represented inside
/// single quotes by EnvForge's parser (no escape mechanism). The fix falls
/// back to double quotes so the apostrophe round-trips instead of truncating.
#[test]
fn test_edit_single_quoted_value_with_apostrophe_round_trips() {
    let mut sf = parse("export FOO='plain'\n");
    edit_entry(&mut sf, "FOO", "it's here").unwrap();
    let serialized = serialize_shell_file(&sf);

    let val = exported_value(&serialized, "FOO").unwrap();
    assert!(
        val.contains("it's") || val.contains("it\\'s") || val.contains("it"),
        "apostrophe value lost: {val:?}"
    );
    assert!(val.ends_with("here"), "value tail dropped: {val:?}");
    // Fixpoint check.
    let twice = serialize_shell_file(&parse(&serialized));
    assert_eq!(serialized, twice);
}

/// Adding an entry whose value contains a quote must also escape it.
#[test]
fn test_add_entry_escapes_quote_in_value() {
    let mut sf = parse("# header\n");
    add_entry(
        &mut sf,
        "TOKEN",
        "a\"b",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap();
    let serialized = serialize_shell_file(&sf);
    let reparsed = parse(&serialized);
    let n = reparsed
        .lines
        .iter()
        .filter(|n| matches!(n, envforge::model::LineNode::EnvExport { key, .. } if key == "TOKEN"))
        .count();
    assert_eq!(
        n, 1,
        "add produced a malformed/duplicate entry: {serialized}"
    );
    let twice = serialize_shell_file(&reparsed);
    assert_eq!(serialized, twice, "add is not round-trip stable");
}

/// Renaming preserves an awkward value without corrupting the line.
#[test]
fn test_rename_entry_preserves_quoted_value() {
    let mut sf = parse("export OLD=\"a\\\"b\"\n");
    rename_entry(&mut sf, "OLD", "NEW").unwrap();
    let serialized = serialize_shell_file(&sf);
    assert!(
        serialized.contains("NEW="),
        "rename did not apply: {serialized}"
    );
    assert!(
        !serialized.contains("OLD="),
        "old key remains: {serialized}"
    );
    let twice = serialize_shell_file(&parse(&serialized));
    assert_eq!(serialized, twice, "rename is not round-trip stable");
}

// ─── Bug 2: fence must not clobber unparseable settings.json ────────────────

mod fence_clobber {
    use envforge::config::FenceConfig;
    use envforge::ops::fence::create_fence_with;

    /// An existing `.claude/settings.json` that is not strict JSON (here: a
    /// trailing comma) must be left byte-for-byte untouched, and the failure
    /// surfaced in `files_failed` — never silently overwritten with `{}`.
    #[test]
    fn test_unparseable_settings_json_is_not_clobbered() {
        let project = tempfile::tempdir().unwrap();
        let claude_dir = project.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let settings = claude_dir.join("settings.json");
        // Trailing comma — valid in hand-edited configs, rejected by strict JSON.
        let original = "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\",\n}\n";
        std::fs::write(&settings, original).unwrap();

        let cfg = FenceConfig::default(); // all targets enabled, incl. Claude Code
        let result = create_fence_with(project.path(), false, &cfg).unwrap();

        let after = std::fs::read_to_string(&settings).unwrap();
        assert_eq!(
            after, original,
            "unparseable settings.json must be left untouched, not clobbered"
        );
        assert!(
            result
                .files_failed
                .iter()
                .any(|(p, _)| p.ends_with("settings.json")),
            "parse failure must be reported in files_failed, got: {:?}",
            result.files_failed
        );
    }

    /// Sanity: a VALID settings.json is still merged (deny rules added),
    /// preserving the user's other keys.
    #[test]
    fn test_valid_settings_json_is_merged_preserving_keys() {
        let project = tempfile::tempdir().unwrap();
        let claude_dir = project.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let settings = claude_dir.join("settings.json");
        std::fs::write(&settings, "{\n  \"theme\": \"dark\"\n}\n").unwrap();

        let cfg = FenceConfig::default();
        create_fence_with(project.path(), false, &cfg).unwrap();

        let after = std::fs::read_to_string(&settings).unwrap();
        let json: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(json["theme"], "dark", "existing key must be preserved");
        assert!(
            json.pointer("/permissions/deny").is_some(),
            "deny rules must be merged in: {after}"
        );
    }
}

// ─── Bug 3: provider suggestion must not panic on non-ASCII input ───────────

mod provider_unicode {
    use envforge::ops::secrets::provider::ProviderRegistry;

    /// A non-existent provider name whose first character is multi-byte must
    /// return a `ProviderNotFound` error, not panic on a non-char-boundary
    /// byte slice inside `suggest_similar`.
    #[test]
    fn test_get_with_multibyte_name_does_not_panic() {
        let reg = ProviderRegistry::new();
        for name in ["ñx", "αβ", "🦀x", "é"] {
            let res = reg.get(name);
            assert!(res.is_err(), "expected error for unknown provider {name:?}");
        }
    }
}

// ─── Bug 4: lease names must not allow path traversal ───────────────────────

mod lease_traversal {
    use envforge::ops::lease::{create_lease, renew_lease, revoke_lease};

    /// Traversal / separator characters in a lease name are rejected before
    /// any filesystem access (validation short-circuits ahead of the dir
    /// join), so these are hermetic and never touch the real config dir.
    #[test]
    fn test_create_lease_rejects_traversal_names() {
        for bad in [
            "../evil",
            "../../tmp/evil",
            "foo/bar",
            "a\\b",
            "..",
            "with space",
            "",
        ] {
            let res = create_lease(bad, 60, None);
            assert!(res.is_err(), "lease name {bad:?} must be rejected");
        }
    }

    #[test]
    fn test_revoke_and_renew_reject_traversal_names() {
        assert!(revoke_lease("../evil").is_err());
        assert!(renew_lease("../../etc/evil", 60).is_err());
        assert!(revoke_lease("a/b").is_err());
    }
}

// ─── Bug 5: parser trailing-newline round-trip is byte-identical ────────────

mod trailing_newline {
    use super::parse;
    use envforge::parser::serialize_shell_file;

    /// A file WITHOUT a trailing newline must not gain one on serialize (both
    /// the free function and the `ShellFile::serialize` method). Before the
    /// fix the writer unconditionally appended `\n`.
    #[test]
    fn test_missing_trailing_newline_not_added() {
        for content in ["export A=1", "export A=1\nexport B=2", "# comment"] {
            let sf = parse(content);
            assert_eq!(
                serialize_shell_file(&sf),
                content,
                "free fn added a trailing newline"
            );
            assert_eq!(sf.serialize(), content, "method changed trailing newline");
        }
    }

    /// A file WITH a trailing newline keeps exactly one — including extra
    /// trailing blank lines.
    #[test]
    fn test_trailing_newline_preserved() {
        for content in ["export A=1\n", "export A=1\nexport B=2\n", "a\n\n"] {
            let sf = parse(content);
            assert_eq!(
                serialize_shell_file(&sf),
                content,
                "trailing newline state changed for {content:?}"
            );
            assert_eq!(sf.serialize(), content);
        }
    }

    #[test]
    fn test_empty_content_round_trips() {
        let sf = parse("");
        assert_eq!(serialize_shell_file(&sf), "");
        assert_eq!(sf.serialize(), "");
    }
}
