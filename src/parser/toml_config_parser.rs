//! Format-preserving TOML config parser — Intent 037, Story 002 (FR2, NFR14).
//!
//! Parses canonical TOML config files (`Cargo.toml`, `pyproject.toml`,
//! `config.toml`, `.cargo/config.toml`) via `toml_edit` into the
//! format-agnostic [`ConfigEntry`] model used by LSP language-feature
//! handlers.
//!
//! # Design constraints
//! - **Format-preserving**: uses `toml_edit` CST; `DocumentMut::to_string()`
//!   returns byte-identical content, so writes are lossless.
//! - **No panics**: malformed TOML returns a `TomlParseError`; callers
//!   degrade to diagnostics rather than crashing.
//! - **UTF-16-correct positions**: all `position.character` values are
//!   UTF-16 code-unit offsets (LSP spec), not byte offsets. Derived from
//!   key/value text scanning.
//! - **Dotted table-path keys**: nested table `[a.b]` + scalar `key = val`
//!   produces the flattened key `a.b.key`. Inline tables, arrays-of-tables,
//!   and dotted-key assignments are all handled unambiguously.
//!
//! # Key flattening rules
//! | Source                         | Flattened key         |
//! |--------------------------------|-----------------------|
//! | `[table]\nkey = "v"`           | `table.key`           |
//! | `[a.b]\nkey = "v"`             | `a.b.key`             |
//! | `[[bin]]\nname = "foo"`        | `bin.name` (idx=0)    |
//! | `x.y = "z"` (dotted key)       | `x.y`                 |
//! | `key = "v"` (top-level)        | `key`                 |
//!
//! # Position strategy
//! `toml_edit`'s `Value::span()` is unreliable for standard-table items when
//! the `Repr` is deserialized (the span can be `None`). Instead we scan the
//! source text line by line for each parsed key, using the line number from the
//! `toml_edit`'s `Item::span()` as a hint when available. When no span hint
//! exists we fall back to a text search over the full source. All character
//! columns are converted to UTF-16 code units.

use toml_edit::{DocumentMut, Item, Value};
use tower_lsp::lsp_types::{Position, Range as LspRange};

use crate::ops::config_format::{ConfigEntry, SourceLayer};

// BUG-8: track whether an entry came from an ArrayOfTables so duplicate-key
// diagnostics can exempt AoT-originated keys (repeated [[bin]] is legal).
/// Per-entry flag: `true` when the entry was derived from an
/// `[[array-of-tables]]` block. Stored as a parallel `Vec` to avoid widening
/// the public `ConfigEntry` struct (which is shared across all format handlers).
pub type AotFlags = Vec<bool>;

// ── Error type ────────────────────────────────────────────────────────────────

/// Error returned when TOML content cannot be parsed.
#[derive(Debug, Clone)]
pub struct TomlParseError {
    /// Human-readable message suitable for an LSP diagnostic.
    pub message: String,
    /// 0-based line number, if available (derived from toml_edit error).
    pub line: Option<u32>,
}

impl std::fmt::Display for TomlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(l) = self.line {
            write!(f, "TOML parse error on line {}: {}", l + 1, self.message)
        } else {
            write!(f, "TOML parse error: {}", self.message)
        }
    }
}

// ── Position helpers ──────────────────────────────────────────────────────────

/// Convert a 0-based byte offset within a source string into a (0-based line,
/// UTF-16 character-column) pair suitable for an LSP `Position`.
///
/// `toml_edit` span ranges are byte offsets into the original source string.
/// LSP positions use UTF-16 code units per line. This function handles the
/// conversion correctly for multi-byte Unicode content (e.g. non-ASCII keys or
/// quoted string values).
fn byte_offset_to_lsp_position(source: &str, byte_offset: usize) -> Position {
    let safe_offset = byte_offset.min(source.len());
    let prefix = &source[..safe_offset];
    let line = prefix.chars().filter(|&c| c == '\n').count() as u32;
    // Find start of the current line.
    let line_start = prefix.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_content = &prefix[line_start..];
    // Convert byte-column to UTF-16 code units.
    let character: u32 = line_content.chars().map(|c| c.len_utf16() as u32).sum();
    Position { line, character }
}

