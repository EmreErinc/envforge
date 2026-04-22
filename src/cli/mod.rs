mod commands;
mod error;
mod secrets_cmd;
mod sync_cmd;
mod wizard;

use clap::{Parser, Subcommand};

pub use commands::*;
pub use error::{CliError, CliResult};
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
        /// Copy key name instead of value
        #[arg(long)]
        key_only: bool,
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

        /// Redact sensitive values as [REDACTED] (safe for AI tools)
        #[arg(long)]
        safe: bool,

        /// Generate .env.example from schema with placeholder values
        #[arg(long)]
        env_example: bool,

        /// Only export entries matching this query
        #[arg(long)]
        filter: Option<String>,

        /// Output format: dotenv, json, yaml, toml, docker, k8s, tfvars
        #[arg(long)]
        format: Option<String>,

        /// Kubernetes Secret name (for k8s format, default: envforge-secrets)
        #[arg(long)]
        k8s_name: Option<String>,

        /// Kubernetes namespace (for k8s format, default: default)
        #[arg(long)]
        k8s_namespace: Option<String>,
    },

    /// Manage Git merge driver for .env files
    Git {
        #[command(subcommand)]
        action: GitAction,
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

        /// Install git pre-commit hook that runs envforge scan --staged
        #[arg(long)]
        install_hook: bool,

        /// Remove the envforge pre-commit hook
        #[arg(long)]
        remove_hook: bool,

        /// Scan MCP config files for hardcoded credentials
        #[arg(long)]
        mcp: bool,
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
        /// Shell type (zsh, bash, fish, kiro, fig)
        shell: String,
        /// Install completion spec to the correct system path
        #[arg(long)]
        install: bool,
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

        /// Load and merge multiple profiles (comma-separated, last wins)
        #[arg(long)]
        profiles: Option<String>,

        /// Resolve secret references (ref:provider:path) at runtime
        #[arg(long)]
        resolve: bool,

        /// Load additional .env file(s) (can be repeated)
        #[arg(long = "env-file", num_args = 1)]
        env_files: Vec<String>,

        /// Override a specific variable (KEY=VALUE, can be repeated)
        #[arg(long = "override", num_args = 1)]
        overrides: Vec<String>,

        /// AI-agent-safe mode: resolve secrets in memory only, skip .env disk files
        #[arg(long)]
        volatile: bool,

        /// Redact known secret values in subprocess output
        #[arg(long)]
        redact: bool,

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

    /// Show all known info about a single environment variable
    Explain {
        /// Variable name to explain
        key: String,
    },

    /// Rotate a secret: update value, reset age, optionally push
    Rotate {
        /// Variable name to rotate
        key: String,

        /// Preview rotation without making changes
        #[arg(long)]
        dry_run: bool,

        /// Rotate all stale secrets interactively
        #[arg(long)]
        stale: bool,

        /// Auto-push to provider and sync after rotation (no interactive prompts)
        #[arg(long)]
        propagate: bool,
    },

    /// Generate shell hook for auto-loading (eval "$(envforge hook zsh)")
    Hook {
        /// Shell type (zsh, bash, fish)
        shell: String,
    },

    /// Output environment variables as shell export statements (for eval)
    Env {
        /// Directory to load from (default: current)
        #[arg(long)]
        dir: Option<String>,
    },

    /// Run health checks on EnvForge setup
    Doctor {
        /// Show detailed output for each check
        #[arg(long)]
        verbose: bool,
    },

    /// Run all checks: doctor + validate + scan + age + drift
    Check {
        /// Only run specific categories (comma-separated: doctor,validate,scan,age,drift)
        #[arg(long)]
        only: Option<String>,
    },

    /// Manage environment snapshots (backup/restore of active profile)
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },

    /// Share encrypted secrets with team members
    Share {
        #[command(subcommand)]
        action: ShareAction,
    },

    /// Resolve secret URIs in a config file (vault://path, aws-ssm://path, etc.)
    ResolveUri {
        /// Path to file with secret URIs
        file: String,
        /// Output as .env format (default: export statements)
        #[arg(long)]
        env: bool,
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<String>,
    },

    /// Harden MCP config files — replace plaintext secrets with env var references
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// View change audit trail from sync history
    Audit {
        /// Filter by key name
        #[arg(long)]
        key: Option<String>,
        /// Filter changes since date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
        /// Filter by machine ID
        #[arg(long)]
        machine: Option<String>,
        /// Number of entries to show
        #[arg(short, long, default_value = "50")]
        n: usize,
        /// Scan git history for secrets leaked in AI-assisted commits
        #[arg(long)]
        ai_leaks: bool,
        /// Show proxy access audit log
        #[arg(long)]
        access: bool,
    },

    /// Create AI tool ignore rules for all supported tools (Cursor, Copilot, Claude Code)
    Fence,

    /// Sanitize a file by replacing secret values with ${KEY} placeholders
    Sanitize {
        /// File to sanitize
        file: String,
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<String>,
    },

    /// Install or remove AI coding tool security hooks
    AiHook {
        #[command(subcommand)]
        action: AiHookAction,
    },

    /// AI agent guard — invoked by AI tool hooks (not for direct use)
    #[command(hide = true)]
    AiGuard {
        /// Hook stage: pre-tool, post-tool
        stage: String,
        /// Tool name
        tool_name: String,
        /// Tool input (JSON string or path)
        tool_input: Option<String>,
    },

    /// Start local credential proxy for AI agents
    Proxy {
        /// Port to listen on
        #[arg(long, default_value = "8100")]
        port: u16,
        /// Only serve these keys (comma-separated)
        #[arg(long)]
        keys: Option<String>,
        /// Profile to use
        #[arg(long)]
        profile: Option<String>,
        /// Allowed origins (comma-separated, default: localhost only)
        #[arg(long)]
        allow_origins: Option<String>,
        /// Require active lease for access
        #[arg(long)]
        require_lease: bool,
        /// Require human approval for each secret access
        #[arg(long)]
        require_approval: bool,
    },

    /// Manage secret access leases (time-bounded, revocable)
    Lease {
        #[command(subcommand)]
        action: LeaseAction,
    },

    /// Manage canary secrets (honeypot credentials for exfiltration detection)
    Canary {
        #[command(subcommand)]
        action: CanaryAction,
    },

    /// Emergency revoke all secret access
    Revoke {
        /// Revoke all active leases (killswitch)
        #[arg(long)]
        all: bool,
        /// Specific lease name to revoke
        name: Option<String>,
    },

    /// Show where an environment variable is referenced across your project
    Deps {
        /// Variable name
        key: String,
        /// Include source code scanning (slower)
        #[arg(long)]
        source: bool,
    },

    /// Show built-in manual page for a command
    Man {
        /// Command name (e.g., "list", "sync push", "secrets pull")
        command: Vec<String>,
    },

    /// Start Language Server Protocol server (for IDE extensions)
    Lsp,
}

