//! Coverage for `ops::secure_memory` zeroization helpers and the
//! core-dump-disable entry point.

use envforge::ops::secure_memory::{disable_core_dumps, zeroize_strings, zeroize_vec_u8};

#[test]
fn test_zeroize_strings_clears_vec() {
    let mut secrets = vec!["sk-secret".to_string(), "hunter2".to_string()];
    zeroize_strings(&mut secrets);
    assert!(secrets.is_empty());
}

#[test]
fn test_zeroize_strings_empty_is_noop() {
    let mut v: Vec<String> = Vec::new();
    zeroize_strings(&mut v);
    assert!(v.is_empty());
}

#[test]
fn test_zeroize_vec_u8_clears() {
    let mut bytes = b"super-secret-bytes".to_vec();
    zeroize_vec_u8(&mut bytes);
    assert!(bytes.is_empty());
}

#[test]
fn test_disable_core_dumps_does_not_panic() {
    // Platform no-op on non-unix; on unix it sets RLIMIT_CORE to 0.
    disable_core_dumps();
}