/// Build an [`LspRange`] from two byte offsets in `source`.
fn span_to_lsp_range(source: &str, start: usize, end: usize) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(source, start),
        end: byte_offset_to_lsp_position(source, end),
    }
}

/// Split `source` into a `Vec` of `(line_number, byte_start, &str)` triples.
/// `line_number` is 0-based. `byte_start` is the byte offset of the line start.
///
/// BUG-6 fix: accounts for `\r\n` line endings — each `\r\n` line consumes
/// `line.len() + 2` bytes, not `+ 1`, so byte offsets are correct on CRLF input.
fn source_lines(source: &str) -> Vec<(u32, usize, &str)> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    for (i, line) in source.lines().enumerate() {
        result.push((i as u32, offset, line));
        // `.lines()` strips both `\n` and `\r\n`; determine which was present.
        let consumed = line.len()
            + if source[offset..].starts_with(&format!("{}\r\n", line)) {
                2
            } else {
                1
            };
        offset += consumed;
    }
    result
}

/// Convert a char-column (0-based) within `line_text` to a UTF-16 code-unit
/// column. If `char_col` exceeds the line length, clamps to line end.
fn char_col_to_utf16_col(line_text: &str, char_col: usize) -> u32 {
    line_text
        .chars()
        .take(char_col)
        .map(|c| c.len_utf16() as u32)
        .sum()
}

// ── Diagnostic shape ──────────────────────────────────────────────────────────

