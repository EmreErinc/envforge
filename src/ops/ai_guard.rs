// ─── AI Guard — invoked by AI tool hooks ──────────────────

/// Patterns that indicate a sensitive file path.
const SENSITIVE_PATTERNS: &[&str] = &[
    ".env",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    ".ssh/",
    ".aws/",
    ".gnupg/",
    "credentials",
    "secret",
    "token",
    "id_rsa",
    "id_ed25519",
];

/// Filename suffixes that are safe even though they contain ".env".
const SAFE_ENV_SUFFIXES: &[&str] = &[
    ".env.schema",
    ".env.example",
    ".env.sample",
    ".env.template",
    ".env.ai.md",
];

/// Guard execution stage.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardStage {
    PreTool,
    PostTool,
}

/// Result of a guard check.
#[derive(Debug)]
pub struct GuardResult {
    pub blocked: bool,
    pub warnings: Vec<String>,
}

/// Check whether a file path looks sensitive.
pub fn is_sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();

    // Allow safe .env suffixes through
    for safe in SAFE_ENV_SUFFIXES {
        if lower.ends_with(safe) {
            return false;
        }
    }

    SENSITIVE_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Run the AI guard check.
///
/// `known_secrets` is a list of (key, value) pairs of sensitive env vars.
/// Only values with length >= 8 should be passed in.
pub fn run_guard(
    stage: GuardStage,
    tool_name: &str,
    tool_input: Option<&str>,
    known_secrets: &[(String, String)],
) -> GuardResult {
    let mut warnings = Vec::new();

    match stage {
        GuardStage::PreTool => {
            // 1. Sensitive file access alert
            if tool_name == "Read" {
                if let Some(input) = tool_input {
                    // tool_input is JSON; try to extract file_path, fall back to raw string
                    let path = extract_path_from_input(input);
                    if is_sensitive_path(&path) {
                        warnings.push(format!(
                            "\u{26a0} EnvForge: AI agent accessing sensitive file: {}",
                            path
                        ));
                    }
                }
            }

            // 2. Secret value in Bash command input
            if tool_name == "Bash" {
                if let Some(input) = tool_input {
                    for (key, value) in known_secrets {
                        if value.len() >= 8 && input.contains(value.as_str()) {
                            warnings.push(format!(
                                "\u{26a0} EnvForge: Secret value detected in command input (key: {})",
                                key
                            ));
                            break; // one warning is enough
                        }
                    }
                }
            }
        }
        GuardStage::PostTool => {
            // 1. Secret value in tool output
            if let Some(input) = tool_input {
                for (key, value) in known_secrets {
                    if value.len() >= 8 && input.contains(value.as_str()) {
                        warnings.push(format!(
                            "\u{26a0} EnvForge: Secret value detected in tool output (key: {})",
                            key
                        ));
                        break;
                    }
                }
            }
        }
    }

    GuardResult {
        blocked: false, // advisory only
        warnings,
    }
}

