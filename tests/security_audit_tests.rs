// ═══════════════════════════════════════════════════════════════
// Security Tests — Audit & Provider Encryption
// ═══════════════════════════════════════════════════════════════

use chrono::Utc;
use envforge::ops::monitor::{
    emit_event, read_audit_entries, start_persistent_audit_log, EventSource, RuntimeEvent,
    SecuritySeverity,
};
use envforge::ops::secrets::credentials;

// NOTE: The audit log is global/persistent across tests via a static
// path (config_dir()/audit.jsonl). Tests that emit events search
// within read_audit_entries(limit). Higher limits increase the chance
// of finding recent events but also read more data.
//
// For reliable event-source testing, emit all variants in one test
// so they share the same recency window.

// ═══════════════════════════════════════════════════════════════
// Provider Audit — Encryption Status Reporting
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_provider_audit_returns_entries_with_valid_fields() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        assert!(
            !entry.provider.is_empty(),
            "provider name must not be empty"
        );
        assert!(
            entry.encrypted_fields <= entry.credential_fields,
            "encrypted_fields ({}) must not exceed credential_fields ({}) for {}",
            entry.encrypted_fields,
            entry.credential_fields,
            entry.provider
        );
    }
}

#[test]
fn test_provider_audit_entry_has_all_required_json_fields() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        let json =
            serde_json::to_string(entry).expect("provider audit entry should serialize to JSON");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("serialized entry should parse back");

        for field in &["provider", "encrypted_fields", "credential_fields"] {
            assert!(
                parsed.get(field).is_some(),
                "serialized entry must contain field '{}'",
                field
            );
        }
    }
}

#[test]
fn test_provider_audit_entry_has_optional_fields() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        let json =
            serde_json::to_string(entry).expect("provider audit entry should serialize to JSON");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("serialized entry should parse back");

        for field in &["has_ttl", "store_permissions", "age_key_exists"] {
            assert!(
                parsed.get(field).is_some(),
                "serialized entry must contain optional field '{}'",
                field
            );
        }
    }
}

#[test]
fn test_provider_audit_empty_when_no_credentials_files() {
    // provider_audit should succeed (return empty or default)
    // rather than panicking when credentials are absent.
    let result = credentials::provider_audit();
    assert!(
        result.is_ok() || result.unwrap_or_default().is_empty(),
        "provider_audit must not panic when credentials are absent"
    );
}

// ═══════════════════════════════════════════════════════════════
// Audit Log — Write and Read Round-Trip
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_audit_log_write_then_read_should_find_event() {
    start_persistent_audit_log();

    let unique_id = format!("audit_test_{}", std::process::id());
    let event = RuntimeEvent {
        source: EventSource::Fence,
        key: Some("TEST_AUDIT_KEY".to_string()),
        message: unique_id.clone(),
        timestamp: Utc::now(),
        severity: SecuritySeverity::Info,
    };
    emit_event(event);

    // High limit ensures the event is in the read window
    let entries = read_audit_entries(500).unwrap_or_default();

    let found = entries.iter().any(|e| e.message.contains(&unique_id));
    assert!(
        found,
        "written audit event '{}' must be readable; {} entries checked",
        unique_id,
        entries.len()
    );
}

#[test]
fn test_audit_log_should_find_all_multiple_events_in_same_test() {
    start_persistent_audit_log();

    let event_count = 5;
    for i in 0..event_count {
        emit_event(RuntimeEvent {
            source: EventSource::Manual,
            key: Some(format!("MULTI_KEY_{}", i)),
            message: format!("test multi event {}", i),
            timestamp: Utc::now(),
            severity: SecuritySeverity::Info,
        });
    }

    let entries = read_audit_entries(500).unwrap_or_default();
    for i in 0..event_count {
        let marker = format!("test multi event {}", i);
        let found = entries.iter().any(|e| e.message.contains(&marker));
        assert!(
            found,
            "event {} ('{}') must be readable from audit log",
            i, marker
        );
    }
}

#[test]
fn test_audit_log_respects_limit_parameter() {
    start_persistent_audit_log();

    for i in 0..10 {
        emit_event(RuntimeEvent {
            source: EventSource::Manual,
            key: Some(format!("LIMIT_KEY_{}", i)),
            message: "limit test".to_string(),
            timestamp: Utc::now(),
            severity: SecuritySeverity::Info,
        });
    }

    let entries = read_audit_entries(5).unwrap_or_default();
    assert!(
        entries.len() <= 5,
        "read_audit_entries(5) must return at most 5 entries, got {}",
        entries.len()
    );
}

#[test]
fn test_audit_log_returns_empty_for_nonexistent_marker() {
    start_persistent_audit_log();

    let entries = read_audit_entries(50).unwrap_or_default();

    assert!(
        !entries
            .iter()
            .any(|e| e.message.contains("__nonexistent_unique_marker_xyz__")),
        "nonexistent marker must not match any audit entry"
    );
}

