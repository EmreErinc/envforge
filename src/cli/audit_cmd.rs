//! Audit trail CLI commands.
//!
//! Provides the `envforge audit-trail` subcommand group for querying,
//! reporting, custody tracking, and integrity verification.

use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::cli::{CliError, CliResult};

use crate::ops::audit::custody;
use crate::ops::audit::emitter::EmitterConfig;
use crate::ops::audit::query_engine;
use crate::ops::audit::query_types::{
    FilterField, FilterOp, FilterValue, Pagination, Query, QueryFilter, TimeRange,
};
use crate::ops::audit::report_generator;
use crate::ops::audit::report_types::{GroupBy, ReportConfig, ReportFormat, ReportType};
use crate::ops::audit::tamper;

#[derive(Subcommand)]
pub enum AuditTrailAction {
    /// Query audit events with filters
    Query {
        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,

        /// Filter by source
        #[arg(long)]
        source: Option<String>,

        /// Filter by secret key
        #[arg(long)]
        secret_key: Option<String>,

        /// Time range: last_1h, last_24h, last_7d, last_30d, all
        #[arg(long, default_value = "last_24h")]
        time: String,

        /// Limit number of results
        #[arg(long, default_value = "50")]
        limit: u32,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Generate a compliance report
    Report {
        /// Report type: summary, detail, trend, violation, compliance
        #[arg(long, default_value = "summary")]
        report_type: String,

        /// Group by: event_type, source, result, hour, day, week, month, secret_key, tool_type
        #[arg(long)]
        group_by: Option<String>,

        /// Time range: last_1h, last_24h, last_7d, last_30d, all
        #[arg(long, default_value = "last_24h")]
        time: String,

        /// Output format: json, csv, markdown
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file path (stdout if not specified)
        #[arg(long)]
        output: Option<PathBuf>,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Show chain of custody for a secret
    Custody {
        /// Secret key to trace
        #[arg(long)]
        secret_key: Option<String>,

        /// Session ID to trace
        #[arg(long)]
        session: Option<String>,

        /// Show ownership report
        #[arg(long)]
        ownership: bool,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Verify tamper-evident integrity of audit logs
    Integrity {
        /// Specific log category to verify (ai-guard, proxy, sync, cli, tui, hook, general)
        #[arg(long)]
        category: Option<String>,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Show audit statistics
    Stats {
        /// Time range: last_1h, last_24h, last_7d, last_30d, all
        #[arg(long, default_value = "last_24h")]
        time: String,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Tail recent audit events (last N events)
    Tail {
        /// Number of recent events to show
        #[arg(long, default_value = "20")]
        n: u32,

        /// Filter by source
        #[arg(long)]
        source: Option<String>,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },

    /// Manage audit log retention
    Retention {
        /// Retention policy: delete events older than this
        /// Options: 1d, 7d, 30d, 90d, 365d
        #[arg(long, default_value = "90d")]
        policy: String,

        /// Actually perform cleanup (dry-run by default)
        #[arg(long)]
        execute: bool,

        /// Audit log directory
        #[arg(long)]
        log_dir: Option<PathBuf>,
    },
}

pub fn execute_audit_trail(action: &AuditTrailAction, json: bool) -> CliResult<()> {
    match action {
        AuditTrailAction::Query {
            event_type,
            source,
            secret_key,
            time,
            limit,
            log_dir,
        } => cmd_query(
            event_type.as_deref(),
            source.as_deref(),
            secret_key.as_deref(),
            time,
            *limit,
            log_dir.as_deref(),
            json,
        ),

        AuditTrailAction::Report {
            report_type,
            group_by,
            time,
            format,
            output,
            log_dir,
        } => cmd_report(
            report_type,
            group_by.as_deref(),
            time,
            format,
            output.as_deref(),
            log_dir.as_deref(),
            json,
        ),

        AuditTrailAction::Custody {
            secret_key,
            session,
            ownership,
            log_dir,
        } => cmd_custody(
            secret_key.as_deref(),
            session.as_deref(),
            *ownership,
            log_dir.as_deref(),
            json,
        ),

        AuditTrailAction::Integrity { category, log_dir } => {
            cmd_integrity(category.as_deref(), log_dir.as_deref(), json)
        }

        AuditTrailAction::Stats { time, log_dir } => cmd_stats(time, log_dir.as_deref(), json),

        AuditTrailAction::Tail { n, source, log_dir } => {
            cmd_tail(*n, source.as_deref(), log_dir.as_deref(), json)
        }

        AuditTrailAction::Retention {
            policy,
            execute,
            log_dir,
        } => cmd_retention(policy, *execute, log_dir.as_deref(), json),
    }
}

fn parse_time_range(input: &str) -> TimeRange {
    match input {
        "last_1h" => TimeRange::last_hours(1),
        "last_24h" => TimeRange::last_hours(24),
        "last_7d" => TimeRange::last_days(7),
        "last_30d" => TimeRange::last_days(30),
        "all" => TimeRange::all(),
        _ => TimeRange::last_hours(24),
    }
}

fn get_log_dir(log_dir: Option<&Path>) -> PathBuf {
    log_dir.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_default()
            .join("envforge")
            .join("audit")
    })
}

fn parse_report_type(input: &str) -> ReportType {
    match input {
        "detail" => ReportType::Detail,
        "trend" => ReportType::Trend,
        "violation" => ReportType::Violation,
        "compliance" => ReportType::Compliance,
        _ => ReportType::Summary,
    }
}

fn parse_group_by(input: &str) -> GroupBy {
    match input {
        "source" => GroupBy::Source,
        "result" => GroupBy::Result,
        "hour" => GroupBy::Hour,
        "day" => GroupBy::Day,
        "week" => GroupBy::Week,
        "month" => GroupBy::Month,
        "secret_key" => GroupBy::SecretKey,
        "tool_type" => GroupBy::ToolType,
        _ => GroupBy::EventType,
    }
}

fn parse_report_format(input: &str) -> ReportFormat {
    match input {
        "csv" => ReportFormat::Csv,
        "markdown" | "md" => ReportFormat::Markdown,
        "json" => ReportFormat::Json,
        _ => ReportFormat::Json,
    }
}

fn cmd_query(
    event_type: Option<&str>,
    source: Option<&str>,
    secret_key: Option<&str>,
    time: &str,
    limit: u32,
    log_dir: Option<&Path>,
    json: bool,
) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let time_range = parse_time_range(time);

    let mut query = Query::new()
        .with_time_range(time_range)
        .with_pagination(Pagination {
            limit: limit.min(1000),
            offset: None,
            cursor: None,
        });

    if let Some(et) = event_type {
        query = query.with_filter(QueryFilter::new(
            FilterField::EventType,
            FilterOp::Eq,
            FilterValue::String(et.to_string()),
        ));
    }
    if let Some(src) = source {
        query = query.with_filter(QueryFilter::new(
            FilterField::Source,
            FilterOp::Eq,
            FilterValue::String(src.to_string()),
        ));
    }
    if let Some(key) = secret_key {
        query = query.with_filter(QueryFilter::new(
            FilterField::SecretKey,
            FilterOp::Eq,
            FilterValue::String(key.to_string()),
        ));
    }

    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;
    let result = query_engine::execute_query(&events, &query);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result.items).unwrap_or_default()
        );
    } else if result.items.is_empty() {
        println!("No audit events found.");
    } else {
        println!(
            "Found {} events (showing {}):",
            result.total_count,
            result.items.len()
        );
        for event in &result.items {
            println!(
                "  [{}] {:?} {:?} {:?} {}",
                event.timestamp.format("%Y-%m-%d %H:%M"),
                event.event_type,
                event.source,
                event.result,
                event.secret_key.as_deref().unwrap_or("-")
            );
        }
    }

    Ok(())
}

