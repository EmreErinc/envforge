/// Render a non-revealing preview of a secret value suitable for use in
/// LSP responses that may surface in popups, hover cards, or completion
/// history (VS Code, JetBrains, Neovim all persist some of these). The
/// real value must flow through a separate `insert_text` / dedicated
/// channel so the user can still accept or copy it intentionally.
///
/// Always returns `***` regardless of sensitivity flag. The LSP is a
/// read-only security boundary — any value preview, even a partial
/// prefix, constitutes metadata leakage that could enable targeted
/// exfiltration (e.g. `gh` prefix reveals GitHub token type, `sk` reveals
/// Stripe key). Full redaction closes this vector.
pub fn redact_for_label(_value: &str, _sensitive: bool) -> String {
    "***".to_string()
}

/// Redact all known secret values from an arbitrary LSP message string.
///
/// Called from message handlers before returning user-visible content.
/// Secrets shorter than 8 characters are skipped (too many false positives).
/// Secrets are sorted by length descending so longer secrets are redacted
/// before their shorter prefixes (e.g. `sk-proj-long-key` before `sk-proj`).
///
/// This is defense-in-depth — the primary protection is that secrets
/// should not be in LSP scope at all (fence handles that).  This utility
/// catches the cases where they leak through anyway.
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
