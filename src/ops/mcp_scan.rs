use std::path::{Path, PathBuf};

/// A credential found in an MCP configuration file.
#[derive(Debug, Clone)]
pub struct McpFinding {
    pub file: PathBuf,
    pub path: String,
    pub key: String,
    pub value_preview: String,
    pub pattern: String,
}

/// Known MCP config file locations (relative to home or project root).
pub fn mcp_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Claude Desktop
        paths.push(home.join(".claude").join("claude_desktop_config.json"));
        // macOS Claude
        paths.push(
            home.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json"),
        );
        // Cursor
        paths.push(home.join(".cursor").join("mcp.json"));
        // GitHub Copilot — scan all JSON files in config dir
        let copilot_dir = home.join(".config").join("github-copilot");
        if copilot_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&copilot_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("json") {
                        paths.push(p);
                    }
                }
            }
        }
    }

    // Project-root configs
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(".mcp.json"));
        paths.push(cwd.join("mcp.json"));
    }

    paths
}

/// Scan an in-memory JSON document for credential patterns. Mirrors
/// [`scan_json_file`] but operates on a `&str` so the LSP server can
/// run findings against the editor's current buffer (which may include
/// unsaved edits) instead of a disk file. The `file` argument is the
/// virtual identifier carried into each finding so downstream code can
/// continue to format file paths consistently. Returns an empty vector
/// for invalid JSON, oversized input, or any IO-free failure.
pub fn scan_mcp_text(content: &str, file: &Path) -> Vec<McpFinding> {
    if content.len() as u64 > MAX_MCP_CONFIG_BYTES {
        return Vec::new();
    }
    let value: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut findings = Vec::new();
    let mut key_count = 0;
    walk_json(
        &value,
        "",
        &file.to_path_buf(),
        &mut findings,
        0,
        &mut key_count,
    );
    findings
}

/// Scan all known MCP config locations for hardcoded credentials.
pub fn scan_mcp_configs() -> Vec<McpFinding> {
    let mut findings = Vec::new();
    for path in mcp_config_paths() {
        if path.is_file() {
            findings.extend(scan_json_file(&path));
        }
    }
    findings
}

/// Return the list of MCP config paths that actually exist on disk.
pub fn scanned_file_count() -> usize {
    mcp_config_paths().iter().filter(|p| p.is_file()).count()
}

/// Maximum size of an MCP config file we will parse. Larger files are
/// skipped to defend against OOM / DoS via a malicious / corrupt config.
const MAX_MCP_CONFIG_BYTES: u64 = 1024 * 1024;

/// Maximum recursion depth allowed during JSON walking. Defends against
/// stack-overflow DoS via deeply nested objects.
const MAX_JSON_DEPTH: usize = 64;

/// Maximum number of top-level + nested keys traversed. A flat object
/// with 100K keys at depth 1 would pass the depth check but exhaust CPU
/// and push 100K findings. This cap bounds total walk iterations.
const MAX_JSON_KEYS: usize = 10_000;

/// Scan a single JSON file for credential patterns.
pub fn scan_json_file(path: &PathBuf) -> Vec<McpFinding> {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_MCP_CONFIG_BYTES {
            eprintln!(
                "mcp_scan: skipping {} (file is {} bytes; limit {})",
                path.display(),
                meta.len(),
                MAX_MCP_CONFIG_BYTES
            );
            return Vec::new();
        }
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut findings = Vec::new();
    let mut key_count = 0;
    walk_json(&value, "", path, &mut findings, 0, &mut key_count);
    findings
}

