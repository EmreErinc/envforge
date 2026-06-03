use tower_lsp::lsp_types::*;

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

pub fn workspace_symbols(
    query: &str,
    managed_vars: &[super::server::ManagedVar],
    schema: Option<&EnvSchema>,
) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();

    managed_vars
        .iter()
        .filter(|v| {
            if query.is_empty() {
                true
            } else {
                v.key.to_lowercase().contains(&query_lower)
            }
        })
        .take(100)
        .map(|v| {
            let is_sensitive = schema
                .and_then(|s| s.variables.get(&v.key))
                .map(|sv| sv.sensitive)
                .unwrap_or(false)
                || is_sensitive_key(&v.key);

            let display_name = if is_sensitive {
                format!("{} (secret)", v.key)
            } else {
                format!("{} (managed)", v.key)
            };

            let source_uri = if is_sensitive {
                String::new()
            } else {
                v.source_file.clone()
            };
            let uri = Url::parse(&format!("file://{}", source_uri))
                .unwrap_or_else(|_| Url::parse("file:///").expect("hardcoded fallback URI"));

            #[allow(deprecated)]
            SymbolInformation {
                name: display_name,
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                location: Location {
                    uri,
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                },
                container_name: None,
            }
        })
        .collect()
}
