use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::init::{read_config, CONFIG_FILE};
use super::marking::get_key_status;
use super::model::*;

/// Overrides directory name.
const OVERRIDES_DIR: &str = "overrides";

/// Get the override file path for a machine.
pub fn override_file_path(sync_path: &Path, machine_id: &str) -> PathBuf {
    sync_path
        .join(OVERRIDES_DIR)
        .join(format!("{}.toml", machine_id))
}

/// Read machine overrides from file.
pub fn read_overrides(path: &Path) -> Result<HashMap<String, String>, SyncError> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(path).map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| SyncError::ConfigParseError {
        message: e.to_string(),
    })
}

/// Write machine overrides to file.
pub fn write_overrides(path: &Path, overrides: &HashMap<String, String>) -> Result<(), SyncError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SyncError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let content = toml::to_string_pretty(overrides).map_err(|e| SyncError::ConfigParseError {
        message: e.to_string(),
    })?;

    std::fs::write(path, content).map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Set a machine-specific override for a key.
pub fn set_override(
    sync_path: &Path,
    machine_id: &str,
    key: &str,
    value: &str,
) -> Result<Vec<String>, SyncError> {
    let mut warnings = Vec::new();

    // Check if key is synced
    let config_path = sync_path.join(CONFIG_FILE);
    let config = read_config(&config_path)?;
    if get_key_status(key, &config) != KeyStatus::Synced {
        warnings.push(format!(
            "Key '{}' is not synced. Override has no effect until the key is marked for sync",
            key
        ));
    }

    let path = override_file_path(sync_path, machine_id);
    let mut overrides = read_overrides(&path)?;
    overrides.insert(key.to_string(), value.to_string());
    write_overrides(&path, &overrides)?;

    Ok(warnings)
}

/// Remove a machine-specific override.
pub fn remove_override(sync_path: &Path, machine_id: &str, key: &str) -> Result<bool, SyncError> {
    let path = override_file_path(sync_path, machine_id);
    let mut overrides = read_overrides(&path)?;
    let removed = overrides.remove(key).is_some();
    if removed {
        write_overrides(&path, &overrides)?;
    }
    Ok(removed)
}

/// List all overrides for a machine.
pub fn list_overrides(
    sync_path: &Path,
    machine_id: &str,
) -> Result<HashMap<String, String>, SyncError> {
    let path = override_file_path(sync_path, machine_id);
    read_overrides(&path)
}

