use std::path::{Path, PathBuf};

use super::OpError;

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

/// Overall fence status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FenceStatus {
    pub files: Vec<FenceFileStatus>,
    pub all_fenced: bool,
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
    });
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
    let mut json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            // Unparseable — leave it alone; user can clean up manually.
            result.files_skipped.push(path);
            return Ok(());
        }
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
pub fn check_fence_status(project_dir: &Path) -> Result<FenceStatus, OpError> {
    let files = vec![
        check_file_status(project_dir, ".envforgeignore"),
        check_file_status(project_dir, ".cursorignore"),
        check_file_status(project_dir, ".cursorrules"),
        check_file_status(project_dir, ".github/copilot-instructions.md"),
        check_file_status(project_dir, ".claude/settings.json"),
    ];
    let all_fenced = files.iter().all(|f| f.fenced);
    Ok(FenceStatus { files, all_fenced })
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