/// Try to extract a file path from JSON tool input, fall back to raw string.
fn extract_path_from_input(input: &str) -> String {
    // Try JSON parse first — Claude Code sends {"file_path": "..."} for Read
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(input) {
        if let Some(fp) = val.get("file_path").and_then(|v| v.as_str()) {
            return fp.to_string();
        }
        // Also check "path" key
        if let Some(fp) = val.get("path").and_then(|v| v.as_str()) {
            return fp.to_string();
        }
    }
    // Fall back to raw string (trimmed)
    input.trim().to_string()
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── is_sensitive_path ─────────────────────────────────

    #[test]
    fn test_sensitive_path_detects_dotenv() {
        assert!(is_sensitive_path(".env"));
        assert!(is_sensitive_path("/home/user/project/.env"));
        assert!(is_sensitive_path(".env.production"));
        assert!(is_sensitive_path(".env.local"));
    }

    #[test]
    fn test_sensitive_path_detects_key_files() {
        assert!(is_sensitive_path("server.pem"));
        assert!(is_sensitive_path("/etc/ssl/private/cert.key"));
        assert!(is_sensitive_path("client.p12"));
        assert!(is_sensitive_path("cert.pfx"));
    }

    #[test]
    fn test_sensitive_path_detects_ssh_aws() {
        assert!(is_sensitive_path("/home/user/.ssh/id_rsa"));
        assert!(is_sensitive_path("~/.ssh/config"));
        assert!(is_sensitive_path("~/.aws/credentials"));
        assert!(is_sensitive_path(".gnupg/private-keys"));
    }

    #[test]
    fn test_sensitive_path_detects_credentials() {
        assert!(is_sensitive_path("credentials.toml"));
        assert!(is_sensitive_path("secret-sources.toml"));
        assert!(is_sensitive_path("token.json"));
        assert!(is_sensitive_path("id_rsa"));
        assert!(is_sensitive_path("id_ed25519"));
    }

    #[test]
    fn test_sensitive_path_skips_safe_env_files() {
        assert!(!is_sensitive_path(".env.schema"));
        assert!(!is_sensitive_path(".env.example"));
        assert!(!is_sensitive_path(".env.sample"));
        assert!(!is_sensitive_path(".env.template"));
        assert!(!is_sensitive_path(".env.ai.md"));
        assert!(!is_sensitive_path("/project/.env.schema"));
        assert!(!is_sensitive_path("/project/.env.example"));
    }

    #[test]
    fn test_sensitive_path_case_insensitive() {
        assert!(is_sensitive_path(".ENV"));
        assert!(is_sensitive_path("SERVER.PEM"));
        assert!(is_sensitive_path(".SSH/id_rsa"));
    }

    #[test]
    fn test_sensitive_path_normal_files_pass() {
        assert!(!is_sensitive_path("src/main.rs"));
        assert!(!is_sensitive_path("README.md"));
        assert!(!is_sensitive_path("package.json"));
        assert!(!is_sensitive_path("Cargo.toml"));
    }

    // ─── PreTool guard ─────────────────────────────────────

    #[test]
    fn test_pre_tool_catches_sensitive_file_read() {
        let secrets = vec![];
        let result = run_guard(
            GuardStage::PreTool,
            "Read",
            Some("/home/user/.env"),
            &secrets,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("sensitive file"));
        assert!(!result.blocked);
    }

    #[test]
    fn test_pre_tool_catches_sensitive_file_read_json_input() {
        let secrets = vec![];
        let input = r#"{"file_path": "/home/user/.ssh/id_rsa"}"#;
        let result = run_guard(GuardStage::PreTool, "Read", Some(input), &secrets);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("sensitive file"));
        assert!(result.warnings[0].contains(".ssh/id_rsa"));
    }

    #[test]
    fn test_pre_tool_no_warning_for_normal_read() {
        let secrets = vec![];
        let result = run_guard(
            GuardStage::PreTool,
            "Read",
            Some("src/main.rs"),
            &secrets,
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_pre_tool_catches_secret_in_bash_command() {
        let secrets = vec![
            ("API_KEY".to_string(), "super-secret-api-key-12345".to_string()),
        ];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("curl -H 'Authorization: Bearer super-secret-api-key-12345' https://api.example.com"),
            &secrets,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("Secret value detected in command input"));
    }

    #[test]
    fn test_pre_tool_no_warning_for_safe_bash() {
        let secrets = vec![
            ("API_KEY".to_string(), "super-secret-api-key-12345".to_string()),
        ];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("ls -la"),
            &secrets,
        );
        assert!(result.warnings.is_empty());
    }

    // ─── PostTool guard ────────────────────────────────────

    #[test]
    fn test_post_tool_catches_secret_in_output() {
        let secrets = vec![
            ("DB_PASSWORD".to_string(), "my-database-password-xyz".to_string()),
        ];
        let result = run_guard(
            GuardStage::PostTool,
            "Bash",
            Some("Connection string: postgres://user:my-database-password-xyz@localhost"),
            &secrets,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("Secret value detected in tool output"));
    }

    #[test]
    fn test_post_tool_no_warning_without_secrets() {
        let secrets = vec![
            ("API_KEY".to_string(), "long-secret-value-here".to_string()),
        ];
        let result = run_guard(
            GuardStage::PostTool,
            "Bash",
            Some("Command completed successfully"),
            &secrets,
        );
        assert!(result.warnings.is_empty());
    }

    // ─── Short secret values are skipped ───────────────────

    #[test]
    fn test_guard_skips_short_secret_values() {
        // Values < 8 chars should be filtered out before calling run_guard,
        // but even if passed, the guard checks length >= 8
        let secrets = vec![
            ("SHORT".to_string(), "abc".to_string()),
        ];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("echo abc"),
            &secrets,
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_guard_catches_exactly_8_char_secret() {
        let secrets = vec![
            ("TOKEN".to_string(), "12345678".to_string()),
        ];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("echo 12345678"),
            &secrets,
        );
        assert_eq!(result.warnings.len(), 1);
    }

    // ─── extract_path_from_input ───────────────────────────

    #[test]
    fn test_extract_path_json() {
        let input = r#"{"file_path": "/foo/bar.txt"}"#;
        assert_eq!(extract_path_from_input(input), "/foo/bar.txt");
    }

    #[test]
    fn test_extract_path_plain_string() {
        assert_eq!(extract_path_from_input("/foo/bar.txt"), "/foo/bar.txt");
    }

    #[test]
    fn test_extract_path_json_with_path_key() {
        let input = r#"{"path": "/some/file.rs"}"#;
        assert_eq!(extract_path_from_input(input), "/some/file.rs");
    }
}
