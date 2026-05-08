use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("failed to read file '{path}': {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("home directory could not be determined")]
    HomeDirNotFound,

    #[error("shell not detected: $SHELL environment variable not set")]
    ShellNotDetected,

    #[error("file '{path}' exceeds {limit}-byte size limit ({size} bytes); refusing to parse")]
    FileTooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
}
