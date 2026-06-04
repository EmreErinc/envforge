use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("failed to read file: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("home directory could not be determined")]
    HomeDirNotFound,

    #[error("shell not detected: $SHELL environment variable not set")]
    ShellNotDetected,

    #[error("file too large ({size} bytes exceeds {limit}-byte limit); refusing to parse")]
    FileTooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },

    #[error("integrity check failed: hash mismatch")]
    IntegrityError {
        path: PathBuf,
        expected: String,
        actual: String,
    },
}
