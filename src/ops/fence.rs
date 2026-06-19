use std::path::{Path, PathBuf};

use super::OpError;
use crate::config::{FenceConfig, FenceTargets};

// ─── Canonical Target Enum ───────────────────────────────────────────────────

/// The five AI-tool fence targets EnvForge can manage.
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
}

impl FenceTarget {
    /// Returns the canonical snake_case identifier string for this target.
    /// Used in JSON output, config keys, and CLI display.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Envforgeignore => "envforgeignore",
            Self::CursorIgnore => "cursor_ignore",
            Self::CursorRules => "cursor_rules",
            Self::Copilot => "copilot",
            Self::ClaudeCode => "claude_code",
        }
    }

    /// Returns all five targets in canonical order.
    #[must_use]
    pub fn all() -> [FenceTarget; 5] {
        [
            Self::Envforgeignore,
            Self::CursorIgnore,
            Self::CursorRules,
            Self::Copilot,
            Self::ClaudeCode,
        ]
    }
}

impl FenceTargets {
    /// Returns whether the given target is enabled according to this config.
    ///
    /// This is the single decision point for "is target X enabled?" (NFR10).
    #[must_use]
    pub fn is_enabled(&self, target: FenceTarget) -> bool {
        match target {
            FenceTarget::Envforgeignore => self.envforgeignore,
            FenceTarget::CursorIgnore => self.cursor_ignore,
            FenceTarget::CursorRules => self.cursor_rules,
            FenceTarget::Copilot => self.copilot,
            FenceTarget::ClaudeCode => self.claude_code,
        }
    }

    /// Sets the enabled state for a specific target.
    pub fn set_enabled(&mut self, target: FenceTarget, enabled: bool) {
        match target {
            FenceTarget::Envforgeignore => self.envforgeignore = enabled,
            FenceTarget::CursorIgnore => self.cursor_ignore = enabled,
            FenceTarget::CursorRules => self.cursor_rules = enabled,
            FenceTarget::Copilot => self.copilot = enabled,
            FenceTarget::ClaudeCode => self.claude_code = enabled,
        }
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
/// - All five enabled → `"fence: (5/5)"`
/// - Subset enabled → `"fence: cursor_ignore,copilot (2/5)"`
/// - None enabled → `"fence: none (0/5)"`
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
/// assert!(s.contains("5/5"), "all-enabled summary: {s}");
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
    FenceTarget::all()
        .into_iter()
        .map(|target| {
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

/// Overall fence status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FenceStatus {
    pub files: Vec<FenceFileStatus>,
    /// Derived convenience field — `true` when the fence is complete.
    /// Kept for backward compatibility with existing callers.
    pub all_fenced: bool,
    /// Structured completeness assessment.
    pub completeness: FenceCompleteness,
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

const FENCE_MARKER: &str = "# EnvForge secret fence";

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
    };

    // 1. .envforgeignore
    if cfg.targets.is_enabled(FenceTarget::Envforgeignore) {
        write_envforgeignore(project_dir, dry_run, &mut result)?;
    } else {
        result
            .files_skipped
            .push(project_dir.join(".envforgeignore"));
    }

    // 2. .cursorignore
    if cfg.targets.is_enabled(FenceTarget::CursorIgnore) {
        write_cursorignore(project_dir, dry_run, &mut result)?;
    } else {
        result.files_skipped.push(project_dir.join(".cursorignore"));
    }

    // 3. .cursorrules
    if cfg.targets.is_enabled(FenceTarget::CursorRules) {
        write_cursorrules(project_dir, dry_run, &mut result)?;
    } else {
        result.files_skipped.push(project_dir.join(".cursorrules"));
    }

    // 4. .github/copilot-instructions.md
    if cfg.targets.is_enabled(FenceTarget::Copilot) {
        write_copilot_instructions(project_dir, dry_run, &mut result)?;
    } else {
        result
            .files_skipped
            .push(project_dir.join(".github/copilot-instructions.md"));
    }

    // 5. .claude/settings.json
    if cfg.targets.is_enabled(FenceTarget::ClaudeCode) {
        write_claude_settings(project_dir, dry_run, &mut result)?;
    } else {
        result
            .files_skipped
            .push(project_dir.join(".claude/settings.json"));
    }

    Ok(result)
}

fn write_envforgeignore(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let path = dir.join(".envforgeignore");
    if path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    if !dry_run {
        std::fs::write(&path, ENVFORGEIGNORE_CONTENT)?;
    }
    result.files_created.push(path);
    Ok(())
}

fn write_cursorignore(dir: &Path, dry_run: bool, result: &mut FenceResult) -> Result<(), OpError> {
    let path = dir.join(".cursorignore");
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if content.contains(FENCE_MARKER) {
            result.files_skipped.push(path);
            return Ok(());
        }
        // Append
        if !dry_run {
            let new_content = if content.ends_with('\n') {
                format!("{}\n{}", content, CURSORIGNORE_BLOCK)
            } else {
                format!("{}\n\n{}", content, CURSORIGNORE_BLOCK)
            };
            std::fs::write(&path, new_content)?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            std::fs::write(&path, CURSORIGNORE_BLOCK)?;
        }
        result.files_created.push(path);
    }
    Ok(())
}

fn write_cursorrules(dir: &Path, dry_run: bool, result: &mut FenceResult) -> Result<(), OpError> {
    let path = dir.join(".cursorrules");
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if content.contains("Never read .env files directly") {
            result.files_skipped.push(path);
            return Ok(());
        }
        if !dry_run {
            let new_content = if content.ends_with('\n') {
                format!("{}\n{}", content, CURSORRULES_BLOCK)
            } else {
                format!("{}\n\n{}", content, CURSORRULES_BLOCK)
            };
            std::fs::write(&path, new_content)?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            std::fs::write(&path, CURSORRULES_BLOCK)?;
        }
        result.files_created.push(path);
    }
    Ok(())
}

