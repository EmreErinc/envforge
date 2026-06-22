//! JSONC config parser — Intent 039, Story 002.
//!
//! Parses `appsettings.json` / `appsettings.{Environment}.json` JSONC files
//! (comments + trailing commas allowed) into the format-agnostic `ConfigEntry`
//! model via the `jsonc-parser` comment-preserving CST.
//!
//! # Key design decisions
//!
//! - **`:`-joined paths** — nested objects flatten to colon-separated keys
//!   following the .NET convention (e.g. `Logging:LogLevel:Default`).
//! - **UTF-16 positions** — all LSP `Range` values use UTF-16 code units,
//!   consistent with the rest of the config-format machinery.
//! - **No panic on malformed input** — syntax errors are returned as
//!   `JsoncDiagnostic` values; the caller gets an empty (or partial) entry
//!   list plus diagnostics rather than a crash.
//! - **No whole-document re-serialization** — the AST is used for position
//!   extraction only; writes use `SurgicalEdit` on the raw byte range.
//! - **Arrays skipped** — JSON arrays are an acknowledged gap (intent 039
//!   scope); array values are omitted from the entry list but do not panic.

use jsonc_parser::{
    ast::{ObjectPropName, Value},
    common::Ranged,
    CollectOptions, CommentCollectionStrategy, ParseOptions, ParseResult,
};
use tower_lsp::lsp_types::{Position, Range as LspRange};

use crate::ops::config_format::{ConfigEntry, SourceLayer};

/// Lightweight diagnostic emitted for JSONC parse / semantic issues.
/// Converted to `tower_lsp::lsp_types::Diagnostic` by the LSP layer.
#[derive(Debug, Clone)]
pub struct JsoncDiagnostic {
    pub range: LspRange,
    pub message: String,
    pub severity: tower_lsp::lsp_types::DiagnosticSeverity,
}

// ── UTF-16 helpers ────────────────────────────────────────────────────────────

/// Convert a byte offset within `content` to a UTF-16 LSP `Position`.
///
/// L-3 fix: for out-of-bounds offsets, clamp to the end of content (last line,
/// last character) instead of returning (0, 0). Returning (0, 0) for a bad
/// offset silently produces a ghost edit at the start of the file; clamping to
/// end-of-content means any edit that uses an out-of-bounds range targets the
/// end rather than corrupting the beginning.
///
/// This function never panics; all inputs are handled gracefully.
fn byte_offset_to_lsp_position(content: &str, byte_offset: usize) -> Position {
    // Clamp to content length so we never slice out of bounds.
    let byte_offset = byte_offset.min(content.len());
    let before = &content[..byte_offset];
    let line_num = before.bytes().filter(|&b| b == b'\n').count();
    let line_start_byte = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let col_slice = &content[line_start_byte..byte_offset];
    let utf16_col: u32 = col_slice.chars().map(|c| c.len_utf16() as u32).sum();
    Position {
        line: line_num as u32,
        character: utf16_col,
    }
}

/// Convert a byte range `[start, end)` to an LSP `Range` with UTF-16 positions.
fn byte_range_to_lsp_range(content: &str, start: usize, end: usize) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(content, start),
        end: byte_offset_to_lsp_position(content, end),
    }
}

// ── Key-span record ──────────────────────────────────────────────────────────

/// The exact byte span of a JSON key string token (including surrounding quotes).
/// Used by `config_jsonc_rename` via `SurgicalEdit` to splice only the key bytes.
#[derive(Debug, Clone)]
pub struct JsoncKeySpan {
    /// Byte range covering the entire quoted key string (e.g. `"Logging"`).
    pub byte_range: std::ops::Range<usize>,
    /// Whether the key was quoted (always `true` for valid JSON).
    pub is_quoted: bool,
}

// ── Prop-name range helper ───────────────────────────────────────────────────

/// Extract the byte range of an `ObjectPropName` node.
fn prop_name_range(name: &ObjectPropName<'_>) -> jsonc_parser::common::Range {
    name.range()
}

// ── Entry flattening ─────────────────────────────────────────────────────────

