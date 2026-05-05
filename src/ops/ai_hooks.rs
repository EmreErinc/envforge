use std::path::{Path, PathBuf};

use super::OpError;

// ─── Types ─────────────────────────────────────────────────

/// Supported AI coding tools.
#[derive(Debug, Clone, PartialEq)]
pub enum AiTool {
    ClaudeCode,
    Cursor,
}

impl AiTool {
    /// Display name for the tool.
    pub fn name(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => "Claude Code",
            AiTool::Cursor => "Cursor",
        }
    }
}

/// Parse a tool name string into an AiTool enum.
pub fn parse_ai_tool(name: &str) -> Result<AiTool, String> {
    match name.to_lowercase().replace(['-', '_'], "").as_str() {
        "claudecode" => Ok(AiTool::ClaudeCode),
        "cursor" => Ok(AiTool::Cursor),
        _ => Err(format!(
            "Unknown AI tool '{}'. Supported: claude-code, cursor",
            name
        )),
    }
}

/// Result of a hook install/remove operation.
#[derive(Debug)]
pub struct HookInstallResult {
    pub tool: String,
    pub config_path: PathBuf,
    pub installed: bool,
    pub message: String,
}

// ─── Claude Code Hook ──────────────────────────────────────

const CLAUDE_PRE_TOOL_MATCHER: &str = "Read|Write|Edit|Bash|MultiEdit";
const CLAUDE_PRE_TOOL_COMMAND: &str =
    "envforge ai-guard pre-tool \"$TOOL_NAME\" \"$TOOL_INPUT\" 2>/dev/null; true";
const CLAUDE_POST_TOOL_MATCHER: &str = "Write|Edit|Bash|MultiEdit";
const CLAUDE_POST_TOOL_COMMAND: &str =
    "envforge ai-guard post-tool \"$TOOL_NAME\" \"$TOOL_INPUT\" 2>/dev/null; true";

/// Build the Claude Code settings path for a project.
fn claude_settings_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".claude").join("settings.json")
}

/// Check if an envforge hook is already present in a hooks array.
fn has_envforge_hook(arr: &[serde_json::Value]) -> bool {
    arr.iter().any(|entry| {
        entry
            .get("hook")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("envforge"))
    })
}

/// Install EnvForge hooks into Claude Code's settings.json.
///
/// Installs PreToolUse and PostToolUse hooks for the `envforge ai-guard` command.
/// Reads existing settings (or creates new), merges hooks without overwriting
/// other hooks, and writes back.
fn install_claude_code_hook(project_dir: &Path) -> Result<HookInstallResult, OpError> {
    let settings_path = claude_settings_path(project_dir);

    let mut settings = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str::<serde_json::Value>(&content)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .ok_or("settings.json root is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks.as_object_mut().ok_or("hooks is not an object")?;

    // Check if already installed (check both stages)
    let pre_arr = hooks_obj
        .get("PreToolUse")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let post_arr = hooks_obj
        .get("PostToolUse")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if has_envforge_hook(&pre_arr) || has_envforge_hook(&post_arr) {
        return Ok(HookInstallResult {
            tool: "Claude Code".to_string(),
            config_path: settings_path,
            installed: false,
            message: "EnvForge hook already installed in Claude Code settings".to_string(),
        });
    }

    // Install PreToolUse hook
    let pre_tool_use = hooks_obj
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));
    let pre_arr = pre_tool_use
        .as_array_mut()
        .ok_or("PreToolUse is not an array")?;
    pre_arr.push(serde_json::json!({
        "matcher": CLAUDE_PRE_TOOL_MATCHER,
        "hook": CLAUDE_PRE_TOOL_COMMAND,
    }));

    // Install PostToolUse hook
    let post_tool_use = hooks_obj
        .entry("PostToolUse")
        .or_insert_with(|| serde_json::json!([]));
    let post_arr = post_tool_use
        .as_array_mut()
        .ok_or("PostToolUse is not an array")?;
    post_arr.push(serde_json::json!({
        "matcher": CLAUDE_POST_TOOL_MATCHER,
        "hook": CLAUDE_POST_TOOL_COMMAND,
    }));

    // Write back
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, pretty)?;

    Ok(HookInstallResult {
        tool: "Claude Code".to_string(),
        config_path: settings_path,
        installed: true,
        message: "Installed EnvForge PreToolUse + PostToolUse hooks in Claude Code settings"
            .to_string(),
    })
}

