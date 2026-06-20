pub mod ai_guard_diagnostics;
pub mod code_action;
pub mod code_lens;
// `dispatch_command` is reused by the constrained, *named* custom LSP requests
// in `server.rs` (H4). It is NOT wired to the generic `workspace/executeCommand`
// handler, which remains permanently disabled — see `Backend::execute_command`.
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
pub mod rate_limit;
pub mod redact;
pub mod references;
pub mod rename;
pub mod security;
pub mod semantic_tokens;
pub mod server;
pub mod workspace_symbol;

pub fn run_lsp() {
    crate::ops::secure_memory::disable_core_dumps();
    crate::ops::monitor::start_persistent_audit_log();
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(server::serve());
}
