//! Data-driven fence target registry (Story 1.1 / Architecture D1).
//!
//! The registry is the single source of truth for *which* AI-tool fence
//! targets exist and their metadata (paths, file kind, ownership, detection
//! hints, upstream convention source, whether the tool has a real ignore
//! mechanism). Adding a new tool is a data entry here plus tests — not new
//! control flow (NFR-M1).
//!
//! Story 1.1 introduces the data model and routes target id / path / iteration
//! through it. The `FileKind`-dispatched writers/strippers (Story 1.2) and the
//! richer status/detection consumers (Stories 1.6/1.7) build on this module.

use super::FenceTarget;

/// How EnvForge protects via a given file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A tool-native ignore file (line list of globs), e.g. `.cursorignore`.
    Ignore,
    /// A rules / instructions file (markdown block), e.g. `.cursorrules`,
    /// `.github/copilot-instructions.md`, `AGENTS.md`.
    Rules,
    /// A structured deny rule merged into a JSON settings file, e.g.
    /// `permissions.deny` in `.claude/settings.json`.
    DenyRule,
    /// The cross-tool `AGENTS.md` standard (rules-only).
    CrossTool,
}

/// Who owns the file's content, which determines disable behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// EnvForge owns the entire file — delete it on disable.
    FullyOwned,
    /// Shared with user content — surgically strip only EnvForge's block.
    Shared,
}

/// A single file a fence target writes/maintains.
#[derive(Debug, Clone, Copy)]
pub struct TargetFile {
    /// Path relative to the project root.
    pub path: &'static str,
    pub kind: FileKind,
    pub ownership: Ownership,
    /// The content EnvForge writes. For `Ignore`/`Rules`/`CrossTool` this is
    /// the block; its first line doubles as the idempotency/strip marker. For
    /// `DenyRule` this is unused (empty) — see `deny_rules`.
    pub block: &'static str,
    /// Deny entries for `DenyRule` files (e.g. `Read(.env)`). Empty otherwise.
    pub deny_rules: &'static [&'static str],
}

/// Declarative specification for one fence target.
#[derive(Debug, Clone, Copy)]
pub struct FenceTargetSpec {
    /// The canonical enum variant this spec backs.
    pub target: FenceTarget,
    /// Stable snake_case id — used in JSON, config keys, CLI, status.
    pub id: &'static str,
    /// Human-readable label for status output.
    pub display: &'static str,
    /// The AI tool this target protects (marketing name).
    pub tool: &'static str,
    /// One or more files this target manages.
    pub files: &'static [TargetFile],
    /// Path/marker hints that signal the tool is installed (Story 1.7).
    pub detection: &'static [&'static str],
    /// Authoritative source for this convention (FR25 verifiability).
    pub source_url: &'static str,
    /// Whether the tool has a real ignore mechanism. `false` means
    /// protection is rules/deny fallback only — status must report
    /// `fallback`, never a false `covered` (Story 1.6 / FR2b).
    pub has_real_ignore: bool,
}

