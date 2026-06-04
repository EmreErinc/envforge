// ─── Submodules ─────────────────────────────────────────

pub mod ai_guard_integration;
pub mod custody;
pub mod emitter;
pub mod query_engine;
pub mod query_types;
pub mod report_generator;
pub mod report_types;
pub mod tamper;
pub mod types;

// ─── Existing audit trail (git sync) ─────────────────────

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use super::OpError;

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub key: String,
    pub action: String,
    pub machine_id: String,
    pub timestamp: String,
    pub commit_hash: String,
}

/// Get audit trail for a specific key (or all keys) from sync git history.
pub fn get_audit_trail(
    sync_path: &Path,
    key_filter: Option<&str>,
    since: Option<&str>,
    machine_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<AuditEntry>, OpError> {
    let mut args = vec![
        "log".to_string(),
        "--pretty=format:%H|%h|%aI|%an|%s".to_string(),
        "-p".to_string(),
    ];

    if let Some(since_date) = since {
        args.push(format!("--since={}", since_date));
    }

    args.push("--".to_string());
    args.push("snapshot.toml".to_string());

    let output = Command::new("git")
        .args(&args)
        .current_dir(sync_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    let entries = parse_git_log(&stdout, key_filter, machine_filter);

    Ok(entries.into_iter().take(limit).collect())
}

/// Parse git log with patches into AuditEntry items.
fn parse_git_log(
    log_output: &str,
    key_filter: Option<&str>,
    machine_filter: Option<&str>,
) -> Vec<AuditEntry> {
    let mut entries = Vec::new();
    let mut current_commit: Option<CommitInfo> = None;
    let mut added_keys: HashMap<String, String> = HashMap::new();
    let mut removed_keys: HashMap<String, String> = HashMap::new();

    for line in log_output.lines() {
        // Check if this is a commit header line (contains | separators)
        if let Some(info) = try_parse_commit_line(line) {
            // Flush previous commit
            if let Some(ref commit) = current_commit {
                flush_commit(
                    commit,
                    &added_keys,
                    &removed_keys,
                    key_filter,
                    machine_filter,
                    &mut entries,
                );
            }
            current_commit = Some(info);
            added_keys.clear();
            removed_keys.clear();
            continue;
        }

        // Parse diff lines for key changes
        if current_commit.is_some() {
            if let Some(stripped) = line.strip_prefix('+') {
                if !stripped.starts_with("++") {
                    if let Some((key, value)) = parse_toml_kv(stripped) {
                        added_keys.insert(key, value);
                    }
                }
            } else if let Some(stripped) = line.strip_prefix('-') {
                if !stripped.starts_with("--") {
                    if let Some((key, value)) = parse_toml_kv(stripped) {
                        removed_keys.insert(key, value);
                    }
                }
            }
        }
    }

    // Flush last commit
    if let Some(ref commit) = current_commit {
        flush_commit(
            commit,
            &added_keys,
            &removed_keys,
            key_filter,
            machine_filter,
            &mut entries,
        );
    }

    entries
}

struct CommitInfo {
    short_hash: String,
    timestamp: String,
    author: String,
}

fn try_parse_commit_line(line: &str) -> Option<CommitInfo> {
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    if parts.len() != 5 {
        return None;
    }

    // Validate that the first part looks like a full SHA hash (40 hex chars)
    let full_hash = parts[0].trim();
    if full_hash.len() != 40 || !full_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    Some(CommitInfo {
        short_hash: parts[1].trim().to_string(),
        timestamp: parts[2].trim().to_string(),
        author: parts[3].trim().to_string(),
    })
}

fn flush_commit(
    commit: &CommitInfo,
    added_keys: &HashMap<String, String>,
    removed_keys: &HashMap<String, String>,
    key_filter: Option<&str>,
    machine_filter: Option<&str>,
    entries: &mut Vec<AuditEntry>,
) {
    // Apply machine filter
    if let Some(mf) = machine_filter {
        if !commit.author.contains(mf) {
            return;
        }
    }

    // Determine actions for each key
    let all_keys: HashSet<&String> = added_keys.keys().chain(removed_keys.keys()).collect();

    for key in all_keys {
        // Apply key filter
        if let Some(kf) = key_filter {
            if key != kf {
                continue;
            }
        }

        let in_added = added_keys.contains_key(key);
        let in_removed = removed_keys.contains_key(key);

        let action = if in_added && in_removed {
            "modified"
        } else if in_added {
            "added"
        } else {
            "removed"
        };

        entries.push(AuditEntry {
            key: key.clone(),
            action: action.to_string(),
            machine_id: commit.author.clone(),
            timestamp: commit.timestamp.clone(),
            commit_hash: commit.short_hash.clone(),
        });
    }
}

/// Parse a TOML key = "value" line, returning (key, value).
fn parse_toml_kv(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('[') || trimmed.starts_with('#') {
        return None;
    }

    let eq_pos = trimmed.find('=')?;
    let key = trimmed[..eq_pos].trim().to_string();
    let value = trimmed[eq_pos + 1..].trim().to_string();

    if key.is_empty() {
        return None;
    }

    // Strip surrounding quotes from value
    let value = if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value
    };

    Some((key, value))
}

// ─── AI Leak Scanning ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AiLeakEntry {
    pub commit_hash: String,
    pub date: String,
    pub author: String,
    pub ai_tool: String,
    pub leaked_patterns: Vec<String>,
    pub file_path: String,
}