/// Recursively walk a JSON value tree, checking string values for credentials.
/// Bails out at [`MAX_JSON_DEPTH`] to defend against stack-overflow DoS.
fn walk_json(
    value: &serde_json::Value,
    json_path: &str,
    file: &PathBuf,
    findings: &mut Vec<McpFinding>,
    depth: usize,
    key_count: &mut usize,
) {
    if depth > MAX_JSON_DEPTH {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                *key_count += 1;
                if *key_count > MAX_JSON_KEYS {
                    return;
                }
                let child_path = if json_path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", json_path, key)
                };
                walk_json(val, &child_path, file, findings, depth + 1, key_count);
            }
        }
        serde_json::Value::Array(arr) => {
            for (idx, val) in arr.iter().enumerate() {
                let child_path = format!("{}[{}]", json_path, idx);
                walk_json(val, &child_path, file, findings, depth + 1, key_count);
            }
        }
        serde_json::Value::String(s) => {
            if let Some(pattern) = detect_credential(json_path, s) {
                let key = extract_key_from_path(json_path);
                findings.push(McpFinding {
                    file: file.clone(),
                    path: json_path.to_string(),
                    key,
                    value_preview: mask_value(s),
                    pattern,
                });
            }
        }
        _ => {}
    }
}

/// Extract the leaf key name from a JSON path like "mcpServers.slack.env.API_KEY".
fn extract_key_from_path(path: &str) -> String {
    // Handle array index: "mcpServers.api.args[2]" -> "args[2]"
    if let Some(last_dot) = path.rfind('.') {
        path[last_dot + 1..].to_string()
    } else {
        path.to_string()
    }
}

/// Check if a JSON string value looks like a hardcoded credential.
/// Returns the pattern name if a match is found.
fn detect_credential(json_path: &str, value: &str) -> Option<String> {
    // Skip empty, very short, or env var reference values
    if value.is_empty() || value.len() < 4 {
        return None;
    }
    if value.starts_with("${") || value.starts_with("$") {
        return None;
    }

    // 1. Known prefix patterns (high confidence)
    if let Some(pattern) = detect_known_prefix(value) {
        return Some(pattern);
    }

    // 2. Connection strings with embedded credentials
    if detect_connection_string(value) {
        return Some("Connection string".to_string());
    }

    // 3. Key-name-based detection: if the JSON key name suggests a secret,
    //    check if the value looks like it could be one
    let path_lower = json_path.to_lowercase();
    if is_secret_key_name(&path_lower) && looks_like_secret_value(value) {
        return Some("Sensitive key value".to_string());
    }

    None
}

/// Detect well-known API key prefixes.
fn detect_known_prefix(value: &str) -> Option<String> {
    let prefixes: &[(&str, &str)] = &[
        ("sk-", "API key (OpenAI/Stripe style)"),
        ("sk_live_", "Stripe live key"),
        ("sk_test_", "Stripe test key"),
        ("pk_live_", "Stripe publishable live key"),
        ("pk_test_", "Stripe publishable test key"),
        ("AKIA", "AWS access key"),
        ("ghp_", "GitHub personal access token"),
        ("ghs_", "GitHub server token"),
        ("gho_", "GitHub OAuth token"),
        ("ghu_", "GitHub user token"),
        ("github_pat_", "GitHub fine-grained PAT"),
        ("glpat-", "GitLab personal access token"),
        ("xoxb-", "Slack bot token"),
        ("xoxp-", "Slack user token"),
        ("xoxa-", "Slack app token"),
        ("xoxs-", "Slack session token"),
        ("whsec_", "Webhook secret"),
        ("rk_live_", "Stripe restricted key"),
        ("rk_test_", "Stripe restricted test key"),
        ("SG.", "SendGrid API key"),
        ("sq0atp-", "Square access token"),
        ("sq0csp-", "Square client secret"),
        ("hf_", "Hugging Face token"),
        ("eyJ", "JWT token (base64)"),
    ];

    for (prefix, name) in prefixes {
        if value.starts_with(prefix) && value.len() > prefix.len() + 4 {
            return Some((*name).to_string());
        }
    }

    None
}

