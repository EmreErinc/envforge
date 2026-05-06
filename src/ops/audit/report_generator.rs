//! Report generator for the AI audit trail.
//!
//! Generates SOC2 compliance reports, detects violations, computes
//! compliance scores, and exports to multiple formats (JSON, CSV, Markdown).

use std::collections::{HashMap, HashSet};
use std::io::Write;

use chrono::Utc;
use thiserror::Error;

use super::custody;
use super::query_engine;
use super::query_types::TimeRange;
use super::report_types::{
    AggregationResult, ComplianceScore, ReportConfig, ReportError, ReportFormat, ReportSummary,
};
use super::types::{AuditEvent, EventResult, EventSource, EventType};

// ─── Violation Detection ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Violation {
    pub violation_type: ViolationType,
    pub severity: ViolationSeverity,
    pub event_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub description: String,
    pub secret_key: Option<String>,
    pub source: EventSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    UnauthorizedAccess,
    PolicyViolation,
    CustodyGap,
    SecretExposure,
    AnomalousFrequency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

pub fn detect_violations(events: &[AuditEvent]) -> Vec<Violation> {
    let mut violations = Vec::new();

    for event in events {
        if let EventResult::Denied(ref reason) = event.result {
            violations.push(Violation {
                violation_type: ViolationType::UnauthorizedAccess,
                severity: ViolationSeverity::High,
                event_id: event.id.to_string(),
                timestamp: event.timestamp,
                description: format!("Access denied: {}", reason),
                secret_key: event.secret_key.clone(),
                source: event.source,
            });
        }

        if event.event_type == EventType::SecretExposure {
            violations.push(Violation {
                violation_type: ViolationType::SecretExposure,
                severity: ViolationSeverity::Critical,
                event_id: event.id.to_string(),
                timestamp: event.timestamp,
                description: "Secret exposure detected".to_string(),
                secret_key: event.secret_key.clone(),
                source: event.source,
            });
        }
    }

    let mut unique_keys: Vec<String> = events.iter().filter_map(|e| e.secret_key.clone()).collect();
    unique_keys.sort();
    unique_keys.dedup();

    for key in &unique_keys {
        let lineage = custody::build_lineage(events, key);
        if custody::has_custody_gaps(&lineage) {
            if let Some(last) = lineage.last_custodian() {
                violations.push(Violation {
                    violation_type: ViolationType::CustodyGap,
                    severity: ViolationSeverity::Medium,
                    event_id: last.event_id.to_string(),
                    timestamp: last.timestamp,
                    description: format!("Custody gap detected for secret: {}", key),
                    secret_key: Some(key.clone()),
                    source: last.source,
                });
            }
        }
    }

    detect_frequency_anomalies(events, &mut violations);

    violations.sort_by_key(|b| std::cmp::Reverse(b.severity));
    violations
}

fn detect_frequency_anomalies(events: &[AuditEvent], violations: &mut Vec<Violation>) {
    let mut access_counts: HashMap<String, HashSet<EventSource>> = HashMap::new();

    for event in events {
        if event.event_type == EventType::SecretAccessed {
            if let Some(ref key) = event.secret_key {
                access_counts
                    .entry(key.clone())
                    .or_default()
                    .insert(event.source);
            }
        }
    }

    for (key, sources) in &access_counts {
        if sources.len() > 3 {
            if let Some(event) = events.iter().find(|e| e.secret_key.as_deref() == Some(key)) {
                violations.push(Violation {
                    violation_type: ViolationType::AnomalousFrequency,
                    severity: ViolationSeverity::Low,
                    event_id: event.id.to_string(),
                    timestamp: event.timestamp,
                    description: format!(
                        "Secret '{}' accessed from {} different sources",
                        key,
                        sources.len()
                    ),
                    secret_key: Some(key.clone()),
                    source: event.source,
                });
            }
        }
    }
}

