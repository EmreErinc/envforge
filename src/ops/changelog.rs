use std::path::PathBuf;

use chrono::Local;

use crate::config::{config_dir, AppConfig};
use crate::model::ParseError;

/// A single changelog entry.
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    pub timestamp: String,
    pub profile: String,
    pub action: String,
    pub key: String,
    pub detail: String,
}

/// Get the changelog file path.
pub fn changelog_path() -> Result<PathBuf, ParseError> {
    Ok(config_dir()?.join("changelog.log"))
}

/// Append a change entry to the changelog.
pub fn log_change(profile: &str, action: &str, key: &str, detail: &str) {
    if let Ok(path) = changelog_path() {
        let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%SZ");
        let line = format!(
            "{} [{}] {} {} {}\n",
            timestamp, profile, action, key, detail
        );

        // Create parent dir if needed
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Append
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = file.write_all(line.as_bytes());
        }

        // Rotate if needed
        let _ = rotate_if_needed(&path, 1000);
    }
}

/// Log multiple changes from a save diff.
pub fn log_save(config: &AppConfig, changes: &[(String, String, String)]) {
    for (action, key, detail) in changes {
        log_change(&config.profiles.active, action, key, detail);
    }
}

/// Read changelog entries, optionally filtered by key.
pub fn read_changelog(
    key_filter: Option<&str>,
    max_entries: usize,
) -> Result<Vec<ChangelogEntry>, std::io::Error> {
    let path = changelog_path().map_err(|e| std::io::Error::other(e.to_string()))?;

    if !path.exists() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&path)?;
    let mut entries: Vec<ChangelogEntry> = content
        .lines()
        .filter_map(parse_changelog_line)
        .filter(|e| {
            if let Some(filter) = key_filter {
                e.key.to_lowercase().contains(&filter.to_lowercase())
            } else {
                true
            }
        })
        .collect();

    // Return last N entries
    if entries.len() > max_entries {
        entries = entries.split_off(entries.len() - max_entries);
    }

    Ok(entries)
}

/// Parse a single changelog line.
fn parse_changelog_line(line: &str) -> Option<ChangelogEntry> {
    // Format: "2026-04-10T10:30:00Z [dev] ADD API_KEY=sk-***"
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }

    let timestamp = parts[0].to_string();
    let profile = parts[1]
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let action = parts[2].to_string();

    let rest = parts[3];
    let (key, detail) = if let Some(space_pos) = rest.find(' ') {
        (
            rest[..space_pos].to_string(),
            rest[space_pos + 1..].to_string(),
        )
    } else {
        (rest.to_string(), String::new())
    };

    Some(ChangelogEntry {
        timestamp,
        profile,
        action,
        key,
        detail,
    })
}

/// Rotate changelog if it exceeds max_entries.
fn rotate_if_needed(path: &PathBuf, max_entries: usize) -> Result<(), std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() > max_entries {
        let keep = &lines[lines.len() - max_entries..];
        std::fs::write(path, keep.join("\n") + "\n")?;
    }

    Ok(())
}

/// Mask a value for changelog (show first 3 chars + ***)
pub fn mask_for_log(value: &str) -> String {
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &value[..3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_short_value() {
        assert_eq!(mask_for_log("abc"), "***");
        assert_eq!(mask_for_log("abcd"), "***");
    }

    #[test]
    fn test_mask_long_value() {
        assert_eq!(mask_for_log("secret_value"), "sec***");
        assert_eq!(mask_for_log("12345"), "123***");
    }

    #[test]
    fn test_parse_changelog_line_valid() {
        let entry = parse_changelog_line("2026-04-10T10:30:00Z [dev] ADD API_KEY sk-***").unwrap();
        assert_eq!(entry.timestamp, "2026-04-10T10:30:00Z");
        assert_eq!(entry.profile, "dev");
        assert_eq!(entry.action, "ADD");
        assert_eq!(entry.key, "API_KEY");
        assert_eq!(entry.detail, "sk-***");
    }

    #[test]
    fn test_parse_changelog_line_invalid() {
        assert!(parse_changelog_line("incomplete").is_none());
        assert!(parse_changelog_line("").is_none());
    }
}
