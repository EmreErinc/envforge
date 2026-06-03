use envforge::ops::sync::*;
use std::path::Path;

// ═══════════════════════════════════════════════════════════════
// SyncSnapshot TOML Round-trip Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_full_roundtrip() {
    let snapshot = SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: "2026-04-15T10:00:00Z".to_string(),
            created_by: "test-machine-a1b2".to_string(),
        },
        entries: vec![
            SyncEntry {
                key: "DATABASE_URL".to_string(),
                value: "postgres://user:pass@localhost:5432/db".to_string(),
                profile: None,
                group: None,
            },
            SyncEntry {
                key: "API_KEY".to_string(),
                value: "sk-12345-abcdef".to_string(),
                profile: Some("dev".to_string()),
                group: Some("api".to_string()),
            },
            SyncEntry {
                key: "EMPTY_VAR".to_string(),
                value: String::new(),
                profile: None,
                group: None,
            },
        ],
    };

    let toml_str = toml::to_string_pretty(&snapshot).unwrap();
    let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
    assert_eq!(snapshot, deserialized);
}

#[test]
fn test_snapshot_unicode_values() {
    let snapshot = SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: "2026-04-15T10:00:00Z".to_string(),
            created_by: "test".to_string(),
        },
        entries: vec![SyncEntry {
            key: "GREETING".to_string(),
            value: "Merhaba dünya! 🌍 こんにちは".to_string(),
            profile: None,
            group: None,
        }],
    };

    let toml_str = toml::to_string_pretty(&snapshot).unwrap();
    let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        deserialized.entries[0].value,
        "Merhaba dünya! 🌍 こんにちは"
    );
}

#[test]
fn test_snapshot_multiline_value() {
    let snapshot = SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: "2026-04-15T10:00:00Z".to_string(),
            created_by: "test".to_string(),
        },
        entries: vec![SyncEntry {
            key: "MULTI".to_string(),
            value: "line1\nline2\nline3".to_string(),
            profile: None,
            group: None,
        }],
    };

    let toml_str = toml::to_string_pretty(&snapshot).unwrap();
    let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
    assert_eq!(deserialized.entries[0].value, "line1\nline2\nline3");
}

#[test]
fn test_snapshot_value_with_quotes() {
    let snapshot = SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: "2026-04-15T10:00:00Z".to_string(),
            created_by: "test".to_string(),
        },
        entries: vec![SyncEntry {
            key: "QUOTED".to_string(),
            value: r#"value with "double" and 'single' quotes"#.to_string(),
            profile: None,
            group: None,
        }],
    };

    let toml_str = toml::to_string_pretty(&snapshot).unwrap();
    let deserialized: SyncSnapshot = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        deserialized.entries[0].value,
        r#"value with "double" and 'single' quotes"#
    );
}

#[test]
fn test_snapshot_optional_fields_omitted_when_none() {
    let entry = SyncEntry {
        key: "KEY".to_string(),
        value: "val".to_string(),
        profile: None,
        group: None,
    };

    let toml_str = toml::to_string_pretty(&entry).unwrap();
    assert!(!toml_str.contains("profile"));
    assert!(!toml_str.contains("group"));
}

#[test]
fn test_snapshot_optional_fields_present_when_set() {
    let entry = SyncEntry {
        key: "KEY".to_string(),
        value: "val".to_string(),
        profile: Some("dev".to_string()),
        group: Some("db".to_string()),
    };

    let toml_str = toml::to_string_pretty(&entry).unwrap();
    assert!(toml_str.contains("profile"));
    assert!(toml_str.contains("group"));
}

// ═══════════════════════════════════════════════════════════════
// SyncConfig Round-trip Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_config_full_roundtrip() {
    let config = SyncConfig {
        sync: SyncSettings {
            machine_id: "macbook-pro-a3f1".to_string(),
            remote_url: Some("git@github.com:user/envforge-sync.git".to_string()),
            default_sync: true,
            auto_push: false,
            conflict_strategy: ConflictStrategy::KeepRemote,
            encrypted: true,
            encryption_policy: envforge::ops::sync::model::SyncEncryptionPolicy::MigrationUntil(
                "2099-01-01T00:00:00Z".into(),
            ),
            verify_signatures: false,
            enforce_ssh: false,
        },
        manifest: ManifestConfig {
            sync_keys: vec!["DB_URL".to_string(), "API_KEY".to_string()],
            local_keys: vec!["LOCAL_SECRET".to_string()],
            patterns: vec![
                GlobPattern {
                    pattern: "AWS_*".to_string(),
                    sync: true,
                },
                GlobPattern {
                    pattern: "LOCAL_*".to_string(),
                    sync: false,
                },
            ],
        },
    };

    let toml_str = toml::to_string_pretty(&config).unwrap();
    let deserialized: SyncConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(config, deserialized);
}

