use std::path::{Path, PathBuf};

use super::OpError;
use crate::config::FenceConfig;

pub mod registry;

// ─── Canonical Target Enum ───────────────────────────────────────────────────

/// The AI-tool fence targets EnvForge can manage.
///
/// This enum is the single source of truth for the target set (NFR10).
/// No other module may define or enumerate these targets independently.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize,
)]
#[clap(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FenceTarget {
    /// `.envforgeignore` — fully-owned by EnvForge.
    Envforgeignore,
    /// `.cursorignore` — Cursor AI ignore rules.
    CursorIgnore,
    /// `.cursorrules` — Cursor AI behavior rules.
    CursorRules,
    /// `.github/copilot-instructions.md` — GitHub Copilot safety rules.
    Copilot,
    /// `.claude/settings.json` — Claude Code deny-list rules.
    ClaudeCode,
    /// `.codeiumignore` + `.windsurf/rules/envforge.md` — Windsurf / Codeium rules.
    Windsurf,
    /// `.clineignore` + `.clinerules` — Cline rules.
    Cline,
    /// `.aiderignore` — Aider ignore rules.
    Aider,
    /// `.geminiignore` + `GEMINI.md` — Gemini CLI rules.
    Gemini,
    /// `AGENTS.md` — cross-tool rules standard (Codex, Zed, and any
    /// AGENTS.md-honoring tool). Rules-only, no ignore mechanism.
    AgentsMd,
    /// `.amazonq/rules/envforge.md` — Amazon Q Developer (no ignore file).
    AmazonQ,
}

impl FenceTarget {
    /// Returns the canonical snake_case identifier string for this target.
    /// Used in JSON output, config keys, and CLI display.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        registry::spec_for(self).id
    }

    /// Returns all targets in canonical order (derived from the registry).
    #[must_use]
    pub fn all() -> Vec<FenceTarget> {
        registry::REGISTRY.iter().map(|s| s.target).collect()
    }
}

// ─── Config Resolution ───────────────────────────────────────────────────────

/// Where a target's enabled state originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    /// No explicit config — using the compiled-in default (`true`).
    Default,
    /// Explicitly set in the user's global config file.
    Global,
}

/// Resolved state for a single fence target: effective value + where it came from.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedTarget {
    pub target: FenceTarget,
    pub enabled: bool,
    pub source: ConfigSource,
}

/// Format the resolved enabled fence targets as a compact read-only summary string.
///
/// Returns a string suitable for status bars and footers:
/// - All targets enabled → `"fence: (9/9)"`
/// - Subset enabled → `"fence: cursor_ignore,copilot (2/9)"`
/// - None enabled → `"fence: none (0/9)"`
///
/// This is a pure function with no I/O so it is directly unit-testable (FR16 / NFR10).
///
/// # Examples
///
/// ```
/// use envforge::ops::fence::{fence_target_summary, resolve_fence_targets};
/// use envforge::config::FenceConfig;
///
/// let cfg = FenceConfig::default();
/// let resolved = resolve_fence_targets(&cfg);
/// let s = fence_target_summary(&resolved);
/// let total = envforge::ops::fence::FenceTarget::all().len();
/// assert!(s.contains(&format!("{total}/{total}")), "all-enabled summary: {s}");
/// ```
#[must_use]
pub fn fence_target_summary(resolved: &[ResolvedTarget]) -> String {
    let enabled_names: Vec<&str> = resolved
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.target.as_str())
        .collect();
    let total = resolved.len();
    let count = enabled_names.len();

    if count == 0 {
        format!("fence: none ({count}/{total})")
    } else if count == total {
        format!("fence: ({count}/{total})")
    } else {
        format!("fence: {} ({count}/{total})", enabled_names.join(","))
    }
}

/// Resolve the effective fence target set from config.
///
/// For MVP, layering is: Global config overrides Default (all-true).
/// If a field matches the default (`true`), source is `Default`; otherwise `Global`.
/// The Growth per-project layer would add a third `ConfigSource::Project` here.
///
/// Reads config once per call (NFR4).
#[must_use]
pub fn resolve_fence_targets(cfg: &FenceConfig) -> Vec<ResolvedTarget> {
    registry::REGISTRY
        .iter()
        .map(|spec| {
            let target = spec.target;
            let enabled = cfg.targets.is_enabled(target);
            // If the value is false, it was explicitly set in global config.
            // If true, it could be default or global-set-to-true; we treat
            // explicit false as the only unambiguous Global indicator at MVP.
            let source = if enabled {
                ConfigSource::Default
            } else {
                ConfigSource::Global
            };
            ResolvedTarget {
                target,
                enabled,
                source,
            }
        })
        .collect()
}

/// Load the fence config with a fail-safe fallback.
///
/// On any `ConfigError` (IO or parse), emits a monitor warning and returns
/// `FenceConfig::default()` (all targets enabled). This ensures fence activation
/// fails-safe: a broken config file never blocks protection (FR19, NFR2).
fn load_fence_config_or_safe_default() -> FenceConfig {
    match crate::config::load_or_create_default() {
        Ok(cfg) => cfg.fence,
        Err(e) => {
            crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
                source: crate::ops::monitor::EventSource::Fence,
                key: None,
                message: format!(
                    "Failed to load fence config (fail-safe: all targets enabled): {}",
                    e
                ),
                timestamp: chrono::Utc::now(),
                severity: crate::ops::monitor::SecuritySeverity::Warn,
            });
            FenceConfig::default()
        }
    }
}

/// Result of creating AI tool fence files.
pub struct FenceResult {
    pub files_created: Vec<PathBuf>,
    pub files_updated: Vec<PathBuf>,
    pub files_skipped: Vec<PathBuf>,
    /// Per-file failures `(path, error)`. One target failing does not abort
    /// the others — failures are collected here so callers can report partial
    /// results and set a non-zero exit code (NFR-R4 / FR1).
    pub files_failed: Vec<(PathBuf, String)>,
}

/// Status of a single fence file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FenceFileStatus {
    pub path: String,
    pub exists: bool,
    pub fenced: bool,
}

/// Completeness of the AI tool fence.
///
/// Replaces the old `all_fenced: bool` which collapsed partial coverage
/// into a single bit.  The sum type preserves the list of files that are
/// NOT fenced so callers can surface actionable diagnostics.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "level", content = "unfenced")]
pub enum FenceCompleteness {
    /// Every expected fence file exists and is properly configured.
    Complete,
    /// One or more fence files are missing, not configured, or stale.
    /// The list contains the status of each file that is NOT fully fenced.
    Partial(Vec<FenceFileStatus>),
}

/// Per-target coverage state (Story 1.6 / FR7, FR11).
///
/// `covered` = a tool with a real ignore mechanism, fully fenced.
/// `fallback` = a tool with NO ignore file (rules/deny only), protection
///   applied via its available mechanism — honest about being weaker than a
///   hard ignore (FR2b).
/// `unfenced` = expected but not (fully) fenced.
/// `not_installed` = detected as absent (populated by detection, Story 1.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetCoverage {
    Covered,
    Fallback,
    Unfenced,
    NotInstalled,
}

/// Per-target coverage entry — the honest, named view (Story 1.6/1.7).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetStatus {
    pub id: String,
    pub tool: String,
    pub state: TargetCoverage,
    /// Whether the tool was detected as present in the workspace (Story 1.7).
    /// EnvForge-owned targets (empty detection hints) are always `true`.
    pub installed: bool,
    pub files: Vec<FenceFileStatus>,
}

/// Overall fence status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FenceStatus {
    pub files: Vec<FenceFileStatus>,
    /// Derived convenience field — `true` when every enabled target is
    /// `covered` or `fallback` (no target left `unfenced`). This is the
    /// honest aggregate behind the "AI BLOCKED" indicator (FR11).
    pub all_fenced: bool,
    /// Structured completeness assessment.
    pub completeness: FenceCompleteness,
    /// Per-target coverage states (FR7) — the named view plugins/CLI render.
    pub targets: Vec<TargetStatus>,
}

