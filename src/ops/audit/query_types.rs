//! Query types for the AI audit trail.
//!
//! Defines [`Query`], [`QueryFilter`], [`TimeRange`], [`Pagination`],
//! and related types for filtering, sorting, and paginating audit events.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use super::types::EventSource;

// ─── Identifier ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryId(pub String);

impl Default for QueryId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl QueryId {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Display for QueryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Time Range ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

impl TimeRange {
    #[must_use]
    pub fn all() -> Self {
        Self {
            start: None,
            end: None,
        }
    }

    #[must_use]
    pub fn last_hours(n: i64) -> Self {
        Self {
            start: Some(Utc::now() - Duration::hours(n)),
            end: None,
        }
    }

    #[must_use]
    pub fn last_days(n: i64) -> Self {
        Self {
            start: Some(Utc::now() - Duration::days(n)),
            end: None,
        }
    }

    #[must_use]
    pub fn today() -> Self {
        Self::last_days(0)
    }

    #[must_use]
    pub fn this_week() -> Self {
        Self::last_days(7)
    }

    #[must_use]
    pub fn this_month() -> Self {
        Self::last_days(30)
    }

    #[must_use]
    pub fn with_start(mut self, start: DateTime<Utc>) -> Self {
        self.start = Some(start);
        self
    }

    #[must_use]
    pub fn with_end(mut self, end: DateTime<Utc>) -> Self {
        self.end = Some(end);
        self
    }
}

// ─── Filter Types ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterField {
    EventType,
    Source,
    Result,
    SessionId,
    ToolType,
    SecretKey,
    Operation,
    MetadataKey(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOp {
    Eq,
    Ne,
    Contains,
    StartsWith,
    EndsWith,
    Gt,
    Lt,
    Gte,
    Lte,
    In,
    NotIn,
    Exists,
    NotExists,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterValue {
    String(String),
    Number(i64),
    Bool(bool),
    List(Vec<String>),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub field: FilterField,
    pub op: FilterOp,
    pub value: FilterValue,
}

impl QueryFilter {
    #[must_use]
    pub fn new(field: FilterField, op: FilterOp, value: FilterValue) -> Self {
        Self { field, op, value }
    }
}

// ─── Sort ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Timestamp,
    EventType,
    Source,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortSpec {
    pub field: SortField,
    pub direction: SortDirection,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            field: SortField::Timestamp,
            direction: SortDirection::Desc,
        }
    }
}

// ─── Pagination ────────────────────────────────────────────────────

pub const DEFAULT_LIMIT: u32 = 100;
pub const MAX_LIMIT: u32 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: u32,
    pub offset: Option<u64>,
    pub cursor: Option<String>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            offset: None,
            cursor: None,
        }
    }
}

impl Pagination {
    #[must_use]
    pub fn new(limit: u32) -> Self {
        Self {
            limit: limit.min(MAX_LIMIT),
            offset: None,
            cursor: None,
        }
    }

    #[must_use]
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    #[must_use]
    pub fn with_cursor(mut self, cursor: String) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

// ─── Output Format ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Csv,
    Markdown,
}

// ─── Query ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub id: QueryId,
    pub time_range: TimeRange,
    pub filters: Vec<QueryFilter>,
    pub sort: SortSpec,
    pub pagination: Pagination,
    pub format: OutputFormat,
}