#[test]
fn test_config_conflict_strategy_serialization_variants() {
    for (strategy, expected_str) in [
        (ConflictStrategy::Ask, "ask"),
        (ConflictStrategy::KeepLocal, "keep-local"),
        (ConflictStrategy::KeepRemote, "keep-remote"),
    ] {
        let config = SyncConfig {
            sync: SyncSettings {
                machine_id: "test".to_string(),
                remote_url: None,
                default_sync: false,
                auto_push: false,
                conflict_strategy: strategy.clone(),
                encrypted: true,
                encryption_policy: envforge::ops::sync::model::SyncEncryptionPolicy::MigrationUntil(
                    "2099-01-01T00:00:00Z".into(),
                ),
                verify_signatures: false,
                enforce_ssh: false,
            },
            manifest: ManifestConfig::default(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            toml_str.contains(expected_str),
            "Expected '{}' in TOML for {:?}",
            expected_str,
            strategy
        );

        let deserialized: SyncConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.sync.conflict_strategy, strategy);
    }
}

#[test]
fn test_config_no_remote_url_omitted() {
    let config = SyncConfig::new("test", None);
    let toml_str = toml::to_string_pretty(&config).unwrap();
    assert!(!toml_str.contains("remote_url"));
}

// ═══════════════════════════════════════════════════════════════
// Snapshot / Config File I/O Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_file_write_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.toml");

    let snapshot = SyncSnapshot {
        metadata: SnapshotMeta {
            version: 1,
            created_at: "2026-04-15T10:00:00Z".to_string(),
            created_by: "test-a1b2".to_string(),
        },
        entries: vec![SyncEntry {
            key: "FOO".to_string(),
            value: "bar".to_string(),
            profile: None,
            group: None,
        }],
    };

    write_snapshot(&path, &snapshot).unwrap();
    assert!(path.exists());

    let loaded = read_snapshot(
        &path,
        &envforge::ops::sync::SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
        false,
    )
    .unwrap();
    assert_eq!(snapshot, loaded);
}

#[test]
fn test_config_file_write_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sync-config.toml");

    let config = SyncConfig::new("my-machine-a1b2", Some("git@example.com:repo.git"));
    write_config(&path, &config).unwrap();
    assert!(path.exists());

    let loaded = read_config(&path).unwrap();
    assert_eq!(config, loaded);
}

#[test]
fn test_read_snapshot_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "this is not valid toml [[[").unwrap();

    let result = read_snapshot(
        &path,
        &envforge::ops::sync::SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
        false,
    );
    assert!(result.is_err());
    match result.unwrap_err() {
        SyncError::SnapshotParseError { .. } => {}
        other => panic!("expected SnapshotParseError, got: {:?}", other),
    }
}