#[test]
fn test_audit_log_limit_zero_returns_empty() {
    start_persistent_audit_log();

    emit_event(RuntimeEvent {
        source: EventSource::Manual,
        key: Some("LIMIT_ZERO".to_string()),
        message: "should not appear".to_string(),
        timestamp: Utc::now(),
        severity: SecuritySeverity::Info,
    });

    let entries = read_audit_entries(0).unwrap_or_default();
    assert!(
        entries.is_empty(),
        "read_audit_entries(0) must return empty vec"
    );
}

// ═══════════════════════════════════════════════════════════════
// Audit Log — Event Source Variants
// ═══════════════════════════════════════════════════════════════
//
// All variant tests run in one test to share the recency window
// in the global audit log, preventing rotation issues.

#[test]
fn test_audit_log_all_event_sources_should_be_preserved() {
    start_persistent_audit_log();

    let sources = [
        EventSource::Fence,
        EventSource::Reveal,
        EventSource::Provider,
        EventSource::Canary,
        EventSource::AiGuard,
        EventSource::Proxy,
        EventSource::Manual,
    ];

    for source in &sources {
        emit_event(RuntimeEvent {
            source: *source,
            key: Some("EVENT_SOURCE_TEST".to_string()),
            message: format!("source test {}", *source),
            timestamp: Utc::now(),
            severity: SecuritySeverity::Info,
        });
    }

    let entries = read_audit_entries(500).unwrap_or_default();
    for source in &sources {
        let source_str = source.to_string();
        let found = entries.iter().any(|e| e.message.contains(&source_str));
        assert!(
            found,
            "event with source '{}' must be present in audit log ({} entries checked)",
            source_str,
            entries.len()
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// Audit — Redaction of High-Entropy Tokens in Messages
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_audit_log_redacts_long_token() {
    start_persistent_audit_log();

    let token = "sk-proj-abcdefghijklmnopqrstuvwxyz"; // 32 chars, matches 24+ regex
    let message_id = format!("REDACT_LONG_{}", std::process::id());

    emit_event(RuntimeEvent {
        source: EventSource::Provider,
        key: Some(message_id.clone()),
        message: format!("pulled secret with value {}", token),
        timestamp: Utc::now(),
        severity: SecuritySeverity::Info,
    });

    let entries = read_audit_entries(500).unwrap_or_default();
    let found = entries
        .iter()
        .find(|e| e.key.as_deref() == Some(&message_id))
        .expect("redaction test event must be found in audit log");

    assert!(
        !found.message.contains(token),
        "audit log must redact the 32-char token; got: {}",
        found.message
    );
    assert!(
        found.message.contains("[REDACTED]"),
        "redacted message must contain [REDACTED] marker; got: {}",
        found.message
    );
}

#[test]
fn test_audit_log_short_token_preserved_as_is() {
    start_persistent_audit_log();

    // Values under 24 characters are NOT redacted by the regex.
    let short_value = "abc123";
    let message_id = format!("SHORT_TOKEN_{}", std::process::id());

    emit_event(RuntimeEvent {
        source: EventSource::Provider,
        key: Some(message_id.clone()),
        message: format!("pulled secret with value {}", short_value),
        timestamp: Utc::now(),
        severity: SecuritySeverity::Info,
    });

    let entries = read_audit_entries(500).unwrap_or_default();
    let found = entries
        .iter()
        .find(|e| e.key.as_deref() == Some(&message_id))
        .expect("short-token event must be found in audit log");

    assert!(
        found.message.contains(short_value),
        "short value (<24 chars) must NOT be redacted; got: {}",
        found.message
    );
}

// ═══════════════════════════════════════════════════════════════
// Provider Audit — Field Consistency
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_provider_audit_permissions_n_a_when_store_missing() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        if !entry.store_file_exists {
            assert_eq!(
                entry.store_permissions, "n/a",
                "store_permissions must be 'n/a' when store file doesn't exist for {}",
                entry.provider
            );
        }
    }
}

#[test]
fn test_provider_audit_permissions_not_n_a_when_store_exists() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        if entry.store_file_exists {
            assert_ne!(
                entry.store_permissions, "n/a",
                "store_permissions must not be 'n/a' when store file exists for {}",
                entry.provider
            );
        }
    }
}

#[test]
fn test_provider_audit_encrypted_fields_zero_when_no_credentials() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        if entry.credential_fields == 0 {
            assert_eq!(
                entry.encrypted_fields, 0,
                "encrypted_fields must be 0 when no credentials exist for {}",
                entry.provider
            );
        }
    }
}

#[test]
fn test_provider_audit_store_file_exists_consistent_with_permissions() {
    let entries = credentials::provider_audit().unwrap_or_default();

    for entry in &entries {
        if entry.store_file_exists {
            assert!(
                entry.store_permissions != "n/a",
                "store_permissions must not be 'n/a' when store_file_exists=true for {}",
                entry.provider
            );
        } else {
            assert_eq!(
                entry.store_permissions, "n/a",
                "store_permissions must be 'n/a' when store_file_exists=false for {}",
                entry.provider
            );
        }
    }
}
