//! Bundled reputation feed: schema + lazy gzip decode.
//!
//! The feed ships unsigned in v0.8; trust anchor is the
//! envforge binary release pipeline. Cosign-sign-blob–verified external
//! feed channel deferred to v0.9.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use super::error::FeedError;

const BUNDLED_FEED: &[u8] = include_bytes!("../../../assets/mcp-reputation-feed.json.gz");

/// On-disk tier enum stored in feed `Entry` records.
/// Strict subset of public [`Tier`](crate::ops::mcp_reputation::Tier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BareTier {
    KnownGood,
    Unknown,
    KnownBad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub tier: BareTier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_good_hashes: Vec<String>,
    #[serde(default)]
    pub volatile: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cve: Vec<String>,
}

impl Entry {
    pub fn validate(&self) -> Result<(), FeedError> {
        if self.name.is_empty() {
            return Err(FeedError::InvalidEntry {
                name: self.name.clone(),
                reason: "name must be non-empty".into(),
            });
        }
        if matches!(self.tier, BareTier::KnownBad) && self.reason.is_none() {
            return Err(FeedError::InvalidEntry {
                name: self.name.clone(),
                reason: "KnownBad entry requires reason".into(),
            });
        }
        if !self.cve.is_empty() && !matches!(self.tier, BareTier::KnownBad) {
            return Err(FeedError::InvalidEntry {
                name: self.name.clone(),
                reason: "CVE non-empty requires KnownBad tier".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FeedRaw {
    feed_version: String,
    last_updated: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    pubkey_id: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Feed {
    pub feed_version: String,
    pub last_updated: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub pubkey_id: String,
    pub entries: BTreeMap<String, Entry>,
}

impl Feed {
    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        now > self.expires_at
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.get(name)
    }
}

pub struct FeedDecoder;

static CACHED_FEED: OnceLock<Result<Feed, FeedError>> = OnceLock::new();

impl FeedDecoder {
    /// Decode the binary-embedded bundled feed (once per process).
    pub fn decode_bundled() -> Result<&'static Feed, FeedError> {
        CACHED_FEED
            .get_or_init(|| Self::decode_bytes(BUNDLED_FEED))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Decode an arbitrary gzipped JSON feed (test-friendly entrypoint).
    pub fn decode_bytes(bytes: &[u8]) -> Result<Feed, FeedError> {
        if bytes.is_empty() {
            return Err(FeedError::Empty);
        }
        let mut decoder = GzDecoder::new(bytes);
        let mut json_bytes = Vec::new();
        decoder
            .read_to_end(&mut json_bytes)
            .map_err(FeedError::CorruptGzip)?;

        let raw: FeedRaw = serde_json::from_slice(&json_bytes)
            .map_err(|e| FeedError::CorruptJson(e.to_string()))?;

        let mut map: BTreeMap<String, Entry> = BTreeMap::new();
        for entry in raw.entries {
            entry.validate()?;
            if map.contains_key(&entry.name) {
                log::warn!("duplicate feed entry '{}'; last-wins applied", entry.name);
            }
            map.insert(entry.name.clone(), entry);
        }

        Ok(Feed {
            feed_version: raw.feed_version,
            last_updated: raw.last_updated,
            expires_at: raw.expires_at,
            pubkey_id: raw.pubkey_id,
            entries: map,
        })
    }
}
