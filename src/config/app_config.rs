use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{AnalyticsConfig, ParseError};

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
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub lifecycle: LifecycleConfig,
    #[serde(default)]
    pub analytics: AnalyticsConfig,
    #[serde(default)]
    pub fence: FenceConfig,
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

/// Security-related clipboard configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// Whether clipboard operations are allowed for secret values.
    /// When false, copying sensitive values to clipboard is blocked.
    pub enabled: bool,
    /// Whether to show a warning when copying sensitive values.
    pub warn_on_secret: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            warn_on_secret: true,
        }
    }
}

/// Lifecycle automation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Default stale threshold in days.
    #[serde(default = "default_stale_threshold")]
    pub default_stale_threshold_days: u32,
    /// Default grace period before decommission in days.
    #[serde(default = "default_grace_period")]
    pub default_grace_period_days: u32,
    /// Default rotation strategy.
    #[serde(default = "default_rotation_strategy")]
    pub default_rotation_strategy: String,
    /// Whether to auto-create lifecycle rules from schema.
    #[serde(default = "default_true")]
    pub auto_create_rules_from_schema: bool,
    /// Days to retain snapshots before auto-cleanup.
    #[serde(default = "default_snapshot_retention")]
    pub snapshot_retention_days: u32,
}

fn default_stale_threshold() -> u32 {
    90
}
fn default_grace_period() -> u32 {
    7
}
fn default_rotation_strategy() -> String {
    "replace".into()
}
fn default_true() -> bool {
    true
}
fn default_snapshot_retention() -> u32 {
    30
}

/// Configuration for the AI tool fence.
///
/// Controls which fence files are written when `envforge fence` runs.
/// All targets default to enabled — absent config is identical to writing all five files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FenceConfig {
    /// Per-target enable/disable flags.
    #[serde(default)]
    pub targets: FenceTargets,
}

/// Per-target enable flags, keyed by fence-target registry id.
///
/// Absent id => enabled (`true`) — so an empty map is byte-identical to
/// enabling every target (NFR5 backward-compat). Only explicit overrides
/// are stored/serialised.
///
/// Adding a new AI-tool target requires no new field here (NFR-M1): add
/// an entry to `src/ops/fence/registry.rs` and the new id is automatically
/// valid and defaults to enabled.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(transparent)]
pub struct FenceTargets {
    overrides: std::collections::BTreeMap<String, bool>,
}

impl<'de> serde::Deserialize<'de> for FenceTargets {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let map = std::collections::BTreeMap::<String, bool>::deserialize(deserializer)?;
        for key in map.keys() {
            if !crate::ops::fence::registry::is_valid_id(key) {
                return Err(serde::de::Error::custom(format!(
                    "unknown fence target '{key}'"
                )));
            }
        }
        Ok(Self { overrides: map })
    }
}

impl FenceTargets {
    /// Returns whether the given target is enabled according to this config.
    ///
    /// Absent key => `true` (NFR5 backward-compat / fail-safe default).
    /// This is the single decision point for "is target X enabled?" (NFR10).
    #[must_use]
    pub fn is_enabled(&self, target: crate::ops::fence::FenceTarget) -> bool {
        self.overrides.get(target.as_str()).copied().unwrap_or(true)
    }

    /// Sets the enabled state for a specific target.
    pub fn set_enabled(&mut self, target: crate::ops::fence::FenceTarget, enabled: bool) {
        self.overrides.insert(target.as_str().to_string(), enabled);
    }
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            default_stale_threshold_days: 90,
            default_grace_period_days: 7,
            default_rotation_strategy: "replace".into(),
            auto_create_rules_from_schema: true,
            snapshot_retention_days: 30,
        }
    }
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
            clipboard: ClipboardConfig::default(),
            lifecycle: LifecycleConfig::default(),
            analytics: AnalyticsConfig::default(),
            fence: FenceConfig::default(),
        }
    }
}

/// Get the default config directory path.
///
/// Respects `ENVFORGE_CONFIG_DIR` environment variable for test isolation.
/// Without the override, returns `$XDG_CONFIG_HOME/envforge` (or platform equivalent).
pub fn config_dir() -> Result<PathBuf, ParseError> {
    if let Ok(dir) = std::env::var("ENVFORGE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
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
/// Uses O_NOFOLLOW to prevent symlink attacks on config paths.
pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    let content = super::safe_fs::safe_read_to_string(path).map_err(|e| ConfigError::IoError {
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
/// Uses O_NOFOLLOW to prevent symlink attacks and enforces 0o600 permissions.
pub fn save_config(config: &AppConfig, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let content =
        toml::to_string_pretty(config).map_err(|e| ConfigError::SerializeError(e.to_string()))?;

    super::safe_fs::safe_write(path, &content).map_err(|e| ConfigError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| ConfigError::IoError {
                path: path.to_path_buf(),
                source: e,
            })?
            .permissions();
        if perms.mode() & 0o077 != 0 {
            perms.set_mode(0o600);
            std::fs::set_permissions(path, perms).map_err(|e| ConfigError::IoError {
                path: path.to_path_buf(),
                source: e,
            })?;
        }
    }

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
    #[error("failed to access config: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid config: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("failed to serialize config: {0}")]
    SerializeError(String),

    #[error("home directory could not be determined")]
    HomeDirNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiles_config_active_entry() {
        let config = ProfilesConfig::default();
        let entry = config.active_entry().unwrap();
        assert!(entry.file.contains("default"));
    }

    #[test]
    fn test_profiles_config_active_file() {
        let config = ProfilesConfig::default();
        let file = config.active_file().unwrap();
        assert!(file.contains("default"));
    }

    #[test]
    fn test_profiles_config_profile_names_sorted() {
        let mut config = ProfilesConfig::default();
        config.entries.insert(
            "staging".to_string(),
            ProfileEntry {
                file: "~/.env_managed.staging".to_string(),
            },
        );
        config.entries.insert(
            "alpha".to_string(),
            ProfileEntry {
                file: "~/.env_managed.alpha".to_string(),
            },
        );
        let names = config.profile_names();
        assert_eq!(names[0], "alpha");
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.general.default_shell, "zsh");
        assert!(config.files.use_reference_file);
        assert_eq!(config.profiles.active, "default");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");

        let config = AppConfig::default();
        save_config(&config, &path).unwrap();

        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.general.default_shell, config.general.default_shell);
        assert_eq!(loaded.profiles.active, config.profiles.active);
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.toml");
        std::fs::write(&path, "this is not valid TOML {{{").unwrap();

        let result = load_config(&path);
        assert!(matches!(result, Err(ConfigError::ParseError { .. })));
    }
}
