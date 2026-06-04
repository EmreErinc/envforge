use std::sync::OnceLock;

use regex::Regex;
use tower_lsp::lsp_types::{Position, Range, TextEdit};

/// Canonicalize an env document. Conservative formatter — preserves
/// comment text, key ordering, value contents (including content
/// inside quotes), and the user's blank-line groupings. Only touches
/// whitespace that has no semantic meaning in `.env` files:
///
/// - Trim trailing whitespace from every line.
/// - Normalize `KEY = value`, `KEY  =value`, `KEY=  value` → `KEY=value`.
/// - Normalize `export   FOO=…` → `export FOO=…` (single space).
/// - Collapse runs of 3 or more consecutive blank lines down to 2.
/// - Ensure exactly one trailing newline at end of file.
///
/// What we deliberately do NOT do:
///
/// - Reorder keys (would clobber intent and break diffs).
/// - Strip whitespace inside quoted values (`FOO="hello "` stays).
/// - Touch unparseable / non-env lines (parser is byte-identical).
/// - Dedupe duplicate keys (user may want to see them).
pub fn format_document(content: &str) -> String {
    let env_line = env_line_regex();
    let mut formatted_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_end();
        if let Some(cap) = env_line.captures(trimmed) {
            let indent = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let export = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let key = cap.get(3).map(|m| m.as_str()).unwrap_or("");
            let value = cap.get(4).map(|m| m.as_str()).unwrap_or("");
            let export_segment = if export.trim().is_empty() {
                ""
            } else {
                "export "
            };
            formatted_lines.push(format!("{}{}{}={}", indent, export_segment, key, value));
        } else {
            formatted_lines.push(trimmed.to_string());
        }
    }

    let mut collapsed: Vec<String> = Vec::with_capacity(formatted_lines.len());
    let mut blank_run = 0usize;
    for line in formatted_lines {
        if line.is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                collapsed.push(String::new());
            }
        } else {
            blank_run = 0;
            collapsed.push(line);
        }
    }

    let mut out = collapsed.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Build a single `TextEdit` that replaces the whole document with the
/// formatted version. Returns an empty vector if the content already
/// matches the formatted output so editors don't dirty unchanged buffers.
pub fn format_text_edits(content: &str) -> Vec<TextEdit> {
    let formatted = format_document(content);
    if formatted == content {
        return Vec::new();
    }
    vec![TextEdit {
        range: full_range(content),
        new_text: formatted,
    }]
}

fn env_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*)((?:export\s+)?)([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)$")
            .expect("hardcoded env line regex is valid")
    })
}

/// Compute the document-spanning range for a full-replace `TextEdit`.
/// `Position` is line-and-UTF-16-code-unit per LSP; for `.env` files
/// that contain almost exclusively ASCII this is also char count.
fn full_range(content: &str) -> Range {
    if content.is_empty() {
        return Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };
    }

    let line_count = content.lines().count();
    if content.ends_with('\n') {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: line_count as u32,
                character: 0,
            },
        }
    } else {
        let last_line = content.lines().last().unwrap_or("");
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: (line_count.saturating_sub(1)) as u32,
                character: last_line.chars().count() as u32,
            },
        }
    }
}
