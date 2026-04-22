use std::collections::HashMap;

use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Range as LspRange, Url};

use super::document::EnvDocEntry;

pub fn goto_definition(
    position: Position,
    entries: &[EnvDocEntry],
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
) -> Option<GotoDefinitionResponse> {
    let schema_uri = schema_uri?;

    // Find which key the cursor is on
    let entry = entries.iter().find(|e| {
        e.line == position.line
            && position.character >= e.key_range.start.character
            && position.character <= e.key_range.end.character
    })?;

    let line = schema_line_map.get(&entry.key)?;

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: schema_uri.clone(),
        range: LspRange {
            start: Position {
                line: *line,
                character: 0,
            },
            end: Position {
                line: *line,
                character: 0,
            },
        },
    }))
}
