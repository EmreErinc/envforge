use std::collections::HashMap;

use tower_lsp::lsp_types::{Location, Position, Range, Url};

use super::document::DocumentState;

/// Collect every location at which `key` is referenced across the
/// schema file and currently-open `.env*` documents. The schema header
/// location is treated as the symbol's declaration; callers pass
/// `include_declaration = false` to omit it (matching LSP semantics
/// for `ReferenceParams.context.includeDeclaration`).
///
/// Source-file references are intentionally not searched here. Doing so
/// would require walking the workspace on every request; that's a
/// separate (workspace-symbol-style) feature and out of scope for the
/// first reference-finder cut.
pub fn find_references(
    key: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_env_docs: &HashMap<Url, DocumentState>,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();

    if include_declaration {
        if let (Some(uri), Some(line)) = (schema_uri, schema_line_map.get(key)) {
            locations.push(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: *line,
                        character: 0,
                    },
                    end: Position {
                        line: *line,
                        character: schema_header_width(key),
                    },
                },
            });
        }
    }

    for (uri, doc) in open_env_docs {
        for entry in &doc.entries {
            if entry.key == key {
                locations.push(Location {
                    uri: uri.clone(),
                    range: entry.key_range,
                });
            }
        }
    }

    locations
}

fn schema_header_width(key: &str) -> u32 {
    (key.len() + 2) as u32
}
