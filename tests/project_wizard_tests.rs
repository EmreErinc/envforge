use envforge::ops::project::*;
use std::path::Path;

// ─── parse_dotenv_simple Tests ─────────────────────────────

#[test]
fn test_parse_dotenv_simple_basic() {
    let temp = std::env::temp_dir().join("envforge-test-dotenv-simple");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env");
    std::fs::write(&path, "DB_HOST=localhost\nDB_PORT=5432\n").unwrap();

    let result = parse_dotenv_simple(&path).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result["DB_HOST"], "localhost");
    assert_eq!(result["DB_PORT"], "5432");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_dotenv_simple_with_quotes() {
    let temp = std::env::temp_dir().join("envforge-test-dotenv-quotes");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env");
    std::fs::write(&path, "KEY1=\"quoted value\"\nKEY2='single'\n").unwrap();

    let result = parse_dotenv_simple(&path).unwrap();
    assert_eq!(result["KEY1"], "quoted value");
    assert_eq!(result["KEY2"], "single");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_dotenv_simple_with_export() {
    let temp = std::env::temp_dir().join("envforge-test-dotenv-export");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env");
    std::fs::write(&path, "export KEY1=val1\nKEY2=val2\n").unwrap();

    let result = parse_dotenv_simple(&path).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result["KEY1"], "val1");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_dotenv_simple_skips_comments_and_blanks() {
    let temp = std::env::temp_dir().join("envforge-test-dotenv-comments");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env");
    std::fs::write(&path, "# comment\n\nKEY=val\n  # indented comment\n").unwrap();

    let result = parse_dotenv_simple(&path).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result["KEY"], "val");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_dotenv_simple_empty_file() {
    let temp = std::env::temp_dir().join("envforge-test-dotenv-empty");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env");
    std::fs::write(&path, "").unwrap();

    let result = parse_dotenv_simple(&path).unwrap();
    assert!(result.is_empty());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_dotenv_simple_nonexistent() {
    let result = parse_dotenv_simple(Path::new("/nonexistent/.env"));
    assert!(result.is_err());
}

// ─── parse_schema_keys Tests ───────────────────────────────

#[test]
fn test_parse_schema_keys_basic() {
    let temp = std::env::temp_dir().join("envforge-test-schema-keys");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env.schema");
    std::fs::write(
        &path,
        r#"[DB_HOST]
type = "string"
required = true

[DB_PORT]
type = "port"
required = true

[API_KEY]
type = "string"
sensitive = true
"#,
    )
    .unwrap();

    let keys = parse_schema_keys(&path).unwrap();
    assert_eq!(keys.len(), 3);
    assert_eq!(
        keys[0],
        ("DB_HOST".to_string(), "string".to_string(), false)
    );
    assert_eq!(keys[1], ("DB_PORT".to_string(), "port".to_string(), false));
    assert_eq!(keys[2], ("API_KEY".to_string(), "string".to_string(), true));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_schema_keys_empty() {
    let temp = std::env::temp_dir().join("envforge-test-schema-empty");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env.schema");
    std::fs::write(&path, "# empty schema\n").unwrap();

    let keys = parse_schema_keys(&path).unwrap();
    assert!(keys.is_empty());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_schema_keys_ignores_env_overrides() {
    let temp = std::env::temp_dir().join("envforge-test-schema-override");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let path = temp.join(".env.schema");
    std::fs::write(
        &path,
        r#"[DB_HOST]
type = "string"

[DB_HOST.production]
pattern = "^prod-"
"#,
    )
    .unwrap();

    let keys = parse_schema_keys(&path).unwrap();
    // Should only have DB_HOST, not DB_HOST.production
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].0, "DB_HOST");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_parse_schema_keys_nonexistent() {
    let result = parse_schema_keys(Path::new("/nonexistent/.env.schema"));
    assert!(result.is_err());
}

// ─── Wizard State Tests ────────────────────────────────────

#[test]
fn test_wizard_state_tracks_steps() {
    let temp = std::env::temp_dir().join("envforge-test-wizard-state");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // Init project
    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "test-wizard".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };
    init_project(&opts).unwrap();

    // Load and check wizard state
    let detected = detect_project_config(&temp).unwrap();
    let config = load_project_config(&detected).unwrap();
    assert!(config.wizard.completed_steps.contains(&"init".to_string()));

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Schema Step Integration Tests ─────────────────────────

#[test]
fn test_schema_step_generates_from_env() {
    let temp = std::env::temp_dir().join("envforge-test-schema-gen");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // Create project
    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "schema-test".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };
    init_project(&opts).unwrap();

    // Write some env vars
    std::fs::write(
        temp.join(".env.development"),
        "DB_HOST=localhost\nDB_PORT=5432\nDEBUG=true\n",
    )
    .unwrap();

    // Run schema step
    let detected = detect_project_config(&temp).unwrap();
    let config = load_project_config(&detected).unwrap();
    let key_count = run_schema_step(&temp, &config).unwrap();
    assert_eq!(key_count, 3);

    // Verify schema file exists
    assert!(temp.join(".env.schema").exists());
    let schema_content = std::fs::read_to_string(temp.join(".env.schema")).unwrap();
    assert!(schema_content.contains("[DB_HOST]"));
    assert!(schema_content.contains("[DB_PORT]"));
    assert!(schema_content.contains("[DEBUG]"));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_schema_step_empty_env() {
    let temp = std::env::temp_dir().join("envforge-test-schema-empty-env");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "empty-test".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };
    init_project(&opts).unwrap();

    // .env.development is auto-created with just a comment
    let detected = detect_project_config(&temp).unwrap();
    let config = load_project_config(&detected).unwrap();
    let key_count = run_schema_step(&temp, &config).unwrap();
    assert_eq!(key_count, 0);

    let _ = std::fs::remove_dir_all(&temp);
}
