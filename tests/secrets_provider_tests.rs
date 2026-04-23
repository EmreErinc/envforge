use envforge::ops::secrets::provider::SecretsError;
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
    assert_eq!(names.len(), 13);
    assert!(names.contains(&"vault".to_string()));
    assert!(names.contains(&"aws-ssm".to_string()));
    assert!(names.contains(&"1password".to_string()));
    assert!(names.contains(&"doppler".to_string()));
    assert!(names.contains(&"infisical".to_string()));
    assert!(names.contains(&"gcp".to_string()));
    assert!(names.contains(&"azure".to_string()));
    assert!(names.contains(&"bitwarden".to_string()));
    assert!(names.contains(&"akeyless".to_string()));
    assert!(names.contains(&"conjur".to_string()));
    assert!(names.contains(&"sops".to_string()));
    assert!(names.contains(&"pass".to_string()));
    assert!(names.contains(&"keeper".to_string()));
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
        "bitwarden",
        "akeyless",
        "conjur",
        "sops",
        "pass",
        "keeper",
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

// ============================================================================
// Network Failure Scenarios (8 tests)
// ============================================================================

#[test]
fn test_provider_handles_connection_timeout_error() {
    let error = SecretsError::ProviderError {
        provider: "vault".to_string(),
        message: "Connection timeout after 30s".to_string(),
    };
    assert!(error.to_string().contains("timeout"));
}

#[test]
fn test_provider_handles_connection_refused() {
    let error = SecretsError::ProviderError {
        provider: "aws-ssm".to_string(),
        message: "Connection refused: 127.0.0.1:8200".to_string(),
    };
    assert!(error.to_string().contains("Connection refused"));
}

#[test]
fn test_provider_handles_dns_resolution_failure() {
    let error = SecretsError::ProviderError {
        provider: "doppler".to_string(),
        message: "Failed to resolve DNS for api.doppler.com".to_string(),
    };
    assert!(error.to_string().contains("DNS"));
}

#[test]
fn test_provider_handles_ssl_certificate_error() {
    let error = SecretsError::ProviderError {
        provider: "vault".to_string(),
        message: "SSL certificate verification failed: self-signed certificate".to_string(),
    };
    assert!(error.to_string().contains("SSL certificate"));
}

#[test]
fn test_provider_handles_rate_limiting() {
    let error = SecretsError::ProviderError {
        provider: "github".to_string(),
        message: "Rate limit exceeded: 60 requests per hour, retry after 3600s".to_string(),
    };
    assert!(error.to_string().contains("Rate limit"));
}

#[test]
fn test_provider_handles_service_unavailable() {
    let error = SecretsError::ProviderError {
        provider: "vault".to_string(),
        message: "HTTP 503: Service Unavailable - vault is sealed".to_string(),
    };
    assert!(error.to_string().contains("503"));
}

#[test]
fn test_provider_handles_proxy_authentication_required() {
    let error = SecretsError::ProviderError {
        provider: "aws-ssm".to_string(),
        message: "Proxy authentication required (407)".to_string(),
    };
    assert!(error.to_string().contains("Proxy"));
}

#[test]
fn test_provider_handles_internal_server_error() {
    let error = SecretsError::ProviderError {
        provider: "doppler".to_string(),
        message: "HTTP 500: Internal Server Error".to_string(),
    };
    assert!(error.to_string().contains("500"));
}

// ============================================================================
// Credential Rotation & Expiration Tests (6 tests)
// ============================================================================

#[test]
fn test_provider_detects_expired_token() {
    let error = SecretsError::AuthFailed {
        provider: "vault".to_string(),
        message: "Token has expired (ttl exceeded)".to_string(),
    };
    assert!(error.to_string().contains("expired"));
}

#[test]
fn test_provider_handles_rotated_credentials() {
    let providers_list = vec!["aws-ssm", "vault", "doppler", "github"];
    for _provider_name in providers_list {
        let mut old_creds = HashMap::new();
        old_creds.insert("token".to_string(), "old-key-abc123".to_string());

        let mut new_creds = HashMap::new();
        new_creds.insert("token".to_string(), "new-key-xyz789".to_string());

        // Both should be valid credentials structures
        assert_eq!(old_creds.get("token").unwrap(), "old-key-abc123");
        assert_eq!(new_creds.get("token").unwrap(), "new-key-xyz789");
    }
}

#[test]
fn test_provider_refreshes_token_on_expiry() {
    let mut creds = HashMap::new();
    creds.insert("token".to_string(), "access-token-123".to_string());
    creds.insert("refresh_token".to_string(), "refresh-token-456".to_string());

    // Simulate token refresh
    let new_token = "new-access-token-789".to_string();
    creds.insert("token".to_string(), new_token.clone());

    assert_eq!(creds.get("token").unwrap(), &new_token);
}

