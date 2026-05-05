use std::collections::HashMap;
use std::path::PathBuf;

use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};

/// Where an ENV entry lives.
#[derive(Debug, Clone, PartialEq)]
pub enum EntryLocation {
    /// Active in a shell config file
    InFile,
    /// Active in the reference file (~/.env_managed)
    InReference,
    /// Commented out by envforge (soft-deleted or moved)
    Commented,
}

/// A single environment variable entry extracted from a ShellFile.
#[derive(Debug, Clone)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
    pub source_file: PathBuf,
    pub line_number: usize,
    /// Vector index in ShellFile.lines
    pub line_index: usize,
    pub location: EntryLocation,
    pub export_style: ExportStyle,
    pub quote_style: QuoteStyle,
    pub is_dirty: bool,
}

/// Status of an ENV variable relative to the runtime environment.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvStatus {
    /// Defined in file and active in runtime
    Active,
    /// Defined in file but not active in runtime
    InFileOnly,
    /// Active in runtime but not defined in any file
    RuntimeOnly,
    /// Commented out (deleted/moved)
    Commented,
}

/// Result of comparing file entries with runtime environment.
#[derive(Debug, Clone)]
pub struct EnvComparison {
    pub key: String,
    pub file_value: Option<String>,
    pub runtime_value: Option<String>,
    pub status: EnvStatus,
    pub entry: Option<EnvEntry>,
}

/// Collect all ENV entries from a parsed ShellFile.
pub fn collect_entries(shell_file: &ShellFile) -> Vec<EnvEntry> {
    let mut entries = Vec::new();

    for (idx, node) in shell_file.lines.iter().enumerate() {
        match node {
            LineNode::EnvExport {
                line_number,
                key,
                value,
                export_style,
                quote_style,
                ..
            } => {
                entries.push(EnvEntry {
                    key: key.clone(),
                    value: value.clone(),
                    source_file: shell_file.path.clone(),
                    line_number: *line_number,
                    line_index: idx,
                    location: EntryLocation::InFile,
                    export_style: *export_style,
                    quote_style: *quote_style,
                    is_dirty: false,
                });
            }
            LineNode::ManagedComment {
                line_number,
                original_export,
                tag,
                ..
            } => {
                // Try to extract key from the tag (e.g., "deleted:API_KEY" or "moved:API_KEY -> path")
                if let Some(key) = extract_key_from_tag(tag) {
                    // Parse value from original_export (e.g., "export KEY=\"value\"")
                    let (value, export_style, quote_style) =
                        extract_value_from_export(original_export);
                    entries.push(EnvEntry {
                        key,
                        value,
                        source_file: shell_file.path.clone(),
                        line_number: *line_number,
                        line_index: idx,
                        location: EntryLocation::Commented,
                        export_style,
                        quote_style,
                        is_dirty: false,
                    });
                }
            }
            _ => {}
        }
    }

    entries
}

/// Collect entries from multiple ShellFiles.
pub fn collect_all_entries(shell_files: &[ShellFile]) -> Vec<EnvEntry> {
    shell_files.iter().flat_map(collect_entries).collect()
}

/// Compare file entries with the runtime environment.
pub fn compare_runtime(entries: &[EnvEntry]) -> Vec<EnvComparison> {
    let runtime_vars: HashMap<String, String> = std::env::vars().collect();

    let mut comparisons = Vec::new();
    let mut seen_keys: HashMap<String, bool> = HashMap::new();

    // Check file entries against runtime
    for entry in entries {
        let runtime_value = runtime_vars.get(&entry.key).cloned();
        let status = if entry.location == EntryLocation::Commented {
            EnvStatus::Commented
        } else if runtime_value.is_some() {
            EnvStatus::Active
        } else {
            EnvStatus::InFileOnly
        };

        comparisons.push(EnvComparison {
            key: entry.key.clone(),
            file_value: Some(entry.value.clone()),
            runtime_value,
            status,
            entry: Some(entry.clone()),
        });

        seen_keys.insert(entry.key.clone(), true);
    }

    // Check runtime-only vars (not in any file)
    for (key, value) in &runtime_vars {
        if !seen_keys.contains_key(key) {
            comparisons.push(EnvComparison {
                key: key.clone(),
                file_value: None,
                runtime_value: Some(value.clone()),
                status: EnvStatus::RuntimeOnly,
                entry: None,
            });
        }
    }

    comparisons
}

/// Filter entries by case-insensitive substring match on key or value.
pub fn filter_entries(entries: &[EnvEntry], query: &str) -> Vec<EnvEntry> {
    let query_lower = query.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            e.key.to_lowercase().contains(&query_lower)
                || e.value.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect()
}

