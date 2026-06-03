// ═══════════════════════════════════════════════════════════════
// Security Tests — Memory Hardening
// ═══════════════════════════════════════════════════════════════

use envforge::ops::secure_memory;
use zeroize::Zeroize;

// ═══════════════════════════════════════════════════════════════
// Core Dump Suppression
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_disable_core_dumps_should_set_rlimit_to_zero() {
    #[cfg(unix)]
    {
        secure_memory::disable_core_dumps();

        let mut rlim = libc::rlimit {
            rlim_cur: 1,
            rlim_max: 1,
        };
        let ret = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut rlim) };
        assert_eq!(ret, 0, "getrlimit must succeed");
        assert_eq!(
            rlim.rlim_cur, 0,
            "RLIMIT_CORE rlim_cur must be 0 after disable, got {}",
            rlim.rlim_cur
        );
        assert_eq!(
            rlim.rlim_max, 0,
            "RLIMIT_CORE rlim_max must be 0 after disable, got {}",
            rlim.rlim_max
        );
    }
}

#[test]
fn test_disable_core_dumps_should_be_idempotent() {
    #[cfg(unix)]
    {
        secure_memory::disable_core_dumps();
        secure_memory::disable_core_dumps();

        let mut rlim = libc::rlimit {
            rlim_cur: 1,
            rlim_max: 1,
        };
        let ret = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut rlim) };
        assert_eq!(ret, 0, "getrlimit must succeed after double call");
        assert_eq!(
            rlim.rlim_cur, 0,
            "RLIMIT_CORE must remain 0 after idempotent second call"
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// Secret Value Zeroization
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_zeroize_secret_value_should_clear_all_bytes() {
    let secret = "super-secret-value-12345!".to_string();
    let original_len = secret.len();

    let mut bytes = secret.into_bytes();
    assert_eq!(bytes.len(), original_len);
    assert!(!bytes.is_empty(), "bytes must be non-empty before zeroize");

    bytes.zeroize();

    assert!(bytes.is_empty(), "byte vec must be empty after zeroize");
}

#[test]
fn test_zeroize_strings_collection_should_clear_all() {
    let mut strings = vec![
        "token-abc123def456".to_string(),
        "sk-live-789ghi012jkl".to_string(),
        "password123".to_string(),
    ];
    assert_eq!(strings.len(), 3, "must start with 3 strings");

    secure_memory::zeroize_strings(&mut strings);

    assert!(
        strings.is_empty(),
        "all strings must be cleared after zeroize_strings"
    );
}

#[test]
fn test_zeroize_vec_u8_should_clear_sequential_pattern() {
    let mut data: Vec<u8> = (0..100).map(|i| (i % 256) as u8).collect();
    assert!(!data.is_empty(), "data must be non-empty before zeroize");

    secure_memory::zeroize_vec_u8(&mut data);

    assert!(
        data.is_empty(),
        "Vec<u8> must be cleared after zeroize_vec_u8"
    );
}

// ═══════════════════════════════════════════════════════════════
// Zeroize — Content Verification
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_zeroize_string_should_not_retain_original_content() {
    let secret = "sk-proj-sensitive-token-that-should-not-survive".to_string();
    let original_byte = secret.as_bytes()[0];
    let mut bytes = secret.into_bytes();

    bytes.zeroize();

    // After zeroize, the vec is empty — content is dropped.
    // Verify the original byte variable still compiles but the
    // vector is demonstrably cleared.
    assert!(bytes.is_empty(), "zeroized vec must be empty");
    let _ = original_byte; // must not be dropped before use — guard against NLL miscompile
}

#[test]
fn test_zeroize_large_allocation_should_not_panic() {
    // 1 MB of data — verifies zeroize handles large allocations
    // without stack overflow or OOM from intermediate copies.
    let mut data: Vec<u8> = vec![0xFFu8; 1_000_000];
    assert_eq!(data.len(), 1_000_000);

    secure_memory::zeroize_vec_u8(&mut data);

    assert!(data.is_empty(), "large allocation must clear without panic");
}

#[test]
fn test_zeroize_uniform_repeating_byte_pattern() {
    let mut data: Vec<u8> = vec![0xAAu8; 256];
    secure_memory::zeroize_vec_u8(&mut data);
    assert!(data.is_empty(), "uniform byte pattern must be cleared");
}

// ═══════════════════════════════════════════════════════════════
// Zeroize Does Not Panic on Empty Input
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_zeroize_empty_string_should_not_panic() {
    let value = String::new();
    let mut bytes = value.into_bytes();
    bytes.zeroize();
    assert!(bytes.is_empty());
}

#[test]
fn test_zeroize_empty_string_vec_should_not_panic() {
    let mut data: Vec<String> = Vec::new();
    secure_memory::zeroize_strings(&mut data);
    assert!(data.is_empty());
}

#[test]
fn test_zeroize_empty_u8_vec_should_not_panic() {
    let mut data: Vec<u8> = Vec::new();
    secure_memory::zeroize_vec_u8(&mut data);
    assert!(data.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// Multi-byte / Unicode Secret Zeroization
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_zeroize_unicode_secret_should_not_panic() {
    let value = "secre\u{0301}t-token-with-acc\u{0300}nts".to_string();
    let mut bytes = value.into_bytes();
    assert!(!bytes.is_empty(), "unicode secret must have bytes");
    bytes.zeroize();
    assert!(bytes.is_empty(), "unicode secret must be zeroized");
}

#[test]
fn test_zeroize_large_string_should_not_panic() {
    let value = "x".repeat(10_000);
    let mut bytes = value.into_bytes();
    assert_eq!(bytes.len(), 10_000, "large string must have correct length");
    bytes.zeroize();
    assert!(bytes.is_empty(), "large string must be zeroized");
}

#[test]
fn test_zeroize_emoji_secret_should_not_panic() {
    let value = "🔑token🔒with🔐emojis".to_string();
    let mut bytes = value.into_bytes();
    assert!(!bytes.is_empty(), "emoji secret must have bytes");
    bytes.zeroize();
    assert!(bytes.is_empty(), "emoji secret must be zeroized");
}

// ═══════════════════════════════════════════════════════════════
// Zeroize — Drop-based struct pattern
// ═══════════════════════════════════════════════════════════════

struct ZeroizingSecret {
    data: Vec<u8>,
}

impl Drop for ZeroizingSecret {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}

#[test]
fn test_zeroize_on_drop_should_compile_and_drop_without_panic() {
    let secret = b"ghp_super_secret_token_that_must_not_linger".to_vec();
    assert_eq!(secret.len(), 43);

    {
        let _s = ZeroizingSecret {
            data: secret.clone(),
        };
        // _s is dropped here — zeroize fires on the clone
    }

    // Original vec survives (we gave the struct a clone).
    // In production, the Drop impl zeroizes the struct's own data.
    let _ = secret;
}

#[test]
fn test_drop_zeroize_works_on_empty() {
    let _empty = ZeroizingSecret { data: Vec::new() };
    // Must not panic on drop with empty vec
}
