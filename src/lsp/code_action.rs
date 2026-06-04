use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::*;

use crate::ops::canary::list_canaries;
use crate::ops::schema::{resolve_effective, EnvSchema};

use super::document::{EnvDocEntry, EnvLineType};
use super::security::guard_workspace_containment;

#[allow(clippy::too_many_arguments)]
pub fn code_actions(
    uri: &Url,
    entries: &[EnvDocEntry],
    diagnostics: &[Diagnostic],
    schema: Option<&EnvSchema>,
    schema_uri: Option<&Url>,
    schema_line_count: Option<u32>,
    schema_lines: Option<&HashMap<String, u32>>,
    workspace_root: Option<&Path>,
) -> Option<CodeActionResponse> {
    let mut actions: Vec<CodeAction> = Vec::new();

    for diag in diagnostics {
        actions.extend(actions_from_diagnostic(
            uri,
            entries,
            diag,
            schema,
            schema_uri,
            schema_line_count,
            schema_lines,
            workspace_root,
        ));
    }

    // Aggregate actions: combine multiple missing-required diagnostics
    // into a single bulk-add when there are 2+ of them.
    if let Some(bulk) = bulk_add_missing(uri, entries, diagnostics, schema) {
        actions.push(bulk);
    }

    // Aggregate: when the doc has no env-var lines but the schema
    // declares some, offer to scaffold the whole file in one shot.
    if let Some(generate) = generate_from_schema(uri, entries, schema) {
        actions.push(generate);
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

#[allow(clippy::too_many_arguments)]
fn actions_from_diagnostic(
    uri: &Url,
    entries: &[EnvDocEntry],
    diag: &Diagnostic,
    schema: Option<&EnvSchema>,
    schema_uri: Option<&Url>,
    schema_line_count: Option<u32>,
    schema_lines: Option<&HashMap<String, u32>>,
    workspace_root: Option<&Path>,
) -> Vec<CodeAction> {
    let mut out = Vec::new();
    let msg = &diag.message;

    if msg.starts_with("Unknown key '") {
        if let Some(action) = action_add_to_schema(diag, schema_uri, schema_line_count) {
            out.push(action);
        }
        return out;
    }

    if msg.starts_with("Missing required variable:") {
        if let Some(action) = action_add_missing_single(uri, entries, diag, schema) {
            out.push(action);
        }
        return out;
    }

    if msg.contains("Sensitive value for '") {
        if let Some(key) = sensitive_key_from_message(msg) {
            if let Some(action) = action_use_secret_reference(uri, entries, diag, &key) {
                out.push(action);
            }
            if let Some(action) =
                action_mark_secret_in_schema(diag, &key, schema, schema_uri, schema_lines)
            {
                out.push(action);
            }
            if let Some(action) = action_plant_canary(uri, diag, &key, workspace_root) {
                out.push(action);
            }
        }
        return out;
    }

    if msg.contains("Type validation failed") || msg.contains("Invalid") {
        if let Some(action) = action_use_default(uri, entries, diag, schema) {
            out.push(action);
        }
    }

    out
}

fn action_add_to_schema(
    diag: &Diagnostic,
    schema_uri: Option<&Url>,
    schema_line_count: Option<u32>,
) -> Option<CodeAction> {
    let msg = &diag.message;
    let start = msg.find('\'').map(|i| i + 1)?;
    let end = msg[start..].find('\'').map(|i| start + i)?;
    if start >= end {
        return None;
    }
    let key = &msg[start..end];
    let schema_uri = schema_uri?;
    let insert_line = schema_line_count.unwrap_or(0);

    let block = format!("\n[{}]\ntype = \"string\"\n", key);

    Some(CodeAction {
        title: format!("Add {} to schema", key),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    schema_uri.clone(),
                    vec![TextEdit {
                        range: zero_range(insert_line),
                        new_text: block,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn action_add_missing_single(
    uri: &Url,
    entries: &[EnvDocEntry],
    diag: &Diagnostic,
    schema: Option<&EnvSchema>,
) -> Option<CodeAction> {
    let key = diag.message.strip_prefix("Missing required variable: ")?;
    let insert_text = build_env_line(key, schema);
    let line = find_last_env_line(entries);

    Some(CodeAction {
        title: format!("Add {}", key),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    uri.clone(),
                    vec![TextEdit {
                        range: zero_range(line + 1),
                        new_text: format!("{}\n", insert_text),
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn action_use_secret_reference(
    uri: &Url,
    entries: &[EnvDocEntry],
    diag: &Diagnostic,
    key: &str,
) -> Option<CodeAction> {
    let entry = entries.iter().find(|e| e.key == key)?;
    Some(CodeAction {
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
    })
}

/// Insert `sensitive = true` into the schema's `[KEY]` block. We place
/// it on the line immediately after the header, which TOML semantics
/// guarantee is still inside the same table. The user can move it
/// afterwards if they prefer a different position.
fn action_mark_secret_in_schema(
    diag: &Diagnostic,
    key: &str,
    schema: Option<&EnvSchema>,
    schema_uri: Option<&Url>,
    schema_lines: Option<&HashMap<String, u32>>,
) -> Option<CodeAction> {
    let schema_uri = schema_uri?;
    let lines = schema_lines?;
    let header_line = *lines.get(key)?;

    // If the schema already marks it sensitive, the diagnostic shouldn't
    // be firing — but defend anyway so we don't double-write.
    if let Some(schema) = schema {
        if schema
            .variables
            .get(key)
            .map(|v| v.sensitive)
            .unwrap_or(false)
        {
            return None;
        }
    }

    Some(CodeAction {
        title: format!("Mark {} as secret in schema", key),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    schema_uri.clone(),
                    vec![TextEdit {
                        range: zero_range(header_line + 1),
                        new_text: "sensitive = true\n".to_string(),
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// Suggest planting a canary tripwire for the given sensitive key. Since
/// execute_command_provider is disabled (LSP is a read-only boundary),
/// this emits a WorkspaceEdit that appends a comment line with the CLI
/// command the user should run. The actual canary value must be generated
/// via the CLI to keep the payload off the LSP wire.
///
/// Suppressed when the key already has a registered canary.
fn action_plant_canary(
    uri: &Url,
    diag: &Diagnostic,
    key: &str,
    workspace_root: Option<&Path>,
) -> Option<CodeAction> {
    let already_planted = list_canaries()
        .map(|cs| cs.iter().any(|c| c.key == key))
        .unwrap_or(false);
    if already_planted {
        return None;
    }

    // Workspace containment: resolve the URI to a filesystem path and
    // verify it stays within the workspace root before any I/O.
    let file_path = uri.to_file_path().ok()?;
    if let Some(root) = workspace_root {
        let file_name = if let Ok(relative) = file_path.strip_prefix(root) {
            relative.to_string_lossy().to_string()
        } else {
            eprintln!(
                "LSP: code_action canary blocked — file outside workspace: {}",
                file_path.display()
            );
            return None;
        };
        if guard_workspace_containment(workspace_root, &file_name).is_err() {
            eprintln!(
                "LSP: code_action canary blocked — workspace containment failed: {}",
                file_path.display()
            );
            return None;
        }
    }

    let hint_line = format!(
        "\n# envforge: run `envforge canary plant --key {} --pattern {}` to plant a tripwire",
        key,
        canary_pattern_hint(key)
    );

    let last_line = if let Ok(doc_text) = std::fs::read_to_string(&file_path) {
        doc_text.lines().count().max(1) as u32
    } else {
        0
    };

    Some(CodeAction {
        title: format!("Hint: plant canary tripwire for {}", key),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    uri.clone(),
                    vec![TextEdit {
                        range: zero_range(last_line),
                        new_text: hint_line,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// Map a key name to one of the patterns understood by
/// `ops::canary::generate_fake_value`. Defaults to `generic` when no
/// strong hint is available; over-classification is fine, the fake
/// value is correctly shaped either way.
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

fn action_use_default(
    uri: &Url,
    entries: &[EnvDocEntry],
    diag: &Diagnostic,
    schema: Option<&EnvSchema>,
) -> Option<CodeAction> {
    let line = diag.range.start.line;
    let entry = entries.iter().find(|e| e.line == line)?;
    let schema = schema?;
    let var_def = schema.variables.get(&entry.key)?;
    let eff = resolve_effective(var_def, None);
    let default = eff.default.as_ref()?;

    Some(CodeAction {
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
    })
}

/// When 2+ missing-required diagnostics are present, offer one
/// combined fix that appends every missing key in a single edit.
fn bulk_add_missing(
    uri: &Url,
    entries: &[EnvDocEntry],
    diagnostics: &[Diagnostic],
    schema: Option<&EnvSchema>,
) -> Option<CodeAction> {
    let missing: Vec<&str> = diagnostics
        .iter()
        .filter_map(|d| d.message.strip_prefix("Missing required variable: "))
        .collect();
    if missing.len() < 2 {
        return None;
    }

    let line = find_last_env_line(entries);
    let mut combined = String::new();
    for k in &missing {
        combined.push_str(&build_env_line(k, schema));
        combined.push('\n');
    }

    Some(CodeAction {
        title: format!("Add all missing keys ({})", missing.len()),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(
            diagnostics
                .iter()
                .filter(|d| d.message.starts_with("Missing required variable:"))
                .cloned()
                .collect(),
        ),
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    uri.clone(),
                    vec![TextEdit {
                        range: zero_range(line + 1),
                        new_text: combined,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// When the env doc has no `EnvVar` lines but the schema declares some,
/// offer to scaffold the entire file from the schema. Each declared key
/// is rendered with its default (or example, or empty value).
fn generate_from_schema(
    uri: &Url,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
) -> Option<CodeAction> {
    let schema = schema?;
    if schema.variables.is_empty() {
        return None;
    }
    let has_env_lines = entries.iter().any(|e| e.line_type == EnvLineType::EnvVar);
    if has_env_lines {
        return None;
    }

    let mut keys: Vec<&String> = schema.variables.keys().collect();
    keys.sort();
    let mut body = String::new();
    for k in &keys {
        body.push_str(&build_env_line(k, Some(schema)));
        body.push('\n');
    }

    let last_line = entries.iter().map(|e| e.line).max().unwrap_or(0);
    let insert_line = if entries.is_empty() { 0 } else { last_line + 1 };

    Some(CodeAction {
        title: format!("Generate .env from schema ({} keys)", keys.len()),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(
                [(
                    uri.clone(),
                    vec![TextEdit {
                        range: zero_range(insert_line),
                        new_text: body,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn build_env_line(key: &str, schema: Option<&EnvSchema>) -> String {
    let value = schema
        .and_then(|s| s.variables.get(key))
        .and_then(|v| v.default.clone().or_else(|| v.example.clone()))
        .unwrap_or_default();
    format!("{}={}", key, value)
}

fn sensitive_key_from_message(msg: &str) -> Option<String> {
    let start = msg.find('\'').map(|i| i + 1)?;
    let end = msg.rfind('\'')?;
    if start >= end {
        return None;
    }
    Some(msg[start..end].to_string())
}

fn zero_range(line: u32) -> Range {
    Range {
        start: Position { line, character: 0 },
        end: Position { line, character: 0 },
    }
}

fn find_last_env_line(entries: &[EnvDocEntry]) -> u32 {
    entries
        .iter()
        .filter(|e| e.line_type == EnvLineType::EnvVar)
        .map(|e| e.line)
        .max()
        .unwrap_or(0)
}