/// Extract the key name from an envforge tag.
///
/// Examples:
/// - "deleted:API_KEY" → Some("API_KEY")
/// - "moved:DB_URL -> ~/.env_managed" → Some("DB_URL")
fn extract_key_from_tag(tag: &str) -> Option<String> {
    let parts: Vec<&str> = tag.splitn(2, ':').collect();
    if parts.len() < 2 {
        return None;
    }

    let key_part = parts[1];
    // Handle "KEY -> path" format for moved tags
    let key = key_part.split(" ->").next().unwrap_or(key_part).trim();

    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

/// Extract value, export style, and quote style from an original export line.
///
/// Examples:
/// - `export FOO="bar"` → ("bar", Export, Double)
/// - `BAZ=123` → ("123", Bare, None)
fn extract_value_from_export(export_line: &str) -> (String, ExportStyle, QuoteStyle) {
    let trimmed = export_line.trim();

    let (style, rest) = if let Some(after) = trimmed.strip_prefix("export ") {
        (ExportStyle::Export, after.trim())
    } else {
        (ExportStyle::Bare, trimmed)
    };

    // Find = separator
    if let Some(eq_pos) = rest.find('=') {
        let raw_value = &rest[eq_pos + 1..];
        let trimmed_val = raw_value.trim();

        if trimmed_val.starts_with('"') && trimmed_val.ends_with('"') && trimmed_val.len() >= 2 {
            let inner = &trimmed_val[1..trimmed_val.len() - 1];
            (inner.to_string(), style, QuoteStyle::Double)
        } else if trimmed_val.starts_with('\'')
            && trimmed_val.ends_with('\'')
            && trimmed_val.len() >= 2
        {
            let inner = &trimmed_val[1..trimmed_val.len() - 1];
            (inner.to_string(), style, QuoteStyle::Single)
        } else {
            (trimmed_val.to_string(), style, QuoteStyle::None)
        }
    } else {
        (String::new(), style, QuoteStyle::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_shell_content;
    use std::path::Path;

    fn make_shell_file(content: &str) -> ShellFile {
        parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
    }

    fn make_shell_file_at(content: &str, path: &str) -> ShellFile {
        parse_shell_content(content, Path::new(path)).unwrap()
    }

    // ─── extract_key_from_tag ─────────────────────────────────

    #[test]
    fn test_extract_key_deleted_tag() {
        assert_eq!(
            extract_key_from_tag("deleted:API_KEY"),
            Some("API_KEY".to_string())
        );
    }

    #[test]
    fn test_extract_key_moved_tag() {
        assert_eq!(
            extract_key_from_tag("moved:DB_URL -> ~/.env_managed"),
            Some("DB_URL".to_string())
        );
    }

    #[test]
    fn test_extract_key_no_colon() {
        assert_eq!(extract_key_from_tag("invalid"), None);
    }

    #[test]
    fn test_extract_key_empty_after_colon() {
        assert_eq!(extract_key_from_tag("deleted:"), None);
    }

    // ─── extract_value_from_export ────────────────────────────

    #[test]
    fn test_extract_export_double_quotes() {
        let (val, style, quote) = extract_value_from_export("export FOO=\"bar\"");
        assert_eq!(val, "bar");
        assert_eq!(style, ExportStyle::Export);
        assert_eq!(quote, QuoteStyle::Double);
    }

    #[test]
    fn test_extract_export_single_quotes() {
        let (val, style, quote) = extract_value_from_export("export FOO='bar'");
        assert_eq!(val, "bar");
        assert_eq!(style, ExportStyle::Export);
        assert_eq!(quote, QuoteStyle::Single);
    }

    #[test]
    fn test_extract_bare_no_quotes() {
        let (val, style, quote) = extract_value_from_export("BAZ=123");
        assert_eq!(val, "123");
        assert_eq!(style, ExportStyle::Bare);
        assert_eq!(quote, QuoteStyle::None);
    }

    #[test]
    fn test_extract_no_equals() {
        let (val, _, quote) = extract_value_from_export("something");
        assert_eq!(val, "");
        assert_eq!(quote, QuoteStyle::None);
    }

    // ─── filter_entries ───────────────────────────────────────

    #[test]
    fn test_filter_entries_matches_key_and_value() {
        let sf = make_shell_file("export API_KEY=\"secret\"\nexport DB_HOST=\"localhost\"");
        let entries = collect_entries(&sf);
        let filtered = filter_entries(&entries, "api");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "API_KEY");
    }

    #[test]
    fn test_filter_entries_empty_query_returns_all() {
        let sf = make_shell_file("export A=\"1\"\nexport B=\"2\"");
        let entries = collect_entries(&sf);
        let filtered = filter_entries(&entries, "");
        assert_eq!(filtered.len(), 2);
    }

    // ─── collect_all_entries ──────────────────────────────────

    #[test]
    fn test_collect_all_entries_multiple_files() {
        let sf1 = make_shell_file_at("export A=\"1\"", "/test/.zshrc");
        let sf2 = make_shell_file_at("export B=\"2\"", "/test/.bashrc");
        let entries = collect_all_entries(&[sf1, sf2]);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.key == "A"));
        assert!(entries.iter().any(|e| e.key == "B"));
    }

    #[test]
    fn test_collect_all_entries_empty() {
        let entries = collect_all_entries(&[]);
        assert!(entries.is_empty());
    }
}
