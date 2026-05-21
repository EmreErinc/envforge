pub mod ai_guard_diagnostics;
pub mod code_action;
pub mod code_lens;
pub mod commands;
pub mod completion;
pub mod definition;
pub mod diagnostics;
pub mod document;
pub mod document_symbol;
pub mod exposure;
pub mod folding_range;
pub mod format;
pub mod hover;
pub mod inlay;
pub mod mcp_diagnostics;
pub mod redact;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod server;
pub mod workspace_symbol;

pub fn run_lsp() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(server::serve());
}
