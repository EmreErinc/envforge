use std::path::PathBuf;

use crate::config::{load_or_create_default, safe_write};

use super::OpError;
use crate::ops::changelog::log_change;
use crate::ops::encrypt::{encrypt_value, is_encrypted};
use crate::ops::secrets::age::{load_sources, record_set};
use crate::ops::sync::{read_config, sync_dir, CONFIG_FILE};
use crate::ops::{collect_all_entries, edit_entry, EntryLocation};
use crate::parser::{parse_shell_file, serialize_shell_file};

/// All metadata needed before rotating a key.
pub struct RotationPlan {
    pub key: String,
    pub current_value: String,
    pub current_masked: String,
    pub source_file: PathBuf,
    pub source_hash: [u8; 32],
    pub has_provider: bool,
    pub provider_name: Option<String>,
    pub provider_path: Option<String>,
    pub is_synced: bool,
    pub is_encrypted: bool,
}

/// Result of applying a rotation.
pub struct RotationResult {
    pub key: String,
    pub local_updated: bool,
    pub age_reset: bool,
    pub provider_pushed: bool,
    pub sync_pushed: bool,
    pub logged: bool,
}

/// Mask a value for display: show first 2 and last 2 chars, mask middle.
///
/// - "sk-abc123def456" -> "sk****56"
/// - short values (<6 chars) -> "****"
/// - empty string -> "****"
pub fn mask_value(value: &str) -> String {
    if value.len() < 6 {
        return "****".to_string();
    }
    let first2 = crate::ops::sanitize::char_prefix(value, 2);
    let last2 = crate::ops::sanitize::char_suffix(value, 2);
    format!("{}****{}", first2, last2)
}

/// Build a rotation plan for a key by gathering all metadata.
pub fn plan_rotation(key: &str) -> Result<RotationPlan, OpError> {
    let config = load_or_create_default()?;
    let mut shell_files = Vec::new();

    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        shell_files.push(parse_shell_file(&primary)?);
    }

    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        shell_files.push(parse_shell_file(&ref_path)?);
    }

    if !config.profiles.active.is_empty() {
        if let Some(profile) = config.profiles.entries.get(&config.profiles.active) {
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                shell_files.push(parse_shell_file(&profile_path)?);
            }
        }
    }

    let shared = shellexpand(&config.profiles.shared_file);
    if shared.exists() {
        shell_files.push(parse_shell_file(&shared)?);
    }

    let entries = collect_all_entries(&shell_files);
    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    let current_value = entry.value.clone();
    let source_file = entry.source_file.clone();

    // Find shell file hash for safe_write
    let source_hash = shell_files
        .iter()
        .find(|sf| sf.path == source_file)
        .map(|sf| sf.hash)
        .unwrap_or([0u8; 32]);

    let encrypted = is_encrypted(&current_value);

    // Check age tracker for provider info
    let sources = load_sources().unwrap_or_default();
    let age_info = sources.secrets.get(key);
    let has_provider = age_info
        .map(|a| a.provider != "local" && !a.provider.is_empty())
        .unwrap_or(false);
    let provider_name = age_info
        .filter(|a| a.provider != "local" && !a.provider.is_empty())
        .map(|a| a.provider.clone());
    let provider_path = age_info
        .filter(|a| !a.path.is_empty())
        .map(|a| a.path.clone());

    let is_synced = check_sync_status(key);

    let masked = mask_value(&current_value);

    Ok(RotationPlan {
        key: key.to_string(),
        current_value,
        current_masked: masked,
        source_file,
        source_hash,
        has_provider,
        provider_name,
        provider_path,
        is_synced,
        is_encrypted: encrypted,
    })
}

/// Apply rotation: update value in shell file, reset age, log to changelog.
pub fn apply_rotation(
    key: &str,
    new_value: &str,
    plan: &RotationPlan,
) -> Result<RotationResult, OpError> {
    let mut sf = parse_shell_file(&plan.source_file)?;

    let write_value = if plan.is_encrypted {
        encrypt_value(new_value)?
    } else {
        new_value.to_string()
    };

    edit_entry(&mut sf, key, &write_value)?;

    let content = serialize_shell_file(&sf);
    safe_write(&sf.path, &content, Some(plan.source_hash))?;

    let provider = plan.provider_name.as_deref().unwrap_or("local");
    let path = plan.provider_path.as_deref().unwrap_or("");
    let age_reset = record_set(key, provider, path).is_ok();

    let config = load_or_create_default()?;
    let profile = &config.profiles.active;
    log_change(profile, "rotated", key, "secret rotated");

    Ok(RotationResult {
        key: key.to_string(),
        local_updated: true,
        age_reset,
        provider_pushed: false,
        sync_pushed: false,
        logged: true,
    })
}

/// Check whether a key is marked for sync.
fn check_sync_status(key: &str) -> bool {
    let sync_path = match sync_dir() {
        Ok(d) => d.join(CONFIG_FILE),
        Err(_) => return false,
    };
    if !sync_path.exists() {
        return false;
    }
    let config = match read_config(&sync_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    config.manifest.sync_keys.contains(&key.to_string())
}

fn shellexpand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_value_normal() {
        assert_eq!(mask_value("sk-abc123def456"), "sk****56");
    }

    #[test]
    fn test_mask_value_exactly_six() {
        assert_eq!(mask_value("abcdef"), "ab****ef");
    }

    #[test]
    fn test_mask_value_short() {
        assert_eq!(mask_value("abc"), "****");
    }

    #[test]
    fn test_mask_value_empty() {
        assert_eq!(mask_value(""), "****");
    }

    #[test]
    fn test_mask_value_five_chars() {
        assert_eq!(mask_value("12345"), "****");
    }

    #[test]
    fn test_mask_value_seven_chars() {
        assert_eq!(mask_value("abcdefg"), "ab****fg");
    }

    #[test]
    fn test_rotation_result_defaults() {
        let result = RotationResult {
            key: "TEST_KEY".to_string(),
            local_updated: true,
            age_reset: true,
            provider_pushed: false,
            sync_pushed: false,
            logged: true,
        };
        assert!(result.local_updated);
        assert!(result.age_reset);
        assert!(!result.provider_pushed);
        assert!(!result.sync_pushed);
        assert!(result.logged);
    }

    #[test]
    fn test_rotation_plan_fields() {
        let plan = RotationPlan {
            key: "API_KEY".to_string(),
            current_value: "sk-abc123".to_string(),
            current_masked: "sk****23".to_string(),
            source_file: PathBuf::from("/tmp/.zshrc"),
            source_hash: [0u8; 32],
            has_provider: true,
            provider_name: Some("vault".to_string()),
            provider_path: Some("secret/myapp".to_string()),
            is_synced: false,
            is_encrypted: false,
        };
        assert_eq!(plan.key, "API_KEY");
        assert!(plan.has_provider);
        assert_eq!(plan.provider_name.as_deref(), Some("vault"));
        assert!(!plan.is_synced);
        assert!(!plan.is_encrypted);
    }
}
