//! Language-feature implementations for `ConfigFormat`-dispatched files
//! — Stories 004–009.
//!
//! All functions accept `&[ConfigEntry]` (the format-agnostic entry model)
//! so they work identically for `.properties` and `.env`-cascade files.
//! Nothing in this module touches file names or URIs except where
//! necessary for cross-file navigation.

use std::collections::{HashMap, HashSet};

// ── UTF-16 position helper ────────────────────────────────────────────────────

/// Convert a UTF-16 code-unit offset (`character` from an LSP `Position`) to a
/// UTF-8 byte offset within `line`.
///
/// LSP positions use UTF-16 code units (per the spec). Indexing a `&str` with a
/// raw `character as usize` value is only correct for ASCII-only content; on
/// any multi-byte code point the index may fall inside a char boundary and
/// cause a panic or silently produce garbage ranges.
///
/// Returns the byte offset corresponding to the given UTF-16 unit, clamped to
/// `line.len()` so the result is always a valid `&line[..byte]` index.
fn utf16_col_to_byte_offset(line: &str, utf16_col: u32) -> usize {
    let mut units_seen: u32 = 0;
    for (byte_idx, ch) in line.char_indices() {
        if units_seen >= utf16_col {
            return byte_idx;
        }
        units_seen += ch.len_utf16() as u32;
    }
    line.len()
}

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Diagnostic, DiagnosticSeverity,
    Documentation, Hover, HoverContents, Location, MarkupContent, MarkupKind, Position,
    Range as LspRange, SemanticToken, SemanticTokens, TextEdit, Url, WorkspaceEdit,
};

use crate::ops::config_format::{ConfigEntry, ResolvedValue, SourceLayer, WriteCapability};
use crate::ops::config_resolution::{
    contains_interpolation, find_unterminated_refs, resolve_effective_value,
};
use crate::ops::schema::EnvSchema;

// ── YAML diagnostic shape (unit 002 / story 004) ─────────────────────────────

/// Lightweight diagnostic returned by the YAML parser and converter.
/// Used as an intermediate before converting to `tower_lsp::lsp_types::Diagnostic`.
#[derive(Debug, Clone)]
pub struct YamlDiagnostic {
    pub range: LspRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

/// Convert `YamlDiagnostic` slices into `tower_lsp::lsp_types::Diagnostic` values.
pub fn yaml_diagnostics_to_lsp(diags: &[YamlDiagnostic]) -> Vec<Diagnostic> {
    diags
        .iter()
        .map(|d| Diagnostic {
            range: d.range,
            severity: Some(d.severity),
            source: Some("envforge".into()),
            message: d.message.clone(),
            ..Default::default()
        })
        .collect()
}

// ── Story 004: Hover with effective value, layer, schema, redaction ──────────

/// Build hover content for the entry under `position`.
///
/// Shows:
/// - Key name (bold)
/// - Resolved effective value + winning layer (sensitive values redacted)
/// - Schema metadata when available
pub fn config_hover(
    position: Position,
    entries: &[ConfigEntry],
    layers: &[Vec<ConfigEntry>],
    schema: Option<&EnvSchema>,
) -> Option<Hover> {
    // Find the entry at the cursor position.
    let entry = entries.iter().find(|e| {
        !e.key.is_empty()
            && e.line == position.line
            && position.character >= e.key_range.start.character
            && position.character <= e.value_range.end.character
    })?;

    let schema_var = schema.and_then(|s| s.variables.get(&entry.key));
    let is_sensitive = schema_var.map(|v| v.sensitive).unwrap_or(false)
        || crate::ops::dotenv::is_sensitive_key(&entry.key);

    // Resolve effective value across layers.
    let resolved: Option<ResolvedValue> = if !layers.is_empty() {
        resolve_effective_value(&entry.key, layers)
    } else {
        Some(ResolvedValue {
            value: entry.value.clone(),
            winning_layer: entry.source_layer.clone(),
            interpolated: contains_interpolation(&entry.value),
        })
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("**{}**", entry.key));

    if let Some(ref res) = resolved {
        // For sensitive values: show a "(redacted)" annotation matching the
        // style used by hover.rs for .env files (FR6 / M1).
        // For non-sensitive values: show the actual value so the hover is
        // informative. `redact_for_label` always returns "***" (by design to
        // close prefix-leakage vectors), so we must NOT call it for non-sensitive
        // values or the popup is useless.
        let display_value = if is_sensitive {
            "_(redacted)_".to_string()
        } else {
            format!("`{}`", res.value)
        };
        lines.push(format!("Value: {}", display_value));
        lines.push(format!("Layer: `{}`", res.winning_layer.display()));
        if res.interpolated {
            lines.push("_(interpolated)_".to_string());
        }
    }

    if let Some(var_def) = schema_var {
        lines.push(String::new());
        lines.push("---".into());
        lines.push("**Schema**".into());
        lines.push(format!("Type: `{}`", var_def.var_type.display()));
        if var_def.required {
            lines.push("Required: **yes**".into());
        }
        if var_def.sensitive {
            lines.push("Sensitive: **yes**".into());
        }
        if let Some(ref desc) = var_def.description {
            lines.push(String::new());
            lines.push(desc.clone());
        }
        if let Some(ref def) = var_def.default {
            lines.push(format!("Default: `{}`", def));
        }
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n"),
        }),
        range: Some(entry.key_range),
    })
}

