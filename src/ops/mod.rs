pub mod ai_guard;
pub mod ai_hooks;
pub mod audit;
pub mod canary;
pub mod changelog;
pub mod check;
mod clipboard;
mod conflict;
mod crud;
pub mod deps;
pub mod doctor;
pub mod dotenv;
pub mod duplicates;
pub mod encrypt;
pub mod explain;
pub mod export_format;
pub mod fence;
pub mod fuzzy;
pub mod grouping;
pub mod hook;
pub mod lease;
mod listing;
pub mod man;
pub mod mcp_scan;
mod offset;
pub mod profile;
pub mod profile_diff;
pub mod proxy;
mod reference;
pub mod rotate;
pub mod run;
pub mod sanitize;
pub mod scanner;
pub mod schema;
pub mod schema_json;
pub mod secrets;
pub mod share;
pub mod snapshot;
pub mod sync;
pub mod undo;
pub mod uri_resolve;
pub mod validation;

// ─── Shared Error Type ───────────────────────────────────────

/// Common error type for ops-layer functions that previously used `Box<dyn Error>`.
///
/// Wraps all concrete error sources that ops modules produce, giving callers
/// typed access to the underlying cause while still allowing easy `?` propagation.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("{0}")]
    Parse(#[from] crate::model::ParseError),

    #[error("{0}")]
    Encrypt(#[from] encrypt::EncryptError),

    #[error("{0}")]
    Sync(#[from] sync::SyncError),

    #[error("{0}")]
    Crud(#[from] crud::OpsError),

    #[error("{0}")]
    Write(#[from] crate::config::WriteError),

    #[error("{0}")]
    Other(String),
}

impl From<String> for OpError {
    fn from(s: String) -> Self {
        OpError::Other(s)
    }
}

impl From<&str> for OpError {
    fn from(s: &str) -> Self {
        OpError::Other(s.to_string())
    }
}

pub use changelog::*;
pub use clipboard::*;
pub use conflict::*;
pub use crud::*;
pub use dotenv::*;
pub use duplicates::*;
pub use encrypt::*;
pub use fuzzy::*;
pub use grouping::*;
pub use listing::*;
pub use offset::*;
pub use profile::*;
pub use reference::*;
pub use scanner::*;
pub use undo::*;
pub use validation::*;
