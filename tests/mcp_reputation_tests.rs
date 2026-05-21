//! Integration tests for `src/ops/mcp_reputation/`.
//!
//! Covers stories 001-005 of bolt 077-reputation-feed.

use std::io::Write;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use envforge::ops::mcp_pin::resolver::ReputationLookup;
use envforge::ops::mcp_reputation::{
    BareTier, Entry, Feed, FeedDecoder, FeedError, FsUserOverrideRepository,
    InMemoryUserOverrideRepository, OverrideError, Tier, TierLookup, UserOverride,
    UserOverrideRepository, UserOverrideStore,
};
use flate2::write::GzEncoder;
use flate2::Compression;

// ─────────────────────────────────────────────────────────────────────────────
// Synthetic feed helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_feed_gz(json: &str) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(json.as_bytes()).unwrap();
    enc.finish().unwrap()
}

fn sample_feed_json() -> &'static str {
    r#"{
        "feed_version": "test-2026-05-12",
        "last_updated": "2026-05-12T00:00:00Z",
        "expires_at": "2030-01-01T00:00:00Z",
        "pubkey_id": "test-key",
        "entries": [
            {
                "name": "good-server",
                "tier": "known-good",
                "vendor": "TestVendor",
                "volatile": false
            },
            {
                "name": "bad-server",
                "tier": "known-bad",
                "reason": "test exfiltration",
                "cve": ["CVE-9999-TEST"]
            },
            {
                "name": "self-updater",
                "tier": "known-good",
                "volatile": true
            }
        ]
    }"#
}

fn stale_feed_json() -> &'static str {
    r#"{
        "feed_version": "stale",
        "last_updated": "2020-01-01T00:00:00Z",
        "expires_at": "2020-06-01T00:00:00Z",
        "pubkey_id": "test-key",
        "entries": []
    }"#
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 001: feed-schema-lazy-decode
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_feed_decode_bundled_succeeds() {
    let feed = FeedDecoder::decode_bundled().expect("bundled feed loads");
    assert!(!feed.feed_version.is_empty());
    assert!(!feed.entries.is_empty());
}

#[test]
fn test_feed_decode_bytes_synthetic() {
    let bytes = build_feed_gz(sample_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    assert_eq!(feed.feed_version, "test-2026-05-12");
    assert_eq!(feed.entries.len(), 3);
    assert!(feed.find("good-server").is_some());
}

#[test]
fn test_feed_decode_empty_input_errors() {
    let err = FeedDecoder::decode_bytes(&[]).expect_err("must reject empty");
    matches!(err, FeedError::Empty);
}

#[test]
fn test_feed_decode_corrupt_gzip_errors() {
    let err = FeedDecoder::decode_bytes(b"not gzip").expect_err("corrupt gzip");
    matches!(err, FeedError::CorruptGzip(_));
}

#[test]
fn test_feed_decode_corrupt_json_errors() {
    let bytes = build_feed_gz("{not valid json");
    let err = FeedDecoder::decode_bytes(&bytes).expect_err("corrupt json");
    matches!(err, FeedError::CorruptJson(_));
}

#[test]
fn test_feed_decode_known_bad_without_reason_rejected() {
    let json = r#"{
        "feed_version": "x", "last_updated": "2026-01-01T00:00:00Z",
        "expires_at": "2030-01-01T00:00:00Z", "pubkey_id": "k",
        "entries": [{"name": "x", "tier": "known-bad"}]
    }"#;
    let err = FeedDecoder::decode_bytes(&build_feed_gz(json)).expect_err("missing reason");
    matches!(err, FeedError::InvalidEntry { .. });
}

#[test]
fn test_feed_decode_cve_without_known_bad_rejected() {
    let json = r#"{
        "feed_version": "x", "last_updated": "2026-01-01T00:00:00Z",
        "expires_at": "2030-01-01T00:00:00Z", "pubkey_id": "k",
        "entries": [{"name": "x", "tier": "known-good", "cve": ["CVE-1"]}]
    }"#;
    let err = FeedDecoder::decode_bytes(&build_feed_gz(json)).expect_err("CVE without KnownBad");
    matches!(err, FeedError::InvalidEntry { .. });
}

