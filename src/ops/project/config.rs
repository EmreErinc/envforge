use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ProjectError;

// ─── Config Format ──────────────────────────────────────────

/// Serialization format for project config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFormat {
    Toml,
    Yaml,
    Json,
}

/// Known config filenames and their formats.
const CONFIG_FILENAMES: &[(&str, ConfigFormat)] = &[
    (".envforge.project.toml", ConfigFormat::Toml),
    (".envforge.project.yaml", ConfigFormat::Yaml),
    (".envforge.project.yml", ConfigFormat::Yaml),
    (".envforge.project.json", ConfigFormat::Json),
];

impl ConfigFormat {
    /// Default filename for this format.
    pub fn default_filename(&self) -> &'static str {
        match self {
            ConfigFormat::Toml => ".envforge.project.toml",
            ConfigFormat::Yaml => ".envforge.project.yaml",
            ConfigFormat::Json => ".envforge.project.json",
        }
    }

    /// Parse format from string (user input).
    pub fn parse(s: &str) -> Result<Self, ProjectError> {
        match s.to_lowercase().as_str() {
            "toml" => Ok(ConfigFormat::Toml),
            "yaml" | "yml" => Ok(ConfigFormat::Yaml),
            "json" => Ok(ConfigFormat::Json),
            _ => Err(ProjectError::ParseError {
                path: PathBuf::new(),
                details: format!("Unknown config format '{}'. Supported: toml, yaml, json", s),
            }),
        }
    }
}

// ─── Project Config Model ───────────────────────────────────

/// Root project configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    #[serde(default)]
    pub wizard: WizardState,
    pub environments: Vec<ProjectEnvironment>,
    #[serde(default)]
    pub ai_guard: AiGuardConfig,
}

/// AI Guard configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiGuardConfig {
    #[serde(default)]
    pub hardening: crate::ops::hardening::HardeningConfig,
    #[serde(default)]
    pub scanners: crate::ops::external_scanner::ScannerRegistry,
}

/// Project metadata section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub schema_path: PathBuf,
    pub active_environment: String,
}

/// Single environment definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvironment {
    pub name: String,
    pub env_file: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Wizard progress tracking for resumability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WizardState {
    #[serde(default)]
    pub completed_steps: Vec<String>,
}

// ─── Detection ──────────────────────────────────────────────

/// Result of detecting a project config file.
#[derive(Debug, Clone)]
pub struct DetectedConfig {
    pub config_path: PathBuf,
    pub project_root: PathBuf,
    pub format: ConfigFormat,
}

/// Walk up directories from `start` looking for project config.
/// Stops at home directory or filesystem root.
pub fn detect_project_config(start: &Path) -> Option<DetectedConfig> {
    let home = dirs::home_dir();
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };

    loop {
        for &(filename, format) in CONFIG_FILENAMES {
            let candidate = current.join(filename);
            if candidate.is_file() {
                return Some(DetectedConfig {
                    config_path: candidate,
                    project_root: current,
                    format,
                });
            }
        }

        // Stop at home dir — don't search above
        if let Some(ref h) = home {
            if current == *h {
                return None;
            }
        }

        // Move to parent
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => return None,
        }
    }
}

// ─── Load / Save ────────────────────────────────────────────

/// Load project config from a detected config path.
pub fn load_project_config(detected: &DetectedConfig) -> Result<ProjectConfig, ProjectError> {
    let content =
        std::fs::read_to_string(&detected.config_path).map_err(|e| ProjectError::IoError {
            path: detected.config_path.clone(),
            source: e,
        })?;

    parse_project_config(&content, detected.format, &detected.config_path)
}

/// Parse project config from string content.
pub fn parse_project_config(
    content: &str,
    format: ConfigFormat,
    path: &Path,
) -> Result<ProjectConfig, ProjectError> {
    match format {
        ConfigFormat::Toml => toml::from_str(content).map_err(|e| ProjectError::ParseError {
            path: path.to_path_buf(),
            details: e.to_string(),
        }),
        ConfigFormat::Yaml => {
            serde_norway::from_str(content).map_err(|e| ProjectError::ParseError {
                path: path.to_path_buf(),
                details: e.to_string(),
            })
        }
        ConfigFormat::Json => serde_json::from_str(content).map_err(|e| ProjectError::ParseError {
            path: path.to_path_buf(),
            details: e.to_string(),
        }),
    }
}