const ENVFORGEIGNORE_CONTENT: &str = ".env
.env.*
!.env.schema
!.env.example
credentials.toml
*.key
*.pem
secret-sources.toml
";

const CURSORIGNORE_BLOCK: &str = "\
# EnvForge secret fence
.env
.env.*
!.env.schema
!.env.example
credentials.toml
*.key
*.pem
";

const COPILOT_INSTRUCTIONS: &str = "\
## Secret Safety Rules
- Never read or output the contents of .env files
- Never hardcode API keys, tokens, passwords, or credentials
- Use environment variable references (process.env.KEY, std::env::var(\"KEY\"), os.environ[\"KEY\"])
- Check .env.schema for variable names and types instead of .env for values
";

const CLAUDE_DENY_RULES: &[&str] = &[
    "Read(.env)",
    "Read(.env.*)",
    "Read(credentials.toml)",
    "Read(*.key)",
    "Read(*.pem)",
];

const CURSORRULES_BLOCK: &str = "\
Never read .env files directly. Use .env.schema or .env.ai.md for variable context.
Never hardcode secrets, API keys, tokens, or passwords.
";

/// Markdown block written to `AGENTS.md` (and other cross-tool/no-ignore
/// rules files). Starts with a `## ` heading so it round-trips as a
/// markdown section (strip excises heading → next `## ` / EOF), preserving
/// any user-authored content around it.
pub(super) const AGENTS_MD_BLOCK: &str = "\
## EnvForge Secret Safety
- Never read or output the contents of .env files or other secret files.
- Never hardcode API keys, tokens, passwords, or credentials.
- Use .env.schema for variable names and types; load values via environment variables.
";

/// Generate and write AI tool ignore rules for all supported tools.
///
/// Loads the fence config with a fail-safe fallback (all targets enabled if
/// config is missing or malformed) then delegates to [`create_fence_with`].
/// All four existing callers (CLI, TUI, LSP, plugins-via-CLI) use this
/// signature unchanged (NFR8).
pub fn create_fence(project_dir: &Path, dry_run: bool) -> Result<FenceResult, OpError> {
    let cfg = load_fence_config_or_safe_default();
    create_fence_with(project_dir, dry_run, &cfg)
}

/// Generate and write AI tool ignore rules gated by `cfg`.
///
/// Targets that are disabled in `cfg` are not written; their paths are
/// pushed to `files_skipped` so callers can report what was omitted.
/// Use this variant in tests to supply explicit configs without touching
/// the global config file.
pub fn create_fence_with(
    project_dir: &Path,
    dry_run: bool,
    cfg: &FenceConfig,
) -> Result<FenceResult, OpError> {
    // Emit monitor event
    crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
        source: crate::ops::monitor::EventSource::Fence,
        key: None,
        message: format!(
            "Fence {} in {}",
            if dry_run { "dry-run" } else { "created" },
            project_dir.display()
        ),
        timestamp: chrono::Utc::now(),
        severity: crate::ops::monitor::SecuritySeverity::Info,
    });
    let mut result = FenceResult {
        files_created: Vec::new(),
        files_updated: Vec::new(),
        files_skipped: Vec::new(),
        files_failed: Vec::new(),
    };

    // Iterate the registry in canonical order. Enabled targets are written;
    // disabled targets push each file path to files_skipped. Writers are
    // dispatched by FileKind/Ownership (Story 1.2). A single file's failure is
    // captured in files_failed and does NOT abort the remaining targets
    // (NFR-R4) — fencing the rest of the toolchain still proceeds.
    for spec in registry::REGISTRY {
        let target = spec.target;
        if cfg.targets.is_enabled(target) {
            for file in spec.files {
                if let Err(e) = write_file(project_dir, file, dry_run, &mut result) {
                    result
                        .files_failed
                        .push((project_dir.join(file.path), e.to_string()));
                }
            }
        } else {
            for file in spec.files {
                result.files_skipped.push(project_dir.join(file.path));
            }
        }
    }

    Ok(result)
}

// ─── Atomic write helper ─────────────────────────────────────────────────────

/// Write `content` to `path` atomically using a temp-file + rename.
/// Creates parent directories as needed. Does NOT force 0o600 permissions.
fn atomic_write_fence(path: &std::path::Path, content: &str) -> Result<(), OpError> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| OpError::Io(e.error))?;
    Ok(())
}

/// Return the first line of a block (the idempotency/detection marker).
fn first_line(block: &str) -> &str {
    block.lines().next().unwrap_or("")
}

// ─── FileKind-dispatched writers ─────────────────────────────────────────────

/// Dispatch a `TargetFile` to the correct writer based on `FileKind`/`Ownership`.
fn write_file(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    match file.kind {
        registry::FileKind::DenyRule => write_deny_rule(dir, file, dry_run, result),
        _ => match file.ownership {
            registry::Ownership::FullyOwned => write_owned_file(dir, file, dry_run, result),
            registry::Ownership::Shared => write_shared_block(dir, file, dry_run, result),
        },
    }
}

/// Write a fully-owned file (e.g. `.envforgeignore`).
/// Skip if it already exists; create fresh otherwise.
fn write_owned_file(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);
    if path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    if !dry_run {
        atomic_write_fence(&path, file.block)?;
    }
    result.files_created.push(path);
    Ok(())
}

/// Append an EnvForge block to a shared file (e.g. `.cursorignore`, `.cursorrules`,
/// `.github/copilot-instructions.md`). Idempotent: skips if the marker is already
/// present. Creates the file (and parent dirs) if it does not exist.
fn write_shared_block(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);
    let marker = first_line(file.block);
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if content.contains(marker) {
            result.files_skipped.push(path);
            return Ok(());
        }
        if !dry_run {
            let new_content = if content.ends_with('\n') {
                format!("{content}\n{}", file.block)
            } else {
                format!("{content}\n\n{}", file.block)
            };
            atomic_write_fence(&path, &new_content)?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            // atomic_write_fence creates parent dirs (e.g. .github)
            atomic_write_fence(&path, file.block)?;
        }
        result.files_created.push(path);
    }
    Ok(())
}

/// Merge deny rules into a JSON settings file (e.g. `.claude/settings.json`).
/// Idempotent: only adds rules not already present. Creates the file if absent.
fn write_deny_rule(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let mut json: serde_json::Value =
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

        // Check which deny rules are already present
        let existing_deny = json
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let existing_strs: Vec<String> = existing_deny
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        let new_rules: Vec<&str> = file
            .deny_rules
            .iter()
            .filter(|r| !existing_strs.contains(&(**r).to_string()))
            .copied()
            .collect();

        if new_rules.is_empty() {
            result.files_skipped.push(path);
            return Ok(());
        }

        if !dry_run {
            // Merge deny rules into existing JSON
            let permissions = json
                .as_object_mut()
                .ok_or_else(|| OpError::Other("settings.json is not a JSON object".into()))?
                .entry("permissions")
                .or_insert_with(|| serde_json::json!({}));
            let deny = permissions
                .as_object_mut()
                .ok_or_else(|| {
                    OpError::Other("settings.json permissions is not a JSON object".into())
                })?
                .entry("deny")
                .or_insert_with(|| serde_json::json!([]));

            if let Some(arr) = deny.as_array_mut() {
                for rule in new_rules {
                    arr.push(serde_json::Value::String(rule.to_string()));
                }
            }

            let output = serde_json::to_string_pretty(&json)?;
            atomic_write_fence(&path, &format!("{output}\n"))?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            // atomic_write_fence creates parent dirs (e.g. .claude)
            let json = serde_json::json!({
                "permissions": {
                    "deny": file.deny_rules
                }
            });
            let output = serde_json::to_string_pretty(&json)?;
            atomic_write_fence(&path, &format!("{output}\n"))?;
        }
        result.files_created.push(path);
    }
    Ok(())
}