// ── Story 005: Completion for keys and `${VAR}` references ──────────────────

/// Produce completion items for a config file.
///
/// - At a key position: suggest keys from schema + sibling entries (de-duped).
/// - After `${`: suggest `${VAR}` references from schema.
pub fn config_completions(
    position: Position,
    content: &str,
    entries: &[ConfigEntry],
    schema: Option<&EnvSchema>,
) -> Vec<CompletionItem> {
    let line = content.lines().nth(position.line as usize).unwrap_or("");
    let col = utf16_col_to_byte_offset(line, position.character);
    let before_cursor = &line[..col];

    // Inside a `${` reference.
    if before_cursor.ends_with('$') || before_cursor.contains("${") {
        return config_ref_completions(entries, schema, position, line);
    }

    // After separator `=` or `:` — value completions.
    if let Some(eq_byte_idx) = before_cursor.find('=').or_else(|| before_cursor.find(':')) {
        // Convert byte indices to UTF-16 units (NFR14).
        let sep_start_utf16: u32 = line[..=eq_byte_idx]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();
        let line_end_utf16: u32 = line.chars().map(|c| c.len_utf16() as u32).sum();
        let value_range = LspRange {
            start: Position {
                line: position.line,
                character: sep_start_utf16,
            },
            end: Position {
                line: position.line,
                character: line_end_utf16,
            },
        };
        let eq_idx = eq_byte_idx;
        let key = before_cursor[..eq_idx].trim();
        return config_value_completions(key, schema, value_range);
    }

    // Key position.
    config_key_completions(before_cursor, entries, schema, position, line)
}