#[test]
fn test_read_config_nonexistent_file() {
    let result = read_config(Path::new("/nonexistent/path/config.toml"));
    assert!(result.is_err());
    match result.unwrap_err() {
        SyncError::IoError { .. } => {}
        other => panic!("expected IoError, got: {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════
// Machine ID Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_machine_id_auto_format() {
    let id = generate_machine_id(None).unwrap();
    let parts: Vec<&str> = id.rsplitn(2, '-').collect();
    assert_eq!(parts.len(), 2);
    // Last segment is 4 hex chars
    assert_eq!(parts[0].len(), 4);
    assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_machine_id_custom_valid() {
    assert!(generate_machine_id(Some("work-laptop")).is_ok());
    assert!(generate_machine_id(Some("server01")).is_ok());
    assert!(generate_machine_id(Some("a")).is_ok());
}

#[test]
fn test_machine_id_custom_invalid() {
    assert!(generate_machine_id(Some("")).is_err());
    assert!(generate_machine_id(Some("Work-Laptop")).is_err());
    assert!(generate_machine_id(Some("has space")).is_err());
    assert!(generate_machine_id(Some("under_score")).is_err());
    assert!(generate_machine_id(Some("special!char")).is_err());
}

// ═══════════════════════════════════════════════════════════════
// Git Version Parsing Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_git_version_display() {
    let v = GitVersion {
        major: 2,
        minor: 39,
        patch: 1,
    };
    assert_eq!(v.to_string(), "2.39.1");
}

#[test]
fn test_git_version_minimum_constant() {
    let min = GitVersion::MINIMUM;
    assert_eq!(min.major, 2);
    assert_eq!(min.minor, 28);
    assert_eq!(min.patch, 0);
}

// ═══════════════════════════════════════════════════════════════
// Repo Init Integration Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_init_fresh_creates_complete_structure() {
    let dir = tempfile::tempdir().unwrap();
    let sync_path = dir.path().join("sync");

    init_fresh(&sync_path, "test-machine-a1b2").unwrap();

    // Verify all expected files/dirs
    assert!(sync_path.join(".git").is_dir());
    assert!(sync_path.join("snapshot.toml").is_file());
    assert!(sync_path.join("sync-config.toml").is_file());
    assert!(sync_path.join(".gitignore").is_file());
    assert!(sync_path.join("overrides").is_dir());

    // Verify gitignore contents
    let gitignore = std::fs::read_to_string(sync_path.join(".gitignore")).unwrap();
    assert!(gitignore.contains("*.backup"));
    assert!(gitignore.contains(".DS_Store"));
    assert!(gitignore.contains("sync-log.toml"));

    // Verify config
    let config = read_config(&sync_path.join("sync-config.toml")).unwrap();
    assert_eq!(config.sync.machine_id, "test-machine-a1b2");
    assert!(!config.sync.default_sync);
    assert_eq!(config.sync.conflict_strategy, ConflictStrategy::Ask);

    // Verify snapshot
    let snapshot = read_snapshot(
        &sync_path.join("snapshot.toml"),
        &envforge::ops::sync::SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
        false,
    )
    .unwrap();
    assert_eq!(snapshot.metadata.version, 1);
    assert!(snapshot.entries.is_empty());
}

#[test]
fn test_init_fresh_double_init_fails() {
    let dir = tempfile::tempdir().unwrap();
    let sync_path = dir.path().join("sync");

    init_fresh(&sync_path, "test-a1b2").unwrap();

    let result = init_fresh(&sync_path, "test-a1b2");
    assert!(result.is_err());
    match result.unwrap_err() {
        SyncError::RepoAlreadyInitialized { path } => {
            assert_eq!(path, sync_path);
        }
        other => panic!("expected RepoAlreadyInitialized, got: {:?}", other),
    }
}

#[test]
fn test_init_fresh_has_git_commit() {
    let dir = tempfile::tempdir().unwrap();
    let sync_path = dir.path().join("sync");

    init_fresh(&sync_path, "test-a1b2").unwrap();

    // Verify there's an initial commit
    let git = GitCommandRunner::new(sync_path);
    let log = git.log(10).unwrap();
    assert_eq!(log.len(), 1);
    assert!(log[0].message.contains("init"));
}

#[test]
fn test_is_initialized_checks_both_git_and_config() {
    let dir = tempfile::tempdir().unwrap();
    let sync_path = dir.path().join("sync");

    // Empty dir — not initialized
    std::fs::create_dir_all(&sync_path).unwrap();
    assert!(!is_initialized(&sync_path));

    // Only .git — not initialized (missing config)
    std::fs::create_dir_all(sync_path.join(".git")).unwrap();
    assert!(!is_initialized(&sync_path));

    // Full init — initialized
    std::fs::remove_dir_all(&sync_path).unwrap();
    init_fresh(&sync_path, "test-a1b2").unwrap();
    assert!(is_initialized(&sync_path));
}

#[test]
fn test_backup_existing_preserves_content() {
    let dir = tempfile::tempdir().unwrap();
    let sync_path = dir.path().join("sync");

    init_fresh(&sync_path, "test-a1b2").unwrap();

    // Write a marker file
    std::fs::write(sync_path.join("marker.txt"), "hello").unwrap();

    let backup_path = backup_existing(&sync_path).unwrap();

    // Original gone
    assert!(!sync_path.exists());

    // Backup has our content
    assert!(backup_path.join("marker.txt").is_file());
    assert_eq!(
        std::fs::read_to_string(backup_path.join("marker.txt")).unwrap(),
        "hello"
    );
}

#[test]
fn test_snapshot_empty_factory() {
    let snapshot = SyncSnapshot::empty("my-machine");
    assert_eq!(snapshot.metadata.version, 1);
    assert_eq!(snapshot.metadata.created_by, "my-machine");
    assert!(snapshot.entries.is_empty());
    // created_at should be a valid timestamp
    assert!(snapshot.metadata.created_at.contains('T'));
}

#[test]
fn test_config_new_factory_without_remote() {
    let config = SyncConfig::new("machine-01", None);
    assert_eq!(config.sync.machine_id, "machine-01");
    assert!(config.sync.remote_url.is_none());
    assert!(!config.sync.default_sync);
    assert!(!config.sync.auto_push);
    assert_eq!(config.sync.conflict_strategy, ConflictStrategy::Ask);
    assert!(config.manifest.sync_keys.is_empty());
}

#[test]
fn test_config_new_factory_with_remote() {
    let config = SyncConfig::new("machine-01", Some("git@github.com:user/repo.git"));
    assert_eq!(
        config.sync.remote_url,
        Some("git@github.com:user/repo.git".to_string())
    );
}

// ═══════════════════════════════════════════════════════════════
// Git Wrapper Integration Tests (real git)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_git_check_available() {
    let dir = tempfile::tempdir().unwrap();
    let git = GitCommandRunner::new(dir.path().to_path_buf());
    let version = git.check_available().unwrap();
    assert!(version.meets_minimum());
}

