use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;

// ─── Types ─────────────────────────────────────────────────

/// Configuration for the credential proxy server.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub port: u16,
    pub allowed_keys: Option<Vec<String>>,
}

/// Audit log entry — NEVER contains secret values, only key names.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub key: Option<String>,
    pub keys_served: Option<usize>,
    pub client_addr: String,
    pub user_agent: Option<String>,
    pub granted: bool,
}

// ─── Audit logging ────────────────────────────────────────

const AUDIT_MAX_ENTRIES: usize = 10_000;

/// Append an audit entry to the JSONL audit log.
/// Rotates the log when it exceeds AUDIT_MAX_ENTRIES.
fn log_audit(entry: &AuditEntry) {
    if let Ok(dir) = crate::config::config_dir() {
        let path = dir.join("access-audit.jsonl");
        // Rotate if needed
        rotate_audit_log(&path);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            if let Ok(json) = serde_json::to_string(entry) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }
}

/// Keep only the last AUDIT_MAX_ENTRIES lines in the audit log.
fn rotate_audit_log(path: &std::path::Path) {
    if let Ok(contents) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = contents.lines().collect();
        if lines.len() >= AUDIT_MAX_ENTRIES {
            let keep = &lines[lines.len() - (AUDIT_MAX_ENTRIES - 1)..];
            let new_contents = keep.join("\n") + "\n";
            let _ = std::fs::write(path, new_contents);
        }
    }
}

/// Read all audit entries from the access-audit.jsonl file.
pub fn read_audit_log() -> Result<Vec<AuditEntry>, Box<dyn std::error::Error>> {
    let dir = crate::config::config_dir()?;
    let path = dir.join("access-audit.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path)?;
    let entries: Vec<AuditEntry> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Ok(entries)
}

// ─── Origin checking ──────────────────────────────────────

/// Extract the `Origin` or `Referer` header value from raw HTTP request text.
pub fn extract_origin(request: &str) -> Option<String> {
    for line in request.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("origin:") {
            return Some(line[7..].trim().to_string());
        }
        if lower.starts_with("referer:") {
            return Some(line[8..].trim().to_string());
        }
    }
    None
}

/// Extract the domain/host from a URL or origin value.
/// e.g. "http://example.com:3000/path" -> "example.com"
pub fn extract_host_from_origin(origin: &str) -> String {
    let without_scheme = if let Some(pos) = origin.find("://") {
        &origin[pos + 3..]
    } else {
        origin
    };
    // Strip path
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    // Strip port
    let host = if host_port.starts_with('[') {
        // IPv6 like [::1]:port
        host_port.split(']').next().unwrap_or(host_port).trim_start_matches('[').to_string()
    } else {
        host_port.split(':').next().unwrap_or(host_port).to_string()
    };
    host
}

/// Default safe origins: loopback addresses only.
const DEFAULT_SAFE_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

/// Check whether a request origin is allowed.
/// - If `allowed_origins` is Some, check against that list (plus loopback always allowed).
/// - If `allowed_origins` is None, only allow loopback.
/// Returns true if allowed.
pub fn is_origin_allowed(origin: Option<&str>, allowed_origins: Option<&[String]>) -> bool {
    let origin = match origin {
        Some(o) if !o.is_empty() => o,
        // No origin header → likely a direct/curl request from localhost; allow
        _ => return true,
    };

    let host = extract_host_from_origin(origin).to_ascii_lowercase();

    // Loopback is always allowed
    if DEFAULT_SAFE_HOSTS.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return true;
    }

    if let Some(allowed) = allowed_origins {
        allowed.iter().any(|a| {
            let allowed_host = extract_host_from_origin(a).to_ascii_lowercase();
            allowed_host == host
        })
    } else {
        // No allowlist set → default safe: only loopback
        false
    }
}

// ─── Request / Response helpers ────────────────────────────

/// Parse an HTTP request line and return the method and path.
pub fn parse_request_line(request: &str) -> (&str, &str) {
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = if !parts.is_empty() { parts[0] } else { "" };
    let path = if parts.len() >= 2 { parts[1] } else { "/" };
    (method, path)
}

