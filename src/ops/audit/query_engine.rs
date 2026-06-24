//! Query engine for the AI audit trail.
//!
//! Executes [`Query`] requests against audit log data, applying time
//! filters, field filters, sorting, pagination, and aggregation.
//! Reads JSONL log files written by the emitter (unit 002).

use std::path::Path;

use crate::ops::audit::emitter::LogCategory;
use crate::ops::audit::query_types::{
    FilterField, FilterOp, FilterValue, Pagination, Query, QueryError, QueryFilter, SortDirection,
    SortField, SortSpec, TimeRange, MAX_LIMIT,
};
use crate::ops::audit::report_types::{AggregationGroup, AggregationResult, GroupBy};
use crate::ops::audit::types::AuditEvent;

// ─── Query Result ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    pub items: Vec<T>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
    pub has_next: bool,
}

impl<T> QueryResult<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total_count: 0,
            page: 1,
            page_size: MAX_LIMIT,
            has_next: false,
        }
    }

    pub fn total_pages(&self) -> u32 {
        if self.page_size == 0 {
            return 0;
        }
        ((self.total_count as f64) / f64::from(self.page_size)).ceil() as u32
    }
}

// ─── Matching Functions ────────────────────────────────────────────

/// Check if an event's timestamp falls within a time range.
pub fn matches_time_range(event: &AuditEvent, range: &TimeRange) -> bool {
    if let Some(start) = range.start {
        if event.timestamp < start {
            return false;
        }
    }
    if let Some(end) = range.end {
        if event.timestamp > end {
            return false;
        }
    }
    true
}

/// Apply a single filter to an event.
pub fn matches_filter(event: &AuditEvent, filter: &QueryFilter) -> bool {
    let field_value = match &filter.field {
        FilterField::EventType => extract_event_type_value(event),
        FilterField::Source => extract_source_value(event),
        FilterField::Result => extract_result_value(event),
        FilterField::SessionId => extract_session_id_value(event),
        FilterField::ToolType => extract_tool_type_value(event),
        FilterField::SecretKey => extract_secret_key_value(event),
        FilterField::Operation => extract_operation_value(event),
        FilterField::MetadataKey(key) => extract_metadata_value(event, key),
    };

    match filter.op {
        FilterOp::Eq => match (&field_value, &filter.value) {
            (Some(fv), FilterValue::String(sv)) => fv == sv,
            (Some(fv), FilterValue::Number(nv)) => fv.parse::<i64>().ok() == Some(*nv),
            (Some(fv), FilterValue::Bool(bv)) => fv.as_str() == bv.to_string(),
            _ => false,
        },
        FilterOp::Ne => !matches_filter(
            event,
            &QueryFilter {
                field: filter.field.clone(),
                op: FilterOp::Eq,
                value: filter.value.clone(),
            },
        ),
        FilterOp::Contains => match (&field_value, &filter.value) {
            (Some(fv), FilterValue::String(sv)) => fv.contains(sv.as_str()),
            _ => false,
        },
        FilterOp::StartsWith => match (&field_value, &filter.value) {
            (Some(fv), FilterValue::String(sv)) => fv.starts_with(sv.as_str()),
            _ => false,
        },
        FilterOp::EndsWith => match (&field_value, &filter.value) {
            (Some(fv), FilterValue::String(sv)) => fv.ends_with(sv.as_str()),
            _ => false,
        },
        FilterOp::Gt => numeric_comparison(field_value.as_ref(), &filter.value, |a, b| a > b),
        FilterOp::Lt => numeric_comparison(field_value.as_ref(), &filter.value, |a, b| a < b),
        FilterOp::Gte => numeric_comparison(field_value.as_ref(), &filter.value, |a, b| a >= b),
        FilterOp::Lte => numeric_comparison(field_value.as_ref(), &filter.value, |a, b| a <= b),
        FilterOp::In => match &filter.value {
            FilterValue::List(list) => field_value.as_ref().is_some_and(|fv| list.contains(fv)),
            _ => false,
        },
        FilterOp::NotIn => match &filter.value {
            FilterValue::List(list) => field_value.as_ref().map_or(true, |fv| !list.contains(fv)),
            _ => false,
        },
        FilterOp::Exists => field_value.is_some(),
        FilterOp::NotExists => field_value.is_none(),
    }
}