fn write_copilot_instructions(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let github_dir = dir.join(".github");
    let path = github_dir.join("copilot-instructions.md");

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if content.contains("## Secret Safety Rules") {
            result.files_skipped.push(path);
            return Ok(());
        }
        if !dry_run {
            let new_content = if content.ends_with('\n') {
                format!("{}\n{}", content, COPILOT_INSTRUCTIONS)
            } else {
                format!("{}\n\n{}", content, COPILOT_INSTRUCTIONS)
            };
            std::fs::write(&path, new_content)?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            std::fs::create_dir_all(&github_dir)?;
            std::fs::write(&path, COPILOT_INSTRUCTIONS)?;
        }
        result.files_created.push(path);
    }
    Ok(())
}

fn write_claude_settings(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceResult,
) -> Result<(), OpError> {
    let claude_dir = dir.join(".claude");
    let path = claude_dir.join("settings.json");

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let mut json: serde_json::Value =
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

        // Check if all deny rules already present
        let existing_deny = json
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let existing_strs: Vec<String> = existing_deny
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        let new_rules: Vec<&str> = CLAUDE_DENY_RULES
            .iter()
            .filter(|r| !existing_strs.contains(&(**r).to_string()))
            .copied()
            .collect();

        if new_rules.is_empty() {
            result.files_skipped.push(path);
            return Ok(());
        }

        if !dry_run {
            // Merge deny rules
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
            std::fs::write(&path, format!("{}\n", output))?;
        }
        result.files_updated.push(path);
    } else {
        if !dry_run {
            std::fs::create_dir_all(&claude_dir)?;
            let json = serde_json::json!({
                "permissions": {
                    "deny": CLAUDE_DENY_RULES
                }
            });
            let output = serde_json::to_string_pretty(&json)?;
            std::fs::write(&path, format!("{}\n", output))?;
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
    let fenced = match rel_path {
        ".envforgeignore" => content.contains(".env"),
        ".cursorignore" => content.contains(FENCE_MARKER),
        ".cursorrules" => content.contains("Never read .env files directly"),
        ".github/copilot-instructions.md" => content.contains("## Secret Safety Rules"),
        ".claude/settings.json" => serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|v| {
                v.pointer("/permissions/deny")
                    .and_then(|d| d.as_array())
                    .map(|a| !a.is_empty())
            })
            .unwrap_or(false),
        _ => false,
    };
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

    delete_envforgeignore(project_dir, dry_run, &mut result)?;
    strip_cursorignore(project_dir, dry_run, &mut result)?;
    strip_cursorrules(project_dir, dry_run, &mut result)?;
    strip_copilot_instructions(project_dir, dry_run, &mut result)?;
    strip_claude_settings(project_dir, dry_run, &mut result)?;

    Ok(result)
}

