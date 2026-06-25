//! Coverage for `ops::envbom::serializer` digest + subject helpers. The
//! ENV-BOM attestation chain depends on a stable SHA-256 over canonical bytes.

use envforge::ops::envbom::serializer::{digest_sha256, subject_for};

#[test]
fn test_subject_for_known_sha256_vectors() {
    // Well-known SHA-256 test vectors pin the digest output.
    assert_eq!(
        subject_for(b"abc").digest_hex,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        subject_for(b"").digest_hex,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_subject_for_metadata() {
    let s = subject_for(b"some bytes");
    assert_eq!(s.name, "envbom");
    assert_eq!(s.digest_alg, "sha256");
    assert_eq!(s.digest_hex.len(), 64); // 32 bytes hex-encoded
}

#[test]
fn test_digest_sha256_is_deterministic_and_distinct() {
    assert_eq!(digest_sha256(b"x"), digest_sha256(b"x"));
    assert_ne!(digest_sha256(b"x"), digest_sha256(b"y"));
    assert_eq!(digest_sha256(b"abc").len(), 32);
}
