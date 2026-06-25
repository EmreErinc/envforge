use std::path::{Path, PathBuf};

use crate::config::{config_file_path, save_config, AppConfig, ProfileEntry};
use crate::model::{LineNode, ShellFile};
use crate::ops::listing::{collect_entries, EnvEntry};
use crate::parser::parse_shell_file;

/// Expand ~ to home directory.
fn shellexpand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Strict profile-name validation: only `[A-Za-z0-9_-]{1,64}`.
///
/// Profile names get interpolated into a filesystem path
/// (`~/.env_managed.{name}`). Without validation, a name like
/// `../../etc/cron.d/evil` would resolve outside the user's home
/// directory and let the caller write/delete arbitrary files.
fn validate_profile_name(name: &str) -> Result<(), ProfileError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ProfileError::InvalidName(name.to_string()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ProfileError::InvalidName(name.to_string()));
    }
    // Reject leading dash so the name can never be parsed as a CLI flag
    // by a downstream caller.
    if name.starts_with('-') {
        return Err(ProfileError::InvalidName(name.to_string()));
    }
    Ok(())
}

/// Switch the active profile.
///
/// Updates config and replaces the profile source directive in the shell config.
pub fn switch_profile(
    config: &mut AppConfig,
    primary_shell_file: &mut ShellFile,
    new_profile: &str,
) -> Result<(), ProfileError> {
    validate_profile_name(new_profile)?;
    if !config.profiles.entries.contains_key(new_profile) {
        return Err(ProfileError::NotFound(new_profile.to_string()));
    }

    let old_file = config.profiles.active_file().unwrap_or_default();
    let new_entry = config.profiles.entries.get(new_profile).unwrap();
    let new_file = new_entry.file.clone();

    update_profile_source_directive(primary_shell_file, &old_file, &new_file);

    config.profiles.active = new_profile.to_string();

    let config_path = config_file_path().map_err(|_| ProfileError::ConfigError)?;
    save_config(config, &config_path).map_err(|_| ProfileError::ConfigError)?;

    Ok(())
}

/// Create a new profile.
pub fn create_profile(config: &mut AppConfig, name: &str) -> Result<PathBuf, ProfileError> {
    validate_profile_name(name)?;
    if config.profiles.entries.contains_key(name) {
        return Err(ProfileError::AlreadyExists(name.to_string()));
    }

    let file_path = format!("~/.env_managed.{}", name);
    config.profiles.entries.insert(
        name.to_string(),
        ProfileEntry {
            file: file_path.clone(),
        },
    );

    let expanded = shellexpand(&file_path);
    if !expanded.exists() {
        std::fs::write(&expanded, format!("# EnvForge profile: {}\n", name))
            .map_err(|_| ProfileError::ConfigError)?;
    }

    let config_path = config_file_path().map_err(|_| ProfileError::ConfigError)?;
    save_config(config, &config_path).map_err(|_| ProfileError::ConfigError)?;

    Ok(expanded)
}

/// Delete a profile (cannot delete active).
pub fn delete_profile(
    config: &mut AppConfig,
    name: &str,
    delete_file: bool,
) -> Result<(), ProfileError> {
    validate_profile_name(name)?;
    if config.profiles.active == name {
        return Err(ProfileError::CannotDeleteActive(name.to_string()));
    }

    let entry = config
        .profiles
        .entries
        .remove(name)
        .ok_or_else(|| ProfileError::NotFound(name.to_string()))?;

    if delete_file {
        let path = shellexpand(&entry.file);
        let _ = std::fs::remove_file(path); // Ignore error if file doesn't exist
    }

    let config_path = config_file_path().map_err(|_| ProfileError::ConfigError)?;
    save_config(config, &config_path).map_err(|_| ProfileError::ConfigError)?;

    Ok(())
}

/// Ensure the shared file exists and has a source directive in the shell config.
pub fn ensure_shared_file(
    config: &AppConfig,
    shell_file: &mut ShellFile,
    header_offset: usize,
    footer_offset: usize,
) -> Result<PathBuf, ProfileError> {
    let shared_path = shellexpand(&config.profiles.shared_file);

    if !shared_path.exists() {
        std::fs::write(&shared_path, "# EnvForge shared environment variables\n")
            .map_err(|_| ProfileError::ConfigError)?;
    }

    let has_shared = shell_file
        .lines
        .iter()
        .any(|node| node.original_text().contains("envforge:shared"));

    if !has_shared {
        let total = shell_file.lines.len();
        let safe_end = total.saturating_sub(footer_offset);

        if header_offset < safe_end {
            let shared_str = config.profiles.shared_file.clone();
            let comment = LineNode::Comment {
                line_number: safe_end,
                original_text: "# [envforge:shared] Shared environment variables".to_string(),
                text: " [envforge:shared] Shared environment variables".to_string(),
            };
            let source = LineNode::Other {
                line_number: safe_end + 1,
                original_text: format!("[ -f \"{}\" ] && source \"{}\"", shared_str, shared_str),
            };
            shell_file.lines.insert(safe_end, comment);
            shell_file.lines.insert(safe_end + 1, source);
        }
    }

    Ok(shared_path)
}

