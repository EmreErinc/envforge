pub mod code_action;
pub mod code_lens;
mod completion;
mod definition;
mod diagnostics;
pub mod document;
pub mod document_symbol;
pub mod folding_range;
mod hover;
pub mod server;
pub mod workspace_symbol;

pub fn run_lsp() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(server::serve());
}
