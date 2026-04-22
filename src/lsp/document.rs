use tower_lsp::lsp_types::Range as LspRange;
use tower_lsp::lsp_types::Position;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DocumentState {
    pub content: String,
    pub version: i32,
    pub entries: Vec<EnvDocEntry>,
}

#[derive(Debug, Clone)]
pub struct EnvDocEntry {
    pub key: String,
    pub value: String,
    pub key_range: LspRange,
    pub value_range: LspRange,
    pub line: u32,
}

/// Parse a .env-style document into positioned entries.
pub fn parse_env_document(content: &str) -> Vec<EnvDocEntry> {
    let mut entries = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Strip optional "export " prefix
        let effective = if let Some(s) = trimmed.strip_prefix("export ") {
            s
        } else {
            trimmed
        };

        if let Some(eq_pos) = effective.find('=') {
            let key = effective[..eq_pos].trim();
            if key.is_empty() {
                continue;
            }
            let raw_value = &effective[eq_pos + 1..];
            // Strip surrounding quotes
            let value = raw_value.trim();
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };

            // Calculate column positions in original line
            let key_start = line.find(key).unwrap_or(0) as u32;
            let key_end = key_start + key.len() as u32;
            let val_start_in_line = line.find('=').map(|p| p + 1).unwrap_or(0) as u32;
            let val_end = line.len() as u32;

            let ln = line_num as u32;
            entries.push(EnvDocEntry {
                key: key.to_string(),
                value: value.to_string(),
                key_range: LspRange {
                    start: Position { line: ln, character: key_start },
                    end: Position { line: ln, character: key_end },
                },
                value_range: LspRange {
                    start: Position { line: ln, character: val_start_in_line },
                    end: Position { line: ln, character: val_end },
                },
                line: ln,
            });
        }
    }

    entries
}

/// Scan .env.schema content for [SECTION_HEADER] line numbers (for go-to-definition).
pub fn schema_line_map(content: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let re = regex::Regex::new(r"^\[([A-Za-z_][A-Za-z0-9_]*)\]").unwrap();
    for (line_num, line) in content.lines().enumerate() {
        if let Some(caps) = re.captures(line.trim()) {
            map.insert(caps[1].to_string(), line_num as u32);
        }
    }
    map
}
