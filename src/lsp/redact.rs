/// Render a non-revealing preview of a secret value suitable for use in
/// LSP responses that may surface in popups, hover cards, or completion
/// history (VS Code, JetBrains, Neovim all persist some of these). The
/// real value must flow through a separate `insert_text` / dedicated
/// channel so the user can still accept or copy it intentionally.
pub fn redact_for_label(value: &str) -> String {
    let len = value.chars().count();
    if len <= 4 {
        "***".to_string()
    } else {
        let head: String = value.chars().take(2).collect();
        format!("{head}***({len} chars)")
    }
}
