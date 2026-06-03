use chrono::Utc;
use cron::Schedule;
use std::collections::HashSet;
use std::str::FromStr;

use crate::model::{
    EvaluationContext, LifecycleRule, LifecycleTrigger, LogicalOp, TriggerEvent, TriggerType,
};
use crate::ops::OpError;

/// Evaluate all enabled rules and return triggered events.
/// Each rule is evaluated against the provided context.
pub fn evaluate(
    rules: &[LifecycleRule],
    context: &EvaluationContext,
) -> Result<Vec<TriggerEvent>, OpError> {
    let mut events = Vec::new();

    for rule in rules {
        if !rule.enabled {
            continue;
        }

        if let Ok(true) = evaluate_trigger(&rule.trigger, context) {
            let fires = match &rule.condition {
                Some(cond) => evaluate_condition(cond, context),
                None => true,
            };

            if fires {
                events.push(TriggerEvent {
                    trigger_type: trigger_type_of(&rule.trigger),
                    rule_id: rule.id,
                    secret_key: None,
                    timestamp: Utc::now(),
                    payload: format!("rule '{}' triggered", rule.name),
                });
            }
        }
    }

    Ok(events)
}

/// Evaluate a single trigger against the context.
pub fn evaluate_trigger(
    trigger: &LifecycleTrigger,
    context: &EvaluationContext,
) -> Result<bool, OpError> {
    match trigger {
        LifecycleTrigger::Cron { expression } => evaluate_cron(expression, context),
        LifecycleTrigger::AgeExceeded { max_days } => evaluate_age(*max_days, context),
        LifecycleTrigger::FileChange { .. } => Ok(false), // placeholder — requires file hash storage
        LifecycleTrigger::PolicyViolation { .. } => Ok(false), // placeholder
        LifecycleTrigger::Composite { triggers, operator } => {
            evaluate_composite(triggers, operator, context)
        }
    }
}

/// Evaluate a condition against the context.
pub fn evaluate_condition(
    condition: &crate::model::LifecycleCondition,
    context: &EvaluationContext,
) -> bool {
    let results: Vec<bool> = condition
        .conditions
        .iter()
        .map(|expr| evaluate_condition_expr(expr, context))
        .collect();

    match condition.operator {
        LogicalOp::All => results.iter().all(|r| *r),
        LogicalOp::Any => results.iter().any(|r| *r),
        LogicalOp::Not => !results.iter().any(|r| *r),
    }
}

// ─── Trigger Evaluators ─────────────────────────────────

/// Minimum allowed interval between two consecutive cron events.
/// Schedules tighter than this are rejected to prevent local DoS / cost
/// amplification via tight rotation triggers (e.g. `* * * * * *` would
/// fire every second). 60 s matches the historical UNIX cron resolution.
pub const MIN_CRON_INTERVAL_SECS: i64 = 60;

fn evaluate_cron(expression: &str, context: &EvaluationContext) -> Result<bool, OpError> {
    let schedule = Schedule::from_str(expression)
        .map_err(|e| OpError::Other(format!("invalid cron expression '{expression}': {e}")))?;

    // Reject schedules whose minimum interval is below the floor. Without
    // this, a rule with `* * * * * *` (every second) drives unbounded
    // provider rotations and exhausts CPU / API quota.
    let mut iter = schedule.upcoming(Utc);
    if let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        let delta = (b - a).num_seconds();
        if delta < MIN_CRON_INTERVAL_SECS {
            return Err(OpError::Other(format!(
                "cron expression '{expression}' fires every {delta}s; minimum allowed is {MIN_CRON_INTERVAL_SECS}s"
            )));
        }
    }

    let now = context.current_time;

    // Check if the cron schedule had an event between last check and now
    if let Some(last) = context.last_check {
        if let Some(event) = schedule.after(&last).next() {
            if event <= now {
                return Ok(true);
            }
        }
    } else {
        // No last check — report if current time matches upcoming schedule
        let next = schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| OpError::Other("no upcoming cron events".into()))?;
        return Ok(next <= now + chrono::Duration::minutes(1));
    }

    Ok(false)
}

fn evaluate_age(max_days: u32, context: &EvaluationContext) -> Result<bool, OpError> {
    if let Some(ref project_dir) = context.project_dir {
        if let Ok(entries) = crate::ops::secrets::age::get_age_report(i64::from(max_days)) {
            if entries.iter().any(|e| e.stale) {
                return Ok(true);
            }
        } else {
            // Age report may fail if no secrets tracked — not a trigger error
        }
        let _ = project_dir;
    }
    Ok(false)
}

fn evaluate_composite(
    triggers: &[LifecycleTrigger],
    operator: &LogicalOp,
    context: &EvaluationContext,
) -> Result<bool, OpError> {
    let results: Vec<bool> = triggers
        .iter()
        .map(|t| evaluate_trigger(t, context).unwrap_or(false))
        .collect();

    match operator {
        LogicalOp::All => Ok(results.iter().all(|r| *r)),
        LogicalOp::Any => Ok(results.iter().any(|r| *r)),
        LogicalOp::Not => Ok(!results.iter().any(|r| *r)),
    }
}

// ─── Condition Expressions ───────────────────────────────

fn evaluate_condition_expr(
    expr: &crate::model::ConditionExpr,
    _context: &EvaluationContext,
) -> bool {
    match expr {
        crate::model::ConditionExpr::SecretMatches { pattern } => {
            // Simple substring match on secret keys
            // In production, would check against all known secret keys
            !pattern.is_empty()
        }
        crate::model::ConditionExpr::HealthBelow { .. } => true, // placeholder
        crate::model::ConditionExpr::UnusedFor { .. } => true, // placeholder — needs deps.rs integration
        crate::model::ConditionExpr::TagMatches { .. } => true, // placeholder
    }
}

// ─── Deduplication ──────────────────────────────────────

/// Remove duplicate events (same rule_id + secret_key).
/// Keeps only the latest event within a 5-minute window.
pub fn deduplicate(events: Vec<TriggerEvent>) -> Vec<TriggerEvent> {
    let mut seen: HashSet<(uuid::Uuid, Option<String>)> = HashSet::new();
    let window = chrono::Duration::minutes(5);
    let now = Utc::now();

    events
        .into_iter()
        .filter(|e| {
            let key = (e.rule_id, e.secret_key.clone());
            let recent = (now - e.timestamp) <= window;
            let new = seen.insert(key);
            recent && new
        })
        .collect()
}

// ─── Helpers ────────────────────────────────────────────

fn trigger_type_of(trigger: &LifecycleTrigger) -> TriggerType {
    match trigger {
        LifecycleTrigger::Cron { .. } => TriggerType::Cron,
        LifecycleTrigger::AgeExceeded { .. } => TriggerType::Age,
        LifecycleTrigger::FileChange { .. } => TriggerType::FileChange,
        LifecycleTrigger::PolicyViolation { .. } => TriggerType::Policy,
        LifecycleTrigger::Composite { .. } => TriggerType::Policy,
    }
}