/// Detect connection strings with embedded credentials (e.g., postgres://user:pass@host/db).
fn detect_connection_string(value: &str) -> bool {
    // Must contain a scheme and an @ sign
    if let Some(scheme_end) = value.find("://") {
        let after_scheme = &value[scheme_end + 3..];
        if after_scheme.contains('@') {
            // Check for user:password pattern
            if let Some(at_pos) = after_scheme.find('@') {
                let user_info = &after_scheme[..at_pos];
                if user_info.contains(':') {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a JSON path segment name suggests it holds a secret.
fn is_secret_key_name(path_lower: &str) -> bool {
    let secret_keywords = [
        "token",
        "secret",
        "password",
        "passwd",
        "credential",
        "api_key",
        "apikey",
        "api-key",
        "access_key",
        "private_key",
        "auth",
    ];
    secret_keywords.iter().any(|kw| path_lower.contains(kw))
}

/// Heuristic: does this value look like an actual secret (not a placeholder)?
fn looks_like_secret_value(value: &str) -> bool {
    // Skip common placeholders / non-secrets
    if value == "true" || value == "false" || value == "null" {
        return false;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        // URLs without auth are fine
        if !value.contains('@') {
            return false;
        }
    }

    // Must be reasonably long
    if value.len() < 8 {
        return false;
    }

    // High entropy check: count distinct character classes
    let has_upper = value.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = value.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    let has_special = value.chars().any(|c| !c.is_ascii_alphanumeric());
    let class_count = [has_upper, has_lower, has_digit, has_special]
        .iter()
        .filter(|&&b| b)
        .count();

    // Long random-looking strings with multiple character classes
    if value.len() >= 20 && class_count >= 2 {
        return true;
    }

    // Shorter but has 3+ char classes (likely a password/token)
    if value.len() >= 8 && class_count >= 3 {
        return true;
    }

    false
}

/// Mask a value: show first 4 and last 4 characters with **** in between.
pub fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        return "****".to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    let first4: String = chars[..4].iter().collect();
    let last4: String = chars[chars.len() - 4..].iter().collect();
    format!("{}****{}", first4, last4)
}

/// Build a suggestion for fixing a finding.
pub fn suggestion_for(finding: &McpFinding) -> String {
    // If the finding is in an args array, suggest moving to env section
    if finding.path.contains("[") {
        format!(
            "Move to env section: \"{}\": \"${{{}}}\"",
            finding
                .key
                .trim_start_matches("args[")
                .trim_end_matches(']'),
            finding
                .key
                .to_uppercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        )
    } else {
        format!("Replace with: ${{{}}}", finding.key)
    }
}

/// Convert findings to JSON value.
pub fn findings_to_json(findings: &[McpFinding], files_scanned: usize) -> serde_json::Value {
    let items: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "file": f.file.to_string_lossy(),
                "json_path": f.path,
                "key": f.key,
                "value_preview": f.value_preview,
                "pattern": f.pattern,
                "suggestion": suggestion_for(f),
            })
        })
        .collect();

    // Additive: include MCP pin status if a lockfile is present in cwd.
    // Consumers using only legacy fields remain backward-compatible.
    let mcp_pin_status = build_mcp_pin_status_summary();

    serde_json::json!({
        "files_scanned": files_scanned,
        "credentials_found": findings.len(),
        "findings": items,
        "mcp_pin_status": mcp_pin_status,
    })
}

/// Build a JSON summary of the MCP pin / reputation state for the current
/// project (cwd). Returns `null` when neither the lockfile nor the
/// reputation feed can be consulted.
///
/// Additive — does not modify
/// any pre-existing field in `findings_to_json`.
fn build_mcp_pin_status_summary() -> serde_json::Value {
    use crate::ops::doctor::{build_mcp_section, DoctorOpts};

    let opts = DoctorOpts {
        include_unknown: false,
    };
    match build_mcp_section(&opts) {
        Some(section) => serde_json::json!({
            "lockfile_exists": section.lockfile_exists,
            "pinned_count": section.pinned_server_count,
            "known_bad_count": section.known_bad_count,
            "unknown_count": section.unknown_count,
            "feed_version": section.feed_version,
            "feed_stale": section.feed_stale,
            "known_bad_servers": section.known_bad_servers,
        }),
        None => serde_json::Value::Null,
    }
}

