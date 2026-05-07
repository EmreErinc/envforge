use envforge::model::*;

// ─── JSON Round-Trip Tests ────────────────────────────

#[test]
fn test_raw_access_event_json_roundtrip() {
    let event = RawAccessEvent {
        secret_name: "DATABASE_URL".to_string(),
        access_type: AccessType::Read,
        accessor: AccessorInfo {
            id: "agent-001".to_string(),
            accessor_type: AccessorType::AiTool,
            name: Some("CodeBuddy".to_string()),
            ip_address: None,
            user_agent: Some("claude-3.5".to_string()),
        },
        timestamp: chrono::Utc::now(),
        source: AccessSource::AiGuard,
        context: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let parsed: RawAccessEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.secret_name, "DATABASE_URL");
    assert!(matches!(parsed.access_type, AccessType::Read));
    assert_eq!(parsed.accessor.id, "agent-001");
}

#[test]
fn test_enriched_access_event_json_roundtrip() {
    let raw = RawAccessEvent {
        secret_name: "API_KEY".to_string(),
        access_type: AccessType::Write,
        accessor: AccessorInfo {
            id: "user-42".to_string(),
            accessor_type: AccessorType::User,
            name: Some("emre".to_string()),
            ip_address: None,
            user_agent: None,
        },
        timestamp: chrono::Utc::now(),
        source: AccessSource::Cli,
        context: None,
    };

    let enriched = EnrichedAccessEvent {
        id: uuid::Uuid::new_v4(),
        raw,
        secret_id: "sec_123".to_string(),
        provider: "aws".to_string(),
        environment: Some("production".to_string()),
        risk_level: RiskLevel::Low,
        enriched_at: chrono::Utc::now(),
    };

    let json = serde_json::to_string(&enriched).unwrap();
    let parsed: EnrichedAccessEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.secret_id, "sec_123");
    assert_eq!(parsed.provider, "aws");
    assert!(matches!(parsed.risk_level, RiskLevel::Low));
}

#[test]
fn test_analytic_report_json_roundtrip() {
    let summary = AnalyticsSummary {
        total_secrets: 42,
        total_events: 1500,
        unused_count: 5,
        dormant_count: 3,
        active_count: 34,
        estimated_monthly_cost: 12.50,
    };

    let report = AnalyticsReport {
        id: uuid::Uuid::new_v4(),
        generated_at: chrono::Utc::now(),
        summary,
        unused: vec![],
        low_usage: vec![],
        trends: vec![],
        correlations: vec![],
        cost: None,
        recommendations: vec![],
    };

    let json = serde_json::to_string(&report).unwrap();
    let parsed: AnalyticsReport = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.summary.total_secrets, 42);
    assert_eq!(parsed.summary.unused_count, 5);
}

// ─── Enum Serialization Tests ─────────────────────────

#[test]
fn test_access_type_serialization() {
    let read = serde_json::to_string(&AccessType::Read).unwrap();
    assert_eq!(read, "\"Read\"");

    let write = serde_json::to_string(&AccessType::Write).unwrap();
    assert_eq!(write, "\"Write\"");

    let delete = serde_json::to_string(&AccessType::Delete).unwrap();
    assert_eq!(delete, "\"Delete\"");

    let list = serde_json::to_string(&AccessType::List).unwrap();
    assert_eq!(list, "\"List\"");
}

#[test]
fn test_access_type_deserialization() {
    let read: AccessType = serde_json::from_str("\"Read\"").unwrap();
    assert!(matches!(read, AccessType::Read));

    let write: AccessType = serde_json::from_str("\"Write\"").unwrap();
    assert!(matches!(write, AccessType::Write));
}

#[test]
fn test_accessor_type_serialization() {
    assert_eq!(
        serde_json::to_string(&AccessorType::User).unwrap(),
        "\"User\""
    );
    assert_eq!(
        serde_json::to_string(&AccessorType::AiTool).unwrap(),
        "\"AiTool\""
    );
    assert_eq!(
        serde_json::to_string(&AccessorType::Unknown).unwrap(),
        "\"Unknown\""
    );
}

