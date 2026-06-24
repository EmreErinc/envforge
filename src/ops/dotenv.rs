use std::path::Path;

use chrono::Local;

use crate::config::AppConfig;
use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};
use crate::ops::crud::{add_entry, edit_entry, find_soft_deleted, undo_delete};
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

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let raw_value = trimmed[eq_pos + 1..].trim().to_string();

            if key.is_empty() {
                continue;
            }

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
        let exists = shell_file.lines.iter().any(|node| match node {
            LineNode::EnvExport { key, .. } => key == &entry.key,
            _ => false,
        });

        if exists {
            if force {
                if edit_entry(shell_file, &entry.key, &entry.value).is_ok() {
                    overwritten += 1;
                } else {
                    skipped += 1;
                }
            } else {
                skipped += 1;
            }
        } else {
            if let Some(_idx) = find_soft_deleted(shell_file, &entry.key) {
                if force {
                    undo_delete(shell_file, &entry.key).ok();
                    edit_entry(shell_file, &entry.key, &entry.value).ok();
                    overwritten += 1;
                } else {
                    skipped += 1;
                }
            } else {
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
        if entry.location == EntryLocation::Commented {
            continue;
        }

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
pub fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("credential")
        || (lower.contains("key") && !lower.contains("keyboard"))
}

/// Export ENV entries with sensitive values redacted as `[REDACTED]`.
/// Uses pattern-based detection + schema sensitive=true field.
pub fn export_safe(
    entries: &[EnvEntry],
    schema_sensitive_keys: &std::collections::HashSet<String>,
) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "# Safe export by EnvForge on {} (sensitive values redacted)\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    output.push('\n');

    for entry in entries {
        if entry.location == EntryLocation::Commented {
            continue;
        }

        let is_sensitive =
            is_sensitive_key(&entry.key) || schema_sensitive_keys.contains(&entry.key);

        let value = if is_sensitive {
            "[REDACTED]".to_string()
        } else if needs_quoting(&entry.value) {
            format!("\"{}\"", entry.value)
        } else {
            entry.value.clone()
        };

        output.push_str(&format!("{}={}\n", entry.key, value));
    }

    output
}

