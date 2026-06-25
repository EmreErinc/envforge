//! Coverage for `ops::ai_guard::is_sensitive_path` — the gate that blocks an AI
//! agent's Read tool on secret-bearing files while letting safe `.env.*`
//! template variants through.

use envforge::ops::ai_guard::is_sensitive_path;

#[test]
fn test_sensitive_paths_blocked() {
    assert!(is_sensitive_path(".env"));
    assert!(is_sensitive_path("config/.env.local"));
    assert!(is_sensitive_path("certs/server.pem"));
    assert!(is_sensitive_path("deploy.key"));
    assert!(is_sensitive_path("/home/u/.ssh/id_rsa"));
    assert!(is_sensitive_path("/home/u/.ssh/id_ed25519"));
    assert!(is_sensitive_path("~/.aws/credentials"));
    assert!(is_sensitive_path("my_secret_store"));
    assert!(is_sensitive_path("auth_token.txt"));
}

#[test]
fn test_safe_env_variants_allowed() {
    assert!(!is_sensitive_path(".env.schema"));
    assert!(!is_sensitive_path(".env.example"));
    assert!(!is_sensitive_path(".env.sample"));
    assert!(!is_sensitive_path(".env.template"));
    assert!(!is_sensitive_path(".env.ai.md"));
}

#[test]
fn test_non_sensitive_paths_allowed() {
    assert!(!is_sensitive_path("src/main.rs"));
    assert!(!is_sensitive_path("README.md"));
    assert!(!is_sensitive_path("Cargo.toml"));
}

#[test]
fn test_case_insensitive_matching() {
    assert!(is_sensitive_path(".ENV"));
    assert!(is_sensitive_path("Secrets.json"));
    assert!(is_sensitive_path("ID_RSA"));
}
