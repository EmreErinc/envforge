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
use crate::ops::schema_unification::{canonical_key_strict, is_key_sensitive, UnifiedSchema};

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
///
/// FR4 (Intent 040 / AI-safety): when `unified_schema` is provided, the
/// sensitivity check consults the canonical-key mapping so a key marked
/// sensitive in `.env.schema` (UPPER_SNAKE) is redacted for all formats
/// (`:` path, `.` path, camelCase, etc.) that map to the same canonical key.
pub fn config_hover(
    position: Position,
    entries: &[ConfigEntry],
    layers: &[Vec<ConfigEntry>],
    schema: Option<&EnvSchema>,
    unified_schema: Option<&UnifiedSchema>,
) -> Option<Hover> {
    // Find the entry at the cursor position.
    let entry = entries.iter().find(|e| {
        !e.key.is_empty()
            && e.line == position.line
            && position.character >= e.key_range.start.character
            && position.character <= e.value_range.end.character
    })?;

    // Single-format schema lookup (exact key match).
    let schema_var = schema.and_then(|s| s.variables.get(&entry.key));

    // Sensitivity: check exact-match schema flag, heuristics, AND canonical
    // cross-format mapping (FR4 Intent 040). The unified_schema path ensures
    // e.g. `Spring:Datasource:Password` is redacted when `.env.schema` marks
    // `SPRING_DATASOURCE_PASSWORD` sensitive.
    let is_sensitive = schema_var.map(|v| v.sensitive).unwrap_or(false)
        || crate::ops::dotenv::is_sensitive_key(&entry.key)
        || unified_schema
            .map(|u| is_key_sensitive(&entry.key, u))
            .unwrap_or(false);

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
        // H-1 fix: use canonical-key matching so `.properties` keys like
        // `spring.datasource.url` are not flagged when schema has `SPRING_DATASOURCE_URL`.
        if let Some(schema) = schema {
            let in_schema = schema.variables.contains_key(&entry.key)
                || canonical_key_strict(&entry.key).is_some_and(|ck| {
                    schema
                        .variables
                        .keys()
                        .any(|k| canonical_key_strict(k).as_deref() == Some(ck.as_str()))
                });
            if !in_schema {
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

// ── Intent 038 / Unit 002: YAML surgical rename ───────────────────────────────

/// Build a `WorkspaceEdit` that renames `old_key` to `new_name` across YAML
/// documents via surgical byte-range splice.
///
/// Uses [`yaml_span_resolver::resolve_yaml_key_span`] to locate the exact byte
/// range of the leaf key token and [`SurgicalEdit`] to splice only that range.
/// Every other byte in every document is byte-identical *by construction*.
///
/// Returns `None` when:
/// - `new_name` is invalid (see [`is_valid_config_key`]).
/// - `new_name == old_key`.
/// - A collision exists (new key already present in any doc).
/// - The key is reached via an anchor/alias (documented gap — never mis-edit).
/// - Malformed YAML (graceful degradation — no panic, no edit).
/// - `write_capability` is `ReadOnly`.
///
/// # Cross-doc behaviour
/// Edits are produced for every open document that contains `old_key`.
/// Changes are sorted by (URI, byte-offset descending) so that multiple edits
/// within the same document do not shift each other's positions.
pub fn config_yaml_rename(
    old_key: &str,
    new_name: &str,
    write_capability: WriteCapability,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
    doc_contents: &HashMap<Url, String>,
) -> Option<WorkspaceEdit> {
    use crate::ops::surgical_edit::SurgicalEdit;
    use crate::parser::yaml_span_resolver::resolve_yaml_key_span;

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

    // Sort URIs for determinism (consistent edit ordering).
    let mut uris: Vec<&Url> = open_docs.keys().collect();
    uris.sort_by_key(|u| u.as_str());

    for uri in uris {
        let entries = &open_docs[uri];
        let has_match = entries.iter().any(|e| e.key == old_key);
        if !has_match {
            continue;
        }

        let content = match doc_contents.get(uri) {
            Some(c) => c,
            None => {
                // No content available: fall back to key_range-based edit.
                for e in entries {
                    if e.key == old_key {
                        let leaf_new = new_name.rsplit('.').next().unwrap_or(new_name);
                        changes.entry(uri.clone()).or_default().push(TextEdit {
                            range: e.key_range,
                            new_text: leaf_new.to_string(),
                        });
                    }
                }
                continue;
            }
        };

        // Resolve the leaf key span via yamlpath (tree-sitter).
        match resolve_yaml_key_span(content, old_key) {
            Ok(span) => {
                let leaf_new = new_name.rsplit('.').next().unwrap_or(new_name);

                // For quoted keys, wrap the new name in matching quotes.
                let replacement = if span.is_quoted {
                    let quote = content.as_bytes()[span.byte_range.start] as char;
                    format!("{}{}{}", quote, leaf_new, quote)
                } else {
                    leaf_new.to_string()
                };

                let se =
                    SurgicalEdit::new(span.byte_range.clone(), replacement.clone(), content.len())?;
                let text_edit = se.to_text_edit(content)?;
                changes.entry(uri.clone()).or_default().push(text_edit);
            }
            Err(_) => {
                // Graceful degradation: if the resolver refused (anchor/alias,
                // malformed, not-found), skip this document — do NOT produce a
                // potentially incorrect edit.
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

/// Build `TextEdit` list for YAML format. **Always returns an empty vec.**
///
/// YAML formatting is intentionally a no-op (rename-only per Open decision 1
/// in Intent 038). Returning an empty vec is safe and correct — the LSP client
/// sees "no changes needed."
///
/// This function exists so callers can dispatch YAML through the normal
/// `config_format_text_edits` path without having to special-case the format.
pub fn config_yaml_format_text_edits(_content: &str) -> Vec<TextEdit> {
    // YAML format is deliberately a no-op. See Intent 038 docs/
    // deferred-config-support-plan.md "Open decision 1: rename-only".
    Vec::new()
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

// ── Intent 037 / Story 003–004: TOML read features + diagnostics ─────────────

/// Convert `TomlDiagnostic` slices into `tower_lsp::lsp_types::Diagnostic` values.
pub fn toml_diagnostics_to_lsp(
    diags: &[crate::parser::toml_config_parser::TomlDiagnostic],
) -> Vec<Diagnostic> {
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

/// Compute diagnostics for a TOML config file.
///
/// Combines:
/// 1. TOML parse/syntax errors (surfaced by the parser).
/// 2. Duplicate-key warnings from the flattened entry model (AoT-aware — BUG-8).
/// 3. Unknown-key-vs-schema warnings (when schema is present).
/// 4. Type-mismatch-vs-schema warnings (when schema is present and value
///    can be type-checked against schema).
///
/// This function is read-safe and never panics; malformed TOML returns a
/// parse-error diagnostic.
pub fn config_toml_diagnostics(
    content: &str,
    layer: crate::ops::config_format::SourceLayer,
    schema: Option<&crate::ops::schema::EnvSchema>,
) -> Vec<Diagnostic> {
    use crate::parser::toml_config_parser::{
        parse_toml_config_with_aot_flags, toml_duplicate_key_diagnostics_aot, TomlDiagnostic,
    };

    let (entries, aot_flags, mut raw_diags, _doc) =
        match parse_toml_config_with_aot_flags(content, layer) {
            Ok(result) => result,
            Err(e) => {
                // BUG-9 fix: use actual line length for the error range end instead
                // of u32::MAX, which is not a valid LSP character offset.
                let line = e.line.unwrap_or(0);
                let line_text = content.lines().nth(line as usize).unwrap_or("");
                let line_len_utf16: u32 = line_text.chars().map(|c| c.len_utf16() as u32).sum();
                let range = LspRange {
                    start: Position { line, character: 0 },
                    end: Position {
                        line,
                        character: line_len_utf16,
                    },
                };
                return vec![Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("envforge".into()),
                    message: format!("TOML syntax error: {}", e.message),
                    ..Default::default()
                }];
            }
        };

    // Duplicate-key pass — BUG-8: skip AoT-origin entries.
    raw_diags.extend(toml_duplicate_key_diagnostics_aot(&entries, &aot_flags));

    // Schema-based diagnostics (unknown key, type mismatch).
    // H-1 fix: use canonical-key matching so a TOML key `spring.datasource.url`
    // is not flagged "unknown" when the schema has `SPRING_DATASOURCE_URL`.
    if let Some(schema) = schema {
        for entry in &entries {
            if entry.key.is_empty() {
                continue;
            }
            // Canonical-key lookup: exact match first, then strict canonical match.
            let schema_var = schema.variables.get(&entry.key).or_else(|| {
                canonical_key_strict(&entry.key).and_then(|ck| {
                    schema
                        .variables
                        .iter()
                        .find(|(k, _)| canonical_key_strict(k).as_deref() == Some(ck.as_str()))
                        .map(|(_, v)| v)
                })
            });
            if let Some(var_def) = schema_var {
                // Type mismatch check.
                let type_ok = check_toml_type_vs_schema(&entry.value, &var_def.var_type);
                if !type_ok {
                    raw_diags.push(TomlDiagnostic {
                        range: entry.value_range,
                        severity: DiagnosticSeverity::WARNING,
                        message: format!(
                            "Type mismatch: key '{}' expects type '{}' per schema",
                            entry.key,
                            var_def.var_type.display(),
                        ),
                    });
                }
            } else {
                // Unknown key — not present in schema under any canonical form.
                raw_diags.push(TomlDiagnostic {
                    range: entry.key_range,
                    severity: DiagnosticSeverity::WARNING,
                    message: format!("Unknown key '{}' (not in schema)", entry.key),
                });
            }
        }
    }

    toml_diagnostics_to_lsp(&raw_diags)
}

/// Heuristic type-check for a TOML string value against a schema `VarType`.
///
/// Returns `true` when the value is compatible with the expected type.
/// Returns `false` only when there is a clear mismatch (e.g. non-numeric
/// string for an `Int` type). When in doubt, returns `true` (conservative).
fn check_toml_type_vs_schema(value: &str, var_type: &crate::ops::schema::VarType) -> bool {
    use crate::ops::schema::VarType;
    match var_type {
        VarType::Bool => matches!(value, "true" | "false"),
        VarType::Number => value.parse::<f64>().is_ok(),
        VarType::Port => value.parse::<u16>().is_ok(),
        // Enum, Url, Email, Regex, String — always valid (conservative).
        VarType::Enum | VarType::Url | VarType::Email | VarType::Regex | VarType::String => true,
    }
}

// ── Intent 037 / Story 005: TOML lossless format ─────────────────────────────

/// Format a TOML file using `toml_edit` lossless round-trip.
///
/// `toml_edit` preserves comments, ordering, and whitespace; `to_string()`
/// is byte-identical unless the document was mutated. This function applies
/// no mutations, so the result is always byte-identical to the input for
/// valid TOML. For invalid TOML it returns the original content unchanged.
///
/// Returns `Vec<TextEdit>` — empty if content is already canonical (i.e.
/// always empty for valid, unmodified TOML — format is idempotent).
///
/// BUG-1 fix: `toml_edit::DocumentMut::to_string()` emits LF-only output.
/// When the original content used CRLF line endings we restore `\r\n`.
///
/// BUG-2 fix: `to_string()` always appends a trailing `\n`. When the original
/// lacked a trailing newline we strip the appended one, so a file that was
/// already canonical produces zero edits.
pub fn config_toml_format_text_edits(content: &str) -> Vec<TextEdit> {
    use crate::ops::config_format::SourceLayer;
    use crate::parser::toml_config_parser::parse_toml_config;

    let doc = match parse_toml_config(content, SourceLayer::Unknown) {
        Ok((_entries, _diags, doc)) => doc,
        Err(_) => return Vec::new(), // malformed — leave unchanged
    };

    let formatted = restore_eol_style(content, &doc.to_string());
    if formatted == content {
        return Vec::new();
    }
    vec![TextEdit {
        range: full_doc_range(content),
        new_text: formatted,
    }]
}

/// Restore the original EOL style (CRLF vs LF) and trailing-newline presence
/// onto a string produced by `toml_edit::DocumentMut::to_string()`.
///
/// `toml_edit` always emits LF-only output and always appends a trailing `\n`.
/// This helper reverses those normalizations so the result differs from the
/// original only in intentional content changes.
fn restore_eol_style(original: &str, toml_edit_output: &str) -> String {
    // BUG-2: strip the trailing newline that toml_edit always adds, if the
    // original did not end with `\n`.
    let had_trailing_newline = original.ends_with('\n');
    let mut out = if !had_trailing_newline && toml_edit_output.ends_with('\n') {
        toml_edit_output[..toml_edit_output.len() - 1].to_string()
    } else {
        toml_edit_output.to_string()
    };

    // BUG-1: restore `\r\n` if the original used CRLF.
    let uses_crlf = original.contains("\r\n");
    if uses_crlf {
        out = out.replace('\n', "\r\n");
    }

    out
}

/// Build a `WorkspaceEdit` that renames `old_key` to `new_name` in a TOML
/// document via `toml_edit` lossless mutation.
///
/// The rename targets only the leaf key (last dotted segment) within its
/// containing table. Returns `None` when:
/// - `new_name` already exists as a sibling key in the same table as `old_key`
///   in any open doc (BUG-5 fix — collision check now compares sibling scope).
/// - The write capability is `ReadOnly`.
/// - `new_name` is the same as `old_key` or invalid.
/// - `toml_rename_key_in_content` returns `None` (e.g. key not found in doc).
///
/// Uses `toml_edit` to locate and rename the key at the correct position,
/// preserving comments, indentation, and ordering. The returned `TextEdit`
/// replaces the full document with the renamed content but with the original
/// EOL style and trailing-newline presence restored (BUG-3 fix).
pub fn config_toml_rename(
    old_key: &str,
    new_name: &str,
    write_capability: WriteCapability,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
    doc_contents: &HashMap<Url, String>,
) -> Option<WorkspaceEdit> {
    if write_capability == WriteCapability::ReadOnly {
        return None;
    }
    if !is_valid_toml_key_segment(new_name) || new_name == old_key {
        return None;
    }

    // BUG-5 fix: collision check must operate on the sibling scope, not the
    // full flattened key list. Compute the new dotted key that would result
    // from the rename: replace the leaf of `old_key` with `new_name`.
    let new_dotted_key = {
        let mut parts: Vec<&str> = old_key.split('.').collect();
        if let Some(last) = parts.last_mut() {
            *last = new_name;
        }
        parts.join(".")
    };

    // Check collision: the new dotted key must not already exist in any doc.
    for entries in open_docs.values() {
        if entries.iter().any(|e| e.key == new_dotted_key) {
            return None;
        }
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    for (uri, entries) in open_docs {
        // Only entries whose key matches old_key have renames.
        let has_match = entries.iter().any(|e| e.key == old_key);
        if !has_match {
            continue;
        }

        // Try toml_edit lossless rename if we have the document content.
        // BUG-5: if toml_rename_key_in_content returns None (collision or parse
        // failure), do NOT fall through to the surgical fallback — return None.
        if let Some(content) = doc_contents.get(uri) {
            match toml_rename_key_in_content(content, old_key, new_name) {
                Some(new_content) => {
                    changes.entry(uri.clone()).or_default().push(TextEdit {
                        range: full_doc_range(content),
                        new_text: new_content,
                    });
                    continue;
                }
                None => {
                    // toml_edit rejected the rename (key not found, parse error, or
                    // internal collision) — abort the entire rename operation.
                    return None;
                }
            }
        }

        // Fallback: surgical key-range text edits (no content available).
        for e in entries {
            if e.key == old_key {
                // Only rename the leaf segment (last dotted component).
                let leaf_new = new_name.rsplit('.').next().unwrap_or(new_name);
                changes.entry(uri.clone()).or_default().push(TextEdit {
                    range: e.key_range,
                    new_text: leaf_new.to_string(),
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

/// Apply a key rename in a TOML document string using `toml_edit`.
///
/// Navigates to the table path implied by the dotted `old_key` and renames
/// the leaf. Returns the mutated document string on success, `None` on any
/// error (parse failure, key not found, collision).
///
/// BUG-3 fix: `toml_edit::DocumentMut::to_string()` emits LF-only output and
/// always appends a trailing `\n`. We restore the original EOL style and
/// trailing-newline presence so the returned string differs from `content`
/// only in the renamed key bytes.
fn toml_rename_key_in_content(content: &str, old_key: &str, new_name: &str) -> Option<String> {
    use toml_edit::DocumentMut;

    let mut doc: DocumentMut = content.parse().ok()?;

    // Split old_key into table path + leaf.
    let parts: Vec<&str> = old_key.split('.').collect();
    let (leaf, table_path) = parts.split_last()?;
    // table_path is &[&str], leaf is &&str; dereference leaf.
    let leaf: &str = leaf;

    // Navigate to the containing table.
    let table = navigate_to_table(&mut doc, table_path)?;

    // Check leaf exists and new_name does not collide.
    if !table.contains_key(leaf) {
        return None;
    }
    if table.contains_key(new_name) {
        return None;
    }

    // Remove + re-insert to rename (toml_edit doesn't have a rename API).
    let item = table.remove(leaf)?;
    table.insert(new_name, item);

    // BUG-3: restore original EOL style and trailing-newline presence.
    Some(restore_eol_style(content, &doc.to_string()))
}

/// Navigate a mutable `toml_edit::DocumentMut` to the table at `path`.
///
/// Returns `None` if any segment along the path is missing or not a table.
fn navigate_to_table<'d>(
    doc: &'d mut toml_edit::DocumentMut,
    path: &[&str],
) -> Option<&'d mut toml_edit::Table> {
    if path.is_empty() {
        return Some(doc.as_table_mut());
    }

    // We can't easily navigate mutable nested tables with a single borrow chain
    // in safe Rust without cloning. Use a simpler approach: rebuild key path.
    let table_ref: &mut toml_edit::Table = doc.as_table_mut();
    // Use unsafe-free recursive navigation via get_mut chain.
    navigate_table_mut(table_ref, path)
}

fn navigate_table_mut<'t>(
    table: &'t mut toml_edit::Table,
    path: &[&str],
) -> Option<&'t mut toml_edit::Table> {
    if path.is_empty() {
        return Some(table);
    }
    let (head, rest) = path.split_first()?;
    let item = table.get_mut(head)?;
    match item {
        toml_edit::Item::Table(sub) => navigate_table_mut(sub, rest),
        _ => None,
    }
}

/// Validate that a string is a valid TOML bare key segment (letters, digits,
/// underscores, hyphens). Quoted keys are not considered here.
///
/// This is more permissive than `is_valid_config_key` which requires starting
/// with a letter/underscore: TOML allows digit-starting keys.
pub fn is_valid_toml_key_segment(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

// ── Intent 039 / Story 005: JSONC surgical rename ────────────────────────────

/// Build a `WorkspaceEdit` that renames `old_key` to `new_name` in JSONC
/// (`appsettings*.json`) documents via surgical byte-range splice.
///
/// Uses [`resolve_jsonc_key_span`][crate::parser::jsonc_config_parser::resolve_jsonc_key_span]
/// to locate the exact byte range of the leaf key token and [`SurgicalEdit`] to
/// splice only that range. Every other byte in every document is byte-identical
/// *by construction* — comments, trailing commas, values, and whitespace are
/// all preserved.
///
/// Returns `None` when:
/// - `new_name` is invalid (see [`is_valid_dotnet_key_segment`]).
/// - `new_name == old_key`.
/// - A collision exists (new key already present in any doc).
/// - The key span cannot be resolved (malformed JSONC, not found).
/// - `write_capability` is `ReadOnly`.
///
/// # Round-trip guarantee
/// The renamed document differs from the original only in the quoted key
/// string bytes (e.g. `"OldName"` → `"NewName"`). All surrounding bytes are
/// byte-identical *by construction* (`SurgicalEdit` splices only the range).
pub fn config_jsonc_rename(
    old_key: &str,
    new_name: &str,
    write_capability: WriteCapability,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
    doc_contents: &HashMap<Url, String>,
) -> Option<WorkspaceEdit> {
    use crate::ops::surgical_edit::SurgicalEdit;
    use crate::parser::jsonc_config_parser::resolve_jsonc_key_span;

    if write_capability == WriteCapability::ReadOnly {
        return None;
    }
    // Validate that the new leaf segment is safe to use as a JSON key.
    if !is_valid_dotnet_key_segment(new_name) || new_name == old_key {
        return None;
    }

    // Collision check: new_name must not already exist in any doc.
    // For JSONC, keys are `:` separated so the new dotted key is:
    // replace the leaf segment (last `:` part) of old_key with new_name.
    let new_colon_key = {
        let mut parts: Vec<&str> = old_key.split(':').collect();
        if let Some(last) = parts.last_mut() {
            *last = new_name;
        }
        parts.join(":")
    };
    for entries in open_docs.values() {
        if entries.iter().any(|e| e.key == new_colon_key) {
            return None;
        }
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    // Sort URIs for determinism (consistent edit ordering).
    let mut uris: Vec<&Url> = open_docs.keys().collect();
    uris.sort_by_key(|u| u.as_str());

    for uri in uris {
        let entries = &open_docs[uri];
        let has_match = entries.iter().any(|e| e.key == old_key);
        if !has_match {
            continue;
        }

        let content = match doc_contents.get(uri) {
            Some(c) => c,
            None => {
                // No content available: fall back to key_range-based edit using
                // the leaf segment (last `:` part) as the rename target.
                let leaf_new = new_name;
                for e in entries {
                    if e.key == old_key {
                        changes.entry(uri.clone()).or_default().push(TextEdit {
                            range: e.key_range,
                            new_text: format!("\"{}\"", leaf_new),
                        });
                    }
                }
                continue;
            }
        };

        // Resolve the leaf key span via jsonc-parser CST.
        match resolve_jsonc_key_span(content, old_key) {
            Some(span) => {
                // The span covers the entire quoted key string including quotes.
                // Produce a replacement that includes quotes.
                let replacement = format!("\"{}\"", new_name);
                let se =
                    SurgicalEdit::new(span.byte_range.clone(), replacement.clone(), content.len());
                let se = match se {
                    Some(s) => s,
                    None => continue, // invalid range — skip
                };
                let text_edit = match se.to_text_edit(content) {
                    Some(t) => t,
                    None => continue,
                };
                changes.entry(uri.clone()).or_default().push(text_edit);
            }
            None => {
                // Key not found or malformed — skip this document gracefully.
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

/// Compute diagnostics for a JSONC (`appsettings*.json`) config file.
///
/// Combines:
/// 1. JSONC parse/syntax errors (surfaced by the parser).
/// 2. Duplicate-key warnings from the flattened entry model.
/// 3. Unknown-key-vs-schema warnings (when schema is present).
///
/// Never panics; malformed JSONC returns a parse-error diagnostic.
pub fn config_jsonc_diagnostics(
    content: &str,
    layer: crate::ops::config_format::SourceLayer,
    schema: Option<&crate::ops::schema::EnvSchema>,
) -> Vec<Diagnostic> {
    use crate::parser::jsonc_config_parser::{parse_jsonc_config, JsoncDiagnostic};
    use std::collections::HashMap as HMap;

    let (entries, jsonc_diags) = parse_jsonc_config(content, layer);

    let mut diags: Vec<Diagnostic> = jsonc_diags
        .iter()
        .map(|d: &JsoncDiagnostic| Diagnostic {
            range: d.range,
            severity: Some(d.severity),
            source: Some("envforge".into()),
            message: d.message.clone(),
            ..Default::default()
        })
        .collect();

    // Duplicate-key pass.
    let mut seen_keys: HMap<String, u32> = HMap::new();
    for entry in &entries {
        if entry.key.is_empty() {
            continue;
        }
        if let Some(&first_line) = seen_keys.get(&entry.key) {
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
            seen_keys.insert(entry.key.clone(), entry.line);
        }
    }

    // Schema-based diagnostics — H-1 fix: use canonical-key matching so a
    // JSONC key `Logging:LogLevel:Default` is NOT flagged "unknown" when the
    // schema has `LOGGING__LOGLEVEL__DEFAULT` (they share the same strict
    // canonical key `logging.loglevel.default`).
    if let Some(schema) = schema {
        for entry in &entries {
            if entry.key.is_empty() {
                continue;
            }
            // Exact match first (fast path); fall back to canonical matching.
            let in_schema = schema.variables.contains_key(&entry.key)
                || canonical_key_strict(&entry.key).is_some_and(|ck| {
                    schema
                        .variables
                        .keys()
                        .any(|k| canonical_key_strict(k).as_deref() == Some(ck.as_str()))
                });
            if !in_schema {
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

/// Validate that a string is a valid .NET / JSONC key segment for rename purposes.
///
/// Accepts letters, digits, underscores, hyphens, and dots (all common in
/// appsettings key segments). Must be non-empty.
pub fn is_valid_dotnet_key_segment(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}