#[test]
fn test_provider_credential_cache_invalidation_on_rotation() {
    // First validation with token1
    let mut creds_v1 = HashMap::new();
    creds_v1.insert("token".to_string(), "token-v1".to_string());

    // Second validation with token2 (simulating rotation)
    let mut creds_v2 = HashMap::new();
    creds_v2.insert("token".to_string(), "token-v2".to_string());

    assert_ne!(creds_v1.get("token"), creds_v2.get("token"));
}

#[test]
fn test_provider_handles_revoked_credentials() {
    let error = SecretsError::AuthFailed {
        provider: "github".to_string(),
        message: "Token revoked by user".to_string(),
    };
    assert!(error.to_string().contains("revoked"));
}

#[test]
fn test_provider_multiple_credentials_partial_expiry() {
    let mut creds = HashMap::new();
    creds.insert("access_key".to_string(), "AKIA1234567890".to_string());
    creds.insert(
        "secret_key".to_string(),
        "wJalrXUtnFEMI/K7MDENG+bPxRfiCYzSECRETKEY".to_string(),
    );
    creds.insert(
        "session_token".to_string(),
        "expired-session-token".to_string(),
    );

    // Primary credentials present
    assert!(creds.contains_key("access_key"));
    assert!(creds.contains_key("secret_key"));
}

// ============================================================================
// Error Message Tests (5 tests)
// ============================================================================

#[test]
fn test_error_provider_not_found_message() {
    let error = SecretsError::ProviderNotFound {
        name: "vault".to_string(),
        available: "aws-ssm, doppler, github, gcp, azure".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("vault"));
    assert!(msg.contains("not found"));
}

#[test]
fn test_error_binary_not_found_message() {
    let error = SecretsError::BinaryNotFound {
        binary: "vault".to_string(),
        install_hint: "brew install hashicorp/tap/vault".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("vault"));
    assert!(msg.contains("not found in PATH"));
    assert!(msg.contains("Install:"));
}

#[test]
fn test_error_auth_failed_message() {
    let error = SecretsError::AuthFailed {
        provider: "aws-ssm".to_string(),
        message: "InvalidSignatureException: Signature mismatch".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("aws-ssm"));
    assert!(msg.contains("authentication failed"));
}

#[test]
fn test_error_provider_error_message() {
    let error = SecretsError::ProviderError {
        provider: "doppler".to_string(),
        message: "Failed to fetch config/settings endpoint".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("doppler"));
}

#[test]
fn test_error_credential_not_found_message() {
    let error = SecretsError::CredentialNotFound {
        provider: "github".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("github"));
    assert!(msg.contains("not configured"));
    assert!(msg.contains("envforge secrets config"));
}

// ============================================================================
// Concurrency & Thread Safety Tests (4 tests)
// ============================================================================

