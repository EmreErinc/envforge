use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

use super::LifecycleState;

#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("rule not found: {id}")]
    RuleNotFound { id: Uuid },

    #[error("invalid state transition: {from:?} -> {to:?} for key '{key}'")]
    InvalidTransition {
        key: String,
        from: LifecycleState,
        to: LifecycleState,
    },

    #[error("rule conflict: {reason}")]
    RuleConflict { reason: String },

    #[error("trigger evaluation failed for rule {rule_id}: {reason}")]
    TriggerEvalFailed { rule_id: Uuid, reason: String },

    #[error("storage error: {message}")]
    StorageError {
        message: String,
        path: Option<PathBuf>,
    },

    #[error("snapshot not found: {id}")]
    SnapshotNotFound { id: Uuid },
}