/// Extract the User-Agent header value.
pub fn extract_user_agent(request: &str) -> Option<String> {
    for line in request.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("user-agent:") {
            return Some(line[11..].trim().to_string());
        }
    }
    None
}

/// Check lease access for a request path. Returns an error response if denied.
/// Returns None if access is granted (or lease not required).
pub fn check_lease_for_request(
    path: &str,
    require_lease: bool,
) -> Option<(String, String)> {
    if !require_lease {
        return None;
    }

    // Health endpoint is always accessible
    if path == "/health" {
        return None;
    }

    if path == "/env" {
        // For /env (all keys), check if any active lease exists
        if super::lease::check_lease_access("*").is_none() {
            // Try with a wildcard; if no lease grants all-keys access, check any
            // Actually we need to check if there's any active lease at all
            let leases = super::lease::list_leases().unwrap_or_default();
            let has_active = leases.iter().any(|s| !s.expired && !s.revoked);
            if !has_active {
                return Some((
                    "403 Forbidden".to_string(),
                    r#"{"error":"no active lease","hint":"Run: envforge lease create --ttl 1h"}"#
                        .to_string(),
                ));
            }
        }
        return None;
    }

    if path.starts_with("/env/") {
        let key = &path[5..];
        if !key.is_empty() {
            if super::lease::check_lease_access(key).is_none() {
                return Some((
                    "403 Forbidden".to_string(),
                    format!(
                        r#"{{"error":"no active lease for key '{}'","hint":"Run: envforge lease create --ttl 1h --keys {}"}}"#,
                        key, key
                    ),
                ));
            }
        }
    }

    None
}

/// Route a request and return (status, body).
pub fn route_request(
    method: &str,
    path: &str,
    env: &HashMap<String, String>,
    allowed_keys: Option<&[String]>,
) -> (String, String) {
    if method != "GET" {
        return (
            "405 Method Not Allowed".to_string(),
            r#"{"error":"method not allowed"}"#.to_string(),
        );
    }

    match path {
        "/health" => (
            "200 OK".to_string(),
            r#"{"status":"ok"}"#.to_string(),
        ),
        "/env" => {
            let filtered: HashMap<&String, &String> = if let Some(keys) = allowed_keys {
                env.iter()
                    .filter(|(k, _)| keys.iter().any(|ak| ak == *k))
                    .collect()
            } else {
                env.iter().collect()
            };
            let body = serde_json::to_string_pretty(&filtered).unwrap_or_default();
            ("200 OK".to_string(), body)
        }
        p if p.starts_with("/env/") => {
            let key = &p[5..];
            if key.is_empty() {
                return (
                    "400 Bad Request".to_string(),
                    r#"{"error":"missing key name"}"#.to_string(),
                );
            }
            if let Some(value) = env.get(key) {
                if allowed_keys.map_or(true, |keys| keys.iter().any(|k| k == key)) {
                    let body = serde_json::json!({"key": key, "value": value}).to_string();
                    ("200 OK".to_string(), body)
                } else {
                    (
                        "403 Forbidden".to_string(),
                        r#"{"error":"key not in allowed list"}"#.to_string(),
                    )
                }
            } else {
                (
                    "404 Not Found".to_string(),
                    format!(r#"{{"error":"key '{}' not found"}}"#, key),
                )
            }
        }
        _ => (
            "404 Not Found".to_string(),
            r#"{"error":"not found"}"#.to_string(),
        ),
    }
}

/// Format an HTTP response.
pub fn format_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    )
}