fn config_key_completions(
    prefix: &str,
    entries: &[ConfigEntry],
    schema: Option<&EnvSchema>,
    position: Position,
    line: &str,
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.trim().to_lowercase();
    let existing_keys: HashSet<&str> = entries.iter().map(|e| e.key.as_str()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut items = Vec::new();

    // Line end in UTF-16 units (NFR14).
    let line_end_utf16: u32 = line.chars().map(|c| c.len_utf16() as u32).sum();
    let replace_range = LspRange {
        start: Position {
            line: position.line,
            character: 0,
        },
        end: Position {
            line: position.line,
            character: line_end_utf16,
        },
    };

    // 1. Schema keys (highest priority).
    if let Some(schema) = schema {
        for (name, var_def) in &schema.variables {
            if !seen.insert(name.clone()) {
                continue;
            }
            if !prefix_lower.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            let detail = format!(
                "{} {}",
                var_def.var_type.display(),
                if var_def.required { "(required)" } else { "" }
            );
            let insert = format!("{}=", name);
            let doc = var_def.description.as_ref().map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.clone(),
                })
            });
            items.push(CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(detail.trim().to_string()),
                documentation: doc,
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: insert,
                })),
                sort_text: Some(format!("0_{}", name)),
                ..Default::default()
            });
        }
    }

    // 2. Keys from sibling/same-file entries (not already in schema / file).
    for entry in entries {
        if entry.key.is_empty() || existing_keys.contains(entry.key.as_str()) {
            continue;
        }
        if !seen.insert(entry.key.clone()) {
            continue;
        }
        if !prefix_lower.is_empty() && !entry.key.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }
        items.push(CompletionItem {
            label: entry.key.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(format!("from {}", entry.source_layer.display())),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: replace_range,
                new_text: format!("{}=", entry.key),
            })),
            sort_text: Some(format!("1_{}", entry.key)),
            ..Default::default()
        });
    }

    items
}

fn config_value_completions(
    key: &str,
    schema: Option<&EnvSchema>,
    value_range: LspRange,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let Some(schema) = schema else { return items };
    let Some(var_def) = schema.variables.get(key) else {
        return items;
    };

    use crate::ops::schema::VarType;
    let value_edit = |new_text: String| {
        Some(CompletionTextEdit::Edit(TextEdit {
            range: value_range,
            new_text,
        }))
    };

    match var_def.var_type {
        VarType::Bool => {
            for val in &["true", "false"] {
                items.push(CompletionItem {
                    label: (*val).to_string(),
                    kind: Some(CompletionItemKind::VALUE),
                    text_edit: value_edit((*val).to_string()),
                    ..Default::default()
                });
            }
        }
        VarType::Enum => {
            if let Some(ref vals) = var_def.values {
                for val in vals {
                    items.push(CompletionItem {
                        label: val.clone(),
                        kind: Some(CompletionItemKind::ENUM_MEMBER),
                        text_edit: value_edit(val.clone()),
                        ..Default::default()
                    });
                }
            }
        }
        _ => {
            if !var_def.sensitive {
                if let Some(ref def) = var_def.default {
                    items.push(CompletionItem {
                        label: def.clone(),
                        kind: Some(CompletionItemKind::VALUE),
                        detail: Some("default".into()),
                        text_edit: value_edit(def.clone()),
                        ..Default::default()
                    });
                }
            }
        }
    }
    items
}

