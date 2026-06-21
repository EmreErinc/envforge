//! Byte-preserving parser for Java `.properties` and `.env`-cascade files
//! — Story 002 (FR-document-basis, NFR2, NFR8, NFR9, AR2, AR4).
//!
//! Both formats share the same flat `KEY=value` / `KEY: value` shape, so
//! one parser handles both. Round-trip byte-equality is guaranteed:
//! only the structured `ConfigEntry` fields are used for language
//! features; the raw text is never re-serialised from the parsed model.
//!
//! `.properties` additionally supports:
//! - `#` and `!` as comment prefixes.
//! - `key: value` (colon separator, optional surrounding whitespace).
//! - `key = value` (equals separator with surrounding whitespace).
//! - `key=value` (equals separator, no whitespace).
//! - Backslash line-continuation (treated as a single logical line for the
//!   entry model; the physical line numbers of the first line are preserved).
//!
//! `.env`-cascade files follow the simpler `KEY=value` / `export KEY=value`
//! syntax already used by `parse_env_document` in `document.rs`, but we
//! re-implement here to produce `ConfigEntry` (with `source_layer`) rather
//! than `EnvDocEntry`.

use tower_lsp::lsp_types::{Position, Range as LspRange};

use super::config_format::{ConfigEntry, SourceLayer};

// ── Public API ──────────────────────────────────────────────────────────────

/// Parse a `.properties` file (Java / Quarkus / MicroProfile) into a list of
/// positioned entries. Comment lines and blank lines produce entries with empty
/// `key` so callers can skip them with `entry.key.is_empty()`.
pub fn parse_properties(content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
    parse_impl(content, layer, Format::Properties)
}

/// Parse a `.env`-cascade file into a list of positioned entries.
pub fn parse_dotenv_cascade(content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
    parse_impl(content, layer, Format::DotEnv)
}

// ── Internal ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Format {
    Properties,
    DotEnv,
}

fn parse_impl(content: &str, layer: SourceLayer, fmt: Format) -> Vec<ConfigEntry> {
    const MAX_LINES: usize = 50_000;
    // Strip a leading UTF-8 BOM (\u{FEFF}) before parsing so the first key
    // is not dropped or mis-parsed on BOM-prefixed files (BOM fix).
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let mut entries = Vec::new();
    let mut lines_iter = content.lines().enumerate().take(MAX_LINES).peekable();

    while let Some((line_num, raw_line)) = lines_iter.next() {
        let ln = line_num as u32;
        let trimmed = raw_line.trim();

        // Blank line
        if trimmed.is_empty() {
            entries.push(blank_entry(ln, &layer));
            continue;
        }

        // Comment prefix
        let is_comment = match fmt {
            Format::Properties => trimmed.starts_with('#') || trimmed.starts_with('!'),
            Format::DotEnv => trimmed.starts_with('#'),
        };
        if is_comment {
            entries.push(comment_entry(ln, raw_line, &layer));
            continue;
        }

        // Line continuation for `.properties` (backslash at end of line).
        // We consume continuation lines but attribute the entry to the
        // first physical line — byte-identical round-trip is guaranteed
        // because we only READ the raw content for the entry model.
        let mut logical = raw_line.to_string();
        if let Format::Properties = fmt {
            while logical.trim_end().ends_with('\\') {
                // Remove the trailing backslash and append the next line.
                let trimmed_back = logical.trim_end().trim_end_matches('\\');
                logical = trimmed_back.to_string();
                if let Some((_, next_line)) = lines_iter.peek() {
                    logical.push_str(next_line.trim_start());
                    let _ = lines_iter.next(); // consume continuation line
                } else {
                    break;
                }
            }
        }

        if let Some(entry) = parse_kv_line(ln, &logical, &layer, fmt) {
            entries.push(entry);
        } else {
            // Unrecognised / non-KV line — store as "other"
            entries.push(other_entry(ln, raw_line, &layer));
        }
    }

    entries
}

