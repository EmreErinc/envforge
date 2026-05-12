//! Tests for bolt 082-docs-tui.
//!
//! - Doc-sync: README + CHANGELOG mention this intent's deliverables
//! - JSON schema-stability: `mcp scan --json` retains legacy fields AND
//!   gains the additive `mcp_pin_status` field

use envforge::ops::mcp_scan::findings_to_json;

// ─────────────────────────────────────────────────────────────────────────────
// Story 001: README + CHANGELOG doc-sync
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_readme_reports_current_test_count() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README exists");
    assert!(
        readme.contains("2073 tests passing"),
        "README test count drift detected — update README.md per feedback_docs_sync.md"
    );
}

#[test]
fn test_readme_mentions_mcp_supply_chain() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README exists");
    assert!(
        readme.contains("MCP supply-chain"),
        "README should mention MCP supply-chain capability"
    );
}

#[test]
fn test_changelog_unreleased_section_present() {
    let changelog = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"))
        .expect("CHANGELOG exists");
    assert!(
        changelog.contains("## [Unreleased]"),
        "CHANGELOG must have [Unreleased] section"
    );
    assert!(
        changelog.contains("MCP Supply-Chain Integrity"),
        "CHANGELOG should describe the MCP supply-chain capability"
    );
}

#[test]
fn test_changelog_documents_new_mcp_commands() {
    let changelog = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"))
        .expect("CHANGELOG exists");
    for cmd in &[
        "envforge mcp pin",
        "envforge mcp verify",
        "envforge mcp diff",
        "envforge mcp trust",
        "envforge mcp explain",
        "envforge mcp launch",
        "envforge doctor --fail-on mcp",
    ] {
        assert!(
            changelog.contains(cmd),
            "CHANGELOG missing user-facing command: {cmd}"
        );
    }
}

#[test]
fn test_changelog_documents_new_audit_events() {
    let changelog = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"))
        .expect("CHANGELOG exists");
    for ev in &[
        "McpPinned",
        "McpVerifyFailed",
        "McpReverifyOk",
        "McpReverifyFailed",
        "McpPoisonDetected",
        "McpFeedFlippedKnownBad",
        "McpUserTrustGranted",
        "McpUserTrustRevoked",
        "McpLaunchBlocked",
        "McpFeedStale",
    ] {
        assert!(
            changelog.contains(ev),
            "CHANGELOG missing audit event: {ev}"
        );
    }
}

#[test]
fn test_changelog_documents_new_dependencies() {
    let changelog = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"))
        .expect("CHANGELOG exists");
    for dep in &[
        "rustls",
        "webpki-roots",
        "x509-cert",
        "flate2",
        "unicode-normalization",
    ] {
        assert!(
            changelog.contains(dep),
            "CHANGELOG missing dependency entry: {dep}"
        );
    }
}

#[test]
fn test_changelog_omits_internal_terms() {
    let changelog = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"))
        .expect("CHANGELOG exists");
    // Slice the Unreleased section so we only test new content.
    let unreleased_start = changelog
        .find("## [Unreleased]")
        .expect("[Unreleased] header present");
    let unreleased_end = changelog[unreleased_start + 16..]
        .find("\n## [")
        .map(|i| unreleased_start + 16 + i)
        .unwrap_or(changelog.len());
    let section = &changelog[unreleased_start..unreleased_end];

    for forbidden in &["Bolt 0", "Intent 034", "Unit 00", "ADR-"] {
        assert!(
            !section.contains(forbidden),
            "Unreleased CHANGELOG section should not include internal term: '{forbidden}'"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 003: mcp scan --json schema-stability
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mcp_scan_json_retains_legacy_fields() {
    let json = findings_to_json(&[], 0);
    let obj = json.as_object().expect("top-level is object");
    assert!(obj.contains_key("files_scanned"));
    assert!(obj.contains_key("credentials_found"));
    assert!(obj.contains_key("findings"));
}

#[test]
fn test_mcp_scan_json_adds_mcp_pin_status_field() {
    let json = findings_to_json(&[], 0);
    let obj = json.as_object().expect("top-level is object");
    assert!(
        obj.contains_key("mcp_pin_status"),
        "mcp_pin_status field missing from JSON output"
    );
}

#[test]
fn test_mcp_scan_json_pin_status_shape_when_present() {
    let json = findings_to_json(&[], 0);
    let pin = &json["mcp_pin_status"];
    // Either null (no feed/no env) or a structured object.
    if pin.is_object() {
        let obj = pin.as_object().unwrap();
        assert!(
            obj.contains_key("lockfile_exists"),
            "lockfile_exists missing"
        );
        assert!(obj.contains_key("pinned_count"));
        assert!(obj.contains_key("known_bad_count"));
        assert!(obj.contains_key("feed_version"));
        assert!(obj.contains_key("feed_stale"));
        // lockfile_exists must be a bool
        assert!(obj["lockfile_exists"].is_boolean());
    }
}

#[test]
fn test_mcp_scan_json_legacy_consumers_can_parse() {
    // Simulate a legacy parser that only reads files_scanned + credentials_found + findings.
    let json = findings_to_json(&[], 0);
    let files_scanned = json["files_scanned"].as_u64().expect("u64");
    let credentials_found = json["credentials_found"].as_u64().expect("u64");
    let findings = json["findings"].as_array().expect("array");
    assert_eq!(files_scanned, 0);
    assert_eq!(credentials_found, 0);
    assert!(findings.is_empty());
}