/// Flatten a JSONC `Value::Object` into `ConfigEntry` items using `:`-joined
/// paths. Nested objects recurse; arrays are skipped (acknowledged gap).
fn flatten_object(
    content: &str,
    value: &jsonc_parser::ast::Object<'_>,
    prefix: &str,
    layer: &SourceLayer,
    entries: &mut Vec<ConfigEntry>,
    _diags: &mut Vec<JsoncDiagnostic>,
) {
    for prop in &value.properties {
        let key_text = prop.name.as_str(); // unquoted key text
        let joined = if prefix.is_empty() {
            key_text.to_string()
        } else {
            format!("{}:{}", prefix, key_text)
        };

        // Byte range of the key string node (includes surrounding quotes).
        let name_range = prop_name_range(&prop.name);
        let key_range = byte_range_to_lsp_range(content, name_range.start, name_range.end);

        match &prop.value {
            Value::Object(nested) => {
                // Recurse into nested object — build path prefix.
                flatten_object(content, nested, &joined, layer, entries, _diags);
            }
            Value::Array(_) => {
                // Arrays are an acknowledged out-of-scope item (intent 039).
                // Silently skip — no crash, no entry.
            }
            Value::StringLit(s) => {
                let value_range = byte_range_to_lsp_range(content, s.range.start, s.range.end);
                let raw_val = s.value.to_string();
                let line = key_range.start.line;
                entries.push(ConfigEntry {
                    key: joined,
                    value: raw_val,
                    key_range,
                    value_range,
                    line,
                    source_layer: layer.clone(),
                });
            }
            Value::NumberLit(n) => {
                let value_range = byte_range_to_lsp_range(content, n.range.start, n.range.end);
                let raw_val = n.value;
                let line = key_range.start.line;
                entries.push(ConfigEntry {
                    key: joined,
                    value: raw_val.to_string(),
                    key_range,
                    value_range,
                    line,
                    source_layer: layer.clone(),
                });
            }
            Value::BooleanLit(b) => {
                let value_range = byte_range_to_lsp_range(content, b.range.start, b.range.end);
                let raw_val = if b.value { "true" } else { "false" };
                let line = key_range.start.line;
                entries.push(ConfigEntry {
                    key: joined,
                    value: raw_val.to_string(),
                    key_range,
                    value_range,
                    line,
                    source_layer: layer.clone(),
                });
            }
            Value::NullKeyword(n) => {
                let value_range = byte_range_to_lsp_range(content, n.range.start, n.range.end);
                let line = key_range.start.line;
                entries.push(ConfigEntry {
                    key: joined,
                    value: String::new(),
                    key_range,
                    value_range,
                    line,
                    source_layer: layer.clone(),
                });
            }
        }
    }
}

// ── Public parse API ─────────────────────────────────────────────────────────

/// Standard `ParseOptions` for JSONC / .NET appsettings files.
/// Allows comments and trailing commas; disallows loose property names so
/// all keys must be quoted strings (standard JSON / appsettings convention).
fn appsettings_parse_options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    }
}

/// Parse JSONC `content` into a positioned `ConfigEntry` list plus diagnostics.
///
/// - Comments and trailing commas are accepted (JSONC dialect).
/// - Nested objects flatten to `:`-joined paths (e.g. `Logging:LogLevel:Default`).
/// - UTF-16-correct `key_range` and `value_range` are set on every entry.
/// - Malformed input returns a non-empty `diags` list; partial entries are
///   returned for however much was successfully parsed.
/// - Never panics on any input (BOM, empty string, deeply nested, huge file).
///
/// # Examples
///
/// ```
/// use envforge::parser::jsonc_config_parser::parse_jsonc_config;
/// use envforge::ops::config_format::SourceLayer;
///
/// let jsonc = r#"{ "Logging": { "LogLevel": { "Default": "Information" } } }"#;
/// let (entries, diags) = parse_jsonc_config(jsonc, SourceLayer::DotNetBase);
/// assert!(diags.is_empty());
/// assert_eq!(entries.len(), 1);
/// assert_eq!(entries[0].key, "Logging:LogLevel:Default");
/// assert_eq!(entries[0].value, "Information");
/// ```
pub fn parse_jsonc_config(
    content: &str,
    layer: SourceLayer,
) -> (Vec<ConfigEntry>, Vec<JsoncDiagnostic>) {
    // Strip UTF-8 BOM (EF BB BF) if present — some .NET tooling emits it.
    let content = if content.starts_with('\u{FEFF}') {
        &content['\u{FEFF}'.len_utf8()..]
    } else {
        content
    };

    let collect = CollectOptions {
        comments: CommentCollectionStrategy::Off,
        tokens: false,
    };
    let opts = appsettings_parse_options();

    let result: ParseResult<'_> = match jsonc_parser::parse_to_ast(content, &collect, &opts) {
        Ok(r) => r,
        Err(e) => {
            // Syntax error — emit a diagnostic. line_display / column_display are
            // 1-indexed display values; convert to 0-indexed LSP line/char.
            let line = (e.line_display().saturating_sub(1)) as u32;
            let character = (e.column_display().saturating_sub(1)) as u32;
            let pos = Position { line, character };
            let diag = JsoncDiagnostic {
                range: LspRange {
                    start: pos,
                    end: Position {
                        line: pos.line,
                        character: pos.character + 1,
                    },
                },
                message: format!("JSONC syntax error: {}", e),
                severity: tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
            };
            return (Vec::new(), vec![diag]);
        }
    };

    let mut entries = Vec::new();
    let mut diags = Vec::new();

    match result.value {
        Some(Value::Object(root)) => {
            flatten_object(content, &root, "", &layer, &mut entries, &mut diags);
        }
        Some(_) => {
            // Top-level value is not an object — not a valid appsettings format.
            diags.push(JsoncDiagnostic {
                range: LspRange::default(),
                message: "appsettings file must be a JSON object at the top level".to_string(),
                severity: tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
            });
        }
        None => {
            // Empty file — valid, no entries.
        }
    }

    (entries, diags)
}