// ─── SOC2 Report ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Soc2Report {
    pub config: ReportConfig,
    pub summary: ReportSummary,
    pub compliance_score: ComplianceScore,
    pub violations: Vec<Violation>,
    pub aggregation: Option<AggregationResult>,
    pub generated_at: chrono::DateTime<Utc>,
}

impl Soc2Report {
    pub fn total_violations(&self) -> usize {
        self.violations.len()
    }

    pub fn critical_violations(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == ViolationSeverity::Critical)
            .count()
    }

    pub fn is_compliant(&self) -> bool {
        self.violations
            .iter()
            .all(|v| v.severity < ViolationSeverity::High)
    }
}

pub fn generate_soc2_report(
    events: &[AuditEvent],
    config: &ReportConfig,
) -> Result<Soc2Report, ReportError> {
    config.validate()?;

    let filtered: Vec<AuditEvent> = events
        .iter()
        .filter(|e| query_engine::matches_time_range(e, &config.time_range))
        .cloned()
        .collect();

    let violations = detect_violations(&filtered);
    let violation_count = violations.len() as u64;
    let total_events = filtered.len() as u64;

    // Score is 0.0–1.0 for ComplianceLevel::from_score
    let score = if total_events == 0 {
        1.0
    } else {
        let violation_weight: f64 = violations
            .iter()
            .map(|v| match v.severity {
                ViolationSeverity::Low => 0.01,
                ViolationSeverity::Medium => 0.02,
                ViolationSeverity::High => 0.05,
                ViolationSeverity::Critical => 0.10,
            })
            .sum();
        (1.0 - violation_weight / total_events as f64).max(0.0)
    };

    let compliance_score = ComplianceScore::new(score, violation_count);

    let aggregation = config
        .group_by
        .map(|group_by| query_engine::aggregate(&filtered, &group_by));

    let summary = build_summary(&filtered, &config.time_range);

    Ok(Soc2Report {
        config: config.clone(),
        summary,
        compliance_score,
        violations,
        aggregation,
        generated_at: Utc::now(),
    })
}

fn build_summary(events: &[AuditEvent], time_range: &TimeRange) -> ReportSummary {
    let unique_sessions: u64 = events
        .iter()
        .filter_map(|e| e.session_id.as_ref())
        .collect::<HashSet<_>>()
        .len() as u64;

    let unique_secrets: u64 = events
        .iter()
        .filter_map(|e| e.secret_key.as_ref())
        .collect::<HashSet<_>>()
        .len() as u64;

    let mut type_counts: HashMap<EventType, u64> = HashMap::new();
    for event in events {
        *type_counts.entry(event.event_type).or_insert(0) += 1;
    }
    let mut top_event_types: Vec<(EventType, u64)> = type_counts.into_iter().collect();
    top_event_types.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut source_counts: HashMap<EventSource, u64> = HashMap::new();
    for event in events {
        *source_counts.entry(event.source).or_insert(0) += 1;
    }
    let mut top_sources: Vec<(EventSource, u64)> = source_counts.into_iter().collect();
    top_sources.sort_by_key(|b| std::cmp::Reverse(b.1));

    ReportSummary {
        total_events: events.len() as u64,
        unique_sessions,
        unique_secrets,
        date_range: time_range.clone(),
        top_event_types,
        top_sources,
    }
}

// ─── Export ──────────────────────────────────────────────────────

pub fn export_report(
    report: &Soc2Report,
    format: ReportFormat,
    writer: &mut dyn Write,
) -> Result<(), ReportError> {
    match format {
        ReportFormat::Json => export_json(report, writer),
        ReportFormat::Csv => export_csv(report, writer),
        ReportFormat::Markdown => export_markdown(report, writer),
        ReportFormat::Html | ReportFormat::Pdf => Err(ReportError::ExportFailed(format!(
            "{:?} export not yet implemented",
            format
        ))),
    }
}

