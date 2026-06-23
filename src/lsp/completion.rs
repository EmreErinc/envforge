use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Position, Range, TextEdit,
};

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::env_keyset::EnvKeySet;
use crate::ops::schema::{EnvSchema, VarType};

use super::document::EnvDocEntry;
use super::server::ManagedVar;

pub fn completions(
    position: Position,
    content: &str,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
    env_keyset: Option<&EnvKeySet>,
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
        return value_completions(key, entries, schema, managed_vars, env_keyset, value_range);
    }

    // $VAR reference
    if before_cursor.ends_with('$') || before_cursor.contains("${") {
        let ref_range = ref_replace_range(line, position);
        let typed_prefix = ref_typed_prefix(before_cursor);
        return reference_completions(entries, managed_vars, ref_range, &typed_prefix, schema);
    }

    // Key position
    let key_range = key_replace_range(line, before_cursor, position);
    key_completions(
        before_cursor,
        entries,
        schema,
        managed_vars,
        env_keyset,
        key_range,
    )
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
    env_keyset: Option<&EnvKeySet>,
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

    // 2. From the project key-set — keys declared in this project's other
    //    environments (FR11). Ranked right after schema (0_) and ABOVE
    //    globally-managed shell vars: in a project env file the project's own
    //    keys are the most relevant, and must not be buried under the user's
    //    whole shell environment. Runs before the managed loop so a key in both
    //    is presented as a project key.
    if let Some(ks) = env_keyset {
        for name in ks.key_names() {
            if existing_keys.contains(name) || !seen.insert(name.to_string()) {
                continue;
            }
            if !prefix_lower.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            let envs: Vec<&str> = ks
                .entry(name)
                .map(|e| e.environments().collect())
                .unwrap_or_default();
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(if envs.is_empty() {
                    "from project".to_string()
                } else {
                    format!("set in: {}", envs.join(", "))
                }),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: format!("{}=", name),
                })),
                sort_text: Some(format!("1_{}", name)),
                ..Default::default()
            });
        }
    }

    // 3. From envforge managed vars (global shell environment). Ranked last —
    //    least relevant inside a project env file.
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
            sort_text: Some(format!("2_{}", mv.key)),
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
    env_keyset: Option<&EnvKeySet>,
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
                            label: (*val).to_string(),
                            kind: Some(CompletionItemKind::VALUE),
                            text_edit: value_edit((*val).to_string()),
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
                    if var_def.sensitive {
                        if let Some(ref def) = var_def.default {
                            items.push(CompletionItem {
                                label: "(sensitive, use your secret)".into(),
                                kind: Some(CompletionItemKind::VALUE),
                                detail: Some("sensitive: do not use schema default".into()),
                                filter_text: Some(String::new()),
                                text_edit: None,
                                ..Default::default()
                            });
                            let _ = def;
                        }
                    } else {
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
    }

    // Suggest current-value placeholder from managed vars (no raw value leak).
    //
    // The label shows a redacted preview or a plain "(managed)" marker.
    // The text_edit inserts nothing — the user must use `reveal.value`
    // to obtain the real secret. This closes the fence bypass where raw
    // secret values appeared in completion popups cached by IDEs.
    for mv in managed_vars {
        if mv.key == key {
            let already = items
                .iter()
                .any(|i| i.detail.as_deref() == Some("managed by envforge"));
            if !already {
                items.push(CompletionItem {
                    label: "(managed by envforge)".into(),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("managed by envforge".into()),
                    filter_text: Some(String::new()),
                    text_edit: None,
                    sort_text: Some("0_current".into()),
                    ..Default::default()
                });
            }
            break;
        }
    }

    // Cross-environment values (FR12): values this key holds in other
    // environments. Sensitive keys never surface a raw cross-env value — only
    // a safe marker (redaction parity, NFR4); non-sensitive keys offer the
    // real values for reuse.
    if let Some(ks) = env_keyset {
        if let Some(entry) = ks.entry(key) {
            if entry.is_sensitive() {
                let marker = "sensitive: set per environment";
                if !items.iter().any(|i| i.detail.as_deref() == Some(marker)) {
                    items.push(CompletionItem {
                        label: "(sensitive — set per environment)".into(),
                        kind: Some(CompletionItemKind::VALUE),
                        detail: Some(marker.into()),
                        filter_text: Some(String::new()),
                        text_edit: None,
                        sort_text: Some("0_xenv".into()),
                        ..Default::default()
                    });
                }
            } else {
                for val in ks.distinct_values(key) {
                    if items.iter().any(|i| i.label == val) {
                        continue;
                    }
                    items.push(CompletionItem {
                        label: val.to_string(),
                        kind: Some(CompletionItemKind::VALUE),
                        detail: Some("from other environments".into()),
                        text_edit: value_edit(val.to_string()),
                        sort_text: Some(format!("2_xenv_{}", val)),
                        ..Default::default()
                    });
                }
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
            filter_text: Some(new_text.clone()),
            kind: Some(CompletionItemKind::REFERENCE),
            detail: Some("variable reference".into()),
            text_edit: value_edit(new_text),
            sort_text: Some(format!("z_{}", entry.key)),
            ..Default::default()
        });
    }

    items
}

