use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::ops::env_keyset::EnvKeySet;
use crate::ops::schema::{EnvSchema, VarType};

use super::document::EnvDocEntry;
use super::redact::redact_for_label;
use super::server::ManagedVar;

pub fn hover_info(
    position: Position,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    managed_vars: &[ManagedVar],
    env_keyset: Option<&EnvKeySet>,
) -> Option<Hover> {
    let entry = entries.iter().find(|e| {
        e.line == position.line
            && position.character >= e.key_range.start.character
            && position.character <= e.value_range.end.character
    })?;

    let schema_var = schema.and_then(|s| s.variables.get(&entry.key));
    let managed_match = managed_vars.iter().find(|m| m.key == entry.key);
    let keyset_entry = env_keyset.and_then(|ks| ks.entry(&entry.key));

    if schema_var.is_none() && managed_match.is_none() && keyset_entry.is_none() {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("**{}**", entry.key));

    if let Some(var_def) = schema_var {
        lines.push(format!("Type: `{}`", var_def.var_type.display()));
        if var_def.required {
            lines.push("Required: **yes**".into());
        }
        if var_def.sensitive {
            lines.push("Sensitive: **yes**".into());
        }
        if let Some(ref desc) = var_def.description {
            lines.push(String::new());
            lines.push(desc.clone());
        }
        if let Some(ref def) = var_def.default {
            lines.push(format!("Default: `{}`", def));
        }
        if let Some(ref ex) = var_def.example {
            lines.push(format!("Example: `{}`", ex));
        }
        if let Some(ref pattern) = var_def.pattern {
            lines.push(format!("Pattern: `{}`", pattern));
        }
        if let Some(ref vals) = var_def.values {
            if var_def.var_type == VarType::Enum {
                lines.push(format!(
                    "Values: {}",
                    vals.iter()
                        .map(|v| format!("`{}`", v))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        if let Some(min) = var_def.min {
            lines.push(format!("Min: `{}`", min));
        }
        if let Some(max) = var_def.max {
            lines.push(format!("Max: `{}`", max));
        }
        if !var_def.env_overrides.is_empty() {
            lines.push(String::new());
            lines.push("**Environment overrides:**".into());
            for env in var_def.env_overrides.keys() {
                lines.push(format!("- `{}`", env));
            }
        }
    }

    lines.push(String::new());
    lines.push("---".into());
    lines.push("**Provenance**".into());

    let defined_by = match (schema_var.is_some(), managed_match.is_some()) {
        (true, true) => "schema + local",
        (true, false) => "schema",
        (false, true) => "local (managed by envforge)",
        // Reached only via the project key-set (key defined in other envs).
        (false, false) => "project environments",
    };
    lines.push(format!("- Defined by: `{}`", defined_by));

    let entry_value = &entry.value;
    let is_sensitive = schema_var.map(|v| v.sensitive).unwrap_or(false);
    let current_value_line = match managed_match {
        Some(_mv) if entry_value.is_empty() => "- Current value: `not set`".to_string(),
        Some(_mv) if is_sensitive => "- Current value: `(sensitive)`".to_string(),
        Some(_mv) => {
            format!(
                "- Current value: `{}` (redacted)",
                redact_for_label(entry_value, is_sensitive)
            )
        }
        None => "- Current value: `not managed`".to_string(),
    };
    lines.push(current_value_line);

    if let Some(mv) = managed_match {
        if !mv.source_file.is_empty() {
            let fname = mv.source_file.rsplit('/').next().unwrap_or(&mv.source_file);
            lines.push(format!("- Source file: `{}`", fname));
        }
    }

    // Per-environment presence (FR15): which environments set this key. Raw
    // values are NOT shown — the LSP is a read-only security boundary that
    // never emits values in display surfaces (see `redact_for_label`); only
    // presence and sensitivity are surfaced here.
    if let Some(ke) = keyset_entry {
        lines.push(String::new());
        lines.push("**Set in environments:**".into());
        for (env, occ) in &ke.values {
            let note = if occ.sensitive { " (sensitive)" } else { "" };
            lines.push(format!("- `{}`{}", env, note));
        }
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n"),
        }),
        range: Some(entry.key_range),
    })
}