fn check_file_status(project_dir: &Path, rel_path: &str) -> FenceFileStatus {
    let full_path = project_dir.join(rel_path);
    let exists = full_path.exists();
    let content = if exists {
        std::fs::read_to_string(&full_path).unwrap_or_default()
    } else {
        String::new()
    };
    // Look up the file in the registry to determine the correct fenced check.
    let fenced = registry::REGISTRY
        .iter()
        .flat_map(|spec| spec.files.iter())
        .find(|f| f.path == rel_path)
        .map(|f| match f.kind {
            registry::FileKind::DenyRule => serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| {
                    v.pointer("/permissions/deny")
                        .and_then(|d| d.as_array())
                        .map(|a| !a.is_empty())
                })
                .unwrap_or(false),
            registry::FileKind::Ignore
            | registry::FileKind::Rules
            | registry::FileKind::CrossTool => {
                let marker = first_line(f.block);
                content.contains(marker)
            }
        })
        .unwrap_or(false);
    FenceFileStatus {
        path: rel_path.to_string(),
        exists,
        fenced,
    }
}

/// Result of removing envforge-owned fence content.
#[derive(Debug, Default)]
pub struct FenceRemoveResult {
    /// Files we fully deleted because they are envforge-owned end-to-end.
    pub files_removed: Vec<PathBuf>,
    /// Files we edited in place to strip envforge-owned sections.
    pub files_updated: Vec<PathBuf>,
    /// Files that didn't exist or contained no envforge content.
    pub files_skipped: Vec<PathBuf>,
}

/// Strip every envforge-owned fence section while preserving any user
/// content in shared files. Symmetric counterpart of [`create_fence`].
///
/// File-by-file behavior:
/// - `.envforgeignore` — deleted (we own the whole file).
/// - `.cursorignore` — `CURSORIGNORE_BLOCK` removed; if the file becomes
///   empty (or whitespace-only) it is deleted as well.
/// - `.cursorrules` — `CURSORRULES_BLOCK` removed; same emptiness rule.
/// - `.github/copilot-instructions.md` — the `## Secret Safety Rules`
///   section is excised up to the next `##` heading or EOF; same rule.
/// - `.claude/settings.json` — `CLAUDE_DENY_RULES` entries are pulled out
///   of `permissions.deny`; if the array empties, `deny` is removed; if
///   `permissions` empties, it is removed too. Resulting `{}` files are
///   deleted to avoid leaving orphan stubs.
///
/// Operates over ALL registry targets regardless of config (config-independent).
pub fn remove_fence(project_dir: &Path, dry_run: bool) -> Result<FenceRemoveResult, OpError> {
    crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
        source: crate::ops::monitor::EventSource::Fence,
        key: None,
        message: format!(
            "Fence {} in {}",
            if dry_run { "remove dry-run" } else { "removed" },
            project_dir.display()
        ),
        timestamp: chrono::Utc::now(),
        severity: crate::ops::monitor::SecuritySeverity::Info,
    });

    let mut result = FenceRemoveResult::default();

    // Config-independent: iterate ALL registry targets so a previously-fenced
    // file is always cleaned even if the target has since been disabled.
    for spec in registry::REGISTRY {
        for file in spec.files {
            strip_file(project_dir, file, dry_run, &mut result)?;
        }
    }

    Ok(result)
}

// ─── FileKind-dispatched strippers ───────────────────────────────────────────

/// Dispatch a `TargetFile` to the correct stripper based on `FileKind`/`Ownership`.
fn strip_file(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    match file.kind {
        registry::FileKind::DenyRule => strip_deny_rule(dir, file, dry_run, result),
        _ => match file.ownership {
            registry::Ownership::FullyOwned => delete_owned_file(dir, file, dry_run, result),
            registry::Ownership::Shared => strip_shared(dir, file, dry_run, result),
        },
    }
}

/// Delete a fully-owned file (e.g. `.envforgeignore`).
fn delete_owned_file(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    if !dry_run {
        std::fs::remove_file(&path)?;
    }
    result.files_removed.push(path);
    Ok(())
}

/// Strip an EnvForge-owned block from a shared file.
/// For `Rules` files whose block starts with `## `, uses markdown-section excision;
/// for `Ignore`/`Rules` (non-markdown-section) files, uses block-substring removal.
fn strip_shared(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let stripped = if file.block.trim_start().starts_with("## ") {
        strip_markdown_section(&content, first_line(file.block))
    } else {
        strip_block(&content, file.block)
    };
    if stripped == content {
        result.files_skipped.push(path);
        return Ok(());
    }
    write_or_delete(&path, &stripped, dry_run, result)?;
    Ok(())
}

/// Strip EnvForge deny rules from a JSON settings file (e.g. `.claude/settings.json`).
fn strip_deny_rule(
    dir: &Path,
    file: &registry::TargetFile,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(file.path);
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut json: serde_json::Value = if let Ok(v) = serde_json::from_str(&content) {
        v
    } else {
        // Unparseable — leave it alone; user can clean up manually.
        result.files_skipped.push(path);
        return Ok(());
    };

    let mut mutated = false;
    let permissions_empty;
    let deny_empty;

    if let Some(perms) = json
        .as_object_mut()
        .and_then(|root| root.get_mut("permissions"))
        .and_then(|p| p.as_object_mut())
    {
        if let Some(deny) = perms.get_mut("deny").and_then(|d| d.as_array_mut()) {
            let before_len = deny.len();
            deny.retain(|entry| {
                entry
                    .as_str()
                    .map(|s| !file.deny_rules.contains(&s))
                    .unwrap_or(true)
            });
            if deny.len() != before_len {
                mutated = true;
            }
            if deny.is_empty() {
                perms.remove("deny");
            }
        }
        permissions_empty = perms.is_empty();
        deny_empty = !perms.contains_key("deny");
    } else {
        permissions_empty = false;
        deny_empty = true;
    }

    if permissions_empty {
        if let Some(root) = json.as_object_mut() {
            root.remove("permissions");
            mutated = true;
        }
    }

    if !mutated {
        result.files_skipped.push(path);
        return Ok(());
    }

    // If JSON is now `{}`, delete the file so we don't leave a stub.
    let is_empty_object = matches!(&json, serde_json::Value::Object(m) if m.is_empty());
    if is_empty_object {
        if !dry_run {
            std::fs::remove_file(&path)?;
        }
        result.files_removed.push(path);
        return Ok(());
    }

    if !dry_run {
        let serialized = serde_json::to_string_pretty(&json)?;
        atomic_write_fence(&path, &format!("{serialized}\n"))?;
    }
    let _ = deny_empty;
    result.files_updated.push(path);
    Ok(())
}

/// Remove a block substring from `content`. Tolerates one preceding
/// blank-line gap so a leading `"\n\n"` separator (which `create_fence`
/// inserts when appending) does not stick around.
fn strip_block(content: &str, block: &str) -> String {
    let Some(idx) = content.find(block) else {
        return content.to_string();
    };
    let block_end = idx + block.len();

    // Trim a single leading `\n` or `\n\n` immediately before the block.
    let mut start = idx;
    let bytes = content.as_bytes();
    while start > 0 && bytes[start - 1] == b'\n' {
        start -= 1;
        if idx - start >= 2 {
            break;
        }
    }
    // Trim a single trailing `\n` if present (block already ends with one).
    let end = block_end;
    let mut out = String::with_capacity(content.len() - (end - start));
    out.push_str(&content[..start]);
    out.push_str(&content[end..]);
    out
}

