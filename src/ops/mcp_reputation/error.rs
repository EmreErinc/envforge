//! Error types for the MCP reputation subsystem.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeedError {
    #[error("corrupt gzip: {0}")]
    CorruptGzip(std::io::Error),

    #[error("corrupt JSON: {0}")]
    CorruptJson(String),

    #[error("invalid feed entry '{name}': {reason}")]
    InvalidEntry { name: String, reason: String },

    #[error("bundled feed is empty")]
    Empty,
}

impl Clone for FeedError {
    fn clone(&self) -> Self {
        match self {
            Self::CorruptGzip(e) => Self::CorruptGzip(std::io::Error::new(e.kind(), e.to_string())),
            Self::CorruptJson(s) => Self::CorruptJson(s.clone()),
            Self::InvalidEntry { name, reason } => Self::InvalidEntry {
                name: name.clone(),
                reason: reason.clone(),
            },
            Self::Empty => Self::Empty,
        }
    }
}

#[derive(Debug, Error)]
pub enum OverrideError {
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("corrupt override file: {0}")]
    Corrupt(serde_json::Error),

    #[error("override reason for '{name}' must be non-empty")]
    EmptyReason { name: String },

    #[error(
        "override for '{name}' refused: only UserTrusted tier writable (requested {requested})"
    )]
    InvalidTier { name: String, requested: String },
}