/// A lightweight diagnostic produced during TOML parsing (syntax errors,
/// duplicate keys).  Callers convert these to
/// `tower_lsp::lsp_types::Diagnostic` for publishing.
#[derive(Debug, Clone)]
pub struct TomlDiagnostic {
    pub range: LspRange,
    pub message: String,
    pub severity: tower_lsp::lsp_types::DiagnosticSeverity,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a canonical TOML config file into a list of positioned entries **and**
/// any diagnostics encountered (duplicate keys, syntax errors).
///
/// On a syntax error the function returns `Err(TomlParseError)` with a
/// diagnostic-friendly message. Callers should surface this as a
/// `DiagnosticSeverity::ERROR` at line 0 if no positional information is
/// available from the error.
///
/// # Round-trip guarantee
/// The returned `DocumentMut` can be converted back with `.to_string()` and
/// the result will be byte-identical to `content` (toml_edit lossless CST).
///
/// # Example
/// ```
/// use envforge::parser::toml_config_parser::parse_toml_config;
/// use envforge::ops::config_format::SourceLayer;
///
/// let toml = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n";
/// let (entries, diags, _doc) = parse_toml_config(toml, SourceLayer::Unknown).unwrap();
/// assert!(diags.is_empty());
/// assert!(entries.iter().any(|e| e.key == "package.name"));
/// ```
pub fn parse_toml_config(
    content: &str,
    layer: SourceLayer,
) -> Result<(Vec<ConfigEntry>, Vec<TomlDiagnostic>, DocumentMut), TomlParseError> {
    let (entries, flags, diags, doc) = parse_toml_config_with_aot_flags(content, layer)?;
    // Drop the AoT flags — public API surface stays the same.
    let _ = flags;
    Ok((entries, diags, doc))
}

/// Extended parse that also returns per-entry AoT origin flags.
///
/// Used internally by `config_toml_diagnostics` so that entries from
/// `[[array-of-tables]]` blocks are excluded from the duplicate-key check
/// (BUG-8).
pub fn parse_toml_config_with_aot_flags(
    content: &str,
    layer: SourceLayer,
) -> Result<(Vec<ConfigEntry>, AotFlags, Vec<TomlDiagnostic>, DocumentMut), TomlParseError> {
    let doc: DocumentMut = content.parse().map_err(|e: toml_edit::TomlError| {
        // toml_edit errors carry a span range (byte offsets).
        let line = if let Some(span) = e.span() {
            let pos = byte_offset_to_lsp_position(content, span.start);
            Some(pos.line)
        } else {
            None
        };
        TomlParseError {
            message: e.to_string(),
            line,
        }
    })?;

    let mut entries = Vec::new();
    let mut aot_flags: AotFlags = Vec::new();
    let mut diags = Vec::new();

    // Pre-split source into lines for position lookup.
    let lines: Vec<(u32, usize, &str)> = source_lines(content);

    // Walk the document top-level items.
    collect_entries(
        content,
        &lines,
        doc.as_table(),
        &[],
        &layer,
        false,
        &mut entries,
        &mut aot_flags,
        &mut diags,
    );

    Ok((entries, aot_flags, diags, doc))
}

/// Walk a `toml_edit::Table` recursively, appending [`ConfigEntry`] items.
///
/// `table_path` is the dot-joined path of ancestor table header names (empty
/// for the top-level table). `in_aot` is `true` when we are inside a
/// `[[array-of-tables]]` element — BUG-8 fix: entries from AoT blocks are
/// tagged so `toml_duplicate_key_diagnostics` can skip them.
#[allow(clippy::only_used_in_recursion)]
#[allow(clippy::too_many_arguments)]
fn collect_entries<'src>(
    source: &'src str,
    lines: &[(u32, usize, &'src str)],
    table: &toml_edit::Table,
    table_path: &[&str],
    layer: &SourceLayer,
    in_aot: bool,
    entries: &mut Vec<ConfigEntry>,
    aot_flags: &mut AotFlags,
    diags: &mut Vec<TomlDiagnostic>,
) {
    for (raw_key, item) in table {
        // Build the full dotted path key for this item.
        let mut path_parts: Vec<&str> = table_path.to_vec();
        path_parts.push(raw_key);

        match item {
            Item::Value(value) => {
                // Scalar / inline-table / array value — emit a ConfigEntry.
                if let Some(entry) = value_to_entry(source, lines, &path_parts, value, layer) {
                    entries.push(entry);
                    aot_flags.push(in_aot);
                }
            }
            Item::Table(sub_table) => {
                // Recurse into a standard `[table]` section.
                collect_entries(
                    source,
                    lines,
                    sub_table,
                    &path_parts,
                    layer,
                    in_aot,
                    entries,
                    aot_flags,
                    diags,
                );
            }
            Item::ArrayOfTables(aot) => {
                // `[[table]]` — recurse into each element; mark as AoT.
                for element in aot {
                    collect_entries(
                        source,
                        lines,
                        element,
                        &path_parts,
                        layer,
                        true,
                        entries,
                        aot_flags,
                        diags,
                    );
                }
            }
            Item::None => {}
        }
    }
}

/// Convert a `toml_edit::Value` to a [`ConfigEntry`].
///
/// The key is the full dotted path (e.g. `dependencies.serde`).
/// The value is the string representation of the TOML value.
/// Positions are derived from text-scanning the source for the leaf key.
fn value_to_entry<'src>(
    source: &'src str,
    lines: &[(u32, usize, &'src str)],
    path_parts: &[&str],
    value: &Value,
    layer: &SourceLayer,
) -> Option<ConfigEntry> {
    let key = path_parts.join(".");

    // Derive value string — use TOML repr for complex types; unquote strings.
    let value_str = toml_value_to_string(value);

    // Derive positions: try toml_edit span first; fall back to text search.
    let (key_range, value_range, line_no) = derive_positions(source, lines, path_parts, value);

    Some(ConfigEntry {
        key,
        value: value_str,
        key_range,
        value_range,
        line: line_no,
        source_layer: layer.clone(),
    })
}

/// Derive key and value LSP ranges.
///
/// Strategy:
/// 1. If `value.span()` is available (byte offsets), use it for the value range
///    and back-compute the key range from the same source line.
/// 2. Otherwise, scan lines for a `leaf_key = <value>` assignment and derive
///    positions from text search. This handles the case where toml_edit's span
///    is `None` for deserialized standard-table items.
///
/// Returns `(key_range, value_range, line_number)`.
fn derive_positions(
    source: &str,
    lines: &[(u32, usize, &str)],
    path_parts: &[&str],
    value: &Value,
) -> (LspRange, LspRange, u32) {
    let leaf = path_parts.last().copied().unwrap_or("");
    let value_str = toml_value_to_string(value);

    // ── Strategy 1: toml_edit span ───────────────────────────────────────────
    if let Some(val_span) = value.span() {
        let value_range = span_to_lsp_range(source, val_span.start, val_span.end);
        let line_no = value_range.start.line;

        // Find the key on the same line by back-scanning to line start.
        let line_start_byte = source[..val_span.start]
            .rfind('\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let line_before = &source[line_start_byte..val_span.start];
        let key_range =
            locate_key_in_line_text(source, line_no, line_start_byte, line_before, leaf);

        return (key_range, value_range, line_no);
    }

    // ── Strategy 2: text scan ────────────────────────────────────────────────
    // Look for a line matching `leaf = ...` (or `leaf=...`, `"leaf"=...`, etc.)
    // Value string used as a hint to narrow candidates when multiple lines match.
    if let Some((line_no, line_byte_start, line_text)) =
        find_kv_line(lines, leaf, &value_str, path_parts)
    {
        let key_range = locate_key_in_line_text(source, line_no, line_byte_start, line_text, leaf);

        // Value range: find `=` separator then the value text.
        let value_range =
            locate_value_in_line(source, line_no, line_byte_start, line_text, &value_str);

        return (key_range, value_range, line_no);
    }

    // ── Fallback: zero-width ranges at line 0 ───────────────────────────────
    (LspRange::default(), LspRange::default(), 0)
}

/// Scan lines for the first occurrence of `leaf = <value>` (or bare `leaf =`).
///
/// Returns `(line_no, byte_start_of_line, line_text)` when found.
/// The `value_hint` is used to disambiguate if multiple lines have the same key.
/// The `path_parts` supply the full dotted key path for dotted-assignment
/// matching (e.g. `a.b = 1`).
///
/// BUG-4 fix: when multiple lines have the same leaf key, we narrow to the
/// correct `[table]` section by scanning for the containing table header
/// (`[table_path]`) in the lines before each candidate, then picking the
/// candidate whose enclosing table header matches the entry's parent path.
///
/// BUG-7 fix: for dotted top-level assignments like `workspace.resolver = "2"`
/// the trimmed line starts with the full dotted path, not just the leaf; we now
/// also try `is_kv_assignment_for(trimmed, &full_dotted_prefix)`.
fn find_kv_line<'s>(
    lines: &[(u32, usize, &'s str)],
    leaf: &str,
    value_hint: &str,
    path_parts: &[&str],
) -> Option<(u32, usize, &'s str)> {
    // Build the full dotted prefix for BUG-7 dotted-key assignment detection
    // (e.g. path_parts = ["workspace", "resolver"] → "workspace.resolver").
    let full_dotted: String = path_parts.join(".");

    // Table path = all segments except the leaf (empty → top-level).
    let table_path: &[&str] = if path_parts.len() > 1 {
        &path_parts[..path_parts.len() - 1]
    } else {
        &[]
    };

    // Collect all candidate lines that contain `leaf` followed by `=`.
    let candidates: Vec<_> = lines
        .iter()
        .filter(|(_, _, line_text)| {
            let trimmed = line_text.trim_start();
            // Skip comment and section header lines.
            if trimmed.starts_with('#') || trimmed.starts_with('[') {
                return false;
            }
            // BUG-7: try full dotted prefix first (e.g. `a.b = 1`), then bare leaf.
            is_kv_assignment_for(trimmed, &full_dotted) || is_kv_assignment_for(trimmed, leaf)
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // If exactly one candidate, return it.
    if candidates.len() == 1 {
        let (ln, bs, lt) = candidates[0];
        return Some((*ln, *bs, lt));
    }

    // BUG-4: multiple candidates sharing the same leaf key — disambiguate by
    // finding the candidate whose enclosing `[table]` header matches table_path.
    if !table_path.is_empty() {
        // Build possible section header strings for this table path:
        // `[a.b]`, `[a.b.c]`, etc.  Also handle `[[a.b]]` (AoT).
        let header_dotted = table_path.join(".");
        let bracket_header = format!("[{}]", header_dotted);
        let aot_header = format!("[[{}]]", header_dotted);

        // For each candidate, scan backwards to find the nearest `[...]` header.
        // Pick the first candidate whose nearest header matches our table path.
        for (candidate_ln, candidate_bs, candidate_lt) in &candidates {
            let enclosing = find_enclosing_section_header(lines, *candidate_ln);
            if let Some(hdr) = enclosing {
                if hdr == bracket_header || hdr == aot_header {
                    return Some((*candidate_ln, *candidate_bs, candidate_lt));
                }
            } else if table_path.is_empty() {
                // Candidate with no enclosing header → top-level; matches.
                return Some((*candidate_ln, *candidate_bs, candidate_lt));
            }
        }
    }

    // Multiple candidates: try to narrow by value hint (exact textual match).
    for (ln, bs, lt) in &candidates {
        if lt.contains(value_hint) {
            return Some((*ln, *bs, lt));
        }
    }

    // Fall back to first candidate.
    let (ln, bs, lt) = candidates[0];
    Some((*ln, *bs, lt))
}

/// Scan backwards from `from_line` (exclusive) to find the nearest section
/// header line (`[...]` or `[[...]]`). Returns the trimmed header string.
fn find_enclosing_section_header(lines: &[(u32, usize, &str)], from_line: u32) -> Option<String> {
    for (line_no, _, line_text) in lines.iter().rev() {
        if *line_no >= from_line {
            continue;
        }
        let trimmed = line_text.trim();
        if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
            return Some(trimmed.to_string());
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Check if a trimmed line is a `key = value` assignment for `leaf`.
///
/// Handles:
/// - Bare key: `name = "foo"`
/// - Quoted key: `"name" = "foo"`, `'name' = "foo"`
/// - Whitespace around `=`
fn is_kv_assignment_for(trimmed_line: &str, leaf: &str) -> bool {
    // Fast path: bare key.
    let rest = if let Some(r) = trimmed_line.strip_prefix(leaf) {
        r
    } else if let Some(r) = trimmed_line
        .strip_prefix('"')
        .and_then(|s| s.strip_prefix(leaf))
        .and_then(|s| s.strip_prefix('"'))
    {
        r
    } else if let Some(r) = trimmed_line
        .strip_prefix('\'')
        .and_then(|s| s.strip_prefix(leaf))
        .and_then(|s| s.strip_prefix('\''))
    {
        r
    } else {
        return false;
    };
    // After the key, must see optional whitespace then `=`.
    rest.trim_start().starts_with('=')
}

/// Build a key LSP range by finding `leaf` in `line_text` (the portion of the
/// source line before the `=` separator).
///
/// BUG-7 fix: for dotted-key assignment lines like `a.b = 1` the leaf `b` is
/// not a standalone bare key in the line — the whole segment `a.b` precedes
/// `=`. In that case we locate the last `.`-separated segment (the leaf) within
/// the `before_eq` portion of the line.
fn locate_key_in_line_text(
    source: &str,
    line_no: u32,
    _line_byte_start: usize,
    line_text: &str,
    leaf: &str,
) -> LspRange {
    // Try bare leaf first.
    let key_char_start = find_bare_key_char_pos(line_text, leaf)
        .or_else(|| find_quoted_key_char_pos(line_text, leaf))
        .or_else(|| {
            // BUG-7: dotted assignment — find the leaf within the `before =` portion.
            // e.g. line = `a.b = 1`, leaf = `b`; look in `a.b`.
            let before_eq = line_text.split('=').next().unwrap_or("");
            // The leaf must appear as the last `.`-separated segment.
            let last_seg = before_eq.trim_end().rsplit('.').next().unwrap_or("").trim();
            if last_seg == leaf {
                // char position of last_seg in line_text.
                let seg_byte = line_text
                    .rfind(leaf)
                    .filter(|&pos| {
                        // Verify this occurrence is before the `=`.
                        line_text[pos..].starts_with(leaf)
                            && line_text[pos + leaf.len()..].trim_start().starts_with('=')
                    })
                    .or_else(|| {
                        // Any position of leaf before `=` in a dotted context.
                        let eq_byte = line_text.find('=').unwrap_or(line_text.len());
                        line_text[..eq_byte].rfind(leaf)
                    });
                seg_byte.map(|b| line_text[..b].chars().count())
            } else {
                None
            }
        });

    if let Some(char_start) = key_char_start {
        let utf16_start = char_col_to_utf16_col(line_text, char_start);
        // Compute UTF-16 end: start + key UTF-16 length.
        let key_utf16_len: u32 = leaf.chars().map(|c| c.len_utf16() as u32).sum();
        let utf16_end = utf16_start + key_utf16_len;
        LspRange {
            start: Position {
                line: line_no,
                character: utf16_start,
            },
            end: Position {
                line: line_no,
                character: utf16_end,
            },
        }
    } else {
        // Fallback: zero-width at line start.
        let _ = source;
        LspRange {
            start: Position {
                line: line_no,
                character: 0,
            },
            end: Position {
                line: line_no,
                character: 0,
            },
        }
    }
}

/// Build a value LSP range by locating `value_str` after `=` in `line_text`.
fn locate_value_in_line(
    _source: &str,
    line_no: u32,
    _line_byte_start: usize,
    line_text: &str,
    value_str: &str,
) -> LspRange {
    // Find the `=` separator position in character units.
    let eq_char_pos = line_text.chars().take_while(|&c| c != '=').count();
    if eq_char_pos >= line_text.chars().count() {
        return LspRange {
            start: Position {
                line: line_no,
                character: 0,
            },
            end: Position {
                line: line_no,
                character: 0,
            },
        };
    }
    let after_eq: &str = &line_text[line_text
        .char_indices()
        .nth(eq_char_pos + 1)
        .map(|(b, _)| b)
        .unwrap_or(line_text.len())..];
    let trimmed_after = after_eq.trim_start();
    // Value is either quoted ("...") or bare.
    let value_char_start_in_after = after_eq.len() - trimmed_after.len();
    let value_start_char_col = eq_char_pos + 1 + value_char_start_in_after;
    // Try to find the value string (bare or in quotes).
    // The value_str is unquoted; the raw form may have quotes around it.
    let raw_value_start_char = if trimmed_after.starts_with('"') || trimmed_after.starts_with('\'')
    {
        value_start_char_col + 1 // skip opening quote
    } else {
        value_start_char_col
    };
    let raw_value_end_char = raw_value_start_char + value_str.chars().count();
    let utf16_start = char_col_to_utf16_col(line_text, raw_value_start_char);
    let utf16_end = char_col_to_utf16_col(line_text, raw_value_end_char);
    LspRange {
        start: Position {
            line: line_no,
            character: utf16_start,
        },
        end: Position {
            line: line_no,
            character: utf16_end,
        },
    }
}

/// Find the char-column of `leaf` as a bare key in `line`.
///
/// Returns `None` if not found or if the match is ambiguous (inside a larger
/// identifier).
fn find_bare_key_char_pos(line: &str, leaf: &str) -> Option<usize> {
    let mut char_offset = 0usize;
    let line_chars: Vec<char> = line.chars().collect();
    let leaf_chars: Vec<char> = leaf.chars().collect();
    let n = leaf_chars.len();
    if n == 0 || line_chars.len() < n {
        return None;
    }
    while char_offset + n <= line_chars.len() {
        if line_chars[char_offset..char_offset + n] == leaf_chars[..] {
            // Check boundaries.
            let before_ok = char_offset == 0 || {
                let c = line_chars[char_offset - 1];
                !c.is_alphanumeric() && c != '_' && c != '-'
            };
            let after_pos = char_offset + n;
            let after_ok = after_pos >= line_chars.len() || {
                let c = line_chars[after_pos];
                !c.is_alphanumeric() && c != '_' && c != '-'
            };
            if before_ok && after_ok {
                return Some(char_offset);
            }
        }
        char_offset += 1;
    }
    None
}

/// Find the char-column of `leaf` as a quoted key (`"leaf"` or `'leaf'`).
fn find_quoted_key_char_pos(line: &str, leaf: &str) -> Option<usize> {
    let dq = format!("\"{}\"", leaf);
    let sq = format!("'{}'", leaf);
    let find_char_col = |pat: &str| -> Option<usize> {
        line.find(pat)
            .map(|byte_pos| line[..byte_pos].chars().count() + 1) // +1 to skip quote
    };
    find_char_col(&dq).or_else(|| find_char_col(&sq))
}

/// Extract the display string for a TOML value.
///
/// - Strings: unquoted (without surrounding `"`/`'`).
/// - Integers/floats/booleans: rendered as their canonical text.
/// - Arrays / inline tables: rendered as TOML repr for hover display.
pub fn toml_value_to_string(value: &Value) -> String {
    match value {
        Value::String(fs) => fs.value().to_string(),
        Value::Integer(fi) => fi.value().to_string(),
        Value::Float(ff) => ff.value().to_string(),
        Value::Boolean(fb) => fb.value().to_string(),
        Value::Datetime(fd) => fd.value().to_string(),
        Value::Array(_) | Value::InlineTable(_) => value.to_string(),
    }
}

/// Apply TOML `toml_edit` diagnostics: duplicate-key detection.
///
/// `toml_edit` already rejects duplicate keys at parse time (it returns an
/// error). This function is a secondary pass for the entry model: it flags
/// any keys that appear more than once in the flattened `entries` list (which
/// can happen if the caller bypasses the parser or for cross-section detection).
///
/// Entries at the given indices in `aot_mask` are treated as originating from
/// `[[array-of-tables]]` blocks and are excluded from the duplicate check —
/// repeated `[[bin]]` sections are legal TOML (BUG-8 fix). When `aot_mask` is
/// empty (e.g. called from tests with manually-constructed entries) all entries
/// are checked.
pub fn toml_duplicate_key_diagnostics(entries: &[ConfigEntry]) -> Vec<TomlDiagnostic> {
    toml_duplicate_key_diagnostics_inner(entries, &[])
}

/// AoT-aware variant used by `config_toml_diagnostics`.
pub fn toml_duplicate_key_diagnostics_aot(
    entries: &[ConfigEntry],
    aot_flags: &AotFlags,
) -> Vec<TomlDiagnostic> {
    toml_duplicate_key_diagnostics_inner(entries, aot_flags)
}

fn toml_duplicate_key_diagnostics_inner(
    entries: &[ConfigEntry],
    aot_flags: &[bool],
) -> Vec<TomlDiagnostic> {
    use std::collections::HashMap;
    let mut seen: HashMap<&str, u32> = HashMap::new();
    let mut diags = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if entry.key.is_empty() {
            continue;
        }
        // BUG-8: skip entries originating from [[array-of-tables]] blocks.
        if aot_flags.get(i).copied().unwrap_or(false) {
            continue;
        }
        if let Some(&first_line) = seen.get(entry.key.as_str()) {
            diags.push(TomlDiagnostic {
                range: entry.key_range,
                severity: tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
                message: format!(
                    "Duplicate key '{}' (first defined on line {})",
                    entry.key,
                    first_line + 1,
                ),
            });
        } else {
            seen.insert(entry.key.as_str(), entry.line);
        }
    }
    diags
}

// Tests for this module live in tests/toml_support_tests.rs
// per the CLAUDE.md convention: "All tests live in tests/ (no in-module tests)".