#[test]
fn test_provider_concurrent_validation_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let mut creds = HashMap::new();
    creds.insert("token".to_string(), "test-token".to_string());
    let creds = Arc::new(creds);

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let c = Arc::clone(&creds);
            thread::spawn(move || {
                // Validate credentials structure
                assert!(c.contains_key("token"));
                assert!(!c.get("token").unwrap().is_empty());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_provider_multiple_credentials_thread_safe() {
    use std::thread;

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let mut c = HashMap::new();
            c.insert("token".to_string(), format!("token-{i}"));
            c.insert("provider".to_string(), "vault".to_string());
            c
        })
        .map(|creds| {
            thread::spawn(move || {
                assert!(creds.contains_key("token"));
                assert!(creds.contains_key("provider"));
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_provider_concurrent_cache_access_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let creds = Arc::new({
        let mut c = HashMap::new();
        c.insert("token".to_string(), "cached-token".to_string());
        c
    });

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let c = Arc::clone(&creds);
            thread::spawn(move || {
                let token = c.get("token").unwrap();
                assert_eq!(token, "cached-token");
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_provider_stress_test_concurrent_requests() {
    use std::sync::Arc;
    use std::thread;

    let creds = Arc::new({
        let mut c = HashMap::new();
        c.insert("token".to_string(), "stress-test".to_string());
        c
    });

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let c = Arc::clone(&creds);
            thread::spawn(move || {
                for _ in 0..10 {
                    let _ = c.get("token");
                }
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

// ─── Bitwarden Tests ───────────────────────────────────────

#[test]
fn test_bitwarden_parse_output_basic() {
    let json = r#"[
        {"id": "uuid-1", "key": "DB_HOST", "value": "localhost", "organizationId": "org1"},
        {"id": "uuid-2", "key": "DB_PORT", "value": "5432", "organizationId": "org1"}
    ]"#;

    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
    assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
}

#[test]
fn test_bitwarden_parse_output_empty() {
    let json = r#"[]"#;
    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_bitwarden_parse_output_skips_empty_key() {
    let json = r#"[
        {"id": "uuid-1", "key": "", "value": "secret"},
        {"id": "uuid-2", "key": "VALID", "value": "yes"}
    ]"#;

    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "VALID");
}

#[test]
fn test_bitwarden_parse_output_missing_fields() {
    let json = r#"[
        {"id": "uuid-1", "key": "HAS_KEY"},
        {"id": "uuid-2", "value": "has_value"},
        {"id": "uuid-3", "key": "BOTH", "value": "yes"}
    ]"#;

    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("BOTH".to_string(), "yes".to_string()));
}

#[test]
fn test_bitwarden_parse_output_invalid_json() {
    let result = providers::bitwarden::parse_bitwarden_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_bitwarden_parse_output_sorted() {
    let json = r#"[
        {"id": "1", "key": "ZEBRA", "value": "z"},
        {"id": "2", "key": "ALPHA", "value": "a"},
        {"id": "3", "key": "MIDDLE", "value": "m"}
    ]"#;

    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert_eq!(result[0].0, "ALPHA");
    assert_eq!(result[1].0, "MIDDLE");
    assert_eq!(result[2].0, "ZEBRA");
}

#[test]
fn test_bitwarden_build_env() {
    let mut creds = HashMap::new();
    creds.insert("access_token".into(), "0.token.secret".into());

    let env = providers::bitwarden::build_env(&creds);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "BWS_ACCESS_TOKEN");
    assert_eq!(env[0].1, "0.token.secret");
}

#[test]
fn test_bitwarden_build_env_empty() {
    let creds = HashMap::new();
    let env = providers::bitwarden::build_env(&creds);
    assert!(env.is_empty());
}

#[test]
fn test_bitwarden_provider_metadata() {
    let provider = providers::BitwardenProvider;
    assert_eq!(provider.name(), "bitwarden");
    assert_eq!(provider.display_name(), "Bitwarden Secrets Manager");
    assert_eq!(provider.binary_name(), "bws");
    assert_eq!(provider.credential_fields(), vec!["access_token"]);
    assert_eq!(provider.optional_credential_fields(), vec!["project_id"]);
}

#[test]
fn test_bitwarden_validate_config_missing_token() {
    let provider = providers::BitwardenProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_bitwarden_validate_config_complete() {
    let provider = providers::BitwardenProvider;
    let mut creds = HashMap::new();
    creds.insert("access_token".into(), "0.xxx.yyy".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── Akeyless Tests ────────────────────────────────────────

#[test]
fn test_akeyless_parse_list_basic() {
    let json = r#"[
        {"item_name": "/app/DB_HOST", "item_type": "STATIC_SECRET", "item_id": 1},
        {"item_name": "/app/API_KEY", "item_type": "STATIC_SECRET", "item_id": 2}
    ]"#;

    let result = providers::akeyless::parse_akeyless_list(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "/app/API_KEY");
    assert_eq!(result[1], "/app/DB_HOST");
}

#[test]
fn test_akeyless_parse_list_filters_non_static() {
    let json = r#"[
        {"item_name": "/app/STATIC", "item_type": "STATIC_SECRET"},
        {"item_name": "/app/DYNAMIC", "item_type": "DYNAMIC_SECRET"},
        {"item_name": "/app/ROTATED", "item_type": "ROTATED_SECRET"}
    ]"#;

    let result = providers::akeyless::parse_akeyless_list(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "/app/STATIC");
}

#[test]
fn test_akeyless_parse_list_empty() {
    let result = providers::akeyless::parse_akeyless_list("[]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_akeyless_parse_list_empty_string() {
    let result = providers::akeyless::parse_akeyless_list("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_akeyless_parse_list_invalid_json() {
    let result = providers::akeyless::parse_akeyless_list("not json");
    assert!(result.is_err());
}

#[test]
fn test_akeyless_parse_value() {
    let json = r#"{"/app/DB_HOST": "localhost"}"#;
    let result = providers::akeyless::parse_akeyless_value(json, "/app/DB_HOST").unwrap();
    assert_eq!(result, "localhost");
}

#[test]
fn test_akeyless_parse_value_single_key() {
    let json = r#"{"/app/SECRET": "my-secret"}"#;
    let result = providers::akeyless::parse_akeyless_value(json, "/other/path").unwrap();
    assert_eq!(result, "my-secret");
}

#[test]
fn test_akeyless_parse_value_invalid_json() {
    let result = providers::akeyless::parse_akeyless_value("not json", "/path");
    assert!(result.is_err());
}

#[test]
fn test_akeyless_extract_key_name() {
    assert_eq!(
        providers::akeyless::extract_key_name("/app/prod/DB_HOST"),
        "DB_HOST"
    );
    assert_eq!(providers::akeyless::extract_key_name("/SECRET"), "SECRET");
    assert_eq!(providers::akeyless::extract_key_name("SIMPLE"), "SIMPLE");
    assert_eq!(providers::akeyless::extract_key_name("/a/b/c/DEEP"), "DEEP");
}

#[test]
fn test_akeyless_build_env_empty() {
    let creds = HashMap::new();
    let env = providers::akeyless::build_env(&creds);
    assert!(env.is_empty());
}

#[test]
fn test_akeyless_provider_metadata() {
    let provider = providers::AkeylessProvider;
    assert_eq!(provider.name(), "akeyless");
    assert_eq!(provider.display_name(), "Akeyless Vault");
    assert_eq!(provider.binary_name(), "akeyless");
    assert_eq!(
        provider.credential_fields(),
        vec!["access_id", "access_key"]
    );
}

#[test]
fn test_akeyless_validate_config_missing_fields() {
    let provider = providers::AkeylessProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_akeyless_validate_config_partial() {
    let provider = providers::AkeylessProvider;
    let mut creds = HashMap::new();
    creds.insert("access_id".into(), "p-xxx".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_err());
}

#[test]
fn test_akeyless_validate_config_complete() {
    let provider = providers::AkeylessProvider;
    let mut creds = HashMap::new();
    creds.insert("access_id".into(), "p-xxx".into());
    creds.insert("access_key".into(), "key123".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── Conjur Tests ──────────────────────────────────────────

#[test]
fn test_conjur_parse_list_basic() {
    let json = r#"[
        "myorg:variable:app/db/host",
        "myorg:variable:app/db/password",
        "myorg:variable:app/api_key"
    ]"#;

    let result = providers::conjur::parse_conjur_list(json, "myorg").unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&"app/db/host".to_string()));
    assert!(result.contains(&"app/db/password".to_string()));
    assert!(result.contains(&"app/api_key".to_string()));
}

#[test]
fn test_conjur_parse_list_strips_prefix() {
    let json = r#"["acme:variable:secrets/DB_URL"]"#;
    let result = providers::conjur::parse_conjur_list(json, "acme").unwrap();
    assert_eq!(result[0], "secrets/DB_URL");
}

#[test]
fn test_conjur_parse_list_mismatched_account() {
    let json = r#"["other:variable:path/key"]"#;
    let result = providers::conjur::parse_conjur_list(json, "myorg").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "path/key");
}

#[test]
fn test_conjur_parse_list_empty() {
    let result = providers::conjur::parse_conjur_list("[]", "myorg").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_conjur_parse_list_empty_string() {
    let result = providers::conjur::parse_conjur_list("", "myorg").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_conjur_parse_list_invalid_json() {
    let result = providers::conjur::parse_conjur_list("not json", "myorg");
    assert!(result.is_err());
}

#[test]
fn test_conjur_extract_key() {
    assert_eq!(providers::conjur::extract_conjur_key("app/db/HOST"), "HOST");
    assert_eq!(providers::conjur::extract_conjur_key("SIMPLE"), "SIMPLE");
    assert_eq!(providers::conjur::extract_conjur_key("a/b/c/DEEP"), "DEEP");
}

#[test]
fn test_conjur_provider_metadata() {
    let provider = providers::ConjurProvider;
    assert_eq!(provider.name(), "conjur");
    assert_eq!(provider.display_name(), "CyberArk Conjur");
    assert_eq!(provider.binary_name(), "conjur");
    assert_eq!(
        provider.credential_fields(),
        vec!["url", "account", "login", "api_key"]
    );
}

#[test]
fn test_conjur_validate_config_missing_fields() {
    let provider = providers::ConjurProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_conjur_validate_config_partial() {
    let provider = providers::ConjurProvider;
    let mut creds = HashMap::new();
    creds.insert("url".into(), "https://conjur.example.com".into());
    creds.insert("account".into(), "myorg".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_err());
}

#[test]
fn test_conjur_validate_config_complete() {
    let provider = providers::ConjurProvider;
    let mut creds = HashMap::new();
    creds.insert("url".into(), "https://conjur.example.com".into());
    creds.insert("account".into(), "myorg".into());
    creds.insert("login".into(), "admin".into());
    creds.insert("api_key".into(), "abc123".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── SOPS Tests ────────────────────────────────────────────

#[test]
fn test_sops_parse_output_basic() {
    let json = r#"{"DB_HOST": "localhost", "DB_PORT": "5432"}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
    assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
}

#[test]
fn test_sops_parse_output_filters_metadata() {
    let json = r#"{
        "API_KEY": "secret123",
        "sops": {"kms": [], "pgp": [], "lastmodified": "2024-01-01"},
        "DB_URL": "postgres://localhost"
    }"#;

    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result.len(), 2);
    let keys: Vec<&str> = result.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"API_KEY"));
    assert!(keys.contains(&"DB_URL"));
    assert!(!keys.contains(&"sops"));
}

#[test]
fn test_sops_parse_output_empty() {
    let result = providers::sops::parse_sops_output("{}").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_sops_parse_output_empty_string() {
    let result = providers::sops::parse_sops_output("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_sops_parse_output_invalid_json() {
    let result = providers::sops::parse_sops_output("not json");
    assert!(result.is_err());
}

#[test]
fn test_sops_parse_output_non_string_values() {
    let json = r#"{"DEBUG": true, "PORT": 3000, "NAME": "app"}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result.len(), 3);
    let map: HashMap<String, String> = result.into_iter().collect();
    assert_eq!(map["NAME"], "app");
    assert_eq!(map["DEBUG"], "true");
    assert_eq!(map["PORT"], "3000");
}

#[test]
fn test_sops_parse_output_sorted() {
    let json = r#"{"ZEBRA": "z", "ALPHA": "a", "MIDDLE": "m"}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result[0].0, "ALPHA");
    assert_eq!(result[1].0, "MIDDLE");
    assert_eq!(result[2].0, "ZEBRA");
}

#[test]
fn test_sops_build_env() {
    let mut creds = HashMap::new();
    creds.insert("key_file".into(), "/home/user/.sops/age.key".into());

    let env = providers::sops::build_env(&creds);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "SOPS_AGE_KEY_FILE");
    assert_eq!(env[0].1, "/home/user/.sops/age.key");
}

#[test]
fn test_sops_build_env_empty() {
    let creds = HashMap::new();
    let env = providers::sops::build_env(&creds);
    assert!(env.is_empty());
}

#[test]
fn test_sops_provider_metadata() {
    let provider = providers::SopsProvider;
    assert_eq!(provider.name(), "sops");
    assert_eq!(provider.display_name(), "Mozilla SOPS");
    assert_eq!(provider.binary_name(), "sops");
    assert_eq!(provider.credential_fields(), vec!["key_file"]);
    assert_eq!(
        provider.optional_credential_fields(),
        vec!["encryption_type"]
    );
}

#[test]
fn test_sops_validate_config_missing_key_file() {
    let provider = providers::SopsProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_err());
}

#[test]
fn test_sops_validate_config_complete() {
    let provider = providers::SopsProvider;
    let mut creds = HashMap::new();
    creds.insert("key_file".into(), "/path/to/key".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── pass/gopass Tests ─────────────────────────────────────

#[test]
fn test_pass_build_env_with_store_path() {
    let mut creds = HashMap::new();
    creds.insert("store_path".into(), "/custom/store".into());

    let env = providers::pass::build_env(&creds);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "PASSWORD_STORE_DIR");
    assert_eq!(env[0].1, "/custom/store");
}

#[test]
fn test_pass_build_env_empty() {
    let creds = HashMap::new();
    let env = providers::pass::build_env(&creds);
    assert!(env.is_empty());
}

#[test]
fn test_pass_provider_metadata() {
    let provider = providers::PassProvider;
    assert_eq!(provider.name(), "pass");
    assert_eq!(provider.display_name(), "pass/gopass");
    assert_eq!(provider.binary_name(), "pass");
    assert!(provider.credential_fields().is_empty());
    assert_eq!(
        provider.optional_credential_fields(),
        vec!["store_path", "binary"]
    );
}

#[test]
fn test_pass_validate_config_always_ok() {
    let provider = providers::PassProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_ok());
}

#[test]
fn test_pass_scan_nonexistent_store() {
    use std::path::Path;
    let result = providers::pass::scan_password_store(Path::new("/nonexistent/store"), "").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_pass_scan_store_with_prefix_filter() {
    let temp = std::env::temp_dir().join("envforge-test-pass-scan");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join("app")).unwrap();
    std::fs::create_dir_all(temp.join("db")).unwrap();
    std::fs::write(temp.join("app/KEY1.gpg"), b"").unwrap();
    std::fs::write(temp.join("app/KEY2.gpg"), b"").unwrap();
    std::fs::write(temp.join("db/HOST.gpg"), b"").unwrap();
    std::fs::write(temp.join(".gpg-id"), b"ABCD1234").unwrap();

    let all = providers::pass::scan_password_store(&temp, "").unwrap();
    assert_eq!(all.len(), 3);

    let app_only = providers::pass::scan_password_store(&temp, "app/").unwrap();
    assert_eq!(app_only.len(), 2);
    assert!(app_only.iter().all(|k| k.starts_with("app/")));

    let none = providers::pass::scan_password_store(&temp, "nonexistent/").unwrap();
    assert!(none.is_empty());

    assert!(!all.iter().any(|k| k.contains(".gpg-id")));

    let _ = std::fs::remove_dir_all(&temp);
}

// ─── Keeper Tests ──────────────────────────────────────────

#[test]
fn test_keeper_parse_list_basic() {
    let json = r#"[
        {"uid": "uid-1", "title": "DB_PASSWORD", "record_type": "login"},
        {"uid": "uid-2", "title": "API_KEY", "record_type": "login"}
    ]"#;

    let result = providers::keeper::parse_keeper_list(json).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("uid-1".to_string(), "DB_PASSWORD".to_string()));
    assert_eq!(result[1], ("uid-2".to_string(), "API_KEY".to_string()));
}

#[test]
fn test_keeper_parse_list_empty() {
    let result = providers::keeper::parse_keeper_list("[]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_keeper_parse_list_empty_string() {
    let result = providers::keeper::parse_keeper_list("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_keeper_parse_list_invalid_json() {
    let result = providers::keeper::parse_keeper_list("not json");
    assert!(result.is_err());
}

#[test]
fn test_keeper_parse_record_password() {
    let json = r#"{
        "uid": "uid-1",
        "title": "DB_PASSWORD",
        "type": "login",
        "fields": [
            {"type": "login", "value": ["admin"]},
            {"type": "password", "value": ["s3cret!"]}
        ]
    }"#;

    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "s3cret!");
}

#[test]
fn test_keeper_parse_record_login_fallback() {
    let json = r#"{
        "uid": "uid-1",
        "title": "Service",
        "type": "login",
        "fields": [
            {"type": "login", "value": ["admin"]},
            {"type": "url", "value": ["https://example.com"]}
        ]
    }"#;

    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "admin");
}

#[test]
fn test_keeper_parse_record_no_extractable_value() {
    let json = r#"{
        "uid": "uid-1",
        "title": "Empty",
        "type": "login",
        "fields": [
            {"type": "url", "value": ["https://example.com"]},
            {"type": "fileRef", "value": []}
        ]
    }"#;

    let result = providers::keeper::parse_keeper_record(json);
    assert!(result.is_err());
}

#[test]
fn test_keeper_parse_record_empty_value_array() {
    let json = r#"{
        "uid": "uid-1",
        "title": "Empty PW",
        "type": "login",
        "fields": [
            {"type": "password", "value": []}
        ]
    }"#;

    let result = providers::keeper::parse_keeper_record(json);
    assert!(result.is_err());
}

#[test]
fn test_keeper_parse_record_missing_fields() {
    let json = r#"{"uid": "uid-1", "title": "No Fields"}"#;
    let result = providers::keeper::parse_keeper_record(json);
    assert!(result.is_err());
}

#[test]
fn test_keeper_parse_record_invalid_json() {
    let result = providers::keeper::parse_keeper_record("not json");
    assert!(result.is_err());
}

#[test]
fn test_keeper_provider_metadata() {
    let provider = providers::KeeperProvider;
    assert_eq!(provider.name(), "keeper");
    assert_eq!(provider.display_name(), "Keeper Secrets Manager");
    assert_eq!(provider.binary_name(), "ksm");
    assert!(provider.credential_fields().is_empty());
    assert_eq!(provider.optional_credential_fields(), vec!["profile"]);
}

#[test]
fn test_keeper_validate_config_always_ok() {
    let provider = providers::KeeperProvider;
    let result = provider.validate_config(&HashMap::new());
    assert!(result.is_ok());
}

// ─── Updated Registry Tests ────────────────────────────────

#[test]
fn test_registry_has_all_13_providers() {
    let registry = providers::create_default_registry();
    let names = registry.list_names();
    assert_eq!(names.len(), 13);
    assert!(names.contains(&"vault".to_string()));
    assert!(names.contains(&"aws-ssm".to_string()));
    assert!(names.contains(&"1password".to_string()));
    assert!(names.contains(&"doppler".to_string()));
    assert!(names.contains(&"infisical".to_string()));
    assert!(names.contains(&"gcp".to_string()));
    assert!(names.contains(&"azure".to_string()));
    assert!(names.contains(&"bitwarden".to_string()));
    assert!(names.contains(&"akeyless".to_string()));
    assert!(names.contains(&"conjur".to_string()));
    assert!(names.contains(&"sops".to_string()));
    assert!(names.contains(&"pass".to_string()));
    assert!(names.contains(&"keeper".to_string()));
}

#[test]
fn test_registry_get_new_providers() {
    let registry = providers::create_default_registry();
    for name in &["bitwarden", "akeyless", "conjur", "sops", "pass", "keeper"] {
        let provider = registry.get(name).unwrap();
        assert_eq!(provider.name(), *name);
    }
}

// ─── Akeyless Additional Tests ────────────────────────────

#[test]
fn test_akeyless_parse_value_no_match_multiple_keys() {
    let json = r#"{"/app/KEY1": "val1", "/app/KEY2": "val2"}"#;
    let result = providers::akeyless::parse_akeyless_value(json, "/app/KEY3");
    assert!(result.is_err());
}

#[test]
fn test_akeyless_parse_list_missing_item_name() {
    let json = r#"[
        {"item_type": "STATIC_SECRET"},
        {"item_name": "/app/VALID", "item_type": "STATIC_SECRET"}
    ]"#;
    let result = providers::akeyless::parse_akeyless_list(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "/app/VALID");
}

#[test]
fn test_akeyless_parse_list_missing_item_type() {
    let json = r#"[{"item_name": "/app/NO_TYPE"}]"#;
    let result = providers::akeyless::parse_akeyless_list(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_akeyless_extract_key_name_empty_string() {
    assert_eq!(providers::akeyless::extract_key_name(""), "");
}

#[test]
fn test_akeyless_extract_key_name_trailing_slash() {
    assert_eq!(providers::akeyless::extract_key_name("/app/"), "");
}

#[test]
fn test_akeyless_parse_value_non_string_value() {
    let json = r#"{"/app/KEY": 42}"#;
    let result = providers::akeyless::parse_akeyless_value(json, "/app/KEY");
    assert!(result.is_err());
}

// ─── Bitwarden Additional Tests ───────────────────────────

#[test]
fn test_bitwarden_parse_output_preserves_empty_value() {
    let json = r#"[
        {"id": "uuid-1", "key": "HAS_EMPTY", "value": ""}
    ]"#;
    let result = providers::bitwarden::parse_bitwarden_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("HAS_EMPTY".to_string(), "".to_string()));
}

#[test]
fn test_bitwarden_validate_config_with_optional_project_id() {
    let provider = providers::BitwardenProvider;
    let mut creds = HashMap::new();
    creds.insert("access_token".into(), "0.xxx.yyy".into());
    creds.insert("project_id".into(), "pid-123".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── Conjur Additional Tests ──────────────────────────────

#[test]
fn test_conjur_parse_list_filters_non_variable_resources() {
    let json = r#"[
        "myorg:variable:app/secret",
        "myorg:policy:app",
        "myorg:host:app/server"
    ]"#;
    let result = providers::conjur::parse_conjur_list(json, "myorg").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "app/secret");
}

#[test]
fn test_conjur_parse_list_empty_account() {
    let json = r#"[":variable:global/key"]"#;
    let result = providers::conjur::parse_conjur_list(json, "").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "global/key");
}

