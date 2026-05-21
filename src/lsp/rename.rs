use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};

use super::document::DocumentState;

/// Build a workspace edit that propagates a rename from `old_key` to
/// `new_name` across:
/// - the schema file (rewrites the `[OLD]` table header on its known line)
/// - every currently-open `.env*` document (rewrites the matching
///   `key_range` for each entry whose key equals `old_key`)
///
/// Returns `None` when:
/// - `new_name` is not a valid env-var identifier
/// - the rename is a no-op (`new_name == old_key`)
/// - neither the schema nor any open env doc references `old_key`
///
/// Source-file references are intentionally not edited here; clients can
/// follow up with their own refactor tooling for code references. The
/// schema + `.env*` propagation is the high-value path because that's
/// where envforge owns the truth.
pub fn build_rename_edit(
    old_key: &str,
    new_name: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_env_docs: &HashMap<Url, DocumentState>,
) -> Option<WorkspaceEdit> {
    if !is_valid_env_identifier(new_name) {
        return None;
    }
    if new_name == old_key {
        return None;
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    if let (Some(uri), Some(line)) = (schema_uri, schema_line_map.get(old_key)) {
        let header_range = Range {
            start: Position {
                line: *line,
                character: 0,
            },
            end: Position {
                line: *line,
                character: schema_header_width(old_key),
            },
        };
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range: header_range,
            new_text: format!("[{}]", new_name),
        });
    }

    for (uri, doc) in open_env_docs {
        for entry in &doc.entries {
            if entry.key == old_key {
                changes.entry(uri.clone()).or_default().push(TextEdit {
                    range: entry.key_range,
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

/// Width in UTF-16 code units of a schema table header `[KEY]`. For
/// ASCII identifiers (which is all envforge supports for env-var names)
/// this is just `len + 2` for the brackets. Kept as a helper so we can
/// extend if non-ASCII keys are ever supported.
fn schema_header_width(key: &str) -> u32 {
    (key.len() + 2) as u32
}

/// Env-var identifier rule: ASCII letter or underscore start, followed
/// by ASCII letters, digits, or underscores. Matches the POSIX/shell
/// convention and rejects anything that wouldn't be a legal shell
/// `export` target.
pub fn is_valid_env_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