/// The fence target registry, in canonical order (matches `FenceTarget::all()`).
pub static REGISTRY: &[FenceTargetSpec] = &[
    FenceTargetSpec {
        target: FenceTarget::Envforgeignore,
        id: "envforgeignore",
        display: "EnvForge ignore",
        tool: "EnvForge",
        files: &[TargetFile {
            path: ".envforgeignore",
            kind: FileKind::Ignore,
            ownership: Ownership::FullyOwned,
            block: super::ENVFORGEIGNORE_CONTENT,
            deny_rules: &[],
        }],
        detection: &[],
        source_url: "https://github.com/envforge/envforge (EnvForge-native format)",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::CursorIgnore,
        id: "cursor_ignore",
        display: "Cursor (ignore)",
        tool: "Cursor",
        files: &[TargetFile {
            path: ".cursorignore",
            kind: FileKind::Ignore,
            ownership: Ownership::Shared,
            block: super::CURSORIGNORE_BLOCK,
            deny_rules: &[],
        }],
        detection: &[".cursor", ".cursorignore"],
        source_url: "https://docs.cursor.com/context/ignore-files",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::CursorRules,
        id: "cursor_rules",
        display: "Cursor (rules)",
        tool: "Cursor",
        files: &[TargetFile {
            path: ".cursorrules",
            kind: FileKind::Rules,
            ownership: Ownership::Shared,
            block: super::CURSORRULES_BLOCK,
            deny_rules: &[],
        }],
        detection: &[".cursorrules", ".cursor/rules"],
        source_url: "https://docs.cursor.com/context/rules",
        has_real_ignore: false,
    },
    FenceTargetSpec {
        target: FenceTarget::Copilot,
        id: "copilot",
        display: "GitHub Copilot",
        tool: "GitHub Copilot",
        files: &[TargetFile {
            path: ".github/copilot-instructions.md",
            kind: FileKind::Rules,
            ownership: Ownership::Shared,
            block: super::COPILOT_INSTRUCTIONS,
            deny_rules: &[],
        }],
        detection: &[".github/copilot-instructions.md"],
        source_url: "https://docs.github.com/en/copilot/customizing-copilot/adding-repository-custom-instructions-for-github-copilot",
        has_real_ignore: false,
    },
    FenceTargetSpec {
        target: FenceTarget::ClaudeCode,
        id: "claude_code",
        display: "Claude Code",
        tool: "Claude Code",
        files: &[TargetFile {
            path: ".claude/settings.json",
            kind: FileKind::DenyRule,
            ownership: Ownership::Shared,
            block: "",
            deny_rules: super::CLAUDE_DENY_RULES,
        }],
        detection: &[".claude", ".claude/settings.json"],
        source_url: "https://code.claude.com/docs/en/settings",
        has_real_ignore: false,
    },
    FenceTargetSpec {
        target: FenceTarget::Windsurf,
        id: "windsurf",
        display: "Windsurf / Codeium",
        tool: "Windsurf",
        files: &[
            TargetFile {
                path: ".codeiumignore",
                kind: FileKind::Ignore,
                ownership: Ownership::Shared,
                block: super::CURSORIGNORE_BLOCK,
                deny_rules: &[],
            },
            TargetFile {
                path: ".windsurf/rules/envforge.md",
                kind: FileKind::Rules,
                ownership: Ownership::Shared,
                block: super::CURSORRULES_BLOCK,
                deny_rules: &[],
            },
        ],
        detection: &[".codeium", ".windsurf", ".codeiumignore"],
        source_url: "https://docs.windsurf.com/windsurf/cascade/memories#ignore-files",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::Cline,
        id: "cline",
        display: "Cline",
        tool: "Cline",
        files: &[
            TargetFile {
                path: ".clineignore",
                kind: FileKind::Ignore,
                ownership: Ownership::Shared,
                block: super::CURSORIGNORE_BLOCK,
                deny_rules: &[],
            },
            TargetFile {
                path: ".clinerules",
                kind: FileKind::Rules,
                ownership: Ownership::Shared,
                block: super::CURSORRULES_BLOCK,
                deny_rules: &[],
            },
        ],
        detection: &[".clinerules", ".clineignore"],
        source_url: "https://docs.cline.bot/features/cline-rules",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::Aider,
        id: "aider",
        display: "Aider",
        tool: "Aider",
        files: &[TargetFile {
            path: ".aiderignore",
            kind: FileKind::Ignore,
            ownership: Ownership::Shared,
            block: super::CURSORIGNORE_BLOCK,
            deny_rules: &[],
        }],
        detection: &[".aiderignore", ".aider.conf.yml"],
        source_url: "https://aider.chat/docs/config/options.html",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::Gemini,
        id: "gemini",
        display: "Gemini CLI",
        tool: "Gemini CLI",
        files: &[
            TargetFile {
                path: ".geminiignore",
                kind: FileKind::Ignore,
                ownership: Ownership::Shared,
                block: super::CURSORIGNORE_BLOCK,
                deny_rules: &[],
            },
            TargetFile {
                path: "GEMINI.md",
                kind: FileKind::Rules,
                ownership: Ownership::Shared,
                block: super::CURSORRULES_BLOCK,
                deny_rules: &[],
            },
        ],
        detection: &[".geminiignore", "GEMINI.md", ".gemini"],
        source_url: "https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/configuration.md",
        has_real_ignore: true,
    },
    FenceTargetSpec {
        target: FenceTarget::AgentsMd,
        id: "agents_md",
        display: "AGENTS.md (cross-tool)",
        tool: "AGENTS.md standard",
        files: &[TargetFile {
            path: "AGENTS.md",
            kind: FileKind::CrossTool,
            ownership: Ownership::Shared,
            block: super::AGENTS_MD_BLOCK,
            deny_rules: &[],
        }],
        detection: &["AGENTS.md", ".codex", ".zed"],
        source_url: "https://agents.md",
        // Rules-only standard — no ignore mechanism. Status reports `fallback`,
        // never a false `covered` (FR2b / Story 1.6).
        has_real_ignore: false,
    },
    FenceTargetSpec {
        target: FenceTarget::AmazonQ,
        id: "amazon_q",
        display: "Amazon Q Developer",
        tool: "Amazon Q",
        files: &[TargetFile {
            path: ".amazonq/rules/envforge.md",
            kind: FileKind::Rules,
            ownership: Ownership::Shared,
            block: super::COPILOT_INSTRUCTIONS,
            deny_rules: &[],
        }],
        detection: &[".amazonq", ".amazonq/rules"],
        source_url: "https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/context-project-rules.html",
        // No ignore file documented — rules fallback only.
        has_real_ignore: false,
    },
];