#[derive(Subcommand)]
pub enum ShareAction {
    /// Create an encrypted share file
    Create {
        /// Recipient's age public key (age1...)
        #[arg(long)]
        recipient: String,
        /// Specific keys to share (comma-separated)
        #[arg(long)]
        keys: Option<String>,
        /// Share all keys
        #[arg(long)]
        all: bool,
        /// Filter by pattern
        #[arg(long)]
        filter: Option<String>,
        /// Output file path (default: envforge-share.age)
        #[arg(long, default_value = "envforge-share.age")]
        output: String,
        /// Expiry in hours
        #[arg(long)]
        expire: Option<u64>,
    },
    /// Receive and import a share file
    Receive {
        /// Path to share file
        file: String,
        /// Import keys into EnvForge config
        #[arg(long)]
        import: bool,
    },
}

#[derive(Subcommand)]
pub enum GitAction {
    /// Install EnvForge as a Git merge driver for .env files
    InstallMergeDriver,

    /// Remove the Git merge driver
    RemoveMergeDriver,

    /// Run merge (called by Git, not directly by users)
    #[command(hide = true)]
    Merge {
        /// Base file (ancestor)
        base: String,
        /// Ours file (current branch)
        ours: String,
        /// Theirs file (other branch)
        theirs: String,
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
    /// Output JSON Schema for .env.schema format
    JsonSchema,
    /// Generate AI-safe context file (names and types, no values)
    EmitAi {
        /// Output file path (default: stdout)
        #[arg(long)]
        output: Option<String>,
        /// Infer types from current env vars (when no .env.schema exists)
        #[arg(long)]
        infer: bool,
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

#[derive(Subcommand)]
pub enum McpAction {
    /// Replace plaintext secrets with ${VAR} env var references (backs up originals)
    Harden,
}

#[derive(Subcommand)]
pub enum AiHookAction {
    /// Install hooks for an AI coding tool
    Install {
        /// Tool name: claude-code, cursor
        tool: String,
    },
    /// Remove hooks from an AI coding tool
    Remove {
        /// Tool name: claude-code, cursor
        tool: String,
    },
}

#[derive(Subcommand)]
pub enum SnapshotAction {
    /// Create a snapshot of current environment variables
    Create {
        /// Snapshot name (default: auto-generated timestamp)
        name: Option<String>,
    },
    /// List all snapshots
    List,
    /// Restore environment variables from a snapshot
    Restore {
        /// Snapshot name (substring match)
        name: Option<String>,
        /// Restore the most recent snapshot
        #[arg(long)]
        last: bool,
    },
    /// Show diff between a snapshot and current environment
    Diff {
        /// Snapshot name (substring match)
        name: Option<String>,
        /// Diff against the most recent snapshot
        #[arg(long)]
        last: bool,
    },
    /// Delete a snapshot
    Delete {
        /// Snapshot name (substring match)
        name: String,
    },
}

#[derive(Subcommand)]
pub enum LeaseAction {
    /// Create a new time-bounded secret access lease
    Create {
        /// Lease name (default: auto-generated)
        #[arg(long)]
        name: Option<String>,
        /// Time-to-live (e.g., "1h", "30m", "8h", "24h", "7d")
        #[arg(long)]
        ttl: String,
        /// Restrict to specific keys (comma-separated)
        #[arg(long)]
        keys: Option<String>,
    },
    /// List all leases
    List,
    /// Clean up expired leases
    Cleanup,
}

#[derive(Subcommand)]
pub enum CanaryAction {
    /// Create a canary secret
    Create {
        /// Key name (e.g., AWS_SECRET_KEY)
        key: String,
        /// Pattern: aws_key, github_token, stripe_key, slack_token, gitlab_token, generic
        #[arg(long, default_value = "generic")]
        pattern: String,
    },
    /// List all canary secrets
    List,
    /// Check for triggered canaries
    Check,
    /// Delete a canary
    Delete {
        key: String,
    },
}
