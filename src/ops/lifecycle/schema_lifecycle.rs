use crate::model::{LifecycleAction, LifecycleRule, LifecycleTrigger, RotationStrategy};
use crate::ops::schema::EnvSchema;

/// Generate lifecycle rules from schema variables that have lifecycle fields set.
///
/// Story 002-auto-rule-generation (Should priority).
pub fn generate_rules_from_schema(schema: &EnvSchema) -> Vec<LifecycleRule> {
    let mut rules = Vec::new();

    for (key, var) in &schema.variables {
        let ttl = match var.ttl_days {
            Some(days) => days,
            None => continue,
        };

        let strategy = var
            .rotation_strategy
            .as_deref()
            .map(parse_strategy)
            .unwrap_or_default();

        let action = if var.auto_rotate.unwrap_or(false) {
            LifecycleAction::Rotate { strategy }
        } else if var.notify_days_before_expiry.is_some() {
            LifecycleAction::Notify {
                message: format!("schema key '{key}' is approaching TTL expiry"),
            }
        } else {
            continue; // No action — skip
        };

        let mut rule = LifecycleRule::new(
            format!("auto-{key}"),
            LifecycleTrigger::AgeExceeded { max_days: ttl },
            action,
        );
        rule.description = format!("Auto-generated from schema for '{key}' — TTL {ttl} days");
        rule.tags = vec!["schema".into(), format!("schema_key:{key}")];
        rules.push(rule);
    }

    rules
}

fn parse_strategy(s: &str) -> RotationStrategy {
    match s {
        "replace" => RotationStrategy::Replace,
        "dual_write" => RotationStrategy::DualWrite,
        "blue_green" => RotationStrategy::BlueGreen,
        "provider_managed" => RotationStrategy::ProviderManaged,
        _ => RotationStrategy::Replace,
    }
}
