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
                    || v.value.to_lowercase().contains(&query_lower)
            }
        })
        .take(100)
        .filter_map(|v| {
            let uri = Url::parse(&format!("file://{}", v.source_file)).ok()?;
            let is_sensitive = schema
                .and_then(|s| s.variables.get(&v.key))
                .map(|sv| sv.sensitive)
                .unwrap_or(false)
                || is_sensitive_key(&v.key);

            let display_name = if is_sensitive {
                format!("{} = ***", v.key)
            } else {
                let val_display = if v.value.len() > 30 {
                    format!("{}...", &v.value[..30])
                } else {
                    v.value.clone()
                };
                format!("{} = {}", v.key, val_display)
            };

            Some(
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
                },
            )
        })
        .collect()
}
