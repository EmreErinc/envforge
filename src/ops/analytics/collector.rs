use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::model::{
    AccessContext, AccessSource, AccessType, AccessorInfo, AccessorType, AnalyticsError,
    EnrichedAccessEvent, RawAccessEvent, RiskLevel, TimeWindow,
};
use crate::ops::secrets::age;
use crate::ops::{changelog, proxy};

/// Collect normalized access events from all sources, filtered by time window.
pub fn collect_events(window: &TimeWindow) -> Result<Vec<EnrichedAccessEvent>, AnalyticsError> {
    let cutoff = window_cutoff(window);
    let mut events = Vec::new();

    // Collect from proxy audit log
    if let Ok(proxy_entries) = proxy::read_audit_log() {
        for entry in &proxy_entries {
            let enriched = normalize_proxy_entry(entry);
            if enriched.enriched_at >= cutoff {
                events.push(enriched);
            }
        }
    }

    // Collect from changelog
    if let Ok(changelog_entries) = changelog::read_changelog(None, usize::MAX) {
        for entry in &changelog_entries {
            let enriched = normalize_changelog_entry(entry);
            if enriched.enriched_at >= cutoff {
                events.push(enriched);
            }
        }
    }

    // Collect from age tracker (synthetic "last-update" events)
    if let Ok(age_entries) = age::get_age_report(90) {
        for entry in &age_entries {
            let enriched = normalize_age_entry(entry);
            if enriched.enriched_at >= cutoff {
                events.push(enriched);
            }
        }
    }

    Ok(events)
}

/// Collect all events without time filtering (defaults to 90-day window).
pub fn collect_all() -> Result<Vec<EnrichedAccessEvent>, AnalyticsError> {
    collect_events(&TimeWindow::Last90Days)
}

/// Resolve a TimeWindow to a DateTime<Utc> cutoff point.
fn window_cutoff(window: &TimeWindow) -> DateTime<Utc> {
    let now = Utc::now();
    let duration = match window {
        TimeWindow::LastHour => Duration::hours(1),
        TimeWindow::Last24Hours => Duration::hours(24),
        TimeWindow::Last7Days => Duration::days(7),
        TimeWindow::Last30Days => Duration::days(30),
        TimeWindow::Last90Days => Duration::days(90),
        TimeWindow::Custom(custom) => Duration::seconds(custom.duration_seconds as i64),
    };
    now - duration
}

/// Normalize a proxy AuditEntry into an EnrichedAccessEvent.
fn normalize_proxy_entry(entry: &proxy::AuditEntry) -> EnrichedAccessEvent {
    let access_type = match entry.action.as_str() {
        "set" | "put" | "SET" | "PUT" => AccessType::Write,
        "delete" | "DELETE" => AccessType::Delete,
        _ => AccessType::Read, // "access", "denied", "approved", etc.
    };

    let accessor_type = classify_proxy_accessor(&entry.client_addr, entry.user_agent.as_deref());

    let accessor = AccessorInfo {
        id: entry.client_addr.clone(),
        accessor_type,
        name: None,
        ip_address: Some(entry.client_addr.clone()),
        user_agent: entry.user_agent.clone(),
    };

    let risk_level = if entry.granted {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    };

    let timestamp = parse_iso8601(&entry.timestamp).unwrap_or_else(Utc::now);

    let raw = RawAccessEvent {
        secret_name: entry.key.clone().unwrap_or_else(|| "unknown".to_string()),
        access_type,
        accessor,
        timestamp,
        source: AccessSource::Proxy,
        context: None,
    };

    EnrichedAccessEvent {
        id: Uuid::new_v4(),
        raw,
        secret_id: entry.key.clone().unwrap_or_else(|| "unknown".to_string()),
        provider: "proxy".to_string(),
        environment: None,
        risk_level,
        enriched_at: Utc::now(),
    }
}

/// Normalize a ChangelogEntry into an EnrichedAccessEvent.
fn normalize_changelog_entry(entry: &changelog::ChangelogEntry) -> EnrichedAccessEvent {
    let access_type = match entry.action.as_str() {
        "ADD" => AccessType::Write,
        "MOD" => AccessType::Write,
        "DEL" => AccessType::Delete,
        _ => AccessType::Read,
    };

    let accessor = AccessorInfo {
        id: "changelog-user".to_string(),
        accessor_type: AccessorType::User,
        name: None,
        ip_address: None,
        user_agent: None,
    };

    let timestamp = parse_iso8601(&entry.timestamp).unwrap_or_else(Utc::now);

    let context = AccessContext {
        command: None,
        file_path: None,
        working_directory: None,
        environment: Some(entry.profile.clone()),
        metadata: None,
    };

    let raw = RawAccessEvent {
        secret_name: entry.key.clone(),
        access_type,
        accessor,
        timestamp,
        source: AccessSource::Changelog,
        context: Some(context),
    };

    EnrichedAccessEvent {
        id: Uuid::new_v4(),
        raw,
        secret_id: entry.key.clone(),
        provider: "local".to_string(),
        environment: Some(entry.profile.clone()),
        risk_level: RiskLevel::Low,
        enriched_at: Utc::now(),
    }
}

/// Create a synthetic "last-update" event from an AgeEntry.
fn normalize_age_entry(entry: &age::AgeEntry) -> EnrichedAccessEvent {
    let accessor = AccessorInfo {
        id: format!("provider:{}", entry.provider),
        accessor_type: AccessorType::Service,
        name: Some(entry.provider.clone()),
        ip_address: None,
        user_agent: None,
    };

    let timestamp = parse_iso8601(&entry.updated_at).unwrap_or_else(Utc::now);

    let risk_level = if entry.stale {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    let raw = RawAccessEvent {
        secret_name: entry.key.clone(),
        access_type: AccessType::Write, // synthetic: "last updated"
        accessor,
        timestamp,
        source: AccessSource::Provider,
        context: None,
    };

    EnrichedAccessEvent {
        id: Uuid::new_v4(),
        raw,
        secret_id: entry.key.clone(),
        provider: entry.provider.clone(),
        environment: None,
        risk_level,
        enriched_at: Utc::now(),
    }
}

/// Classify proxy accessor type based on client address and user agent.
fn classify_proxy_accessor(client_addr: &str, user_agent: Option<&str>) -> AccessorType {
    let addr_lower = client_addr.to_lowercase();
    if addr_lower.contains("127.") || addr_lower.contains("::1") || addr_lower == "localhost" {
        return AccessorType::User;
    }

    let ua_lower = user_agent.unwrap_or("").to_lowercase();
    if ua_lower.contains("ci")
        || ua_lower.contains("jenkins")
        || ua_lower.contains("github")
        || ua_lower.contains("gitlab")
        || ua_lower.contains("circleci")
        || ua_lower.contains("buildkite")
    {
        return AccessorType::CiCdPipeline;
    }

    if ua_lower.contains("ai")
        || ua_lower.contains("claude")
        || ua_lower.contains("gpt")
        || ua_lower.contains("copilot")
        || ua_lower.contains("codex")
    {
        return AccessorType::AiTool;
    }

    AccessorType::Unknown
}

/// Parse an ISO 8601 timestamp string.
fn parse_iso8601(s: &str) -> Option<DateTime<Utc>> {
    // Try RFC3339 first (chrono::DateTime::parse_from_rfc3339)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // Try basic format with timezone offset
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ") {
        return Some(dt.with_timezone(&Utc));
    }
    // Try without timezone
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.with_timezone(&Utc));
    }
    None
}
