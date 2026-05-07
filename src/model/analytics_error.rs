use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("storage error at '{path}': {source}")]
    StorageError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse event: {source}")]
    EventParseError { source: serde_json::Error },

    #[error("invalid time window: {description}")]
    InvalidTimeWindow { description: String },

    #[error("no data available: {message}")]
    NoDataAvailable { message: String },

    #[error("provider not found: {provider}")]
    ProviderNotFound { provider: String },

    #[error("export error ({format}): {reason}")]
    ExportError { format: String, reason: String },
}