#[test]
fn test_git_init_and_status() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path.clone());

    git.init("main").unwrap();
    assert!(repo_path.join(".git").is_dir());

    let status = git.status().unwrap();
    assert!(status.is_empty()); // Empty repo, no files
}

#[test]
fn test_git_add_commit_log() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path.clone());

    git.init("main").unwrap();
    git.ensure_user_config().unwrap();

    // Create a file
    std::fs::write(repo_path.join("test.txt"), "hello world").unwrap();

    git.add(&["test.txt"]).unwrap();
    git.commit("initial commit").unwrap();

    let log = git.log(10).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].message, "initial commit");
}

#[test]
fn test_git_has_changes() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path.clone());

    git.init("main").unwrap();
    git.ensure_user_config().unwrap();

    // No changes initially
    // (empty repo with no files)

    // Create a file
    std::fs::write(repo_path.join("test.txt"), "content").unwrap();
    assert!(git.has_changes().unwrap());

    // After commit
    git.add_all().unwrap();
    git.commit("add file").unwrap();
    assert!(!git.has_changes().unwrap());
}

#[test]
fn test_git_show_file_at_commit() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path.clone());

    git.init("main").unwrap();
    git.ensure_user_config().unwrap();

    std::fs::write(repo_path.join("data.txt"), "version 1").unwrap();
    git.add_all().unwrap();
    git.commit("v1").unwrap();

    let log = git.log(1).unwrap();
    let first_hash = &log[0].hash;

    std::fs::write(repo_path.join("data.txt"), "version 2").unwrap();
    git.add_all().unwrap();
    git.commit("v2").unwrap();

    // Show file at first commit
    let content = git.show(first_hash, "data.txt").unwrap();
    assert_eq!(content.trim(), "version 1");
}

#[test]
fn test_git_remote_url_none_when_no_remote() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path);

    git.init("main").unwrap();

    let url = git.remote_url().unwrap();
    assert!(url.is_none());
}

#[test]
fn test_git_push_no_remote() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    let git = GitCommandRunner::new(repo_path);

    git.init("main").unwrap();
    git.ensure_user_config().unwrap();
    std::fs::write(dir.path().join("repo/test.txt"), "content").unwrap();
    git.add_all().unwrap();
    git.commit("test").unwrap();

    let result = git.push().unwrap();
    assert_eq!(result, PushResult::NoRemote);
}
