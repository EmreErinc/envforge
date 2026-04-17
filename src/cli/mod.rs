mod commands;
mod secrets_cmd;
mod sync_cmd;
mod wizard;

use clap::{Parser, Subcommand};

pub use commands::*;
pub use secrets_cmd::*;
pub use sync_cmd::*;
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

    /// Validate ENV values against config rules and/or .env.schema
    Validate {
        /// Path to .env.schema (auto-detected if omitted)
        #[arg(long)]
        schema: Option<String>,

        /// Validate a specific .env file instead of EnvForge config
        #[arg(long = "env")]
        env_file: Option<String>,

        /// Environment name for schema overrides (e.g., production)
        #[arg(long)]
        environment: Option<String>,
    },

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

    /// Sync environment variables across machines
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },

    /// Manage secrets from external secret managers
    Secrets {
        #[command(subcommand)]
        action: SecretsAction,
    },

    /// Run a command with EnvForge-managed environment variables
    Run {
        /// Profile to use (default: active profile)
        #[arg(long)]
        profile: Option<String>,

        /// Resolve secret references (ref:provider:path) at runtime
        #[arg(long)]
        resolve: bool,

        /// Load additional .env file(s) (can be repeated)
        #[arg(long = "env-file", num_args = 1)]
        env_files: Vec<String>,

        /// Override a specific variable (KEY=VALUE, can be repeated)
        #[arg(long = "override", num_args = 1)]
        overrides: Vec<String>,

        /// Command and arguments to run (after --)
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Generate documentation from .env.schema
    Docs {
        /// Path to .env.schema (auto-detected if omitted)
        #[arg(long)]
        schema: Option<String>,

        /// Write output to file instead of stdout
        #[arg(long)]
        output: Option<String>,
    },

    /// Detect environment variable drift across .env files
    Drift {
        /// Path to .env.schema (auto-detected if omitted)
        #[arg(long)]
        schema: Option<String>,

        /// Environment name for schema overrides
        #[arg(long)]
        environment: Option<String>,

        /// .env files to compare
        #[arg(long = "envs", num_args = 1.., required = true)]
        env_files: Vec<String>,
    },

    /// Generate .env.schema from existing environment variables
    Schema {
        #[command(subcommand)]
        action: SchemaAction,
    },

    /// Interactive environment setup from .env.schema
    Init {
        /// Path to .env.schema (auto-detected if omitted)
        #[arg(long)]
        schema: Option<String>,

        /// Output .env file path
        #[arg(long, default_value = ".env")]
        output: String,
    },

    /// Run health checks on EnvForge setup
    Doctor {
        /// Show detailed output for each check
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
pub enum SchemaAction {
    /// Generate .env.schema from current environment variables
    Generate {
        /// Write output to file instead of stdout
        #[arg(long)]
        output: Option<String>,
    },
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

    /// Compare environment variables between two profiles
    Diff {
        /// First profile name
        a: String,

        /// Second profile name
        b: String,
    },
}
