//! Report types for the AI audit trail.
//!
//! Defines [`ReportConfig`], [`ReportType`], [`ComplianceScore`],
//! and related types for generating analytics reports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::{EventSource, EventType};
use crate::ops::audit::query_types::{QueryFilter, TimeRange};

// ─── Report Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportType {
    Summary,
    Detail,
    Trend,
    Violation,
    Compliance,
}

impl std::fmt::Display for ReportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Summary => write!(f, "summary"),
            Self::Detail => write!(f, "detail"),
            Self::Trend => write!(f, "trend"),
            Self::Violation => write!(f, "violation"),
            Self::Compliance => write!(f, "compliance"),
        }
    }
}

// ─── Report Format ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    Html,
    Pdf,
    Csv,
    Json,
    Markdown,
}

impl std::fmt::Display for ReportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Html => write!(f, "html"),
            Self::Pdf => write!(f, "pdf"),
            Self::Csv => write!(f, "csv"),
            Self::Json => write!(f, "json"),
            Self::Markdown => write!(f, "markdown"),
        }
    }
}

// ─── Group By ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupBy {
    EventType,
    Source,
    Result,
    Hour,
    Day,
    Week,
    Month,
    SecretKey,
    ToolType,
}

// ─── Group Compatibility ──────────────────────────────────────────

static COMPATIBLE_GROUPS: &[(ReportType, &[GroupBy])] = &[
    (
        ReportType::Summary,
        &[GroupBy::EventType, GroupBy::Source, GroupBy::Result],
    ),
    (
        ReportType::Detail,
        &[
            GroupBy::EventType,
            GroupBy::Source,
            GroupBy::Result,
            GroupBy::SecretKey,
            GroupBy::ToolType,
        ],
    ),
    (
        ReportType::Trend,
        &[GroupBy::Hour, GroupBy::Day, GroupBy::Week, GroupBy::Month],
    ),
    (
        ReportType::Violation,
        &[GroupBy::EventType, GroupBy::Source, GroupBy::SecretKey],
    ),
    (
        ReportType::Compliance,
        &[GroupBy::EventType, GroupBy::Source],
    ),
];

fn is_compatible_group(report_type: ReportType, group_by: GroupBy) -> bool {
    COMPATIBLE_GROUPS
        .iter()
        .any(|(rt, groups)| *rt == report_type && groups.contains(&group_by))
}

// ─── Report Config ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    pub report_type: ReportType,
    pub time_range: TimeRange,
    pub filters: Vec<QueryFilter>,
    pub group_by: Option<GroupBy>,
    pub format: ReportFormat,
}

impl ReportConfig {
    #[must_use]
    pub fn new(report_type: ReportType, time_range: TimeRange) -> Self {
        Self {
            report_type,
            time_range,
            filters: Vec::new(),
            group_by: None,
            format: ReportFormat::Markdown,
        }
    }

    #[must_use]
    pub fn with_filter(mut self, filter: QueryFilter) -> Self {
        self.filters.push(filter);
        self
    }

    #[must_use]
    pub fn with_filters(mut self, filters: Vec<QueryFilter>) -> Self {
        self.filters = filters;
        self
    }

    #[must_use]
    pub fn with_group_by(mut self, group_by: GroupBy) -> Self {
        self.group_by = Some(group_by);
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: ReportFormat) -> Self {
        self.format = format;
        self
    }

    pub fn validate(&self) -> Result<(), ReportError> {
        if self.time_range.start.is_none() && self.time_range.end.is_none() {
            return Err(ReportError::TimeRangeRequired);
        }

        if let Some(group_by) = self.group_by {
            if !is_compatible_group(self.report_type, group_by) {
                return Err(ReportError::IncompatibleGroupBy {
                    report_type: self.report_type,
                    group_by,
                });
            }
        }

        Ok(())
    }
}

// ─── Report Summary ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_events: u64,
    pub unique_sessions: u64,
    pub unique_secrets: u64,
    pub date_range: TimeRange,
    pub top_event_types: Vec<(EventType, u64)>,
    pub top_sources: Vec<(EventSource, u64)>,
}

impl ReportSummary {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            total_events: 0,
            unique_sessions: 0,
            unique_secrets: 0,
            date_range: TimeRange::all(),
            top_event_types: Vec::new(),
            top_sources: Vec::new(),
        }
    }
}

// ─── Compliance ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceLevel {
    Green,
    Yellow,
    Red,
}

impl ComplianceLevel {
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        if score >= 0.8 {
            Self::Green
        } else if score >= 0.5 {
            Self::Yellow
        } else {
            Self::Red
        }
    }
}

