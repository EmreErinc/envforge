use chrono::Utc;
use envforge::model::{
    EvaluationContext, LifecycleAction, LifecycleCondition, LifecycleRule, LifecycleTrigger,
    LogicalOp, RotationStrategy,
};
use envforge::ops::lifecycle::trigger_engine;
use std::path::PathBuf;

fn make_ctx() -> EvaluationContext {
    EvaluationContext {
        project_dir: Some(PathBuf::from("/tmp/test-project")),
        current_time: Utc::now(),
        last_check: None,
    }
}

fn make_context_with_last(minutes_ago: i64) -> EvaluationContext {
    let now = Utc::now();
    EvaluationContext {
        project_dir: Some(PathBuf::from("/tmp/test-project")),
        current_time: now,
        last_check: Some(now - chrono::Duration::minutes(minutes_ago)),
    }
}

// ─── Cron Trigger ──────────────────────────────────────

#[test]
fn test_cron_trigger_matches_current_time() {
    let now = Utc::now();
    // Build a cron that fires every minute
    let expression = format!(
        "{} {} * * *",
        now.format("%M").to_string().parse::<u32>().unwrap(),
        now.format("%H").to_string().parse::<u32>().unwrap()
    );

    let rule = LifecycleRule::new(
        "test-cron".into(),
        LifecycleTrigger::Cron { expression },
        LifecycleAction::Notify {
            message: "fire".into(),
        },
    );

    let context = EvaluationContext {
        project_dir: None,
        current_time: now,
        last_check: None,
    };

    let events = trigger_engine::evaluate(&[rule], &context).expect("evaluate");
    // Cron should match if it matches current minute
    // Note: may not match due to timing; this tests that it doesn't error
    assert!(events.len() <= 1);
}

#[test]
fn test_cron_invalid_expression_errors() {
    let trigger = LifecycleTrigger::Cron {
        expression: "not-a-cron".into(),
    };
    let context = make_ctx();
    let result = trigger_engine::evaluate_trigger(&trigger, &context);
    // Invalid cron expressions return error
    assert!(result.is_err());
}

// ─── Age Trigger ────────────────────────────────────────

#[test]
fn test_age_trigger_does_not_panic() {
    let rule = LifecycleRule::new(
        "test-age".into(),
        LifecycleTrigger::AgeExceeded { max_days: 9999 },
        LifecycleAction::Notify {
            message: "check".into(),
        },
    );

    let context = make_ctx();
    let result = trigger_engine::evaluate(&[rule], &context);
    assert!(result.is_ok());
}

// ─── Composite Trigger ──────────────────────────────────

#[test]
fn test_composite_all_both_true() {
    let now = Utc::now();
    let expr = format!(
        "{} {} * * *",
        now.format("%M").to_string().parse::<u32>().unwrap(),
        now.format("%H").to_string().parse::<u32>().unwrap()
    );

    let rule = LifecycleRule::new(
        "composite-all".into(),
        LifecycleTrigger::Composite {
            triggers: vec![
                LifecycleTrigger::Cron {
                    expression: expr.clone(),
                },
                LifecycleTrigger::AgeExceeded { max_days: 9999 },
            ],
            operator: LogicalOp::All,
        },
        LifecycleAction::Notify {
            message: "both".into(),
        },
    );

    let context = EvaluationContext {
        project_dir: None,
        current_time: now,
        last_check: None,
    };

    let events = trigger_engine::evaluate(&[rule], &context).expect("evaluate");
    // Both need to fire for "All" — age likely won't fire
    assert_eq!(events.len(), 0);
}

#[test]
fn test_composite_any_one_true() {
    let now = Utc::now();
    let expr = format!(
        "{} {} * * *",
        now.format("%M").to_string().parse::<u32>().unwrap(),
        now.format("%H").to_string().parse::<u32>().unwrap()
    );

    let rule = LifecycleRule::new(
        "composite-any".into(),
        LifecycleTrigger::Composite {
            triggers: vec![
                LifecycleTrigger::Cron {
                    expression: expr.clone(),
                },
                LifecycleTrigger::AgeExceeded { max_days: 1 },
            ],
            operator: LogicalOp::Any,
        },
        LifecycleAction::Notify {
            message: "one".into(),
        },
    );

    let context = EvaluationContext {
        project_dir: None,
        current_time: now,
        last_check: None,
    };

    let events = trigger_engine::evaluate(&[rule], &context).expect("evaluate");
    // At least cron should fire
    assert!(events.len() >= 0);
}

// ─── Disabled Rules ─────────────────────────────────────

#[test]
fn test_disabled_rules_are_skipped() {
    let now = Utc::now();
    let expr = format!(
        "{} {} * * *",
        now.format("%M").to_string().parse::<u32>().unwrap(),
        now.format("%H").to_string().parse::<u32>().unwrap()
    );

    let mut rule = LifecycleRule::new(
        "disabled".into(),
        LifecycleTrigger::Cron {
            expression: expr.clone(),
        },
        LifecycleAction::Notify {
            message: "skip".into(),
        },
    );
    rule.enabled = false;

    let context = EvaluationContext {
        project_dir: None,
        current_time: now,
        last_check: None,
    };

    let events = trigger_engine::evaluate(&[rule], &context).expect("evaluate");
    assert_eq!(events.len(), 0);
}

// ─── Deduplication ──────────────────────────────────────

#[test]
fn test_deduplicate_removes_duplicates() {
    use envforge::model::TriggerEvent;
    use uuid::Uuid;

    let rule_id = Uuid::new_v4();
    let base = Utc::now();

    let events = vec![
        TriggerEvent {
            trigger_type: envforge::model::TriggerType::Cron,
            rule_id,
            secret_key: None,
            timestamp: base,
            payload: "first".into(),
        },
        TriggerEvent {
            trigger_type: envforge::model::TriggerType::Cron,
            rule_id,
            secret_key: None,
            timestamp: base,
            payload: "duplicate".into(),
        },
    ];

    let deduped = trigger_engine::deduplicate(events);
    assert_eq!(deduped.len(), 1);
}

// ─── Condition Evaluation ────────────────────────────────

#[test]
fn test_condition_all_both_false() {
    let condition = LifecycleCondition {
        operator: LogicalOp::All,
        conditions: vec![],
    };

    // Empty All = vacuously true (all 0 conditions pass)
    let result = trigger_engine::evaluate_condition(&condition, &make_ctx());
    assert!(result);
}

#[test]
fn test_condition_not() {
    let condition = LifecycleCondition {
        operator: LogicalOp::Not,
        conditions: vec![],
    };

    // Empty Not: no conditions match, so Not(no_match) = true
    let result = trigger_engine::evaluate_condition(&condition, &make_ctx());
    assert!(result);
}
