mod commands;
mod wizard;

use clap::{Parser, Subcommand};

pub use commands::*;
pub use wizard::*;

#[derive(Parser)]
#[command(
    name = "envforge",
    version,
    about = "Terminal environment variable manager"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Preview changes without writing to disk
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all environment variables
    List,

    /// Get the value of a specific variable
    Get {
        /// Variable name
        key: String,
    },

    /// Set a variable (create or update)
    Set {
        /// KEY=VALUE pair
        assignment: String,
    },

    /// Soft-delete a variable
    Delete {
        /// Variable name
        key: String,
    },

    /// Copy a variable's value to clipboard
    Copy {
        /// Variable name
        key: String,
    },

    /// Move a variable to the reference file
    Move {
        /// Variable name
        key: String,
    },

    /// Import variables from a .env file
    Import {
        /// Path to .env file
        path: String,

        /// Overwrite existing keys without prompting
        #[arg(long)]
        force: bool,
    },

    /// Export variables to .env format
    Export {
        /// Output file path (stdout if omitted)
        path: Option<String>,

        /// Exclude sensitive keys (SECRET, TOKEN, PASSWORD, etc.)
        #[arg(long)]
        exclude_sensitive: bool,

        /// Only export entries matching this query
        #[arg(long)]
        filter: Option<String>,
    },

    /// Detect and list duplicate keys
    Duplicates,

    /// Scan files for leaked secrets
    Scan {
        /// Path to scan (default: current directory)
        path: Option<String>,

        /// Only scan git staged files
        #[arg(long)]
        staged: bool,
    },

    /// Show pending changes as a diff
    Diff,

    /// Backup operations
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Manage environment profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Validate ENV values against config rules
    Validate,

    /// Generate shell completion scripts
    Completions {
        /// Shell type (zsh, bash, fish)
        shell: String,
    },

    /// Encrypt a variable's value
    Encrypt {
        /// Variable name
        key: String,
    },

    /// Decrypt a variable's value
    Decrypt {
        /// Variable name
        key: String,
    },

    /// View change history
    Log {
        /// Filter by key name
        key: Option<String>,

        /// Number of entries to show
        #[arg(short, long, default_value = "50")]
        n: usize,
    },

    /// Show current configuration
    Config,
}

#[derive(Subcommand)]
pub enum BackupAction {
    /// Restore from a backup file
    Restore {
        /// Path to backup file
        file: String,
    },

    /// List available backups
    List,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List all profiles
    List,

    /// Switch to a profile
    Switch {
        /// Profile name
        name: String,
    },

    /// Create a new profile
    Create {
        /// Profile name
        name: String,
    },

    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
}
