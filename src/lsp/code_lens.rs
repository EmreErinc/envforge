use std::collections::HashSet;

use tower_lsp::lsp_types::*;

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::{EnvDocEntry, EnvLineType};

/// Build the CodeLens set rendered above each env-var line.
/// Since execute_command_provider is disabled (the LSP is a read-only
/// security boundary), all lenses are decorative — they convey
/// information without actionable commands. Mutations (fence, canary,
/// reveal) must go through the CLI.
pub fn code_lenses(
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    canary_keys: Option<&HashSet<String>>,
    uri: Option<&Url>,
) -> Vec<CodeLens> {
    let _ = uri;
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
            let canary_label = if already_canary {
                "$(bug) canary active"
            } else {
                "$(bug) canary: use `envforge canary plant`"
            };
            lenses.push(CodeLens {
                range,
                command: Some(Command {
                    title: canary_label.to_string(),
                    command: String::new(),
                    arguments: None,
                }),
                data: None,
            });
            lenses.push(CodeLens {
                range,
                command: Some(Command {
                    title: "$(shield) fence: use `envforge fence enable`".to_string(),
                    command: String::new(),
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
