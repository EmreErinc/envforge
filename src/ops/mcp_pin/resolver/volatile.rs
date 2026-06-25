//! `ReputationLookup` trait + `StubReputationLookup` stub and
//! `VolatileChecker` dispatch service.
//!
//! `mcp_reputation::TierLookup` provides the real
//! implementation. The canonical `Tier` enum lives in `mcp_reputation::tier`;
//! this module re-exports it so existing callers keep importing from
//! `mcp_pin::resolver`.

use std::sync::Arc;

// `Tier` is owned by `mcp_reputation`; re-export here for
// trait-method signatures and back-compat with test imports.
pub use crate::ops::mcp_reputation::Tier;

/// `mcp_reputation::TierLookup` satisfies this
/// trait; tests use `StubReputationLookup` below.
pub trait ReputationLookup: Send + Sync {
    fn lookup(&self, name: &str) -> Tier;

    fn is_feed_volatile(&self, name: &str) -> bool {
        matches!(self.lookup(name), Tier::Volatile)
    }
}

/// Safe-default stub. Returns `Unknown` for all names. Retained for tests
/// and as a fallback when `mcp_reputation` is not wired (e.g. early in the
/// Resolver pipeline before the CLI is invoked).
pub struct StubReputationLookup;

impl ReputationLookup for StubReputationLookup {
    fn lookup(&self, _name: &str) -> Tier {
        Tier::Unknown
    }
}

/// Domain service. Wraps a `ReputationLookup` and exposes the
/// `is_volatile` decision used by the resolver pipeline.
pub struct VolatileChecker {
    lookup: Arc<dyn ReputationLookup>,
}

impl VolatileChecker {
    pub fn new(lookup: Arc<dyn ReputationLookup>) -> Self {
        Self { lookup }
    }

    pub fn is_volatile(&self, name: &str) -> bool {
        self.lookup.is_feed_volatile(name)
    }
}
