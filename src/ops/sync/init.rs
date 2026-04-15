use std::path::{Path, PathBuf};

use super::git::{GitCommandRunner, GitOps};
use super::model::*;

/// Default sync directory name under ~/.envforge/
const SYNC_DIR_NAME: &str = "sync";

/// Default git branch name.
const DEFAULT_BRANCH: &str = "main";

/// Snapshot file name.
pub const SNAPSHOT_FILE: &str = "snapshot.toml";

/// Sync config file name.
pub const CONFIG_FILE: &str = "sync-config.toml";

/// Overrides directory name.
const OVERRIDES_DIR: &str = "overrides";

/// Gitignore contents for sync repo.
const GITIGNORE_CONTENT: &str = "*.backup\n.DS_Store\nsync-log.toml\n";

// ─── Path Helpers ────────────────────────────────────────────

/// Get the default sync directory path (~/.envforge/sync/).
pub fn sync_dir() -> Result<PathBuf, SyncError> {
    let home = dirs::home_dir().ok_or(SyncError::IoError {
        path: PathBuf::from("~"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "home directory not found"),
    })?;
    Ok(home.join(".envforge").join(SYNC_DIR_NAME))
}

/// Check if sync repo is initialized at the given path.
pub fn is_initialized(base_path: &Path) -> bool {
    base_path.join(".git").is_dir() && base_path.join(CONFIG_FILE).is_file()
}

// ─── Machine ID ──────────────────────────────────────────────

/// Generate a machine ID from hostname + random suffix.
pub fn generate_machine_id(custom: Option<&str>) -> Result<String, SyncError> {
    match custom {
        Some(id) => {
            validate_machine_id(id)?;
            Ok(id.to_string())
        }
        None => {
            let hostname = get_hostname();
            let sanitized = sanitize_hostname(&hostname);
            let suffix = random_hex_suffix();
            Ok(format!("{}-{}", sanitized, suffix))
        }
    }
}

/// Validate a custom machine ID format.
fn validate_machine_id(id: &str) -> Result<(), SyncError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(SyncError::InvalidMachineId { id: id.to_string() });
    }
    Ok(())
}

/// Get system hostname.
fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Sanitize hostname: lowercase, replace invalid chars, truncate.
fn sanitize_hostname(hostname: &str) -> String {
    let sanitized: String = hostname
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Trim leading/trailing dashes, truncate to 50 chars
    let trimmed = sanitized.trim_matches('-');
    if trimmed.len() > 50 {
        trimmed[..50].trim_end_matches('-').to_string()
    } else {
        trimmed.to_string()
    }
}

/// Generate a 4-character hex suffix.
fn random_hex_suffix() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: [u8; 2] = rng.random();
    format!("{:02x}{:02x}", bytes[0], bytes[1])
}

// ─── Snapshot I/O ────────────────────────────────────────────

/// Read a sync snapshot from a TOML file.
pub fn read_snapshot(path: &Path) -> Result<SyncSnapshot, SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| SyncError::SnapshotParseError {
        message: e.to_string(),
    })
}

/// Write a sync snapshot to a TOML file atomically.
pub fn write_snapshot(path: &Path, snapshot: &SyncSnapshot) -> Result<(), SyncError> {
    let content = toml::to_string_pretty(snapshot).map_err(|e| SyncError::SnapshotParseError {
        message: e.to_string(),
    })?;

    atomic_write(path, &content)
}

// ─── Config I/O ──────────────────────────────────────────────

/// Read sync config from a TOML file.
pub fn read_config(path: &Path) -> Result<SyncConfig, SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| SyncError::ConfigParseError {
        message: e.to_string(),
    })
}

/// Write sync config to a TOML file.
pub fn write_config(path: &Path, config: &SyncConfig) -> Result<(), SyncError> {
    let content = toml::to_string_pretty(config).map_err(|e| SyncError::ConfigParseError {
        message: e.to_string(),
    })?;

    atomic_write(path, &content)
}

// ─── Atomic Write ────────────────────────────────────────────

/// Write content to a file atomically using tempfile + rename.
fn atomic_write(path: &Path, content: &str) -> Result<(), SyncError> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SyncError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let dir = path.parent().unwrap_or(Path::new("."));
    let mut temp = NamedTempFile::new_in(dir).map_err(|e| SyncError::IoError {
        path: dir.to_path_buf(),
        source: e,
    })?;

    temp.write_all(content.as_bytes())
        .map_err(|e| SyncError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;

    temp.flush().map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    temp.persist(path).map_err(|e| SyncError::IoError {
        path: path.to_path_buf(),
        source: e.into(),
    })?;

    Ok(())
}

// ─── Repo Initialization ─────────────────────────────────────