/// Ensure the active profile has a source directive in the shell config.
pub fn ensure_profile_source(
    config: &AppConfig,
    shell_file: &mut ShellFile,
    header_offset: usize,
    footer_offset: usize,
) -> Result<(), ProfileError> {
    let profile_file = config
        .profiles
        .active_file()
        .ok_or(ProfileError::NoActiveProfile)?;

    let has_profile = shell_file
        .lines
        .iter()
        .any(|node| node.original_text().contains("envforge:profile"));

    if !has_profile {
        let total = shell_file.lines.len();
        let safe_end = total.saturating_sub(footer_offset);

        if header_offset < safe_end {
            let comment = LineNode::Comment {
                line_number: safe_end,
                original_text:
                    "# [envforge:profile] Profile environment variables (managed by envforge)"
                        .to_string(),
                text: " [envforge:profile] Profile environment variables (managed by envforge)"
                    .to_string(),
            };
            let source = LineNode::Other {
                line_number: safe_end + 1,
                original_text: format!(
                    "[ -f \"{}\" ] && source \"{}\"",
                    profile_file, profile_file
                ),
            };
            shell_file.lines.insert(safe_end, comment);
            shell_file.lines.insert(safe_end + 1, source);
        }
    }

    Ok(())
}

/// Update the profile source directive to point to a new file.
fn update_profile_source_directive(shell_file: &mut ShellFile, old_file: &str, new_file: &str) {
    for node in &mut shell_file.lines {
        let text = node.original_text().to_string();
        if text.contains("envforge:profile") {
            // This is the comment line — skip, update the next source line
            continue;
        }
        if text.contains(&shellexpand(old_file).to_string_lossy().to_string())
            || text.contains(old_file)
        {
            let new_text = format!("[ -f \"{}\" ] && source \"{}\"", new_file, new_file);
            *node = LineNode::Other {
                line_number: node.line_number(),
                original_text: new_text,
            };
            break;
        }
    }
}

/// Load entries from shared + active profile + shell config with precedence.
///
/// Returns merged entries where:
/// - Profile entries override shared entries with same key
/// - Shared entries not in profile are included
/// - Shell config entries included as-is
pub fn load_profile_entries(config: &AppConfig, shell_file: &ShellFile) -> Vec<EnvEntry> {
    let mut all_entries = Vec::new();

    all_entries.extend(collect_entries(shell_file));

    let shared_path = shellexpand(&config.profiles.shared_file);
    if shared_path.exists() {
        if let Ok(shared_sf) = parse_shell_file(&shared_path) {
            all_entries.extend(collect_entries(&shared_sf));
        }
    }

    if let Some(profile_file) = config.profiles.active_file() {
        let profile_path = shellexpand(&profile_file);
        if profile_path.exists() {
            if let Ok(profile_sf) = parse_shell_file(&profile_path) {
                all_entries.extend(collect_entries(&profile_sf));
            }
        }
    }

    all_entries
}

/// Determine if a path is the shared file.
pub fn is_shared_file(config: &AppConfig, path: &Path) -> bool {
    let shared = shellexpand(&config.profiles.shared_file);
    path == shared
}

/// Determine if a path is the active profile file.
pub fn is_profile_file(config: &AppConfig, path: &Path) -> bool {
    if let Some(profile_file) = config.profiles.active_file() {
        let profile = shellexpand(&profile_file);
        path == profile
    } else {
        false
    }
}

/// Get a display name for an entry's source (shared, profile name, or file name).
pub fn source_display_name(config: &AppConfig, path: &Path) -> String {
    if is_shared_file(config, path) {
        "shared".to_string()
    } else if is_profile_file(config, path) {
        config.profiles.active.clone()
    } else {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("profile '{0}' not found")]
    NotFound(String),

    #[error("profile '{0}' already exists")]
    AlreadyExists(String),

    #[error("cannot delete active profile '{0}' — switch to another first")]
    CannotDeleteActive(String),

    #[error("no active profile configured")]
    NoActiveProfile,

    #[error("config error")]
    ConfigError,

    #[error("invalid profile name '{0}': must be 1-64 chars of [A-Za-z0-9_-], no leading dash")]
    InvalidName(String),
}
