#![no_main]
//! Fuzz target for `CanonicalJsonHasher::canonicalize_and_hash`.
//!
//! Implements story 004 (bolt 075-lockfile-hasher).
//!
//! Run locally:
//!
//! ```
//! cargo +nightly fuzz run canonical_hasher -- -max_total_time=3600
//! ```
//!
//! Only panics, aborts, or memory exhaustion fail the target. Structured
//! `HasherError` returns are expected on adversarial inputs and are NOT
//! failures.

use envforge::ops::mcp_pin::CanonicalJsonHasher;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = CanonicalJsonHasher::canonicalize_and_hash(data);
});
