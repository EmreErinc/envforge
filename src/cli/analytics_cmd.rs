use clap::Subcommand;
use serde_json::json;

use crate::config::*;
use crate::ops::analytics::*;

#[derive(Subcommand)]
pub enum AnalyticsAction {
    /// Detect unused (dormant) secrets with no access in N days
    Unused {
        /// Days threshold (default: 90)
        #[arg(long, default_value = "90")]
        threshold: u32,
    },

    /// Detect secrets with low usage below threshold
    LowUsage {
        /// Maximum access count threshold (default: 5)
        #[arg(long, default_value = "5")]
        max_accesses: u64,

        /// Time window in days (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,
    },

    /// Show deprecation recommendations
    Deprecation,

    /// Show analytics summary (event count, secret count)
    Summary {
        /// Time window in days (default: 7)
        #[arg(long, default_value = "7")]
        days: u32,
    },

    /// Recompute aggregates from raw events
    Recompute,

    /// Show or set data retention configuration
    Retention {
        #[command(subcommand)]
        action: RetentionAction,
    },

    /// Prune raw events older than a date
    Prune {
        /// Remove events before this date (ISO 8601)
        #[arg(long)]
        before: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum RetentionAction {
    /// Show current retention settings
    Show,

    /// Set retention days
    Set {
        /// Number of days to retain raw events
        #[arg(long)]
        days: u32,
    },
}

pub fn execute_analytics(
    action: &AnalyticsAction,
    json: bool,
    _dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        AnalyticsAction::Unused { threshold } => cmd_unused(*threshold, json),
        AnalyticsAction::LowUsage { max_accesses, days } => {
            cmd_low_usage(*max_accesses, *days, json)
        }
        AnalyticsAction::Deprecation => cmd_deprecation(json),
        AnalyticsAction::Summary { days: _ } => cmd_summary(json),
        AnalyticsAction::Recompute => cmd_recompute(json),
        AnalyticsAction::Retention { action } => match action {
            RetentionAction::Show => cmd_retention_show(json),
            RetentionAction::Set { days } => cmd_retention_set(*days, json),
        },
        AnalyticsAction::Prune { before } => cmd_prune(before.clone(), json),
    }
}

fn cmd_unused(threshold: u32, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let events = load_events()?;
    let dormant = unused::detect_dormant(&events, threshold);

    if json {
        println!("{}", serde_json::to_string_pretty(&dormant)?);
    } else if dormant.is_empty() {
        println!(
            "No unused secrets detected (threshold: {} days).",
            threshold
        );
    } else {
        println!("Unused secrets (no access in {} days):\n", threshold);
        for secret in &dormant {
            println!(
                "  {} — {} (confidence: {:.0}%)",
                secret.secret_name,
                secret.reason,
                secret.confidence * 100.0
            );
        }
    }
    Ok(())
}

fn cmd_low_usage(
    max_accesses: u64,
    days: u32,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let events = load_events()?;
    let low = unused::detect_low_usage(&events, max_accesses, days);

    if json {
        println!("{}", serde_json::to_string_pretty(&low)?);
    } else if low.is_empty() {
        println!(
            "No low-usage secrets detected (threshold: {} accesses in {} days).",
            max_accesses, days
        );
    } else {
        println!(
            "Low-usage secrets (below {} accesses in {} days):\n",
            max_accesses, days
        );
        for secret in &low {
            println!(
                "  {} — {} accesses (threshold: {})",
                secret.secret_name, secret.access_count, secret.threshold
            );
        }
    }
    Ok(())
}

fn cmd_deprecation(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let events = load_events()?;
    let dormant = unused::detect_dormant(&events, 90);
    let low = unused::detect_low_usage(&events, 5, 30);
    let recs = unused::generate_recommendations(&dormant, &low);

    if json {
        println!("{}", serde_json::to_string_pretty(&recs)?);
    } else if recs.is_empty() {
        println!("No deprecation recommendations.");
    } else {
        println!("Deprecation recommendations:\n");
        for rec in &recs {
            println!("  {} — {}", rec.secret_name, rec.reason);
            println!(
                "    Review by: {}, Deprecate by: {}, Remove by: {}",
                rec.timeline.review_by.format("%Y-%m-%d"),
                rec.timeline.deprecate_by.format("%Y-%m-%d"),
                rec.timeline.remove_by.format("%Y-%m-%d"),
            );
            println!(
                "    Confidence: {:.0}%, Dependents: {}",
                rec.unused.confidence * 100.0,
                rec.dependent_count
            );
            println!();
        }
    }
    Ok(())
}

fn cmd_summary(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let summary = aggregation::recompute(&config.analytics)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Analytics Summary:\n");
        println!("  Total secrets:    {}", summary.total_secrets);
        println!("  Total events:     {}", summary.total_events);
        println!("  Active secrets:   {}", summary.active_count);
        println!();
        println!("  Config:");
        println!("    enabled:        {}", config.analytics.enabled);
        println!("    retention_days: {}", config.analytics.retention_days);
        println!("    max_events:     {}", config.analytics.max_events);
        println!("    auto_aggregate: {}", config.analytics.auto_aggregate);
    }
    Ok(())
}

