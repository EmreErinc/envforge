use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::ParseError;

/// EnvForge application configuration.
///
/// Stored at `~/.config/envforge/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub files: FilesConfig,
    pub offsets: OffsetsConfig,
    pub protected_blocks: ProtectedBlocksConfig,
    #[serde(default)]
    pub groups: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub profiles: ProfilesConfig,
    #[serde(default)]
    pub validation: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub default_shell: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    pub primary: String,
    pub reference: String,
    pub use_reference_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetsConfig {
    pub header_protected_lines: usize,
    pub footer_protected_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedBlocksConfig {
    pub markers: Vec<String>,
}

/// Profile configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilesConfig {
    /// Currently active profile name.
    pub active: String,
    /// Path to the shared ENV file (always sourced).
    pub shared_file: String,
    /// Named profiles, each with their own reference file.
    #[serde(flatten)]
    pub entries: HashMap<String, ProfileEntry>,
}

/// A single profile definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub file: String,
}

impl Default for ProfilesConfig {
    fn default() -> Self {
        let mut entries = HashMap::new();
        entries.insert(
            "default".to_string(),
            ProfileEntry {
                file: "~/.env_managed.default".to_string(),
            },
        );
        Self {
            active: "default".to_string(),
            shared_file: "~/.env_managed.shared".to_string(),
            entries,
        }
    }
}

impl ProfilesConfig {
    /// Get the active profile entry.
    pub fn active_entry(&self) -> Option<&ProfileEntry> {
        self.entries.get(&self.active)
    }

    /// Get the active profile's file path (with ~ expansion).
    pub fn active_file(&self) -> Option<String> {
        self.active_entry().map(|e| e.file.clone())
    }

    /// Get all profile names (sorted).
    pub fn profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.entries.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                default_shell: "zsh".to_string(),
            },
            files: FilesConfig {
                primary: "~/.zshrc".to_string(),
                reference: "~/.env_managed".to_string(),
                use_reference_file: true,
            },
            offsets: OffsetsConfig {
                header_protected_lines: 0,
                footer_protected_lines: 0,
            },
            protected_blocks: ProtectedBlocksConfig { markers: vec![] },
            groups: HashMap::new(),
            profiles: ProfilesConfig::default(),
            validation: HashMap::new(),
        }
    }
}

/// Get the default config directory path.
pub fn config_dir() -> Result<PathBuf, ParseError> {
    let base = dirs::config_dir().ok_or(ParseError::HomeDirNotFound)?;
    Ok(base.join("envforge"))
}

/// Get the default config file path.
pub fn config_file_path() -> Result<PathBuf, ParseError> {
    Ok(config_dir()?.join("config.toml"))
}

/// Get the backups directory path.
pub fn backups_dir() -> Result<PathBuf, ParseError> {
    Ok(config_dir()?.join("backups"))
}

/// Load configuration from a TOML file.
pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| ConfigError::ParseError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Load configuration from the default path, or create a default config if it doesn't exist.
pub fn load_or_create_default() -> Result<AppConfig, ConfigError> {
    let path = config_file_path().map_err(|_| ConfigError::HomeDirNotFound)?;

    if path.exists() {
        let mut config = load_config(&path)?;
        // Migration: if profiles section is empty/default and old reference file exists
        migrate_if_needed(&mut config)?;
        Ok(config)
    } else {
        let config = AppConfig::default();
        save_config(&config, &path)?;
        Ok(config)
    }
}

/// Migrate from old single-reference-file setup to profile-based.
///
/// If `~/.env_managed` exists and no profile files exist yet,
/// rename it to `~/.env_managed.default` and update config.
fn migrate_if_needed(config: &mut AppConfig) -> Result<(), ConfigError> {
    let old_ref = shellexpand_tilde(&config.files.reference);
    let default_profile_file = shellexpand_tilde(
        config
            .profiles
            .active_file()
            .as_deref()
            .unwrap_or("~/.env_managed.default"),
    );

    // Only migrate if old file exists and new profile file doesn't
    if old_ref.exists() && !default_profile_file.exists() && old_ref != default_profile_file {
        // Rename old → default profile
        std::fs::rename(&old_ref, &default_profile_file).map_err(|e| ConfigError::IoError {
            path: old_ref.clone(),
            source: e,
        })?;

        // Save updated config
        let config_path = config_file_path().map_err(|_| ConfigError::HomeDirNotFound)?;
        save_config(config, &config_path)?;
    }

    Ok(())
}

/// Save configuration to a TOML file.
pub fn save_config(config: &AppConfig, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let content =
        toml::to_string_pretty(config).map_err(|e| ConfigError::SerializeError(e.to_string()))?;

    std::fs::write(path, content).map_err(|e| ConfigError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Expand ~ to home directory.
fn shellexpand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to access '{path}': {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid config at '{path}': {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("failed to serialize config: {0}")]
    SerializeError(String),

    #[error("home directory could not be determined")]
    HomeDirNotFound,
}