fn config_ref_completions(
    entries: &[ConfigEntry],
    schema: Option<&EnvSchema>,
    position: Position,
    line: &str,
) -> Vec<CompletionItem> {
    let col = utf16_col_to_byte_offset(line, position.character);
    let before = &line[..col];
    // `rfind` gives a byte offset; convert to UTF-16 units for LSP (F8/NFR14).
    let start_byte = before.rfind('$').unwrap_or(col);
    let start_utf16: u32 = line[..start_byte]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    // Line end in UTF-16 units.
    let line_end_utf16: u32 = line.chars().map(|c| c.len_utf16() as u32).sum();
    let replace_range = LspRange {
        start: Position {
            line: position.line,
            character: start_utf16,
        },
        end: Position {
            line: position.line,
            character: line_end_utf16,
        },
    };

    // Extract partial typed identifier after `${`.
    let typed_prefix = {
        let from = before
            .rfind("${")
            .map(|i| i + 2)
            .or_else(|| before.rfind('$').map(|i| i + 1))
            .unwrap_or(before.len());
        before[from..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase()
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut items = Vec::new();
    let edit = |new_text: String| {
        Some(CompletionTextEdit::Edit(TextEdit {
            range: replace_range,
            new_text,
        }))
    };

    let matches_prefix = |key: &str| -> bool {
        typed_prefix.is_empty() || key.to_lowercase().starts_with(&typed_prefix)
    };

    // Keys from schema.
    if let Some(schema) = schema {
        for name in schema.variables.keys() {
            if !matches_prefix(name) {
                continue;
            }
            if seen.insert(name.clone()) {
                let new_text = format!("${{{}}}", name);
                items.push(CompletionItem {
                    label: name.clone(),
                    filter_text: Some(format!("${{{}}}", name)),
                    kind: Some(CompletionItemKind::REFERENCE),
                    detail: Some("schema".into()),
                    text_edit: edit(new_text),
                    sort_text: Some(format!("0_{}", name)),
                    ..Default::default()
                });
            }
        }
    }

    // Keys from same-file entries.
    for e in entries {
        if e.key.is_empty() {
            continue;
        }
        if !matches_prefix(&e.key) {
            continue;
        }
        if seen.insert(e.key.clone()) {
            let new_text = format!("${{{}}}", e.key);
            items.push(CompletionItem {
                label: e.key.clone(),
                filter_text: Some(format!("${{{}}}", e.key)),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("this file".into()),
                text_edit: edit(new_text),
                sort_text: Some(format!("1_{}", e.key)),
                ..Default::default()
            });
        }
    }

    items
}

// ── Story 006: Go-to-definition and find-references ──────────────────────────

/// Resolve go-to-definition for the entry under `position`.
///
/// Priority:
/// 1. Schema file (via `schema_line_map`).
/// 2. Base-layer entry across `open_docs`.
/// 3. The entry in the current file if it IS the defining occurrence.
pub fn config_goto_definition(
    position: Position,
    entries: &[ConfigEntry],
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
) -> Option<tower_lsp::lsp_types::GotoDefinitionResponse> {
    // Determine key under cursor.
    let key = entries
        .iter()
        .find(|e| {
            !e.key.is_empty()
                && e.line == position.line
                && position.character >= e.key_range.start.character
                && position.character <= e.key_range.end.character
        })
        .map(|e| e.key.as_str())?;

    // 1. Schema definition.
    if let (Some(uri), Some(&line)) = (schema_uri, schema_line_map.get(key)) {
        // Use UTF-16 length (not byte length) for non-ASCII key names (NFR14).
        let key_utf16_len: u32 = key.chars().map(|c| c.len_utf16() as u32).sum();
        return Some(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(
            Location {
                uri: uri.clone(),
                range: LspRange {
                    start: Position { line, character: 0 },
                    end: Position {
                        line,
                        character: key_utf16_len + 2,
                    },
                },
            },
        ));
    }

    // 2. Base-layer occurrence in open docs — sorted by URI for determinism.
    let mut sorted_docs: Vec<(&Url, &Vec<ConfigEntry>)> = open_docs.iter().collect();
    sorted_docs.sort_by_key(|(u, _)| u.as_str());
    for (uri, doc_entries) in &sorted_docs {
        for e in *doc_entries {
            if e.key == key && e.source_layer == SourceLayer::Base {
                return Some(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(
                    Location {
                        uri: (*uri).clone(),
                        range: e.key_range,
                    },
                ));
            }
        }
    }

    // 3. First occurrence in this file.
    if let Some(e) = entries.iter().find(|e| e.key == key) {
        return Some(tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(
            Location {
                uri: Url::parse("file:///unknown").ok()?,
                range: e.key_range,
            },
        ));
    }

    None
}

/// Find every reference to `key` across `open_docs` (and optionally the
/// schema file). `include_declaration = false` omits the schema entry.
pub fn config_find_references(
    key: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();

    if include_declaration {
        if let (Some(uri), Some(&line)) = (schema_uri, schema_line_map.get(key)) {
            // Use UTF-16 length (not byte length) for non-ASCII key names (NFR14).
            let key_utf16_len: u32 = key.chars().map(|c| c.len_utf16() as u32).sum();
            locations.push(Location {
                uri: uri.clone(),
                range: LspRange {
                    start: Position { line, character: 0 },
                    end: Position {
                        line,
                        character: key_utf16_len + 2,
                    },
                },
            });
        }
    }

    // Iterate open docs sorted by URI for determinism (should-fix).
    let mut sorted_docs: Vec<(&Url, &Vec<ConfigEntry>)> = open_docs.iter().collect();
    sorted_docs.sort_by_key(|(u, _)| u.as_str());
    for (uri, doc_entries) in &sorted_docs {
        for e in *doc_entries {
            if e.key == key {
                locations.push(Location {
                    uri: (*uri).clone(),
                    range: e.key_range,
                });
            }
        }
    }

    // Sort by (uri, line) for determinism (should-fix).
    locations.sort_by(|a, b| {
        a.uri
            .as_str()
            .cmp(b.uri.as_str())
            .then_with(|| a.range.start.line.cmp(&b.range.start.line))
    });

    locations
}

// ── Story 007: Semantic tokens ────────────────────────────────────────────────

/// Token-type indices (same legend as the existing `semantic_tokens.rs`).
const TYPE_VARIABLE: u32 = 0;
const TYPE_STRING: u32 = 1;
const TYPE_COMMENT: u32 = 2;
const MOD_SENSITIVE: u32 = 1 << 0;

#[derive(Debug, Clone)]
struct RawTok {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

/// Compute semantic tokens for config-format entries.
/// Reuses the same token-type/modifier legend as `compute_semantic_tokens`
/// so no new vocabulary is introduced.
pub fn config_semantic_tokens(
    entries: &[ConfigEntry],
    schema: Option<&EnvSchema>,
) -> SemanticTokens {
    let mut raws: Vec<RawTok> = Vec::new();

    for entry in entries {
        if entry.key.is_empty() {
            // Blank or comment — emit comment token if value starts with `#` / `!`.
            if entry.value.starts_with('#') || entry.value.starts_with('!') {
                let len = entry.key_range.end.character - entry.key_range.start.character;
                if len > 0 {
                    raws.push(RawTok {
                        line: entry.line,
                        start: entry.key_range.start.character,
                        length: len,
                        token_type: TYPE_COMMENT,
                        modifiers: 0,
                    });
                }
            }
            continue;
        }

        let sensitive = schema
            .and_then(|s| s.variables.get(&entry.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || crate::ops::dotenv::is_sensitive_key(&entry.key);
        let mods = if sensitive { MOD_SENSITIVE } else { 0 };

        let key_len = entry.key_range.end.character - entry.key_range.start.character;
        if key_len > 0 {
            raws.push(RawTok {
                line: entry.line,
                start: entry.key_range.start.character,
                length: key_len,
                token_type: TYPE_VARIABLE,
                modifiers: mods,
            });
        }

        if !sensitive {
            let val_len = entry.value_range.end.character - entry.value_range.start.character;
            if val_len > 0 {
                raws.push(RawTok {
                    line: entry.line,
                    start: entry.value_range.start.character,
                    length: val_len,
                    token_type: TYPE_STRING,
                    modifiers: 0,
                });
            }
        }
    }

    raws.sort_by_key(|a| (a.line, a.start));

    let mut data = Vec::with_capacity(raws.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    for raw in raws {
        let delta_line = raw.line - prev_line;
        let delta_start = if delta_line == 0 {
            raw.start - prev_start
        } else {
            raw.start
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: raw.length,
            token_type: raw.token_type,
            token_modifiers_bitset: raw.modifiers,
        });
        prev_line = raw.line;
        prev_start = raw.start;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

// ── Story 008: Diagnostics ────────────────────────────────────────────────────

/// Compute diagnostics for a set of config entries:
/// - Duplicate key within the same file.
/// - Unterminated `${` interpolation.
/// - Unknown key vs schema (when schema is present).
///
/// Never panics; all errors are recoverable.
pub fn config_diagnostics(entries: &[ConfigEntry], schema: Option<&EnvSchema>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Track keys seen so far for duplicate detection.
    let mut seen_keys: HashMap<&str, u32> = HashMap::new();

    for entry in entries {
        if entry.key.is_empty() {
            // Blank / comment — check for unterminated `${` in comment text.
            if !entry.value.is_empty() && contains_interpolation(&entry.value) {
                let positions = find_unterminated_refs(&entry.value);
                for _pos in positions {
                    diags.push(Diagnostic {
                        range: entry.value_range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        source: Some("envforge".into()),
                        message: "Unterminated interpolation `${`".to_string(),
                        ..Default::default()
                    });
                }
            }
            continue;
        }

        // Duplicate key (same file only — cross-file overrides are legal).
        if let Some(&first_line) = seen_keys.get(entry.key.as_str()) {
            diags.push(Diagnostic {
                range: entry.key_range,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("envforge".into()),
                message: format!(
                    "Duplicate key '{}' (first defined on line {})",
                    entry.key,
                    first_line + 1
                ),
                ..Default::default()
            });
        } else {
            seen_keys.insert(entry.key.as_str(), entry.line);
        }

        // Unterminated interpolation in value.
        if contains_interpolation(&entry.value) {
            let positions = find_unterminated_refs(&entry.value);
            for _pos in positions {
                diags.push(Diagnostic {
                    range: entry.value_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("envforge".into()),
                    message: "Unterminated interpolation `${`".to_string(),
                    ..Default::default()
                });
            }
        }

        // Unknown key vs schema.
        if let Some(schema) = schema {
            if !schema.variables.contains_key(&entry.key) {
                diags.push(Diagnostic {
                    range: entry.key_range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("envforge".into()),
                    message: format!("Unknown key '{}' (not in schema)", entry.key),
                    ..Default::default()
                });
            }
        }
    }

    diags
}

// ── Unit 002 / Story 004: YAML diagnostics (read-only) ───────────────────────

/// Compute diagnostics for a YAML config file.
///
/// Combines:
/// 1. YAML parse/syntax errors (surfaced by the parser).
/// 2. Duplicate-key warnings from the flattened entry model.
/// 3. Unterminated `${` interpolation errors in values.
///
/// This function is read-only: it never produces write edits.
/// Malformed YAML returns a parse-error diagnostic; it never panics.
pub fn config_yaml_diagnostics(
    content: &str,
    layer: crate::ops::config_format::SourceLayer,
) -> Vec<Diagnostic> {
    use crate::parser::yaml_config_parser::parse_yaml_to_entries;

    let (_entries, yaml_diags) = parse_yaml_to_entries(content, layer);
    yaml_diagnostics_to_lsp(&yaml_diags)
}

// ── Story 009: Rename and format ──────────────────────────────────────────────

/// Build a `WorkspaceEdit` that renames `old_key` to `new_name` across all
/// provided write-capable documents. Returns `None` when:
/// - `new_name` is invalid.
/// - The rename would collide with an existing key in any doc.
/// - `write_capability` is `ReadOnly`.
pub fn config_rename(
    old_key: &str,
    new_name: &str,
    write_capability: WriteCapability,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
) -> Option<WorkspaceEdit> {
    if write_capability == WriteCapability::ReadOnly {
        return None;
    }
    if !is_valid_config_key(new_name) || new_name == old_key {
        return None;
    }

    // Collision check: new_name must not already exist in any doc.
    for entries in open_docs.values() {
        if entries.iter().any(|e| e.key == new_name) {
            return None;
        }
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    // Schema header rename.
    if let (Some(uri), Some(&line)) = (schema_uri, schema_line_map.get(old_key)) {
        // Use UTF-16 length (not byte length) for non-ASCII key names (NFR14).
        let old_key_utf16_len: u32 = old_key.chars().map(|c| c.len_utf16() as u32).sum();
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range: LspRange {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: old_key_utf16_len + 2,
                },
            },
            new_text: format!("[{}]", new_name),
        });
    }

    // All occurrences across open docs.
    for (uri, entries) in open_docs {
        for e in entries {
            if e.key == old_key {
                changes.entry(uri.clone()).or_default().push(TextEdit {
                    range: e.key_range,
                    new_text: new_name.to_string(),
                });
            }
        }
    }

    if changes.is_empty() {
        return None;
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

/// Format a `.properties` or `.env` file content to a canonical form.
///
/// Conservative rules (same spirit as `format_document` in `format.rs`):
/// - Trim trailing whitespace per line.
/// - Normalise `KEY = value` / `KEY  =  value` → `KEY=value`.
/// - Preserve comments and blank lines.
/// - **Preserve** the original line-ending style (CRLF or LF) — NFR9.
/// - **Preserve** whether the file originally ends with a newline — NFR9.
///
/// Returns the formatted content. The caller compares to the original to
/// decide whether to produce a `TextEdit`.
pub fn config_format_document(content: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static KV_RE: OnceLock<Regex> = OnceLock::new();
    let kv_re = KV_RE.get_or_init(|| {
        // Matches optional leading whitespace, optional `export `, key, separator, value.
        Regex::new(r"^(\s*)((?:export\s+)?)([A-Za-z_][A-Za-z0-9_.]*)\s*[=:]\s*(.*?)$")
            .expect("hardcoded config format regex")
    });

    // Detect original line endings and trailing-newline presence (NFR9).
    let uses_crlf = content.contains("\r\n");
    let had_trailing_newline = content.ends_with('\n');

    let mut formatted: Vec<String> = Vec::new();
    // `.lines()` strips the line endings, so we iterate over logical lines
    // regardless of whether they're CRLF or LF.
    for line in content.lines() {
        // `line` already has the `\r` stripped by `.lines()` for CRLF input.
        let trimmed = line.trim_end_matches('\r').trim_end();
        if let Some(cap) = kv_re.captures(trimmed) {
            let indent = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let export = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let key = cap.get(3).map(|m| m.as_str()).unwrap_or("");
            let value = cap.get(4).map(|m| m.as_str()).unwrap_or("");
            let export_seg = if export.trim().is_empty() {
                ""
            } else {
                "export "
            };
            formatted.push(format!("{}{}{}={}", indent, export_seg, key, value));
        } else {
            formatted.push(trimmed.to_string());
        }
    }

    // Collapse 3+ consecutive blank lines to 2.
    let mut collapsed: Vec<String> = Vec::with_capacity(formatted.len());
    let mut blank_run = 0usize;
    for line in formatted {
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

    // Reconstruct using the original line-ending style.
    let eol = if uses_crlf { "\r\n" } else { "\n" };
    let mut out = collapsed.join(eol);
    // Restore trailing newline only if the original had one.
    if had_trailing_newline {
        out.push_str(eol);
    }
    out
}

/// Build `TextEdit` list for formatting. Returns empty vec if already canonical.
pub fn config_format_text_edits(content: &str, write_capability: WriteCapability) -> Vec<TextEdit> {
    if write_capability == WriteCapability::ReadOnly {
        return Vec::new();
    }
    let formatted = config_format_document(content);
    if formatted == content {
        return Vec::new();
    }
    vec![TextEdit {
        range: full_doc_range(content),
        new_text: formatted,
    }]
}

fn full_doc_range(content: &str) -> LspRange {
    if content.is_empty() {
        return LspRange {
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
        LspRange {
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
        // LSP positions use UTF-16 code units (NFR14). Use ch.len_utf16() not
        // chars().count() so emoji / surrogate-pair lines are counted correctly.
        let last_line_utf16: u32 = last_line.chars().map(|c| c.len_utf16() as u32).sum();
        LspRange {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: (line_count.saturating_sub(1)) as u32,
                character: last_line_utf16,
            },
        }
    }
}

/// Validate that a config key is a valid identifier (letters, digits, dots,
/// underscores, hyphens; must start with a letter or underscore).
pub fn is_valid_config_key(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}
