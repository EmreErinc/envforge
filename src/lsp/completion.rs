use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Position, Range, TextEdit,
};

use crate::ops::schema::{EnvSchema, VarType};

use super::document::EnvDocEntry;
use super::server::ManagedVar;

pub fn completions(
    position: Position,
    content: &str,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
) -> Vec<CompletionItem> {
    let line = content.lines().nth(position.line as usize).unwrap_or("");
    let col = position.character as usize;
    let before_cursor = if col <= line.len() {
        &line[..col]
    } else {
        line
    };

    // After '=' — value completions
    if let Some(eq_idx) = before_cursor.find('=') {
        let key = before_cursor[..eq_idx].trim();
        let key = key.strip_prefix("export ").unwrap_or(key);
        let value_start = Position {
            line: position.line,
            character: (eq_idx + 1) as u32,
        };
        let value_end = Position {
            line: position.line,
            character: line.len() as u32,
        };
        let value_range = Range {
            start: value_start,
            end: value_end,
        };
        return value_completions(key, entries, schema, managed_vars, value_range);
    }

    // $VAR reference
    if before_cursor.ends_with('$') || before_cursor.contains("${") {
        let ref_range = ref_replace_range(line, position);
        return reference_completions(entries, managed_vars, ref_range);
    }

    // Key position
    let key_range = key_replace_range(line, before_cursor, position);
    key_completions(before_cursor, entries, schema, managed_vars, key_range)
}

/// Compute the LSP range covering the partial KEY identifier currently being
/// typed: from the start of the current word (right after the last separator)
/// up to the cursor. If the line is empty, returns a zero-width range at
/// cursor. Sent as `text_edit.range` so editor clients (lsp4ij, VSCode)
/// don't wipe surrounding text on accept.
fn key_replace_range(line: &str, before_cursor: &str, position: Position) -> Range {
    let trimmed = before_cursor
        .strip_prefix("export ")
        .unwrap_or(before_cursor);
    let prefix_len = trimmed
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .count();
    let start_char = before_cursor.len().saturating_sub(prefix_len);
    Range {
        start: Position {
            line: position.line,
            character: start_char as u32,
        },
        end: Position {
            line: position.line,
            character: line.len().max(position.character as usize) as u32,
        },
    }
}

/// Range covering the in-progress `$VAR` / `${VAR}` reference. Replaces from
/// the `$` up to end-of-line so accept doesn't leave trailing partial text.
fn ref_replace_range(line: &str, position: Position) -> Range {
    let col = position.character as usize;
    let before = if col <= line.len() {
        &line[..col]
    } else {
        line
    };
    // Walk back to the last `$` on this line.
    let start = before.rfind('$').unwrap_or(col);
    Range {
        start: Position {
            line: position.line,
            character: start as u32,
        },
        end: Position {
            line: position.line,
            character: line.len() as u32,
        },
    }
}

fn key_completions(
    prefix: &str,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
    replace_range: Range,
) -> Vec<CompletionItem> {
    let existing_keys: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.key.as_str()).collect();
    let prefix_lower = prefix.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // 1. From schema (highest priority)
    if let Some(schema) = schema {
        for (name, var_def) in &schema.variables {
            if existing_keys.contains(name.as_str()) || !seen.insert(name.clone()) {
                continue;
            }
            if !prefix_lower.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }

            let detail = format!(
                "{} {}",
                var_def.var_type.display(),
                if var_def.required { "(required)" } else { "" }
            );

            let insert = if let Some(ref def) = var_def.default {
                format!("{}={}", name, def)
            } else if let Some(ref ex) = var_def.example {
                format!("{}={}", name, ex)
            } else {
                format!("{}=", name)
            };

            let doc = var_def.description.as_ref().map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.clone(),
                })
            });

            items.push(CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(detail.trim().to_string()),
                documentation: doc,
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: insert,
                })),
                sort_text: Some(format!("0_{}", name)),
                ..Default::default()
            });
        }
    }

    // 2. From envforge managed vars
    for mv in managed_vars {
        if existing_keys.contains(mv.key.as_str()) || !seen.insert(mv.key.clone()) {
            continue;
        }
        if !prefix_lower.is_empty() && !mv.key.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }

        let source = if mv.source_file.is_empty() {
            String::new()
        } else {
            let fname = mv.source_file.rsplit('/').next().unwrap_or(&mv.source_file);
            format!("from {}", fname)
        };

        items.push(CompletionItem {
            label: mv.key.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(if source.is_empty() {
                "envforge".into()
            } else {
                source
            }),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: replace_range,
                new_text: format!("{}=", mv.key),
            })),
            sort_text: Some(format!("1_{}", mv.key)),
            ..Default::default()
        });
    }

    items.sort_by(|a, b| {
        a.sort_text
            .as_deref()
            .cmp(&b.sort_text.as_deref())
            .then(a.label.cmp(&b.label))
    });
    items
}

