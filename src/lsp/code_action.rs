use tower_lsp::lsp_types::*;

use crate::ops::schema::{resolve_effective, EnvSchema};

use super::document::EnvDocEntry;

pub fn code_actions(
    uri: &Url,
    entries: &[EnvDocEntry],
    diagnostics: &[Diagnostic],
    schema: Option<&EnvSchema>,
) -> Option<CodeActionResponse> {
    let mut actions: Vec<CodeAction> = Vec::new();

    for diag in diagnostics {
        if let Some(action) = action_from_message(uri, entries, diag, schema) {
            actions.push(action);
        }
    }

    if actions.is_empty() {
        None
    } else {
        Some(
            actions
                .into_iter()
                .map(CodeActionOrCommand::CodeAction)
                .collect(),
        )
    }
}

fn action_from_message(
    uri: &Url,
    entries: &[EnvDocEntry],
    diag: &Diagnostic,
    schema: Option<&EnvSchema>,
) -> Option<CodeAction> {
    let msg = &diag.message;

    if msg.starts_with("Missing required variable:") {
        let key = msg.strip_prefix("Missing required variable: ")?;
        let value = schema
            .and_then(|s| s.variables.get(key))
            .and_then(|v| v.default.clone())
            .unwrap_or_default();

        let insert_text = if value.is_empty() {
            format!("{}=", key)
        } else {
            format!("{}={}", key, value)
        };

        let line = find_last_env_line(entries);

        return Some(CodeAction {
            title: format!("Add {}", key),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diag.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    [(
                        uri.clone(),
                        vec![TextEdit {
                            range: Range {
                                start: Position {
                                    line: line + 1,
                                    character: 0,
                                },
                                end: Position {
                                    line: line + 1,
                                    character: 0,
                                },
                            },
                            new_text: format!("{}\n", insert_text),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });
    }

    if msg.contains("Sensitive value for '") {
        let start = msg.find('\'').map(|i| i + 1)?;
        let end = msg.rfind('\'')?;
        if start >= end {
            return None;
        }
        let key = &msg[start..end];

        let entry = entries.iter().find(|e| e.key == key)?;

        return Some(CodeAction {
            title: format!("Use secret reference for {}", key),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diag.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    [(
                        uri.clone(),
                        vec![TextEdit {
                            range: entry.value_range,
                            new_text: format!("ref:provider/{}", key.to_lowercase()),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });
    }

    if msg.contains("Type validation failed") || msg.contains("Invalid") {
        let line = diag.range.start.line;
        let entry = entries.iter().find(|e| e.line == line)?;

        if let Some(schema) = schema {
            if let Some(var_def) = schema.variables.get(&entry.key) {
                let eff = resolve_effective(var_def, None);
                if let Some(ref default) = eff.default {
                    return Some(CodeAction {
                        title: format!("Use default value for {}", entry.key),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(
                                [(
                                    uri.clone(),
                                    vec![TextEdit {
                                        range: entry.value_range,
                                        new_text: default.clone(),
                                    }],
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
            }
        }
    }

    None
}

fn find_last_env_line(entries: &[EnvDocEntry]) -> u32 {
    entries
        .iter()
        .filter(|e| e.line_type == super::document::EnvLineType::EnvVar)
        .map(|e| e.line)
        .max()
        .unwrap_or(0)
}