fn extract_event_type_value(event: &AuditEvent) -> Option<String> {
    Some(format!("{:?}", event.event_type))
}

fn extract_source_value(event: &AuditEvent) -> Option<String> {
    Some(format!("{:?}", event.source))
}

fn extract_result_value(event: &AuditEvent) -> Option<String> {
    match &event.result {
        crate::ops::audit::types::EventResult::Success => Some("Success".to_string()),
        crate::ops::audit::types::EventResult::Failure(msg) => Some(format!("Failure:{}", msg)),
        crate::ops::audit::types::EventResult::Denied(msg) => Some(format!("Denied:{}", msg)),
        crate::ops::audit::types::EventResult::Warning(msg) => Some(format!("Warning:{}", msg)),
    }
}

fn extract_session_id_value(event: &AuditEvent) -> Option<String> {
    event.session_id.as_ref().map(|s| s.0.clone())
}

fn extract_tool_type_value(event: &AuditEvent) -> Option<String> {
    event.tool_type.clone()
}

fn extract_secret_key_value(event: &AuditEvent) -> Option<String> {
    event.secret_key.clone()
}

fn extract_operation_value(event: &AuditEvent) -> Option<String> {
    event.operation.clone()
}

fn extract_metadata_value(event: &AuditEvent, key: &str) -> Option<String> {
    if let serde_json::Value::Object(map) = &event.metadata {
        map.get(key).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        })
    } else {
        None
    }
}

fn numeric_comparison(
    field_value: Option<&String>,
    filter_value: &FilterValue,
    cmp: impl Fn(i64, i64) -> bool,
) -> bool {
    match (field_value, filter_value) {
        (Some(fv), FilterValue::Number(nv)) => fv.parse::<i64>().is_ok_and(|n| cmp(n, *nv)),
        _ => false,
    }
}

// ─── Sorting ───────────────────────────────────────────────────────

/// Sort events by a sort specification.
pub fn sort_events(events: &mut [AuditEvent], sort: &SortSpec) {
    events.sort_by(|a, b| {
        let ord = match sort.field {
            SortField::Timestamp => a.timestamp.cmp(&b.timestamp),
            SortField::EventType => {
                format!("{:?}", a.event_type).cmp(&format!("{:?}", b.event_type))
            }
            SortField::Source => format!("{:?}", a.source).cmp(&format!("{:?}", b.source)),
            SortField::Result => result_order(a).cmp(&result_order(b)),
        };
        match sort.direction {
            SortDirection::Asc => ord,
            SortDirection::Desc => ord.reverse(),
        }
    });
}

fn result_order(event: &AuditEvent) -> u8 {
    match &event.result {
        crate::ops::audit::types::EventResult::Success => 0,
        crate::ops::audit::types::EventResult::Warning(_) => 1,
        crate::ops::audit::types::EventResult::Failure(_) => 2,
        crate::ops::audit::types::EventResult::Denied(_) => 3,
    }
}

// ─── Pagination ────────────────────────────────────────────────────

/// Apply pagination to a sorted list of events.
pub fn paginate(events: &[AuditEvent], pagination: &Pagination) -> QueryResult<AuditEvent> {
    let total_count = events.len() as u64;
    let offset = pagination.offset.unwrap_or(0) as usize;
    let limit = pagination.limit as usize;

    let page = (offset / limit) as u32 + 1;
    let end = (offset + limit).min(events.len());

    let items: Vec<AuditEvent> = if offset < events.len() {
        events[offset..end].to_vec()
    } else {
        Vec::new()
    };

    let has_next = end < events.len();

    QueryResult {
        items,
        total_count,
        page,
        page_size: pagination.limit,
        has_next,
    }
}