/// Serialize project config to string in the given format.
pub fn serialize_project_config(
    config: &ProjectConfig,
    format: ConfigFormat,
) -> Result<String, ProjectError> {
    match format {
        ConfigFormat::Toml => {
            let body = toml::to_string_pretty(config).map_err(|e| ProjectError::ParseError {
                path: PathBuf::new(),
                details: e.to_string(),
            })?;
            Ok(format!("{}{}", PROJECT_TOML_HEADER, body))
        }
        ConfigFormat::Yaml => {
            let body = serde_norway::to_string(config).map_err(|e| ProjectError::ParseError {
                path: PathBuf::new(),
                details: e.to_string(),
            })?;
            Ok(format!("{}{}", PROJECT_YAML_HEADER, body))
        }
        ConfigFormat::Json => {
            // JSON has no comment syntax — emit as-is.
            serde_json::to_string_pretty(config).map_err(|e| ProjectError::ParseError {
                path: PathBuf::new(),
                details: e.to_string(),
            })
        }
    }
}

const PROJECT_TOML_HEADER: &str = "\
# EnvForge project config.
#
# Generated by `envforge project wizard`. Safe to commit.
# Docs: https://github.com/emreerinc/envforge#project-mode
#
# Sections:
#   [project]            name, schema path, active env
#   [wizard]             wizard resume state (completed_steps)
#   [[environments]]     one entry per env (development / staging / production / ...)
#   [ai_guard]           AI hardening config (fence / canary / scanners)

";

const PROJECT_YAML_HEADER: &str = "\
# EnvForge project config.
#
# Generated by `envforge project wizard`. Safe to commit.
# Docs: https://github.com/emreerinc/envforge#project-mode

";

/// Save project config to disk using atomic write (tempfile + rename).
pub fn save_project_config(
    config: &ProjectConfig,
    path: &Path,
    format: ConfigFormat,
) -> Result<(), ProjectError> {
    let content = serialize_project_config(config, format)?;

    // Atomic write: write to temp file in same dir, then rename
    let parent = path.parent().unwrap_or(Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| ProjectError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    std::fs::write(tmp.path(), &content).map_err(|e| ProjectError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    tmp.persist(path).map_err(|e| ProjectError::IoError {
        path: path.to_path_buf(),
        source: e.error,
    })?;

    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────

/// Get the active environment's .env file path, resolved relative to project root.
pub fn active_env_path(
    config: &ProjectConfig,
    project_root: &Path,
) -> Result<PathBuf, ProjectError> {
    let active = &config.project.active_environment;
    let env = config
        .environments
        .iter()
        .find(|e| e.name == *active)
        .ok_or_else(|| {
            let available = config
                .environments
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            ProjectError::EnvironmentNotFound {
                name: active.clone(),
                available,
            }
        })?;

    Ok(project_root.join(&env.env_file))
}

/// Validate environment name: lowercase alphanumeric + hyphens, non-empty.
pub fn validate_env_name(name: &str) -> Result<(), ProjectError> {
    if name.is_empty() {
        return Err(ProjectError::InvalidEnvironmentName {
            name: name.to_string(),
        });
    }

    let valid = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    if !valid || name.starts_with('-') || name.ends_with('-') {
        return Err(ProjectError::InvalidEnvironmentName {
            name: name.to_string(),
        });
    }

    Ok(())
}

/// Find a specific environment by name.
pub fn find_environment<'a>(
    config: &'a ProjectConfig,
    name: &str,
) -> Result<&'a ProjectEnvironment, ProjectError> {
    config
        .environments
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| {
            let available = config
                .environments
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            ProjectError::EnvironmentNotFound {
                name: name.to_string(),
                available,
            }
        })
}
