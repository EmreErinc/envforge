use std::collections::HashSet;

use tower_lsp::lsp_types::*;

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::{EnvDocEntry, EnvLineType};

/// Build the CodeLens set rendered above each env-var line. The previous
/// implementation emitted decorative-only entries with empty command
/// strings; clients displayed the text but the lenses did nothing on
/// click. This pass keeps a small amount of schema-info text but adds
/// real actionable lenses that drive the executeCommand command bus.
///
/// Per-line lenses emitted on `EnvVar` entries:
/// - **Plant canary** (sensitive keys without a registered canary):
///   invokes `envforge.canary.plant` with `{ key, pattern }`. The
///   pattern hint mirrors the L6 plant quick-fix so subprocess vs LSP
///   paths produce the same canary fingerprint.
/// - **Activate fence** (sensitive keys, always offered): invokes
///   `envforge.fence.enable`. We do not gate this by current fence
///   state — the underlying op is idempotent and reading state per
///   lens request would multiply IO cost.
/// - **Schema type label** (decorative, kept for parity with the prior
///   experience): no command attached.
/// - **Required label** (decorative).
pub fn code_lenses(
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    canary_keys: Option<&HashSet<String>>,
    uri: Option<&Url>,
) -> Vec<CodeLens> {
    let file_path: Option<String> = uri
        .and_then(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned());
    let mut lenses = Vec::new();

    for entry in entries {
        if entry.line_type != EnvLineType::EnvVar {
            continue;
        }

        let schema_var = schema.and_then(|s| s.variables.get(&entry.key));
        let is_sensitive =
            schema_var.map(|v| v.sensitive).unwrap_or(false) || is_sensitive_key(&entry.key);

        let range = Range {
            start: Position {
                line: entry.line,
                character: 0,
            },
            end: Position {
                line: entry.line,
                character: 0,
            },
        };

        if is_sensitive {
            let already_canary = canary_keys
                .map(|set| set.contains(&entry.key))
                .unwrap_or(false);
            if !already_canary {
                let mut plant_args = serde_json::json!({
                    "key": entry.key,
                    "pattern": canary_pattern_hint(&entry.key),
                });
                if let Some(ref fp) = file_path {
                    plant_args["file"] = serde_json::Value::String(fp.clone());
                }
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: "$(bug) Plant canary".to_string(),
                        command: "envforge.canary.plant".to_string(),
                        arguments: Some(vec![plant_args]),
                    }),
                    data: None,
                });
            } else {
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: "$(bug) canary active".to_string(),
                        command: String::new(),
                        arguments: None,
                    }),
                    data: None,
                });
            }
            lenses.push(CodeLens {
                range,
                command: Some(Command {
                    title: "$(shield) Activate fence".to_string(),
                    command: "envforge.fence.enable".to_string(),
                    arguments: None,
                }),
                data: None,
            });
        }

        if let Some(var_def) = schema_var {
            lenses.push(CodeLens {
                range,
                command: Some(Command {
                    title: format!("type: {}", var_def.var_type.display()),
                    command: String::new(),
                    arguments: None,
                }),
                data: None,
            });
            if var_def.required {
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: "required".into(),
                        command: String::new(),
                        arguments: None,
                    }),
                    data: None,
                });
            }
        }
    }

    lenses
}

fn canary_pattern_hint(key: &str) -> &'static str {
    let upper = key.to_uppercase();
    if upper.contains("AWS") {
        "aws_key"
    } else if upper.contains("TOKEN") || upper.contains("API_KEY") || upper.contains("APIKEY") {
        "api_token"
    } else {
        "generic"
    }
}
