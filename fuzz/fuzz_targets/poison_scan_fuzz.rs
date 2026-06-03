#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        // Fuzz the MCP poison injection scanner. Must never panic or
        // ReDoS (catastrophic backtracking) on arbitrary input.
        let _ = envforge::ops::mcp_poison::description_scan::DescriptionScanner::scan(
            "fuzz_input",
            content,
        );
    }
});