/// Build an AuditEntry for a request.
fn build_audit_entry(
    path: &str,
    status: &str,
    client_addr: &str,
    user_agent: Option<String>,
    env: &HashMap<String, String>,
    allowed_keys: Option<&[String]>,
) -> AuditEntry {
    let granted = status.starts_with("200");
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string();

    match path {
        "/health" => AuditEntry {
            timestamp: now,
            action: "health".to_string(),
            key: None,
            keys_served: None,
            client_addr: client_addr.to_string(),
            user_agent,
            granted: true,
        },
        "/env" => {
            let count = if let Some(keys) = allowed_keys {
                env.iter()
                    .filter(|(k, _)| keys.iter().any(|ak| ak == *k))
                    .count()
            } else {
                env.len()
            };
            AuditEntry {
                timestamp: now,
                action: "access".to_string(),
                key: None,
                keys_served: Some(count),
                client_addr: client_addr.to_string(),
                user_agent,
                granted,
            }
        }
        p if p.starts_with("/env/") => {
            let key_name = &p[5..];
            AuditEntry {
                timestamp: now,
                action: "access".to_string(),
                key: if key_name.is_empty() { None } else { Some(key_name.to_string()) },
                keys_served: None,
                client_addr: client_addr.to_string(),
                user_agent,
                granted,
            }
        }
        _ => AuditEntry {
            timestamp: now,
            action: "denied".to_string(),
            key: None,
            keys_served: None,
            client_addr: client_addr.to_string(),
            user_agent,
            granted: false,
        },
    }
}

/// Prompt the human operator on stderr/stdin for approval.
/// Returns true if approved, false otherwise.
fn prompt_approval(description: &str, client_addr: &str) -> bool {
    use std::io::BufRead;
    eprint!("\u{1f512} Secret access request: {} from {}\n   Approve? [y/N]: ", description, client_addr);
    let stdin = std::io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_ok() {
        let trimmed = line.trim().to_lowercase();
        trimmed == "y" || trimmed == "yes"
    } else {
        false
    }
}

