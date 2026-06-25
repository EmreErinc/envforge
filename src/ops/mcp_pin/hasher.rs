//! Canonical JSON/JSONC hasher and binary file hasher.
//!
//! Foundation primitives for the MCP pin subsystem.
//!
//! - `CanonicalJsonHasher` strips JSONC comments via a hand-rolled byte-level
//!   state machine, recursively sorts object keys, re-emits in
//!   compact form, and SHA-256s the result.
//! - `BinaryHasher` streams SHA-256 over a binary file, canonicalizes its
//!   realpath, and records the symlink-target (if any).
//!
//! Both hashers are DoS-resistant: input-size cap, depth cap, time budget,
//! constant-memory streaming.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::types::Platform;

// ──────────────────────────────────────────────────────────────────────────────
// Error
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum HasherError {
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("input too large: {size} bytes (limit {limit})")]
    InputTooLarge { size: usize, limit: usize },

    #[error("nesting too deep: {depth} levels (limit {limit})")]
    DepthLimit { depth: usize, limit: usize },

    #[error("time budget exceeded: {elapsed_ms} ms (limit {limit_ms} ms)")]
    TimeBudgetExceeded { elapsed_ms: u128, limit_ms: u128 },

    #[error("unterminated block comment starting at byte offset {offset}")]
    UnterminatedBlockComment { offset: usize },

    #[error("broken symlink '{path}' → '{target}'")]
    BrokenSymlink { path: PathBuf, target: PathBuf },

    #[error("unknown platform: '{requested}'")]
    UnknownPlatform { requested: String },
}

// ──────────────────────────────────────────────────────────────────────────────
// CanonicalJson value object
// ──────────────────────────────────────────────────────────────────────────────

/// Canonical form of a JSON/JSONC document.
///
/// Two inputs that differ only by whitespace, JSONC comments, or object-key
/// order produce identical `CanonicalJson` values and therefore identical
/// SHA-256 digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalJson {
    bytes: Vec<u8>,
}

impl CanonicalJson {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CanonicalJsonHasher
// ──────────────────────────────────────────────────────────────────────────────

pub struct CanonicalJsonHasher;

impl CanonicalJsonHasher {
    pub const MAX_INPUT_BYTES: usize = 64 * 1024;
    pub const MAX_DEPTH: usize = 100;
    pub const TIME_BUDGET_MS: u128 = 50;

    /// Strip JSONC comments using a byte-level state machine.
    ///
    /// Preserves string contents byte-for-byte. Discards `//` line comments
    /// and `/* */` block comments outside string literals. Unterminated block
    /// comments are reported as a structured error rather than silently
    /// accepted.
    pub(crate) fn strip_jsonc_comments(input: &[u8]) -> Result<Vec<u8>, HasherError> {
        #[derive(Clone, Copy)]
        enum State {
            Normal,
            SlashSeen,
            InString,
            InEscape,
            InLineComment,
            InBlockComment,
            InBlockCommentMaybeEnd,
        }

        let mut out = Vec::with_capacity(input.len());
        let mut state = State::Normal;
        let mut block_comment_start: usize = 0;

        for (i, &b) in input.iter().enumerate() {
            match state {
                State::Normal => match b {
                    b'/' => state = State::SlashSeen,
                    b'"' => {
                        out.push(b);
                        state = State::InString;
                    }
                    _ => out.push(b),
                },
                State::SlashSeen => match b {
                    b'/' => state = State::InLineComment,
                    b'*' => {
                        block_comment_start = i - 1;
                        state = State::InBlockComment;
                    }
                    _ => {
                        // Not a comment; emit the deferred '/' plus current byte.
                        out.push(b'/');
                        if b == b'"' {
                            out.push(b);
                            state = State::InString;
                        } else {
                            out.push(b);
                            state = State::Normal;
                        }
                    }
                },
                State::InString => match b {
                    b'\\' => {
                        out.push(b);
                        state = State::InEscape;
                    }
                    b'"' => {
                        out.push(b);
                        state = State::Normal;
                    }
                    _ => out.push(b),
                },
                State::InEscape => {
                    out.push(b);
                    state = State::InString;
                }
                State::InLineComment => {
                    if b == b'\n' {
                        out.push(b);
                        state = State::Normal;
                    }
                    // else: discard
                }
                State::InBlockComment => {
                    if b == b'*' {
                        state = State::InBlockCommentMaybeEnd;
                    }
                    // else: discard
                }
                State::InBlockCommentMaybeEnd => match b {
                    b'/' => state = State::Normal,
                    b'*' => { /* stay in this state */ }
                    _ => state = State::InBlockComment,
                },
            }
        }

        // Trailing single '/' was deferred; emit it.
        if matches!(state, State::SlashSeen) {
            out.push(b'/');
        }

        if matches!(state, State::InBlockComment | State::InBlockCommentMaybeEnd) {
            return Err(HasherError::UnterminatedBlockComment {
                offset: block_comment_start,
            });
        }

        Ok(out)
    }

