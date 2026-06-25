use std::collections::HashMap;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range as LspRange;

#[derive(Debug, Clone)]
pub struct DocumentState {
    pub content: String,
    pub version: i32,
    pub entries: Vec<EnvDocEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvLineType {
    EnvVar,
    Comment,
    Blank,
    Other,
}

#[derive(Debug, Clone)]
pub struct EnvDocEntry {
    pub key: String,
    pub value: String,
    pub key_range: LspRange,
    pub value_range: LspRange,
    pub line: u32,
    pub line_type: EnvLineType,
}

/// Parse a .env-style document into positioned entries with line type tracking.
pub fn parse_env_document(content: &str) -> Vec<EnvDocEntry> {
    // Bound the per-document entry count. The 1 MiB byte cap upstream still
    // allows ~500K single-char lines (~hundreds of MB of transient entries);
    // cap the line count so a malformed/hostile document can't exhaust memory.
    const MAX_LINES: usize = 50_000;

    let mut entries = Vec::new();

    for (line_num, line) in content.lines().take(MAX_LINES).enumerate() {
        let trimmed = line.trim();
        let ln = line_num as u32;

        if trimmed.is_empty() {
            entries.push(EnvDocEntry {
                key: String::new(),
                value: String::new(),
                key_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: 0,
                    },
                },
                value_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: 0,
                    },
                },
                line: ln,
                line_type: EnvLineType::Blank,
            });
            continue;
        }

        if trimmed.starts_with('#') {
            entries.push(EnvDocEntry {
                key: String::new(),
                value: trimmed.to_string(),
                key_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: line.len() as u32,
                    },
                },
                value_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: line.len() as u32,
                    },
                },
                line: ln,
                line_type: EnvLineType::Comment,
            });
            continue;
        }

        let effective = if let Some(s) = trimmed.strip_prefix("export ") {
            s
        } else {
            trimmed
        };

        if let Some(eq_pos) = effective.find('=') {
            let key = effective[..eq_pos].trim();
            if key.is_empty() {
                entries.push(EnvDocEntry {
                    key: String::new(),
                    value: line.to_string(),
                    key_range: LspRange {
                        start: Position {
                            line: ln,
                            character: 0,
                        },
                        end: Position {
                            line: ln,
                            character: line.len() as u32,
                        },
                    },
                    value_range: LspRange {
                        start: Position {
                            line: ln,
                            character: 0,
                        },
                        end: Position {
                            line: ln,
                            character: line.len() as u32,
                        },
                    },
                    line: ln,
                    line_type: EnvLineType::Other,
                });
                continue;
            }
            let raw_value = &effective[eq_pos + 1..];
            let value = raw_value.trim();
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };

            let key_start = line.find(key).unwrap_or(0) as u32;
            let key_end = key_start + key.len() as u32;
            let val_start_in_line = line.find('=').map(|p| p + 1).unwrap_or(0) as u32;
            let val_end = line.len() as u32;

            entries.push(EnvDocEntry {
                key: key.to_string(),
                value: value.to_string(),
                key_range: LspRange {
                    start: Position {
                        line: ln,
                        character: key_start,
                    },
                    end: Position {
                        line: ln,
                        character: key_end,
                    },
                },
                value_range: LspRange {
                    start: Position {
                        line: ln,
                        character: val_start_in_line,
                    },
                    end: Position {
                        line: ln,
                        character: val_end,
                    },
                },
                line: ln,
                line_type: EnvLineType::EnvVar,
            });
        } else {
            entries.push(EnvDocEntry {
                key: String::new(),
                value: line.to_string(),
                key_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: line.len() as u32,
                    },
                },
                value_range: LspRange {
                    start: Position {
                        line: ln,
                        character: 0,
                    },
                    end: Position {
                        line: ln,
                        character: line.len() as u32,
                    },
                },
                line: ln,
                line_type: EnvLineType::Other,
            });
        }
    }

    entries
}

/// Extract only the EnvVar entries (backward-compatible helper).
#[allow(dead_code)]
pub fn env_var_entries(entries: &[EnvDocEntry]) -> Vec<&EnvDocEntry> {
    entries
        .iter()
        .filter(|e| e.line_type == EnvLineType::EnvVar)
        .collect()
}

/// Scan `.env.schema` content for `[SECTION_HEADER]` line numbers (for go-to-definition).
pub fn schema_line_map(content: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let re = regex::Regex::new(r"^\[([A-Za-z_][A-Za-z0-9_]*)\]")
        .expect("hardcoded schema section regex is valid");
    for (line_num, line) in content.lines().enumerate() {
        if let Some(caps) = re.captures(line.trim()) {
            map.insert(caps[1].to_string(), line_num as u32);
        }
    }
    map
}
