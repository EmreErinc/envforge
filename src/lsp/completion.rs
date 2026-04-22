use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
    Position,
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
    if before_cursor.contains('=') {
        let key = before_cursor.split('=').next().unwrap_or("").trim();
        let key = key.strip_prefix("export ").unwrap_or(key);
        return value_completions(key, entries, schema, managed_vars);
    }

    // $VAR reference
    if before_cursor.ends_with('$') || before_cursor.contains("${") {
        return reference_completions(entries, managed_vars);
    }

    // Key position
    key_completions(before_cursor, entries, schema, managed_vars)
}

fn key_completions(
    prefix: &str,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
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
                insert_text: Some(insert),
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
            insert_text: Some(format!("{}=", mv.key)),
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
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    // Schema-based value completions
    if let Some(schema) = schema {
        if let Some(var_def) = schema.variables.get(key) {
            match var_def.var_type {
                VarType::Bool => {
                    for val in &["true", "false"] {
                        items.push(CompletionItem {
                            label: val.to_string(),
                            kind: Some(CompletionItemKind::VALUE),
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
                            ..Default::default()
                        });
                    }
                    if let Some(ref ex) = var_def.example {
                        if var_def.default.as_deref() != Some(ex.as_str()) {
                            items.push(CompletionItem {
                                label: ex.clone(),
                                kind: Some(CompletionItemKind::VALUE),
                                detail: Some("example".into()),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    }

    // Suggest current value from managed vars
    for mv in managed_vars {
        if mv.key == key && !mv.value.is_empty() {
            let already = items.iter().any(|i| i.label == mv.value);
            if !already {
                items.push(CompletionItem {
                    label: mv.value.clone(),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("current value".into()),
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
        items.push(CompletionItem {
            label: format!("${{{}}}", entry.key),
            kind: Some(CompletionItemKind::REFERENCE),
            detail: Some("variable reference".into()),
            sort_text: Some(format!("z_{}", entry.key)),
            ..Default::default()
        });
    }

    items
}

fn reference_completions(
    entries: &[EnvDocEntry],
    managed_vars: &[ManagedVar],
) -> Vec<CompletionItem> {
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // From current file
    for e in entries {
        if seen.insert(e.key.clone()) {
            items.push(CompletionItem {
                label: e.key.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("this file".into()),
                insert_text: Some(format!("{{{}}}", e.key)),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0_{}", e.key)),
                ..Default::default()
            });
        }
    }

    // From managed vars
    for mv in managed_vars {
        if seen.insert(mv.key.clone()) {
            items.push(CompletionItem {
                label: mv.key.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("envforge".into()),
                insert_text: Some(format!("{{{}}}", mv.key)),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("1_{}", mv.key)),
                ..Default::default()
            });
        }
    }

    items
}
