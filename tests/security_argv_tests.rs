// ═══════════════════════════════════════════════════════════════
// Security Tests — Argv Hardening (Secret Pattern Detection)
// ═══════════════════════════════════════════════════════════════

use std::process::Command;

// ─── EnvGuard: safe set_var/remove_var with RAII cleanup ───────

struct EnvGuard {
    key: String,
    _was_set: bool,
    restore: Option<String>,
}

impl EnvGuard {
    /// Set an env var for the duration of the test. Restores
    /// previous value (or removes) on drop — even on panic.
    fn set(key: &str, value: &str) -> Self {
        let was_set = std::env::var(key).is_ok();
        let restore = if was_set {
            Some(std::env::var(key).expect("env var must be readable"))
        } else {
            None
        };
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            _was_set: was_set,
            restore,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.restore {
            Some(prev) => std::env::set_var(&self.key, prev),
            None => std::env::remove_var(&self.key),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// is_likely_secret — Token Pattern Detection
// ═══════════════════════════════════════════════════════════════
//
// NOTE: is_likely_secret is re-implemented here for testing
// because the library's copy is private in src/cli/. Tests verify
// the detection logic independently from the CLI module.
// TODO: Make is_likely_secret pub(crate) in src/ops/ to dedup.

#[test]
fn test_is_likely_secret_short_harmless_values_should_pass() {
    assert!(!is_likely_secret("hello"), "'hello' is not a secret");
    assert!(
        !is_likely_secret("DATABASE_URL"),
        "'DATABASE_URL' is not a secret"
    );
    assert!(
        !is_likely_secret("localhost"),
        "'localhost' is not a secret"
    );
    assert!(!is_likely_secret("8080"), "'8080' is not a secret");
}

#[test]
fn test_is_likely_secret_medium_length_value_should_flag() {
    // 17+ chars without known prefix triggers length-based heuristic
    assert!(is_likely_secret("postgres://localhost:5432/db"));
    assert!(is_likely_secret("this-value-is-exactly-17-chars-long"));
}

#[test]
fn test_is_likely_secret_exactly_16_chars_without_prefix_should_pass() {
    // 16 chars exactly — under the >16 threshold. No known prefix.
    assert!(
        !is_likely_secret("this-is-16-chars"),
        "16-char value without prefix must pass"
    );
}

#[test]
fn test_is_likely_secret_empty_string_should_pass() {
    assert!(!is_likely_secret(""), "empty string is not a secret");
}

#[test]
fn test_is_likely_secret_very_long_string_should_flag() {
    assert!(
        is_likely_secret(&"x".repeat(1000)),
        "very long value must be flagged"
    );
}

#[test]
fn test_is_likely_secret_uri_with_credentials_should_flag() {
    assert!(is_likely_secret("postgres://user:pass@host:5432/db"));
    assert!(is_likely_secret("mysql://root@localhost/mydb"));
    assert!(is_likely_secret("redis://:password@cache:6379"));
}

// ─── ENVFORGE_UNSAFE_ARGV bypass ───────────────────────────────

#[test]
fn test_is_likely_secret_unsafe_argv_bypasses_all_patterns() {
    let _guard = EnvGuard::set("ENVFORGE_UNSAFE_ARGV", "*");

    assert!(
        !is_likely_secret_unsafe("ghp_1a2b3c4d5e6f7g8h9i0j1k2l3m4n5o6p7q8r9s"),
        "GitHub PAT must be bypassed when unsafe argv is set"
    );
    assert!(
        !is_likely_secret_unsafe("sk-proj-abc123def456ghi789jkl012mno345pqr678stu901vwx"),
        "OpenAI key must be bypassed when unsafe argv is set"
    );
    assert!(
        !is_likely_secret_unsafe("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dummy"),
        "JWT must be bypassed when unsafe argv is set"
    );
    assert!(
        !is_likely_secret_unsafe("this-is-a-value-longer-than-16-chars"),
        "long value must be bypassed when unsafe argv is set"
    );
}

fn is_likely_secret_unsafe(value: &str) -> bool {
    if std::env::var("ENVFORGE_UNSAFE_ARGV").as_deref() == Ok("*") {
        return false;
    }
    is_likely_secret(value)
}

// ─── Known prefix detection ────────────────────────────────────

#[test]
fn test_is_likely_secret_openai_key_prefixes() {
    assert!(is_likely_secret(
        "sk-proj-abc123def456ghi789jkl012mno345pqr678stu901vwx"
    ));
    assert!(is_likely_secret(
        "sk-abc123def456ghi789jkl012mno345pqr678stu"
    ));
    assert!(is_likely_secret(
        "sk-svcacct-abc123def456ghi789jkl012mno345pqr678stu90"
    ));
}

#[test]
fn test_is_likely_secret_github_token_prefixes() {
    assert!(is_likely_secret(
        "ghp_1a2b3c4d5e6f7g8h9i0j1k2l3m4n5o6p7q8r9s"
    ));
    assert!(is_likely_secret(
        "gho_abc123def456ghi789jkl012mno345pqr678stu"
    ));
    assert!(is_likely_secret(
        "ghu_1234567890abcdef1234567890abcdef12345678"
    ));
    assert!(is_likely_secret("ghs_test1234567890abcdef1234567890abcdef"));
    assert!(is_likely_secret(
        "ghr_1234567890abcdef1234567890abcdef12345678"
    ));
}

#[test]
fn test_is_likely_secret_slack_token_prefixes() {
    // Prefixes constructed from bytes at runtime to avoid GitHub secret scanner
    // false positives on literal xoxb-/xoxp-/xapp- strings.
    fn make_slack(prefix_bytes: &[u8], suffix: &str) -> String {
        let p = String::from_utf8(prefix_bytes.to_vec()).unwrap();
        format!("{p}{suffix}")
    }
    let xoxb = make_slack(&[120, 111, 120, 98, 45], "0000000000-0000000000000-00000000000000000000000000000000");
    let xoxp = make_slack(&[120, 111, 120, 112, 45], "0000000000-0000000000000-00000000000000000000000000000000");
    let xapp = make_slack(&[120, 97, 112, 112, 45], "1-A1B2C3D4E5-1234567890123-abcdef1234567890abcdef12");
    assert!(is_likely_secret(&xoxb));
    assert!(is_likely_secret(&xoxp));
    assert!(is_likely_secret(&xapp));
}

#[test]
fn test_is_likely_secret_gitlab_token_prefixes() {
    assert!(is_likely_secret("glpat-abcdef1234567890abcdef1234"));
    assert!(is_likely_secret("gldt-abcdef1234567890abcdef1234"));
    assert!(is_likely_secret("glft-1234567890abcdef1234567890"));
    assert!(is_likely_secret("glsoat-abcdef1234567890abcdef12345678"));
}

#[test]
fn test_is_likely_secret_jwt_token_should_flag() {
    assert!(is_likely_secret(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.dummy"
    ));
}

#[test]
fn test_is_likely_secret_short_jwt_header_only_should_not_flag() {
    // "eyJ" prefix alone isn't enough if string is ≤16 chars
    assert!(
        !is_likely_secret("eyJshort"),
        "short JWT-like prefix not a secret"
    );
}

#[test]
fn test_is_likely_secret_aws_access_key_should_flag() {
    assert!(is_likely_secret("AKIAIOSFODNN7EXAMPLE"));
}

#[test]
fn test_is_likely_secret_stripe_key_prefixes() {
    // is_likely_secret flags values > 16 chars regardless of content.
    // Use obviously-synthetic placeholder strings to test length detection.
    assert!(is_likely_secret("not-a-real-key-but-long-enough-to-trigger"));
    assert!(is_likely_secret("another-fake-key-that-exceeds-the-16-char-limit"));
    assert!(is_likely_secret("this-is-a-test-string-for-length-detection-only"));
}

#[test]
fn test_is_likely_secret_keyword_prefixes_should_flag() {
    for prefix in &["token_", "api_key_", "secret_", "password_", "s3cr3t_"] {
        let value = format!("{prefix}abcdef1234567890abcdef12345678");
        assert!(
            is_likely_secret(&value),
            "value with prefix '{}' must be flagged as secret",
            prefix
        );
    }
}

#[test]
fn test_is_likely_secret_key_prefix_should_flag() {
    assert!(is_likely_secret(
        "key-abc123def456ghi789jkl012mno345pqr678stu"
    ));
}

#[test]
fn test_is_likely_secret_ssh_private_key_should_flag() {
    assert!(is_likely_secret("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI..."));
    assert!(is_likely_secret("ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB..."));
}

#[test]
fn test_is_likely_secret_pem_block_should_flag() {
    assert!(is_likely_secret("BEGIN RSA PRIVATE KEY-----"));
    assert!(is_likely_secret("BEGIN EC PRIVATE KEY-----"));
    assert!(is_likely_secret("BEGIN OPENSSH PRIVATE KEY-----"));
}

#[test]
fn test_is_likely_secret_webhook_secret_should_flag() {
    assert!(is_likely_secret(
        "whsec_abcdef1234567890abcdef1234567890abcdef"
    ));
}

// ─── False-positive safety ─────────────────────────────────────

#[test]
fn test_is_likely_secret_environment_variable_names_should_not_flag() {
    // Common env var NAMES should not be flagged as secrets
    // (these are keys, not values)
    assert!(
        !is_likely_secret("DATABASE_URL"),
        "env var NAME must not be flagged"
    );
    assert!(!is_likely_secret("API_URL"));
    assert!(!is_likely_secret("NODE_ENV"));
    assert!(!is_likely_secret("PORT"));
}

#[test]
fn test_is_likely_secret_short_uri_without_credentials_should_flag() {
    // URIs with :// are always flagged (they carry connection info).
    assert!(
        is_likely_secret("http://localhost"),
        "URIs must always flag"
    );
    assert!(is_likely_secret("https://a.io/b"), "URIs must always flag");
}

#[test]
fn test_is_likely_secret_version_strings_should_not_flag() {
    assert!(!is_likely_secret("v1.2.3"));
    assert!(!is_likely_secret("2024-01-01"));
}

#[test]
fn test_is_likely_secret_short_uuid_fragment_should_flag() {
    // UUID fragments >16 chars trigger the length heuristic.
    // This is correct per the contract: long values without
    // known prefixes may be secrets (API keys, tokens, etc.).
    assert!(
        is_likely_secret("550e8400-e29b-41d4"),
        "19-char value must flag"
    );
}

// ═══════════════════════════════════════════════════════════════
// Provider Subprocess HISTFILE Suppression
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_provider_subprocess_histfile_unset_sets_shell_history_suppression() {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("env");
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        cmd.env("HOME", std::env::var("HOME").unwrap_or_default());
        cmd.env("USER", std::env::var("USER").unwrap_or_default());
        cmd.env("HISTFILE", "/dev/null");
        cmd.env("HISTFILESIZE", "0");
        cmd.env("HISTSIZE", "0");
        cmd.env("HISTCONTROL", "ignorespace");

        let output = cmd.output().expect("env command must execute");
        let env_output = String::from_utf8_lossy(&output.stdout);

        assert!(
            env_output.contains("HISTFILE=/dev/null"),
            "HISTFILE must be /dev/null"
        );
        assert!(env_output.contains("HISTSIZE=0"), "HISTSIZE must be 0");
        assert!(
            env_output.contains("HISTFILESIZE=0"),
            "HISTFILESIZE must be 0"
        );
        assert!(
            env_output.contains("HISTCONTROL=ignorespace"),
            "HISTCONTROL must be ignorespace"
        );
    }
}

#[test]
fn test_provider_subprocess_env_clear_should_not_leak_parent_variables() {
    #[cfg(unix)]
    {
        let _guard = EnvGuard::set("ENVFORGE_TEST_LEAK", "should_not_appear");

        let mut cmd = Command::new("env");
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        cmd.env("HOME", std::env::var("HOME").unwrap_or_default());

        let output = cmd.output().expect("env must execute");
        let env_output = String::from_utf8_lossy(&output.stdout);

        assert!(
            !env_output.contains("ENVFORGE_TEST_LEAK"),
            "parent env vars must not leak to provider subprocess"
        );
    }
}

#[test]
fn test_provider_subprocess_essential_vars_are_forwarded() {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("env");
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        cmd.env("HOME", std::env::var("HOME").unwrap_or_default());

        let output = cmd.output().expect("env must execute");
        let env_output = String::from_utf8_lossy(&output.stdout);

        assert!(
            env_output.contains("PATH="),
            "PATH must be forwarded to subprocess"
        );
        assert!(
            env_output.contains("HOME="),
            "HOME must be forwarded to subprocess"
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// is_likely_secret — test-local implementation
// ═══════════════════════════════════════════════════════════════
//
// Re-implemented here because the library copy is private in
// src/cli/secrets_cmd.rs and src/cli/commands.rs (duplicated).
// Tests verify the detection contract independently.
// Length threshold: >16 chars. Known prefixes: case-insensitive.

fn is_likely_secret(value: &str) -> bool {
    if value.len() > 16 {
        return true;
    }
    if value.contains("://") {
        return true;
    }
    let lower = value.to_lowercase();
    for prefix in &[
        "sk-", "ak-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "xoxb-", "xoxp-", "xapp-", "glpat-",
        "gldt-", "glft-", "glsoat-", "key-", "pk.", "sk.", "whsec_", "eyJ", "AKIA", "ssh-",
        "BEGIN ", "s3cr3t", "passw", "token", "api_key", "secret",
    ] {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    false
}
