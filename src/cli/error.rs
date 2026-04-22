//! CLI-specific error types and conversions.

use std::io;
use thiserror::Error;

use crate::config::ConfigError;
use crate::model::ParseError;
use crate::ops::OpsError;

/// Errors that can occur during CLI command execution.
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration file error.
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    /// Shell file parsing error.
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// Business operation error.
    #[error("operation error: {0}")]
    Ops(#[from] OpsError),

    /// IO error (file read/write).
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// User-provided input error.
    #[error("{0}")]
    InvalidInput(String),

    /// Required resource not found.
    #[error("{0}")]
    NotFound(String),

    /// Git operation failed.
    #[error("git error: {0}")]
    Git(String),

    /// Sync operation failed.
    #[error("sync error: {0}")]
    Sync(String),

    /// Secret encryption/decryption error.
    #[error("secret error: {0}")]
    Secret(String),

    /// Schema validation error.
    #[error("schema error: {0}")]
    Schema(String),

    /// LSP or protocol error.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Generic CLI error with custom message.
    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// Create an invalid input error.
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not found error.
    pub fn not_found<S: Into<String>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a git error.
    pub fn git<S: Into<String>>(msg: S) -> Self {
        Self::Git(msg.into())
    }

    /// Create a sync error.
    pub fn sync<S: Into<String>>(msg: S) -> Self {
        Self::Sync(msg.into())
    }

    /// Create a secret error.
    pub fn secret<S: Into<String>>(msg: S) -> Self {
        Self::Secret(msg.into())
    }

    /// Create a schema error.
    pub fn schema<S: Into<String>>(msg: S) -> Self {
        Self::Schema(msg.into())
    }

    /// Create a protocol error.
    pub fn protocol<S: Into<String>>(msg: S) -> Self {
        Self::Protocol(msg.into())
    }

    /// Create a generic error.
    pub fn other<S: Into<String>>(msg: S) -> Self {
        Self::Other(msg.into())
    }
}

/// Convenience type alias for CLI results.
pub type CliResult<T> = Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_error_display() {
        let err = CliError::not_found("test.env");
        assert!(err.to_string().contains("test.env"));
    }

    #[test]
    fn test_invalid_input() {
        let err = CliError::invalid_input("bad format");
        assert_eq!(err.to_string(), "bad format");
    }
}