/// Merge snapshot entries with machine overrides.
///
/// Override values take precedence over shared snapshot values.
pub fn merge_with_overrides(
    snapshot_entries: &[SyncEntry],
    overrides: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = snapshot_entries
        .iter()
        .map(|e| {
            let value = overrides
                .get(&e.key)
                .cloned()
                .unwrap_or_else(|| e.value.clone());
            (e.key.clone(), value)
        })
        .collect();

    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Get machine info for display.
pub fn machine_info(sync_path: &Path) -> Result<MachineInfo, SyncError> {
    let config_path = sync_path.join(CONFIG_FILE);
    let config = read_config(&config_path)?;
    let override_path = override_file_path(sync_path, &config.sync.machine_id);
    let override_count = read_overrides(&override_path)?.len();

    Ok(MachineInfo {
        machine_id: config.sync.machine_id,
        override_count,
    })
}

/// Machine info for display.
#[derive(Debug, Clone)]
pub struct MachineInfo {
    pub machine_id: String,
    pub override_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::sync::init::init_fresh;

    fn setup_sync() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");
        init_fresh(&sync_path, "test-a1b2").unwrap();

        // Mark a key as synced
        let config_path = sync_path.join(CONFIG_FILE);
        let mut config = read_config(&config_path).unwrap();
        config.manifest.sync_keys = vec!["DB_HOST".to_string(), "API_KEY".to_string()];
        crate::ops::sync::init::write_config(&config_path, &config).unwrap();

        (dir, sync_path)
    }

    #[test]
    fn test_set_override() {
        let (_dir, sync_path) = setup_sync();
        let warnings = set_override(&sync_path, "test-a1b2", "DB_HOST", "localhost").unwrap();
        assert!(warnings.is_empty()); // key is synced

        let overrides = list_overrides(&sync_path, "test-a1b2").unwrap();
        assert_eq!(overrides.get("DB_HOST"), Some(&"localhost".to_string()));
    }

    #[test]
    fn test_set_override_non_synced_key_warns() {
        let (_dir, sync_path) = setup_sync();
        let warnings = set_override(&sync_path, "test-a1b2", "NOT_SYNCED", "value").unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not synced"));
    }

    #[test]
    fn test_remove_override() {
        let (_dir, sync_path) = setup_sync();
        set_override(&sync_path, "test-a1b2", "DB_HOST", "localhost").unwrap();

        let removed = remove_override(&sync_path, "test-a1b2", "DB_HOST").unwrap();
        assert!(removed);

        let overrides = list_overrides(&sync_path, "test-a1b2").unwrap();
        assert!(!overrides.contains_key("DB_HOST"));
    }

    #[test]
    fn test_remove_override_nonexistent() {
        let (_dir, sync_path) = setup_sync();
        let removed = remove_override(&sync_path, "test-a1b2", "NOPE").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_list_overrides_empty() {
        let (_dir, sync_path) = setup_sync();
        let overrides = list_overrides(&sync_path, "test-a1b2").unwrap();
        assert!(overrides.is_empty());
    }

    #[test]
    fn test_list_overrides_multiple() {
        let (_dir, sync_path) = setup_sync();
        set_override(&sync_path, "test-a1b2", "DB_HOST", "localhost").unwrap();
        set_override(&sync_path, "test-a1b2", "API_KEY", "dev-key").unwrap();

        let overrides = list_overrides(&sync_path, "test-a1b2").unwrap();
        assert_eq!(overrides.len(), 2);
    }

    #[test]
    fn test_merge_with_overrides_no_overrides() {
        let entries = vec![
            SyncEntry {
                key: "A".to_string(),
                value: "shared_a".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "B".to_string(),
                value: "shared_b".to_string(),
                profile: None,
                group: None,
            },
        ];
        let overrides = HashMap::new();

        let merged = merge_with_overrides(&entries, &overrides);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], ("A".to_string(), "shared_a".to_string()));
        assert_eq!(merged[1], ("B".to_string(), "shared_b".to_string()));
    }

    #[test]
    fn test_merge_with_overrides_partial() {
        let entries = vec![
            SyncEntry {
                key: "A".to_string(),
                value: "shared_a".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "B".to_string(),
                value: "shared_b".to_string(),
                profile: None,
                group: None,
            },
        ];
        let mut overrides = HashMap::new();
        overrides.insert("A".to_string(), "override_a".to_string());

        let merged = merge_with_overrides(&entries, &overrides);
        assert_eq!(merged[0], ("A".to_string(), "override_a".to_string()));
        assert_eq!(merged[1], ("B".to_string(), "shared_b".to_string()));
    }

    #[test]
    fn test_machine_info() {
        let (_dir, sync_path) = setup_sync();
        let info = machine_info(&sync_path).unwrap();
        assert_eq!(info.machine_id, "test-a1b2");
        assert_eq!(info.override_count, 0);

        set_override(&sync_path, "test-a1b2", "DB_HOST", "localhost").unwrap();
        let info = machine_info(&sync_path).unwrap();
        assert_eq!(info.override_count, 1);
    }

    #[test]
    fn test_override_update_existing() {
        let (_dir, sync_path) = setup_sync();
        set_override(&sync_path, "test-a1b2", "DB_HOST", "first").unwrap();
        set_override(&sync_path, "test-a1b2", "DB_HOST", "second").unwrap();

        let overrides = list_overrides(&sync_path, "test-a1b2").unwrap();
        assert_eq!(overrides.get("DB_HOST"), Some(&"second".to_string()));
    }
}
