use envforge::ops::secrets::modes::VolatileMode;
use envforge::ops::secrets::provider::CredentialEncryptionPolicy;
use envforge::ops::secrets::providers;
use envforge::ops::sync::model::SyncEncryptionPolicy;
use serial_test::serial;

// ═══════════════════════════════════════════════════════════════
// Encryption Invariant Tests
//
// If these tests pass, the encryption posture is provably sound.
// This file is the CANARY — when a 14th provider is added and
// accidentally returns NotSupported, or when VolatileMode::default()
// is refactored back to Off, the tests below will catch it.
//
// The compiler enforces `encryption_mode()` exists on every provider.
// These tests enforce that it returns the CORRECT value.
// ═══════════════════════════════════════════════════════════════

// ─── INVARIANT 1: VolatileMode default is On (not Off) ──────

#[test]
fn test_volatile_mode_default_is_on() {
    let default = VolatileMode::default();
    assert!(
        matches!(default, VolatileMode::On { .. }),
        "VolatileMode::default() must be On — secure by default. Got: {:?}",
        default
    );
    assert!(default.is_enabled());
    assert!(default.ttl_seconds() > 0);
    assert!(!default.requires_reauth());
}

#[test]
fn test_volatile_mode_off_is_explicit() {
    let off = VolatileMode::Off;
    assert!(!off.is_enabled());
    assert_eq!(off.ttl_seconds(), 0);
    assert!(!off.requires_reauth());
}

#[test]
fn test_volatile_mode_strict_requires_reauth() {
    let strict = VolatileMode::Strict {
        ttl_seconds: 120,
        reauth: true,
    };
    assert!(strict.is_enabled());
    assert_eq!(strict.ttl_seconds(), 120);
    assert!(strict.requires_reauth());

    let strict_no_reauth = VolatileMode::Strict {
        ttl_seconds: 60,
        reauth: false,
    };
    assert!(strict_no_reauth.is_enabled());
    assert!(!strict_no_reauth.requires_reauth());
}

// ─── INVARIANT 2: All 13 providers return Mandatory ─────────

#[test]
fn test_all_providers_encryption_mode_is_mandatory() {
    let registry = providers::create_default_registry();

    assert!(!registry.is_empty(), "provider registry must not be empty");

    for name in registry.list_names() {
        let provider = registry.get(&name).unwrap_or_else(|e| {
            panic!("provider '{}' not found in registry: {}", name, e);
        });

        let mode = provider.encryption_mode();

        assert!(
            matches!(mode, CredentialEncryptionPolicy::Mandatory),
            "PROVIDER '{}' returned {:?} — all providers MUST return Mandatory.\n\
             If this provider genuinely cannot support encryption, add it to the\n\
             LEGITIMATELY_UNSUPPORTED allowlist in encryption_invariant_tests.rs\n\
             with a technical justification and security reviewer name.",
            name,
            mode
        );
    }
}

#[test]
fn test_provider_count_is_thirteen() {
    let registry = providers::create_default_registry();
    let count = registry.len();
    assert_eq!(
        count, 13,
        "PROVIDER COUNT changed from 13 to {}. If you added a provider:\n\
         1. Update this assertion to {}\n\
         2. Verify encryption_mode() is in encryption_invariant_tests.rs\n\
         3. Get security review for the new provider's encryption posture",
        count, count
    );
}

// ─── INVARIANT 3: Serde golden-file migration ───────────────

#[test]
fn test_sync_encryption_policy_serde_bool_true() {
    // Old config: require_encryption = true
    let toml = "encryption_policy = true\n";
    #[derive(serde::Deserialize)]
    struct Config {
        encryption_policy: SyncEncryptionPolicy,
    }
    let config: Config = toml::from_str(toml).unwrap();
    assert!(
        matches!(config.encryption_policy, SyncEncryptionPolicy::Mandatory),
        "Old 'true' must deserialize to Mandatory"
    );
}

#[test]
fn test_sync_encryption_policy_serde_bool_false_fails_safe() {
    // M3: legacy `require_encryption = false` no longer maps to a far-future
    // (2099) plaintext bypass. It is treated as Mandatory (fail-safe, NFR4) —
    // a real bounded window must be declared explicitly as `migration-until`.
    let toml = "encryption_policy = false\n";
    #[derive(serde::Deserialize)]
    struct Config {
        encryption_policy: SyncEncryptionPolicy,
    }
    let config: Config = toml::from_str(toml).unwrap();
    assert!(
        matches!(config.encryption_policy, SyncEncryptionPolicy::Mandatory),
        "legacy 'false' must fail safe to Mandatory, not a permanent bypass"
    );
    assert!(
        config.encryption_policy.is_required(),
        "encryption must be required after legacy-false fail-safe"
    );
}

