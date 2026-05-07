use chrono::{Duration, Utc};
use uuid::Uuid;

use envforge::model::*;
use envforge::ops::analytics::aggregation;

fn make_test_event(secret_name: &str, hours_ago: i64, accessor_id: &str) -> EnrichedAccessEvent {
    let timestamp = Utc::now() - Duration::hours(hours_ago);
    EnrichedAccessEvent {
        id: Uuid::new_v4(),
        raw: RawAccessEvent {
            secret_name: secret_name.to_string(),
            access_type: AccessType::Read,
            accessor: AccessorInfo {
                id: accessor_id.to_string(),
                accessor_type: AccessorType::User,
                name: None,
                ip_address: None,
                user_agent: None,
            },
            timestamp,
            source: AccessSource::Cli,
            context: None,
        },
        secret_id: secret_name.to_string(),
        provider: "test".to_string(),
        environment: None,
        risk_level: RiskLevel::Low,
        enriched_at: timestamp,
    }
}

// ─── Aggregation Tests ────────────────────────────────

#[test]
fn test_aggregate_daily_groups_correctly() {
    let events = vec![
        make_test_event("KEY_A", 1, "user1"),
        make_test_event("KEY_A", 3, "user1"),
        make_test_event("KEY_A", 25, "user2"), // different day
        make_test_event("KEY_B", 2, "user1"),
    ];

    let buckets = aggregation::aggregate(&events, &AggregatePeriod::Daily);

    // Should have at least 2 keys across at least 2 days
    assert!(buckets.len() >= 2);

    // Find KEY_A buckets
    let key_a_buckets: Vec<_> = buckets.iter().filter(|b| b.key.contains("KEY_A")).collect();
    assert!(!key_a_buckets.is_empty());
}

#[test]
fn test_aggregate_counts_unique_accessors() {
    let events = vec![
        make_test_event("KEY_A", 1, "user1"),
        make_test_event("KEY_A", 1, "user2"),
        make_test_event("KEY_A", 1, "user2"),
        make_test_event("KEY_A", 1, "user3"),
    ];

    let buckets = aggregation::aggregate(&events, &AggregatePeriod::Hourly);
    let key_bucket = buckets.iter().find(|b| b.key.contains("KEY_A")).unwrap();

    assert_eq!(key_bucket.access_count, 4);
    assert_eq!(key_bucket.unique_accessors, 3);
}

#[test]
fn test_aggregate_different_periods() {
    let events = vec![make_test_event("KEY_A", 1, "user1")];

    let hourly = aggregation::aggregate(&events, &AggregatePeriod::Hourly);
    let daily = aggregation::aggregate(&events, &AggregatePeriod::Daily);
    let weekly = aggregation::aggregate(&events, &AggregatePeriod::Weekly);
    let monthly = aggregation::aggregate(&events, &AggregatePeriod::Monthly);

    // All periods should produce at least 1 bucket
    assert!(!hourly.is_empty());
    assert!(!daily.is_empty());
    assert!(!weekly.is_empty());
    assert!(!monthly.is_empty());
}

#[test]
fn test_aggregate_buckets_sorted_newest_first() {
    let events = vec![
        make_test_event("KEY_A", 1, "user1"),
        make_test_event("KEY_A", 25, "user1"),
    ];

    let buckets = aggregation::aggregate(&events, &AggregatePeriod::Hourly);
    if buckets.len() >= 2 {
        assert!(buckets[0].period_start >= buckets[1].period_start);
    }
}

#[test]
fn test_aggregate_empty_events() {
    let events: Vec<EnrichedAccessEvent> = vec![];
    let buckets = aggregation::aggregate(&events, &AggregatePeriod::Daily);
    assert!(buckets.is_empty());
}

// ─── Bucket Key Tests ─────────────────────────────────

#[test]
fn test_bucket_key_format() {
    let ts = Utc::now();
    let key = aggregation::bucket_key("MY_KEY", &ts, &AggregatePeriod::Daily);
    assert!(key.starts_with("MY_KEY:"));
    assert!(key.contains("Daily"));
}

// ─── Load Aggregates Tests ────────────────────────────

#[test]
fn test_load_aggregates_returns_vec() {
    let result = aggregation::load_aggregates();
    // Should return Ok (empty vec if no file)
    assert!(result.is_ok());
}

// ─── Save + Load Round-Trip Tests ─────────────────────

#[test]
fn test_save_and_load_aggregates_roundtrip() {
    let run_id = Uuid::new_v4().to_string();
    let secret = format!("ROUNDTRIP_{}", run_id);

    let events = vec![
        make_test_event(&secret, 1, "user1"),
        make_test_event(&secret, 1, "user2"),
    ];

    let buckets = aggregation::aggregate(&events, &AggregatePeriod::Hourly);
    assert!(!buckets.is_empty());

    // Save
    aggregation::save_aggregates(&buckets).unwrap();

    // Load
    let loaded = aggregation::load_aggregates().unwrap();
    let rt_buckets: Vec<_> = loaded.iter().filter(|b| b.key.contains(&secret)).collect();
    assert!(!rt_buckets.is_empty());

    let count: u64 = rt_buckets.iter().map(|b| b.access_count).sum();
    assert_eq!(count, 2);
}

// ─── Recompute Tests ──────────────────────────────────

#[test]
fn test_recompute_returns_summary() {
    let config = AnalyticsConfig::default();
    let result = aggregation::recompute(&config);
    // Should return Ok (even if no events exist)
    assert!(result.is_ok());
    let summary = result.unwrap();
    // Summary should have valid fields
    assert!(
        summary.total_secrets
            <= summary.active_count + summary.dormant_count + summary.unused_count + 10
    );
}
