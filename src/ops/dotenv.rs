use std::path::Path;

use chrono::Local;

use crate::config::AppConfig;
use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};
use crate::ops::crud::{add_entry, edit_entry};
use crate::ops::listing::{filter_entries, EntryLocation, EnvEntry};

/// Result of an import operation.
#[derive(Debug)]
pub struct ImportResult {
    pub added: usize,
    pub skipped: usize,
    pub overwritten: usize,
}

/// A parsed .env entry.
#[derive(Debug, Clone)]
pub struct DotenvEntry {
    pub key: String,
    pub value: String,
}

/// Parse a .env file into key-value pairs.
///
/// Handles: KEY=VALUE, KEY="VALUE", KEY='VALUE', comments (#), blank lines.
pub fn parse_dotenv(path: &Path) -> Result<Vec<DotenvEntry>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_dotenv_content(&content))
}

/// Parse .env content from a string.
pub fn parse_dotenv_content(content: &str) -> Vec<DotenvEntry> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Find the = separator
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let raw_value = trimmed[eq_pos + 1..].trim().to_string();

            if key.is_empty() {
                continue;
            }

            // Strip quotes from value
            let value = strip_quotes(&raw_value);

            entries.push(DotenvEntry { key, value });
        }
    }

    entries
}

/// Import .env entries into a ShellFile.
///
/// If `force` is true, duplicates are overwritten silently.
/// Otherwise, duplicates are skipped (CLI caller should prompt interactively).
pub fn import_entries(
    shell_file: &mut ShellFile,
    entries: &[DotenvEntry],
    config: &AppConfig,
    force: bool,
) -> ImportResult {
    let mut added = 0;
    let mut skipped = 0;
    let mut overwritten = 0;

    for entry in entries {
        // Check if key already exists
        let exists = shell_file.lines.iter().any(|node| match node {
            LineNode::EnvExport { key, .. } => key == &entry.key,
            _ => false,
        });

        if exists {
            if force {
                // Overwrite existing
                if edit_entry(shell_file, &entry.key, &entry.value).is_ok() {
                    overwritten += 1;
                } else {
                    skipped += 1;
                }
            } else {
                skipped += 1;
            }
        } else {
            // Add new entry
            match add_entry(
                shell_file,
                &entry.key,
                &entry.value,
                ExportStyle::Export,
                QuoteStyle::Double,
                config.offsets.header_protected_lines,
                config.offsets.footer_protected_lines,
            ) {
                Ok(()) => added += 1,
                Err(_) => skipped += 1,
            }
        }
    }

    ImportResult {
        added,
        skipped,
        overwritten,
    }
}

/// Export ENV entries to .env format string.
///
/// If `exclude_sensitive` is true, keys matching sensitive patterns are skipped.
/// If `filter_query` is provided, only matching entries are exported.
pub fn export_entries(
    entries: &[EnvEntry],
    exclude_sensitive: bool,
    filter_query: Option<&str>,
) -> String {
    let filtered = if let Some(query) = filter_query {
        filter_entries(entries, query)
    } else {
        entries.to_vec()
    };

    let mut output = String::new();
    output.push_str(&format!(
        "# Exported by EnvForge on {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    output.push('\n');

    for entry in &filtered {
        // Skip commented/deleted entries
        if entry.location == EntryLocation::Commented {
            continue;
        }

        // Skip sensitive if requested
        if exclude_sensitive && is_sensitive_key(&entry.key) {
            continue;
        }

        let formatted_value = if needs_quoting(&entry.value) {
            format!("\"{}\"", entry.value)
        } else {
            entry.value.clone()
        };

        output.push_str(&format!("{}={}\n", entry.key, formatted_value));
    }

    output
}

/// Check if a key matches sensitive patterns.
fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("credential")
        || (lower.contains("key") && !lower.contains("keyboard"))
}

/// Check if a value needs quoting in .env format.
fn needs_quoting(value: &str) -> bool {
    value.contains(' ')
        || value.contains('#')
        || value.contains('"')
        || value.contains('\'')
        || value.contains('\\')
        || value.contains('\n')
        || value.is_empty()
}

/// Strip surrounding quotes from a value.
fn strip_quotes(value: &str) -> String {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}