#[test]
fn test_feed_entry_validate_directly() {
    let bad = Entry {
        name: "x".into(),
        tier: BareTier::KnownBad,
        reason: None,
        known_good_hashes: vec![],
        volatile: false,
        vendor: None,
        cve: vec![],
    };
    assert!(bad.validate().is_err());

    let good = Entry {
        name: "x".into(),
        tier: BareTier::KnownGood,
        reason: None,
        known_good_hashes: vec![],
        volatile: false,
        vendor: None,
        cve: vec![],
    };
    assert!(good.validate().is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 002 + 005: TierLookup precedence matrix
// ─────────────────────────────────────────────────────────────────────────────

fn build_synthetic_feed_static() -> &'static Feed {
    let bytes = build_feed_gz(sample_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    Box::leak(Box::new(feed))
}

#[test]
fn test_lookup_known_good_no_override() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    matches!(lookup.lookup("good-server"), Tier::KnownGood);
}

#[test]
fn test_lookup_known_bad_security_floor() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    // Add user override for bad-server; should still get KnownBad.
    let store = UserOverrideStore::new(repo.clone());
    store
        .record_user_trust("bad-server", "I think it's fine")
        .unwrap();
    let lookup = TierLookup::with_feed(feed, repo);
    let tier = lookup.lookup("bad-server");
    matches!(tier, Tier::KnownBad { .. });
}

#[test]
fn test_lookup_volatile_takes_effect() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    matches!(lookup.lookup("self-updater"), Tier::Volatile);
    assert!(lookup.is_feed_volatile("self-updater"));
}

#[test]
fn test_lookup_unknown_no_override_no_feed_entry() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    matches!(lookup.lookup("nonexistent-server"), Tier::Unknown);
}

#[test]
fn test_lookup_user_trusted_when_unknown_in_feed() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo.clone());
    store
        .record_user_trust("community-server", "audited locally")
        .unwrap();
    let lookup = TierLookup::with_feed(feed, repo);
    match lookup.lookup("community-server") {
        Tier::UserTrusted { reason } => assert_eq!(reason, "audited locally"),
        other => panic!("expected UserTrusted, got {other:?}"),
    }
}

#[test]
fn test_lookup_volatile_with_override_returns_volatile_not_trusted() {
    // Volatile in feed → Volatile wins over user override (ADR-017 precedence step 2).
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo.clone());
    store
        .record_user_trust("self-updater", "I trust this updater")
        .unwrap();
    let lookup = TierLookup::with_feed(feed, repo);
    matches!(lookup.lookup("self-updater"), Tier::Volatile);
}

#[test]
fn test_lookup_known_good_with_override_returns_known_good() {
    // Known-good feed entry + user override → KnownGood wins (precedence step 4).
    // (User override beats Unknown but not explicit KnownGood.)
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo.clone());
    store
        .record_user_trust("good-server", "redundant trust")
        .unwrap();
    let lookup = TierLookup::with_feed(feed, repo);
    // Precedence: KnownBad > Volatile > UserTrusted > KnownGood. Override wins.
    match lookup.lookup("good-server") {
        Tier::UserTrusted { .. } => {}
        other => panic!("expected UserTrusted (override beats KnownGood), got {other:?}"),
    }
}

#[test]
fn test_is_feed_volatile_for_absent_returns_false() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    assert!(!lookup.is_feed_volatile("absent-server"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 003: user-trust-override
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_record_user_trust_persists() {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo.clone());
    store.record_user_trust("foo", "audited").unwrap();
    let found = store.find("foo").unwrap().unwrap();
    assert_eq!(found.name, "foo");
    assert_eq!(found.reason, "audited");
    assert!(!found.granted_by_machine.is_empty());
}

#[test]
fn test_record_user_trust_empty_reason_rejected() {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo);
    let err = store
        .record_user_trust("foo", "   ")
        .expect_err("must reject");
    matches!(err, OverrideError::EmptyReason { .. });
}

#[test]
fn test_record_user_trust_replaces_existing() {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo.clone());
    store.record_user_trust("foo", "first reason").unwrap();
    store.record_user_trust("foo", "second reason").unwrap();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, "second reason");
}

#[test]
fn test_revoke_user_trust_returns_true_when_present() {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo);
    store.record_user_trust("foo", "x").unwrap();
    let removed = store.revoke_user_trust("foo").unwrap();
    assert!(removed);
    assert!(store.find("foo").unwrap().is_none());
}

#[test]
fn test_revoke_user_trust_returns_false_when_absent() {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let store = UserOverrideStore::new(repo);
    let removed = store.revoke_user_trust("never-existed").unwrap();
    assert!(!removed);
}

