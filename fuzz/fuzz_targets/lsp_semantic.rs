#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        let _ = envforge::lsp::document::parse_env_document(content);
        let _ = envforge::lsp::semantic_tokens::compute_semantic_tokens(
            &envforge::lsp::document::parse_env_document(content),
            None,
        );
    }
});
