use tower_lsp::lsp_types::*;

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::EnvDocEntry;
use super::redact::redact_for_label;

pub fn document_symbols(
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
) -> Option<DocumentSymbolResponse> {
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

            let sensitive = schema
                .and_then(|s| s.variables.get(&entry.key))
                .map(|v| v.sensitive)
                .unwrap_or(false)
                || is_sensitive_key(&entry.key);

            let detail = if entry.value.is_empty() {
                None
            } else {
                Some(redact_for_label(&entry.value, sensitive))
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