/// AI tool indicators in commit messages.
const AI_INDICATORS: &[(&str, &str)] = &[
    ("co-authored-by: claude", "Claude"),
    ("claude code", "Claude"),
    ("claude-code", "Claude"),
    ("co-authored-by: github copilot", "Copilot"),
    ("co-authored-by: copilot", "Copilot"),
    ("co-authored-by: cursor", "Cursor"),
    ("generated by ai", "Unknown AI"),
    ("ai-assisted", "Unknown AI"),
    ("vibe coded", "Unknown AI"),
];

/// Secret prefixes to detect in diffs (reused from mcp_scan patterns).
const SECRET_PREFIXES: &[(&str, &str)] = &[
    ("sk-", "API key (OpenAI/Stripe style)"),
    ("sk_live_", "Stripe live key"),
    ("sk_test_", "Stripe test key"),
    ("pk_live_", "Stripe publishable live key"),
    ("pk_test_", "Stripe publishable test key"),
    ("AKIA", "AWS access key"),
    ("ghp_", "GitHub personal access token"),
    ("ghs_", "GitHub server token"),
    ("gho_", "GitHub OAuth token"),
    ("ghu_", "GitHub user token"),
    ("github_pat_", "GitHub fine-grained PAT"),
    ("glpat-", "GitLab personal access token"),
    ("xoxb-", "Slack bot token"),
    ("xoxp-", "Slack user token"),
    ("xoxa-", "Slack app token"),
    ("xoxs-", "Slack session token"),
    ("whsec_", "Webhook secret"),
    ("rk_live_", "Stripe restricted key"),
    ("rk_test_", "Stripe restricted test key"),
    ("SG.", "SendGrid API key"),
    ("sq0atp-", "Square access token"),
    ("sq0csp-", "Square client secret"),
    ("hf_", "Hugging Face token"),
    ("eyJ", "JWT token (base64)"),
];

/// Detect which AI tool is indicated by a commit message.
fn detect_ai_tool(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    for (pattern, tool) in AI_INDICATORS {
        if lower.contains(pattern) {
            return Some((*tool).to_string());
        }
    }
    None
}

/// Check if a diff line contains a secret pattern. Returns pattern descriptions found.
fn detect_secret_in_line(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let trimmed = line.trim();

    // Check known prefixes
    for (prefix, name) in SECRET_PREFIXES {
        if trimmed.contains(prefix) {
            // Verify the prefix appears with enough trailing characters
            if let Some(pos) = trimmed.find(prefix) {
                let after = &trimmed[pos + prefix.len()..];
                if after.len() >= 4 {
                    found.push((*name).to_string());
                }
            }
        }
    }

    // Check connection strings with credentials
    if let Some(scheme_end) = trimmed.find("://") {
        let after_scheme = &trimmed[scheme_end + 3..];
        if after_scheme.contains('@') {
            if let Some(at_pos) = after_scheme.find('@') {
                let user_info = &after_scheme[..at_pos];
                if user_info.contains(':') && user_info.len() > 3 {
                    found.push("Connection string with credentials".to_string());
                }
            }
        }
    }

    found
}