#[test]
fn test_risk_level_variants() {
    let levels = vec![
        RiskLevel::Low,
        RiskLevel::Medium,
        RiskLevel::High,
        RiskLevel::Critical,
    ];

    for level in levels {
        let json = serde_json::to_string(&level).unwrap();
        let parsed: RiskLevel = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn test_time_window_serialization() {
    assert_eq!(
        serde_json::to_string(&TimeWindow::LastHour).unwrap(),
        "\"LastHour\""
    );
    assert_eq!(
        serde_json::to_string(&TimeWindow::Last24Hours).unwrap(),
        "\"Last24Hours\""
    );
    assert_eq!(
        serde_json::to_string(&TimeWindow::Last7Days).unwrap(),
        "\"Last7Days\""
    );
    assert_eq!(
        serde_json::to_string(&TimeWindow::Last90Days).unwrap(),
        "\"Last90Days\""
    );
}

// ─── Config Default Tests ─────────────────────────────

#[test]
fn test_analytics_config_default() {
    let config = AnalyticsConfig::default();

    assert!(config.enabled);
    assert_eq!(config.retention_days, 90);
    assert_eq!(config.max_events, 10000);
    assert!(config.auto_aggregate);
    assert!(config.pricing_file.is_none());
}

#[test]
fn test_analytics_config_json_roundtrip() {
    let config = AnalyticsConfig {
        enabled: false,
        retention_days: 30,
        max_events: 5000,
        auto_aggregate: false,
        pricing_file: Some("/path/to/pricing.toml".to_string()),
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: AnalyticsConfig = serde_json::from_str(&json).unwrap();

    assert!(!parsed.enabled);
    assert_eq!(parsed.retention_days, 30);
    assert_eq!(parsed.max_events, 5000);
    assert!(!parsed.auto_aggregate);
    assert_eq!(
        parsed.pricing_file,
        Some("/path/to/pricing.toml".to_string())
    );
}

// ─── Backward Compatibility Tests ─────────────────────

#[test]
fn test_app_config_loads_without_analytics_section() {
    let toml_str = r#"
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 5
footer_protected_lines = 3

[protected_blocks]
markers = []
"#;

    let config: envforge::config::AppConfig = toml::from_str(toml_str).unwrap();

    // Analytics section should get defaults when missing
    assert!(config.analytics.enabled);
    assert_eq!(config.analytics.retention_days, 90);
    assert_eq!(config.analytics.max_events, 10000);
}

#[test]
fn test_app_config_with_analytics_section() {
    let toml_str = r#"
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 5
footer_protected_lines = 3

[protected_blocks]
markers = []

[analytics]
enabled = false
retention_days = 180
max_events = 20000
auto_aggregate = false
"#;

    let config: envforge::config::AppConfig = toml::from_str(toml_str).unwrap();

    assert!(!config.analytics.enabled);
    assert_eq!(config.analytics.retention_days, 180);
    assert_eq!(config.analytics.max_events, 20000);
    assert!(!config.analytics.auto_aggregate);
    assert!(config.analytics.pricing_file.is_none());
}

// ─── PricingData Tests ────────────────────────────────

#[test]
fn test_pricing_data_default() {
    let data = PricingData::default();
    assert!(data.providers.is_empty());
}

#[test]
fn test_pricing_data_json_roundtrip() {
    let entry = PricingEntry {
        provider: "aws".to_string(),
        monthly_base: 0.05,
        per_secret: 0.01,
        per_access: 0.0001,
        currency: "USD".to_string(),
    };

    let data = PricingData {
        providers: vec![entry],
    };

    let json = serde_json::to_string(&data).unwrap();
    let parsed: PricingData = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.providers.len(), 1);
    assert_eq!(parsed.providers[0].provider, "aws");
    assert_eq!(parsed.providers[0].currency, "USD");
}

// ─── Unused/Detection Types Tests ─────────────────────

#[test]
fn test_unused_secret_json_roundtrip() {
    let unused = UnusedSecret {
        secret_name: "OLD_API_KEY".to_string(),
        reason: "No access in 90 days".to_string(),
        days_since_last_access: 95,
        confidence: 0.98,
    };

    let json = serde_json::to_string(&unused).unwrap();
    let parsed: UnusedSecret = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.secret_name, "OLD_API_KEY");
    assert_eq!(parsed.days_since_last_access, 95);
}

#[test]
fn test_deprecation_recommendation_json_roundtrip() {
    let unused = UnusedSecret {
        secret_name: "LEGACY_TOKEN".to_string(),
        reason: "Unused after migration".to_string(),
        days_since_last_access: 120,
        confidence: 0.99,
    };

    let timeline = SuggestedTimeline {
        review_by: chrono::Utc::now(),
        deprecate_by: chrono::Utc::now(),
        remove_by: chrono::Utc::now(),
    };

    let rec = DeprecationRecommendation {
        secret_name: "LEGACY_TOKEN".to_string(),
        reason: "Migration complete".to_string(),
        unused,
        timeline,
        dependent_count: 0,
    };

    let json = serde_json::to_string(&rec).unwrap();
    let parsed: DeprecationRecommendation = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.secret_name, "LEGACY_TOKEN");
    assert_eq!(parsed.dependent_count, 0);
}

// ─── AggregateBucket Tests ────────────────────────────

#[test]
fn test_aggregate_bucket_json_roundtrip() {
    let bucket = AggregateBucket {
        key: "DATABASE_URL:2026-05-07".to_string(),
        period_start: chrono::Utc::now(),
        period_type: AggregatePeriod::Daily,
        access_count: 42,
        unique_accessors: 3,
    };

    let json = serde_json::to_string(&bucket).unwrap();
    let parsed: AggregateBucket = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.key, "DATABASE_URL:2026-05-07");
    assert_eq!(parsed.access_count, 42);
    assert_eq!(parsed.unique_accessors, 3);
}

// ─── Option Skip Serialization Tests ──────────────────

#[test]
fn test_option_fields_skip_when_none() {
    let accessor = AccessorInfo {
        id: "test-id".to_string(),
        accessor_type: AccessorType::User,
        name: None,
        ip_address: None,
        user_agent: None,
    };

    let json = serde_json::to_string(&accessor).unwrap();
    // None fields should not appear in output
    assert!(!json.contains("name"));
    assert!(!json.contains("ip_address"));
    assert!(!json.contains("user_agent"));
}

#[test]
fn test_context_metadata_optional() {
    let context = AccessContext {
        command: Some("envforge list".to_string()),
        file_path: None,
        working_directory: None,
        environment: None,
        metadata: None,
    };

    let json = serde_json::to_string(&context).unwrap();
    assert!(!json.contains("file_path"));
    assert!(!json.contains("metadata"));
    assert!(json.contains("command"));
}
