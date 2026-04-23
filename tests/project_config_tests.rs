use envforge::ops::project::*;
use std::path::Path;

// ─── ConfigFormat Tests ────────────────────────────────────

#[test]
fn test_config_format_parse_toml() {
    assert_eq!(ConfigFormat::parse("toml").unwrap(), ConfigFormat::Toml);
    assert_eq!(ConfigFormat::parse("TOML").unwrap(), ConfigFormat::Toml);
}

#[test]
fn test_config_format_parse_yaml() {
    assert_eq!(ConfigFormat::parse("yaml").unwrap(), ConfigFormat::Yaml);
    assert_eq!(ConfigFormat::parse("yml").unwrap(), ConfigFormat::Yaml);
    assert_eq!(ConfigFormat::parse("YAML").unwrap(), ConfigFormat::Yaml);
}

#[test]
fn test_config_format_parse_json() {
    assert_eq!(ConfigFormat::parse("json").unwrap(), ConfigFormat::Json);
    assert_eq!(ConfigFormat::parse("JSON").unwrap(), ConfigFormat::Json);
}

#[test]
fn test_config_format_parse_unknown() {
    let result = ConfigFormat::parse("xml");
    assert!(result.is_err());
}

#[test]
fn test_config_format_default_filename() {
    assert_eq!(
        ConfigFormat::Toml.default_filename(),
        ".envforge.project.toml"
    );
    assert_eq!(
        ConfigFormat::Yaml.default_filename(),
        ".envforge.project.yaml"
    );
    assert_eq!(
        ConfigFormat::Json.default_filename(),
        ".envforge.project.json"
    );
}

// ─── Config Serialization Roundtrip Tests ──────────────────

fn sample_config() -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            name: "test-app".to_string(),
            schema_path: ".env.schema".into(),
            active_environment: "development".to_string(),
        },
        wizard: WizardState {
            completed_steps: vec!["init".to_string()],
        },
        environments: vec![
            ProjectEnvironment {
                name: "development".to_string(),
                env_file: ".env.development".into(),
                description: Some("Dev environment".to_string()),
            },
            ProjectEnvironment {
                name: "production".to_string(),
                env_file: ".env.production".into(),
                description: None,
            },
        ],
    }
}

#[test]
fn test_toml_roundtrip() {
    let config = sample_config();
    let serialized = serialize_project_config(&config, ConfigFormat::Toml).unwrap();
    let parsed = parse_project_config(&serialized, ConfigFormat::Toml, Path::new("test")).unwrap();
    assert_eq!(parsed.project.name, "test-app");
    assert_eq!(parsed.project.active_environment, "development");
    assert_eq!(parsed.environments.len(), 2);
    assert_eq!(parsed.environments[0].name, "development");
    assert_eq!(parsed.environments[1].name, "production");
    assert_eq!(parsed.wizard.completed_steps, vec!["init"]);
}

#[test]
fn test_yaml_roundtrip() {
    let config = sample_config();
    let serialized = serialize_project_config(&config, ConfigFormat::Yaml).unwrap();
    let parsed = parse_project_config(&serialized, ConfigFormat::Yaml, Path::new("test")).unwrap();
    assert_eq!(parsed.project.name, "test-app");
    assert_eq!(parsed.environments.len(), 2);
    assert_eq!(parsed.project.active_environment, "development");
}

#[test]
fn test_json_roundtrip() {
    let config = sample_config();
    let serialized = serialize_project_config(&config, ConfigFormat::Json).unwrap();
    let parsed = parse_project_config(&serialized, ConfigFormat::Json, Path::new("test")).unwrap();
    assert_eq!(parsed.project.name, "test-app");
    assert_eq!(parsed.environments.len(), 2);
    assert_eq!(parsed.project.active_environment, "development");
}