/// Remove envforge hooks from a single hook stage array. Returns true if any were removed.
fn remove_envforge_from_array(arr: &mut Vec<serde_json::Value>) -> bool {
    let before = arr.len();
    arr.retain(|entry| {
        !entry
            .get("hook")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("envforge"))
    });
    before != arr.len()
}

/// Remove the EnvForge hooks from Claude Code's settings.json.
fn remove_claude_code_hook(project_dir: &Path) -> Result<HookInstallResult, OpError> {
    let settings_path = claude_settings_path(project_dir);

    if !settings_path.exists() {
        return Ok(HookInstallResult {
            tool: "Claude Code".to_string(),
            config_path: settings_path,
            installed: false,
            message: "No Claude Code settings.json found; nothing to remove".to_string(),
        });
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;

    let mut removed = false;
    if let Some(hooks) = settings.get_mut("hooks") {
        // Remove from all hook stages
        for stage in &["PreToolUse", "PostToolUse"] {
            if let Some(stage_val) = hooks.get_mut(*stage) {
                if let Some(arr) = stage_val.as_array_mut() {
                    if remove_envforge_from_array(arr) {
                        removed = true;
                    }
                }
            }
        }
    }

    if removed {
        let pretty = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&settings_path, pretty)?;
    }

    Ok(HookInstallResult {
        tool: "Claude Code".to_string(),
        config_path: settings_path,
        installed: false,
        message: if removed {
            "Removed EnvForge hooks from Claude Code settings".to_string()
        } else {
            "No EnvForge hook found in Claude Code settings".to_string()
        },
    })
}

// ─── Cursor Rules ──────────────────────────────────────────

const CURSOR_RULES_MARKER: &str = "## EnvForge Security Rules";

const CURSOR_RULES_BLOCK: &str = r#"
## EnvForge Security Rules
- Before writing any file, check that no environment variable values are hardcoded
- Never read .env files; use .env.schema or .env.ai.md for context
- If you need a secret value, use process.env.KEY_NAME, not the actual value
- Run `envforge scan --staged` before suggesting commits
"#;

/// Determine the Cursor rules file path. Prefer `.cursor/rules` if the dir exists.
fn cursor_rules_path(project_dir: &Path) -> PathBuf {
    let cursor_dir = project_dir.join(".cursor");
    if cursor_dir.is_dir() {
        cursor_dir.join("rules")
    } else {
        project_dir.join(".cursorrules")
    }
}

fn install_cursor_hook(project_dir: &Path) -> Result<HookInstallResult, OpError> {
    let rules_path = cursor_rules_path(project_dir);

    let existing = if rules_path.exists() {
        std::fs::read_to_string(&rules_path)?
    } else {
        String::new()
    };

    if existing.contains(CURSOR_RULES_MARKER) {
        return Ok(HookInstallResult {
            tool: "Cursor".to_string(),
            config_path: rules_path,
            installed: false,
            message: "EnvForge rules already present in Cursor rules file".to_string(),
        });
    }

    let new_content = if existing.is_empty() {
        CURSOR_RULES_BLOCK.trim_start().to_string()
    } else {
        format!("{}\n{}", existing.trim_end(), CURSOR_RULES_BLOCK)
    };

    if let Some(parent) = rules_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&rules_path, new_content)?;

    Ok(HookInstallResult {
        tool: "Cursor".to_string(),
        config_path: rules_path,
        installed: true,
        message: "Appended EnvForge security rules to Cursor rules file".to_string(),
    })
}