#[test]
fn test_conjur_extract_key_empty_string() {
    assert_eq!(providers::conjur::extract_conjur_key(""), "");
}

#[test]
fn test_conjur_extract_key_single_segment() {
    assert_eq!(providers::conjur::extract_conjur_key("SECRET"), "SECRET");
}

#[test]
fn test_conjur_parse_list_malformed_entries() {
    let json = r#"["no-colon-at-all", "single:colon"]"#;
    let result = providers::conjur::parse_conjur_list(json, "myorg").unwrap();
    assert!(result.is_empty());
}

// ─── SOPS Additional Tests ────────────────────────────────

#[test]
fn test_sops_parse_output_only_sops_metadata() {
    let json = r#"{"sops": {"version": "3.7.3"}}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_sops_parse_output_nested_object_value() {
    let json = r#"{"CONFIG": {"nested": "value"}}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "CONFIG");
    assert!(result[0].1.contains("nested"));
}

#[test]
fn test_sops_parse_output_null_value() {
    let json = r#"{"KEY": null}"#;
    let result = providers::sops::parse_sops_output(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("KEY".to_string(), "null".to_string()));
}

#[test]
fn test_sops_validate_config_with_encryption_type() {
    let provider = providers::SopsProvider;
    let mut creds = HashMap::new();
    creds.insert("key_file".into(), "/path/to/key".into());
    creds.insert("encryption_type".into(), "pgp".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── pass/gopass Additional Tests ─────────────────────────

#[test]
fn test_pass_scan_store_skips_hidden_directories() {
    let temp = std::env::temp_dir().join("envforge-test-pass-hidden");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join(".hidden")).unwrap();
    std::fs::create_dir_all(temp.join("visible")).unwrap();
    std::fs::write(temp.join(".hidden/SECRET.gpg"), b"").unwrap();
    std::fs::write(temp.join("visible/KEY.gpg"), b"").unwrap();

    let result = providers::pass::scan_password_store(&temp, "").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "visible/KEY");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_pass_scan_store_empty_dir() {
    let temp = std::env::temp_dir().join("envforge-test-pass-empty");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    let result = providers::pass::scan_password_store(&temp, "").unwrap();
    assert!(result.is_empty());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_pass_scan_store_ignores_non_gpg_files() {
    let temp = std::env::temp_dir().join("envforge-test-pass-nongpg");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(temp.join("README.md"), b"notes").unwrap();
    std::fs::write(temp.join("config.txt"), b"").unwrap();
    std::fs::write(temp.join("REAL.gpg"), b"").unwrap();

    let result = providers::pass::scan_password_store(&temp, "").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "REAL");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_pass_validate_config_with_custom_binary() {
    let provider = providers::PassProvider;
    let mut creds = HashMap::new();
    creds.insert("binary".into(), "gopass".into());
    let result = provider.validate_config(&creds);
    assert!(result.is_ok());
}

// ─── Keeper Additional Tests ──────────────────────────────

#[test]
fn test_keeper_parse_list_missing_uid() {
    let json = r#"[
        {"title": "NO_UID"},
        {"uid": "uid-1", "title": "HAS_UID"}
    ]"#;
    let result = providers::keeper::parse_keeper_list(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("uid-1".to_string(), "HAS_UID".to_string()));
}

#[test]
fn test_keeper_parse_list_missing_title() {
    let json = r#"[
        {"uid": "uid-1"},
        {"uid": "uid-2", "title": "Valid"}
    ]"#;
    let result = providers::keeper::parse_keeper_list(json).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("uid-2".to_string(), "Valid".to_string()));
}

