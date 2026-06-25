//! `PoisonFinding` and `Severity`.
//!
//! No raw-text field. Raw matched text is consumed at `new()`
//! time to compute the SHA-256 and pre-render the redacted marker, then
//! dropped. No accessor returns the raw matched string.

use sha2::{Digest, Sha256};

use crate::ops::mcp_poison::patterns::PATTERN_SET_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoisonFinding {
    pub tool_name: String,
    pub pattern_id: &'static str,
    pub pattern_set_version: &'static str,
    pub severity: Severity,
    pub span: (usize, usize),
    pub text_sha256: [u8; 32],
    pub decoded_layer: u8,
    pub canonicalized: bool,
    pub redacted_marker: String,
}

impl PoisonFinding {
    /// Construct a finding. Consumes `matched_text` by reference, hashes
    /// it for `text_sha256`, builds the redacted marker, and drops the
    /// borrow. The raw matched text is never stored in the struct.
    pub fn new(
        tool_name: &str,
        pattern_id: &'static str,
        severity: Severity,
        span: (usize, usize),
        matched_text: &str,
        decoded_layer: u8,
        canonicalized: bool,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(matched_text.as_bytes());
        let text_sha256: [u8; 32] = hasher.finalize().into();
        let sha_prefix = hex::encode(&text_sha256[..4]);
        let len = span.1.saturating_sub(span.0);
        let redacted_marker = format!(
            "[REDACTED:{pattern_id} @offset={} len={} sha256={sha_prefix}]",
            span.0, len
        );
        Self {
            tool_name: tool_name.to_string(),
            pattern_id,
            pattern_set_version: PATTERN_SET_VERSION,
            severity,
            span,
            text_sha256,
            decoded_layer,
            canonicalized,
            redacted_marker,
        }
    }

    pub fn text_sha256_hex(&self) -> String {
        hex::encode(self.text_sha256)
    }
}