// ── Key-span resolver for SurgicalEdit ───────────────────────────────────────

/// Resolve the byte span of the **leaf** key node for a `:`-joined `dotnet_key`
/// path within `content`. Returns `None` when the path is not found, the
/// content is malformed, or the path traverses an array (acknowledged gap).
///
/// The returned `JsoncKeySpan.byte_range` covers the entire quoted string
/// token (e.g. `"Default"` — quotes included). `SurgicalEdit` should replace
/// the whole quoted range to produce a valid renamed key.
///
/// # Round-trip safety
/// Only the quoted key string bytes are replaced. Every other byte — comments,
/// whitespace, values, structure — is unchanged by construction.
pub fn resolve_jsonc_key_span(content: &str, dotnet_key: &str) -> Option<JsoncKeySpan> {
    let collect = CollectOptions {
        comments: CommentCollectionStrategy::Off,
        tokens: false,
    };
    let opts = appsettings_parse_options();

    // C-3 fix: track BOM byte length so returned ranges can be adjusted back
    // to be relative to the ORIGINAL content (including the BOM), not the
    // BOM-stripped slice.  SurgicalEdit always operates on the original bytes.
    let bom_len = if content.starts_with('\u{FEFF}') {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    };

    // Strip BOM if present before parsing.
    let content_trimmed = &content[bom_len..];

    let result = jsonc_parser::parse_to_ast(content_trimmed, &collect, &opts).ok()?;

    let segments: Vec<&str> = dotnet_key.split(':').collect();
    let (leaf, path) = segments.split_last()?;

    // Navigate to the containing object.
    let root_obj = match result.value? {
        Value::Object(obj) => obj,
        _ => return None,
    };

    // Walk down the path segments to find the leaf key span.
    // The returned span is relative to content_trimmed (BOM-stripped).
    let span = find_key_span_in_object(root_obj, path, leaf)?;

    // C-3 fix: shift the byte range by bom_len so it's relative to the
    // original content (which the caller, SurgicalEdit, operates on).
    Some(JsoncKeySpan {
        byte_range: (span.byte_range.start + bom_len)..(span.byte_range.end + bom_len),
        is_quoted: span.is_quoted,
    })
}

/// Recursively navigate into `obj` following `remaining_path` segments, then
/// find and return the span of `leaf` key in the final object.
fn find_key_span_in_object(
    obj: jsonc_parser::ast::Object<'_>,
    remaining_path: &[&str],
    leaf: &str,
) -> Option<JsoncKeySpan> {
    if remaining_path.is_empty() {
        // At the containing object — find the leaf.
        for prop in obj.properties {
            if prop.name.as_str() == leaf {
                let name_range = prop_name_range(&prop.name);
                let is_quoted = matches!(prop.name, ObjectPropName::String(_));
                return Some(JsoncKeySpan {
                    byte_range: name_range.start..name_range.end,
                    is_quoted,
                });
            }
        }
        return None;
    }

    // Navigate into the next path segment.
    let (head, rest) = remaining_path.split_first()?;
    for prop in obj.properties {
        if prop.name.as_str() == *head {
            return match prop.value {
                Value::Object(nested) => find_key_span_in_object(nested, rest, leaf),
                _ => None, // path traverses non-object
            };
        }
    }
    None
}

// ── Env-var binding (story 003) ──────────────────────────────────────────────

/// Map a .NET double-underscore environment-variable name to its
/// `:`-joined JSON configuration path.
///
/// .NET binds `Logging__LogLevel__Default` to `Logging:LogLevel:Default`.
///
/// # Examples
///
/// ```
/// use envforge::parser::jsonc_config_parser::env_var_to_json_path;
///
/// assert_eq!(env_var_to_json_path("Logging__LogLevel__Default"), "Logging:LogLevel:Default");
/// assert_eq!(env_var_to_json_path("ConnectionStrings__Default"), "ConnectionStrings:Default");
/// assert_eq!(env_var_to_json_path("ASPNETCORE_ENVIRONMENT"), "ASPNETCORE_ENVIRONMENT");
/// ```
pub fn env_var_to_json_path(env_var: &str) -> String {
    env_var.replace("__", ":")
}

/// Map a `:`-joined JSON configuration path to its .NET environment-variable name.
///
/// # Examples
///
/// ```
/// use envforge::parser::jsonc_config_parser::json_path_to_env_var;
///
/// assert_eq!(json_path_to_env_var("Logging:LogLevel:Default"), "Logging__LogLevel__Default");
/// assert_eq!(json_path_to_env_var("ConnectionStrings:Default"), "ConnectionStrings__Default");
/// ```
pub fn json_path_to_env_var(json_path: &str) -> String {
    json_path.replace(':', "__")
}

#[cfg(test)]
mod tests {
    // Tests live in tests/dotnet_appsettings_tests.rs per CLAUDE.md.
}
