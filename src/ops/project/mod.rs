pub mod config;
pub mod init;
pub mod resolve;
pub mod wizard;

pub use config::*;
pub use init::*;
pub use resolve::*;
pub use wizard::*;

use std::path::PathBuf;

// ─── Error Type ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("No project config found. Run: envforge project init")]
    ConfigNotFound,

    #[error("Failed to parse project config at {path}: {details}")]
    ParseError { path: PathBuf, details: String },

    #[error("I/O error at {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Environment '{name}' not found. Available: {available}")]
    EnvironmentNotFound { name: String, available: String },

    #[error("Environment '{name}' already exists")]
    EnvironmentExists { name: String },

    #[error("Invalid environment name '{name}': must be lowercase alphanumeric + hyphens")]
    InvalidEnvironmentName { name: String },

    #[error("Schema not found at {path}")]
    SchemaNotFound { path: PathBuf },

    #[error("Project already initialized at {path}. Use --force to reinitialize")]
    AlreadyInitialized { path: PathBuf },

    #[error("Wizard error at step '{step}': {message}")]
    WizardError { step: String, message: String },
}
