use std::path::Path;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::ops::mcp_scan::{scan_mcp_text, McpFinding};

/// Compute LSP diagnostics for an MCP config document. Reuses the
/// production credential-detection rules from `ops::mcp_scan`, then
/// locates each finding's value substring in the source text to
/// attach a precise editor range. Findings whose value cannot be
/// located fall back to a line-zero range so the warning is still
/// surfaced.
pub fn compute_mcp_diagnostics(content: &str, virtual_path: &Path) -> Vec<Diagnostic> {
    let findings = scan_mcp_text(content, virtual_path);
    findings
        .into_iter()
        .map(|f| diagnostic_for_finding(content, f))
        .collect()
}

fn diagnostic_for_finding(content: &str, finding: McpFinding) -> Diagnostic {
    let range = locate_value(content, &finding).unwrap_or(Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 0,
        },
    });

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("envforge-mcp".into()),
        message: format!(
            "Hardcoded credential in MCP config: {} at `{}` (value `{}`). \
             Replace with `${{ENV_VAR}}` and load via envforge.",
            finding.pattern, finding.path, finding.value_preview,
        ),
        ..Default::default()
    }
}

/// Best-effort locator: walk lines, find the first occurrence of the
/// masked-or-raw value preview. We need the actual raw value to produce
/// an accurate range, but findings carry only a masked preview. We
/// approximate by scanning for the leaf key followed by `:` and capturing
/// the JSON string literal that follows. Falls back to `None` if no
/// reasonable match is found.
fn locate_value(content: &str, finding: &McpFinding) -> Option<Range> {
    let key = strip_array_suffix(&finding.key);

    for (idx, line) in content.lines().enumerate() {
        let needle = format!("\"{}\"", key);
        let Some(key_col) = line.find(&needle) else {
            continue;
        };
        let after = &line[key_col + needle.len()..];
        let Some(colon_off) = after.find(':') else {
            continue;
        };
        let after_colon = &after[colon_off + 1..];
        let Some(quote_off) = after_colon.find('"') else {
            continue;
        };
        let value_start = key_col + needle.len() + colon_off + 1 + quote_off;
        let value_inner_start = value_start + 1;

        let remainder = &line[value_inner_start..];
        let Some(end_off) = remainder.find('"') else {
            continue;
        };
        let value_inner_end = value_inner_start + end_off;

        return Some(Range {
            start: Position {
                line: idx as u32,
                character: value_inner_start as u32,
            },
            end: Position {
                line: idx as u32,
                character: value_inner_end as u32,
            },
        });
    }
    None
}

/// Trim `[N]` array-index suffix from a finding's leaf key so we can
/// search for the JSON key as-written (e.g. `args[2]` → `args`).
fn strip_array_suffix(key: &str) -> &str {
    match key.find('[') {
        Some(idx) => &key[..idx],
        None => key,
    }
}
