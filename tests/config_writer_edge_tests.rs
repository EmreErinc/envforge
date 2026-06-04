// ═══════════════════════════════════════════════════════════════
// Config Writer Edge-Case Tests
// ═══════════════════════════════════════════════════════════════
// Tests for `src/config/writer.rs` edge cases not covered by
// the inline #[cfg(test)] module in writer.rs.
//
// Focus: parent directory creation, empty content, hash boundary
// conditions, safe_write backup interaction.

use std::path::Path;

use envforge::config::{atomic_write, compute_hash, WriteError};

// ═══════════════════════════════════════════════════════════════
// Atomic Write — Parent Directory Creation
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_atomic_write_creates_parent_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a/b/c/test.txt");
    assert!(!nested.parent().unwrap().exists());
    atomic_write(&nested, "content", None).unwrap();
    assert_eq!(std::fs::read_to_string(&nested).unwrap(), "content");
}

#[test]
fn test_atomic_write_empty_content() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("empty.txt");
    atomic_write(&path, "", None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn test_atomic_write_overwrite_existing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("overwrite.txt");
    std::fs::write(&path, "old").unwrap();
    atomic_write(&path, "new", None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[test]
fn test_atomic_write_hash_match_on_new_file() {
    // First-time write with a hash: since file doesn't exist,
    // hash verification is skipped (documented behavior)
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("new_with_hash.txt");
    let result = atomic_write(&path, "content", Some([0u8; 32]));
    // Should succeed — hash check skipped for new files
    assert!(result.is_ok());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "content");
}

#[test]
fn test_atomic_write_hash_mismatch_preserves_content() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("preserved.txt");
    let original = "important data";
    std::fs::write(&path, original).unwrap();

    let wrong_hash = compute_hash(b"something else");
    let result = atomic_write(&path, "overwrite attempt", Some(wrong_hash));
    assert!(matches!(result, Err(WriteError::HashMismatch { .. })));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

// ═══════════════════════════════════════════════════════════════
// Hash Computation
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_compute_hash_empty_data() {
    let hash = compute_hash(b"");
    // SHA-256 of empty string
    let expected: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(hash, expected);
}

#[test]
fn test_compute_hash_large_data() {
    let data = vec![0x41u8; 1_000_000]; // 1 MB of 'A'
    let h1 = compute_hash(&data);
    let h2 = compute_hash(&data);
    assert_eq!(h1, h2);
}

#[test]
fn test_compute_hash_deterministic_across_calls() {
    for _ in 0..100 {
        let h1 = compute_hash(b"consistent input");
        let h2 = compute_hash(b"consistent input");
        assert_eq!(h1, h2);
    }
}

// ═══════════════════════════════════════════════════════════════
// Safe Write — Backup Integration
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_safe_write_creates_file_when_none_exists() {
    use envforge::config::safe_write;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("first_time.txt");
    safe_write(&path, "hello", None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
}

#[test]
fn test_safe_write_overwrites_existing() {
    use envforge::config::safe_write;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("existing.txt");
    std::fs::write(&path, "old").unwrap();
    safe_write(&path, "new", None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

// ═══════════════════════════════════════════════════════════════
// WriteError — Display / Debug
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_write_error_hash_mismatch_display() {
    let err = WriteError::HashMismatch {
        path: Path::new("/tmp/test.txt").to_path_buf(),
    };
    let msg = err.to_string();
    assert!(msg.contains("hash mismatch"));
    assert!(msg.contains("modified externally"));
}

#[test]
fn test_write_error_temp_file_display() {
    let err = WriteError::TempFileError {
        dir: Path::new("/tmp").to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("temp file"));
}

// ═══════════════════════════════════════════════════════════════
// Stress: Multiple Sequential Writes
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_atomic_write_many_sequential_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("sequential.txt");

    for i in 0..100 {
        let content = format!("iteration_{}", i);
        atomic_write(&path, &content, None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
}
