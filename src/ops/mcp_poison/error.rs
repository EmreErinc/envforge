//! Errors for the MCP poisoning scanner.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScannerError {
    #[error("input too large: {size} bytes (limit {limit})")]
    InputTooLarge { size: usize, limit: usize },

    #[error("invalid schema JSON: {0}")]
    InvalidSchema(#[from] serde_json::Error),

    #[error("regex compile error: {0}")]
    Regex(#[from] regex::Error),

    #[error("internal: {0}")]
    Internal(String),
}