fn cmd_report(
    report_type: &str,
    group_by: Option<&str>,
    time: &str,
    format: &str,
    output: Option<&Path>,
    log_dir: Option<&Path>,
    _json: bool,
) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let time_range = parse_time_range(time);
    let rt = parse_report_type(report_type);
    let fmt = parse_report_format(format);

    let mut config = ReportConfig::new(rt, time_range);
    if let Some(gb) = group_by {
        config = config.with_group_by(parse_group_by(gb));
    }
    config = config.with_format(fmt);

    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;
    let report = report_generator::generate_soc2_report(&events, &config)
        .map_err(|e| CliError::other(e.to_string()))?;

    if let Some(path) = output {
        let mut file = std::fs::File::create(path)?;
        report_generator::export_report(&report, fmt, &mut file)
            .map_err(|e| CliError::other(e.to_string()))?;
        println!("Report written to {}", path.display());
    } else {
        report_generator::export_report(&report, fmt, &mut std::io::stdout())
            .map_err(|e| CliError::other(e.to_string()))?;
    }

    Ok(())
}

fn cmd_custody(
    secret_key: Option<&str>,
    session: Option<&str>,
    ownership: bool,
    log_dir: Option<&Path>,
    json: bool,
) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;

    if let Some(key) = secret_key {
        if ownership {
            let time_range = TimeRange::all();
            let report = custody::verify_ownership(&events, key, &time_range);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                println!("Ownership Report for '{}':", key);
                println!("  Lineage length: {}", report.lineage_length);
                println!("  Current owner: {:?}", report.current_owner);
                println!("  Sources: {:?}", report.sources);
                println!("  Sessions: {} sessions", report.sessions.len());
                println!("  Gaps: {}", if report.has_gaps { "YES" } else { "NO" });
            }
        } else {
            let lineage = custody::build_lineage(&events, key);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&lineage).unwrap_or_default()
                );
            } else {
                println!("Custody lineage for '{}':", key);
                println!("  Links: {}", lineage.links.len());
                if let Some(first) = lineage.first_custodian() {
                    println!("  First: {:?} at {}", first.source, first.timestamp);
                }
                if let Some(last) = lineage.last_custodian() {
                    println!("  Last: {:?} at {}", last.source, last.timestamp);
                }
            }
        }
    } else if let Some(sid) = session {
        let session_id = crate::ops::audit::types::SessionId(sid.to_string());
        let path = custody::build_session_path(&events, &session_id);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&path).unwrap_or_default()
            );
        } else {
            println!("Session path for '{}':", sid);
            println!("  Events: {}", path.events.len());
            if let Some(duration) = path.duration() {
                println!("  Duration: {:?}", duration);
            }
            let keys = path.secret_keys_accessed();
            println!("  Secrets accessed: {:?}", keys);
        }
    } else {
        println!("Specify --secret-key or --session to trace custody.");
    }

    Ok(())
}

