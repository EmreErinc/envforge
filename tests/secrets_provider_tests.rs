use envforge::ops::secrets::providers;
use envforge::ops::secrets::SecretProvider;
use std::collections::HashMap;

// ─── AWS SSM Tests ──────────────────────────────────────────

#[test]
fn test_aws_ssm_parse_output_basic() {
    let json = r#"{
        "Parameters": [
            {"Name": "/myapp/prod/DB_HOST", "Value": "localhost", "Type": "SecureString"},
            {"Name": "/myapp/prod/DB_PORT", "Value": "5432", "Type": "SecureString"}
        ]
    }"#;

    let result = providers::aws_ssm::parse_ssm_output(json, "/myapp/prod").unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
    assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
}

#[test]
fn test_aws_ssm_parse_output_strips_prefix() {
    let json = r#"{
        "Parameters": [
            {"Name": "/app/SECRET_KEY", "Value": "abc123"}
        ]
    }"#;

    let result = providers::aws_ssm::parse_ssm_output(json, "/app").unwrap();
    assert_eq!(result[0].0, "SECRET_KEY");
}

#[test]
fn test_aws_ssm_parse_output_trailing_slash_prefix() {
    let json = r#"{
        "Parameters": [
            {"Name": "/app/KEY", "Value": "val"}
        ]
    }"#;

    // Prefix with trailing slash should still work
    let result = providers::aws_ssm::parse_ssm_output(json, "/app/").unwrap();
    assert_eq!(result[0].0, "KEY");
}

#[test]
fn test_aws_ssm_parse_output_empty_parameters() {
    let json = r#"{"Parameters": []}"#;
    let result = providers::aws_ssm::parse_ssm_output(json, "/app").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_aws_ssm_parse_output_invalid_json() {
    let result = providers::aws_ssm::parse_ssm_output("not json", "/app");
    assert!(result.is_err());
}

#[test]
fn test_aws_ssm_parse_output_missing_parameters_field() {
    let json = r#"{"SomethingElse": []}"#;
    let result = providers::aws_ssm::parse_ssm_output(json, "/app");
    assert!(result.is_err());
}

#[test]
fn test_aws_ssm_parse_output_nested_path() {
    let json = r#"{
        "Parameters": [
            {"Name": "/myapp/prod/db/HOST", "Value": "db.example.com"},
            {"Name": "/myapp/prod/db/PORT", "Value": "3306"}
        ]
    }"#;

    let result = providers::aws_ssm::parse_ssm_output(json, "/myapp/prod").unwrap();
    assert_eq!(
        result[0],
        ("db/HOST".to_string(), "db.example.com".to_string())
    );
    assert_eq!(result[1], ("db/PORT".to_string(), "3306".to_string()));
}

#[test]
fn test_aws_ssm_build_env_full() {
    let mut creds = HashMap::new();
    creds.insert("access_key".into(), "AKIA123".into());
    creds.insert("secret_key".into(), "secret".into());
    creds.insert("profile".into(), "myprofile".into());
    creds.insert("region".into(), "us-west-2".into());

    let env = providers::aws_ssm::build_env(&creds);
    assert_eq!(env.len(), 4);

    let env_map: HashMap<&str, &str> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    assert_eq!(env_map["AWS_ACCESS_KEY_ID"], "AKIA123");
    assert_eq!(env_map["AWS_SECRET_ACCESS_KEY"], "secret");
    assert_eq!(env_map["AWS_PROFILE"], "myprofile");
    assert_eq!(env_map["AWS_DEFAULT_REGION"], "us-west-2");
}

#[test]
fn test_aws_ssm_build_env_empty() {
    let creds = HashMap::new();
    let env = providers::aws_ssm::build_env(&creds);
    assert!(env.is_empty());
}

#[test]
fn test_aws_ssm_provider_metadata() {
    let provider = providers::AwsSsmProvider;
    assert_eq!(provider.name(), "aws-ssm");
    assert_eq!(provider.display_name(), "AWS SSM Parameter Store");
    assert_eq!(provider.binary_name(), "aws");
    assert!(provider.credential_fields().is_empty());
    assert_eq!(
        provider.optional_credential_fields(),
        vec!["access_key", "secret_key", "region", "profile"]
    );
}

// ─── Doppler Tests ──────────────────────────────────────────

#[test]
fn test_doppler_parse_output_basic() {
    let json = r#"{"API_KEY": "abc123", "DB_URL": "postgres://localhost"}"#;
    let result = providers::doppler::parse_doppler_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("API_KEY".to_string(), "abc123".to_string()));
    assert_eq!(
        result[1],
        ("DB_URL".to_string(), "postgres://localhost".to_string())
    );
}