/// Replace a value at a JSON path with `${KEY}` env var reference.
fn replace_in_json(json: &mut serde_json::Value, json_path: &str, key: &str) -> bool {
    let parts: Vec<&str> = json_path.split('.').collect();
    let mut current = json;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Handle array index: "args[2]"
            if let Some(bracket) = part.find('[') {
                let arr_key = &part[..bracket];
                let idx_str = part[bracket + 1..].trim_end_matches(']');
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(arr) = current.get_mut(arr_key).and_then(|v| v.as_array_mut()) {
                        if idx < arr.len() {
                            let ref_value = format!(
                                "${{{}}}",
                                key.to_uppercase()
                                    .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
                            );
                            arr[idx] = serde_json::Value::String(ref_value);
                            return true;
                        }
                    }
                }
                return false;
            }

            // Regular object key
            if let Some(obj) = current.as_object_mut() {
                if obj.contains_key(*part) {
                    let ref_value = format!("${{{}}}", key.to_uppercase());
                    obj.insert((*part).to_string(), serde_json::Value::String(ref_value));
                    return true;
                }
            }
            return false;
        }
        // Navigate deeper
        if let Some(next) = current.get_mut(*part) {
            current = next;
        } else {
            return false;
        }
    }
    false
}

/// Harden an MCP config file by replacing plaintext secrets with env var references.
/// Returns (modified_count, list of replaced keys, backup_path).
pub fn harden_mcp_config(
    file_path: &Path,
    dry_run: bool,
) -> Result<(usize, Vec<String>, Option<PathBuf>), super::OpError> {
    // Same size cap as scan_json_file. Don't OOM trying to "harden" a
    // crafted huge config.
    if let Ok(meta) = std::fs::metadata(file_path) {
        if meta.len() > MAX_MCP_CONFIG_BYTES {
            return Err(super::OpError::from(format!(
                "mcp_scan: refusing to harden {} (file is {} bytes; limit {})",
                file_path.display(),
                meta.len(),
                MAX_MCP_CONFIG_BYTES
            )));
        }
    }
    let content = std::fs::read_to_string(file_path)?;
    let mut json: serde_json::Value = serde_json::from_str(&content)?;

    let findings = scan_json_file(&file_path.to_path_buf());
    if findings.is_empty() {
        return Ok((0, Vec::new(), None));
    }

    let mut modified = 0;
    let mut replaced_keys = Vec::new();
    for finding in &findings {
        let env_key = if finding.path.contains('[') {
            finding
                .key
                .to_uppercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        } else {
            finding.key.to_uppercase()
        };
        if replace_in_json(&mut json, &finding.path, &finding.key) {
            modified += 1;
            replaced_keys.push(env_key);
        }
    }

    if dry_run || modified == 0 {
        return Ok((modified, replaced_keys, None));
    }

    // Backup original. The backup contains plaintext secrets, so create it
    // 0600 *at creation time* rather than copy-then-chmod — `std::fs::copy`
    // lands the file at umask perms (e.g. 0644), leaving a window in which any
    // same-uid process (incl. an AI agent) could read it. Errors are
    // propagated, not discarded: a chmod/IO failure must not silently leave an
    // over-permissioned secret on disk.
    let backup = file_path.with_extension("json.bak");
    if backup.exists() {
        std::fs::remove_file(&backup)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let original = std::fs::read(file_path)?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&backup)?;
        f.write_all(&original)?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(file_path, &backup)?;
    }

    // Write hardened config
    let hardened = serde_json::to_string_pretty(&json)?;
    std::fs::write(file_path, hardened)?;

    Ok((modified, replaced_keys, Some(backup)))
}