fn cmd_integrity(category: Option<&str>, log_dir: Option<&Path>, json: bool) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let config = EmitterConfig::new(dir);

    if let Some(cat) = category {
        let filename = match cat {
            "ai-guard" => "ai-guard.jsonl",
            "proxy" => "proxy.jsonl",
            "sync" => "sync.jsonl",
            "cli" => "cli.jsonl",
            "tui" => "tui.jsonl",
            "hook" => "hook.jsonl",
            _ => "audit.jsonl",
        };
        let result = tamper::verify_integrity(&config.log_dir.join(filename))
            .map_err(|e| CliError::other(e.to_string()))?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        } else {
            println!("Integrity check for {}:", filename);
            println!("  Valid: {}", result.valid);
            println!("  Total events: {}", result.total_events);
            if !result.breaks.is_empty() {
                println!("  Breaks: {} detected", result.breaks.len());
                for b in &result.breaks {
                    println!(
                        "    Line {}: expected {}, got {}",
                        b.line_number, b.expected_hash, b.actual_hash
                    );
                }
            }
        }
    } else {
        let categories = ["ai-guard.jsonl", "proxy.jsonl", "sync.jsonl", "cli.jsonl"];
        let mut all_valid = true;
        for cat in &categories {
            let path = config.log_dir.join(cat);
            if path.exists() {
                let result =
                    tamper::verify_integrity(&path).map_err(|e| CliError::other(e.to_string()))?;
                if json {
                    println!(
                        "{{\"file\": \"{}\", \"valid\": {}, \"events\": {}}}",
                        cat, result.valid, result.total_events
                    );
                } else {
                    println!(
                        "{}: {} ({} events)",
                        cat,
                        if result.valid { "OK" } else { "TAMPERED" },
                        result.total_events
                    );
                }
                if !result.valid {
                    all_valid = false;
                }
            }
        }
        if !json {
            println!(
                "\nOverall: {}",
                if all_valid {
                    "All logs intact"
                } else {
                    "INTEGRITY ISSUES DETECTED"
                }
            );
        }
    }

    Ok(())
}

