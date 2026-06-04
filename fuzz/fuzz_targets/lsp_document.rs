#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        // Fuzz the LSP env-document parser. Must never panic or OOM on
        // arbitrary UTF-8 input regardless of line count or line length.
        let _ = envforge::lsp::document::parse_env_document(content);
    }
});
