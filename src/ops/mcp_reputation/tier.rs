//! Canonical `Tier` enum returned by `TierLookup::lookup`.
//!
//! Per ADR-016 (cross-bolt trait-stub seam), this unit owns the
//! canonical `Tier`. Unit 002's resolver re-exports it via
//! `pub use crate::ops::mcp_reputation::Tier`.

/// Public lookup-result enum. See `ReputationLookup::lookup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    KnownGood,
    Unknown,
    KnownBad { reason: String, cve: Vec<String> },
    UserTrusted { reason: String },
    Volatile,
}
