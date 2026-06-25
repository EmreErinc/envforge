//! Coverage for `ops::proxy` pure request-handling helpers: `route_request`
//! status matrix, key-format validation, allowlist filtering, request-line
//! parsing, user-agent extraction, and lease-bypass logic.

use envforge::ops::proxy::{
    check_lease_for_request, extract_user_agent, parse_request_line, route_request,
};
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

// ---- route_request ---------------------------------------------------------

#[test]
fn test_route_request_rejects_non_get() {
    let (status, _) = route_request("POST", "/env", &env(&[]), None);
    assert!(status.starts_with("405"));
}

#[test]
fn test_route_request_health_ok() {
    let (status, body) = route_request("GET", "/health", &env(&[]), None);
    assert!(status.starts_with("200"));
    assert!(body.contains("ok"));
}

#[test]
fn test_route_request_env_empty_map_is_empty_object() {
    let (status, body) = route_request("GET", "/env", &env(&[]), None);
    assert!(status.starts_with("200"));
    assert_eq!(body, "{}");
}

#[test]
fn test_route_request_env_allowlist_filters() {
    let e = env(&[("A", "1"), ("SECRET", "x")]);
    let allowed = vec!["A".to_string()];
    let (status, body) = route_request("GET", "/env", &e, Some(&allowed));
    assert!(status.starts_with("200"));
    assert!(body.contains("\"A\""));
    assert!(!body.contains("SECRET"));
}

#[test]
fn test_route_request_env_key_paths() {
    let e = env(&[("API_KEY", "v")]);

    // Present key → 200 with value.
    let (s_ok, b_ok) = route_request("GET", "/env/API_KEY", &e, None);
    assert!(s_ok.starts_with("200"));
    assert!(b_ok.contains("API_KEY") && b_ok.contains("\"v\""));

    // Missing key → 404.
    assert!(route_request("GET", "/env/MISSING", &e, None)
        .0
        .starts_with("404"));

    // Empty key name → 400.
    assert!(route_request("GET", "/env/", &e, None).0.starts_with("400"));

    // Invalid key format (dash) → 400.
    assert!(route_request("GET", "/env/API-KEY", &e, None)
        .0
        .starts_with("400"));

    // Present but not in allowlist → 403.
    let allowed = vec!["OTHER".to_string()];
    assert!(route_request("GET", "/env/API_KEY", &e, Some(&allowed))
        .0
        .starts_with("403"));
}

#[test]
fn test_route_request_unknown_path_404() {
    assert!(route_request("GET", "/nope", &env(&[]), None)
        .0
        .starts_with("404"));
}

// ---- parse_request_line ----------------------------------------------------

#[test]
fn test_parse_request_line() {
    assert_eq!(parse_request_line("GET /env HTTP/1.1"), ("GET", "/env"));
    // Malformed / empty falls back to ("", "/").
    assert_eq!(parse_request_line(""), ("", "/"));
}

// ---- extract_user_agent ----------------------------------------------------

#[test]
fn test_extract_user_agent() {
    let req = "GET / HTTP/1.1\r\nUser-Agent: curl/8.0\r\n\r\n";
    assert_eq!(extract_user_agent(req).as_deref(), Some("curl/8.0"));
    assert!(extract_user_agent("GET / HTTP/1.1\r\n\r\n").is_none());
}

// ---- check_lease_for_request -----------------------------------------------

#[test]
fn test_check_lease_for_request_bypass_paths() {
    // Lease not required → always granted (None).
    assert!(check_lease_for_request("/env", false).is_none());
    assert!(check_lease_for_request("/env/KEY", false).is_none());
    // Health endpoint is always accessible even when leases are required.
    assert!(check_lease_for_request("/health", true).is_none());
}
