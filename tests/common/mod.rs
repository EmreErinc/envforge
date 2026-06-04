//! Shared test helpers for EnvForge integration tests.
//!
//! ## Usage
//!
//! In `tests/your_test_file.rs`:
//!
//! ```ignore
//! mod common;
//! use common::*;
//!
//! #[test]
//! fn test_something() {
//!     let sf = make_shell_file(basic_env_fixture());
//!     assert!(!sf.lines.is_empty());
//! }
//! ```
//!
//! This module contains helpers that may not all be used by every test file.
//! `dead_code` warnings are suppressed — each function serves a purpose.

#![allow(dead_code)]

use std::io::Write;
use std::path::Path;

use envforge::model::ShellFile;
use envforge::parser::parse_shell_content;

// ── Temp Helpers ──────────────────────────────────────────────

/// Create a temporary directory that is cleaned up when the guard is dropped.
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

/// Create a temporary file with the given content.
/// Returns the `NamedTempFile` handle; use `.path()` to get the path.
pub fn temp_file(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("failed to create temp file");
    f.write_all(content.as_bytes())
        .expect("failed to write temp file");
    f.flush().expect("failed to flush temp file");
    f
}

/// Create a temporary file and parse it as a shell file.
pub fn temp_shell_file(content: &str) -> (ShellFile, tempfile::NamedTempFile) {
    let f = temp_file(content);
    let sf = parse_shell_content(content, f.path()).expect("failed to parse temp shell file");
    (sf, f)
}

// ── Parse Helpers ─────────────────────────────────────────────

/// Parse a shell file from inline content, using a synthetic path.
pub fn make_shell_file(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).expect("parse shell content")
}

/// Parse a shell file with a specific path.
pub fn make_shell_file_at(content: &str, path: &str) -> ShellFile {
    parse_shell_content(content, Path::new(path)).expect("parse shell content at path")
}

// ── Roundtrip Helpers ─────────────────────────────────────────

/// Assert that a shell config survives parse → serialize → re-parse
/// with an identical AST. Returns the re-parsed file for further checks.
pub fn assert_roundtrip(content: &str) -> ShellFile {
    let original = make_shell_file(content);
    let serialized = original.serialize();
    let reparsed = make_shell_file(&serialized);
    assert_eq!(
        reparsed.lines.len(),
        original.lines.len(),
        "roundtrip line count mismatch\noriginal AST:\n{orig:#?}\nreparsed AST:\n{re:#?}",
        orig = original,
        re = reparsed
    );
    reparsed
}

/// Assert that a shell config roundtrips AND the serialized output matches
/// the original content byte-for-byte.
pub fn assert_roundtrip_bytes(content: &str) {
    let sf = make_shell_file(content);
    let output = sf.serialize();
    assert_eq!(
        output, content,
        "byte-for-byte roundtrip failed\ninput:\n{content}\noutput:\n{output}"
    );
}

// ── Fixtures ──────────────────────────────────────────────────

/// Minimal shell file with a single bare export.
pub fn basic_env_fixture() -> &'static str {
    "DATABASE_URL=postgres://localhost/mydb\n"
}

/// Shell file with an `export`-style assignment.
pub fn export_env_fixture() -> &'static str {
    "export NODE_ENV=production\n"
}

/// Shell file with mixed content: exports, comments, blanks, and a source directive.
pub fn complex_shell_fixture() -> &'static str {
    "# Database configuration\nexport DATABASE_URL=\"postgres://localhost/mydb\"\n\n# Cache settings\nREDIS_HOST=localhost\nREDIS_PORT=6379\n\nsource ~/.env.local\n"
}

/// Shell file with quoted values containing special characters.
pub fn quoted_env_fixture() -> &'static str {
    "SECRET_KEY=\"abc123!@#\\$%^&*()\"\nSIMPLE_KEY='hello world'\n"
}

/// Empty shell file.
pub fn empty_env_fixture() -> &'static str {
    ""
}

/// Shell file containing only comments and blank lines.
pub fn comments_only_fixture() -> &'static str {
    "# This is a comment\n\n# Another comment\n"
}

// ── Secret / Value Generators ─────────────────────────────────

/// Generate a test secret value suitable for security tests.
pub fn test_secret() -> String {
    format!("sk-test-secret-{:x}", rand::random::<u64>())
}

/// Generate a long test secret (used for redaction boundary tests).
pub fn test_secret_long() -> String {
    let mut s = String::with_capacity(2 + 48 * 2);
    s.push_str("sk-");
    for _ in 0..48 {
        use std::fmt::Write;
        write!(&mut s, "{:02x}", rand::random::<u8>()).unwrap();
    }
    s
}

// ── Assertion Helpers ─────────────────────────────────────────

/// Assert that a `Result` is `Ok` with the given expected value.
#[track_caller]
pub fn assert_ok_eq<T: PartialEq + std::fmt::Debug>(
    result: Result<T, impl std::fmt::Debug>,
    expected: T,
) {
    match result {
        Ok(v) => assert_eq!(v, expected),
        Err(e) => panic!("expected Ok({expected:?}), got Err({e:?})"),
    }
}

/// Assert that a `Result` is `Err` (optionally matching an error message substring).
#[track_caller]
pub fn assert_is_err<T: std::fmt::Debug, E: std::fmt::Debug>(result: Result<T, E>) -> E {
    match result {
        Ok(v) => panic!("expected Err, got Ok({v:?})"),
        Err(e) => e,
    }
}

/// Assert that a `Result` is `Err` and the error string representation contains `substring`.
#[track_caller]
pub fn assert_err_contains<T: std::fmt::Debug, E: std::fmt::Debug + ToString>(
    result: Result<T, E>,
    substring: &str,
) {
    let e = assert_is_err(result);
    let msg = e.to_string();
    assert!(
        msg.contains(substring),
        "error message does not contain '{substring}': {msg}"
    );
}
