#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        // Fuzz the MCP credential scanner. Must never panic or OOM on
        // arbitrary JSON input (valid, invalid, deeply nested, oversized).
        let _ = envforge::ops::mcp_scan::scan_mcp_text(content, Path::new("fuzz_input"));
    }
});