#[test]
fn test_parse_invalid_toml() {
    let result = parse_project_config("not valid toml {{", ConfigFormat::Toml, Path::new("bad"));
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_yaml() {
    let result = parse_project_config(":\n  - :\n  bad", ConfigFormat::Yaml, Path::new("bad"));
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_json() {
    let result = parse_project_config("{bad json", ConfigFormat::Json, Path::new("bad"));
    assert!(result.is_err());
}

// ─── Env Name Validation Tests ─────────────────────────────

#[test]
fn test_validate_env_name_valid() {
    assert!(validate_env_name("development").is_ok());
    assert!(validate_env_name("staging").is_ok());
    assert!(validate_env_name("prod-v2").is_ok());
    assert!(validate_env_name("test1").is_ok());
}

#[test]
fn test_validate_env_name_empty() {
    assert!(validate_env_name("").is_err());
}

#[test]
fn test_validate_env_name_uppercase() {
    assert!(validate_env_name("PRODUCTION").is_err());
}

#[test]
fn test_validate_env_name_spaces() {
    assert!(validate_env_name("my env").is_err());
}

#[test]
fn test_validate_env_name_starts_with_hyphen() {
    assert!(validate_env_name("-dev").is_err());
}

#[test]
fn test_validate_env_name_ends_with_hyphen() {
    assert!(validate_env_name("dev-").is_err());
}

#[test]
fn test_validate_env_name_special_chars() {
    assert!(validate_env_name("dev_test").is_err());
    assert!(validate_env_name("dev.test").is_err());
    assert!(validate_env_name("dev@test").is_err());
}

// ─── Active Env Path Tests ─────────────────────────────────

#[test]
fn test_active_env_path_resolves() {
    let config = sample_config();
    let root = Path::new("/project");
    let path = active_env_path(&config, root).unwrap();
    assert_eq!(path, Path::new("/project/.env.development"));
}

#[test]
fn test_active_env_path_nonexistent_env() {
    let mut config = sample_config();
    config.project.active_environment = "nonexistent".to_string();
    let result = active_env_path(&config, Path::new("/project"));
    assert!(result.is_err());
}

// ─── Find Environment Tests ────────────────────────────────

#[test]
fn test_find_environment_exists() {
    let config = sample_config();
    let env = find_environment(&config, "development").unwrap();
    assert_eq!(env.name, "development");
}

#[test]
fn test_find_environment_not_found() {
    let config = sample_config();
    let result = find_environment(&config, "nonexistent");
    assert!(result.is_err());
}

// ─── Detection Tests ───────────────────────────────────────

#[test]
fn test_detect_project_config_in_cwd() {
    let temp = std::env::temp_dir().join("envforge-test-detect-cwd");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(
        temp.join(".envforge.project.toml"),
        "[project]\nname = \"t\"\nschema_path = \".env.schema\"\nactive_environment = \"dev\"\n\n[[environments]]\nname = \"dev\"\nenv_file = \".env.dev\"\n",
    )
    .unwrap();

    let detected = detect_project_config(&temp);
    assert!(detected.is_some());
    let d = detected.unwrap();
    assert_eq!(d.format, ConfigFormat::Toml);
    assert_eq!(d.project_root, temp);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_detect_project_config_in_parent() {
    let temp = std::env::temp_dir().join("envforge-test-detect-parent");
    let _ = std::fs::remove_dir_all(&temp);
    let child = temp.join("src/components");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::write(
        temp.join(".envforge.project.yaml"),
        "project:\n  name: t\n  schema_path: .env.schema\n  active_environment: dev\nenvironments:\n  - name: dev\n    env_file: .env.dev\n",
    )
    .unwrap();

    let detected = detect_project_config(&child);
    assert!(detected.is_some());
    let d = detected.unwrap();
    assert_eq!(d.format, ConfigFormat::Yaml);
    assert_eq!(d.project_root, temp);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_detect_project_config_json() {
    let temp = std::env::temp_dir().join("envforge-test-detect-json");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(
        temp.join(".envforge.project.json"),
        r#"{"project":{"name":"t","schema_path":".env.schema","active_environment":"dev"},"environments":[{"name":"dev","env_file":".env.dev"}]}"#,
    )
    .unwrap();

    let detected = detect_project_config(&temp);
    assert!(detected.is_some());
    assert_eq!(detected.unwrap().format, ConfigFormat::Json);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_detect_project_config_none() {
    let temp = std::env::temp_dir().join("envforge-test-detect-none");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let detected = detect_project_config(&temp);
    assert!(detected.is_none());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_detect_project_config_toml_priority_over_yaml() {
    let temp = std::env::temp_dir().join("envforge-test-detect-priority");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // TOML is checked first in CONFIG_FILENAMES
    let toml_content = "[project]\nname = \"t\"\nschema_path = \".env.schema\"\nactive_environment = \"dev\"\n\n[[environments]]\nname = \"dev\"\nenv_file = \".env.dev\"\n";
    std::fs::write(temp.join(".envforge.project.toml"), toml_content).unwrap();
    std::fs::write(
        temp.join(".envforge.project.yaml"),
        "project:\n  name: t\n  schema_path: .env.schema\n  active_environment: dev\nenvironments:\n  - name: dev\n    env_file: .env.dev\n",
    )
    .unwrap();

    let detected = detect_project_config(&temp).unwrap();
    assert_eq!(detected.format, ConfigFormat::Toml);

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Save / Load Tests ─────────────────────────────────────

#[test]
fn test_save_and_load_project_config() {
    let temp = std::env::temp_dir().join("envforge-test-save-load");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let config = sample_config();
    let path = temp.join(".envforge.project.toml");

    save_project_config(&config, &path, ConfigFormat::Toml).unwrap();
    assert!(path.exists());

    let detected = DetectedConfig {
        config_path: path,
        project_root: temp.clone(),
        format: ConfigFormat::Toml,
    };
    let loaded = load_project_config(&detected).unwrap();
    assert_eq!(loaded.project.name, "test-app");
    assert_eq!(loaded.environments.len(), 2);

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Init Tests ────────────────────────────────────────────

#[test]
fn test_init_project_creates_files() {
    let temp = std::env::temp_dir().join("envforge-test-init");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "my-app".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    let result = init_project(&opts).unwrap();
    assert!(result.config_path.exists());
    assert!(result.env_file_path.exists());
    assert_eq!(result.project_name, "my-app");
    assert_eq!(result.environment_name, "development");

    // Verify config content
    let detected = detect_project_config(&temp).unwrap();
    let config = load_project_config(&detected).unwrap();
    assert_eq!(config.project.name, "my-app");
    assert_eq!(config.environments.len(), 1);
    assert_eq!(config.environments[0].name, "development");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_init_project_rejects_double_init() {
    let temp = std::env::temp_dir().join("envforge-test-double-init");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "my-app".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    init_project(&opts).unwrap();

    // Second init should fail
    let result = init_project(&opts);
    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_init_project_force_reinit() {
    let temp = std::env::temp_dir().join("envforge-test-force-init");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "my-app".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    init_project(&opts).unwrap();

    // Force reinit should succeed
    let force_opts = InitOptions {
        force: true,
        ..InitOptions {
            root: temp.clone(),
            format: ConfigFormat::Toml,
            project_name: "renamed-app".to_string(),
            default_env_name: "development".to_string(),
            env_file_path: ".env.development".into(),
            schema_path: ".env.schema".into(),
            force: true,
        }
    };
    let result = init_project(&force_opts).unwrap();
    assert_eq!(result.project_name, "renamed-app");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_init_project_yaml_format() {
    let temp = std::env::temp_dir().join("envforge-test-init-yaml");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Yaml,
        project_name: "yaml-app".to_string(),
        default_env_name: "dev".to_string(),
        env_file_path: ".env.dev".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    let result = init_project(&opts).unwrap();
    assert!(result.config_path.to_string_lossy().ends_with(".yaml"));

    let detected = detect_project_config(&temp).unwrap();
    assert_eq!(detected.format, ConfigFormat::Yaml);
    let config = load_project_config(&detected).unwrap();
    assert_eq!(config.project.name, "yaml-app");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_init_project_json_format() {
    let temp = std::env::temp_dir().join("envforge-test-init-json");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Json,
        project_name: "json-app".to_string(),
        default_env_name: "dev".to_string(),
        env_file_path: ".env.dev".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    let result = init_project(&opts).unwrap();
    assert!(result.config_path.to_string_lossy().ends_with(".json"));

    let detected = detect_project_config(&temp).unwrap();
    assert_eq!(detected.format, ConfigFormat::Json);

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Import Existing Env Tests ─────────────────────────────

#[test]
fn test_import_existing_env() {
    let temp = std::env::temp_dir().join("envforge-test-import");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let source = temp.join(".env");
    std::fs::write(
        &source,
        "DB_HOST=localhost\nDB_PORT=5432\n# comment\n\nAPI_KEY=secret\n",
    )
    .unwrap();

    let target = temp.join(".env.development");
    let count = import_existing_env(&source, &target).unwrap();
    assert_eq!(count, 3);
    assert!(target.exists());

    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("DB_HOST=localhost"));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_import_existing_env_empty_file() {
    let temp = std::env::temp_dir().join("envforge-test-import-empty");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let source = temp.join(".env");
    std::fs::write(&source, "# only comments\n\n").unwrap();

    let target = temp.join(".env.development");
    let count = import_existing_env(&source, &target).unwrap();
    assert_eq!(count, 0);

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Gitignore Tests ───────────────────────────────────────

#[test]
fn test_add_to_gitignore_creates_new() {
    let temp = std::env::temp_dir().join("envforge-test-gitignore-new");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let added = add_to_gitignore(&temp).unwrap();
    assert!(added);

    let content = std::fs::read_to_string(temp.join(".gitignore")).unwrap();
    assert!(content.contains(".env.*"));
    assert!(content.contains("!.env.schema"));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_add_to_gitignore_appends_existing() {
    let temp = std::env::temp_dir().join("envforge-test-gitignore-append");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(temp.join(".gitignore"), "node_modules/\ntarget/\n").unwrap();

    let added = add_to_gitignore(&temp).unwrap();
    assert!(added);

    let content = std::fs::read_to_string(temp.join(".gitignore")).unwrap();
    assert!(content.contains("node_modules/"));
    assert!(content.contains(".env.*"));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_add_to_gitignore_idempotent() {
    let temp = std::env::temp_dir().join("envforge-test-gitignore-idem");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(temp.join(".gitignore"), ".env.*\n").unwrap();

    let added = add_to_gitignore(&temp).unwrap();
    assert!(!added);

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Derive Project Name Tests ─────────────────────────────

#[test]
fn test_derive_project_name() {
    assert_eq!(
        derive_project_name(Path::new("/home/user/my-app")),
        "my-app"
    );
    assert_eq!(
        derive_project_name(Path::new("/projects/envforge")),
        "envforge"
    );
}

// ─── Error Display Tests ───────────────────────────────────

#[test]
fn test_error_config_not_found() {
    let err = ProjectError::ConfigNotFound;
    let msg = err.to_string();
    assert!(msg.contains("project init"));
}

#[test]
fn test_error_env_not_found() {
    let err = ProjectError::EnvironmentNotFound {
        name: "staging".to_string(),
        available: "development, production".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("staging"));
    assert!(msg.contains("development, production"));
}

#[test]
fn test_error_env_exists() {
    let err = ProjectError::EnvironmentExists {
        name: "dev".to_string(),
    };
    assert!(err.to_string().contains("dev"));
}

#[test]
fn test_error_invalid_env_name() {
    let err = ProjectError::InvalidEnvironmentName {
        name: "BAD".to_string(),
    };
    assert!(err.to_string().contains("BAD"));
}

#[test]
fn test_error_already_initialized() {
    let err = ProjectError::AlreadyInitialized {
        path: "/project/.envforge.project.toml".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("already initialized"));
    assert!(msg.contains("--force"));
}

// ─── Config with Optional Description Tests ────────────────

#[test]
fn test_config_env_without_description() {
    let config = ProjectConfig {
        project: ProjectMeta {
            name: "t".to_string(),
            schema_path: ".env.schema".into(),
            active_environment: "dev".to_string(),
        },
        wizard: WizardState::default(),
        environments: vec![ProjectEnvironment {
            name: "dev".to_string(),
            env_file: ".env.dev".into(),
            description: None,
        }],
    };

    // Serialize and parse back — None description should not appear
    let serialized = serialize_project_config(&config, ConfigFormat::Toml).unwrap();
    assert!(!serialized.contains("description"));

    let parsed = parse_project_config(&serialized, ConfigFormat::Toml, Path::new("t")).unwrap();
    assert!(parsed.environments[0].description.is_none());
}

#[test]
fn test_wizard_state_default_empty() {
    let state = WizardState::default();
    assert!(state.completed_steps.is_empty());
}

// ─── Additional Edge-Case Tests ───────────────────────────

#[test]
fn test_validate_env_name_single_char() {
    assert!(validate_env_name("a").is_ok());
    assert!(validate_env_name("1").is_ok());
}

#[test]
fn test_validate_env_name_consecutive_hyphens() {
    // Consecutive hyphens are technically valid (lowercase + hyphens rule)
    assert!(validate_env_name("a--b").is_ok());
}

#[test]
fn test_validate_env_name_digits_only() {
    assert!(validate_env_name("123").is_ok());
}

#[test]
fn test_validate_env_name_underscore_rejected() {
    assert!(validate_env_name("dev_test").is_err());
}

#[test]
fn test_config_format_parse_yml_alias() {
    assert_eq!(ConfigFormat::parse("YML").unwrap(), ConfigFormat::Yaml);
}

#[test]
fn test_detect_project_config_yml_extension() {
    let temp = std::env::temp_dir().join("envforge-test-detect-yml");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(
        temp.join(".envforge.project.yml"),
        "project:\n  name: t\n  schema_path: .env.schema\n  active_environment: dev\nenvironments:\n  - name: dev\n    env_file: .env.dev\n",
    )
    .unwrap();

    let detected = detect_project_config(&temp);
    assert!(detected.is_some());
    assert_eq!(detected.unwrap().format, ConfigFormat::Yaml);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_init_project_preserves_existing_env() {
    let temp = std::env::temp_dir().join("envforge-test-init-preserve");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // Pre-create env file with content
    let env_path = temp.join(".env.development");
    std::fs::write(&env_path, "EXISTING=value\n").unwrap();

    let opts = InitOptions {
        root: temp.clone(),
        format: ConfigFormat::Toml,
        project_name: "preserve-test".to_string(),
        default_env_name: "development".to_string(),
        env_file_path: ".env.development".into(),
        schema_path: ".env.schema".into(),
        force: false,
    };

    init_project(&opts).unwrap();

    // Existing content should be preserved
    let content = std::fs::read_to_string(&env_path).unwrap();
    assert!(content.contains("EXISTING=value"));

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_serialize_all_formats_no_description() {
    let config = ProjectConfig {
        project: ProjectMeta {
            name: "t".to_string(),
            schema_path: ".env.schema".into(),
            active_environment: "dev".to_string(),
        },
        wizard: WizardState::default(),
        environments: vec![ProjectEnvironment {
            name: "dev".to_string(),
            env_file: ".env.dev".into(),
            description: None,
        }],
    };

    // All formats should serialize without description field
    let toml_out = serialize_project_config(&config, ConfigFormat::Toml).unwrap();
    assert!(!toml_out.contains("description"));

    let yaml_out = serialize_project_config(&config, ConfigFormat::Yaml).unwrap();
    assert!(!yaml_out.contains("description"));

    let json_out = serialize_project_config(&config, ConfigFormat::Json).unwrap();
    assert!(!json_out.contains("description"));
}

#[test]
fn test_find_environment_by_name() {
    let config = sample_config();
    let env = find_environment(&config, "production").unwrap();
    assert_eq!(env.env_file, std::path::PathBuf::from(".env.production"));
}

#[test]
fn test_find_environment_error_lists_available() {
    let config = sample_config();
    let err = find_environment(&config, "staging").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("staging"));
    assert!(msg.contains("development"));
    assert!(msg.contains("production"));
}

#[test]
fn test_active_env_path_with_nested_path() {
    let config = ProjectConfig {
        project: ProjectMeta {
            name: "t".to_string(),
            schema_path: ".env.schema".into(),
            active_environment: "dev".to_string(),
        },
        wizard: WizardState::default(),
        environments: vec![ProjectEnvironment {
            name: "dev".to_string(),
            env_file: "envs/.env.dev".into(),
            description: None,
        }],
    };
    let path = active_env_path(&config, Path::new("/project")).unwrap();
    assert_eq!(path, Path::new("/project/envs/.env.dev"));
}

#[test]
fn test_derive_project_name_root() {
    // Edge case: root path
    let name = derive_project_name(Path::new("/"));
    // Should not panic, returns something
    assert!(!name.is_empty() || name == "my-project");
}

#[test]
fn test_config_with_multiple_wizard_steps() {
    let config = ProjectConfig {
        project: ProjectMeta {
            name: "t".to_string(),
            schema_path: ".env.schema".into(),
            active_environment: "dev".to_string(),
        },
        wizard: WizardState {
            completed_steps: vec![
                "init".to_string(),
                "schema".to_string(),
                "values".to_string(),
            ],
        },
        environments: vec![ProjectEnvironment {
            name: "dev".to_string(),
            env_file: ".env.dev".into(),
            description: None,
        }],
    };

    // Roundtrip all 3 formats
    for fmt in [ConfigFormat::Toml, ConfigFormat::Yaml, ConfigFormat::Json] {
        let serialized = serialize_project_config(&config, fmt).unwrap();
        let parsed = parse_project_config(&serialized, fmt, Path::new("test")).unwrap();
        assert_eq!(parsed.wizard.completed_steps.len(), 3);
        assert!(parsed
            .wizard
            .completed_steps
            .contains(&"values".to_string()));
    }
}

#[test]
fn test_error_parse_error_display() {
    let err = ProjectError::ParseError {
        path: "/test/config.toml".into(),
        details: "expected table".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/test/config.toml"));
    assert!(msg.contains("expected table"));
}

#[test]
fn test_error_wizard_error_display() {
    let err = ProjectError::WizardError {
        step: "schema".to_string(),
        message: "failed to write".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("schema"));
    assert!(msg.contains("failed to write"));
}

#[test]
fn test_error_schema_not_found_display() {
    let err = ProjectError::SchemaNotFound {
        path: "/project/.env.schema".into(),
    };
    assert!(err.to_string().contains(".env.schema"));
}
