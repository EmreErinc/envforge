use uuid::Uuid;

use crate::model::{LifecycleError, LifecycleState, StateEvent, StateTransition};

/// Transition from one lifecycle state to another given an event.
/// Returns the new state on success, or `LifecycleError::InvalidTransition` if the transition is not allowed.
pub fn transition(
    current: &LifecycleState,
    event: &StateEvent,
) -> Result<LifecycleState, LifecycleError> {
    use LifecycleState::{
        Active, Creating, Decommissioned, Deprecated, Failed, PendingDeprecation, Rotating,
    };
    use StateEvent::{
        CreateComplete, DecommissionComplete, DeprecationRequested, Failure, GracePeriodExpired,
        Recovery, RotationComplete, RotationRequested,
    };

    match (current, event) {
        (Creating, CreateComplete) => Ok(Active),
        (Active, RotationRequested) => Ok(Rotating),
        (Active, DeprecationRequested) => Ok(PendingDeprecation),
        (Rotating, RotationComplete) => Ok(Active),
        (Rotating, Failure { .. }) => Ok(Failed),
        (PendingDeprecation, GracePeriodExpired) => Ok(Deprecated),
        (Deprecated, DecommissionComplete) => Ok(Decommissioned),
        (Failed, Recovery) => Ok(Active),

        // Any state can transition to Failed on failure
        (Active, Failure { .. })
        | (Creating, Failure { .. })
        | (PendingDeprecation, Failure { .. })
        | (Deprecated, Failure { .. })
        | (Decommissioned, Failure { .. }) => Ok(Failed),

        _ => Err(LifecycleError::InvalidTransition {
            key: String::new(),
            from: current.clone(),
            to: target_state(event),
        }),
    }
}

/// List all valid events for a given state.
pub fn valid_transitions(state: &LifecycleState) -> Vec<StateEvent> {
    use LifecycleState::{
        Active, Creating, Decommissioned, Deprecated, Failed, PendingDeprecation, Rotating,
    };
    use StateEvent::{
        CreateComplete, DecommissionComplete, DeprecationRequested, Failure, GracePeriodExpired,
        Recovery, RotationComplete, RotationRequested,
    };

    match state {
        Creating => vec![
            CreateComplete,
            Failure {
                reason: String::new(),
            },
        ],
        Active => vec![
            RotationRequested,
            DeprecationRequested,
            Failure {
                reason: String::new(),
            },
        ],
        Rotating => vec![
            RotationComplete,
            Failure {
                reason: String::new(),
            },
        ],
        PendingDeprecation => vec![
            GracePeriodExpired,
            Failure {
                reason: String::new(),
            },
        ],
        Deprecated => vec![
            DecommissionComplete,
            Failure {
                reason: String::new(),
            },
        ],
        Decommissioned => vec![],
        Failed => vec![Recovery],
    }
}

/// Returns true if the state is terminal (no further transitions possible).
pub fn is_terminal(state: &LifecycleState) -> bool {
    matches!(state, LifecycleState::Decommissioned)
}

/// Create a state transition record for history tracking.
pub fn create_transition(
    from: LifecycleState,
    to: LifecycleState,
    triggered_by: &str,
    operation_id: Option<Uuid>,
) -> StateTransition {
    StateTransition {
        from,
        to,
        timestamp: chrono::Utc::now(),
        triggered_by: triggered_by.to_string(),
        operation_id,
    }
}

/// Guess the target state for an event (used in error messages).
fn target_state(event: &StateEvent) -> LifecycleState {
    use LifecycleState::{
        Active, Decommissioned, Deprecated, Failed, PendingDeprecation, Rotating,
    };
    use StateEvent::{
        CreateComplete, DecommissionComplete, DeprecationRequested, Failure, GracePeriodExpired,
        Recovery, RotationComplete, RotationRequested,
    };

    match event {
        CreateComplete => Active,
        RotationRequested => Rotating,
        RotationComplete => Active,
        DeprecationRequested => PendingDeprecation,
        GracePeriodExpired => Deprecated,
        DecommissionComplete => Decommissioned,
        Failure { .. } => Failed,
        Recovery => Active,
    }
}