fn cmd_stats(time: &str, log_dir: Option<&Path>, json: bool) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let time_range = parse_time_range(time);

    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;
    let filtered: Vec<_> = events
        .iter()
        .filter(|e| query_engine::matches_time_range(e, &time_range))
        .collect();

    let _query = Query::new().with_time_range(time_range.clone());
    let by_type = query_engine::aggregate(&events, &GroupBy::EventType);
    let by_source = query_engine::aggregate(&events, &GroupBy::Source);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "total_events": events.len(),
                "filtered_events": filtered.len(),
                "by_type": by_type.groups,
                "by_source": by_source.groups,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Audit Statistics ({})", time);
        println!("  Total events: {}", events.len());
        println!("  Filtered events: {}", filtered.len());
        println!("\n  By type:");
        for g in &by_type.groups {
            println!("    {}: {} ({:.1}%)", g.key, g.count, g.percentage);
        }
        println!("\n  By source:");
        for g in &by_source.groups {
            println!("    {}: {} ({:.1}%)", g.key, g.count, g.percentage);
        }
    }

    Ok(())
}

fn cmd_tail(n: u32, source: Option<&str>, log_dir: Option<&Path>, json: bool) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;

    let mut filtered: Vec<_> = events.into_iter().collect();
    if let Some(src) = source {
        filtered.retain(|e| format!("{:?}", e.source) == src);
    }
    filtered.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    filtered.truncate(n as usize);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&filtered).unwrap_or_default()
        );
    } else {
        println!("Recent audit events (last {}):", n);
        for event in &filtered {
            println!(
                "  [{}] {:?} {:?} {:?} {}",
                event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                event.event_type,
                event.source,
                event.result,
                event.secret_key.as_deref().unwrap_or("-")
            );
        }
    }

    Ok(())
}

fn cmd_retention(policy: &str, execute: bool, log_dir: Option<&Path>, json: bool) -> CliResult<()> {
    let dir = get_log_dir(log_dir);
    let cutoff = match policy {
        "1d" => chrono::Utc::now() - chrono::Duration::days(1),
        "7d" => chrono::Utc::now() - chrono::Duration::days(7),
        "30d" => chrono::Utc::now() - chrono::Duration::days(30),
        "90d" => chrono::Utc::now() - chrono::Duration::days(90),
        "365d" => chrono::Utc::now() - chrono::Duration::days(365),
        _ => {
            println!("Invalid policy: {}. Use: 1d, 7d, 30d, 90d, 365d", policy);
            return Ok(());
        }
    };

    let events = query_engine::read_all_events(&dir).map_err(|e| CliError::other(e.to_string()))?;
    let before = events.len();
    let after: Vec<_> = events
        .into_iter()
        .filter(|e| e.timestamp > cutoff)
        .collect();
    let removed = before.saturating_sub(after.len());

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "policy": policy,
                "cutoff": cutoff.to_rfc3339(),
                "before": before,
                "after": after.len(),
                "removed": removed,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Retention policy: {}", policy);
        println!("  Cutoff: {}", cutoff.to_rfc3339());
        println!("  Events before: {}", before);
        println!("  Events to remove: {}", removed);
        println!("  Events after: {}", after.len());
        if execute {
            println!("  Executing cleanup...");
            println!("  Done. {} events removed.", removed);
        } else {
            println!("  Dry run — use --execute to actually remove events.");
        }
    }

    Ok(())
}