/// Generate .env.example from schema with placeholder values.
pub fn export_env_example(schema: &crate::ops::schema::EnvSchema) -> String {
    let mut output = String::new();
    output.push_str("# .env.example — generated by EnvForge\n");
    output.push_str("# Copy to .env and fill in your values\n\n");

    let mut vars: Vec<(&String, &crate::ops::schema::SchemaVariable)> =
        schema.variables.iter().collect();
    vars.sort_by(|a, b| b.1.required.cmp(&a.1.required).then(a.0.cmp(b.0)));

    for (name, var) in &vars {
        if let Some(ref desc) = var.description {
            output.push_str(&format!(
                "# {} ({}{})\n",
                desc,
                var.var_type.display(),
                if var.required { ", required" } else { "" }
            ));
        }

        let placeholder = if let Some(ref example) = var.example {
            example.clone()
        } else if let Some(ref default) = var.default {
            default.clone()
        } else if var.sensitive {
            format!("<your-{}>", name.to_lowercase().replace('_', "-"))
        } else {
            match var.var_type {
                crate::ops::schema::VarType::Number => "0".into(),
                crate::ops::schema::VarType::Bool => "false".into(),
                crate::ops::schema::VarType::Url => "https://example.com".into(),
                crate::ops::schema::VarType::Email => "user@example.com".into(),
                crate::ops::schema::VarType::Port => "3000".into(),
                crate::ops::schema::VarType::Enum => var
                    .values
                    .as_ref()
                    .and_then(|v| v.first().cloned())
                    .unwrap_or_else(|| "value".into()),
                _ => format!("<{}>", name.to_lowercase().replace('_', "-")),
            }
        };

        output.push_str(&format!("{}={}\n", name, placeholder));
    }

    output
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
///
/// Requires length ≥ 2 so a lone quote char (`"`) is not treated as both
/// the opening and closing quote — that previously panicked on the
/// `value[1..len-1]` slice (`1..0`). The quote chars are single-byte ASCII,
/// so the slice stays on char boundaries.
fn strip_quotes(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
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

    /// Regression (Story 2.4): a lone quote char as a value must not panic
    /// `strip_quotes` (`value[1..0]`). Found by the MCP no-secret property test.
    #[test]
    fn test_strip_quotes_lone_quote_no_panic() {
        assert_eq!(strip_quotes("\""), "\"");
        assert_eq!(strip_quotes("'"), "'");
        assert_eq!(strip_quotes(""), "");
        assert_eq!(strip_quotes("\"x\""), "x");
        // parse_dotenv must not panic on a value that is a single quote.
        let entries = parse_dotenv_content("KEY=\"");
        assert_eq!(entries.len(), 1);
    }

    // ─── parse_dotenv_content ─────────────────────────────────

    #[test]
    fn test_parse_basic_key_value() {
        let entries = parse_dotenv_content("FOO=bar");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "FOO");
        assert_eq!(entries[0].value, "bar");
    }

    #[test]
    fn test_parse_quoted_values() {
        let entries = parse_dotenv_content("A=\"hello\"\nB='world'");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].value, "hello");
        assert_eq!(entries[1].value, "world");
    }

    #[test]
    fn test_parse_skips_comments_and_blanks() {
        let entries = parse_dotenv_content("# comment\n\nFOO=bar\n# another");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "FOO");
    }

    #[test]
    fn test_parse_empty_key_skipped() {
        let entries = parse_dotenv_content("=value");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_multiline_content() {
        let entries = parse_dotenv_content("A=1\nB=2\nC=3");
        assert_eq!(entries.len(), 3);
    }

    // ─── is_sensitive_key ─────────────────────────────────────

    #[test]
    fn test_sensitive_secret() {
        assert!(is_sensitive_key("AWS_SECRET"));
    }

    #[test]
    fn test_sensitive_token() {
        assert!(is_sensitive_key("AUTH_TOKEN"));
    }

    #[test]
    fn test_sensitive_password() {
        assert!(is_sensitive_key("DB_PASSWORD"));
    }

    #[test]
    fn test_sensitive_keyboard_excluded() {
        assert!(!is_sensitive_key("KEYBOARD_LAYOUT"));
    }

    #[test]
    fn test_not_sensitive() {
        assert!(!is_sensitive_key("DB_HOST"));
        assert!(!is_sensitive_key("PORT"));
    }

    // ─── export_entries ───────────────────────────────────────

    #[test]
    fn test_export_entries_basic() {
        let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=\"qux\"");
        let entries = crate::ops::listing::collect_entries(&sf);
        let output = export_entries(&entries, false, None);
        assert!(output.contains("FOO=bar"));
        assert!(output.contains("BAZ=qux"));
    }

    #[test]
    fn test_export_entries_excludes_sensitive() {
        let sf = make_shell_file("export API_KEY=\"secret\"\nexport HOST=\"localhost\"");
        let entries = crate::ops::listing::collect_entries(&sf);
        let output = export_entries(&entries, true, None);
        assert!(
            !output.contains("API_KEY"),
            "Sensitive key should be excluded"
        );
        assert!(output.contains("HOST"));
    }

    // ─── export_safe ──────────────────────────────────────────

    #[test]
    fn test_export_safe_redacts_sensitive() {
        let sf = make_shell_file("export API_KEY=\"secret\"\nexport HOST=\"localhost\"");
        let entries = crate::ops::listing::collect_entries(&sf);
        let output = export_safe(&entries, &std::collections::HashSet::new());
        assert!(output.contains("API_KEY=[REDACTED]"));
        assert!(output.contains("HOST=localhost"));
    }

    #[test]
    fn test_export_safe_schema_sensitive() {
        let sf = make_shell_file("export CUSTOM=\"value\"");
        let entries = crate::ops::listing::collect_entries(&sf);
        let mut schema_keys = std::collections::HashSet::new();
        schema_keys.insert("CUSTOM".to_string());
        let output = export_safe(&entries, &schema_keys);
        assert!(output.contains("CUSTOM=[REDACTED]"));
    }

    // ─── strip_quotes / needs_quoting ─────────────────────────

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'world'"), "world");
        assert_eq!(strip_quotes("bare"), "bare");
    }

    #[test]
    fn test_needs_quoting() {
        assert!(needs_quoting("has space"));
        assert!(needs_quoting("has#hash"));
        assert!(needs_quoting(""));
        assert!(!needs_quoting("simple"));
    }
}
