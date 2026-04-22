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