fn value_completions(
    key: &str,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
    value_range: Range,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let value_edit = |new_text: String| {
        Some(CompletionTextEdit::Edit(TextEdit {
            range: value_range,
            new_text,
        }))
    };

    // Schema-based value completions
    if let Some(schema) = schema {
        if let Some(var_def) = schema.variables.get(key) {
            match var_def.var_type {
                VarType::Bool => {
                    for val in &["true", "false"] {
                        items.push(CompletionItem {
                            label: val.to_string(),
                            kind: Some(CompletionItemKind::VALUE),
                            text_edit: value_edit(val.to_string()),
                            ..Default::default()
                        });
                    }
                }
                VarType::Enum => {
                    if let Some(ref vals) = var_def.values {
                        for val in vals {
                            items.push(CompletionItem {
                                label: val.clone(),
                                kind: Some(CompletionItemKind::ENUM_MEMBER),
                                text_edit: value_edit(val.clone()),
                                ..Default::default()
                            });
                        }
                    }
                }
                _ => {
                    if let Some(ref def) = var_def.default {
                        items.push(CompletionItem {
                            label: def.clone(),
                            kind: Some(CompletionItemKind::VALUE),
                            detail: Some("default".into()),
                            text_edit: value_edit(def.clone()),
                            ..Default::default()
                        });
                    }
                    if let Some(ref ex) = var_def.example {
                        if var_def.default.as_deref() != Some(ex.as_str()) {
                            items.push(CompletionItem {
                                label: ex.clone(),
                                kind: Some(CompletionItemKind::VALUE),
                                detail: Some("example".into()),
                                text_edit: value_edit(ex.clone()),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    }

    // Suggest current value from managed vars.
    //
    // The completion label is shown in the editor's completion popup AND
    // is often persisted in completion / suggestion history (VS Code,
    // Neovim, JetBrains). Putting the live secret value in `label`
    // turns the LSP into a side-channel for secret exposure (screen
    // recording, pair programming, history files). Use a redacted
    // preview as the visible label and `insert_text` for the actual
    // value the user will get on accept.
    for mv in managed_vars {
        if mv.key == key && !mv.value.is_empty() {
            let preview = redact_value_for_label(&mv.value);
            let already = items
                .iter()
                .any(|i| i.detail.as_deref() == Some("current value") && i.label == preview);
            if !already {
                items.push(CompletionItem {
                    label: preview,
                    filter_text: Some(String::new()),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("current value".into()),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: value_range,
                        new_text: mv.value.clone(),
                    })),
                    sort_text: Some("0_current".into()),
                    ..Default::default()
                });
            }
        }
    }

    // $VAR references
    for entry in entries {
        if entry.key == key {
            continue;
        }
        let new_text = format!("${{{}}}", entry.key);
        items.push(CompletionItem {
            label: new_text.clone(),
            kind: Some(CompletionItemKind::REFERENCE),
            detail: Some("variable reference".into()),
            text_edit: value_edit(new_text),
            sort_text: Some(format!("z_{}", entry.key)),
            ..Default::default()
        });
    }

    items
}

fn reference_completions(
    entries: &[EnvDocEntry],
    managed_vars: &[ManagedVar],
    replace_range: Range,
) -> Vec<CompletionItem> {
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    let edit = |new_text: String| {
        Some(CompletionTextEdit::Edit(TextEdit {
            range: replace_range,
            new_text,
        }))
    };

    // From current file
    for e in entries {
        if seen.insert(e.key.clone()) {
            let new_text = format!("${{{}}}", e.key);
            items.push(CompletionItem {
                label: e.key.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("this file".into()),
                text_edit: edit(new_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0_{}", e.key)),
                ..Default::default()
            });
        }
    }

    // From managed vars
    for mv in managed_vars {
        if seen.insert(mv.key.clone()) {
            let new_text = format!("${{{}}}", mv.key);
            items.push(CompletionItem {
                label: mv.key.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("envforge".into()),
                text_edit: edit(new_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("1_{}", mv.key)),
                ..Default::default()
            });
        }
    }

    items
}

/// Render a non-revealing preview of a secret value for use as an LSP
/// completion `label`. Editor clients display labels in popups and may
/// persist them in history; the real value still flows through
/// `insert_text` so accepting the suggestion works normally.
fn redact_value_for_label(value: &str) -> String {
    let len = value.chars().count();
    if len <= 4 {
        "***".to_string()
    } else {
        let head: String = value.chars().take(2).collect();
        format!("{head}***({len} chars)")
    }
}
