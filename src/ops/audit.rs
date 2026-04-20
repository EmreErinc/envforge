use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

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
) -> Result<Vec<AuditEntry>, Box<dyn std::error::Error>> {
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
}