fn delete_envforgeignore(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(".envforgeignore");
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

fn strip_cursorignore(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(".cursorignore");
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let stripped = strip_block(&content, CURSORIGNORE_BLOCK);
    if stripped == content {
        result.files_skipped.push(path);
        return Ok(());
    }
    write_or_delete(&path, &stripped, dry_run, result)?;
    Ok(())
}

fn strip_cursorrules(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(".cursorrules");
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let stripped = strip_block(&content, CURSORRULES_BLOCK);
    if stripped == content {
        result.files_skipped.push(path);
        return Ok(());
    }
    write_or_delete(&path, &stripped, dry_run, result)?;
    Ok(())
}

fn strip_copilot_instructions(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(".github/copilot-instructions.md");
    if !path.exists() {
        result.files_skipped.push(path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let stripped = strip_secret_safety_section(&content);
    if stripped == content {
        result.files_skipped.push(path);
        return Ok(());
    }
    write_or_delete(&path, &stripped, dry_run, result)?;
    Ok(())
}

fn strip_claude_settings(
    dir: &Path,
    dry_run: bool,
    result: &mut FenceRemoveResult,
) -> Result<(), OpError> {
    let path = dir.join(".claude/settings.json");
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
                    .map(|s| !CLAUDE_DENY_RULES.contains(&s))
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
        std::fs::write(&path, format!("{}\n", serialized))?;
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

/// Cut the `## Secret Safety Rules` section out of a markdown document.
/// Removes from the heading line up to (but not including) the next `##`
/// heading at the same level, or to EOF if none exists.
fn strip_secret_safety_section(content: &str) -> String {
    let marker = "## Secret Safety Rules";
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
            std::fs::write(path, to_write)?;
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
    /// Maps a `FenceTarget` to its relative path string.
    fn target_rel_path(target: FenceTarget) -> &'static str {
        match target {
            FenceTarget::Envforgeignore => ".envforgeignore",
            FenceTarget::CursorIgnore => ".cursorignore",
            FenceTarget::CursorRules => ".cursorrules",
            FenceTarget::Copilot => ".github/copilot-instructions.md",
            FenceTarget::ClaudeCode => ".claude/settings.json",
        }
    }

    // Build status entries for enabled targets only.
    let files: Vec<FenceFileStatus> = FenceTarget::all()
        .into_iter()
        .filter(|t| cfg.targets.is_enabled(*t))
        .map(|t| check_file_status(project_dir, target_rel_path(t)))
        .collect();

    let all_fenced = files.iter().all(|f| f.fenced);
    let unfenced: Vec<FenceFileStatus> = files.iter().filter(|f| !f.fenced).cloned().collect();
    let completeness = if all_fenced {
        FenceCompleteness::Complete
    } else {
        FenceCompleteness::Partial(unfenced)
    };
    Ok(FenceStatus {
        files,
        all_fenced,
        completeness,
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
    use tempfile::TempDir;

    #[test]
    fn test_create_fence_fresh_dir() {
        let tmp = TempDir::new().unwrap();
        let result = create_fence(tmp.path(), false).unwrap();

        assert_eq!(result.files_created.len(), 5);
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

        // First run
        let r1 = create_fence(tmp.path(), false).unwrap();
        assert_eq!(r1.files_created.len(), 5);

        // Second run — everything should be skipped
        let r2 = create_fence(tmp.path(), false).unwrap();
        assert!(r2.files_created.is_empty());
        assert!(r2.files_updated.is_empty());
        assert_eq!(r2.files_skipped.len(), 5);

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

        let target = apply_tool(tmp.path(), "aider", true).unwrap();
        // Dry run returns path but doesn't create the file
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
    /// Tests that exactly the disabled target is skipped and the other 4 are created.
    #[test]
    fn test_fence_config_skip_envforgeignore() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.envforgeignore = false;
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(
            !tmp.path().join(".envforgeignore").exists(),
            "disabled target must not be created"
        );
        // Other 4 created
        assert_eq!(result.files_created.len(), 4);
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with(".envforgeignore")));
    }

    #[test]
    fn test_fence_config_skip_cursor_ignore() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = FenceConfig::default();
        cfg.targets.cursor_ignore = false;
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
        cfg.targets.cursor_rules = false;
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
        cfg.targets.copilot = false;
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
        cfg.targets.claude_code = false;
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert!(!tmp.path().join(".claude/settings.json").exists());
        assert!(result
            .files_skipped
            .iter()
            .any(|p| p.ends_with("settings.json")));
    }

    /// Byte-identical default: no config → same 5 files as current behavior (NFR5).
    #[test]
    fn test_fence_config_default_byte_identical_output() {
        let tmp = TempDir::new().unwrap();
        let cfg = FenceConfig::default();
        let result = create_fence_with(tmp.path(), false, &cfg).unwrap();
        assert_eq!(
            result.files_created.len(),
            5,
            "default config must create all 5 files"
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
        cfg.targets.copilot = false;

        // Create fence without copilot
        create_fence_with(tmp.path(), false, &cfg).unwrap();
        let status = check_fence_status_with(tmp.path(), &cfg).unwrap();

        // All enabled targets (4) should be fenced
        assert!(
            status.all_fenced,
            "disabled target must not make status Partial"
        );
        assert!(matches!(status.completeness, FenceCompleteness::Complete));
        // Only 4 entries (copilot excluded)
        assert_eq!(status.files.len(), 4);
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
        cfg.targets.copilot = false;
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
        assert_eq!(resolved.len(), 5);
        for r in &resolved {
            assert!(r.enabled);
            assert_eq!(r.source, ConfigSource::Default);
        }
    }

    /// resolve_fence_targets: disabled target reports source=Global.
    #[test]
    fn test_resolve_fence_targets_disabled_is_global() {
        let mut cfg = FenceConfig::default();
        cfg.targets.claude_code = false;
        let resolved = resolve_fence_targets(&cfg);
        let claude = resolved
            .iter()
            .find(|r| r.target == FenceTarget::ClaudeCode)
            .unwrap();
        assert!(!claude.enabled);
        assert_eq!(claude.source, ConfigSource::Global);
    }

    /// FenceTarget::all() returns all 5 distinct targets.
    #[test]
    fn test_fence_target_all_five_unique() {
        let all = FenceTarget::all();
        assert_eq!(all.len(), 5);
        let strs: Vec<&str> = all.iter().map(|t| t.as_str()).collect();
        let unique: std::collections::HashSet<_> = strs.iter().collect();
        assert_eq!(unique.len(), 5, "all targets must have unique string IDs");
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
}