#[test]
fn test_doppler_parse_output_filters_system_keys() {
    let json = r#"{
        "API_KEY": "abc123",
        "DOPPLER_PROJECT": "my-project",
        "DOPPLER_CONFIG": "prd",
        "DOPPLER_ENVIRONMENT": "production",
        "SECRET": "shhh"
    }"#;

    let result = providers::doppler::parse_doppler_output(json).unwrap();
    assert_eq!(result.len(), 2);
    let keys: Vec<&str> = result.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"API_KEY"));
    assert!(keys.contains(&"SECRET"));
    assert!(!keys.contains(&"DOPPLER_PROJECT"));
    assert!(!keys.contains(&"DOPPLER_CONFIG"));
    assert!(!keys.contains(&"DOPPLER_ENVIRONMENT"));
}

#[test]
fn test_doppler_parse_output_empty() {
    let json = r#"{}"#;
    let result = providers::doppler::parse_doppler_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_doppler_parse_output_only_system_keys() {
    let json = r#"{
        "DOPPLER_PROJECT": "my-project",
        "DOPPLER_CONFIG": "prd",
        "DOPPLER_ENVIRONMENT": "production"
    }"#;

    let result = providers::doppler::parse_doppler_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_doppler_parse_output_invalid_json() {
    let result = providers::doppler::parse_doppler_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_doppler_provider_metadata() {
    let provider = providers::DopplerProvider;
    assert_eq!(provider.name(), "doppler");
    assert_eq!(provider.display_name(), "Doppler");
    assert_eq!(provider.binary_name(), "doppler");
    assert_eq!(
        provider.credential_fields(),
        vec!["token", "project", "config"]
    );
}

// ─── Infisical Tests ────────────────────────────────────────

#[test]
fn test_infisical_parse_output_basic() {
    let json = r#"[
        {"key": "DB_HOST", "value": "localhost"},
        {"key": "DB_PORT", "value": "5432"}
    ]"#;

    let result = providers::infisical::parse_infisical_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
    assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
}

#[test]
fn test_infisical_parse_output_empty() {
    let json = r#"[]"#;
    let result = providers::infisical::parse_infisical_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_infisical_parse_output_skips_missing_fields() {
    let json = r#"[
        {"key": "VALID", "value": "yes"},
        {"key": "NO_VALUE"},
        {"value": "no_key"},
        {"other": "field"}
    ]"#;

    let result = providers::infisical::parse_infisical_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("VALID".to_string(), "yes".to_string()));
}

#[test]
fn test_infisical_parse_output_invalid_json() {
    let result = providers::infisical::parse_infisical_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_infisical_provider_metadata() {
    let provider = providers::InfisicalProvider;
    assert_eq!(provider.name(), "infisical");
    assert_eq!(provider.display_name(), "Infisical");
    assert_eq!(provider.binary_name(), "infisical");
    assert_eq!(
        provider.credential_fields(),
        vec!["token", "project_id", "environment"]
    );
}

// ─── Azure Key Vault Tests ──────────────────────────────────

#[test]
fn test_azure_to_vault_name() {
    assert_eq!(providers::azure::to_vault_name("DB_HOST"), "DB-HOST");
    assert_eq!(
        providers::azure::to_vault_name("MY_SECRET_KEY"),
        "MY-SECRET-KEY"
    );
    assert_eq!(providers::azure::to_vault_name("SIMPLE"), "SIMPLE");
    assert_eq!(providers::azure::to_vault_name("a_b_c"), "a-b-c");
}

#[test]
fn test_azure_from_vault_name() {
    assert_eq!(providers::azure::from_vault_name("DB-HOST"), "DB_HOST");
    assert_eq!(
        providers::azure::from_vault_name("MY-SECRET-KEY"),
        "MY_SECRET_KEY"
    );
    assert_eq!(providers::azure::from_vault_name("SIMPLE"), "SIMPLE");
    assert_eq!(providers::azure::from_vault_name("a-b-c"), "a_b_c");
}

#[test]
fn test_azure_name_roundtrip() {
    let names = vec!["DB_HOST", "API_KEY", "SECRET_TOKEN", "SIMPLE"];
    for name in names {
        let vault_name = providers::azure::to_vault_name(name);
        let recovered = providers::azure::from_vault_name(&vault_name);
        assert_eq!(recovered, name, "roundtrip failed for {}", name);
    }
}

#[test]
fn test_azure_provider_metadata() {
    let provider = providers::AzureKeyVaultProvider;
    assert_eq!(provider.name(), "azure");
    assert_eq!(provider.display_name(), "Azure Key Vault");
    assert_eq!(provider.binary_name(), "az");
    assert_eq!(provider.credential_fields(), vec!["vault_name"]);
}

// ─── Provider Registry Tests ────────────────────────────────

#[test]
fn test_registry_has_all_providers() {
    let registry = providers::create_default_registry();
    let names = registry.list_names();
    assert_eq!(names.len(), 7);
    assert!(names.contains(&"vault".to_string()));
    assert!(names.contains(&"aws-ssm".to_string()));
    assert!(names.contains(&"1password".to_string()));
    assert!(names.contains(&"doppler".to_string()));
    assert!(names.contains(&"infisical".to_string()));
    assert!(names.contains(&"gcp".to_string()));
    assert!(names.contains(&"azure".to_string()));
}

#[test]
fn test_registry_get_each_provider() {
    let registry = providers::create_default_registry();
    for name in &[
        "vault",
        "aws-ssm",
        "1password",
        "doppler",
        "infisical",
        "gcp",
        "azure",
    ] {
        let provider = registry.get(name).unwrap();
        assert_eq!(provider.name(), *name);
    }
}

#[test]
fn test_registry_get_unknown_provider() {
    let registry = providers::create_default_registry();
    let result = registry.get("nonexistent");
    assert!(result.is_err());
}

// ─── Validate Config Tests ──────────────────────────────────

#[test]
fn test_aws_validate_config_no_required_fields() {
    let provider = providers::AwsSsmProvider;
    // AWS has no required fields — should always pass
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_ok());
}

