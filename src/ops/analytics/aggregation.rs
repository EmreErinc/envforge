use std::collections::{HashMap, HashSet};
use std::io::Write;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc};

use crate::model::{
    AggregateBucket, AggregatePeriod, AnalyticsConfig, AnalyticsError, AnalyticsSummary,
};

/// Aggregate events into time buckets by key and period.
pub fn aggregate(
    events: &[crate::model::EnrichedAccessEvent],
    period: &AggregatePeriod,
) -> Vec<AggregateBucket> {
    let mut buckets: HashMap<String, (u64, HashSet<String>)> = HashMap::new();

    for event in events {
        let period_start = truncate_to_period(event.enriched_at, period);
        let key = bucket_key(&event.raw.secret_name, &period_start, period);

        let entry = buckets.entry(key).or_insert_with(|| (0, HashSet::new()));
        entry.0 += 1;
        entry.1.insert(event.raw.accessor.id.clone());
    }

    let mut result: Vec<AggregateBucket> = buckets
        .into_iter()
        .map(|(key, (count, accessors))| {
            let parts: Vec<&str> = key.rsplitn(2, ':').collect();
            let period_start = if parts.len() == 2 {
                DateTime::parse_from_rfc3339(parts[0])
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now())
            } else {
                Utc::now()
            };

            AggregateBucket {
                key,
                period_start,
                period_type: period.clone(),
                access_count: count,
                unique_accessors: accessors.len() as u64,
            }
        })
        .collect();

    result.sort_by_key(|b| std::cmp::Reverse(b.period_start));
    result
}

/// Load stored aggregates from aggregates.jsonl.
pub fn load_aggregates() -> Result<Vec<AggregateBucket>, AnalyticsError> {
    let dir = crate::config::config_dir().map_err(|e| AnalyticsError::StorageError {
        path: std::path::PathBuf::from("config_dir"),
        source: std::io::Error::other(e.to_string()),
    })?;
    let path = dir.join("analytics").join("aggregates.jsonl");

    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&path).map_err(|e| AnalyticsError::StorageError {
        path: path.clone(),
        source: e,
    })?;

    let buckets: Vec<AggregateBucket> = contents
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    Ok(buckets)
}

/// Full recompute: re-aggregate all raw events and replace aggregates.jsonl.
pub fn recompute(config: &AnalyticsConfig) -> Result<AnalyticsSummary, AnalyticsError> {
    use crate::ops::analytics::collector;

    let events = collector::collect_all()?;

    if !events.is_empty() {
        let buckets = aggregate(&events, &AggregatePeriod::Daily);
        save_aggregates(&buckets)?;
    }

    // Prune raw events older than retention_days
    prune_raw_events(config)?;

    Ok(compute_summary(&events))
}

/// Save aggregate buckets to aggregates.jsonl (atomic write).
pub fn save_aggregates(buckets: &[AggregateBucket]) -> Result<(), AnalyticsError> {
    let dir = crate::config::config_dir().map_err(|e| AnalyticsError::StorageError {
        path: std::path::PathBuf::from("config_dir"),
        source: std::io::Error::other(e.to_string()),
    })?;
    let analytics_dir = dir.join("analytics");
    std::fs::create_dir_all(&analytics_dir).map_err(|e| AnalyticsError::StorageError {
        path: analytics_dir.clone(),
        source: e,
    })?;

    let path = analytics_dir.join("aggregates.jsonl");

    // Atomic write via tempfile + rename
    let parent = analytics_dir;
    let mut tmp =
        tempfile::NamedTempFile::new_in(&parent).map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        })?;

    for bucket in buckets {
        let json = serde_json::to_string(bucket)
            .map_err(|e| AnalyticsError::EventParseError { source: e })?;
        writeln!(tmp, "{}", json).map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        })?;
    }

    tmp.persist(&path)
        .map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e.error,
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }

    Ok(())
}

/// Compute an AnalyticsSummary from raw events.
fn compute_summary(events: &[crate::model::EnrichedAccessEvent]) -> AnalyticsSummary {
    let mut secret_names = HashSet::new();
    for event in events {
        secret_names.insert(event.raw.secret_name.clone());
    }

    AnalyticsSummary {
        total_secrets: secret_names.len() as u64,
        total_events: events.len() as u64,
        unused_count: 0,
        dormant_count: 0,
        active_count: secret_names.len() as u64,
        estimated_monthly_cost: 0.0,
    }
}

/// Generate a bucket key from secret name, period start, and period type.
pub fn bucket_key(
    secret_name: &str,
    period_start: &DateTime<Utc>,
    period: &AggregatePeriod,
) -> String {
    format!("{}:{}:{:?}", secret_name, period_start.to_rfc3339(), period)
}

/// Truncate a DateTime to the start of the given period.
fn truncate_to_period(dt: DateTime<Utc>, period: &AggregatePeriod) -> DateTime<Utc> {
    match period {
        AggregatePeriod::Hourly => {
            let naive = dt.naive_utc();
            let truncated = NaiveDateTime::new(
                naive.date(),
                chrono::NaiveTime::from_hms_opt(naive.hour(), 0, 0).unwrap(),
            );
            DateTime::from_naive_utc_and_offset(truncated, Utc)
        }
        AggregatePeriod::Daily => {
            let naive = dt.naive_utc();
            let truncated = NaiveDateTime::new(
                naive.date(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            );
            DateTime::from_naive_utc_and_offset(truncated, Utc)
        }
        AggregatePeriod::Weekly => {
            let naive = dt.naive_utc();
            let weekday = naive.date().weekday();
            let days_from_mon = weekday.num_days_from_monday();
            let monday = naive.date() - Duration::days(i64::from(days_from_mon));
            let truncated =
                NaiveDateTime::new(monday, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
            DateTime::from_naive_utc_and_offset(truncated, Utc)
        }
        AggregatePeriod::Monthly => {
            let naive = dt.naive_utc();
            let first_of_month = NaiveDate::from_ymd_opt(naive.year(), naive.month(), 1).unwrap();
            let truncated = NaiveDateTime::new(
                first_of_month,
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            );
            DateTime::from_naive_utc_and_offset(truncated, Utc)
        }
    }
}

/// Prune raw events older than retention_days from events.jsonl.
fn prune_raw_events(config: &AnalyticsConfig) -> Result<(), AnalyticsError> {
    use crate::ops::analytics::storage;

    let mut events = storage::load_events()?;
    if events.is_empty() {
        return Ok(());
    }

    let cutoff = Utc::now() - Duration::days(i64::from(config.retention_days));
    events.retain(|e| e.enriched_at >= cutoff);
    storage::save_events(&events, config)?;

    Ok(())
}
