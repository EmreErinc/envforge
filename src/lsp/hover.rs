use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::ops::schema::{EnvSchema, VarType};

use super::document::EnvDocEntry;

pub fn hover_info(
    position: Position,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
) -> Option<Hover> {
    // Find which entry the cursor is on
    let entry = entries.iter().find(|e| {
        e.line == position.line
            && position.character >= e.key_range.start.character
            && position.character <= e.value_range.end.character
    })?;

    let schema = schema?;
    let var_def = schema.variables.get(&entry.key)?;

    let mut lines = Vec::new();
    lines.push(format!("**{}**", entry.key));
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
            lines.push(format!("Values: {}", vals.iter().map(|v| format!("`{}`", v)).collect::<Vec<_>>().join(", ")));
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

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n"),
        }),
        range: Some(entry.key_range),
    })
}