/// Look up the spec for a target. Every `FenceTarget` has exactly one entry;
/// this is enforced by `test_registry_covers_every_target`.
#[must_use]
pub fn spec_for(target: FenceTarget) -> &'static FenceTargetSpec {
    REGISTRY
        .iter()
        .find(|s| s.target == target)
        .expect("every FenceTarget must have a registry entry")
}

/// The primary (first) file path for a target. All current targets manage a
/// single file; multi-file targets use `spec.files` directly.
#[must_use]
pub fn primary_path(target: FenceTarget) -> &'static str {
    spec_for(target).files[0].path
}

/// Whether `id` matches a known fence-target registry id.
#[must_use]
pub fn is_valid_id(id: &str) -> bool {
    REGISTRY.iter().any(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build-time validation: every entry is well-formed (NFR-M2).
    #[test]
    fn test_registry_entries_well_formed() {
        for spec in REGISTRY {
            assert!(!spec.id.is_empty(), "spec id must be non-empty");
            assert!(
                spec.id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "id '{}' must be snake_case",
                spec.id
            );
            assert!(!spec.display.is_empty(), "{}: display non-empty", spec.id);
            assert!(!spec.tool.is_empty(), "{}: tool non-empty", spec.id);
            assert!(!spec.files.is_empty(), "{}: must declare ≥1 file", spec.id);
            assert!(
                !spec.source_url.is_empty(),
                "{}: source_url required (FR25)",
                spec.id
            );
            for f in spec.files {
                assert!(!f.path.is_empty(), "{}: file path non-empty", spec.id);
                match f.kind {
                    FileKind::DenyRule => assert!(
                        !f.deny_rules.is_empty(),
                        "{}: DenyRule file must declare deny_rules",
                        spec.id
                    ),
                    _ => assert!(
                        !f.block.is_empty(),
                        "{}: {:?} file must declare a content block",
                        spec.id,
                        f.kind
                    ),
                }
            }
        }
    }

    /// The registry covers every `FenceTarget` exactly once, in the same order
    /// as `FenceTarget::all()`.
    #[test]
    fn test_registry_covers_every_target() {
        let all = FenceTarget::all();
        assert_eq!(
            REGISTRY.len(),
            all.len(),
            "registry must have one entry per FenceTarget"
        );
        for (i, target) in all.into_iter().enumerate() {
            assert_eq!(
                REGISTRY[i].target, target,
                "registry order must match FenceTarget::all()"
            );
        }
        // Unique ids.
        let ids: std::collections::HashSet<&str> = REGISTRY.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), REGISTRY.len(), "target ids must be unique");
    }

    /// Registry ids match the enum's `as_str` (single source of truth).
    #[test]
    fn test_registry_id_matches_as_str() {
        for spec in REGISTRY {
            assert_eq!(
                spec.id,
                spec.target.as_str(),
                "registry id must equal FenceTarget::as_str"
            );
        }
    }
}
