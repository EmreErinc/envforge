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
    hardening: Option<&crate::ops::hardening::HardeningConfig>,
    scanner_findings: Option<&[crate::ops::external_scanner::ScannerFinding]>,
) -> GuardResult {
    let mut warnings = Vec::new();

    // Build the list of strings to scan for secrets
    let mut scan_strings: Vec<String> = Vec::new();
    if let Some(input) = tool_input {
        scan_strings.push(input.to_string());

        // Hardening: derive additional strings from adversarial input
        if let Some(config) = hardening {
            let hardener = crate::ops::hardening::HardenInput::new(config.clone());
            for derived in hardener.harden(input) {
                scan_strings.push(derived.text);
            }
        }
    }

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

            // 2. Secret value in Bash command input (scan original + hardened inputs)
            if tool_name == "Bash" {
                for scan_input in &scan_strings {
                    for (key, value) in known_secrets {
                        if value.len() >= 8 && scan_input.contains(value.as_str()) {
                            let source = if scan_input == tool_input.unwrap_or("") {
                                "command input"
                            } else {
                                "decoded input"
                            };
                            warnings.push(format!(
                                "\u{26a0} EnvForge: Secret value detected in {} (key: {})",
                                source, key
                            ));
                            break; // one warning is enough
                        }
                    }
                }
            }
        }
        GuardStage::PostTool => {
            // 1. Secret value in tool output (scan original + hardened inputs)
            for scan_input in &scan_strings {
                for (key, value) in known_secrets {
                    if value.len() >= 8 && scan_input.contains(value.as_str()) {
                        let source = if scan_input == tool_input.unwrap_or("") {
                            "tool output"
                        } else {
                            "decoded output"
                        };
                        warnings.push(format!(
                            "\u{26a0} EnvForge: Secret value detected in {} (key: {})",
                            source, key
                        ));
                        break;
                    }
                }
            }

            // 2. Canary value detection in tool output
            if let Some(input) = tool_input {
                if let Ok(canaries) = super::canary::load_canaries() {
                    for (canary_key, canary) in &canaries.canaries {
                        if input.contains(canary.fake_value.as_str()) {
                            warnings.push(format!(
                                "\u{1f6a8} EnvForge: CANARY SECRET detected in tool output (key: {})",
                                canary_key
                            ));
                            let _ = super::canary::trigger_canary(
                                canary_key,
                                "ai_guard",
                                "detected in tool output",
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    // External scanner findings (from new scanner pipeline)
    if let Some(findings) = scanner_findings {
        for finding in findings {
            for line in &finding.findings {
                warnings.push(format!(
                    "\u{26a0} EnvForge: Scanner '{}' finding: {}",
                    finding.scanner_name, line
                ));
            }
        }
    }

    // Legacy external scanner integration (deprecated, env var based)
    if scanner_findings.is_none() {
        if let Some(input) = tool_input {
            if let Some(findings) = run_external_scanner(input) {
                for finding in findings {
                    warnings.push(format!("\u{26a0} EnvForge: External scanner: {}", finding));
                }
            }
        }
    }

    GuardResult {
        blocked: false, // advisory only
        warnings,
    }
}

/// Run external scanner if configured via ENVFORGE_EXTERNAL_SCANNER env var.
///
/// The scanner receives a temp file path as argument. If it exits non-zero,
/// stdout+stderr lines are returned as findings. If it exits zero, no findings.
pub fn run_external_scanner(content: &str) -> Option<Vec<String>> {
    let scanner_cmd = match std::env::var("ENVFORGE_EXTERNAL_SCANNER") {
        Ok(cmd) if !cmd.is_empty() => cmd,
        _ => return None,
    };
    run_external_scanner_with_cmd(&scanner_cmd, content)
}

/// Run a specific scanner command on the given content.
/// Note: passes the temp file path to the scanner command via a shell.
/// The scanner command is user-configured via ENVFORGE_EXTERNAL_SCANNER env var.
/// The temp file path is system-generated and contains only safe characters.
fn run_external_scanner_with_cmd(scanner_cmd: &str, content: &str) -> Option<Vec<String>> {
    let tmp = tempfile::NamedTempFile::new().ok()?;
    std::fs::write(tmp.path(), content).ok()?;

    let output = std::process::Command::new("sh")
        .args(["-c", &format!("{} {}", scanner_cmd, tmp.path().display())])
        .output()
        .ok()?;

    if output.status.success() {
        None // No findings
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let findings: Vec<String> = format!("{}{}", stdout, stderr)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        if findings.is_empty() {
            None
        } else {
            Some(findings)
        }
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
            None,
            None,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("sensitive file"));
        assert!(!result.blocked);
    }

    #[test]
    fn test_pre_tool_catches_sensitive_file_read_json_input() {
        let secrets = vec![];
        let input = r#"{"file_path": "/home/user/.ssh/id_rsa"}"#;
        let result = run_guard(
            GuardStage::PreTool,
            "Read",
            Some(input),
            &secrets,
            None,
            None,
        );
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
            None,
            None,
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_pre_tool_catches_secret_in_bash_command() {
        let secrets = vec![(
            "API_KEY".to_string(),
            "super-secret-api-key-12345".to_string(),
        )];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("curl -H 'Authorization: Bearer super-secret-api-key-12345' https://api.example.com"),
            &secrets,
            None,
            None,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("Secret value detected in command input"));
    }

    #[test]
    fn test_pre_tool_no_warning_for_safe_bash() {
        let secrets = vec![(
            "API_KEY".to_string(),
            "super-secret-api-key-12345".to_string(),
        )];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("ls -la"),
            &secrets,
            None,
            None,
        );
        assert!(result.warnings.is_empty());
    }

    // ─── PostTool guard ────────────────────────────────────

    #[test]
    fn test_post_tool_catches_secret_in_output() {
        let secrets = vec![(
            "DB_PASSWORD".to_string(),
            "my-database-password-xyz".to_string(),
        )];
        let result = run_guard(
            GuardStage::PostTool,
            "Bash",
            Some("Connection string: postgres://user:my-database-password-xyz@localhost"),
            &secrets,
            None,
            None,
        );
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("Secret value detected in tool output"));
    }

    #[test]
    fn test_post_tool_no_warning_without_secrets() {
        let secrets = vec![("API_KEY".to_string(), "long-secret-value-here".to_string())];
        let result = run_guard(
            GuardStage::PostTool,
            "Bash",
            Some("Command completed successfully"),
            &secrets,
            None,
            None,
        );
        assert!(result.warnings.is_empty());
    }

    // ─── Short secret values are skipped ───────────────────

    #[test]
    fn test_guard_skips_short_secret_values() {
        // Values < 8 chars should be filtered out before calling run_guard,
        // but even if passed, the guard checks length >= 8
        let secrets = vec![("SHORT".to_string(), "abc".to_string())];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("echo abc"),
            &secrets,
            None,
            None,
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_guard_catches_exactly_8_char_secret() {
        let secrets = vec![("TOKEN".to_string(), "12345678".to_string())];
        let result = run_guard(
            GuardStage::PreTool,
            "Bash",
            Some("echo 12345678"),
            &secrets,
            None,
            None,
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

    // ─── External scanner ─────────────────────────────────────

    #[test]
    fn test_external_scanner_success_command_returns_none() {
        // A command that exits 0 means "no findings" -> None
        let result = run_external_scanner_with_cmd("cat", "some content");
        assert!(result.is_none());
    }

    #[test]
    fn test_external_scanner_with_echo_command() {
        // A scanner that always "finds" something (exit 1 + output)
        let cmd = "sh -c 'echo \"found secret in\" && exit 1' #";
        let result = run_external_scanner_with_cmd(cmd, "test content");

        assert!(result.is_some());
        let findings = result.unwrap();
        assert!(!findings.is_empty());
        assert!(findings[0].contains("found secret in"));
    }

    #[test]
    fn test_external_scanner_success_returns_none() {
        // A scanner that exits 0 means no findings
        let result = run_external_scanner_with_cmd("true", "safe content");
        assert!(result.is_none());
    }

    #[test]
    fn test_external_scanner_findings_collected() {
        // Scanner that outputs multiple lines on failure
        let cmd = "sh -c 'echo line1 && echo line2 && exit 1' #";
        let result = run_external_scanner_with_cmd(cmd, "content");
        assert!(result.is_some());
        let findings = result.unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0], "line1");
        assert_eq!(findings[1], "line2");
    }
}
