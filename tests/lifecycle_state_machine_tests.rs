use envforge::model::LifecycleState;
use envforge::model::StateEvent;
use envforge::ops::lifecycle::state_machine;

// ─── Valid Transitions ──────────────────────────────────

#[test]
fn test_transition_creating_to_active() {
    let result =
        state_machine::transition(&LifecycleState::Creating, &StateEvent::CreateComplete);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LifecycleState::Active);
}

#[test]
fn test_transition_active_to_rotating() {
    let result = state_machine::transition(
        &LifecycleState::Active,
        &StateEvent::RotationRequested,
    );
    assert_eq!(result.unwrap(), LifecycleState::Rotating);
}

#[test]
fn test_transition_active_to_pending_deprecation() {
    let result = state_machine::transition(
        &LifecycleState::Active,
        &StateEvent::DeprecationRequested,
    );
    assert_eq!(result.unwrap(), LifecycleState::PendingDeprecation);
}

#[test]
fn test_transition_rotating_to_active() {
    let result = state_machine::transition(
        &LifecycleState::Rotating,
        &StateEvent::RotationComplete,
    );
    assert_eq!(result.unwrap(), LifecycleState::Active);
}

#[test]
fn test_transition_rotating_to_failed() {
    let result = state_machine::transition(
        &LifecycleState::Rotating,
        &StateEvent::Failure {
            reason: "provider timeout".into(),
        },
    );
    assert_eq!(result.unwrap(), LifecycleState::Failed);
}

#[test]
fn test_transition_pending_deprecation_to_deprecated() {
    let result = state_machine::transition(
        &LifecycleState::PendingDeprecation,
        &StateEvent::GracePeriodExpired,
    );
    assert_eq!(result.unwrap(), LifecycleState::Deprecated);
}

#[test]
fn test_transition_deprecated_to_decommissioned() {
    let result = state_machine::transition(
        &LifecycleState::Deprecated,
        &StateEvent::DecommissionComplete,
    );
    assert_eq!(result.unwrap(), LifecycleState::Decommissioned);
}

#[test]
fn test_transition_failed_to_active_recovery() {
    let result =
        state_machine::transition(&LifecycleState::Failed, &StateEvent::Recovery);
    assert_eq!(result.unwrap(), LifecycleState::Active);
}

#[test]
fn test_transition_any_to_failed() {
    let states = [
        LifecycleState::Active,
        LifecycleState::Creating,
        LifecycleState::PendingDeprecation,
        LifecycleState::Deprecated,
        LifecycleState::Decommissioned,
    ];

    for state in &states {
        let result = state_machine::transition(
            state,
            &StateEvent::Failure {
                reason: "error".into(),
            },
        );
        assert!(result.is_ok(), "state {:?} should allow Failure", state);
        assert_eq!(result.unwrap(), LifecycleState::Failed);
    }
}

// ─── Invalid Transitions ────────────────────────────────

#[test]
fn test_transition_decommissioned_no_valid_transitions() {
    let result = state_machine::transition(
        &LifecycleState::Decommissioned,
        &StateEvent::RotationRequested,
    );
    assert!(result.is_err());
}

#[test]
fn test_transition_invalid_for_state() {
    // Can't complete rotation from Active state
    let result = state_machine::transition(
        &LifecycleState::Active,
        &StateEvent::RotationComplete,
    );
    assert!(result.is_err());
}

#[test]
fn test_transition_creating_invalid_event() {
    // Can't deprecate a key that hasn't been created yet
    let result = state_machine::transition(
        &LifecycleState::Creating,
        &StateEvent::DeprecationRequested,
    );
    assert!(result.is_err());
}

// ─── Valid Transitions Helper ────────────────────────────

#[test]
fn test_valid_transitions_active_has_three() {
    let events = state_machine::valid_transitions(&LifecycleState::Active);
    assert_eq!(events.len(), 3);
}

#[test]
fn test_valid_transitions_decommissioned_empty() {
    let events = state_machine::valid_transitions(&LifecycleState::Decommissioned);
    assert!(events.is_empty());
}

#[test]
fn test_valid_transitions_failed_only_recovery() {
    let events = state_machine::valid_transitions(&LifecycleState::Failed);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], StateEvent::Recovery));
}

// ─── Terminal Check ─────────────────────────────────────

#[test]
fn test_is_terminal_decommissioned() {
    assert!(state_machine::is_terminal(&LifecycleState::Decommissioned));
}

#[test]
fn test_is_not_terminal_for_active_states() {
    assert!(!state_machine::is_terminal(&LifecycleState::Active));
    assert!(!state_machine::is_terminal(&LifecycleState::Creating));
    assert!(!state_machine::is_terminal(&LifecycleState::Failed));
}

// ─── Transition Record ───────────────────────────────────

#[test]
fn test_create_transition_basic() {
    let st = state_machine::create_transition(
        LifecycleState::Active,
        LifecycleState::Rotating,
        "manual",
        None,
    );

    assert_eq!(st.from, LifecycleState::Active);
    assert_eq!(st.to, LifecycleState::Rotating);
    assert_eq!(st.triggered_by, "manual");
    assert!(st.operation_id.is_none());
}

#[test]
fn test_create_transition_with_operation_id() {
    let op_id = uuid::Uuid::new_v4();
    let st = state_machine::create_transition(
        LifecycleState::Rotating,
        LifecycleState::Active,
        "rotation-complete",
        Some(op_id),
    );

    assert_eq!(st.from, LifecycleState::Rotating);
    assert_eq!(st.to, LifecycleState::Active);
    assert_eq!(st.triggered_by, "rotation-complete");
    assert_eq!(st.operation_id, Some(op_id));
}