/// Extract the partial identifier the user has typed inside a `${...}`
/// reference, e.g. `URL=${AB` → `"AB"`, `$` → `""`, `${` → `""`.
/// Returned lowercase so we can do case-insensitive prefix matching
/// without re-allocating per managed var.
fn ref_typed_prefix(before_cursor: &str) -> String {
    let from = before_cursor
        .rfind("${")
        .map(|i| i + 2)
        .or_else(|| before_cursor.rfind('$').map(|i| i + 1))
        .unwrap_or(before_cursor.len());
    before_cursor[from..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

fn reference_completions(
    entries: &[EnvDocEntry],
    managed_vars: &[ManagedVar],
    replace_range: Range,
    typed_prefix: &str,
    schema: Option<&EnvSchema>,
) -> Vec<CompletionItem> {
    // Cap the number of managed-var refs we surface even after the
    // server-side prefix filter, so popups stay quick to read. Same-
    // file entries are never capped — they are the most relevant
    // suggestions. Lowered from 50 to 20: with prefix filtering in
    // place, anything beyond ~20 hits is a sign the user hasn't typed
    // enough to disambiguate yet and a wall of results actively hurts.
    const MAX_MANAGED_REF_SUGGESTIONS: usize = 20;

    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    let edit = |new_text: String| {
        Some(CompletionTextEdit::Edit(TextEdit {
            range: replace_range,
            new_text,
        }))
    };

    let matches_prefix = |key: &str| -> bool {
        if typed_prefix.is_empty() {
            return true;
        }
        key.to_lowercase().starts_with(typed_prefix)
    };

    // From current file — always surfaced when prefix matches; never
    // capped because same-file refs are the most relevant.
    for e in entries {
        if !matches_prefix(&e.key) {
            continue;
        }
        if seen.insert(e.key.clone()) {
            let new_text = format!("${{{}}}", e.key);
            items.push(CompletionItem {
                label: e.key.clone(),
                // `filter_text` must match what the user is typing
                // after `$`. If we leave it unset, editors compare the
                // typed prefix (`${`, `${ABC`) against the bare-key
                // label — no match → list empties. Setting
                // `filter_text = "${KEY}"` keeps the items visible as
                // the user types more of the reference.
                filter_text: Some(format!("${{{}}}", e.key)),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("this file".into()),
                text_edit: edit(new_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0_{}", e.key)),
                ..Default::default()
            });
        }
    }

    // From managed vars: prefix-filtered server-side, then capped.
    // Secret keys are excluded to prevent AI enumeration via $ prefix probing.
    let mut managed_added = 0usize;
    for mv in managed_vars {
        if managed_added >= MAX_MANAGED_REF_SUGGESTIONS {
            break;
        }
        if !matches_prefix(&mv.key) {
            continue;
        }
        let sensitive = schema
            .and_then(|s| s.variables.get(&mv.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || is_sensitive_key(&mv.key);
        if sensitive {
            continue;
        }
        if seen.insert(mv.key.clone()) {
            let new_text = format!("${{{}}}", mv.key);
            items.push(CompletionItem {
                label: mv.key.clone(),
                filter_text: Some(format!("${{{}}}", mv.key)),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("envforge".into()),
                text_edit: edit(new_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("1_{}", mv.key)),
                ..Default::default()
            });
            managed_added += 1;
        }
    }

    items
}