/// Start a local credential proxy server.
///
/// This blocks until the process is interrupted (Ctrl+C).
pub fn start_proxy(
    port: u16,
    env: &HashMap<String, String>,
    allowed_keys: Option<&[String]>,
    allowed_origins: Option<&[String]>,
    require_lease: bool,
    require_approval: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
    eprintln!(
        "EnvForge credential proxy listening on http://127.0.0.1:{}",
        port
    );
    eprintln!("Endpoints:");
    eprintln!("  GET /env          - all variables (JSON)");
    eprintln!("  GET /env/KEY_NAME - single variable");
    eprintln!("  GET /health       - health check");
    eprintln!(
        "Serving {} variable(s){}",
        if let Some(keys) = allowed_keys {
            keys.len()
        } else {
            env.len()
        },
        if allowed_keys.is_some() {
            " (filtered)"
        } else {
            ""
        }
    );
    if let Some(origins) = allowed_origins {
        eprintln!("Allowed origins: {} + loopback", origins.join(", "));
    } else {
        eprintln!("Allowed origins: loopback only (127.0.0.1, localhost, ::1)");
    }
    eprintln!("Audit log: ~/.config/envforge/access-audit.jsonl");
    eprintln!("Press Ctrl+C to stop.\n");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let client_addr = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                let mut buffer = [0u8; 4096];
                let n = match stream.read(&mut buffer) {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Read error: {}", e);
                        continue;
                    }
                };
                let request = String::from_utf8_lossy(&buffer[..n]);
                let (method, path) = parse_request_line(&request);
                let user_agent = extract_user_agent(&request);

                // Origin check
                let origin = extract_origin(&request);
                if !is_origin_allowed(origin.as_deref(), allowed_origins) {
                    let body = r#"{"error":"origin not allowed"}"#;
                    let response = format_response("403 Forbidden", body);
                    let _ = stream.write_all(response.as_bytes());

                    let entry = AuditEntry {
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%dT%H:%M:%S%z")
                            .to_string(),
                        action: "denied".to_string(),
                        key: None,
                        keys_served: None,
                        client_addr,
                        user_agent,
                        granted: false,
                    };
                    log_audit(&entry);
                    continue;
                }

                // Lease check
                if let Some((lease_status, lease_body)) =
                    check_lease_for_request(path, require_lease)
                {
                    let response = format_response(&lease_status, &lease_body);
                    let _ = stream.write_all(response.as_bytes());

                    let entry = AuditEntry {
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%dT%H:%M:%S%z")
                            .to_string(),
                        action: "lease_denied".to_string(),
                        key: if path.starts_with("/env/") {
                            Some(path[5..].to_string())
                        } else {
                            None
                        },
                        keys_served: None,
                        client_addr,
                        user_agent,
                        granted: false,
                    };
                    log_audit(&entry);
                    continue;
                }

                // Approval check
                if require_approval && (path == "/env" || path.starts_with("/env/")) {
                    let description = if path == "/env" {
                        let count = if let Some(keys) = allowed_keys {
                            env.iter()
                                .filter(|(k, _)| keys.iter().any(|ak| ak == *k))
                                .count()
                        } else {
                            env.len()
                        };
                        format!("ALL ({} keys)", count)
                    } else {
                        path[5..].to_string()
                    };

                    if !prompt_approval(&description, &client_addr) {
                        let body = r#"{"error":"access denied by human"}"#;
                        let response = format_response("403 Forbidden", body);
                        let _ = stream.write_all(response.as_bytes());

                        let entry = AuditEntry {
                            timestamp: chrono::Local::now()
                                .format("%Y-%m-%dT%H:%M:%S%z")
                                .to_string(),
                            action: "denied_by_human".to_string(),
                            key: if path.starts_with("/env/") {
                                Some(path[5..].to_string())
                            } else {
                                None
                            },
                            keys_served: None,
                            client_addr: client_addr.clone(),
                            user_agent: user_agent.clone(),
                            granted: false,
                        };
                        log_audit(&entry);
                        continue;
                    }

                    // Log approval
                    let entry = AuditEntry {
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%dT%H:%M:%S%z")
                            .to_string(),
                        action: "approved".to_string(),
                        key: if path.starts_with("/env/") {
                            Some(path[5..].to_string())
                        } else {
                            None
                        },
                        keys_served: None,
                        client_addr: client_addr.clone(),
                        user_agent: user_agent.clone(),
                        granted: true,
                    };
                    log_audit(&entry);
                }

                let (status, body) = route_request(method, path, env, allowed_keys);

                let entry = build_audit_entry(
                    path,
                    &status,
                    &client_addr,
                    user_agent,
                    env,
                    allowed_keys,
                );
                log_audit(&entry);

                let response = format_response(&status, &body);
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }
    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_env() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("DATABASE_URL".to_string(), "postgres://localhost/db".to_string());
        m.insert("API_KEY".to_string(), "sk-test-123".to_string());
        m.insert("APP_NAME".to_string(), "myapp".to_string());
        m
    }

    #[test]
    fn test_parse_request_line_get() {
        let req = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (method, path) = parse_request_line(req);
        assert_eq!(method, "GET");
        assert_eq!(path, "/health");
    }

    #[test]
    fn test_parse_request_line_post() {
        let req = "POST /env HTTP/1.1\r\n\r\n";
        let (method, path) = parse_request_line(req);
        assert_eq!(method, "POST");
        assert_eq!(path, "/env");
    }

    #[test]
    fn test_parse_request_line_empty() {
        let (method, path) = parse_request_line("");
        assert_eq!(method, "");
        assert_eq!(path, "/");
    }

    #[test]
    fn test_route_health() {
        let env = sample_env();
        let (status, body) = route_request("GET", "/health", &env, None);
        assert_eq!(status, "200 OK");
        assert!(body.contains("ok"));
    }

    #[test]
    fn test_route_env_all() {
        let env = sample_env();
        let (status, body) = route_request("GET", "/env", &env, None);
        assert_eq!(status, "200 OK");
        let parsed: HashMap<String, String> = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed["API_KEY"], "sk-test-123");
    }

    #[test]
    fn test_route_env_filtered() {
        let env = sample_env();
        let allowed = vec!["APP_NAME".to_string()];
        let (status, body) = route_request("GET", "/env", &env, Some(&allowed));
        assert_eq!(status, "200 OK");
        let parsed: HashMap<String, String> = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["APP_NAME"], "myapp");
    }

    #[test]
    fn test_route_env_single_key() {
        let env = sample_env();
        let (status, body) = route_request("GET", "/env/API_KEY", &env, None);
        assert_eq!(status, "200 OK");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["key"], "API_KEY");
        assert_eq!(parsed["value"], "sk-test-123");
    }

    #[test]
    fn test_route_env_single_key_not_found() {
        let env = sample_env();
        let (status, body) = route_request("GET", "/env/MISSING", &env, None);
        assert_eq!(status, "404 Not Found");
        assert!(body.contains("not found"));
    }

    #[test]
    fn test_route_env_single_key_forbidden() {
        let env = sample_env();
        let allowed = vec!["APP_NAME".to_string()];
        let (status, _body) = route_request("GET", "/env/API_KEY", &env, Some(&allowed));
        assert_eq!(status, "403 Forbidden");
    }

    #[test]
    fn test_route_env_empty_key() {
        let env = sample_env();
        let (status, _body) = route_request("GET", "/env/", &env, None);
        assert_eq!(status, "400 Bad Request");
    }

    #[test]
    fn test_route_not_found() {
        let env = sample_env();
        let (status, _) = route_request("GET", "/unknown", &env, None);
        assert_eq!(status, "404 Not Found");
    }

    #[test]
    fn test_route_method_not_allowed() {
        let env = sample_env();
        let (status, _) = route_request("POST", "/health", &env, None);
        assert_eq!(status, "405 Method Not Allowed");
    }

    #[test]
    fn test_format_response() {
        let resp = format_response("200 OK", r#"{"status":"ok"}"#);
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp.contains("Content-Type: application/json"));
        assert!(resp.contains("Content-Length: 15"));
        assert!(resp.contains("Access-Control-Allow-Origin: *"));
        assert!(resp.ends_with(r#"{"status":"ok"}"#));
    }

    // ─── Audit entry tests ────────────────────────────────

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            timestamp: "2026-04-20T10:00:00+0000".to_string(),
            action: "access".to_string(),
            key: Some("API_KEY".to_string()),
            keys_served: None,
            client_addr: "127.0.0.1:54321".to_string(),
            user_agent: Some("curl/8.0".to_string()),
            granted: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"action\":\"access\""));
        assert!(json.contains("\"key\":\"API_KEY\""));
        assert!(json.contains("\"granted\":true"));
        // Ensure no secret values leak — only key name
        assert!(!json.contains("sk-test"));

        // Round-trip
        let parsed: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.action, "access");
        assert_eq!(parsed.key, Some("API_KEY".to_string()));
        assert!(parsed.granted);
    }

    #[test]
    fn test_audit_entry_health() {
        let entry = AuditEntry {
            timestamp: "2026-04-20T10:00:00+0000".to_string(),
            action: "health".to_string(),
            key: None,
            keys_served: None,
            client_addr: "127.0.0.1:12345".to_string(),
            user_agent: None,
            granted: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"action\":\"health\""));
        assert!(json.contains("\"key\":null"));
    }

    #[test]
    fn test_audit_entry_keys_served() {
        let entry = AuditEntry {
            timestamp: "2026-04-20T10:00:00+0000".to_string(),
            action: "access".to_string(),
            key: None,
            keys_served: Some(5),
            client_addr: "127.0.0.1:12345".to_string(),
            user_agent: None,
            granted: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"keys_served\":5"));
    }

    // ─── Origin checking tests ────────────────────────────

    #[test]
    fn test_extract_origin_header() {
        let req = "GET /env HTTP/1.1\r\nHost: localhost\r\nOrigin: http://example.com\r\n\r\n";
        assert_eq!(extract_origin(req), Some("http://example.com".to_string()));
    }

    #[test]
    fn test_extract_referer_header() {
        let req = "GET /env HTTP/1.1\r\nReferer: http://app.test.com/page\r\n\r\n";
        assert_eq!(
            extract_origin(req),
            Some("http://app.test.com/page".to_string())
        );
    }

    #[test]
    fn test_extract_origin_none() {
        let req = "GET /env HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(extract_origin(req), None);
    }

    #[test]
    fn test_extract_host_from_origin() {
        assert_eq!(extract_host_from_origin("http://example.com:3000/path"), "example.com");
        assert_eq!(extract_host_from_origin("https://localhost:8080"), "localhost");
        assert_eq!(extract_host_from_origin("http://127.0.0.1:9000"), "127.0.0.1");
        assert_eq!(extract_host_from_origin("example.com"), "example.com");
    }

    #[test]
    fn test_origin_allowed_no_origin_header() {
        // No origin header → allowed (direct/curl request)
        assert!(is_origin_allowed(None, None));
        assert!(is_origin_allowed(None, Some(&["http://example.com".to_string()])));
    }

    #[test]
    fn test_origin_allowed_localhost_default() {
        // Localhost always allowed, even with no allowlist
        assert!(is_origin_allowed(Some("http://localhost:3000"), None));
        assert!(is_origin_allowed(Some("http://127.0.0.1:8080"), None));
    }

    #[test]
    fn test_origin_denied_non_localhost_default() {
        // Non-localhost denied when no allowlist set
        assert!(!is_origin_allowed(Some("http://evil.com"), None));
        assert!(!is_origin_allowed(Some("http://example.com"), None));
    }

    #[test]
    fn test_origin_allowed_with_allowlist() {
        let allowed = vec!["http://myapp.com".to_string()];
        assert!(is_origin_allowed(Some("http://myapp.com:443"), Some(&allowed)));
        // Loopback still allowed
        assert!(is_origin_allowed(Some("http://localhost:3000"), Some(&allowed)));
    }

    #[test]
    fn test_origin_denied_not_in_allowlist() {
        let allowed = vec!["http://myapp.com".to_string()];
        assert!(!is_origin_allowed(Some("http://evil.com"), Some(&allowed)));
    }

    #[test]
    fn test_origin_case_insensitive() {
        let allowed = vec!["http://MyApp.COM".to_string()];
        assert!(is_origin_allowed(Some("http://myapp.com"), Some(&allowed)));
        assert!(is_origin_allowed(Some("http://MYAPP.COM"), Some(&allowed)));
    }

    #[test]
    fn test_loopback_always_allowed() {
        // Even with a restrictive allowlist, loopback is always permitted
        let allowed = vec!["http://only-this.com".to_string()];
        assert!(is_origin_allowed(Some("http://127.0.0.1:5000"), Some(&allowed)));
        assert!(is_origin_allowed(Some("http://localhost:5000"), Some(&allowed)));
    }

    // ─── Build audit entry tests ──────────────────────────

    #[test]
    fn test_build_audit_entry_health() {
        let env = sample_env();
        let entry = build_audit_entry("/health", "200 OK", "127.0.0.1:12345", None, &env, None);
        assert_eq!(entry.action, "health");
        assert!(entry.granted);
        assert!(entry.key.is_none());
    }

    #[test]
    fn test_build_audit_entry_env_all() {
        let env = sample_env();
        let entry = build_audit_entry("/env", "200 OK", "127.0.0.1:12345", None, &env, None);
        assert_eq!(entry.action, "access");
        assert_eq!(entry.keys_served, Some(3));
        assert!(entry.granted);
    }

    #[test]
    fn test_build_audit_entry_env_key() {
        let env = sample_env();
        let entry = build_audit_entry(
            "/env/API_KEY",
            "200 OK",
            "127.0.0.1:12345",
            Some("test-agent".to_string()),
            &env,
            None,
        );
        assert_eq!(entry.action, "access");
        assert_eq!(entry.key, Some("API_KEY".to_string()));
        assert!(entry.granted);
        assert_eq!(entry.user_agent, Some("test-agent".to_string()));
    }

    #[test]
    fn test_build_audit_entry_denied() {
        let env = sample_env();
        let allowed = vec!["APP_NAME".to_string()];
        let entry = build_audit_entry(
            "/env/API_KEY",
            "403 Forbidden",
            "127.0.0.1:12345",
            None,
            &env,
            Some(&allowed),
        );
        assert_eq!(entry.action, "access");
        assert_eq!(entry.key, Some("API_KEY".to_string()));
        assert!(!entry.granted);
    }

    #[test]
    fn test_build_audit_entry_unknown_path() {
        let env = sample_env();
        let entry =
            build_audit_entry("/unknown", "404 Not Found", "127.0.0.1:12345", None, &env, None);
        assert_eq!(entry.action, "denied");
        assert!(!entry.granted);
    }

    #[test]
    fn test_extract_user_agent() {
        let req =
            "GET /env HTTP/1.1\r\nHost: localhost\r\nUser-Agent: curl/8.4.0\r\n\r\n";
        assert_eq!(extract_user_agent(req), Some("curl/8.4.0".to_string()));
    }

    #[test]
    fn test_extract_user_agent_none() {
        let req = "GET /env HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(extract_user_agent(req), None);
    }
}