    /// Iterative depth check that walks structural punctuation outside strings.
    /// Cheaper than parse-then-recurse against adversarial inputs.
    fn check_depth(input: &[u8], limit: usize) -> Result<(), HasherError> {
        let mut depth: usize = 0;
        let mut max_depth: usize = 0;
        let mut in_string = false;
        let mut in_escape = false;
        for &b in input {
            if in_escape {
                in_escape = false;
                continue;
            }
            if in_string {
                match b {
                    b'\\' => in_escape = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > max_depth {
                        max_depth = depth;
                    }
                    if depth > limit {
                        return Err(HasherError::DepthLimit { depth, limit });
                    }
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        let _ = max_depth;
        Ok(())
    }

    /// Recursively sort object keys; arrays preserve order.
    fn sort_keys(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                let mut out = serde_json::Map::with_capacity(entries.len());
                for (k, v) in entries {
                    out.insert(k, Self::sort_keys(v));
                }
                serde_json::Value::Object(out)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Self::sort_keys).collect())
            }
            other => other,
        }
    }

    /// Canonicalize a JSON/JSONC byte slice to its canonical UTF-8 form.
    pub fn canonicalize(input: &[u8]) -> Result<CanonicalJson, HasherError> {
        let start = Instant::now();

        if input.len() > Self::MAX_INPUT_BYTES {
            return Err(HasherError::InputTooLarge {
                size: input.len(),
                limit: Self::MAX_INPUT_BYTES,
            });
        }

        let stripped = Self::strip_jsonc_comments(input)?;
        Self::check_time(start)?;

        Self::check_depth(&stripped, Self::MAX_DEPTH)?;
        Self::check_time(start)?;

        let parsed: serde_json::Value = serde_json::from_slice(&stripped)?;
        Self::check_time(start)?;

        let sorted = Self::sort_keys(parsed);
        Self::check_time(start)?;

        // Compact (no whitespace) UTF-8 emission.
        let bytes = serde_json::to_vec(&sorted)?;
        Self::check_time(start)?;

        Ok(CanonicalJson { bytes })
    }

    /// SHA-256 of a canonical form.
    pub fn hash(canonical: &CanonicalJson) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&canonical.bytes);
        hasher.finalize().into()
    }

    /// Fused canonicalize + hash pipeline.
    pub fn canonicalize_and_hash(input: &[u8]) -> Result<[u8; 32], HasherError> {
        let c = Self::canonicalize(input)?;
        Ok(Self::hash(&c))
    }

    fn check_time(start: Instant) -> Result<(), HasherError> {
        let elapsed = start.elapsed().as_millis();
        if elapsed > Self::TIME_BUDGET_MS {
            return Err(HasherError::TimeBudgetExceeded {
                elapsed_ms: elapsed,
                limit_ms: Self::TIME_BUDGET_MS,
            });
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// BinaryHasher
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedBinary {
    pub realpath: PathBuf,
    pub symlink_target: Option<PathBuf>,
    pub sha256: [u8; 32],
    pub platform: Platform,
}

pub struct BinaryHasher;

impl BinaryHasher {
    /// Hash a binary file, canonicalizing its realpath and recording the
    /// symlink target when the input path differs from the resolved realpath.
    pub fn hash_binary(path: &Path) -> Result<HashedBinary, HasherError> {
        let symlink_meta = std::fs::symlink_metadata(path).map_err(|e| HasherError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let realpath = std::fs::canonicalize(path).map_err(|e| {
            // Detect broken symlink for a clearer error.
            if symlink_meta.file_type().is_symlink() && e.kind() == std::io::ErrorKind::NotFound {
                if let Ok(target) = std::fs::read_link(path) {
                    return HasherError::BrokenSymlink {
                        path: path.to_path_buf(),
                        target,
                    };
                }
            }
            HasherError::Io {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        // Record the symlink's literal target whenever the input is a
        // symlink. The earlier `t != realpath` filter was meant to
        // avoid recording redundant data, but on platforms where the
        // tempdir base is itself canonicalized (notably macOS, where
        // `/var/folders/...` → `/private/var/folders/...`) the
        // comparison rejects legitimate symlink-target metadata.
        // Always storing the read_link value gives a faithful audit
        // trail and removes a platform-dependent test flake.
        let symlink_target = if symlink_meta.file_type().is_symlink() {
            std::fs::read_link(path).ok()
        } else {
            None
        };

        let sha256 = Self::streaming_sha256(&realpath)?;

        Ok(HashedBinary {
            realpath,
            symlink_target,
            sha256,
            platform: Platform::current(),
        })
    }

    /// Constant-memory streaming SHA-256.
    fn streaming_sha256(path: &Path) -> Result<[u8; 32], HasherError> {
        let mut file = File::open(path).map_err(|e| HasherError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = file.read(&mut buf).map_err(|e| HasherError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize().into())
    }

    pub fn current_platform() -> Platform {
        Platform::current()
    }
}
