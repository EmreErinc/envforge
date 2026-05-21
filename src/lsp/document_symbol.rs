use tower_lsp::lsp_types::*;

use super::document::EnvDocEntry;

pub fn document_symbols(entries: &[EnvDocEntry]) -> Option<DocumentSymbolResponse> {
    let symbols: Vec<DocumentSymbol> = entries
        .iter()
        .filter(|e| e.line_type == super::document::EnvLineType::EnvVar)
        .map(|entry| {
            let full_range = Range {
                start: Position {
                    line: entry.line,
                    character: 0,
                },
                end: Position {
                    line: entry.value_range.end.line,
                    character: entry.value_range.end.character,
                },
            };

            let detail = if entry.value.is_empty() {
                None
            } else if entry.value.len() > 40 {
                Some(format!("{}...", &entry.value[..40]))
            } else {
                Some(entry.value.clone())
            };

            #[allow(deprecated)]
            DocumentSymbol {
                name: entry.key.clone(),
                detail,
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                range: full_range,
                selection_range: entry.key_range,
                children: None,
            }
        })
        .collect();

    if symbols.is_empty() {
        None
    } else {
        Some(DocumentSymbolResponse::Nested(symbols))
    }
}
