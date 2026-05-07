use envforge::model::{
    LifecycleAction, LifecycleCondition, LifecycleError, LifecycleRule, LifecycleState,
    LifecycleTrigger, LogicalOp, RotationPolicy, RotationStrategy, SecretGenerator,
    SecretLifecycle, SecretTemplate, StateTransition, TriggerType,
};

// ─── LifecycleRule Construction ────────────────────────

#[test]
fn test_lifecycle_rule_new_has_default_values() {
    let rule = LifecycleRule::new(
        "test-rule".into(),
        LifecycleTrigger::AgeExceeded { max_days: 90 },
        LifecycleAction::Rotate {
            strategy: RotationStrategy::Replace,
        },
    );

    assert_eq!(rule.name, "test-rule");
    assert!(rule.description.is_empty());
    assert!(rule.enabled);
    assert!(rule.condition.is_none());
    assert!(!rule.id.is_nil());
}

#[test]
fn test_lifecycle_rule_with_condition() {
    let mut rule = LifecycleRule::new(
        "conditional".into(),
        LifecycleTrigger::Cron {
            expression: "0 0 * * *".into(),
        },
        LifecycleAction::Notify {
            message: "check secrets".into(),
        },
    );

    rule.condition = Some(LifecycleCondition {
        operator: LogicalOp::All,
        conditions: vec![],
    });

    assert!(rule.condition.is_some());
}

// ─── SecretLifecycle Construction ───────────────────────

#[test]
fn test_secret_lifecycle_new_defaults_to_active() {
    let lc = SecretLifecycle::new("DATABASE_URL".into());

    assert_eq!(lc.key, "DATABASE_URL");
    assert_eq!(lc.state, LifecycleState::Active);
    assert!(lc.history.is_empty());
    assert_eq!(lc.rotation_count, 0);
    assert!(lc.last_rotation.is_none());
    assert!(lc.expiry.is_none());
}

// ─── RotationStrategy Default ───────────────────────────

#[test]
fn test_rotation_strategy_default_is_replace() {
    let strategy = RotationStrategy::default();
    assert!(matches!(strategy, RotationStrategy::Replace));
}

// ─── JSON Round-Trip ────────────────────────────────────

#[test]
fn test_lifecycle_rule_json_round_trip() {
    let rule = LifecycleRule::new(
        "round-trip".into(),
        LifecycleTrigger::AgeExceeded { max_days: 30 },
        LifecycleAction::Rotate {
            strategy: RotationStrategy::DualWrite,
        },
    );

    let json = serde_json::to_string(&rule).expect("serialize");
    let parsed: LifecycleRule = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.name, "round-trip");
    assert!(matches!(
        parsed.trigger,
        LifecycleTrigger::AgeExceeded { max_days: 30 }
    ));
}

#[test]
fn test_lifecycle_trigger_serde_tagged_enum() {
    let cron_json = r#"{"type":"Cron","config":{"expression":"0 0 * * *"}}"#;
    let trigger: LifecycleTrigger = serde_json::from_str(cron_json).expect("deserialize cron");
    assert!(matches!(
        trigger,
        LifecycleTrigger::Cron { ref expression } if expression == "0 0 * * *"
    ));

    let age_json = r#"{"type":"AgeExceeded","config":{"max_days":90}}"#;
    let trigger: LifecycleTrigger = serde_json::from_str(age_json).expect("deserialize age");
    assert!(matches!(
        trigger,
        LifecycleTrigger::AgeExceeded { max_days: 90 }
    ));
}

#[test]
fn test_lifecycle_action_serde_tagged_enum() {
    let json = r#"{"type":"Rotate","config":{"strategy":"DualWrite"}}"#;
    let action: LifecycleAction = serde_json::from_str(json).expect("deserialize rotate");
    assert!(matches!(
        action,
        LifecycleAction::Rotate {
            strategy: RotationStrategy::DualWrite
        }
    ));
}

#[test]
fn test_secret_lifecycle_json_round_trip() {
    let mut lc = SecretLifecycle::new("API_KEY".into());
    lc.history.push(StateTransition {
        from: LifecycleState::Active,
        to: LifecycleState::Rotating,
        timestamp: chrono::Utc::now(),
        triggered_by: "manual".into(),
        operation_id: None,
    });

    let json = serde_json::to_string(&lc).expect("serialize");
    let parsed: SecretLifecycle = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.key, "API_KEY");
    assert_eq!(parsed.state, LifecycleState::Active);
    assert_eq!(parsed.history.len(), 1);
}