// ─── Execute Query ─────────────────────────────────────────────────

/// Execute a query against a slice of AuditEvents.
pub fn execute_query(events: &[AuditEvent], query: &Query) -> QueryResult<AuditEvent> {
    let mut filtered: Vec<AuditEvent> = events
        .iter()
        .filter(|event| matches_time_range(event, &query.time_range))
        .filter(|event| query.filters.iter().all(|f| matches_filter(event, f)))
        .cloned()
        .collect();

    sort_events(&mut filtered, &query.sort);
    paginate(&filtered, &query.pagination)
}

/// Execute a query against log files in a directory.
pub fn execute_query_on_files(
    query: &Query,
    log_dir: &Path,
) -> Result<QueryResult<AuditEvent>, QueryError> {
    let events = read_all_events(log_dir)?;
    Ok(execute_query(&events, query))
}

// ─── Aggregation ──────────────────────────────────────────────────

/// Aggregate events by a dimension, returning group counts and percentages.
pub fn aggregate(events: &[AuditEvent], group_by: &GroupBy) -> AggregationResult {
    use std::collections::HashMap;

    let time_range = compute_time_range(events);
    let mut counts: HashMap<String, u64> = HashMap::new();

    for event in events {
        let key = extract_group_key(event, *group_by);
        *counts.entry(key).or_insert(0) += 1;
    }

    let total_events: u64 = counts.values().sum();
    let mut groups: Vec<AggregationGroup> = counts
        .into_iter()
        .map(|(key, count)| {
            let percentage = if total_events > 0 {
                (count as f64 / total_events as f64) * 100.0
            } else {
                0.0
            };
            AggregationGroup {
                key,
                count,
                percentage,
            }
        })
        .collect();

    groups.sort_by_key(|g| std::cmp::Reverse(g.count));

    AggregationResult {
        group_by: *group_by,
        groups,
        total_events,
        time_range,
    }
}