/// Scan git history for potential secret leaks in AI-assisted commits.
pub fn scan_ai_leaks(repo_path: &Path, limit: usize) -> Result<Vec<AiLeakEntry>, OpError> {
    let output = Command::new("git")
        .args([
            "log",
            "--all",
            "--format=COMMIT_START|%h|%aI|%an%nBODY_START%n%b%nBODY_END",
            "-p",
            &format!("-{}", limit),
        ])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ai_leak_log(&stdout)
}

/// Parse git log output and find AI-assisted commits with leaked secrets.
fn parse_ai_leak_log(log_output: &str) -> Result<Vec<AiLeakEntry>, OpError> {
    let mut leaks = Vec::new();

    // Split by commits
    let mut current_hash = String::new();
    let mut current_date = String::new();
    let mut current_author = String::new();
    let mut current_body = String::new();
    let mut in_body = false;
    let mut in_diff = false;
    let mut ai_tool: Option<String> = None;
    let mut diff_findings: Vec<(String, Vec<String>)> = Vec::new(); // (file_path, patterns)
    let mut current_diff_file = String::new();
    let mut current_file_patterns: Vec<String> = Vec::new();

    for line in log_output.lines() {
        if let Some(rest) = line.strip_prefix("COMMIT_START|") {
            // Flush previous commit
            if !current_hash.is_empty() {
                // Flush last diff file
                if !current_diff_file.is_empty() && !current_file_patterns.is_empty() {
                    diff_findings.push((current_diff_file.clone(), current_file_patterns.clone()));
                }

                if let Some(ref tool) = ai_tool {
                    for (file_path, patterns) in &diff_findings {
                        if !patterns.is_empty() {
                            leaks.push(AiLeakEntry {
                                commit_hash: current_hash.clone(),
                                date: current_date.clone(),
                                author: current_author.clone(),
                                ai_tool: tool.clone(),
                                leaked_patterns: patterns.clone(),
                                file_path: file_path.clone(),
                            });
                        }
                    }
                }
            }

            // Parse new commit header
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                current_hash = parts[0].to_string();
                current_date = parts[1].to_string();
                current_author = parts[2].to_string();
            }
            current_body.clear();
            in_body = false;
            in_diff = false;
            ai_tool = None;
            diff_findings.clear();
            current_diff_file.clear();
            current_file_patterns.clear();
            continue;
        }

        if line == "BODY_START" {
            in_body = true;
            continue;
        }

        if line == "BODY_END" {
            in_body = false;
            // Check body for AI indicators
            ai_tool = detect_ai_tool(&current_body);
            in_diff = true;
            continue;
        }

        if in_body {
            current_body.push_str(line);
            current_body.push('\n');
            continue;
        }

        if in_diff {
            // Track which file we're in
            if line.starts_with("diff --git") {
                // Flush previous file
                if !current_diff_file.is_empty() && !current_file_patterns.is_empty() {
                    diff_findings.push((current_diff_file.clone(), current_file_patterns.clone()));
                }
                // Extract file path: "diff --git a/path b/path"
                current_diff_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
                current_file_patterns.clear();
                continue;
            }

            // Only check added lines in the diff
            if let Some(added) = line.strip_prefix('+') {
                if !added.starts_with("++") {
                    let secrets = detect_secret_in_line(added);
                    for s in secrets {
                        if !current_file_patterns.contains(&s) {
                            current_file_patterns.push(s);
                        }
                    }
                }
            }
        }
    }

    // Flush last commit
    if !current_hash.is_empty() {
        if !current_diff_file.is_empty() && !current_file_patterns.is_empty() {
            diff_findings.push((current_diff_file.clone(), current_file_patterns.clone()));
        }
        if let Some(ref tool) = ai_tool {
            for (file_path, patterns) in &diff_findings {
                if !patterns.is_empty() {
                    leaks.push(AiLeakEntry {
                        commit_hash: current_hash.clone(),
                        date: current_date.clone(),
                        author: current_author.clone(),
                        ai_tool: tool.clone(),
                        leaked_patterns: patterns.clone(),
                        file_path: file_path.clone(),
                    });
                }
            }
        }
    }

    Ok(leaks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_kv_basic() {
        let result = parse_toml_kv(r#"API_KEY = "secret123""#);
        assert_eq!(
            result,
            Some(("API_KEY".to_string(), "secret123".to_string()))
        );
    }

    #[test]
    fn test_parse_toml_kv_no_quotes() {
        let result = parse_toml_kv("DB_PORT = 5432");
        assert_eq!(result, Some(("DB_PORT".to_string(), "5432".to_string())));
    }

    #[test]
    fn test_parse_toml_kv_empty_line() {
        assert_eq!(parse_toml_kv(""), None);
    }

    #[test]
    fn test_parse_toml_kv_comment() {
        assert_eq!(parse_toml_kv("# comment"), None);
    }

    #[test]
    fn test_parse_toml_kv_section_header() {
        assert_eq!(parse_toml_kv("[section]"), None);
    }

    #[test]
    fn test_try_parse_commit_line_valid() {
        let line = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2|a1b2c3d|2025-01-15T10:30:00+00:00|macbook-pro-abc|sync push";
        let result = try_parse_commit_line(line);
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.short_hash, "a1b2c3d");
        assert_eq!(info.author, "macbook-pro-abc");
    }

    #[test]
    fn test_try_parse_commit_line_invalid() {
        assert!(try_parse_commit_line("not a commit line").is_none());
        assert!(try_parse_commit_line("+API_KEY = \"foo\"").is_none());
    }

    #[test]
    fn test_parse_git_log_full() {
        let log = "\
a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2|a1b2c3d|2025-01-15T10:30:00+00:00|macbook-abc|sync push
diff --git a/snapshot.toml b/snapshot.toml
--- a/snapshot.toml
+++ b/snapshot.toml
-API_KEY = \"old_value\"
+API_KEY = \"new_value\"
+NEW_VAR = \"hello\"
";
        let entries = parse_git_log(log, None, None);
        assert_eq!(entries.len(), 2);

        let api_entry = entries.iter().find(|e| e.key == "API_KEY").unwrap();
        assert_eq!(api_entry.action, "modified");
        assert_eq!(api_entry.machine_id, "macbook-abc");

        let new_entry = entries.iter().find(|e| e.key == "NEW_VAR").unwrap();
        assert_eq!(new_entry.action, "added");
    }

    #[test]
    fn test_parse_git_log_with_key_filter() {
        let log = "\
a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2|a1b2c3d|2025-01-15T10:30:00+00:00|macbook-abc|sync push
diff --git a/snapshot.toml b/snapshot.toml
+API_KEY = \"new_value\"
+DB_HOST = \"localhost\"
";
        let entries = parse_git_log(log, Some("API_KEY"), None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "API_KEY");
    }

    #[test]
    fn test_parse_git_log_with_machine_filter() {
        let log = "\
a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2|a1b2c3d|2025-01-15T10:30:00+00:00|macbook-abc|sync push
+API_KEY = \"val1\"
b1c2d3e4f5a6b1c2d3e4f5a6b1c2d3e4f5a6b1c2|b1c2d3e|2025-01-14T09:00:00+00:00|linux-server|sync push
+DB_HOST = \"localhost\"
";
        let entries = parse_git_log(log, None, Some("linux-server"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].machine_id, "linux-server");
    }

    #[test]
    fn test_parse_git_log_removed_key() {
        let log = "\
a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2|a1b2c3d|2025-01-15T10:30:00+00:00|macbook-abc|sync push
-OLD_VAR = \"gone\"
";
        let entries = parse_git_log(log, None, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "OLD_VAR");
        assert_eq!(entries[0].action, "removed");
    }

    // ─── AI Leak Detection Tests ────────────────────────────

    #[test]
    fn test_detect_ai_tool_claude() {
        assert_eq!(
            detect_ai_tool("Co-Authored-By: Claude <noreply@anthropic.com>"),
            Some("Claude".to_string())
        );
        assert_eq!(
            detect_ai_tool("co-authored-by: Claude Code"),
            Some("Claude".to_string())
        );
        assert_eq!(
            detect_ai_tool("Generated with claude-code"),
            Some("Claude".to_string())
        );
    }

    #[test]
    fn test_detect_ai_tool_copilot() {
        assert_eq!(
            detect_ai_tool("Co-Authored-By: GitHub Copilot <copilot@github.com>"),
            Some("Copilot".to_string())
        );
    }

    #[test]
    fn test_detect_ai_tool_cursor() {
        assert_eq!(
            detect_ai_tool("Co-Authored-By: Cursor <cursor@cursor.sh>"),
            Some("Cursor".to_string())
        );
    }

    #[test]
    fn test_detect_ai_tool_generic() {
        assert_eq!(
            detect_ai_tool("This was generated by AI"),
            Some("Unknown AI".to_string())
        );
        assert_eq!(
            detect_ai_tool("AI-assisted implementation"),
            Some("Unknown AI".to_string())
        );
        assert_eq!(
            detect_ai_tool("vibe coded this feature"),
            Some("Unknown AI".to_string())
        );
    }

    #[test]
    fn test_detect_ai_tool_none() {
        assert_eq!(detect_ai_tool("fix: regular commit message"), None);
        assert_eq!(detect_ai_tool("refactor database layer"), None);
    }

    #[test]
    fn test_detect_secret_in_line_api_key() {
        let patterns = detect_secret_in_line("API_KEY=sk-proj-abcdefghijklmnop");
        assert!(!patterns.is_empty());
        assert!(patterns[0].contains("API key"));
    }

    #[test]
    fn test_detect_secret_in_line_aws() {
        let patterns = detect_secret_in_line("AWS_KEY=AKIA_FAKE_TEST_VALUE");
        assert!(!patterns.is_empty());
        assert!(patterns[0].contains("AWS"));
    }

    #[test]
    fn test_detect_secret_in_line_github() {
        let patterns = detect_secret_in_line("TOKEN=ghp_FAKE00TEST00TOKEN00VALUE00000000");
        assert!(!patterns.is_empty());
        assert!(patterns[0].contains("GitHub"));
    }

    #[test]
    fn test_detect_secret_in_line_connection_string() {
        let patterns =
            detect_secret_in_line("DATABASE_URL=postgres://admin:secret@db.example.com/mydb");
        assert!(!patterns.is_empty());
        assert!(patterns[0].contains("Connection string"));
    }

    #[test]
    fn test_detect_secret_in_line_clean() {
        let patterns = detect_secret_in_line("PORT=3000");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_parse_ai_leak_log_with_leak() {
        let log = "\
COMMIT_START|abc1234|2025-01-15T10:00:00+00:00|dev-user
BODY_START
feat: add database config

Co-Authored-By: Claude <noreply@anthropic.com>
BODY_END
diff --git a/config.env b/config.env
+DB_URL=postgres://admin:supersecret@db.example.com:5432/mydb
+API_KEY=sk-proj-abcdefghijklmnopqrstuvwx
";
        let leaks = parse_ai_leak_log(log).unwrap();
        assert_eq!(leaks.len(), 1); // one file with findings
        assert_eq!(leaks[0].commit_hash, "abc1234");
        assert_eq!(leaks[0].ai_tool, "Claude");
        assert_eq!(leaks[0].file_path, "config.env");
        assert!(leaks[0].leaked_patterns.len() >= 2);
    }

    #[test]
    fn test_parse_ai_leak_log_no_ai_commit() {
        let log = "\
COMMIT_START|abc1234|2025-01-15T10:00:00+00:00|dev-user
BODY_START
feat: add database config
BODY_END
diff --git a/config.env b/config.env
+API_KEY=sk-proj-abcdefghijklmnopqrstuvwx
";
        let leaks = parse_ai_leak_log(log).unwrap();
        assert!(leaks.is_empty(), "Non-AI commits should not be flagged");
    }

    #[test]
    fn test_parse_ai_leak_log_ai_commit_no_secrets() {
        let log = "\
COMMIT_START|abc1234|2025-01-15T10:00:00+00:00|dev-user
BODY_START
fix: update port

Co-Authored-By: Claude <noreply@anthropic.com>
BODY_END
diff --git a/config.env b/config.env
+PORT=3000
+HOST=localhost
";
        let leaks = parse_ai_leak_log(log).unwrap();
        assert!(
            leaks.is_empty(),
            "AI commit with clean diff should have no leaks"
        );
    }
}
