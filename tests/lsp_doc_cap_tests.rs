//! Regression test for L10 — `parse_env_document` caps the per-document entry
//! count so a malformed/hostile document can't exhaust memory.

#[test]
fn test_parse_env_document_caps_entry_count() {
    // 60k lines — above the 50k cap.
    let content = "A=1\n".repeat(60_000);
    let entries = envforge::lsp::document::parse_env_document(&content);
    assert!(
        entries.len() <= 50_000,
        "parse_env_document must cap line/entry count, got {}",
        entries.len()
    );
}

#[test]
fn test_parse_env_document_small_doc_unaffected() {
    let entries = envforge::lsp::document::parse_env_document("A=1\nB=2\n");
    assert!(entries.len() >= 2, "normal docs still parse fully");
}