fn extract_group_key(event: &AuditEvent, group_by: GroupBy) -> String {
    match group_by {
        GroupBy::EventType => format!("{:?}", event.event_type),
        GroupBy::Source => format!("{:?}", event.source),
        GroupBy::Result => match &event.result {
            crate::ops::audit::types::EventResult::Success => "Success".to_string(),
            crate::ops::audit::types::EventResult::Failure(_) => "Failure".to_string(),
            crate::ops::audit::types::EventResult::Denied(_) => "Denied".to_string(),
            crate::ops::audit::types::EventResult::Warning(_) => "Warning".to_string(),
        },
        GroupBy::Hour => event.timestamp.format("%Y-%m-%dT%H:00").to_string(),
        GroupBy::Day => event.timestamp.format("%Y-%m-%d").to_string(),
        GroupBy::Week => event.timestamp.format("%Y-W%W").to_string(),
        GroupBy::Month => event.timestamp.format("%Y-%m").to_string(),
        GroupBy::SecretKey => event.secret_key.clone().unwrap_or_default(),
        GroupBy::ToolType => event
            .tool_type
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

fn compute_time_range(events: &[AuditEvent]) -> TimeRange {
    if events.is_empty() {
        return TimeRange::all();
    }
    let start = events.iter().map(|e| e.timestamp).min();
    let end = events.iter().map(|e| e.timestamp).max();
    TimeRange { start, end }
}

// ─── Log Reader ────────────────────────────────────────────────────

/// Read all events from all JSONL log files in a directory.
/// Hard cap on the number of events loaded from disk in a single
/// query. With long-running installs, audit logs grow without bound;
/// loading a multi-million-entry log into a `Vec<AuditEvent>` exhausts
/// memory before the filter pipeline ever runs. When the cap is hit,
/// the call returns the events read so far so the user gets *something*
/// useful, plus an `eprintln!` so the truncation is visible.
pub const MAX_EVENTS_LOADED: usize = 250_000;

pub fn read_all_events(log_dir: &Path) -> Result<Vec<AuditEvent>, QueryError> {
    let mut events = Vec::new();

    if !log_dir.exists() {
        return Err(QueryError::LogDirNotFound(log_dir.to_path_buf()));
    }

    for category in LogCategory::all() {
        let path = log_dir.join(category.filename());
        if path.exists() {
            let remaining = MAX_EVENTS_LOADED.saturating_sub(events.len());
            if remaining == 0 {
                eprintln!(
                    "audit query: hit MAX_EVENTS_LOADED ({}); results truncated. \
                     Use --since / --until to narrow the window.",
                    MAX_EVENTS_LOADED
                );
                break;
            }
            let category_events = read_events_file_capped(&path, remaining)?;
            events.extend(category_events);
        }
    }

    events.sort_by_key(|e| e.timestamp);
    Ok(events)
}

/// Read events from a specific log category.
pub fn read_events_by_category(
    log_dir: &Path,
    category: &LogCategory,
) -> Result<Vec<AuditEvent>, QueryError> {
    let path = log_dir.join(category.filename());
    if !path.exists() {
        return Ok(Vec::new());
    }
    read_events_file(&path)
}

fn read_events_file(path: &Path) -> Result<Vec<AuditEvent>, QueryError> {
    read_events_file_capped(path, usize::MAX)
}

/// Read events from a JSONL log file, line-by-line, stopping after
/// `cap` events. Streams through `BufRead::lines` instead of loading
/// the entire file into one `String` so a single 1 GiB log file does
/// not require 1 GiB resident memory just to be parsed.
fn read_events_file_capped(path: &Path, cap: usize) -> Result<Vec<AuditEvent>, QueryError> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).map_err(|e| QueryError::ReadFailed {
        path: path.to_path_buf(),
        source: e,
    })?;
    let reader = std::io::BufReader::new(file);

    let mut events = Vec::new();
    for (idx, line_result) in reader.lines().enumerate() {
        if events.len() >= cap {
            break;
        }
        let line = line_result.map_err(|e| QueryError::ReadFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event: AuditEvent =
            serde_json::from_str(line).map_err(|e| QueryError::ParseFailed {
                path: path.to_path_buf(),
                line: (idx + 1) as u64,
                source: e,
            })?;
        events.push(event);
    }

    Ok(events)
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::audit::emitter::EmitterConfig;
    use crate::ops::audit::types::{EventResult, EventSource, EventType};

    fn test_event_at(source: EventSource, event_type: EventType, hours_ago: i64) -> AuditEvent {
        let mut event = AuditEvent::new(event_type, source, EventResult::Success);
        event.timestamp = chrono::Utc::now() - chrono::Duration::hours(hours_ago);
        event
    }

    fn test_events() -> Vec<AuditEvent> {
        vec![
            test_event_at(EventSource::AiGuard, EventType::SecretAccessed, 1),
            test_event_at(EventSource::AiGuard, EventType::SecretBound, 2),
            test_event_at(EventSource::Proxy, EventType::AccessDenied, 3),
            test_event_at(EventSource::Cli, EventType::ConfigChange, 4),
            test_event_at(EventSource::Sync, EventType::SyncPush, 5),
        ]
    }

    // ─── Time Range Tests ──────────────────────────────────────

    #[test]
    fn test_matches_time_range_event_in_range() {
        let event = &test_events()[0];
        let range = TimeRange::last_hours(2);
        assert!(matches_time_range(event, &range));
    }

    #[test]
    fn test_matches_time_range_event_outside_range() {
        let event = &test_events()[4]; // 5 hours ago
        let range = TimeRange::last_hours(3);
        assert!(!matches_time_range(event, &range));
    }

    #[test]
    fn test_matches_time_range_all_time() {
        let event = &test_events()[0];
        assert!(matches_time_range(event, &TimeRange::all()));
    }

    // ─── Filter Tests ──────────────────────────────────────────

    #[test]
    fn test_matches_filter_event_type_eq() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::EventType,
            FilterOp::Eq,
            FilterValue::String("SecretAccessed".to_string()),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_event_type_ne() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::EventType,
            FilterOp::Ne,
            FilterValue::String("SecretBound".to_string()),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_source_eq() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::Source,
            FilterOp::Eq,
            FilterValue::String("AiGuard".to_string()),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_contains() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::EventType,
            FilterOp::Contains,
            FilterValue::String("Secret".to_string()),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_starts_with() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::EventType,
            FilterOp::StartsWith,
            FilterValue::String("Secret".to_string()),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_in_list() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::Source,
            FilterOp::In,
            FilterValue::List(vec!["AiGuard".to_string(), "Proxy".to_string()]),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_not_in_list() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::Source,
            FilterOp::NotIn,
            FilterValue::List(vec!["Proxy".to_string(), "Cli".to_string()]),
        );
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_exists() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(FilterField::EventType, FilterOp::Exists, FilterValue::Null);
        assert!(matches_filter(event, &filter));
    }

    #[test]
    fn test_matches_filter_not_exists() {
        let event = &test_events()[0];
        let filter = QueryFilter::new(
            FilterField::ToolType,
            FilterOp::NotExists,
            FilterValue::Null,
        );
        assert!(matches_filter(event, &filter)); // tool_type is None
    }

    #[test]
    fn test_matches_filter_metadata() {
        let mut event = test_events()[0].clone();
        event.add_metadata("env", serde_json::Value::String("production".to_string()));
        let filter = QueryFilter::new(
            FilterField::MetadataKey("env".to_string()),
            FilterOp::Eq,
            FilterValue::String("production".to_string()),
        );
        assert!(matches_filter(&event, &filter));
    }

    // ─── Sort Tests ────────────────────────────────────────────

    #[test]
    fn test_sort_events_by_timestamp_desc() {
        let mut events = test_events();
        sort_events(
            &mut events,
            &SortSpec {
                field: SortField::Timestamp,
                direction: SortDirection::Desc,
            },
        );
        assert!(events[0].timestamp > events[1].timestamp);
    }

    #[test]
    fn test_sort_events_by_timestamp_asc() {
        let mut events = test_events();
        sort_events(
            &mut events,
            &SortSpec {
                field: SortField::Timestamp,
                direction: SortDirection::Asc,
            },
        );
        assert!(events[0].timestamp < events[1].timestamp);
    }

    #[test]
    fn test_sort_events_by_event_type() {
        let mut events = test_events();
        sort_events(
            &mut events,
            &SortSpec {
                field: SortField::EventType,
                direction: SortDirection::Asc,
            },
        );
        for i in 0..events.len().saturating_sub(1) {
            assert!(
                format!("{:?}", events[i].event_type) <= format!("{:?}", events[i + 1].event_type)
            );
        }
    }

    // ─── Pagination Tests ──────────────────────────────────────

    #[test]
    fn test_paginate_first_page() {
        let events = test_events();
        let pagination = Pagination::new(2);
        let result = paginate(&events, &pagination);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.total_count, 5);
        assert_eq!(result.page, 1);
        assert!(result.has_next);
    }

    #[test]
    fn test_paginate_second_page() {
        let events = test_events();
        let pagination = Pagination::new(2).with_offset(2);
        let result = paginate(&events, &pagination);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.page, 2);
        assert!(result.has_next);
    }

    #[test]
    fn test_paginate_last_page() {
        let events = test_events();
        let pagination = Pagination::new(2).with_offset(4);
        let result = paginate(&events, &pagination);
        assert_eq!(result.items.len(), 1);
        assert!(!result.has_next);
    }

    #[test]
    fn test_paginate_empty() {
        let events: Vec<AuditEvent> = Vec::new();
        let pagination = Pagination::new(10);
        let result = paginate(&events, &pagination);
        assert_eq!(result.items.len(), 0);
        assert_eq!(result.total_count, 0);
    }

    // ─── Execute Query Tests ────────────────────────────────────

    #[test]
    fn test_execute_query_no_filters() {
        let events = test_events();
        let query = Query::new();
        let result = execute_query(&events, &query);
        assert_eq!(result.total_count, 5);
    }

    #[test]
    fn test_execute_query_time_filter() {
        let events = test_events();
        // last_hours(3) means start=3h ago, end=None
        // Events are at 1h, 2h, 3h ago — the 3h-old one may be right at the boundary
        let query = Query::new().with_time_range(TimeRange::last_hours(4));
        let result = execute_query(&events, &query);
        assert!(result.total_count >= 3); // At minimum the 1h, 2h, 3h events
    }

    #[test]
    fn test_execute_query_source_filter() {
        let events = test_events();
        let query = Query::new().with_filter(QueryFilter::new(
            FilterField::Source,
            FilterOp::Eq,
            FilterValue::String("AiGuard".to_string()),
        ));
        let result = execute_query(&events, &query);
        assert_eq!(result.total_count, 2); // AiGuard appears twice
    }

    #[test]
    fn test_execute_query_with_pagination() {
        let events = test_events();
        let query = Query::new().with_pagination(Pagination::new(2));
        let result = execute_query(&events, &query);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.total_count, 5);
    }

    // ─── Aggregation Tests ─────────────────────────────────────

    #[test]
    fn test_aggregate_by_source() {
        let events = test_events();
        let result = aggregate(&events, &GroupBy::Source);
        assert_eq!(result.total_events, 5);
        assert_eq!(result.groups.len(), 4); // AiGuard, Proxy, Cli, Sync
    }

    #[test]
    fn test_aggregate_by_event_type() {
        let events = test_events();
        let result = aggregate(&events, &GroupBy::EventType);
        assert_eq!(result.total_events, 5);
        assert_eq!(result.groups.len(), 5); // each type appears once
    }

    #[test]
    fn test_aggregate_empty() {
        let events: Vec<AuditEvent> = Vec::new();
        let result = aggregate(&events, &GroupBy::Source);
        assert_eq!(result.total_events, 0);
        assert!(result.groups.is_empty());
    }

    #[test]
    fn test_aggregate_percentages_sum_to_100() {
        let events = test_events();
        let result = aggregate(&events, &GroupBy::Source);
        let total_pct: f64 = result.groups.iter().map(|g| g.percentage).sum();
        assert!((total_pct - 100.0).abs() < 0.01);
    }

    // ─── Query Result Tests ────────────────────────────────────

    #[test]
    fn test_query_result_empty() {
        let result: QueryResult<AuditEvent> = QueryResult::empty();
        assert_eq!(result.total_count, 0);
        assert_eq!(result.page, 1);
    }

    #[test]
    fn test_query_result_total_pages() {
        let result: QueryResult<AuditEvent> = QueryResult {
            items: vec![],
            total_count: 25,
            page: 1,
            page_size: 10,
            has_next: true,
        };
        assert_eq!(result.total_pages(), 3);
    }

    // ─── Log Reader Tests ────────────────────────────────────────

    #[test]
    fn test_read_all_events_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let events = read_all_events(dir.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_read_all_events_nonexistent_dir() {
        let result = read_all_events(Path::new("/tmp/nonexistent_dir_for_test"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_and_query_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = EmitterConfig::new(dir.path().to_path_buf());
        let mut state = crate::ops::audit::tamper::ChainState::new();

        let events = test_events();
        for event in events {
            crate::ops::audit::tamper::write_tamper_evident(event, &config, &mut state).unwrap();
        }

        let query = Query::new().with_filter(QueryFilter::new(
            FilterField::Source,
            FilterOp::Eq,
            FilterValue::String("AiGuard".to_string()),
        ));

        let result = execute_query_on_files(&query, dir.path()).unwrap();
        assert_eq!(result.total_count, 2);
    }
}
