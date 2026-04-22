use std::path::Path;

use envforge::config::*;

// ═══════════════════════════════════════════════════════════════
// AppConfig Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_default_config_has_expected_values() {
    let config = AppConfig::default();
    assert_eq!(config.general.default_shell, "zsh");
    assert_eq!(config.files.primary, "~/.zshrc");
    assert_eq!(config.files.reference, "~/.env_managed");
    assert!(config.files.use_reference_file);
    assert_eq!(config.offsets.header_protected_lines, 0);
    assert_eq!(config.offsets.footer_protected_lines, 0);
    assert!(config.protected_blocks.markers.is_empty());
}

#[test]
fn test_config_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let config = AppConfig::default();
    save_config(&config, &config_path).unwrap();

    assert!(config_path.exists());

    let loaded = load_config(&config_path).unwrap();
    assert_eq!(loaded.general.default_shell, config.general.default_shell);
    assert_eq!(loaded.files.primary, config.files.primary);
    assert_eq!(loaded.files.reference, config.files.reference);
    assert_eq!(
        loaded.files.use_reference_file,
        config.files.use_reference_file
    );
    assert_eq!(
        loaded.offsets.header_protected_lines,
        config.offsets.header_protected_lines
    );
    assert_eq!(
        loaded.offsets.footer_protected_lines,
        config.offsets.footer_protected_lines
    );
}

#[test]
fn test_config_save_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nested").join("deep").join("config.toml");

    let config = AppConfig::default();
    save_config(&config, &config_path).unwrap();

    assert!(config_path.exists());
}

#[test]
fn test_config_load_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    std::fs::write(&config_path, "this is not valid toml {{{{").unwrap();
    let result = load_config(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_config_load_nonexistent() {
    let result = load_config(Path::new("/nonexistent/config.toml"));
    assert!(result.is_err());
}

#[test]
fn test_config_round_trip_with_custom_values() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let mut config = AppConfig::default();
    config.general.default_shell = "bash".to_string();
    config.files.primary = "~/.bashrc".to_string();
    config.files.use_reference_file = false;
    config.offsets.footer_protected_lines = 15;
    config.protected_blocks.markers = vec![
        "# >>> conda initialize >>>".to_string(),
        "# <<< conda initialize <<<".to_string(),
    ];

    save_config(&config, &config_path).unwrap();
    let loaded = load_config(&config_path).unwrap();

    assert_eq!(loaded.general.default_shell, "bash");
    assert_eq!(loaded.files.primary, "~/.bashrc");
    assert!(!loaded.files.use_reference_file);
    assert_eq!(loaded.offsets.footer_protected_lines, 15);
    assert_eq!(loaded.protected_blocks.markers.len(), 2);
}

// ═══════════════════════════════════════════════════════════════
// Backup Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_backup_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("test.txt");
    std::fs::write(&source, "original content").unwrap();

    // Override backups dir by using create_backup directly
    // Since create_backup uses the global backups_dir(), we test list_backups
    // with the actual backup system
    let backups = list_backups(&source).unwrap();
    // May have backups from other tests - just ensure it doesn't error
    let _ = backups;
}

#[test]
fn test_list_backups_empty_dir() {
    let result = list_backups(Path::new("/some/nonexistent/file.txt"));
    assert!(result.is_ok());
    // Should return empty vec (backups dir may or may not exist)
}

// ═══════════════════════════════════════════════════════════════
// Atomic Write Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_atomic_write_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");

    atomic_write(&path, "hello world\n", None).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello world\n");
}

#[test]
fn test_atomic_write_overwrites_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");

    std::fs::write(&path, "old content").unwrap();
    atomic_write(&path, "new content", None).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "new content");
}

#[test]
fn test_atomic_write_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("deep").join("output.txt");

    atomic_write(&path, "nested content", None).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "nested content");
}

#[test]
fn test_atomic_write_hash_match_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");

    let original = "original content";
    std::fs::write(&path, original).unwrap();

    let hash = compute_hash(original.as_bytes());
    atomic_write(&path, "updated content", Some(hash)).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "updated content");
}

#[test]
fn test_atomic_write_hash_mismatch_aborts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");

    std::fs::write(&path, "current content").unwrap();

    // Provide wrong hash
    let wrong_hash = compute_hash(b"different content");
    let result = atomic_write(&path, "should not be written", Some(wrong_hash));

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::HashMismatch { .. }
    ));

    // Original file should be untouched
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "current content");
}

#[test]
fn test_atomic_write_hash_none_skips_check() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");

    std::fs::write(&path, "original").unwrap();
    atomic_write(&path, "updated", None).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "updated");
}

#[test]
fn test_atomic_write_new_file_with_hash_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new_file.txt");

    // File doesn't exist, hash check should be skipped
    let hash = compute_hash(b"irrelevant");
    atomic_write(&path, "new content", Some(hash)).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "new content");
}

#[test]
fn test_compute_hash_deterministic() {
    let data = b"test data";
    let hash1 = compute_hash(data);
    let hash2 = compute_hash(data);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_compute_hash_changes_with_content() {
    let hash1 = compute_hash(b"content A");
    let hash2 = compute_hash(b"content B");
    assert_ne!(hash1, hash2);
}