impl Query {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: QueryId::new(),
            time_range: TimeRange::all(),
            filters: Vec::new(),
            sort: SortSpec::default(),
            pagination: Pagination::default(),
            format: OutputFormat::default(),
        }
    }

    #[must_use]
    pub fn with_time_range(mut self, range: TimeRange) -> Self {
        self.time_range = range;
        self
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
    pub fn with_sort(mut self, sort: SortSpec) -> Self {
        self.sort = sort;
        self
    }

    #[must_use]
    pub fn with_pagination(mut self, pagination: Pagination) -> Self {
        self.pagination = pagination;
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    pub fn validate(&self) -> Result<(), QueryError> {
        if let (Some(start), Some(end)) = (self.time_range.start, self.time_range.end) {
            if start > end {
                return Err(QueryError::InvalidTimeRange(
                    "start must be before end".to_string(),
                ));
            }
        }

        if self.pagination.limit > MAX_LIMIT {
            return Err(QueryError::LimitExceeded(self.pagination.limit));
        }

        for filter in &self.filters {
            validate_filter_compatibility(&filter.field, filter.op, &filter.value)?;
        }

        Ok(())
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_filter_compatibility(
    field: &FilterField,
    op: FilterOp,
    value: &FilterValue,
) -> Result<(), QueryError> {
    match op {
        FilterOp::In | FilterOp::NotIn => {
            if !matches!(value, FilterValue::List(_)) {
                return Err(QueryError::InvalidFilter {
                    field: field.clone(),
                    op,
                    reason: "In/NotIn operators require a List value".to_string(),
                });
            }
        }
        FilterOp::Exists | FilterOp::NotExists => {
            if !matches!(value, FilterValue::Null) {
                return Err(QueryError::InvalidFilter {
                    field: field.clone(),
                    op,
                    reason: "Exists/NotExists operators require Null value".to_string(),
                });
            }
        }
        FilterOp::Gt | FilterOp::Lt | FilterOp::Gte | FilterOp::Lte => {
            if !matches!(value, FilterValue::Number(_)) {
                return Err(QueryError::InvalidFilter {
                    field: field.clone(),
                    op,
                    reason: "Comparison operators require a Number value".to_string(),
                });
            }
        }
        _ => {}
    }

    if let FilterField::MetadataKey(key) = field {
        if key.is_empty() {
            return Err(QueryError::InvalidFilter {
                field: field.clone(),
                op,
                reason: "MetadataKey cannot be empty".to_string(),
            });
        }
    }

    Ok(())
}

// ─── Query Convenience Builders ────────────────────────────────────

impl Query {
    #[must_use]
    pub fn by_event_type(event_type: &str) -> Self {
        Self::new().with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::Eq,
            FilterValue::String(event_type.to_string()),
        ))
    }

    #[must_use]
    pub fn by_source(source: EventSource) -> Self {
        Self::new().with_filter(QueryFilter::new(
            FilterField::Source,
            FilterOp::Eq,
            FilterValue::String(format!("{:?}", source)),
        ))
    }

    #[must_use]
    pub fn by_session(session_id: &str) -> Self {
        Self::new().with_filter(QueryFilter::new(
            FilterField::SessionId,
            FilterOp::Eq,
            FilterValue::String(session_id.to_string()),
        ))
    }

    #[must_use]
    pub fn by_secret_key(key: &str) -> Self {
        Self::new().with_filter(QueryFilter::new(
            FilterField::SecretKey,
            FilterOp::Eq,
            FilterValue::String(key.to_string()),
        ))
    }
}

// ─── Query Error ───────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("invalid time range: {0}")]
    InvalidTimeRange(String),

    #[error("invalid filter on {field:?} with {op:?}: {reason}")]
    InvalidFilter {
        field: FilterField,
        op: FilterOp,
        reason: String,
    },

    #[error("pagination limit exceeds maximum: {0} > {MAX_LIMIT}")]
    LimitExceeded(u32),

    #[error("query execution failed: {0}")]
    ExecutionFailed(String),

    #[error("log directory not found: {0}")]
    LogDirNotFound(PathBuf),

    #[error("failed to read log file {path}: {source}")]
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse event at line {line} in {path}: {source}")]
    ParseFailed {
        path: PathBuf,
        line: u64,
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_default() {
        let q = Query::default();
        assert!(q.time_range.start.is_none());
        assert!(q.time_range.end.is_none());
        assert!(q.filters.is_empty());
        assert_eq!(q.sort.field, SortField::Timestamp);
        assert_eq!(q.sort.direction, SortDirection::Desc);
        assert_eq!(q.pagination.limit, 100);
        assert_eq!(q.format, OutputFormat::Table);
    }

    #[test]
    fn test_query_builder() {
        let q = Query::new()
            .with_time_range(TimeRange::last_hours(24))
            .with_filter(QueryFilter::new(
                FilterField::EventType,
                FilterOp::Eq,
                FilterValue::String("SecretAccessed".to_string()),
            ))
            .with_format(OutputFormat::Json);

        assert!(q.time_range.start.is_some());
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.format, OutputFormat::Json);
    }

    #[test]
    fn test_query_validate_valid() {
        let q = Query::new();
        assert!(q.validate().is_ok());
    }

    #[test]
    fn test_query_validate_invalid_time_range() {
        let q = Query::new().with_time_range(TimeRange {
            start: Some(Utc::now()),
            end: Some(Utc::now() - Duration::hours(1)),
        });
        assert!(q.validate().is_err());
    }

    #[test]
    fn test_query_validate_limit_exceeded() {
        let mut q = Query::new();
        q.pagination.limit = MAX_LIMIT + 1;
        assert!(q.validate().is_err());
    }

    #[test]
    fn test_query_validate_in_requires_list() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::In,
            FilterValue::String("test".to_string()),
        ));
        assert!(q.validate().is_err());
    }

    #[test]
    fn test_query_validate_in_with_list_ok() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::In,
            FilterValue::List(vec!["a".to_string(), "b".to_string()]),
        ));
        assert!(q.validate().is_ok());
    }

    #[test]
    fn test_query_validate_exists_requires_null() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::SecretKey,
            FilterOp::Exists,
            FilterValue::String("test".to_string()),
        ));
        assert!(q.validate().is_err());
    }

    #[test]
    fn test_query_validate_exists_with_null_ok() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::SecretKey,
            FilterOp::Exists,
            FilterValue::Null,
        ));
        assert!(q.validate().is_ok());
    }

    #[test]
    fn test_time_range_presets() {
        let all = TimeRange::all();
        assert!(all.start.is_none() && all.end.is_none());

        let day = TimeRange::last_days(1);
        assert!(day.start.is_some() && day.end.is_none());
    }

    #[test]
    fn test_pagination_new_clamps() {
        let p = Pagination::new(5_000_000);
        assert_eq!(p.limit, MAX_LIMIT);
    }

    #[test]
    fn test_convenience_builders() {
        let q = Query::by_event_type("SecretAccessed");
        assert_eq!(q.filters.len(), 1);

        let q = Query::by_session("abc-123");
        assert_eq!(q.filters.len(), 1);
        assert!(matches!(q.filters[0].field, FilterField::SessionId));
    }

    #[test]
    fn test_query_id_uniqueness() {
        let a = QueryId::new();
        let b = QueryId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn test_comparison_operators_require_number() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::Gt,
            FilterValue::String("test".to_string()),
        ));
        assert!(q.validate().is_err());
    }

    #[test]
    fn test_comparison_operators_with_number_ok() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::Gt,
            FilterValue::Number(42),
        ));
        assert!(q.validate().is_ok());
    }

    #[test]
    fn test_metadata_key_empty_fails() {
        let q = Query::new().with_filter(QueryFilter::new(
            FilterField::MetadataKey(String::new()),
            FilterOp::Eq,
            FilterValue::String("v".to_string()),
        ));
        assert!(q.validate().is_err());
    }
}