/// Initialize a fresh sync repo (no remote).
pub fn init_fresh(base_path: &Path, machine_id: &str) -> Result<(), SyncError> {
    if is_initialized(base_path) {
        return Err(SyncError::RepoAlreadyInitialized {
            path: base_path.to_path_buf(),
        });
    }

    // Create directory structure
    std::fs::create_dir_all(base_path).map_err(|e| SyncError::IoError {
        path: base_path.to_path_buf(),
        source: e,
    })?;

    let overrides_path = base_path.join(OVERRIDES_DIR);
    std::fs::create_dir_all(&overrides_path).map_err(|e| SyncError::IoError {
        path: overrides_path,
        source: e,
    })?;

    // Initialize git repo
    let git = GitCommandRunner::new(base_path.to_path_buf());
    git.init(DEFAULT_BRANCH)?;

    // Write .gitignore
    std::fs::write(base_path.join(".gitignore"), GITIGNORE_CONTENT).map_err(|e| {
        SyncError::IoError {
            path: base_path.join(".gitignore"),
            source: e,
        }
    })?;

    // Write default config
    let config = SyncConfig::new(machine_id, None);
    write_config(&base_path.join(CONFIG_FILE), &config)?;

    // Write empty snapshot
    let snapshot = SyncSnapshot::empty(machine_id);
    write_snapshot(&base_path.join(SNAPSHOT_FILE), &snapshot)?;

    // Initial commit
    git.add_all()?;
    git.commit("init: envforge sync repository")?;

    Ok(())
}

/// Initialize sync repo from an existing remote.
/// Returns true if an existing snapshot was found in the remote.
pub fn init_from_remote(
    base_path: &Path,
    remote_url: &str,
    machine_id: &str,
) -> Result<bool, SyncError> {
    if is_initialized(base_path) {
        return Err(SyncError::RepoAlreadyInitialized {
            path: base_path.to_path_buf(),
        });
    }

    // Ensure parent directory exists
    if let Some(parent) = base_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SyncError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // Clone remote
    GitCommandRunner::clone_repo(remote_url, base_path)?;

    let has_existing_snapshot = base_path.join(SNAPSHOT_FILE).is_file();

    // Ensure overrides directory exists
    let overrides_path = base_path.join(OVERRIDES_DIR);
    if !overrides_path.is_dir() {
        std::fs::create_dir_all(&overrides_path).map_err(|e| SyncError::IoError {
            path: overrides_path,
            source: e,
        })?;
    }

    // Update or create config with local machine_id
    let config_path = base_path.join(CONFIG_FILE);
    let config = if config_path.is_file() {
        let mut existing = read_config(&config_path)?;
        existing.sync.machine_id = machine_id.to_string();
        existing
    } else {
        SyncConfig::new(machine_id, Some(remote_url))
    };
    write_config(&config_path, &config)?;

    // Create empty snapshot if none exists
    if !has_existing_snapshot {
        let snapshot = SyncSnapshot::empty(machine_id);
        write_snapshot(&base_path.join(SNAPSHOT_FILE), &snapshot)?;
    }

    // Ensure .gitignore exists
    let gitignore_path = base_path.join(".gitignore");
    if !gitignore_path.is_file() {
        std::fs::write(&gitignore_path, GITIGNORE_CONTENT).map_err(|e| SyncError::IoError {
            path: gitignore_path,
            source: e,
        })?;
    }

    // Commit any changes made during setup
    let git = GitCommandRunner::new(base_path.to_path_buf());
    if git.has_changes()? {
        git.add_all()?;
        git.commit(&format!("init: joined from {}", machine_id))?;
    }

    Ok(has_existing_snapshot)
}

