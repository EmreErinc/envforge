//! MCP server reputation: bundled gzip feed + per-user trust override store.
//!
//! Owns the canonical `Tier` enum. Provides `TierLookup`
//! which implements the `ReputationLookup` trait declared in `mcp_pin`.

pub mod error;
pub mod feed;
pub mod tier;
pub mod user_override;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;

pub use error::{FeedError, OverrideError};
pub use feed::{BareTier, Entry, Feed, FeedDecoder};
pub use tier::Tier;
pub use user_override::{
    FsUserOverrideRepository, InMemoryUserOverrideRepository, UserOverride, UserOverrideRepository,
    UserOverrideStore,
};

use crate::ops::mcp_pin::resolver::ReputationLookup;

/// Public façade. Implements the `ReputationLookup` trait.
///
/// Combines the bundled feed with the user override store and applies the
/// canonical 5-step precedence rule:
///
/// ```text
/// 1. Feed KnownBad → KnownBad (security floor; always wins)
/// 2. Feed volatile  → Volatile
/// 3. User override  → UserTrusted
/// 4. Feed KnownGood → KnownGood
/// 5. Else           → Unknown
/// ```
pub struct TierLookup {
    feed: &'static Feed,
    overrides: Arc<dyn UserOverrideRepository>,
    stale_warned: AtomicBool,
}

impl TierLookup {
    pub fn new(overrides: Arc<dyn UserOverrideRepository>) -> Result<Self, FeedError> {
        let feed = FeedDecoder::decode_bundled()?;
        Ok(Self {
            feed,
            overrides,
            stale_warned: AtomicBool::new(false),
        })
    }

    /// Construct with an arbitrary feed (test helper).
    pub fn with_feed(feed: &'static Feed, overrides: Arc<dyn UserOverrideRepository>) -> Self {
        Self {
            feed,
            overrides,
            stale_warned: AtomicBool::new(false),
        }
    }

    pub fn feed_version(&self) -> &str {
        &self.feed.feed_version
    }

    pub fn is_feed_stale(&self) -> bool {
        self.feed.is_stale(Utc::now())
    }

    fn check_stale(&self) {
        if self.is_feed_stale() && !self.stale_warned.swap(true, Ordering::SeqCst) {
            eprintln!(
                "warning: mcp reputation feed expired on {} (version {})",
                self.feed.expires_at, self.feed.feed_version
            );
        }
    }
}

impl ReputationLookup for TierLookup {
    fn lookup(&self, name: &str) -> Tier {
        self.check_stale();
        let user_override = self
            .overrides
            .load()
            .ok()
            .and_then(|list| list.into_iter().find(|o| o.name == name));
        let feed_entry = self.feed.find(name);

        // 1. KnownBad always wins (security floor)
        if let Some(e) = feed_entry {
            if matches!(e.tier, BareTier::KnownBad) {
                if user_override.is_some() {
                    eprintln!(
                        "warning: user trust override for '{}' refused by security floor (feed says KnownBad)",
                        name
                    );
                }
                return Tier::KnownBad {
                    reason: e.reason.clone().unwrap_or_default(),
                    cve: e.cve.clone(),
                };
            }
            // 2. Volatile takes effect when not bad
            if e.volatile {
                return Tier::Volatile;
            }
        }
        // 3. User override
        if let Some(o) = user_override {
            return Tier::UserTrusted { reason: o.reason };
        }
        // 4. Feed KnownGood
        if let Some(e) = feed_entry {
            if matches!(e.tier, BareTier::KnownGood) {
                return Tier::KnownGood;
            }
        }
        Tier::Unknown
    }

    fn is_feed_volatile(&self, name: &str) -> bool {
        self.feed.find(name).is_some_and(|e| e.volatile)
    }
}