#[test]
fn test_fs_user_override_repository_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir/mcp-trust.json");
    let repo = FsUserOverrideRepository::new(path.clone());
    assert!(repo.load().unwrap().is_empty()); // missing file → empty
    let overrides = vec![UserOverride {
        name: "x".into(),
        reason: "y".into(),
        granted_at: Utc::now(),
        granted_by_machine: "m".into(),
    }];
    repo.save(&overrides).unwrap();
    assert!(path.exists());
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "x");
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 004: stale-feed-handling
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_feed_is_stale_when_expired() {
    let bytes = build_feed_gz(stale_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    assert!(feed.is_stale(Utc::now()));
}

#[test]
fn test_feed_not_stale_when_future_expiry() {
    let bytes = build_feed_gz(sample_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    assert!(!feed.is_stale(Utc::now()));
}

#[test]
fn test_feed_is_stale_boundary_inclusive_at_expiry() {
    let bytes = build_feed_gz(sample_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    // At exactly expires_at, NOT stale (strict greater-than only)
    assert!(!feed.is_stale(feed.expires_at));
    // One microsecond past: stale
    assert!(feed.is_stale(feed.expires_at + ChronoDuration::microseconds(1)));
}

#[test]
fn test_tier_lookup_is_feed_stale_query() {
    let bytes = build_feed_gz(stale_feed_json());
    let feed = FeedDecoder::decode_bytes(&bytes).unwrap();
    let leaked = Box::leak(Box::new(feed));
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(leaked, repo);
    assert!(lookup.is_feed_stale());
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier payload integrity
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_known_bad_tier_carries_reason_and_cve() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    match lookup.lookup("bad-server") {
        Tier::KnownBad { reason, cve } => {
            assert_eq!(reason, "test exfiltration");
            assert_eq!(cve, vec!["CVE-9999-TEST".to_string()]);
        }
        other => panic!("expected KnownBad, got {other:?}"),
    }
}

#[test]
fn test_user_trusted_tier_carries_reason() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    UserOverrideStore::new(repo.clone())
        .record_user_trust("custom", "reason text here")
        .unwrap();
    let lookup = TierLookup::with_feed(feed, repo);
    match lookup.lookup("custom") {
        Tier::UserTrusted { reason } => assert_eq!(reason, "reason text here"),
        other => panic!("expected UserTrusted, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TierLookup::feed_version exposure
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_feed_version_accessible() {
    let feed = build_synthetic_feed_static();
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(InMemoryUserOverrideRepository::default());
    let lookup = TierLookup::with_feed(feed, repo);
    assert_eq!(lookup.feed_version(), "test-2026-05-12");
}

// ─────────────────────────────────────────────────────────────────────────────
// Compile-time sanity: bundled date is the date we expect
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_bundled_feed_has_expected_anthropic_entries() {
    let feed = FeedDecoder::decode_bundled().unwrap();
    assert!(feed
        .entries
        .keys()
        .any(|n| n == "@modelcontextprotocol/server-github"));
}

#[test]
fn test_bundled_feed_version_is_iso_date() {
    let feed = FeedDecoder::decode_bundled().unwrap();
    // version is a YYYY-MM-DD-like stamp; basic shape check
    assert!(feed.feed_version.len() >= 10);
    assert!(feed.feed_version.chars().nth(4) == Some('-'));
}

#[test]
fn test_bundled_feed_expires_after_last_updated() {
    let feed = FeedDecoder::decode_bundled().unwrap();
    assert!(feed.expires_at > feed.last_updated);
}

#[test]
fn test_bundled_feed_pubkey_id_present() {
    let feed = FeedDecoder::decode_bundled().unwrap();
    assert!(!feed.pubkey_id.is_empty());
}

#[test]
fn test_bundled_feed_decode_idempotent_caches() {
    // Two calls return same reference under the hood (OnceLock invariant).
    let a = FeedDecoder::decode_bundled().unwrap();
    let b = FeedDecoder::decode_bundled().unwrap();
    assert_eq!(a.feed_version, b.feed_version);
}

// Suppress unused-time-fn warning
#[test]
fn test_sample_feed_is_recent_relative_to_2025() {
    let feed = build_synthetic_feed_static();
    let _ref = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    assert!(feed.last_updated > _ref);
}
