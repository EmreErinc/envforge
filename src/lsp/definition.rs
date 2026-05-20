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

    Some(schema_location(schema_uri, *line))
}

/// Resolve a goto-definition request originating from a source file
/// (TypeScript, JavaScript, Python, Rust, Go, Java, etc.) to the schema
/// entry that declares the env var identifier under the cursor. The
/// identifier is extracted by walking outward from the cursor over
/// `[A-Z0-9_]` characters and required to be `UPPER_SNAKE_CASE` so we
/// don't pop the schema open for arbitrary local variables.
pub fn goto_definition_from_source(
    position: Position,
    source_text: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
) -> Option<GotoDefinitionResponse> {
    let schema_uri = schema_uri?;
    let line = source_text.lines().nth(position.line as usize)?;
    let identifier = extract_upper_snake_identifier(line, position.character as usize)?;
    let schema_line = schema_line_map.get(&identifier)?;
    Some(schema_location(schema_uri, *schema_line))
}

fn schema_location(uri: &Url, line: u32) -> GotoDefinitionResponse {
    GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range: LspRange {
            start: Position { line, character: 0 },
            end: Position { line, character: 0 },
        },
    })
}

/// Pull the `UPPER_SNAKE_CASE` identifier the cursor is currently sitting
/// inside or directly adjacent to. Returns `None` for: empty matches,
/// single-character matches, all-digit / underscore-only matches, or any
/// identifier that does not contain at least one ASCII uppercase letter.
/// The letter rule is what gates this from firing on unrelated local
/// identifiers (`x`, `foo`, `count`, …).
pub fn extract_upper_snake_identifier(line: &str, col: usize) -> Option<String> {
    if !line.is_ascii() {
        // Conservative: bytes-based walk only on ASCII lines so we never
        // slice into the middle of a multi-byte UTF-8 sequence.
        return extract_upper_snake_identifier_unicode(line, col);
    }

    let bytes = line.as_bytes();
    let len = bytes.len();
    let clamped = col.min(len);
    let is_id = |b: u8| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_';

    let mut start = clamped;
    while start > 0 && is_id(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = clamped;
    while end < len && is_id(bytes[end]) {
        end += 1;
    }

    if start >= end {
        return None;
    }
    let candidate = &line[start..end];
    if candidate.len() < 2 {
        return None;
    }
    if !candidate.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some(candidate.to_string())
}

fn extract_upper_snake_identifier_unicode(line: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let clamped = col.min(len);
    let is_id = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_';

    let mut start = clamped;
    while start > 0 && is_id(chars[start - 1]) {
        start -= 1;
    }
    let mut end = clamped;
    while end < len && is_id(chars[end]) {
        end += 1;
    }

    if start >= end {
        return None;
    }
    let candidate: String = chars[start..end].iter().collect();
    if candidate.len() < 2 {
        return None;
    }
    if !candidate.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some(candidate)
}