/// Try to parse a logical line as `key = value` / `key: value` / `KEY=value`
/// / `export KEY=value`. Returns `None` for lines that don't match.
fn parse_kv_line(ln: u32, logical: &str, layer: &SourceLayer, fmt: Format) -> Option<ConfigEntry> {
    let effective = match fmt {
        Format::DotEnv => logical
            .trim()
            .strip_prefix("export ")
            .map(str::trim_start)
            .unwrap_or_else(|| logical.trim()),
        Format::Properties => logical.trim(),
    };

    // Find separator: `=` or `:` (properties); only `=` for dotenv.
    let sep_pos = match fmt {
        Format::DotEnv => effective.find('='),
        Format::Properties => {
            // Prefer `=` at first occurrence; fall back to first `:`.
            let eq = effective.find('=');
            let col = effective.find(':');
            match (eq, col) {
                (Some(e), Some(c)) => Some(e.min(c)),
                (Some(e), None) => Some(e),
                (None, Some(c)) => Some(c),
                (None, None) => None,
            }
        }
    };

    let sep_pos = sep_pos?;

    let raw_key = effective[..sep_pos].trim();
    if raw_key.is_empty() {
        return None;
    }

    // Validate key: must start with letter or underscore.
    {
        let first = raw_key.chars().next()?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }
    }

    let raw_value = effective[sep_pos + 1..].trim();

    // Strip surrounding quotes (double or single) for dotenv.
    let value = match fmt {
        Format::DotEnv => {
            if (raw_value.starts_with('"') && raw_value.ends_with('"'))
                || (raw_value.starts_with('\'') && raw_value.ends_with('\''))
            {
                &raw_value[1..raw_value.len() - 1]
            } else {
                raw_value
            }
        }
        Format::Properties => raw_value,
    };

    // Compute UTF-16 character positions within the ORIGINAL raw line.
    // We search for the key substring starting from the beginning. For
    // the value we use the separator position in the original line.
    let original_line = logical;
    let key_start = find_key_char_offset(original_line, raw_key) as u32;
    let key_end = key_start + utf16_len(raw_key) as u32;

    let val_sep_byte = find_sep_byte_in_original(original_line, sep_pos);
    // val_start must point at the first value character, not the separator.
    // Skip the separator byte itself (+1) and any following whitespace, then
    // compute the UTF-16 column of the first non-whitespace value char.
    // For `KEY = value` the separator byte is `=`, so we skip `=` + space(s).
    let after_sep = {
        let after_sep_byte = val_sep_byte + 1; // byte just after the separator
        if after_sep_byte < original_line.len() {
            // Count leading whitespace bytes after the separator.
            let after_sep_str = &original_line[after_sep_byte..];
            let ws_bytes = after_sep_str.len() - after_sep_str.trim_start().len();
            after_sep_byte + ws_bytes
        } else {
            after_sep_byte
        }
    };
    let val_start = utf16_char_count(&original_line[..after_sep]) as u32;
    let val_end = utf16_char_count(original_line.trim_end()) as u32;
    let val_end = val_end.max(val_start);

    Some(ConfigEntry {
        key: raw_key.to_string(),
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
                character: val_start,
            },
            end: Position {
                line: ln,
                character: val_end,
            },
        },
        line: ln,
        source_layer: layer.clone(),
    })
}

// ── Helper constructors ───────────────────────────────────────────────────────

fn blank_entry(ln: u32, layer: &SourceLayer) -> ConfigEntry {
    let pos = Position {
        line: ln,
        character: 0,
    };
    let range = LspRange {
        start: pos,
        end: pos,
    };
    ConfigEntry {
        key: String::new(),
        value: String::new(),
        key_range: range,
        value_range: range,
        line: ln,
        source_layer: layer.clone(),
    }
}

fn comment_entry(ln: u32, raw_line: &str, layer: &SourceLayer) -> ConfigEntry {
    let len = utf16_char_count(raw_line) as u32;
    let range = LspRange {
        start: Position {
            line: ln,
            character: 0,
        },
        end: Position {
            line: ln,
            character: len,
        },
    };
    ConfigEntry {
        key: String::new(),
        value: raw_line.to_string(),
        key_range: range,
        value_range: range,
        line: ln,
        source_layer: layer.clone(),
    }
}

fn other_entry(ln: u32, raw_line: &str, layer: &SourceLayer) -> ConfigEntry {
    let len = utf16_char_count(raw_line) as u32;
    let range = LspRange {
        start: Position {
            line: ln,
            character: 0,
        },
        end: Position {
            line: ln,
            character: len,
        },
    };
    ConfigEntry {
        key: String::new(),
        value: raw_line.to_string(),
        key_range: range,
        value_range: range,
        line: ln,
        source_layer: layer.clone(),
    }
}

// ── Position helpers ──────────────────────────────────────────────────────────

/// UTF-16 code-unit length of `s`. For BMP-only strings this equals
/// `s.chars().count()`; supplementary plane chars use 2 units.
fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

/// UTF-16 code-unit count of `s` (same as `utf16_len`).
fn utf16_char_count(s: &str) -> usize {
    utf16_len(s)
}

/// Find the byte offset within `original_line` where `key` appears,
/// searching from the start. Returns 0 if not found (safe fallback).
fn find_key_char_offset(original_line: &str, key: &str) -> usize {
    let Some(byte_off) = original_line.find(key) else {
        return 0;
    };
    utf16_char_count(&original_line[..byte_off])
}

/// Map the separator position in the trimmed `effective` string back to a
/// byte offset in `original_line` (accounts for leading whitespace and
/// `export` + any number of trailing spaces prefix).
///
/// Fix for H4: the original code used a fixed `"export ".len()` (7) offset,
/// but `parse_kv_line` trims all whitespace after `export` before looking for
/// the separator. So `export  FOO=bar` (two spaces) would yield `sep_pos`
/// measured from `FOO`, not from `export  FOO`. We must compute how many bytes
/// `export` + its following whitespace actually consume in the trimmed string.
fn find_sep_byte_in_original(original_line: &str, sep_pos: usize) -> usize {
    let trimmed = original_line.trim();
    // Account for leading whitespace bytes.
    let lead_bytes = original_line.len() - original_line.trim_start().len();
    // Account for `export` + all following whitespace bytes.
    // `strip_prefix("export")` gives us the tail starting after the word;
    // we then measure how many leading spaces that tail has.
    let export_bytes = trimmed.strip_prefix("export").map_or(0, |after_export| {
        let spaces = after_export.len() - after_export.trim_start().len();
        "export".len() + spaces
    });
    lead_bytes + export_bytes + sep_pos
}

// Tests for this module live in tests/properties_env_intelligence_tests.rs
// per the CLAUDE.md convention: "All tests live in tests/ (no in-module tests)".