/// Cut a `## heading` section out of a markdown document.
/// Removes from the heading line up to (but not including) the next `##`
/// heading at the same level, or to EOF if none exists.
fn strip_markdown_section(content: &str, heading: &str) -> String {
    let marker = heading;
    let Some(start) = content.find(marker) else {
        return content.to_string();
    };

    // Walk forward from the marker for the next line that starts with
    // `## ` (any second-level heading) to find the section terminator.
    let after = &content[start + marker.len()..];
    let mut end_offset = after.len();
    for (idx, line_start) in line_start_offsets(after) {
        if idx == 0 {
            continue;
        }
        if after[line_start..].starts_with("## ") {
            end_offset = line_start;
            break;
        }
    }
    let section_end = start + marker.len() + end_offset;

    // Trim a preceding blank-line gap so we don't leave a `\n\n\n` scar.
    let mut cut_start = start;
    let bytes = content.as_bytes();
    while cut_start > 0 && bytes[cut_start - 1] == b'\n' {
        cut_start -= 1;
        if start - cut_start >= 2 {
            break;
        }
    }

    let mut out = String::with_capacity(content.len() - (section_end - cut_start));
    out.push_str(&content[..cut_start]);
    out.push_str(&content[section_end..]);
    out
}

/// Iterate over `(line_idx, byte_offset)` for the start of each line in
/// `s`. Used by `strip_secret_safety_section` to locate the next `##`
/// heading without allocating split slices.
fn line_start_offsets(s: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut idx = 0usize;
    let mut next_line = 0usize;
    std::iter::from_fn(move || {
        if next_line > s.len() {
            return None;
        }
        let cur = next_line;
        let i = idx;
        idx += 1;
        if let Some(nl_pos) = s[cur..].find('\n') {
            next_line = cur + nl_pos + 1;
        } else {
            next_line = s.len() + 1;
        }
        Some((i, cur))
    })
}

/// Write the stripped content back, or delete the file if the strip
/// left nothing but whitespace (so we don't leave behind a meaningless
/// empty file).
fn write_or_delete(
    path: &Path,
    new_content: &str,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    if new_content.trim().is_empty() {
        if !dry_run {
            std::fs::remove_file(path)?;
        }
        result.files_removed.push(path.to_path_buf());
    } else {
        if !dry_run {
            // Normalize: ensure exactly one trailing newline.
            let mut to_write = new_content.trim_end().to_string();
            to_write.push('\n');
            atomic_write_fence(path, &to_write)?;
        }
        result.files_updated.push(path.to_path_buf());
    }
    Ok(())
}

/// Check which fence files exist and are properly configured.
///
/// Status is computed over the **enabled** target set only (D5).
/// A disabled target's stale file on disk does not make the fence `Partial`.
/// `all_fenced` is `true` iff every *enabled* target is present and fenced.
pub fn check_fence_status(project_dir: &Path) -> Result<FenceStatus, OpError> {
    check_fence_status_with(project_dir, &load_fence_config_or_safe_default())
}

/// Like [`check_fence_status`] but takes an explicit config (for testing / callers
/// that have already loaded config).
pub fn check_fence_status_with(
    project_dir: &Path,
    cfg: &FenceConfig,
) -> Result<FenceStatus, OpError> {
    // Build per-target coverage for enabled targets only. A target is fenced
    // iff ALL of its files are fenced (multi-file targets need every file).
    // `covered` vs `fallback` distinguishes a real ignore mechanism from a
    // rules/deny-only fallback (has_real_ignore) — FR7/FR2b honesty.
    let mut targets: Vec<TargetStatus> = Vec::new();
    let mut files: Vec<FenceFileStatus> = Vec::new();
    for spec in registry::REGISTRY
        .iter()
        .filter(|spec| cfg.targets.is_enabled(spec.target))
    {
        let f_stats: Vec<FenceFileStatus> = spec
            .files
            .iter()
            .map(|f| check_file_status(project_dir, f.path))
            .collect();
        let target_fenced = !f_stats.is_empty() && f_stats.iter().all(|f| f.fenced);
        // Detection (Story 1.7): a target is "installed" if any detection hint
        // exists in the workspace. EnvForge-owned targets (no hints) are always
        // applicable. An installed-but-unfenced tool is the dangerous case
        // (FR8); an absent tool is `not_installed` and does NOT break the
        // aggregate (FR11).
        let installed = spec.detection.is_empty()
            || spec
                .detection
                .iter()
                .any(|hint| project_dir.join(hint).exists());
        let state = if target_fenced {
            if spec.has_real_ignore {
                TargetCoverage::Covered
            } else {
                TargetCoverage::Fallback
            }
        } else if installed {
            TargetCoverage::Unfenced
        } else {
            TargetCoverage::NotInstalled
        };
        files.extend(f_stats.iter().cloned());
        targets.push(TargetStatus {
            id: spec.id.to_string(),
            tool: spec.tool.to_string(),
            state,
            installed,
            files: f_stats,
        });
    }

    // Aggregate is honest: protected unless some DETECTED target is unfenced.
    // not_installed targets are not exposure and do not break the aggregate
    // (FR11). Only an installed-but-unfenced target flips it to false.
    let all_fenced = targets.iter().all(|t| t.state != TargetCoverage::Unfenced);
    // Completeness lists only the files of installed-but-unfenced targets.
    let unfenced: Vec<FenceFileStatus> = targets
        .iter()
        .filter(|t| t.state == TargetCoverage::Unfenced)
        .flat_map(|t| t.files.iter().filter(|f| !f.fenced).cloned())
        .collect();
    let completeness = if all_fenced {
        FenceCompleteness::Complete
    } else {
        FenceCompleteness::Partial(unfenced)
    };
    Ok(FenceStatus {
        files,
        all_fenced,
        completeness,
        targets,
    })
}

// ─── Multi-Tool Propagation ──────────────────────────────────

/// AI tools known to respect project-level ignore files.
/// Each entry maps a tool name to the relative path of its ignore file.
pub const KNOWN_TOOLS: &[(&str, &str)] = &[
    ("cursor", ".cursorignore"),
    ("claude", ".claude/settings.json"),
    ("copilot", ".github/copilot-instructions.md"),
    ("aider", ".aiderignore"),
    ("windsurf", ".windsurfrules"),
    ("continue", ".continueignore"),
];