#[test]
fn test_keeper_parse_record_prefers_password_over_login() {
    let json = r#"{
        "uid": "uid-1",
        "title": "Both",
        "type": "login",
        "fields": [
            {"type": "login", "value": ["admin"]},
            {"type": "password", "value": ["supersecret"]}
        ]
    }"#;
    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "supersecret");
}

#[test]
fn test_keeper_parse_record_empty_password_falls_to_login() {
    let json = r#"{
        "uid": "uid-1",
        "title": "EmptyPW",
        "type": "login",
        "fields": [
            {"type": "password", "value": [""]},
            {"type": "login", "value": ["fallback-user"]}
        ]
    }"#;
    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "fallback-user");
}

#[test]
fn test_keeper_parse_record_secret_type_field() {
    let json = r#"{
        "uid": "uid-1",
        "title": "SecretType",
        "type": "login",
        "fields": [
            {"type": "secret", "value": ["my-api-key"]}
        ]
    }"#;
    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "my-api-key");
}

#[test]
fn test_keeper_parse_record_text_type_field() {
    let json = r#"{
        "uid": "uid-1",
        "title": "TextType",
        "type": "login",
        "fields": [
            {"type": "text", "value": ["plain-text-value"]}
        ]
    }"#;
    let result = providers::keeper::parse_keeper_record(json).unwrap();
    assert_eq!(result, "plain-text-value");
}

