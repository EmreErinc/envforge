//! Regression tests for H1/NFR2 — the secrets disk cache must be encrypted at
//! rest, and legacy plaintext caches must never be served (graceful migration).
//!
//! Serialized because they mutate process-global env (`ENVFORGE_CONFIG_DIR`,
//! age-key vars) to isolate the cache + key into a tempdir.

use envforge::ops::secrets::cache::{read_cache, read_cache_stale, write_cache};
use serial_test::serial;

fn isolate(dir: &std::path::Path) {
    std::env::set_var("ENVFORGE_CONFIG_DIR", dir);
    // Let the age key auto-generate inside the isolated config dir.
    std::env::remove_var("ENVFORGE_AGE_KEY");
    std::env::remove_var("ENVFORGE_AGE_KEY_FILE");
}

#[test]
#[serial]
fn test_cache_value_encrypted_at_rest_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    let secret = "super-secret-cache-value-123";
    write_cache("vault", "DB_PASS", secret, Some(300)).unwrap();

    // The on-disk file must be age-encrypted: it carries the ENC[age: marker
    // and does NOT contain the plaintext anywhere.
    let cache_file = dir
        .path()
        .join("secrets-cache")
        .join("vault")
        .join("DB_PASS.cache");
    let raw = std::fs::read_to_string(&cache_file).unwrap();
    assert!(raw.contains("ENC[age:"), "cache value not encrypted: {raw}");
    assert!(
        !raw.contains(secret),
        "cache leaked plaintext on disk: {raw}"
    );

    // And it round-trips back to the original plaintext on read.
    let got = read_cache("vault", "DB_PASS").unwrap();
    assert_eq!(got.as_deref(), Some(secret));

    std::env::remove_var("ENVFORGE_CONFIG_DIR");
}

#[test]
#[serial]
fn test_legacy_plaintext_cache_treated_as_miss_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    // Hand-write a pre-encryption (plaintext) cache entry.
    let pdir = dir.path().join("secrets-cache").join("vault");
    std::fs::create_dir_all(&pdir).unwrap();
    let file = pdir.join("DB_PASS.cache");
    std::fs::write(
        &file,
        "value = \"legacy-plaintext-secret\"\nfetched_at = \"2030-01-01T00:00:00+00:00\"\nttl_secs = 300\n",
    )
    .unwrap();

    // Stale read decrypts regardless of TTL: a plaintext value fails to decrypt
    // → treated as a miss, and the offending file is removed (migration).
    let got = read_cache_stale("vault", "DB_PASS").unwrap();
    assert_eq!(got, None, "legacy plaintext must never be served");
    assert!(!file.exists(), "undecryptable cache file should be removed");

    std::env::remove_var("ENVFORGE_CONFIG_DIR");
}

#[test]
#[serial]
fn test_concurrent_first_run_key_generation_converges() {
    // Regression: parallel first-run key generation used to let each racer use
    // its own in-memory key while the on-disk file held a different one, so a
    // value encrypted by one racer couldn't be decrypted after another's key
    // won the rename. ensure_age_key now re-reads the persisted file so all
    // racers converge on the winning key.
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());

    // No key exists yet — many threads race to generate one.
    let ciphertexts: Vec<String> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                envforge::ops::encrypt::encrypt_value(&format!("secret-{i}")).unwrap()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // Every ciphertext must decrypt under the single converged key.
    for (i, ct) in ciphertexts.iter().enumerate() {
        let pt = envforge::ops::encrypt::decrypt_value(ct).unwrap();
        assert_eq!(pt, format!("secret-{i}"));
    }

    std::env::remove_var("ENVFORGE_CONFIG_DIR");
}