impl std::fmt::Display for ComplianceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Green => write!(f, "green"),
            Self::Yellow => write!(f, "yellow"),
            Self::Red => write!(f, "red"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceScore {
    pub score: f64,
    pub level: ComplianceLevel,
    pub violations: u64,
    pub checked_at: DateTime<Utc>,
}

impl ComplianceScore {
    #[must_use]
    pub fn new(score: f64, violations: u64) -> Self {
        let level = ComplianceLevel::from_score(score);
        Self {
            score,
            level,
            violations,
            checked_at: Utc::now(),
        }
    }
}

// ─── Report Error ──────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("time range required for reports")]
    TimeRangeRequired,

    #[error("incompatible group_by {group_by:?} for report type {report_type:?}")]
    IncompatibleGroupBy {
        report_type: ReportType,
        group_by: GroupBy,
    },

    #[error("report generation failed: {0}")]
    GenerationFailed(String),

    #[error("export failed: {0}")]
    ExportFailed(String),
}

// ─── Aggregation Types ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationGroup {
    pub key: String,
    pub count: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationResult {
    pub group_by: GroupBy,
    pub groups: Vec<AggregationGroup>,
    pub total_events: u64,
    pub time_range: TimeRange,
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::audit::query_types::TimeRange;

    #[test]
    fn test_report_type_display() {
        assert_eq!(ReportType::Summary.to_string(), "summary");
        assert_eq!(ReportType::Compliance.to_string(), "compliance");
    }

    #[test]
    fn test_report_format_display() {
        assert_eq!(ReportFormat::Json.to_string(), "json");
        assert_eq!(ReportFormat::Markdown.to_string(), "markdown");
    }

    #[test]
    fn test_compliance_level_from_score() {
        assert_eq!(ComplianceLevel::from_score(0.9), ComplianceLevel::Green);
        assert_eq!(ComplianceLevel::from_score(0.8), ComplianceLevel::Green);
        assert_eq!(ComplianceLevel::from_score(0.65), ComplianceLevel::Yellow);
        assert_eq!(ComplianceLevel::from_score(0.5), ComplianceLevel::Yellow);
        assert_eq!(ComplianceLevel::from_score(0.3), ComplianceLevel::Red);
        assert_eq!(ComplianceLevel::from_score(0.0), ComplianceLevel::Red);
    }

    #[test]
    fn test_compliance_score_new() {
        let cs = ComplianceScore::new(0.85, 0);
        assert!((cs.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(cs.level, ComplianceLevel::Green);
        assert_eq!(cs.violations, 0);
    }

    #[test]
    fn test_report_config_validate_valid() {
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_report_config_validate_no_time_range() {
        let config = ReportConfig::new(ReportType::Summary, TimeRange::all());
        assert!(matches!(
            config.validate(),
            Err(ReportError::TimeRangeRequired)
        ));
    }

    #[test]
    fn test_report_config_validate_compatible_group() {
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24))
            .with_group_by(GroupBy::EventType);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_report_config_validate_incompatible_group() {
        let config = ReportConfig::new(ReportType::Summary, TimeRange::last_hours(24))
            .with_group_by(GroupBy::Hour);
        assert!(matches!(
            config.validate(),
            Err(ReportError::IncompatibleGroupBy { .. })
        ));
    }

    #[test]
    fn test_report_config_builder() {
        let config = ReportConfig::new(ReportType::Trend, TimeRange::last_days(7))
            .with_group_by(GroupBy::Day)
            .with_format(ReportFormat::Json);

        assert_eq!(config.report_type, ReportType::Trend);
        assert_eq!(config.group_by, Some(GroupBy::Day));
        assert_eq!(config.format, ReportFormat::Json);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_report_summary_empty() {
        let s = ReportSummary::empty();
        assert_eq!(s.total_events, 0);
        assert_eq!(s.unique_sessions, 0);
        assert_eq!(s.unique_secrets, 0);
    }

    #[test]
    fn test_trend_compatible_groups() {
        for group in [GroupBy::Hour, GroupBy::Day, GroupBy::Week, GroupBy::Month] {
            assert!(is_compatible_group(ReportType::Trend, group));
        }
        assert!(!is_compatible_group(ReportType::Trend, GroupBy::SecretKey));
    }

    #[test]
    fn test_violation_compatible_groups() {
        assert!(is_compatible_group(
            ReportType::Violation,
            GroupBy::EventType
        ));
        assert!(is_compatible_group(ReportType::Violation, GroupBy::Source));
        assert!(!is_compatible_group(ReportType::Violation, GroupBy::Day));
    }
}
