mod completion;
mod definition;
mod diagnostics;
mod document;
mod hover;
mod server;

pub fn run_lsp() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(server::serve());
}