/// Harden all known MCP config files.
pub fn harden_all_mcp_configs(
    dry_run: bool,
) -> Vec<(PathBuf, usize, Vec<String>, Option<PathBuf>)> {
    let mut results = Vec::new();
    for path in mcp_config_paths() {
        if path.is_file() {
            if let Ok((count, keys, backup)) = harden_mcp_config(&path, dry_run) {
                if count > 0 {
                    results.push((path, count, keys, backup));
                }
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to scan a JSON string directly.
    fn scan_json_str(json_str: &str) -> Vec<McpFinding> {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), json_str).unwrap();
        scan_json_file(&tmp.path().to_path_buf())
    }

    #[test]
    fn test_detect_api_key_openai() {
        let json = r#"{
            "mcpServers": {
                "my-server": {
                    "env": {
                        "OPENAI_API_KEY": "sk-proj-abcdef1234567890abcdef1234567890"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].key, "OPENAI_API_KEY");
        assert!(findings[0].pattern.contains("API key"));
    }

    #[test]
    fn test_detect_github_token() {
        let json = r#"{
            "mcpServers": {
                "gh": {
                    "env": {
                        "GITHUB_TOKEN": "ghp_FAKE00TEST00TOKEN00VALUE00000000"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("GitHub"));
    }

    #[test]
    fn test_detect_slack_token() {
        let json = r#"{
            "mcpServers": {
                "slack": {
                    "env": {
                        "SLACK_TOKEN": "xoxb-0000-0000-FAKE_TEST_VALUE0"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("Slack"));
    }

    #[test]
    fn test_detect_aws_access_key() {
        let json = r#"{
            "mcpServers": {
                "aws": {
                    "env": {
                        "AWS_ACCESS_KEY_ID": "AKIA_FAKE_TEST_VALUE"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("AWS"));
    }

    #[test]
    fn test_detect_connection_string() {
        let json = r#"{
            "mcpServers": {
                "db": {
                    "env": {
                        "DATABASE_URL": "postgres://admin:supersecret@db.example.com:5432/mydb"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("Connection string"));
    }

    #[test]
    fn test_detect_token_in_args() {
        let json = r#"{
            "mcpServers": {
                "api": {
                    "command": "node",
                    "args": ["server.js", "--token", "ghp_FAKE00TEST00TOKEN00VALUE00000000"]
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "mcpServers.api.args[2]");
    }

    #[test]
    fn test_detect_sensitive_key_name() {
        let json = r#"{
            "mcpServers": {
                "svc": {
                    "env": {
                        "MY_SECRET": "FAKE_TEST_LONG_RANDOM_VALUE_FOR_TESTING"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("Sensitive key value"));
    }

    #[test]
    fn test_clean_config_no_findings() {
        let json = r#"{
            "mcpServers": {
                "my-server": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {
                        "PORT": "3000",
                        "HOST": "localhost",
                        "DEBUG": "true"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert!(
            findings.is_empty(),
            "Expected no findings, got: {:?}",
            findings
        );
    }

    #[test]
    fn test_env_var_references_ignored() {
        let json = r#"{
            "mcpServers": {
                "my-server": {
                    "env": {
                        "API_KEY": "${API_KEY}",
                        "TOKEN": "$MY_TOKEN"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_mask_value_long() {
        assert_eq!(mask_value("sk-proj-abcdef1234567890"), "sk-p****7890");
    }

    #[test]
    fn test_mask_value_short() {
        assert_eq!(mask_value("abcd"), "****");
    }

    #[test]
    fn test_mask_value_exact_boundary() {
        // 8 chars: show first 4 and last 4 overlap — treated as short
        assert_eq!(mask_value("12345678"), "****");
        // 9 chars: show first 4 + last 4
        assert_eq!(mask_value("123456789"), "1234****6789");
    }

    #[test]
    fn test_nested_json_traversal() {
        let json = r#"{
            "mcpServers": {
                "level1": {
                    "nested": {
                        "deep": {
                            "env": {
                                "API_KEY": "sk-aaaaaabbbbbbccccccdddddd"
                            }
                        }
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].path,
            "mcpServers.level1.nested.deep.env.API_KEY"
        );
    }

    #[test]
    fn test_multiple_findings_in_one_file() {
        let json = r#"{
            "mcpServers": {
                "a": {
                    "env": {
                        "KEY1": "sk-aaaaaabbbbbbccccccdddddd",
                        "KEY2": "ghp_FAKE00TEST00TOKEN00VALUE00000000"
                    }
                },
                "b": {
                    "env": {
                        "DB": "postgres://user:pass@host/db"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn test_gitlab_token() {
        let json = r#"{
            "mcpServers": {
                "gl": {
                    "env": {
                        "GITLAB_TOKEN": "glpat-FAKE_TEST_TOKEN_VALUE"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("GitLab"));
    }

    #[test]
    fn test_stripe_live_key() {
        let json = r#"{
            "mcpServers": {
                "pay": {
                    "env": {
                        "STRIPE_KEY": "sk_live_FAKE_TEST_TOKEN_VAL"
                    }
                }
            }
        }"#;
        let findings = scan_json_str(json);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].pattern.contains("Stripe live"));
    }

    #[test]
    fn test_findings_to_json() {
        let findings = vec![McpFinding {
            file: PathBuf::from("/tmp/test.json"),
            path: "mcpServers.s.env.KEY".to_string(),
            key: "KEY".to_string(),
            value_preview: "sk-a****efgh".to_string(),
            pattern: "API key".to_string(),
        }];
        let json = findings_to_json(&findings, 1);
        assert_eq!(json["credentials_found"], 1);
        assert_eq!(json["files_scanned"], 1);
        assert_eq!(json["findings"][0]["key"], "KEY");
    }

    #[test]
    fn test_invalid_json_no_panic() {
        let findings = scan_json_str("this is not json { broken");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_empty_json_no_findings() {
        let findings = scan_json_str("{}");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_suggestion_for_env_key() {
        let finding = McpFinding {
            file: PathBuf::from("test.json"),
            path: "mcpServers.s.env.API_KEY".to_string(),
            key: "API_KEY".to_string(),
            value_preview: "sk-a****efgh".to_string(),
            pattern: "API key".to_string(),
        };
        assert_eq!(suggestion_for(&finding), "Replace with: ${API_KEY}");
    }

    #[test]
    fn test_suggestion_for_args_value() {
        let finding = McpFinding {
            file: PathBuf::from("test.json"),
            path: "mcpServers.api.args[2]".to_string(),
            key: "args[2]".to_string(),
            value_preview: "ghp_****0000".to_string(),
            pattern: "GitHub token".to_string(),
        };
        let suggestion = suggestion_for(&finding);
        assert!(suggestion.contains("Move to env section"));
    }

    #[test]
    fn test_replace_in_json_nested_path() {
        let mut json: serde_json::Value = serde_json::from_str(
            r#"{
            "mcpServers": {
                "slack": {
                    "env": {
                        "SLACK_TOKEN": "xoxb-0000-0000-FAKE_TEST_VALUE0"
                    }
                }
            }
        }"#,
        )
        .unwrap();

        let replaced =
            replace_in_json(&mut json, "mcpServers.slack.env.SLACK_TOKEN", "SLACK_TOKEN");
        assert!(replaced);
        assert_eq!(
            json["mcpServers"]["slack"]["env"]["SLACK_TOKEN"],
            "${SLACK_TOKEN}"
        );
    }

    #[test]
    fn test_replace_in_json_preserves_non_secrets() {
        let mut json: serde_json::Value = serde_json::from_str(
            r#"{
            "mcpServers": {
                "my-server": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {
                        "PORT": "3000",
                        "API_KEY": "sk-proj-abcdef1234567890abcdef1234567890"
                    }
                }
            }
        }"#,
        )
        .unwrap();

        let replaced = replace_in_json(&mut json, "mcpServers.my-server.env.API_KEY", "API_KEY");
        assert!(replaced);
        // Secret replaced
        assert_eq!(
            json["mcpServers"]["my-server"]["env"]["API_KEY"],
            "${API_KEY}"
        );
        // Non-secret preserved
        assert_eq!(json["mcpServers"]["my-server"]["env"]["PORT"], "3000");
        assert_eq!(json["mcpServers"]["my-server"]["command"], "node");
    }

    #[test]
    fn test_replace_in_json_nonexistent_path() {
        let mut json: serde_json::Value = serde_json::from_str(
            r#"{
            "mcpServers": {}
        }"#,
        )
        .unwrap();

        let replaced = replace_in_json(&mut json, "mcpServers.missing.env.KEY", "KEY");
        assert!(!replaced);
    }

    #[test]
    fn test_replace_in_json_array_index() {
        let mut json: serde_json::Value = serde_json::from_str(
            r#"{
            "mcpServers": {
                "api": {
                    "command": "node",
                    "args": ["server.js", "--token", "ghp_FAKE00TEST00TOKEN00VALUE00000000"]
                }
            }
        }"#,
        )
        .unwrap();

        let replaced = replace_in_json(&mut json, "mcpServers.api.args[2]", "args[2]");
        assert!(replaced);
        assert_eq!(json["mcpServers"]["api"]["args"][2], "${ARGS_2_}");
        // Other args preserved
        assert_eq!(json["mcpServers"]["api"]["args"][0], "server.js");
        assert_eq!(json["mcpServers"]["api"]["args"][1], "--token");
    }

    #[test]
    fn test_harden_mcp_config_dry_run() {
        let tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        let json_content = r#"{
            "mcpServers": {
                "slack": {
                    "env": {
                        "SLACK_TOKEN": "xoxb-0000-0000-FAKE_TEST_VALUE0"
                    }
                }
            }
        }"#;
        std::fs::write(tmp.path(), json_content).unwrap();

        let (count, keys, backup) = harden_mcp_config(tmp.path(), true).unwrap();
        assert_eq!(count, 1);
        assert!(keys.contains(&"SLACK_TOKEN".to_string()));
        assert!(backup.is_none()); // dry run — no backup

        // File should be unchanged
        let after = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(after.contains("xoxb-0000-0000"));
    }

    #[test]
    fn test_harden_mcp_config_writes_and_backs_up() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.json");
        let json_content = r#"{
            "mcpServers": {
                "db": {
                    "env": {
                        "DATABASE_URL": "postgres://admin:supersecret@db.example.com:5432/mydb"
                    }
                }
            }
        }"#;
        std::fs::write(&file, json_content).unwrap();

        let (count, keys, backup) = harden_mcp_config(&file, false).unwrap();
        assert_eq!(count, 1);
        assert!(keys.contains(&"DATABASE_URL".to_string()));
        assert!(backup.is_some());

        // Backup should exist and contain original
        let backup_path = backup.unwrap();
        assert!(backup_path.exists());
        let backup_content = std::fs::read_to_string(&backup_path).unwrap();
        assert!(backup_content.contains("postgres://admin:supersecret"));

        // Hardened file should have env var ref
        let hardened = std::fs::read_to_string(&file).unwrap();
        assert!(hardened.contains("${DATABASE_URL}"));
        assert!(!hardened.contains("supersecret"));
    }

    #[test]
    fn test_harden_mcp_config_no_findings() {
        let tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        let json_content = r#"{
            "mcpServers": {
                "safe": {
                    "command": "node",
                    "env": {
                        "PORT": "3000"
                    }
                }
            }
        }"#;
        std::fs::write(tmp.path(), json_content).unwrap();

        let (count, keys, backup) = harden_mcp_config(tmp.path(), false).unwrap();
        assert_eq!(count, 0);
        assert!(keys.is_empty());
        assert!(backup.is_none());
    }

    #[test]
    fn test_harden_multiple_secrets_in_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("multi.json");
        let json_content = r#"{
            "mcpServers": {
                "a": {
                    "env": {
                        "KEY1": "sk-aaaaaabbbbbbccccccdddddd",
                        "KEY2": "ghp_FAKE00TEST00TOKEN00VALUE00000000"
                    }
                }
            }
        }"#;
        std::fs::write(&file, json_content).unwrap();

        let (count, keys, backup) = harden_mcp_config(&file, false).unwrap();
        assert_eq!(count, 2);
        assert!(keys.contains(&"KEY1".to_string()));
        assert!(keys.contains(&"KEY2".to_string()));
        assert!(backup.is_some());

        let hardened = std::fs::read_to_string(&file).unwrap();
        assert!(hardened.contains("${KEY1}"));
        assert!(hardened.contains("${KEY2}"));
    }
}
