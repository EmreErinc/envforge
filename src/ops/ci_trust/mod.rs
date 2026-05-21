//! CI trigger trust classification + secret quarantine.
//!
//! See `memory-bank/bolts/071-comment-control-guard/` for the design docs and ADR-010
//! (why this lives in the Rust binary instead of inline bash).
//!
//! Usage flow:
//! 1. `envforge ci-trust classify --json` — emits a `TrustVerdict` JSON to stdout
//! 2. `envforge ci-trust quarantine [--allow-key K]` — emits `KEY=VALUE` env-var lines
//!    to stdout suitable for `eval` consumption in shell glue
//! 3. `envforge ci-trust summary` — emits Step Summary markdown to stdout

pub mod classifier;
pub mod quarantine;
pub mod summary;

pub use classifier::{
    cached_or_compute, classify, from_env, AuthorAssociation, TriggerContext, TrustLevel,
    TrustReason, TrustVerdict, CLASSIFIER_VERSION,
};
pub use quarantine::{apply, DecisionSource, MaskHit, MaskedVia, QuarantineDecision, ScrubReport};
pub use summary::{emit_action_outputs, render_step_summary};
