use clap::Subcommand;

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Initialize project-scoped env management
    Init {
        /// Config format: toml, yaml, json
        #[arg(long, default_value = "toml")]
        format: String,
        /// Force reinitialize (overwrite existing config)
        #[arg(long)]
        force: bool,
    },
    /// Interactive guided project setup (init → schema → values)
    Wizard {
        /// Force re-run all steps
        #[arg(long)]
        force: bool,
    },
    /// Manage project environments
    Env {
        #[command(subcommand)]
        action: ProjectEnvAction,
    },
    /// Validate project env against schema
    Validate {
        /// Validate a specific environment (default: active)
        #[arg(long)]
        environment: Option<String>,
    },
    /// Scan project for leaked secrets
    Scan {
        /// Only scan git staged files
        #[arg(long)]
        staged: bool,
        /// Scan MCP config files
        #[arg(long)]
        mcp: bool,
    },
    /// Project schema tools
    Schema {
        #[command(subcommand)]
        action: ProjectSchemaAction,
    },
    /// Show or edit project configuration
    Config {
        /// Set a config value (key=value)
        #[arg(long)]
        set: Option<String>,
    },
    /// Show project health overview
    Status,
    /// Pull secrets from provider into project env
    Pull {
        /// Provider name
        #[arg(long)]
        from: String,
        /// Secret path in provider
        #[arg(long, default_value = "")]
        path: String,
        /// Filter keys by glob pattern
        #[arg(long)]
        filter: Option<String>,
        /// Target specific environment (default: active)
        #[arg(long)]
        environment: Option<String>,
    },
    /// Push project env to provider
    Push {
        /// Provider name
        #[arg(long)]
        to: String,
        /// Secret path in provider
        #[arg(long, default_value = "")]
        path: String,
        /// Specific keys to push (comma-separated)
        #[arg(long)]
        keys: Option<String>,
        /// Push all keys
        #[arg(long)]
        all: bool,
        /// Filter keys by glob pattern
        #[arg(long)]
        filter: Option<String>,
    },
    /// Create AI ignore rules for project
    Fence,
    /// Sanitize file using project env values
    Sanitize {
        /// File to sanitize
        file: String,
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<String>,
    },
    /// Export project env
    Export {
        /// Output file path
        path: Option<String>,
        /// Redact sensitive values
        #[arg(long)]
        safe: bool,
        /// Output format
        #[arg(long)]
        format: Option<String>,
        /// Filter keys
        #[arg(long)]
        filter: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProjectEnvAction {
    /// Create a new environment
    Create {
        /// Environment name
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// List all environments
    List,
    /// Switch active environment
    Switch {
        /// Environment name
        name: String,
    },
    /// Delete an environment
    Delete {
        /// Environment name
        name: String,
    },
    /// Compare two environments
    Diff {
        /// First environment name
        a: String,
        /// Second environment name
        b: String,
    },
}

#[derive(Subcommand)]
pub enum ProjectSchemaAction {
    /// Generate .env.schema from project env
    Generate {
        /// Output file path
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate AI-safe context file (no values)
    EmitAi {
        /// Output file path
        #[arg(long)]
        output: Option<String>,
        /// Infer types from current env vars
        #[arg(long)]
        infer: bool,
    },
}
