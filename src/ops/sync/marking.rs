use std::path::Path;

use super::init::{read_config, write_config};
use super::model::*;

/// Mark specific keys as synced.
pub fn mark_sync(
    config_path: &Path,
    keys: &[String],
    available_keys: &[String],
) -> Result<MarkResult, SyncError> {
    let mut config = read_config(config_path)?;
    let mut marked = Vec::new();
    let mut warnings = Vec::new();

    for key in keys {
        if !available_keys.contains(key) {
            warnings.push(format!("Key '{}' not found in environment", key));
            continue;
        }
        // Remove from local_keys if present
        config.manifest.local_keys.retain(|k| k != key);
        // Add to sync_keys if not already there
        if !config.manifest.sync_keys.contains(key) {
            config.manifest.sync_keys.push(key.clone());
        }
        marked.push(key.clone());
    }

    config.manifest.sync_keys.sort();
    write_config(config_path, &config)?;

    Ok(MarkResult {
        marked_keys: marked,
        warnings,
    })
}

/// Mark specific keys as local-only.
pub fn mark_local(
    config_path: &Path,
    keys: &[String],
    available_keys: &[String],
) -> Result<MarkResult, SyncError> {
    let mut config = read_config(config_path)?;
    let mut marked = Vec::new();
    let mut warnings = Vec::new();

    for key in keys {
        if !available_keys.contains(key) {
            warnings.push(format!("Key '{}' not found in environment", key));
            continue;
        }
        // Remove from sync_keys if present
        config.manifest.sync_keys.retain(|k| k != key);
        // Add to local_keys if not already there
        if !config.manifest.local_keys.contains(key) {
            config.manifest.local_keys.push(key.clone());
        }
        marked.push(key.clone());
    }

    config.manifest.local_keys.sort();
    write_config(config_path, &config)?;

    Ok(MarkResult {
        marked_keys: marked,
        warnings,
    })
}

/// Mark keys matching a glob pattern.
pub fn mark_by_pattern(
    config_path: &Path,
    pattern: &str,
    sync: bool,
    available_keys: &[String],
) -> Result<MarkResult, SyncError> {
    let matched: Vec<String> = available_keys
        .iter()
        .filter(|k| glob_match(pattern, k))
        .cloned()
        .collect();

    if matched.is_empty() {
        return Err(SyncError::PatternMatchesNothing {
            pattern: pattern.to_string(),
        });
    }

    // Also store the pattern in config
    let mut config = read_config(config_path)?;
    let glob = GlobPattern {
        pattern: pattern.to_string(),
        sync,
    };
    // Replace existing pattern if same, else add
    config.manifest.patterns.retain(|p| p.pattern != pattern);
    config.manifest.patterns.push(glob);
    write_config(config_path, &config)?;

    if sync {
        mark_sync(config_path, &matched, available_keys)
    } else {
        mark_local(config_path, &matched, available_keys)
    }
}

/// Mark all available keys as sync or local.
pub fn mark_all(
    config_path: &Path,
    sync: bool,
    available_keys: &[String],
) -> Result<MarkResult, SyncError> {
    if sync {
        mark_sync(config_path, available_keys, available_keys)
    } else {
        mark_local(config_path, available_keys, available_keys)
    }
}

/// Get the sync status of a single key.
pub fn get_key_status(key: &str, config: &SyncConfig) -> KeyStatus {
    if config.manifest.sync_keys.contains(&key.to_string()) {
        return KeyStatus::Synced;
    }
    if config.manifest.local_keys.contains(&key.to_string()) {
        return KeyStatus::LocalOnly;
    }
    // Check patterns
    for pat in &config.manifest.patterns {
        if glob_match(&pat.pattern, key) {
            return if pat.sync {
                KeyStatus::Synced
            } else {
                KeyStatus::LocalOnly
            };
        }
    }
    // Use default
    if config.sync.default_sync {
        KeyStatus::Synced
    } else {
        KeyStatus::Unset
    }
}

/// List all keys with their sync status.
pub fn list_keys_with_status(
    config: &SyncConfig,
    available_keys: &[String],
) -> Vec<(String, KeyStatus)> {
    available_keys
        .iter()
        .map(|k| (k.clone(), get_key_status(k, config)))
        .collect()
}

/// Simple glob matching (supports * and ?).
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_inner(&pat_chars, &text_chars)
}

fn glob_match_inner(pattern: &[char], text: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // Try matching * as empty, or consuming one text char
            glob_match_inner(&pattern[1..], text)
                || (!text.is_empty() && glob_match_inner(pattern, &text[1..]))
        }
        (Some('?'), Some(_)) => glob_match_inner(&pattern[1..], &text[1..]),
        (Some(p), Some(t)) if p == t => glob_match_inner(&pattern[1..], &text[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("AWS_*", "AWS_ACCESS_KEY"));
        assert!(glob_match("AWS_*", "AWS_"));
        assert!(!glob_match("AWS_*", "GCP_KEY"));
        assert!(glob_match("*_KEY", "API_KEY"));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("KEY_?", "KEY_A"));
        assert!(!glob_match("KEY_?", "KEY_AB"));
        assert!(!glob_match("KEY_?", "KEY_"));
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("EXACT", "EXACT"));
        assert!(!glob_match("EXACT", "EXACTX"));
    }

    #[test]
    fn test_get_key_status_explicit() {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: false,
                auto_push: false,
                conflict_strategy: ConflictStrategy::Ask,
                encrypted: true,
                verify_signatures: false,
            },
            manifest: ManifestConfig {
                sync_keys: vec!["DB_URL".to_string()],
                local_keys: vec!["SECRET".to_string()],
                patterns: vec![],
            },
        };

        assert_eq!(get_key_status("DB_URL", &config), KeyStatus::Synced);
        assert_eq!(get_key_status("SECRET", &config), KeyStatus::LocalOnly);
        assert_eq!(get_key_status("OTHER", &config), KeyStatus::Unset);
    }

    #[test]
    fn test_get_key_status_pattern() {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: false,
                auto_push: false,
                conflict_strategy: ConflictStrategy::Ask,
                encrypted: true,
                verify_signatures: false,
            },
            manifest: ManifestConfig {
                sync_keys: vec![],
                local_keys: vec![],
                patterns: vec![GlobPattern {
                    pattern: "AWS_*".to_string(),
                    sync: true,
                }],
            },
        };

        assert_eq!(get_key_status("AWS_KEY", &config), KeyStatus::Synced);
        assert_eq!(get_key_status("GCP_KEY", &config), KeyStatus::Unset);
    }

    #[test]
    fn test_get_key_status_default_sync() {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: true,
                auto_push: false,
                conflict_strategy: ConflictStrategy::Ask,
                encrypted: true,
                verify_signatures: false,
            },
            manifest: ManifestConfig::default(),
        };

        assert_eq!(get_key_status("ANY_KEY", &config), KeyStatus::Synced);
    }
}
