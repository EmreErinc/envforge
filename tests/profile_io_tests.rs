//! IO coverage for `ops::profile` create/delete + name validation.
//!
//! Serialized because it mutates process-global env: `HOME` (profile files live
//! at `~/.env_managed.<name>`) and `ENVFORGE_CONFIG_DIR` (config save target)
//! are pointed at a tempdir to isolate on-disk state.

use envforge::config::AppConfig;
use envforge::ops::profile::{create_profile, delete_profile, ProfileError};
use serial_test::serial;
use std::path::Path;

fn isolate(dir: &Path) {
    std::env::set_var("HOME", dir);
    std::env::set_var("ENVFORGE_CONFIG_DIR", dir.join("cfg"));
}

fn cleanup() {
    std::env::remove_var("HOME");
    std::env::remove_var("ENVFORGE_CONFIG_DIR");
}

#[test]
#[serial]
fn test_create_profile_then_delete() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    std::fs::create_dir_all(dir.path().join("cfg")).unwrap();

    let mut config = AppConfig::default();
    let path = create_profile(&mut config, "staging").unwrap();
    assert!(path.exists());
    assert!(config.profiles.entries.contains_key("staging"));

    delete_profile(&mut config, "staging", true).unwrap();
    assert!(!config.profiles.entries.contains_key("staging"));
    assert!(!path.exists(), "delete_file=true removes the profile file");

    cleanup();
}

#[test]
#[serial]
fn test_create_profile_duplicate_errors() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    std::fs::create_dir_all(dir.path().join("cfg")).unwrap();

    let mut config = AppConfig::default();
    create_profile(&mut config, "dev").unwrap();
    assert!(matches!(
        create_profile(&mut config, "dev"),
        Err(ProfileError::AlreadyExists(_))
    ));

    cleanup();
}

#[test]
#[serial]
fn test_create_profile_invalid_names() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    std::fs::create_dir_all(dir.path().join("cfg")).unwrap();

    let mut config = AppConfig::default();
    for bad in ["", "-leadingdash", "has space", "bad/slash"] {
        assert!(
            matches!(
                create_profile(&mut config, bad),
                Err(ProfileError::InvalidName(_))
            ),
            "expected InvalidName for {bad:?}"
        );
    }

    cleanup();
}

#[test]
#[serial]
fn test_delete_active_and_missing_profile_errors() {
    let dir = tempfile::tempdir().unwrap();
    isolate(dir.path());
    std::fs::create_dir_all(dir.path().join("cfg")).unwrap();

    let mut config = AppConfig::default(); // active profile = "default"
    assert!(matches!(
        delete_profile(&mut config, "default", false),
        Err(ProfileError::CannotDeleteActive(_))
    ));
    assert!(matches!(
        delete_profile(&mut config, "ghost", false),
        Err(ProfileError::NotFound(_))
    ));

    cleanup();
}