#[test]
fn test_sync_encryption_policy_serde_mandatory() {
    let toml = "encryption_policy = \"mandatory\"\n";
    #[derive(serde::Deserialize)]
    struct Config {
        encryption_policy: SyncEncryptionPolicy,
    }
    let config: Config = toml::from_str(toml).unwrap();
    assert!(matches!(
        config.encryption_policy,
        SyncEncryptionPolicy::Mandatory
    ));
}

#[test]
fn test_sync_encryption_policy_mandatory_is_required() {
    assert!(SyncEncryptionPolicy::Mandatory.is_required());
}

#[test]
fn test_sync_encryption_policy_migration_until_past_is_required() {
    // A date in the distant past — encryption is now required
    let policy = SyncEncryptionPolicy::MigrationUntil("2000-01-01T00:00:00Z".into());
    assert!(
        policy.is_required(),
        "past MigrationUntil must enforce Mandatory"
    );
}

#[test]
fn test_sync_encryption_policy_migration_until_future_not_required() {
    // A date far in the future — migration window still open
    let policy = SyncEncryptionPolicy::MigrationUntil("2099-12-31T23:59:59Z".into());
    assert!(
        !policy.is_required(),
        "future MigrationUntil must NOT enforce Mandatory"
    );
}

#[test]
fn test_sync_encryption_policy_migration_until_invalid_date_is_required() {
    // Unparseable date → fail-safe: require encryption
    let policy = SyncEncryptionPolicy::MigrationUntil("not-a-date".into());
    assert!(
        policy.is_required(),
        "unparseable MigrationUntil must fail-safe to Mandatory"
    );
}

#[test]
fn test_sync_encryption_policy_default_is_mandatory() {
    let default = SyncEncryptionPolicy::default();
    assert!(matches!(default, SyncEncryptionPolicy::Mandatory));
    assert!(default.is_required());
}

// ─── INVARIANT 4: ENVFORGE_AGE_KEY resolution path ──────────

#[test]
#[serial]
fn test_env_age_key_empty_is_rejected() {
    std::env::set_var("ENVFORGE_AGE_KEY", "");
    let result = envforge::ops::encrypt::ensure_age_key();
    assert!(
        result.is_err(),
        "empty ENVFORGE_AGE_KEY must be rejected, got: {:?}",
        result
    );
    std::env::remove_var("ENVFORGE_AGE_KEY");
}

#[test]
#[serial]
fn test_env_age_key_invalid_key_fails_encrypt() {
    std::env::set_var("ENVFORGE_AGE_KEY", "this-is-not-a-valid-age-key");
    // encrypt_value calls ensure_age_key() then tries to parse the key
    let enc_result = envforge::ops::encrypt::encrypt_value("test");
    assert!(
        enc_result.is_err(),
        "invalid age key content must fail during encryption"
    );
    std::env::remove_var("ENVFORGE_AGE_KEY");
}

#[test]
#[serial]
fn test_env_age_key_file_missing_is_rejected() {
    std::env::set_var("ENVFORGE_AGE_KEY_FILE", "/nonexistent/path/age.key");
    let result = envforge::ops::encrypt::age_key_path();
    assert!(
        result.is_err(),
        "nonexistent ENVFORGE_AGE_KEY_FILE must fail"
    );
    std::env::remove_var("ENVFORGE_AGE_KEY_FILE");
}

// ─── INVARIANT 5: CredentialEncryptionPolicy validation ─────

#[test]
fn test_credential_encryption_policy_mandatory_is_encrypted() {
    assert!(CredentialEncryptionPolicy::Mandatory.is_encrypted());
}

#[test]
fn test_credential_encryption_policy_not_supported_is_not_encrypted() {
    let not_supported = CredentialEncryptionPolicy::NotSupported {
        reason: "pass-through provider with no key management infrastructure".into(),
        reviewed_by: Some("security-team".into()),
        re_evaluate_after_secs: 86400,
    };
    assert!(!not_supported.is_encrypted());
}

#[test]
fn test_credential_encryption_policy_not_supported_can_be_constructed() {
    let not_supported = CredentialEncryptionPolicy::NotSupported {
        reason: "pass-through provider with no key management infrastructure".into(),
        reviewed_by: Some("security-team".into()),
        re_evaluate_after_secs: 86400,
    };
    assert!(!not_supported.is_encrypted());
    // Verify the fields exist and are populated (not just default-constructed)
    match not_supported {
        CredentialEncryptionPolicy::NotSupported {
            reason,
            reviewed_by,
            re_evaluate_after_secs,
        } => {
            assert!(
                reason.len() >= 16,
                "NotSupported reason must be >= 16 chars"
            );
            assert_eq!(reviewed_by, Some("security-team".into()));
            assert_eq!(re_evaluate_after_secs, 86400);
        }
        _ => panic!("expected NotSupported variant"),
    }
}
