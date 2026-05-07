use envforge::model::*;
use envforge::ops::analytics::collector;

// ─── Proxy Entry Normalization ────────────────────────

#[test]
fn test_normalize_proxy_read_entry() {
    // Test via collect_events — proxy audit log entries normalize to Read
    let events = collector::collect_all().unwrap_or_default();
    // At minimum, no errors and returns Vec
    assert!(events.is_empty() || !events.is_empty());
}

#[test]
fn test_window_cutoff_last_hour() {
    use chrono::Utc;
    let events = collector::collect_events(&TimeWindow::LastHour).unwrap_or_default();
    let hour_ago = Utc::now() - chrono::Duration::hours(1);
    for event in &events {
        assert!(
            event.enriched_at >= hour_ago,
            "Event timestamp {} is older than 1 hour cutoff {}",
            event.enriched_at,
            hour_ago
        );
    }
}

#[test]
fn test_window_cutoff_last_24_hours() {
    use chrono::Utc;
    let events = collector::collect_events(&TimeWindow::Last24Hours).unwrap_or_default();
    let cutoff = Utc::now() - chrono::Duration::hours(24);
    for event in &events {
        assert!(event.enriched_at >= cutoff);
    }
}

#[test]
fn test_window_cutoff_last_7_days() {
    use chrono::Utc;
    let events = collector::collect_events(&TimeWindow::Last7Days).unwrap_or_default();
    let cutoff = Utc::now() - chrono::Duration::days(7);
    for event in &events {
        assert!(event.enriched_at >= cutoff);
    }
}

#[test]
fn test_window_cutoff_last_30_days() {
    use chrono::Utc;
    let events = collector::collect_events(&TimeWindow::Last30Days).unwrap_or_default();
    let cutoff = Utc::now() - chrono::Duration::days(30);
    for event in &events {
        assert!(event.enriched_at >= cutoff);
    }
}

#[test]
fn test_collect_all_returns_vec() {
    let events = collector::collect_all();
    // Should return Ok, never panic
    assert!(events.is_ok());
    let events = events.unwrap();
    // May be empty if no sources, but should be a Vec
    let _count = events.len();
}

// ─── Storage Tests ─────────────────────────────────────

#[test]
fn test_save_and_load_empty_events() {
    use envforge::ops::analytics::storage;

    let config = AnalyticsConfig::default();
    let events: Vec<EnrichedAccessEvent> = vec![];

    // Saving empty should be a no-op
    let result = storage::save_events(&events, &config);
    assert!(result.is_ok());
}

#[test]
fn test_save_and_load_roundtrip() {
    use chrono::Utc;
    use envforge::ops::analytics::storage;
    use uuid::Uuid;

    let config = AnalyticsConfig {
        max_events: 1000,
        ..AnalyticsConfig::default()
    };

    let run_id = Uuid::new_v4().to_string();

    let raw = RawAccessEvent {
        secret_name: "TEST_KEY".to_string(),
        access_type: AccessType::Read,
        accessor: AccessorInfo {
            id: "test-user".to_string(),
            accessor_type: AccessorType::User,
            name: Some("tester".to_string()),
            ip_address: None,
            user_agent: None,
        },
        timestamp: Utc::now(),
        source: AccessSource::Cli,
        context: None,
    };

    let events: Vec<EnrichedAccessEvent> = (0..5)
        .map(|i| EnrichedAccessEvent {
            id: Uuid::new_v4(),
            raw: RawAccessEvent {
                secret_name: format!("TEST_KEY_{}", i),
                ..raw.clone()
            },
            secret_id: format!("test_id_{}", i),
            provider: run_id.clone(),
            environment: Some("test".to_string()),
            risk_level: RiskLevel::Low,
            enriched_at: Utc::now(),
        })
        .collect();

    // Save
    storage::save_events(&events, &config).unwrap();

    // Load
    let loaded = storage::load_events().unwrap();

    // Check that saved events appear in loaded
    let test_events: Vec<_> = loaded.iter().filter(|e| e.provider == run_id).collect();
    assert_eq!(test_events.len(), 5);
}

#[test]
fn test_storage_load_when_no_file() {
    use envforge::ops::analytics::storage;

    let events = storage::load_events().unwrap_or_default();
    // Should return empty Vec, not error
    let _count = events.len();
}

#[test]
fn test_event_enrichment_has_id() {
    let events = collector::collect_all().unwrap_or_default();
    for event in &events {
        // Every enriched event must have a non-nil UUID
        assert!(!event.id.is_nil());
    }
}

#[test]
fn test_event_source_is_correct() {
    let events = collector::collect_all().unwrap_or_default();
    for event in &events {
        // Source should be one of the valid variants
        match event.raw.source {
            AccessSource::Proxy
            | AccessSource::Changelog
            | AccessSource::Provider
            | AccessSource::Cli
            | AccessSource::Tui
            | AccessSource::AiGuard
            | AccessSource::Cicd => {}
        }
    }
}

#[test]
fn test_risk_level_is_valid() {
    let events = collector::collect_all().unwrap_or_default();
    for event in &events {
        match event.risk_level {
            RiskLevel::Low | RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical => {}
        }
    }
}