fn remove_cursor_hook(project_dir: &Path) -> Result<HookInstallResult, OpError> {
    let rules_path = cursor_rules_path(project_dir);

    if !rules_path.exists() {
        return Ok(HookInstallResult {
            tool: "Cursor".to_string(),
            config_path: rules_path,
            installed: false,
            message: "No Cursor rules file found; nothing to remove".to_string(),
        });
    }

    let content = std::fs::read_to_string(&rules_path)?;

    if !content.contains(CURSOR_RULES_MARKER) {
        return Ok(HookInstallResult {
            tool: "Cursor".to_string(),
            config_path: rules_path,
            installed: false,
            message: "No EnvForge rules found in Cursor rules file".to_string(),
        });
    }

    // Remove the block: from marker to next "## " heading or end of file
    let mut result = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.starts_with(CURSOR_RULES_MARKER) {
            in_block = true;
            continue;
        }
        if in_block {
            // End of our block: another heading or empty followed by heading
            if line.starts_with("## ") {
                in_block = false;
                result.push_str(line);
                result.push('\n');
            }
            // Skip lines inside the block
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    // Trim trailing whitespace
    let result = result.trim_end().to_string();
    if result.is_empty() {
        std::fs::remove_file(&rules_path)?;
    } else {
        std::fs::write(&rules_path, format!("{}\n", result))?;
    }

    Ok(HookInstallResult {
        tool: "Cursor".to_string(),
        config_path: rules_path,
        installed: false,
        message: "Removed EnvForge rules from Cursor rules file".to_string(),
    })
}

// ─── Public API ────────────────────────────────────────────

/// Install EnvForge hook for an AI coding tool.
pub fn install_ai_hook(tool: &AiTool, project_dir: &Path) -> Result<HookInstallResult, OpError> {
    match tool {
        AiTool::ClaudeCode => install_claude_code_hook(project_dir),
        AiTool::Cursor => install_cursor_hook(project_dir),
    }
}

/// Remove EnvForge hook from an AI coding tool.
pub fn remove_ai_hook(tool: &AiTool, project_dir: &Path) -> Result<HookInstallResult, OpError> {
    match tool {
        AiTool::ClaudeCode => remove_claude_code_hook(project_dir),
        AiTool::Cursor => remove_cursor_hook(project_dir),
    }
}

/// Check status of all supported AI tool hooks.
pub fn check_hook_status(project_dir: &Path) -> serde_json::Value {
    let claude_path = claude_settings_path(project_dir);
    let claude_installed = if claude_path.exists() {
        match std::fs::read_to_string(&claude_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(settings) => settings
                    .get("hooks")
                    .and_then(|h| h.get("PreToolUse"))
                    .and_then(|p| p.as_array())
                    .map(|arr| has_envforge_hook(arr))
                    .unwrap_or(false),
                Err(_) => false,
            },
            Err(_) => false,
        }
    } else {
        false
    };

    let cursor_path = cursor_rules_path(project_dir);
    let cursor_installed = if cursor_path.exists() {
        match std::fs::read_to_string(&cursor_path) {
            Ok(content) => content.contains(CURSOR_RULES_MARKER),
            Err(_) => false,
        }
    } else {
        false
    };

    serde_json::json!({
        "version": 1,
        "tools": [
            {
                "name": "Claude Code",
                "installed": claude_installed,
                "path": claude_path.to_string_lossy(),
            },
            {
                "name": "Cursor",
                "installed": cursor_installed,
                "path": cursor_path.to_string_lossy(),
            }
        ],
        "enabled": claude_installed || cursor_installed
    })
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_ai_tool_claude_code() {
        assert_eq!(parse_ai_tool("claude-code").unwrap(), AiTool::ClaudeCode);
        assert_eq!(parse_ai_tool("Claude-Code").unwrap(), AiTool::ClaudeCode);
        assert_eq!(parse_ai_tool("claude_code").unwrap(), AiTool::ClaudeCode);
        assert_eq!(parse_ai_tool("claudecode").unwrap(), AiTool::ClaudeCode);
    }

    #[test]
    fn test_parse_ai_tool_cursor() {
        assert_eq!(parse_ai_tool("cursor").unwrap(), AiTool::Cursor);
        assert_eq!(parse_ai_tool("Cursor").unwrap(), AiTool::Cursor);
    }

    #[test]
    fn test_parse_ai_tool_unknown() {
        assert!(parse_ai_tool("vscode").is_err());
        assert!(parse_ai_tool("").is_err());
    }

    #[test]
    fn test_claude_code_install_fresh() {
        let tmp = TempDir::new().unwrap();
        let result = install_claude_code_hook(tmp.path()).unwrap();
        assert!(result.installed);

        // Verify settings.json was created
        let settings_path = tmp.path().join(".claude").join("settings.json");
        assert!(settings_path.exists());

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();

        // PreToolUse hook
        let pre = &content["hooks"]["PreToolUse"];
        assert!(pre.is_array());
        assert_eq!(pre.as_array().unwrap().len(), 1);
        assert!(pre[0]["hook"]
            .as_str()
            .unwrap()
            .contains("envforge ai-guard"));
        assert_eq!(
            pre[0]["matcher"].as_str().unwrap(),
            "Read|Write|Edit|Bash|MultiEdit"
        );

        // PostToolUse hook
        let post = &content["hooks"]["PostToolUse"];
        assert!(post.is_array());
        assert_eq!(post.as_array().unwrap().len(), 1);
        assert!(post[0]["hook"]
            .as_str()
            .unwrap()
            .contains("envforge ai-guard"));
        assert_eq!(
            post[0]["matcher"].as_str().unwrap(),
            "Write|Edit|Bash|MultiEdit"
        );
    }

    #[test]
    fn test_claude_code_install_merge_existing() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        // Write existing settings with other hooks
        let existing = serde_json::json!({
            "permissions": {
                "allow": ["Read"]
            },
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": ".*",
                        "hook": "echo pre-hook"
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "Bash",
                        "hook": "echo post-bash"
                    }
                ]
            }
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let result = install_claude_code_hook(tmp.path()).unwrap();
        assert!(result.installed);

        // Verify existing hooks are preserved
        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(claude_dir.join("settings.json")).unwrap(),
        )
        .unwrap();

        // PreToolUse has existing + our new hook
        assert_eq!(content["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        // PostToolUse has existing + our new hook
        assert_eq!(content["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);
        // Permissions preserved
        assert!(content["permissions"]["allow"].is_array());
    }

    #[test]
    fn test_claude_code_install_idempotent() {
        let tmp = TempDir::new().unwrap();

        let r1 = install_claude_code_hook(tmp.path()).unwrap();
        assert!(r1.installed);

        let r2 = install_claude_code_hook(tmp.path()).unwrap();
        assert!(!r2.installed); // already there
        assert!(r2.message.contains("already"));
    }

    #[test]
    fn test_claude_code_remove() {
        let tmp = TempDir::new().unwrap();

        install_claude_code_hook(tmp.path()).unwrap();
        let result = remove_claude_code_hook(tmp.path()).unwrap();
        assert!(result.message.contains("Removed"));

        // Verify hooks are gone from both stages
        let settings_path = tmp.path().join(".claude").join("settings.json");
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(content["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(content["hooks"]["PostToolUse"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_claude_code_remove_no_settings() {
        let tmp = TempDir::new().unwrap();
        let result = remove_claude_code_hook(tmp.path()).unwrap();
        assert!(result.message.contains("nothing to remove"));
    }

    #[test]
    fn test_cursor_install_fresh() {
        let tmp = TempDir::new().unwrap();
        let result = install_cursor_hook(tmp.path()).unwrap();
        assert!(result.installed);

        let rules_path = tmp.path().join(".cursorrules");
        assert!(rules_path.exists());
        let content = std::fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains(CURSOR_RULES_MARKER));
        assert!(content.contains("envforge scan --staged"));
    }

    #[test]
    fn test_cursor_install_with_cursor_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".cursor")).unwrap();

        let result = install_cursor_hook(tmp.path()).unwrap();
        assert!(result.installed);

        // Should use .cursor/rules, not .cursorrules
        let rules_path = tmp.path().join(".cursor").join("rules");
        assert!(rules_path.exists());
        assert!(!tmp.path().join(".cursorrules").exists());
    }

    #[test]
    fn test_cursor_install_appends_to_existing() {
        let tmp = TempDir::new().unwrap();
        let rules_path = tmp.path().join(".cursorrules");
        std::fs::write(&rules_path, "## Project Rules\n- Use TypeScript\n").unwrap();

        let result = install_cursor_hook(tmp.path()).unwrap();
        assert!(result.installed);

        let content = std::fs::read_to_string(&rules_path).unwrap();
        assert!(content.starts_with("## Project Rules"));
        assert!(content.contains(CURSOR_RULES_MARKER));
    }

    #[test]
    fn test_cursor_install_idempotent() {
        let tmp = TempDir::new().unwrap();

        let r1 = install_cursor_hook(tmp.path()).unwrap();
        assert!(r1.installed);

        let r2 = install_cursor_hook(tmp.path()).unwrap();
        assert!(!r2.installed);
        assert!(r2.message.contains("already"));
    }

    #[test]
    fn test_cursor_remove() {
        let tmp = TempDir::new().unwrap();

        install_cursor_hook(tmp.path()).unwrap();
        let result = remove_cursor_hook(tmp.path()).unwrap();
        assert!(result.message.contains("Removed"));

        // File should be removed since it was only our content
        assert!(!tmp.path().join(".cursorrules").exists());
    }

    #[test]
    fn test_cursor_remove_preserves_other_rules() {
        let tmp = TempDir::new().unwrap();
        let rules_path = tmp.path().join(".cursorrules");

        // Write existing rules, then install
        std::fs::write(&rules_path, "## Project Rules\n- Use TypeScript\n").unwrap();
        install_cursor_hook(tmp.path()).unwrap();

        // Remove
        remove_cursor_hook(tmp.path()).unwrap();

        // File should still exist with original content
        assert!(rules_path.exists());
        let content = std::fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains("## Project Rules"));
        assert!(!content.contains(CURSOR_RULES_MARKER));
    }
}
