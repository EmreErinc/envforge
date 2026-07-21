use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, Position, Range, TextEdit, Url,
    WorkspaceEdit,
};

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
            "Hardcoded credential in MCP config: {} at `{}`. \
             Replace with `${{ENV_VAR}}` and load via envforge.",
            finding.pattern, finding.path,
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

/// Convert an arbitrary JSON key string to SCREAMING_SNAKE_CASE suitable
/// for use as an environment variable name.
///
/// Rules:
/// - Insert `_` before each uppercase letter that follows a lowercase letter
///   (camelCase → CAMEL_CASE).
/// - Replace any non-alphanumeric character with `_`.
/// - Collapse consecutive `_` into one.
/// - Trim leading/trailing `_`.
/// - Uppercase the whole string.
/// - Fall back to `ENV_VAR` if the result is empty.
///
/// # Examples
///
/// ```
/// # use envforge::lsp::mcp_diagnostics::key_to_env_var;
/// assert_eq!(key_to_env_var("apiKey"), "API_KEY");
/// assert_eq!(key_to_env_var("GITHUB_TOKEN"), "GITHUB_TOKEN");
/// assert_eq!(key_to_env_var("db-password"), "DB_PASSWORD");
/// ```
pub fn key_to_env_var(key: &str) -> String {
    // Insert underscore on camelCase transitions first.
    let mut expanded = String::with_capacity(key.len() + 8);
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            if let Some(prev) = expanded.chars().last() {
                if prev.is_ascii_lowercase() {
                    expanded.push('_');
                }
            }
        }
        expanded.push(c);
    }

    // Replace non-alphanumeric with `_`, uppercase everything.
    let replaced: String = expanded
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_ascii_uppercase();

    // Collapse consecutive underscores and trim.
    let mut result = String::with_capacity(replaced.len());
    let mut last_was_underscore = false;
    for c in replaced.chars() {
        if c == '_' {
            if !last_was_underscore {
                result.push('_');
            }
            last_was_underscore = true;
        } else {
            result.push(c);
            last_was_underscore = false;
        }
    }
    let result = result.trim_matches('_').to_string();

    if result.is_empty() {
        "ENV_VAR".to_string()
    } else {
        result
    }
}

/// Derive the JSON key name from a source line given a character offset
/// that points into the value portion.
///
/// Scans the prefix of the line (up to `value_char`) left-to-right for
/// `"key":` tokens and returns the key of the last such token found. This
/// naturally handles both simple lines (`"apiKey": "val"`) and nested-object
/// lines where multiple quoted tokens appear before the value.
fn derive_key_from_line(line: &str, value_char: u32) -> Option<String> {
    let prefix = &line[..usize::min(value_char as usize, line.len())];

    let mut last_key: Option<String> = None;
    let bytes = prefix.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Find opening quote of a potential key.
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        // Find closing quote.
        let key_start = i + 1;
        let Some(close_rel) = bytes[key_start..].iter().position(|&b| b == b'"') else {
            break;
        };
        let key_end = key_start + close_rel;
        let after_quote = key_end + 1; // index after closing quote

        // Skip whitespace; check for ':'.
        let rest = &bytes[after_quote..];
        let colon_pos = rest.iter().position(|&b| b == b':');
        if let Some(cp) = colon_pos {
            // Ensure only whitespace between closing quote and colon.
            if rest[..cp].iter().all(|&b| b == b' ' || b == b'\t') {
                let key = std::str::from_utf8(&bytes[key_start..key_end]).ok()?;
                if !key.is_empty() {
                    last_key = Some(key.to_string());
                }
                i = after_quote + cp + 1; // advance past the colon
                continue;
            }
        }
        i = key_end + 1;
    }

    last_key
}

