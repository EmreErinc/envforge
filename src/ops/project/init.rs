use std::path::{Path, PathBuf};

use super::config::{
    detect_project_config, save_project_config, ConfigFormat, ProjectConfig, ProjectEnvironment,
    ProjectMeta, WizardState,
};
use super::ProjectError;

/// Options for project initialization.
pub struct InitOptions {
    pub root: PathBuf,
    pub format: ConfigFormat,
    pub project_name: String,
    pub default_env_name: String,
    pub env_file_path: PathBuf,
    pub schema_path: PathBuf,
    pub force: bool,
}

/// Result of project initialization.
#[derive(Debug)]
pub struct InitResult {
    pub config_path: PathBuf,
    pub env_file_path: PathBuf,
    pub project_name: String,
    pub environment_name: String,
    pub format: ConfigFormat,
    pub imported_keys: usize,
}

/// Initialize a new project. Creates config file and default environment .env file.
pub fn init_project(opts: &InitOptions) -> Result<InitResult, ProjectError> {
    // Check if already initialized (unless --force)
    if !opts.force {
        if let Some(detected) = detect_project_config(&opts.root) {
            return Err(ProjectError::AlreadyInitialized {
                path: detected.config_path,
            });
        }
    }

    // Build config
    let config = ProjectConfig {
        project: ProjectMeta {
            name: opts.project_name.clone(),
            schema_path: opts.schema_path.clone(),
            active_environment: opts.default_env_name.clone(),
        },
        wizard: WizardState {
            completed_steps: vec!["init".to_string()],
        },
        environments: vec![ProjectEnvironment {
            name: opts.default_env_name.clone(),
            env_file: opts.env_file_path.clone(),
            description: Some("Default environment".to_string()),
        }],
        ai_guard: crate::ops::project::config::AiGuardConfig::default(),
    };

    // Write config file
    let config_path = opts.root.join(opts.format.default_filename());
    save_project_config(&config, &config_path, opts.format)?;

    // Create .env file if it doesn't exist
    let env_path = opts.root.join(&opts.env_file_path);
    if !env_path.exists() {
        let stub = env_file_template(&opts.default_env_name);
        std::fs::write(&env_path, stub).map_err(|e| ProjectError::IoError {
            path: env_path.clone(),
            source: e,
        })?;
    }

    Ok(InitResult {
        config_path,
        env_file_path: env_path,
        project_name: opts.project_name.clone(),
        environment_name: opts.default_env_name.clone(),
        format: opts.format,
        imported_keys: 0,
    })
}

/// Import keys from an existing .env file into the project's env file.
/// Returns the number of keys imported.
pub fn import_existing_env(source: &Path, target: &Path) -> Result<usize, ProjectError> {
    let content = std::fs::read_to_string(source).map_err(|e| ProjectError::IoError {
        path: source.to_path_buf(),
        source: e,
    })?;

    // Count non-empty, non-comment lines
    let key_count = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .count();

    // Copy content to target
    std::fs::write(target, &content).map_err(|e| ProjectError::IoError {
        path: target.to_path_buf(),
        source: e,
    })?;

    Ok(key_count)
}

/// Render a starter `.env.<env>` file with commented examples.
/// Validates against `.env.schema` when present; safe to commit (no real secrets).
pub fn env_file_template(env_name: &str) -> String {
    format!(
        "# EnvForge project environment: {env_name}
#
# Add KEY=VALUE lines below. One per line. Values are NOT quoted unless they
# contain spaces or special characters.
#
# Variables defined in .env.schema are validated by `envforge project validate`.
# Sensitive keys (sensitive = true in schema) are masked when shown by the CLI.
#
# ─── Examples (uncomment to use) ───────────────────────────
#
# FOO=BAR
# DATABASE_URL=postgres://user:pass@localhost:5432/myapp
# PORT=8080
# LOG_LEVEL=info
# FEATURE_FLAG_X=false
"
    )
}

/// Derive project name from directory name.
pub fn derive_project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string()
}

/// Suggest adding .env patterns to .gitignore.
/// Returns true if .gitignore was updated.
pub fn add_to_gitignore(root: &Path) -> Result<bool, ProjectError> {
    let gitignore_path = root.join(".gitignore");
    let patterns = ".env.*\n!.env.schema\n!.env.example\n!.env.ai.md\n";

    if gitignore_path.exists() {
        let content =
            std::fs::read_to_string(&gitignore_path).map_err(|e| ProjectError::IoError {
                path: gitignore_path.clone(),
                source: e,
            })?;

        // Don't add if already present
        if content.contains(".env.*") {
            return Ok(false);
        }

        let updated = format!(
            "{}\n# EnvForge project environments\n{}",
            content.trim_end(),
            patterns
        );
        std::fs::write(&gitignore_path, updated).map_err(|e| ProjectError::IoError {
            path: gitignore_path,
            source: e,
        })?;
    } else {
        let content = format!("# EnvForge project environments\n{}", patterns);
        std::fs::write(&gitignore_path, content).map_err(|e| ProjectError::IoError {
            path: gitignore_path,
            source: e,
        })?;
    }

    Ok(true)
}