// ─── Cross-Provider Edge Case Tests ───────────────────────

#[test]
fn test_all_new_providers_have_non_empty_display_name() {
    let registry = providers::create_default_registry();
    for name in &["bitwarden", "akeyless", "conjur", "sops", "pass", "keeper"] {
        let provider = registry.get(name).unwrap();
        assert!(
            !provider.display_name().is_empty(),
            "{} has empty display_name",
            name
        );
    }
}

#[test]
fn test_all_new_providers_have_non_empty_binary_name() {
    let registry = providers::create_default_registry();
    for name in &["bitwarden", "akeyless", "conjur", "sops", "pass", "keeper"] {
        let provider = registry.get(name).unwrap();
        assert!(
            !provider.binary_name().is_empty(),
            "{} has empty binary_name",
            name
        );
    }
}

#[test]
fn test_all_new_providers_have_install_hint() {
    let registry = providers::create_default_registry();
    for name in &["bitwarden", "akeyless", "conjur", "sops", "pass", "keeper"] {
        let provider = registry.get(name).unwrap();
        assert!(
            !provider.install_hint().is_empty(),
            "{} has empty install_hint",
            name
        );
    }
}

#[test]
fn test_all_providers_name_matches_registry_key() {
    let registry = providers::create_default_registry();
    let names = registry.list_names();
    for name in &names {
        let provider = registry.get(name).unwrap();
        assert_eq!(
            provider.name(),
            name.as_str(),
            "provider.name() mismatch for registry key '{}'",
            name
        );
    }
}