/// Build LSP quick-fix `CodeAction`s for every `envforge-mcp` diagnostic.
///
/// For each diagnostic whose `source` is `Some("envforge-mcp")` the function
/// produces one `QUICKFIX` action that replaces the offending value with an
/// `${ENV_VAR}` reference. The variable name is derived from the JSON key on
/// the same line (SCREAMING_SNAKE_CASE). The diagnostic range covers the
/// *inner* text of the JSON string (without surrounding quotes), so the
/// replacement text is `${VAR}` (no extra quotes needed).
pub fn mcp_config_code_actions(
    uri: &Url,
    content: &str,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let lines: Vec<&str> = content.lines().collect();

    diagnostics
        .iter()
        .filter(|d| d.source.as_deref() == Some("envforge-mcp"))
        .map(|diag| {
            let line_idx = diag.range.start.line as usize;
            let line_text = lines.get(line_idx).copied().unwrap_or("");
            let var_name = derive_key_from_line(line_text, diag.range.start.character)
                .as_deref()
                .map(key_to_env_var)
                .unwrap_or_else(|| "ENV_VAR".to_string());

            // The diagnostic range covers only the inner text (no quotes).
            // Replace inner text with ${VAR} so the surrounding JSON quotes
            // remain, producing valid JSON: "apiKey": "${API_KEY}"
            let new_text = format!("${{{var_name}}}");

            let workspace_edit = WorkspaceEdit {
                changes: Some(
                    [(
                        uri.clone(),
                        vec![TextEdit {
                            range: diag.range,
                            new_text,
                        }],
                    )]
                    .into_iter()
                    .collect::<HashMap<_, _>>(),
                ),
                ..Default::default()
            };

            CodeAction {
                title: format!("Replace hardcoded credential with ${{{var_name}}}"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diag.clone()]),
                edit: Some(workspace_edit),
                is_preferred: Some(true),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // key_to_env_var derivation
    // -----------------------------------------------------------------------

    #[test]
    fn test_mcp_quickfix_var_name_derivation() {
        assert_eq!(key_to_env_var("apiKey"), "API_KEY");
        assert_eq!(key_to_env_var("GITHUB_TOKEN"), "GITHUB_TOKEN");
        assert_eq!(key_to_env_var("db-password"), "DB_PASSWORD");
        assert_eq!(key_to_env_var("mySecretKey"), "MY_SECRET_KEY");
        assert_eq!(key_to_env_var("access_token"), "ACCESS_TOKEN");
        assert_eq!(key_to_env_var(""), "ENV_VAR");
        assert_eq!(key_to_env_var("---"), "ENV_VAR");
    }

    // -----------------------------------------------------------------------
    // Quick-fix: non-mcp diagnostic source → no actions
    // -----------------------------------------------------------------------

    #[test]
    fn test_mcp_quickfix_none_for_non_mcp_diagnostic() {
        let uri = Url::parse("file:///home/user/.cursor/mcp.json").unwrap();
        let content = r#"{ "apiKey": "sk-live-abc123def456" }"#;
        let diag = Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 12,
                },
                end: Position {
                    line: 0,
                    character: 32,
                },
            },
            source: Some("some-other-linter".into()),
            message: "Some other warning".into(),
            ..Default::default()
        };
        let actions = mcp_config_code_actions(&uri, content, &[diag]);
        assert!(actions.is_empty());
    }

    // -----------------------------------------------------------------------
    // Quick-fix: full round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_mcp_quickfix_replaces_value_with_env_ref() {
        let path = std::path::Path::new(".mcp.json");
        let uri = Url::parse("file:///home/user/.mcp.json").unwrap();
        // Minimal MCP config with a hardcoded API key.
        let content = r#"{
  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["my-mcp-server"],
      "env": {
        "apiKey": "sk-live-abc123def456"
      }
    }
  }
}"#;
        // Compute diagnostics using the production scanner.
        let diags = compute_mcp_diagnostics(content, path);
        assert!(
            !diags.is_empty(),
            "expected at least one mcp diagnostic for the hardcoded key"
        );

        // All should carry the envforge-mcp source.
        for d in &diags {
            assert_eq!(d.source.as_deref(), Some("envforge-mcp"));
        }

        // Compute code actions.
        let actions = mcp_config_code_actions(&uri, content, &diags);
        assert_eq!(actions.len(), diags.len(), "one action per diagnostic");

        let action = &actions[0];

        // Kind must be QUICKFIX.
        assert_eq!(action.kind.as_ref(), Some(&CodeActionKind::QUICKFIX));

        // Title must mention the derived var name.
        assert!(
            action.title.contains("API_KEY"),
            "expected API_KEY in title, got: {}",
            action.title
        );
        assert_eq!(action.is_preferred, Some(true));

        // Extract the single TextEdit and verify new_text.
        let edit = action.edit.as_ref().expect("edit must be present");
        let changes = edit.changes.as_ref().expect("changes must be present");
        let text_edits = changes
            .get(&uri)
            .expect("changes must contain the document URI");
        assert_eq!(text_edits.len(), 1);
        let te = &text_edits[0];
        assert_eq!(te.new_text, "${API_KEY}");

        // Apply the edit manually: splice at the byte range.
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = te.range.start.line as usize;
        let line = lines[line_idx];
        let start = te.range.start.character as usize;
        let end = te.range.end.character as usize;
        let patched_line = format!("{}{}{}", &line[..start], te.new_text, &line[end..]);
        let patched: String = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == line_idx {
                    patched_line.as_str()
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Result must be valid JSON.
        assert!(
            serde_json::from_str::<serde_json::Value>(&patched).is_ok(),
            "patched content is not valid JSON:\n{patched}"
        );
        // Must contain the env-var reference, not the raw secret.
        assert!(
            patched.contains("${API_KEY}"),
            "patched content missing ${{API_KEY}}"
        );
        assert!(
            !patched.contains("sk-live-abc123def456"),
            "patched content still contains the raw secret"
        );
    }
}