fn export_json(report: &Soc2Report, writer: &mut dyn Write) -> Result<(), ReportError> {
    let json = serde_json::json!({
        "report_type": "soc2",
        "generated_at": report.generated_at.to_rfc3339(),
        "compliance_score": report.compliance_score.score,
        "compliance_level": report.compliance_score.level.to_string(),
        "total_events": report.summary.total_events,
        "total_violations": report.total_violations(),
        "critical_violations": report.critical_violations(),
        "is_compliant": report.is_compliant(),
        "violations": report.violations.iter().map(|v| serde_json::json!({
            "type": format!("{:?}", v.violation_type),
            "severity": format!("{:?}", v.severity),
            "event_id": v.event_id,
            "timestamp": v.timestamp.to_rfc3339(),
            "description": v.description,
            "secret_key": v.secret_key,
        })).collect::<Vec<_>>(),
    });

    writeln!(
        writer,
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_default()
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    Ok(())
}

fn export_csv(report: &Soc2Report, writer: &mut dyn Write) -> Result<(), ReportError> {
    writeln!(
        writer,
        "type,severity,event_id,timestamp,description,secret_key"
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    for v in &report.violations {
        writeln!(
            writer,
            "{:?},{:?},{},{},{}",
            v.violation_type,
            v.severity,
            v.event_id,
            v.timestamp.to_rfc3339(),
            v.description.replace(',', ";"),
        )
        .map_err(|e| ReportError::ExportFailed(e.to_string()))?;
    }

    Ok(())
}

fn export_markdown(report: &Soc2Report, writer: &mut dyn Write) -> Result<(), ReportError> {
    writeln!(writer, "# SOC2 Compliance Report")
        .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    writeln!(
        writer,
        "\n**Generated:** {}",
        report.generated_at.to_rfc3339()
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    writeln!(
        writer,
        "\n**Compliance Score:** {:.0}% ({})",
        report.compliance_score.score * 100.0,
        report.compliance_score.level
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    writeln!(
        writer,
        "\n**Total Events:** {}",
        report.summary.total_events
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    writeln!(
        writer,
        "\n**Violations:** {} ({} critical)",
        report.total_violations(),
        report.critical_violations()
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    writeln!(writer, "\n## Violations\n").map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    for v in &report.violations {
        writeln!(
            writer,
            "- **[{:?}]** {:?}: {} (at {})",
            v.severity,
            v.violation_type,
            v.description,
            v.timestamp.to_rfc3339(),
        )
        .map_err(|e| ReportError::ExportFailed(e.to_string()))?;
    }

    writeln!(
        writer,
        "\n**Compliant:** {}",
        if report.is_compliant() { "YES" } else { "NO" }
    )
    .map_err(|e| ReportError::ExportFailed(e.to_string()))?;

    Ok(())
}

// ─── Report Error ─────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("report configuration error: {0}")]
    Config(String),

    #[error("export failed: {0}")]
    ExportFailed(String),
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::audit::report_types::{ComplianceLevel, ReportType};

    fn test_event(
        event_type: EventType,
        source: EventSource,
        result: EventResult,
        hours_ago: i64,
        secret_key: Option<&str>,
    ) -> AuditEvent {
        let mut event = AuditEvent::new(event_type, source, result);
        event.timestamp = Utc::now() - chrono::Duration::hours(hours_ago);
        if let Some(key) = secret_key {
            event.secret_key = Some(key.to_string());
        }
        event
    }

    fn test_events() -> Vec<AuditEvent> {
        vec![
            test_event(
                EventType::SecretAccessed,
                EventSource::AiGuard,
                EventResult::Success,
                5,
                Some("DB_PASSWORD"),
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::Proxy,
                EventResult::Success,
                4,
                Some("API_KEY"),
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::AiGuard,
                EventResult::Denied("unauthorized".to_string()),
                3,
                Some("DB_PASSWORD"),
            ),
            test_event(
                EventType::SecretExposure,
                EventSource::Cli,
                EventResult::Success,
                2,
                Some("AWS_SECRET"),
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::Proxy,
                EventResult::Success,
                1,
                Some("API_KEY"),
            ),
        ]
    }

    fn default_config() -> ReportConfig {
        ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24))
    }

    #[test]
    fn test_detect_violations_denied_access() {
        let events = test_events();
        let violations = detect_violations(&events);
        let denied: Vec<_> = violations
            .iter()
            .filter(|v| v.violation_type == ViolationType::UnauthorizedAccess)
            .collect();
        assert!(!denied.is_empty());
        assert_eq!(denied[0].severity, ViolationSeverity::High);
    }

    #[test]
    fn test_detect_violations_secret_exposure() {
        let events = test_events();
        let violations = detect_violations(&events);
        let exposures: Vec<_> = violations
            .iter()
            .filter(|v| v.violation_type == ViolationType::SecretExposure)
            .collect();
        assert!(!exposures.is_empty());
        assert_eq!(exposures[0].severity, ViolationSeverity::Critical);
    }

    #[test]
    fn test_detect_violations_no_serious_violations() {
        let events = vec![
            test_event(
                EventType::SecretAccessed,
                EventSource::Cli,
                EventResult::Success,
                1,
                Some("KEY"),
            ),
            test_event(
                EventType::SecretBound,
                EventSource::Cli,
                EventResult::Success,
                2,
                Some("KEY"),
            ),
        ];
        let violations = detect_violations(&events);
        let serious: Vec<_> = violations
            .iter()
            .filter(|v| v.severity >= ViolationSeverity::High)
            .collect();
        assert!(serious.is_empty());
    }

    #[test]
    fn test_generate_soc2_report_basic() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        assert!(report.summary.total_events >= 5);
        assert!(!report.violations.is_empty());
    }

    #[test]
    fn test_soc2_report_compliant_when_clean() {
        let events = vec![test_event(
            EventType::SecretAccessed,
            EventSource::Cli,
            EventResult::Success,
            1,
            Some("KEY"),
        )];
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24));
        let report = generate_soc2_report(&events, &config).unwrap();
        assert!(report.compliance_score.score > 0.9);
        assert!(report.is_compliant());
    }

    #[test]
    fn test_soc2_report_with_violations() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        assert!(report.total_violations() > 0);
        assert!(report.critical_violations() > 0);
    }

    #[test]
    fn test_export_json() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        let mut buf = Vec::new();
        export_report(&report, ReportFormat::Json, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("soc2"));
        assert!(output.contains("compliance_score"));
    }

    #[test]
    fn test_export_csv() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        let mut buf = Vec::new();
        export_report(&report, ReportFormat::Csv, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("type,severity"));
    }

    #[test]
    fn test_export_markdown() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        let mut buf = Vec::new();
        export_report(&report, ReportFormat::Markdown, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("SOC2 Compliance Report"));
        assert!(output.contains("Compliance Score"));
    }

    #[test]
    fn test_export_markdown_compliant() {
        let events = vec![test_event(
            EventType::SecretAccessed,
            EventSource::Cli,
            EventResult::Success,
            1,
            Some("KEY"),
        )];
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24));
        let report = generate_soc2_report(&events, &config).unwrap();
        let mut buf = Vec::new();
        export_report(&report, ReportFormat::Markdown, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Compliant:** YES"));
    }

    #[test]
    fn test_compliance_score_green_when_clean() {
        let events = vec![test_event(
            EventType::SecretAccessed,
            EventSource::Cli,
            EventResult::Success,
            1,
            Some("KEY"),
        )];
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24));
        let report = generate_soc2_report(&events, &config).unwrap();
        assert_eq!(report.compliance_score.level, ComplianceLevel::Green);
    }

    #[test]
    fn test_compliance_score_with_violations() {
        let events = test_events();
        let config = default_config();
        let report = generate_soc2_report(&events, &config).unwrap();
        assert!(report.compliance_score.score < 1.0);
    }

    #[test]
    fn test_build_summary() {
        let events = test_events();
        let time_range = TimeRange::last_hours(24);
        let summary = build_summary(&events, &time_range);
        assert_eq!(summary.total_events, 5);
        assert!(summary.unique_secrets >= 3);
    }
}