#[test]
fn test_doppler_validate_config_missing_fields() {
    let provider = providers::DopplerProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_doppler_validate_config_complete() {
    let provider = providers::DopplerProvider;
    let mut creds = HashMap::new();
    creds.insert("token".into(), "dp.st.xxx".into());
    creds.insert("project".into(), "myproject".into());
    creds.insert("config".into(), "prd".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

#[test]
fn test_infisical_validate_config_missing_fields() {
    let provider = providers::InfisicalProvider;
    let mut creds = HashMap::new();
    creds.insert("token".into(), "xxx".into());
    // Missing project_id and environment
    let result = provider.validate_config(&creds);
    assert!(result.is_err());
}

#[test]
fn test_azure_validate_config_missing_vault() {
    let provider = providers::AzureKeyVaultProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_azure_validate_config_complete() {
    let provider = providers::AzureKeyVaultProvider;
    let mut creds = HashMap::new();
    creds.insert("vault_name".into(), "my-vault".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── Vault Tests ────────────────────────────────────────────

#[test]
fn test_vault_parse_kv_get_v2() {
    let json = r#"{
        "data": {
            "data": {
                "DB_HOST": "localhost",
                "DB_PORT": "5432"
            },
            "metadata": {"version": 1}
        }
    }"#;

    let result = providers::vault::parse_kv_get_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
    assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
}

#[test]
fn test_vault_parse_kv_get_v1() {
    let json = r#"{
        "data": {
            "SECRET": "value123"
        }
    }"#;

    let result = providers::vault::parse_kv_get_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("SECRET".to_string(), "value123".to_string()));
}

#[test]
fn test_vault_parse_kv_get_invalid_json() {
    let result = providers::vault::parse_kv_get_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_vault_parse_kv_get_missing_data() {
    let json = r#"{"something_else": {}}"#;
    let result = providers::vault::parse_kv_get_output(json);
    assert!(result.is_err());
}

#[test]
fn test_vault_parse_kv_list_output() {
    let json = r#"{
        "data": {
            "keys": ["secret1", "secret2", "subpath/"]
        }
    }"#;

    let result = providers::vault::parse_kv_list_output(json).unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&"secret1".to_string()));
    assert!(result.contains(&"secret2".to_string()));
    assert!(result.contains(&"subpath/".to_string()));
}

#[test]
fn test_vault_parse_kv_list_empty() {
    let json = r#"{"data": {"keys": []}}"#;
    let result = providers::vault::parse_kv_list_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_vault_parse_kv_list_invalid_structure() {
    // Bare array (the old broken format) should now fail
    let json = r#"["key1", "key2"]"#;
    let result = providers::vault::parse_kv_list_output(json);
    assert!(result.is_err());
}

#[test]
fn test_vault_build_env() {
    let mut creds = HashMap::new();
    creds.insert("addr".into(), "http://127.0.0.1:8200".into());
    creds.insert("token".into(), "s.mytoken".into());

    let env = providers::vault::build_env(&creds);
    let env_map: HashMap<&str, &str> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    assert_eq!(env_map["VAULT_ADDR"], "http://127.0.0.1:8200");
    assert_eq!(env_map["VAULT_TOKEN"], "s.mytoken");
}

#[test]
fn test_vault_provider_metadata() {
    let provider = providers::VaultProvider;
    assert_eq!(provider.name(), "vault");
    assert_eq!(provider.display_name(), "HashiCorp Vault");
    assert_eq!(provider.binary_name(), "vault");
    assert_eq!(provider.credential_fields(), vec!["addr"]);
    assert_eq!(
        provider.optional_credential_fields(),
        vec!["token", "role_id", "secret_id", "auth_method"]
    );
}

#[test]
fn test_vault_validate_config_missing_addr() {
    let provider = providers::VaultProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_vault_validate_config_complete() {
    let provider = providers::VaultProvider;
    let mut creds = HashMap::new();
    creds.insert("addr".into(), "http://vault:8200".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── 1Password Tests ───────────────────────────────────────

#[test]
fn test_onepassword_parse_item_output() {
    let json = r#"{
        "id": "abc123",
        "title": "My Item",
        "fields": [
            {"label": "username", "value": "admin", "type": "STRING"},
            {"label": "password", "value": "secret123", "type": "CONCEALED"},
            {"label": "", "value": "empty_label", "type": "STRING"},
            {"label": "empty_val", "value": "", "type": "STRING"}
        ]
    }"#;

    let result = providers::onepassword::parse_item_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("password".to_string(), "secret123".to_string()));
    assert_eq!(result[1], ("username".to_string(), "admin".to_string()));
}

#[test]
fn test_onepassword_parse_item_output_empty_fields() {
    let json = r#"{"id": "abc", "title": "Empty", "fields": []}"#;
    let result = providers::onepassword::parse_item_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_onepassword_parse_item_output_no_fields() {
    let json = r#"{"id": "abc", "title": "No Fields"}"#;
    let result = providers::onepassword::parse_item_output(json);
    assert!(result.is_err());
}

#[test]
fn test_onepassword_parse_item_output_invalid_json() {
    let result = providers::onepassword::parse_item_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_onepassword_build_env() {
    let mut creds = HashMap::new();
    creds.insert("service_account_token".into(), "ops_token123".into());

    let env = providers::onepassword::build_env(&creds);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "OP_SERVICE_ACCOUNT_TOKEN");
    assert_eq!(env[0].1, "ops_token123");
}

#[test]
fn test_onepassword_provider_metadata() {
    let provider = providers::OnePasswordProvider;
    assert_eq!(provider.name(), "1password");
    assert_eq!(provider.display_name(), "1Password");
    assert_eq!(provider.binary_name(), "op");
    assert_eq!(provider.credential_fields(), vec!["service_account_token"]);
}

// ─── GCP Tests ──────────────────────────────────────────────

#[test]
fn test_gcp_parse_list_output() {
    let json = r#"[
        {"name": "projects/123456/secrets/DB_HOST", "createTime": "2024-01-01T00:00:00Z"},
        {"name": "projects/123456/secrets/API_KEY", "createTime": "2024-01-02T00:00:00Z"}
    ]"#;

    let result = providers::gcp::parse_gcp_list_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "API_KEY");
    assert_eq!(result[1], "DB_HOST");
}

#[test]
fn test_gcp_parse_list_output_empty_array() {
    let result = providers::gcp::parse_gcp_list_output("[]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_gcp_parse_list_output_empty_string() {
    let result = providers::gcp::parse_gcp_list_output("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_gcp_parse_list_output_whitespace_only() {
    let result = providers::gcp::parse_gcp_list_output("  \n  ").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_gcp_parse_list_output_invalid_json() {
    let result = providers::gcp::parse_gcp_list_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_gcp_provider_metadata() {
    let provider = providers::GcpSecretManagerProvider;
    assert_eq!(provider.name(), "gcp");
    assert_eq!(provider.display_name(), "GCP Secret Manager");
    assert_eq!(provider.binary_name(), "gcloud");
    assert_eq!(provider.credential_fields(), vec!["project_id"]);
}

#[test]
fn test_gcp_validate_config_missing_project() {
    let provider = providers::GcpSecretManagerProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_gcp_validate_config_complete() {
    let provider = providers::GcpSecretManagerProvider;
    let mut creds = HashMap::new();
    creds.insert("project_id".into(), "my-project".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}