/// Propagate fence rules from `.envforgeignore` to a specific tool's
/// ignore file.  Uses symlinks on Unix (auto-updates when the source
/// changes) and file copies on Windows.
pub fn apply_tool(project_dir: &Path, tool: &str, dry_run: bool) -> Result<PathBuf, OpError> {
    let (_name, relative_path) = KNOWN_TOOLS
        .iter()
        .find(|(n, _)| *n == tool)
        .ok_or_else(|| {
            let available: Vec<&str> = KNOWN_TOOLS.iter().map(|(n, _)| *n).collect();
            OpError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "unknown tool '{}'. Available: {}",
                    tool,
                    available.join(", ")
                ),
            ))
        })?;

    let target = project_dir.join(relative_path);
    let source = project_dir.join(".envforgeignore");

    if !source.exists() {
        return Err(OpError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "run 'envforge fence' first to create .envforgeignore",
        )));
    }

    if dry_run {
        return Ok(target);
    }

    // Create parent directories if needed
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Symlink on Unix, copy on Windows
    #[cfg(unix)]
    {
        if target.exists() {
            std::fs::remove_file(&target)?;
        }
        std::os::unix::fs::symlink(&source, &target)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::copy(&source, &target)?;
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FenceTargets;
    use tempfile::TempDir;

    /// The fence marker string — first line of `CURSORIGNORE_BLOCK`, used in tests
    /// to verify that shared ignore files contain the EnvForge block.
    const FENCE_MARKER: &str = "# EnvForge secret fence";

    #[test]
    fn test_create_fence_fresh_dir() {
        let tmp = TempDir::new().unwrap();
        let result = create_fence(tmp.path(), false).unwrap();

        let expected_files: usize = registry::REGISTRY.iter().map(|s| s.files.len()).sum();
        assert_eq!(result.files_created.len(), expected_files);
        assert!(result.files_updated.is_empty());
        assert!(result.files_skipped.is_empty());

        // Verify file contents
        let ignore = std::fs::read_to_string(tmp.path().join(".envforgeignore")).unwrap();
        assert!(ignore.contains(".env"));
        assert!(ignore.contains("!.env.schema"));

        let cursorignore = std::fs::read_to_string(tmp.path().join(".cursorignore")).unwrap();
        assert!(cursorignore.contains(FENCE_MARKER));

        let cursorrules = std::fs::read_to_string(tmp.path().join(".cursorrules")).unwrap();
        assert!(cursorrules.contains("Never read .env files directly"));

        let copilot =
            std::fs::read_to_string(tmp.path().join(".github/copilot-instructions.md")).unwrap();
        assert!(copilot.contains("## Secret Safety Rules"));

        let claude = std::fs::read_to_string(tmp.path().join(".claude/settings.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&claude).unwrap();
        let deny = json["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 5);
        assert!(deny.iter().any(|v| v.as_str() == Some("Read(.env)")));
    }

    #[test]
    fn test_create_fence_idempotent() {
        let tmp = TempDir::new().unwrap();

        let expected_files: usize = registry::REGISTRY.iter().map(|s| s.files.len()).sum();

        // First run
        let r1 = create_fence(tmp.path(), false).unwrap();
        assert_eq!(r1.files_created.len(), expected_files);

        // Second run — everything should be skipped
        let r2 = create_fence(tmp.path(), false).unwrap();
        assert!(r2.files_created.is_empty());
        assert!(r2.files_updated.is_empty());
        assert_eq!(r2.files_skipped.len(), expected_files);

        // Verify files haven't been duplicated
        let cursorignore = std::fs::read_to_string(tmp.path().join(".cursorignore")).unwrap();
        assert_eq!(
            cursorignore.matches(FENCE_MARKER).count(),
            1,
            "Fence marker should appear only once"
        );
    }

    #[test]
    fn test_create_fence_merge_claude_settings() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        // Pre-existing settings with one rule
        let existing = serde_json::json!({
            "permissions": {
                "deny": ["Read(.env)"],
                "allow": ["Write(src/*)"]
            }
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let result = create_fence(tmp.path(), false).unwrap();

        // Claude settings should be updated (merged), not created
        assert!(result
            .files_updated
            .iter()
            .any(|p| p.ends_with("settings.json")));

        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Should have merged: 1 existing + 4 new = 5
        let deny = json["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 5);

        // Allow should be preserved
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 1);
    }

    // ─── Multi-Tool Propagation Tests ──────────────────────

    #[test]
    fn test_known_tools_nonempty_and_unique() {
        assert!(!KNOWN_TOOLS.is_empty());
        let names: Vec<&str> = KNOWN_TOOLS.iter().map(|(n, _)| *n).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "duplicate tool names in KNOWN_TOOLS"
        );
    }

    #[test]
    fn test_apply_tool_creates_file() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();

        let target = apply_tool(tmp.path(), "aider", false).unwrap();
        assert!(target.exists());
    }

    #[test]
    fn test_apply_tool_unknown_rejected() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();

        let result = apply_tool(tmp.path(), "nonexistent", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_tool_no_fence_rejected() {
        let tmp = TempDir::new().unwrap();
        // No fence created — should fail
        let result = apply_tool(tmp.path(), "aider", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_tool_dry_run_no_file() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();

        // Use "continue" (.continueignore) — a KNOWN_TOOLS entry not managed by the registry,
        // so create_fence does not pre-create it, and dry-run must leave it absent.
        let target = apply_tool(tmp.path(), "continue", true).unwrap();
        assert!(!target.exists());
    }

    #[test]
    fn test_create_fence_append_to_existing_cursorignore() {
        let tmp = TempDir::new().unwrap();

        // Pre-existing .cursorignore
        std::fs::write(tmp.path().join(".cursorignore"), "node_modules/\n").unwrap();

        let result = create_fence(tmp.path(), false).unwrap();
        assert!(result
            .files_updated
            .iter()
            .any(|p| p.ends_with(".cursorignore")));

        let content = std::fs::read_to_string(tmp.path().join(".cursorignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(FENCE_MARKER));
    }

    // ─── FenceConfig / FenceTarget / Resolution Tests ──────────────

    /// Absent config → all five targets enabled (NFR5 / AC3 from Story 1.1).
    #[test]
    fn test_fence_config_absent_all_enabled() {
        let cfg = FenceConfig::default();
        for target in FenceTarget::all() {
            assert!(
                cfg.targets.is_enabled(target),
                "target {:?} should default to enabled",
                target
            );
        }
    }

    /// Per-target skip: each target can be disabled individually.
    /// Tests that exactly the disabled target is skipped and the rest are created.
    #[test]
    fn test_fence_config_skip_envforgeignore() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::Envforgeignore, false);
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(
            !tmp.path().join(".envforgeignore").exists(),
            "disabled target must not be created"
        );
        // Envforgeignore spec has 1 file; remaining total files are created
        let total_files: usize = registry::REGISTRY.iter().map(|s| s.files.len()).sum();
        let envforgeignore_files = registry::spec_for(FenceTarget::Envforgeignore).files.len();
        assert_eq!(
            result.files_created.len(),
            total_files - envforgeignore_files
        );
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with(".envforgeignore")));
    }

    #[test]
    fn test_fence_config_skip_cursor_ignore() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::CursorIgnore, false);
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(!tmp.path().join(".cursorignore").exists());
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with(".cursorignore")));
    }

    #[test]
    fn test_fence_config_skip_cursor_rules() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::CursorRules, false);
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(!tmp.path().join(".cursorrules").exists());
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with(".cursorrules")));
    }

    #[test]
    fn test_fence_config_skip_copilot() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::Copilot, false);
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(!tmp.path().join(".github/copilot-instructions.md").exists());
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with("copilot-instructions.md")));
    }

    #[test]
    fn test_fence_config_skip_claude_code() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::ClaudeCode, false);
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(!tmp.path().join(".claude/settings.json").exists());
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with("settings.json")));
    }

    /// Byte-identical default: no config → same files as current behavior (NFR5).
    #[test]
    fn test_fence_config_default_byte_identical_output() {
        let tmp = TempDir::new().unwrap();
        let cfg = FenceConfig::default();
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        let expected_files: usize = registry::REGISTRY.iter().map(|s| s.files.len()).sum();
        assert_eq!(
            result.files_created.len(),
            expected_files,
            "default config must create all registry files"
        );
        assert!(result.files_updated.is_empty());
        // The skipped list should be empty (all enabled, fresh dir)
        assert!(result.files_skipped.is_empty());
    }

    /// check_fence_status_with: disabled target does not make status Partial.
    #[test]
    fn test_check_fence_status_disabled_target_not_partial() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::Copilot, false);

        // Create fence without copilot
        create_fence_with(tmp.path(), false, &cfg).unwrap();
        let status = check_fence_status_with(tmp.path(), &cfg).unwrap();

        // All enabled targets (4) should be fenced
        assert!(
            status.all_fenced,
            "disabled target must not make status Partial"
        );
        assert!(matches!(status.completeness, FenceCompleteness::Complete));
        // One per-target entry per enabled target (copilot excluded).
        assert_eq!(status.targets.len(), registry::REGISTRY.len() - 1);
        assert!(!status.targets.iter().any(|t| t.id == "copilot"));
    }

    /// Stale disabled file must not flip status to Partial.
    #[test]
    fn test_check_fence_status_stale_disabled_file_not_partial() {
        let tmp = TempDir::new().unwrap();

        // Create an unfenced copilot file (stale) before configuring fence
        let github_dir = tmp.path().join(".github");
        std::fs::create_dir_all(&github_dir).unwrap();
        std::fs::write(github_dir.join("copilot-instructions.md"), "# My docs\n").unwrap();

        // Config: copilot disabled
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::Copilot, false);
        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let status = check_fence_status_with(tmp.path(), &cfg).unwrap();
        assert!(
            status.all_fenced,
            "stale disabled file must not cause Partial status"
        );
    }

    /// Enabled-missing target makes status Partial.
    #[test]
    fn test_check_fence_status_enabled_missing_is_partial() {
        let tmp = TempDir::new().unwrap();
        let cfg = FenceConfig::default();
        // Don't create any files; all enabled but none exist
        let status = check_fence_status_with(tmp.path(), &cfg).unwrap();
        assert!(!status.all_fenced);
        assert!(matches!(status.completeness, FenceCompleteness::Partial(_)));
    }

    /// resolve_fence_targets: source is Default when all enabled.
    #[test]
    fn test_resolve_fence_targets_all_default() {
        let cfg = FenceConfig::default();
        let resolved = resolve_fence_targets(&cfg);
        assert_eq!(resolved.len(), registry::REGISTRY.len());
        for r in &resolved {
            assert!(r.enabled);
            assert_eq!(r.source, ConfigSource::Default);
        }
    }

    /// resolve_fence_targets: disabled target reports source=Global.
    #[test]
    fn test_resolve_fence_targets_disabled_is_global() {
        let mut cfg = FenceConfig::default();
        cfg.targets.set_enabled(FenceTarget::ClaudeCode, false);
        let resolved = resolve_fence_targets(&cfg);
        let claude = resolved
            .iter()
            .find(|r| r.target == FenceTarget::ClaudeCode)
            .unwrap();
        assert!(!claude.enabled);
        assert_eq!(claude.source, ConfigSource::Global);
    }

    /// FenceTarget::all() returns all distinct targets (count == registry length).
    #[test]
    fn test_fence_target_all_unique() {
        let all = FenceTarget::all();
        assert_eq!(all.len(), registry::REGISTRY.len());
        let strs: Vec<&str> = all.iter().map(|t| t.as_str()).collect();
        let unique: std::collections::HashSet<_> = strs.iter().collect();
        assert_eq!(
            unique.len(),
            registry::REGISTRY.len(),
            "all targets must have unique string IDs"
        );
    }

    /// Story 1.5: remove_fence operates on all targets regardless of config.
    #[test]
    fn test_remove_fence_config_independent_cleans_disabled_target() {
        let tmp = TempDir::new().unwrap();

        // Create with all enabled first (simulating a prior full fence)
        let full_cfg = FenceConfig::default();
        create_fence_with(tmp.path(), false, &full_cfg).unwrap();

        // Now disable copilot in config — but remove_fence must still clean it
        let remove_result = super::remove_fence(tmp.path(), false).unwrap();
        // copilot file should have been cleaned (it exists and has fence content)
        assert!(
            remove_result
                .files_removed
                .iter()
                .chain(remove_result.files_updated.iter())
                .any(|p| p.to_str().unwrap_or("").contains("copilot")),
            "remove_fence must clean copilot even if config disables it"
        );
    }

    /// is_enabled / set_enabled round-trip for each target.
    #[test]
    fn test_fence_targets_set_enabled_roundtrip() {
        for target in FenceTarget::all() {
            let mut targets = FenceTargets::default();
            // Disable
            targets.set_enabled(target, false);
            assert!(
                !targets.is_enabled(target),
                "set_enabled false failed for {:?}",
                target
            );
            // Re-enable
            targets.set_enabled(target, true);
            assert!(
                targets.is_enabled(target),
                "set_enabled true failed for {:?}",
                target
            );
        }
    }

    // ─── Story 1.2: FileKind-dispatched writer/stripper round-trip tests ──────

    /// NFR-S4 / R1: Shared Ignore file round-trip preserves user content.
    /// Pre-write `.cursorignore` with user content, create fence (appends block),
    /// remove fence, assert user content survives and fence block is gone.
    #[test]
    fn test_shared_block_roundtrip_preserves_user_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".cursorignore");

        // Pre-existing user content
        std::fs::write(&path, "node_modules/\n").unwrap();

        create_fence(tmp.path(), false).unwrap();

        let after_create = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_create.contains("node_modules/"),
            "user content must survive create"
        );
        assert!(
            after_create.contains(FENCE_MARKER),
            "fence block must be present after create"
        );

        remove_fence(tmp.path(), false).unwrap();

        let after_remove = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_remove.contains("node_modules/"),
            "user content must survive remove"
        );
        assert!(
            !after_remove.contains(FENCE_MARKER),
            "fence block must be gone after remove"
        );
    }

    /// NFR-R1: Shared Rules (markdown section) round-trip preserves user content.
    /// Pre-write `.github/copilot-instructions.md` with a user section, create fence
    /// (appends Secret Safety Rules), remove fence, assert user section preserved.
    #[test]
    fn test_rules_section_roundtrip_preserves_user_content() {
        let tmp = TempDir::new().unwrap();
        let github_dir = tmp.path().join(".github");
        std::fs::create_dir_all(&github_dir).unwrap();
        let path = github_dir.join("copilot-instructions.md");

        std::fs::write(&path, "## My Rules\n- foo\n").unwrap();

        create_fence(tmp.path(), false).unwrap();

        let after_create = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_create.contains("## My Rules"),
            "user section must survive create"
        );
        assert!(
            after_create.contains("## Secret Safety Rules"),
            "fence section must be present after create"
        );

        remove_fence(tmp.path(), false).unwrap();

        let after_remove = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_remove.contains("## My Rules"),
            "user section must survive remove"
        );
        assert!(
            after_remove.contains("- foo"),
            "user content must survive remove"
        );
        assert!(
            !after_remove.contains("## Secret Safety Rules"),
            "fence section must be gone after remove"
        );
    }

    /// NFR-R1 / R3: DenyRule round-trip preserves unrelated keys.
    /// Pre-write `.claude/settings.json` with permissions.allow + an unrelated deny rule,
    /// create fence, remove fence, assert allow + unrelated deny preserved, envforge deny rules gone.
    #[test]
    fn test_deny_rule_roundtrip_preserves_other_keys() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let existing = serde_json::json!({
            "permissions": {
                "allow": ["Write(src/*)"],
                "deny": ["Read(supersecret.txt)"]
            }
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        create_fence(tmp.path(), false).unwrap();

        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = json["permissions"]["deny"].as_array().unwrap();
        assert!(
            deny.iter()
                .any(|v| v.as_str() == Some("Read(supersecret.txt)")),
            "unrelated deny rule must survive create"
        );
        assert!(
            deny.iter().any(|v| v.as_str() == Some("Read(.env)")),
            "envforge deny rule must be present after create"
        );
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 1, "allow must survive create");

        remove_fence(tmp.path(), false).unwrap();

        let content2 = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let json2: serde_json::Value = serde_json::from_str(&content2).unwrap();
        let deny2 = json2["permissions"]["deny"].as_array().unwrap();
        assert!(
            deny2
                .iter()
                .any(|v| v.as_str() == Some("Read(supersecret.txt)")),
            "unrelated deny rule must survive remove"
        );
        assert!(
            !deny2.iter().any(|v| v.as_str() == Some("Read(.env)")),
            "envforge deny rule must be gone after remove"
        );
        let allow2 = json2["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow2.len(), 1, "allow must survive remove");
    }

    /// NFR-R1: FullyOwned file is deleted on remove_fence.
    #[test]
    fn test_fully_owned_deleted_on_remove() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();

        let path = tmp.path().join(".envforgeignore");
        assert!(path.exists(), ".envforgeignore must exist after create");

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !path.exists(),
            ".envforgeignore must be deleted after remove_fence"
        );
    }

    /// NFR-R3: create_fence is idempotent — file bytes identical after two runs.
    #[test]
    fn test_create_fence_idempotent_no_diff() {
        let tmp = TempDir::new().unwrap();

        create_fence(tmp.path(), false).unwrap();

        // Capture bytes after first run
        let snap1: Vec<(std::path::PathBuf, Vec<u8>)> = [
            ".envforgeignore",
            ".cursorignore",
            ".cursorrules",
            ".github/copilot-instructions.md",
            ".claude/settings.json",
        ]
        .iter()
        .map(|p| {
            let full = tmp.path().join(p);
            let bytes = std::fs::read(&full).unwrap();
            (full, bytes)
        })
        .collect();

        create_fence(tmp.path(), false).unwrap();

        // Capture bytes after second run and compare
        for (path, bytes_before) in &snap1 {
            let bytes_after = std::fs::read(path).unwrap();
            assert_eq!(
                bytes_before, &bytes_after,
                "file {:?} changed on second create_fence run (not idempotent)",
                path
            );
        }
    }

    // ─── Story 1.3: New tool round-trip tests ─────────────────────────────────

    /// Windsurf: create_fence writes .codeiumignore + .windsurf/rules/envforge.md;
    /// remove_fence cleans both.
    #[test]
    fn test_windsurf_fence_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        // Only create windsurf so the test is focused
        for t in FenceTarget::all() {
            if t != FenceTarget::Windsurf {
                cfg.targets.set_enabled(t, false);
            }
        }

        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let codeiumignore = tmp.path().join(".codeiumignore");
        let windsurf_rules = tmp.path().join(".windsurf/rules/envforge.md");
        assert!(codeiumignore.exists(), ".codeiumignore must be created");
        assert!(
            windsurf_rules.exists(),
            ".windsurf/rules/envforge.md must be created"
        );
        let ignore_content = std::fs::read_to_string(&codeiumignore).unwrap();
        assert!(
            ignore_content.contains(FENCE_MARKER),
            ".codeiumignore must contain fence marker"
        );
        let rules_content = std::fs::read_to_string(&windsurf_rules).unwrap();
        assert!(
            rules_content.contains("Never read .env files directly"),
            ".windsurf/rules/envforge.md must contain rules text"
        );

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !codeiumignore.exists(),
            ".codeiumignore must be removed after remove_fence"
        );
        assert!(
            !windsurf_rules.exists(),
            ".windsurf/rules/envforge.md must be removed after remove_fence"
        );
    }

    /// Cline: create_fence writes .clineignore + .clinerules;
    /// remove_fence cleans both.
    #[test]
    fn test_cline_fence_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::Cline {
                cfg.targets.set_enabled(t, false);
            }
        }

        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let clineignore = tmp.path().join(".clineignore");
        let clinerules = tmp.path().join(".clinerules");
        assert!(clineignore.exists(), ".clineignore must be created");
        assert!(clinerules.exists(), ".clinerules must be created");
        let ignore_content = std::fs::read_to_string(&clineignore).unwrap();
        assert!(
            ignore_content.contains(FENCE_MARKER),
            ".clineignore must contain fence marker"
        );
        let rules_content = std::fs::read_to_string(&clinerules).unwrap();
        assert!(
            rules_content.contains("Never read .env files directly"),
            ".clinerules must contain rules text"
        );

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !clineignore.exists(),
            ".clineignore must be removed after remove_fence"
        );
        assert!(
            !clinerules.exists(),
            ".clinerules must be removed after remove_fence"
        );
    }

    /// Aider: create_fence writes .aiderignore; remove_fence cleans it.
    #[test]
    fn test_aider_fence_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::Aider {
                cfg.targets.set_enabled(t, false);
            }
        }

        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let aiderignore = tmp.path().join(".aiderignore");
        assert!(aiderignore.exists(), ".aiderignore must be created");
        let content = std::fs::read_to_string(&aiderignore).unwrap();
        assert!(
            content.contains(FENCE_MARKER),
            ".aiderignore must contain fence marker"
        );

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !aiderignore.exists(),
            ".aiderignore must be removed after remove_fence"
        );
    }

    /// Gemini: create_fence writes .geminiignore + GEMINI.md; remove_fence cleans both.
    #[test]
    fn test_gemini_fence_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::Gemini {
                cfg.targets.set_enabled(t, false);
            }
        }

        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let geminiignore = tmp.path().join(".geminiignore");
        let gemini_md = tmp.path().join("GEMINI.md");
        assert!(geminiignore.exists(), ".geminiignore must be created");
        assert!(gemini_md.exists(), "GEMINI.md must be created");
        let ignore_content = std::fs::read_to_string(&geminiignore).unwrap();
        assert!(
            ignore_content.contains(FENCE_MARKER),
            ".geminiignore must contain fence marker"
        );
        let rules_content = std::fs::read_to_string(&gemini_md).unwrap();
        assert!(
            rules_content.contains("Never read .env files directly"),
            "GEMINI.md must contain rules text"
        );

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !geminiignore.exists(),
            ".geminiignore must be removed after remove_fence"
        );
        assert!(
            !gemini_md.exists(),
            "GEMINI.md must be removed after remove_fence"
        );
    }

    /// Story 1.6: per-target coverage states — covered vs fallback vs unfenced.
    #[test]
    fn test_target_coverage_states() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();
        let status = check_fence_status(tmp.path()).unwrap();

        let by_id = |id: &str| {
            status
                .targets
                .iter()
                .find(|t| t.id == id)
                .unwrap_or_else(|| panic!("target {id} missing"))
                .state
        };

        // Real-ignore tool, fenced → Covered.
        assert_eq!(by_id("cursor_ignore"), TargetCoverage::Covered);
        assert_eq!(by_id("aider"), TargetCoverage::Covered);
        // No-ignore tools, protected via rules/deny → Fallback (honest, FR2b).
        assert_eq!(by_id("copilot"), TargetCoverage::Fallback);
        assert_eq!(by_id("claude_code"), TargetCoverage::Fallback);
        assert_eq!(by_id("agents_md"), TargetCoverage::Fallback);
        // Multi-file target fully fenced → Covered (windsurf has a real ignore).
        assert_eq!(by_id("windsurf"), TargetCoverage::Covered);
    }

    /// Story 1.6 / FR11: aggregate "AI BLOCKED" only when no target is unfenced.
    #[test]
    fn test_aggregate_protected_only_when_all_covered() {
        let tmp = TempDir::new().unwrap();
        // Empty dir: no AI tools detected → tool targets are NotInstalled, but
        // EnvForge's own .envforgeignore is always-applicable and unfenced, so
        // the project is not yet protected.
        let status = check_fence_status(tmp.path()).unwrap();
        assert!(!status.all_fenced, "empty dir must not be protected");
        let envforge = status
            .targets
            .iter()
            .find(|t| t.id == "envforgeignore")
            .unwrap();
        assert_eq!(envforge.state, TargetCoverage::Unfenced);
        // Tools with detection hints are not present → NotInstalled.
        assert_eq!(
            status
                .targets
                .iter()
                .find(|t| t.id == "cursor_ignore")
                .unwrap()
                .state,
            TargetCoverage::NotInstalled
        );

        // Fence everything → all covered/fallback → protected.
        create_fence(tmp.path(), false).unwrap();
        let status = check_fence_status(tmp.path()).unwrap();
        assert!(status.all_fenced, "fully fenced must be protected");
        assert!(status
            .targets
            .iter()
            .all(|t| matches!(t.state, TargetCoverage::Covered | TargetCoverage::Fallback)));
    }

    /// Story 1.7: a tool present in the workspace but not fenced is the
    /// dangerous installed-but-unfenced case — distinct from not_installed.
    #[test]
    fn test_detection_installed_but_unfenced() {
        let tmp = TempDir::new().unwrap();
        // Simulate Cursor being installed (its detection hint) without fencing.
        std::fs::create_dir_all(tmp.path().join(".cursor")).unwrap();

        let status = check_fence_status(tmp.path()).unwrap();
        let cursor = status
            .targets
            .iter()
            .find(|t| t.id == "cursor_ignore")
            .unwrap();
        assert!(cursor.installed, "Cursor must be detected as installed");
        assert_eq!(
            cursor.state,
            TargetCoverage::Unfenced,
            "installed-but-unfenced tool must be Unfenced, not NotInstalled"
        );
        assert!(
            !status.all_fenced,
            "an installed-but-unfenced tool breaks the aggregate (FR8/FR11)"
        );

        // A tool with no hints present stays NotInstalled and does NOT break it.
        let claude = status
            .targets
            .iter()
            .find(|t| t.id == "claude_code")
            .unwrap();
        assert!(!claude.installed);
        assert_eq!(claude.state, TargetCoverage::NotInstalled);
    }

    /// Story 1.7 / FR11: not_installed tools do not break the aggregate. A repo
    /// with one fenced tool and the rest absent is protected.
    #[test]
    fn test_not_installed_does_not_break_aggregate() {
        let tmp = TempDir::new().unwrap();
        // Fence only envforgeignore + cursor; pretend nothing else installed.
        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if !matches!(
                t,
                FenceTarget::Envforgeignore | FenceTarget::CursorIgnore | FenceTarget::CursorRules
            ) {
                cfg.targets.set_enabled(t, false);
            }
        }
        create_fence_with(tmp.path(), false, &cfg).unwrap();
        // Status over the SAME reduced config: enabled targets all fenced.
        let status = check_fence_status_with(tmp.path(), &cfg).unwrap();
        assert!(
            status.all_fenced,
            "all enabled targets fenced → protected even though other tools exist in the registry"
        );
    }

    /// Story 1.6: a multi-file target needs ALL its files fenced to be covered.
    #[test]
    fn test_multifile_target_unfenced_if_one_file_missing() {
        let tmp = TempDir::new().unwrap();
        create_fence(tmp.path(), false).unwrap();
        // Delete one of windsurf's two files (the rules file).
        std::fs::remove_file(tmp.path().join(".windsurf/rules/envforge.md")).unwrap();

        let status = check_fence_status(tmp.path()).unwrap();
        let windsurf = status.targets.iter().find(|t| t.id == "windsurf").unwrap();
        assert_eq!(
            windsurf.state,
            TargetCoverage::Unfenced,
            "missing one file of a multi-file target → unfenced"
        );
        assert!(
            !status.all_fenced,
            "one unfenced target breaks the aggregate"
        );
    }

    /// Story 1.5 / NFR-R4: one target failing does not abort the others.
    /// A directory placed where a fence file should go makes that target fail;
    /// the rest must still be written and the failure captured (not swallowed).
    #[test]
    fn test_fence_partial_failure_isolated() {
        let tmp = TempDir::new().unwrap();
        // Block .cursorignore by creating a directory at its path → write fails.
        std::fs::create_dir_all(tmp.path().join(".cursorignore")).unwrap();

        let result = create_fence(tmp.path(), false).unwrap();

        // The blocked target is reported as failed...
        assert!(
            result
                .files_failed
                .iter()
                .any(|(p, _)| p.ends_with(".cursorignore")),
            "blocked target must be in files_failed"
        );
        // ...but other targets were still written (loop did not abort).
        assert!(
            tmp.path().join(".envforgeignore").exists(),
            "other targets must still be fenced despite one failure"
        );
        assert!(
            !result.files_created.is_empty(),
            "partial success expected, not total abort"
        );
    }

    /// Story 1.5 / NFR-P1: a full fence pass completes well under 500 ms.
    #[test]
    fn test_fence_full_pass_under_budget() {
        let tmp = TempDir::new().unwrap();
        let start = std::time::Instant::now();
        create_fence(tmp.path(), false).unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "full fence pass took {elapsed:?}, budget is 500ms (NFR-P1)"
        );
    }

    /// Story 1.4: AGENTS.md cross-tool target round-trips and preserves user content.
    #[test]
    fn test_agents_md_fence_roundtrip_preserves_user_content() {
        let tmp = TempDir::new().unwrap();
        // Pre-existing user AGENTS.md with their own section.
        let agents = tmp.path().join("AGENTS.md");
        std::fs::write(&agents, "# Project Agents\n\n## Build\n- run cargo build\n").unwrap();

        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::AgentsMd {
                cfg.targets.set_enabled(t, false);
            }
        }
        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let content = std::fs::read_to_string(&agents).unwrap();
        assert!(
            content.contains("## EnvForge Secret Safety"),
            "AGENTS.md must gain the EnvForge section"
        );
        assert!(
            content.contains("## Build") && content.contains("run cargo build"),
            "user section must be preserved on create"
        );

        remove_fence(tmp.path(), false).unwrap();
        let after = std::fs::read_to_string(&agents).unwrap();
        assert!(
            !after.contains("## EnvForge Secret Safety"),
            "EnvForge section must be stripped on remove"
        );
        assert!(
            after.contains("## Build") && after.contains("run cargo build"),
            "user section must survive remove (FR3 / NFR-S4)"
        );
    }

    /// Story 1.4: Amazon Q rules fallback (no ignore file) round-trips.
    #[test]
    fn test_amazon_q_fence_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::AmazonQ {
                cfg.targets.set_enabled(t, false);
            }
        }
        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let rules = tmp.path().join(".amazonq/rules/envforge.md");
        assert!(rules.exists(), ".amazonq/rules/envforge.md must be created");
        assert!(std::fs::read_to_string(&rules)
            .unwrap()
            .contains("## Secret Safety Rules"));

        remove_fence(tmp.path(), false).unwrap();
        assert!(
            !rules.exists(),
            ".amazonq rules file must be removed after remove_fence"
        );
    }

    /// Story 1.4 / FR2b: no-ignore-file tools are marked has_real_ignore=false
    /// so status reports a fallback, never a false "covered".
    #[test]
    fn test_fallback_tools_marked_no_real_ignore() {
        use registry::spec_for;
        for t in [
            FenceTarget::Copilot,
            FenceTarget::CursorRules,
            FenceTarget::ClaudeCode,
            FenceTarget::AgentsMd,
            FenceTarget::AmazonQ,
        ] {
            assert!(
                !spec_for(t).has_real_ignore,
                "{:?} has no real ignore mechanism — must be marked fallback",
                t
            );
        }
        // Tools that DO have a native ignore file.
        for t in [
            FenceTarget::CursorIgnore,
            FenceTarget::Windsurf,
            FenceTarget::Cline,
            FenceTarget::Aider,
            FenceTarget::Gemini,
        ] {
            assert!(
                spec_for(t).has_real_ignore,
                "{:?} has a native ignore file",
                t
            );
        }
    }

    /// FR3 / NFR-S4: .codeiumignore preserves user content across create + remove.
    #[test]
    fn test_codeiumignore_preserves_user_content() {
        let tmp = TempDir::new().unwrap();

        // Pre-write .codeiumignore with user content
        std::fs::write(tmp.path().join(".codeiumignore"), "dist/\n").unwrap();

        let mut cfg = FenceConfig::default();
        for t in FenceTarget::all() {
            if t != FenceTarget::Windsurf {
                cfg.targets.set_enabled(t, false);
            }
        }

        create_fence_with(tmp.path(), false, &cfg).unwrap();

        let after_create = std::fs::read_to_string(tmp.path().join(".codeiumignore")).unwrap();
        assert!(
            after_create.contains("dist/"),
            "user content must survive create_fence"
        );
        assert!(
            after_create.contains(FENCE_MARKER),
            "fence marker must be present after create_fence"
        );

        remove_fence(tmp.path(), false).unwrap();

        let after_remove = std::fs::read_to_string(tmp.path().join(".codeiumignore")).unwrap();
        assert!(
            after_remove.contains("dist/"),
            "user content must survive remove_fence"
        );
        assert!(
            !after_remove.contains(FENCE_MARKER),
            "fence marker must be gone after remove_fence"
        );
    }
}