#[test]
fn test_lifecycle_state_serde_round_trip() {
    let states = vec![
        LifecycleState::Creating,
        LifecycleState::Active,
        LifecycleState::Rotating,
        LifecycleState::PendingDeprecation,
        LifecycleState::Deprecated,
        LifecycleState::Decommissioned,
        LifecycleState::Failed,
    ];

    for state in states {
        let json = serde_json::to_string(&state).expect("serialize");
        let parsed: LifecycleState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, parsed);
    }
}

// ─── Error Formatting ───────────────────────────────────

#[test]
fn test_lifecycle_error_rule_not_found_format() {
    let id = uuid::Uuid::nil();
    let err = LifecycleError::RuleNotFound { id };
    let msg = err.to_string();
    assert!(msg.contains("rule not found"));
    assert!(msg.contains(&id.to_string()));
}

#[test]
fn test_lifecycle_error_invalid_transition_format() {
    let err = LifecycleError::InvalidTransition {
        key: "DB_PASS".into(),
        from: LifecycleState::Active,
        to: LifecycleState::Decommissioned,
    };
    let msg = err.to_string();
    assert!(msg.contains("DB_PASS"));
    assert!(msg.contains("Active"));
    assert!(msg.contains("Decommissioned"));
}

#[test]
fn test_lifecycle_error_storage_format() {
    let err = LifecycleError::StorageError {
        message: "disk full".into(),
        path: Some(std::path::PathBuf::from("/tmp/rules/abc.toml")),
    };
    let msg = err.to_string();
    assert!(msg.contains("disk full"));
}

// ─── TriggerType Display ────────────────────────────────

#[test]
fn test_trigger_type_display() {
    assert_eq!(TriggerType::Cron.to_string(), "cron");
    assert_eq!(TriggerType::Age.to_string(), "age");
    assert_eq!(TriggerType::FileChange.to_string(), "file-change");
    assert_eq!(TriggerType::Policy.to_string(), "policy");
}

// ─── Composite Types ────────────────────────────────────

#[test]
fn test_composite_trigger_serialization() {
    let trigger = LifecycleTrigger::Composite {
        triggers: vec![
            LifecycleTrigger::AgeExceeded { max_days: 30 },
            LifecycleTrigger::Cron {
                expression: "0 0 * * *".into(),
            },
        ],
        operator: LogicalOp::All,
    };

    let json = serde_json::to_string(&trigger).expect("serialize");
    let parsed: LifecycleTrigger = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        LifecycleTrigger::Composite { triggers, operator } => {
            assert_eq!(triggers.len(), 2);
            assert!(matches!(operator, LogicalOp::All));
        }
        _ => panic!("expected Composite trigger"),
    }
}

#[test]
fn test_composite_action_serialization() {
    let action = LifecycleAction::Composite {
        actions: vec![
            LifecycleAction::Rotate {
                strategy: RotationStrategy::Replace,
            },
            LifecycleAction::Notify {
                message: "done".into(),
            },
        ],
    };

    let json = serde_json::to_string(&action).expect("serialize");
    let parsed: LifecycleAction = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        LifecycleAction::Composite { actions } => {
            assert_eq!(actions.len(), 2);
        }
        _ => panic!("expected Composite action"),
    }
}

// ─── LifecycleState PartialEq ───────────────────────────

#[test]
fn test_lifecycle_state_equality() {
    assert_eq!(LifecycleState::Active, LifecycleState::Active);
    assert_ne!(LifecycleState::Active, LifecycleState::Rotating);
    assert_ne!(LifecycleState::Creating, LifecycleState::Decommissioned);
}

// ─── RotationPolicy ─────────────────────────────────────

#[test]
fn test_rotation_policy_all_fields() {
    let policy = RotationPolicy {
        strategy: RotationStrategy::DualWrite,
        interval_days: Some(30),
        notify_days_before: Some(7),
    };

    assert_eq!(policy.interval_days, Some(30));
    assert_eq!(policy.notify_days_before, Some(7));
}

#[test]
fn test_rotation_policy_minimal() {
    let policy = RotationPolicy {
        strategy: RotationStrategy::Replace,
        interval_days: None,
        notify_days_before: None,
    };

    assert!(policy.interval_days.is_none());
    assert!(policy.notify_days_before.is_none());
}

// ─── SecretTemplate ─────────────────────────────────────

#[test]
fn test_secret_template_basic() {
    let template = SecretTemplate {
        id: uuid::Uuid::new_v4(),
        name: "db-pass".into(),
        generator: SecretGenerator::Random {
            length: 32,
            chars: "abcdef".into(),
        },
        target_paths: vec![std::path::PathBuf::from(".env")],
        rotation_policy: None,
        tags: vec!["production".into()],
    };

    assert_eq!(template.name, "db-pass");
    assert_eq!(template.tags.len(), 1);
    assert_eq!(template.target_paths.len(), 1);
}
