use std::path::{Path, PathBuf};

use super::OpError;

/// Result of creating AI tool fence files.
pub struct FenceResult {
    pub files_created: Vec<PathBuf>,
    pub files_updated: Vec<PathBuf>,
    pub files_skipped: Vec<PathBuf>,
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
pub fn create_fence(project_dir: &Path, dry_run: bool) -> Result<FenceResult, OpError> {
    let mut result = FenceResult {
        files_created: Vec::new(),
        files_updated: Vec::new(),
        files_skipped: Vec::new(),
    };

    // 1. .envforgeignore
    write_envforgeignore(project_dir, dry_run, &mut result)?;

    // 2. .cursorignore
    write_cursorignore(project_dir, dry_run, &mut result)?;

    // 3. .cursorrules
    write_cursorrules(project_dir, dry_run, &mut result)?;

    // 4. .github/copilot-instructions.md
    write_copilot_instructions(project_dir, dry_run, &mut result)?;

    // 5. .claude/settings.json
    write_claude_settings(project_dir, dry_run, &mut result)?;

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
            .filter(|r| !existing_strs.contains(&r.to_string()))
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
                .unwrap()
                .entry("permissions")
                .or_insert_with(|| serde_json::json!({}));
            let deny = permissions
                .as_object_mut()
                .unwrap()
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

    #[test]
    fn test_create_fence_dry_run() {
        let tmp = TempDir::new().unwrap();
        let result = create_fence(tmp.path(), true).unwrap();

        assert_eq!(result.files_created.len(), 5);
        // No files should actually exist
        assert!(!tmp.path().join(".envforgeignore").exists());
        assert!(!tmp.path().join(".cursorignore").exists());
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
}
