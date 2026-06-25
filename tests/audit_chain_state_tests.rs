//! Coverage for `ops::audit::tamper::ChainState` — the in-memory per-file
//! hash-chain head tracker used by the tamper-evident audit log.

use envforge::ops::audit::tamper::ChainState;

#[test]
fn test_chain_state_new_is_empty() {
    let state = ChainState::new();
    assert!(state.get_last_hash("audit.log").is_none());
    assert!(state.last_hashes.is_empty());
    assert!(state.chain_ids.is_empty());
}

#[test]
fn test_chain_state_update_and_lookup() {
    let mut state = ChainState::new();
    state.update(
        "audit.log".to_string(),
        "evt-1".to_string(),
        "hash-1".to_string(),
    );
    assert_eq!(
        state.get_last_hash("audit.log"),
        Some(&"hash-1".to_string())
    );
    assert_eq!(state.chain_ids.get("audit.log"), Some(&"evt-1".to_string()));

    // Updating the same file replaces the head (chain advances).
    state.update(
        "audit.log".to_string(),
        "evt-2".to_string(),
        "hash-2".to_string(),
    );
    assert_eq!(
        state.get_last_hash("audit.log"),
        Some(&"hash-2".to_string())
    );
    assert_eq!(state.chain_ids.get("audit.log"), Some(&"evt-2".to_string()));
}

#[test]
fn test_chain_state_tracks_files_independently() {
    let mut state = ChainState::new();
    state.update("a.log".to_string(), "e1".to_string(), "h1".to_string());
    state.update("b.log".to_string(), "e2".to_string(), "h2".to_string());
    assert_eq!(state.get_last_hash("a.log"), Some(&"h1".to_string()));
    assert_eq!(state.get_last_hash("b.log"), Some(&"h2".to_string()));
    assert!(state.get_last_hash("c.log").is_none());
}

#[test]
fn test_chain_state_default_matches_new() {
    let state = ChainState::default();
    assert!(state.last_hashes.is_empty());
}
