use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range as LspRange};

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::{resolve_effective, validate_value, EnvSchema};

use super::document::EnvDocEntry;

pub fn compute_diagnostics(entries: &[EnvDocEntry], schema: Option<&EnvSchema>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if let Some(schema) = schema {
        let present_keys: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.key.as_str()).collect();

        // Missing required variables
        for (var_name, var_def) in &schema.variables {
            let eff = resolve_effective(var_def, None);
            if eff.required && eff.default.is_none() && !present_keys.contains(var_name.as_str()) {
                diags.push(Diagnostic {
                    range: LspRange {
                        start: Position { line: 0, character: 0 },
                        end: Position { line: 0, character: 0 },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("envforge".into()),
                    message: format!("Missing required variable: {}", var_name),
                    ..Default::default()
                });
            }
        }

        // Type validation
        for entry in entries {
            if let Some(var_def) = schema.variables.get(&entry.key) {
                let eff = resolve_effective(var_def, None);
                if let Some(err) = validate_value(&entry.key, &entry.value, &eff) {
                    diags.push(Diagnostic {
                        range: entry.value_range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some("envforge".into()),
                        message: err.message,
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Secret leak warnings
    for entry in entries {
        let is_sensitive = schema
            .and_then(|s| s.variables.get(&entry.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || is_sensitive_key(&entry.key);

        if is_sensitive && !entry.value.is_empty() && looks_like_real_secret(&entry.value) {
            diags.push(Diagnostic {
                range: entry.value_range,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("envforge".into()),
                message: format!("Sensitive value for '{}' — consider using a secret reference", entry.key),
                ..Default::default()
            });
        }
    }

    diags
}

fn looks_like_real_secret(value: &str) -> bool {
    // Not a placeholder or variable reference
    !value.starts_with("${")
        && !value.starts_with("$")
        && !value.starts_with("<")
        && !value.starts_with("ref:")
        && !value.contains("TODO")
        && !value.contains("CHANGEME")
        && value.len() > 3
}
