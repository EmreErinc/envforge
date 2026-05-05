use tower_lsp::lsp_types::*;

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::EnvDocEntry;

pub fn code_lenses(entries: &[EnvDocEntry], schema: Option<&EnvSchema>) -> Vec<CodeLens> {
    let mut lenses = Vec::new();

    for entry in entries {
        if entry.line_type != super::document::EnvLineType::EnvVar {
            continue;
        }

        let is_sensitive = schema
            .and_then(|s| s.variables.get(&entry.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || is_sensitive_key(&entry.key);

        if is_sensitive {
            lenses.push(CodeLens {
                range: Range {
                    start: Position {
                        line: entry.line,
                        character: 0,
                    },
                    end: Position {
                        line: entry.line,
                        character: 0,
                    },
                },
                command: Some(Command {
                    title: "sensitive".into(),
                    command: "".into(),
                    arguments: None,
                }),
                data: None,
            });
        }

        if let Some(schema) = schema {
            if let Some(var_def) = schema.variables.get(&entry.key) {
                lenses.push(CodeLens {
                    range: Range {
                        start: Position {
                            line: entry.line,
                            character: 0,
                        },
                        end: Position {
                            line: entry.line,
                            character: 0,
                        },
                    },
                    command: Some(Command {
                        title: format!("type: {}", var_def.var_type.display()),
                        command: "".into(),
                        arguments: None,
                    }),
                    data: None,
                });

                if var_def.required {
                    lenses.push(CodeLens {
                        range: Range {
                            start: Position {
                                line: entry.line,
                                character: 0,
                            },
                            end: Position {
                                line: entry.line,
                                character: 0,
                            },
                        },
                        command: Some(Command {
                            title: "required".into(),
                            command: "".into(),
                            arguments: None,
                        }),
                        data: None,
                    });
                }
            }
        }
    }

    lenses
}
