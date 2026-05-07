#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        let _ = envforge::parser::parse_shell_content(content, Path::new("fuzz_input"));
    }
});