/// Backup existing sync directory before force-reinit.
pub fn backup_existing(base_path: &Path) -> Result<PathBuf, SyncError> {
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let backup_name = format!("sync.backup.{}", timestamp);
    let backup_path = base_path.parent().unwrap_or(base_path).join(backup_name);

    std::fs::rename(base_path, &backup_path).map_err(|e| SyncError::IoError {
        path: base_path.to_path_buf(),
        source: e,
    })?;

    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_hostname_basic() {
        assert_eq!(sanitize_hostname("MacBook-Pro"), "macbook-pro");
    }

    #[test]
    fn test_sanitize_hostname_special_chars() {
        assert_eq!(sanitize_hostname("my.host_name!"), "my-host-name");
    }

    #[test]
    fn test_sanitize_hostname_long() {
        let long = "a".repeat(60);
        let result = sanitize_hostname(&long);
        assert!(result.len() <= 50);
    }

    #[test]
    fn test_sanitize_hostname_leading_trailing_dashes() {
        assert_eq!(sanitize_hostname("--host--"), "host");
    }

    #[test]
    fn test_validate_machine_id_valid() {
        assert!(validate_machine_id("my-laptop-a3f1").is_ok());
        assert!(validate_machine_id("workstation01").is_ok());
    }

    #[test]
    fn test_validate_machine_id_invalid() {
        assert!(validate_machine_id("").is_err());
        assert!(validate_machine_id("My-Laptop").is_err()); // uppercase
        assert!(validate_machine_id("host name").is_err()); // space
        assert!(validate_machine_id("host_name").is_err()); // underscore
    }

    #[test]
    fn test_generate_machine_id_custom() {
        let id = generate_machine_id(Some("work-laptop")).unwrap();
        assert_eq!(id, "work-laptop");
    }

    #[test]
    fn test_generate_machine_id_auto() {
        let id = generate_machine_id(None).unwrap();
        // Should be hostname-XXXX format
        assert!(id.contains('-'));
        // Last 4 chars should be hex
        let suffix = &id[id.rfind('-').unwrap() + 1..];
        assert_eq!(suffix.len(), 4);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sync_snapshot_roundtrip() {
        let snapshot = SyncSnapshot {
            metadata: SnapshotMeta {
                version: 1,
                created_at: "2026-04-15T10:00:00Z".to_string(),
                created_by: "test-machine-a1b2".to_string(),
            },
            entries: vec![
                SyncEntry {
                    key: "DATABASE_URL".to_string(),
                    value: "postgres://localhost:5432/db".to_string(),
                    profile: None,
                    group: None,
                },
                SyncEntry {
                    key: "API_KEY".to_string(),
                    value: "sk-12345".to_string(),
                    profile: Some("dev".to_string()),
                    group: Some("api".to_string()),
                },
            ],
        };

        let toml_str = toml::to_string_pretty(&snapshot).unwrap();
        let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
        assert_eq!(snapshot, deserialized);
    }

    #[test]
    fn test_sync_snapshot_empty_roundtrip() {
        let snapshot = SyncSnapshot::empty("test-a1b2");
        let toml_str = toml::to_string_pretty(&snapshot).unwrap();
        let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
        assert_eq!(snapshot.metadata.version, deserialized.metadata.version);
        assert!(deserialized.entries.is_empty());
    }

    #[test]
    fn test_sync_config_roundtrip() {
        let config = SyncConfig::new("test-machine-a1b2", Some("git@github.com:user/repo.git"));
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: SyncConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_sync_config_defaults() {
        let config = SyncConfig::new("test", None);
        assert!(!config.sync.default_sync);
        assert!(!config.sync.auto_push);
        assert_eq!(config.sync.conflict_strategy, ConflictStrategy::Ask);
        assert!(config.sync.remote_url.is_none());
        assert!(config.manifest.sync_keys.is_empty());
    }

    #[test]
    fn test_sync_entry_special_chars() {
        let entry = SyncEntry {
            key: "SPECIAL".to_string(),
            value: "line1\nline2\twith\"quotes\"".to_string(),
            profile: None,
            group: None,
        };

        let snapshot = SyncSnapshot {
            metadata: SnapshotMeta {
                version: 1,
                created_at: "2026-04-15T10:00:00Z".to_string(),
                created_by: "test".to_string(),
            },
            entries: vec![entry.clone()],
        };

        let toml_str = toml::to_string_pretty(&snapshot).unwrap();
        let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.entries[0].value, entry.value);
    }

    #[test]
    fn test_conflict_strategy_serialization() {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: false,
                auto_push: false,
                conflict_strategy: ConflictStrategy::KeepLocal,
            },
            manifest: ManifestConfig::default(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("keep-local"));

        let deserialized: SyncConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            deserialized.sync.conflict_strategy,
            ConflictStrategy::KeepLocal
        );
    }

    #[test]
    fn test_is_initialized_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_initialized(dir.path()));
    }

    #[test]
    fn test_init_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");

        init_fresh(&sync_path, "test-machine-a1b2").unwrap();

        assert!(is_initialized(&sync_path));
        assert!(sync_path.join(SNAPSHOT_FILE).is_file());
        assert!(sync_path.join(CONFIG_FILE).is_file());
        assert!(sync_path.join(".gitignore").is_file());
        assert!(sync_path.join(OVERRIDES_DIR).is_dir());

        // Verify config content
        let config = read_config(&sync_path.join(CONFIG_FILE)).unwrap();
        assert_eq!(config.sync.machine_id, "test-machine-a1b2");
        assert!(config.sync.remote_url.is_none());
        assert!(!config.sync.default_sync);

        // Verify snapshot content
        let snapshot = read_snapshot(&sync_path.join(SNAPSHOT_FILE)).unwrap();
        assert_eq!(snapshot.metadata.version, 1);
        assert_eq!(snapshot.metadata.created_by, "test-machine-a1b2");
        assert!(snapshot.entries.is_empty());
    }

    #[test]
    fn test_init_fresh_already_initialized() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");

        init_fresh(&sync_path, "test-a1b2").unwrap();

        let result = init_fresh(&sync_path, "test-a1b2");
        assert!(result.is_err());
        match result.unwrap_err() {
            SyncError::RepoAlreadyInitialized { .. } => {}
            other => panic!("expected RepoAlreadyInitialized, got: {:?}", other),
        }
    }

    #[test]
    fn test_backup_existing() {
        let dir = tempfile::tempdir().unwrap();
        let sync_path = dir.path().join("sync");

        init_fresh(&sync_path, "test-a1b2").unwrap();
        assert!(sync_path.exists());

        let backup_path = backup_existing(&sync_path).unwrap();
        assert!(!sync_path.exists());
        assert!(backup_path.exists());
        assert!(backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("sync.backup."));
    }
}