fn cmd_recompute(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let summary = aggregation::recompute(&config.analytics)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "ok",
                "total_secrets": summary.total_secrets,
                "total_events": summary.total_events,
                "message": "Aggregates recomputed successfully"
            }))?
        );
    } else {
        println!("Aggregates recomputed successfully.");
        println!("  Total secrets: {}", summary.total_secrets);
        println!("  Total events:  {}", summary.total_events);
    }
    Ok(())
}

fn cmd_retention_show(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "enabled": config.analytics.enabled,
                "retention_days": config.analytics.retention_days,
                "max_events": config.analytics.max_events,
                "auto_aggregate": config.analytics.auto_aggregate,
            }))?
        );
    } else {
        println!("Retention Configuration:\n");
        println!("  Analytics enabled: {}", config.analytics.enabled);
        println!("  Retention days:    {}", config.analytics.retention_days);
        println!("  Max events:        {}", config.analytics.max_events);
        println!("  Auto-aggregate:    {}", config.analytics.auto_aggregate);
    }
    Ok(())
}

fn cmd_retention_set(days: u32, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_or_create_default()?;
    config.analytics.retention_days = days;

    let config_path = config_file_path()?;
    save_config(&config, &config_path)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "ok",
                "retention_days": days,
            }))?
        );
    } else {
        println!("Retention days set to {}.", days);
    }
    Ok(())
}

fn cmd_prune(before: Option<String>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let retention_days = config.analytics.retention_days;

    if let Some(date_str) = &before {
        // Parse the date string and prune manually
        let cutoff = chrono::DateTime::parse_from_rfc3339(date_str)
            .or_else(|_| {
                chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map(|d| {
                    d.and_hms_opt(0, 0, 0)
                        .expect("midnight time 00:00:00 is always valid")
                        .and_utc()
                        .into()
                })
            })
            .map_err(|e| crate::model::AnalyticsError::InvalidTimeWindow {
                description: format!("Invalid date '{}': {}", date_str, e),
            })?;

        let cutoff_utc = cutoff.with_timezone(&chrono::Utc);
        let mut events = storage::load_events()?;
        let before_count = events.len();
        events.retain(|e| e.enriched_at >= cutoff_utc);
        storage::save_events(&events, &config.analytics)?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "ok",
                    "pruned": before_count - events.len(),
                    "remaining": events.len(),
                }))?
            );
        } else {
            println!(
                "Pruned {} events. {} remaining.",
                before_count - events.len(),
                events.len()
            );
        }
    } else {
        // Prune based on retention_days
        let mut events = storage::load_events()?;
        let before_count = events.len();
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(retention_days));
        events.retain(|e| e.enriched_at >= cutoff);
        storage::save_events(&events, &config.analytics)?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "ok",
                    "pruned": before_count - events.len(),
                    "remaining": events.len(),
                }))?
            );
        } else {
            println!(
                "Pruned {} events (retention: {} days). {} remaining.",
                before_count - events.len(),
                retention_days,
                events.len()
            );
        }
    }
    Ok(())
}
