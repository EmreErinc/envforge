use envforge::model::{DecommissionPlan, LifecycleState, RotationStrategy};
use envforge::ops::lifecycle::orchestrator;

fn random_key(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().to_string().replace('-', "_"))
}

// ─── State Management ───────────────────────────────────

#[test]
fn test_get_state_defaults_to_active() {
    let key = random_key("GET_STATE");
    let state = orchestrator::get_state(&key).expect("get state");
    assert_eq!(state, LifecycleState::Active);
}

// ─── Decommission (no grace) ─────────────────────────────

#[test]
fn test_decommission_immediate_updates_state() {
    let key = random_key("DECOMM_IMMEDIATE");
    let plan = DecommissionPlan {
        key: key.clone(),
        has_active_dependents: false,
        recommended_grace_days: 0,
    };

    let result = orchestrator::decommission_secret(&key, &plan).expect("decommission");
    assert!(result.success);
    assert!(!result.grace_period_applied);

    let state = orchestrator::get_state(&key).expect("get state");
    assert_eq!(state, LifecycleState::Decommissioned);
}

#[test]
fn test_decommission_graceful_enters_pending() {
    let key = random_key("DECOMM_GRACE");
    let plan = DecommissionPlan {
        key: key.clone(),
        has_active_dependents: false,
        recommended_grace_days: 7,
    };

    let result = orchestrator::decommission_secret(&key, &plan).expect("decommission");
    assert!(result.success);
    assert!(result.grace_period_applied);

    let state = orchestrator::get_state(&key).expect("get state");
    assert_eq!(state, LifecycleState::PendingDeprecation);
}

// ─── RotationStrategy Default ───────────────────────────

#[test]
fn test_rotation_strategy_has_default() {
    let strategy = RotationStrategy::default();
    assert!(matches!(strategy, RotationStrategy::Replace));
}
