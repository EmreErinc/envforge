use tower_lsp::lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintLabelPart, Position, Range,
};

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::{EnvDocEntry, EnvLineType};
use super::redact::redact_for_label;

/// Inlay hints rendered after the value of each env-var line. Provides
/// at-a-glance feedback without requiring hover: schema type for empty
/// values, "(default)" marker when the literal value equals the schema
/// default, and a redacted preview of any unresolved `${REF}` substitution.
pub fn compute_inlay_hints(
    range: Range,
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
) -> Vec<InlayHint> {
    let mut hints = Vec::new();

    for entry in entries {
        if entry.line_type != EnvLineType::EnvVar {
            continue;
        }
        if entry.line < range.start.line || entry.line > range.end.line {
            continue;
        }

        let label = inlay_label_for(entry, schema, entries);
        let Some(label_text) = label else { continue };

        hints.push(InlayHint {
            position: Position {
                line: entry.value_range.end.line,
                character: entry.value_range.end.character,
            },
            label: InlayHintLabel::LabelParts(vec![InlayHintLabelPart {
                value: format!(" {}", label_text),
                tooltip: None,
                location: None,
                command: None,
            }]),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    hints
}

fn inlay_label_for(
    entry: &EnvDocEntry,
    schema: Option<&EnvSchema>,
    entries: &[EnvDocEntry],
) -> Option<String> {
    let var_def = schema.and_then(|s| s.variables.get(&entry.key));
    let sensitive = var_def.map(|v| v.sensitive).unwrap_or(false) || is_sensitive_key(&entry.key);

    if entry.value.is_empty() {
        if let Some(var_def) = var_def {
            return Some(format!("({})", var_def.var_type.display()));
        }
        return None;
    }

    if let Some(var_def) = var_def {
        if let Some(ref default) = var_def.default {
            if default == &entry.value {
                return Some("(default)".into());
            }
        }
    }

    if is_ref_expression(&entry.value) {
        let resolved = resolve_ref(&entry.value, entries);
        if let Some(value) = resolved {
            let preview = redact_for_label(&value, sensitive);
            return Some(format!("→ {}", preview));
        }
        return Some("→ ?".into());
    }

    if sensitive {
        return Some(format!("({})", redact_for_label(&entry.value, sensitive)));
    }

    None
}

fn is_ref_expression(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("${") && trimmed.ends_with('}') && trimmed.len() > 3
}

fn resolve_ref(value: &str, entries: &[EnvDocEntry]) -> Option<String> {
    let trimmed = value.trim();
    let inner = trimmed.strip_prefix("${")?.strip_suffix('}')?;
    let key = inner.split(':').next().unwrap_or(inner);
    entries
        .iter()
        .find(|e| e.key == key)
        .map(|e| e.value.clone())
}
