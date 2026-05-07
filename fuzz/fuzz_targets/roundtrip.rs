#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        if let Ok(shell_file) = envforge::parser::parse_shell_content(content, Path::new("fuzz_input")) {
            let serialized = envforge::parser::serialize_shell_file(&shell_file);
            // Reparse the serialized output and verify it matches
            if let Ok(reparsed) = envforge::parser::parse_shell_content(&serialized, Path::new("fuzz_input")) {
                let reserialized = envforge::parser::serialize_shell_file(&reparsed);
                // After first round-trip, second parse+serialize should be idempotent
                assert_eq!(serialized, reserialized, "round-trip not idempotent");
            }
        }
    }
});
