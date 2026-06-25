//! Shared redaction routine.
//!
//! Relocated from `lsp::redact` so every surface — CLI, LSP, and the MCP
//! server — funnels secret-bearing output through one choke point. The LSP
//! continues to expose these via `lsp::redact` (re-export) so existing call
//! sites are unchanged.

/// Render a non-revealing preview of a secret value suitable for any surface
/// that may persist or display it (LSP popups/hover/completion history, MCP
/// tool responses, CLI output). The real value, when legitimately needed,
/// must flow through a separate, intentional channel — never this label.
///
/// Always returns `***` regardless of the sensitivity flag. Any value preview,
/// even a partial prefix, is metadata leakage that could enable targeted
/// exfiltration (e.g. a `gh` prefix reveals a GitHub token type, `sk` reveals
/// a Stripe key). Full redaction closes that vector.
pub fn redact_for_label(_value: &str, _sensitive: bool) -> String {
    "***".to_string()
}

/// Redact all known secret values from an arbitrary message string.
///
/// Called before returning user-visible content. Secrets shorter than 8
/// characters are skipped (too many false positives). Secrets are sorted by
/// length descending so longer secrets are redacted before their shorter
/// prefixes (e.g. `sk-proj-long-key` before `sk-proj`).
///
/// Defense-in-depth — the primary protection is that secrets should not be in
/// scope at all (fence handles that); this catches leaks that slip through.
pub fn redact_secrets_in_message(msg: &str, secrets: &[String]) -> String {
    if secrets.is_empty() {
        return msg.to_string();
    }
    let mut sorted: Vec<&String> = secrets.iter().filter(|s| s.len() >= 8).collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.len())); // longest first
    let mut out = msg.to_string();
    for secret in &sorted {
        out = out.replace(secret.as_str(), "***");
    }
    out
}
