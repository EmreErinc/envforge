//! Tests for the EnvForge MCP server (Story 2.1 skeleton + Story 2.3 tools + Story 2.4 audit).
//!
//! These tests are compiled and run only when the `mcp-server` feature is enabled:
//!
//! ```bash
//! cargo test --features mcp-server --test mcp_server_tests
//! ```

#![cfg(feature = "mcp-server")]

use envforge::mcp::server::{audit_message, collect_key_names, describe, EnvForgeMcp};
use rmcp::ServerHandler;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ── Story 2.3 helpers ────────────────────────────────────────────────────────

/// Create a temporary project directory with a schema and a `.env` file.
///
/// Schema declares:
/// - `DATABASE_URL` (url, required, non-sensitive, with description)
/// - `STRIPE_KEY`   (string, required, sensitive, with example)
/// - `APP_DEBUG`    (bool, optional, non-sensitive, with default)
///
/// `.env` sets only `DATABASE_URL` to a fake value ("postgres://localhost/testdb").
fn make_project_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let schema = r#"
[DATABASE_URL]
type = "url"
required = true
description = "Primary database connection string"
sensitive = false

[STRIPE_KEY]
type = "string"
required = true
sensitive = true
example = "sk_test_EXAMPLE"

[APP_DEBUG]
type = "bool"
required = false
sensitive = false
default = "false"
"#;
    let dotenv = "DATABASE_URL=postgres://localhost/testdb\n";
    std::fs::write(dir.path().join(".env.schema.toml"), schema).expect("write schema");
    std::fs::write(dir.path().join(".env"), dotenv).expect("write .env");
    dir
}

/// Verifies that `EnvForgeMcp::get_info` returns the expected server name and
/// that capabilities expose tools (matching the skeleton configuration).
#[test]
fn test_server_info_metadata() {
    let handler = EnvForgeMcp;
    let info = handler.get_info();

    assert_eq!(
        info.server_info.name, "envforge",
        "server name must be 'envforge'"
    );
    assert_eq!(
        info.server_info.version,
        env!("CARGO_PKG_VERSION"),
        "server version must match crate version"
    );
    assert!(
        info.capabilities.tools.is_some(),
        "tools capability must be present in the skeleton"
    );
    assert!(
        info.instructions.is_some(),
        "instructions must be set for AI agent guidance"
    );
}

/// Verifies the MCP initialize handshake end-to-end using a duplex transport.
///
/// The test drives the client side by writing raw JSON-RPC over the stdio
/// (line-delimited newline-terminated JSON), without requiring the rmcp
/// `client` feature. It asserts that:
///
/// 1. The server responds to `initialize` with an `InitializeResult`.
/// 2. The result carries `serverInfo.name == "envforge"`.
/// 3. The server shuts down cleanly after the transport closes.
#[tokio::test]
async fn test_handshake_over_duplex_transport() {
    use rmcp::ServiceExt;

    let (server_transport, client_transport) = tokio::io::duplex(8192);

    // Spawn the server.
    let server_handle = tokio::spawn(async move { EnvForgeMcp.serve(server_transport).await });

    // Split the client half.
    let (client_read, mut client_write) = tokio::io::split(client_transport);
    let mut client_reader = BufReader::new(client_read);

    // Send: initialize request (newline-delimited JSON per MCP stdio transport).
    let init_req =
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.0.1\"}}}\n";
    client_write
        .write_all(init_req.as_bytes())
        .await
        .expect("write init request");

    // Receive the initialize response line.
    let mut response_line = String::new();
    client_reader
        .read_line(&mut response_line)
        .await
        .expect("read init response");

    let response: serde_json::Value =
        serde_json::from_str(response_line.trim()).expect("valid JSON response");

    // Assert it is a successful InitializeResult with our server name.
    assert_eq!(response["jsonrpc"], "2.0", "must be JSON-RPC 2.0");
    assert_eq!(response["id"], 1, "response id must match request id");
    assert!(
        response["error"].is_null(),
        "no error in initialize response: {response}"
    );
    let server_name = &response["result"]["serverInfo"]["name"];
    assert_eq!(
        server_name, "envforge",
        "serverInfo.name must be 'envforge', got: {server_name}"
    );

    // Send: initialized notification so the server enters main loop.
    let init_notif = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
    client_write
        .write_all(init_notif.as_bytes())
        .await
        .expect("write initialized notification");

    // Drop the client transport to trigger a clean server shutdown.
    drop(client_write);
    drop(client_reader);

    let running = server_handle
        .await
        .expect("server task did not panic")
        .expect("server handshake succeeded");

    running.cancel().await.expect("clean shutdown");
}

// ── Story 2.3: list_keys inner function ─────────────────────────────────────

/// list_keys returns schema keys ∪ .env keys, sorted and deduplicated.
#[test]
fn test_collect_key_names_union_sorted_deduped() {
    let dir = make_project_dir();
    let keys = collect_key_names(dir.path());

    // Schema declares DATABASE_URL, STRIPE_KEY, APP_DEBUG.
    // .env also sets DATABASE_URL (overlap → deduped).
    assert!(
        keys.contains(&"DATABASE_URL".to_string()),
        "DATABASE_URL must be present"
    );
    assert!(
        keys.contains(&"STRIPE_KEY".to_string()),
        "STRIPE_KEY must be present"
    );
    assert!(
        keys.contains(&"APP_DEBUG".to_string()),
        "APP_DEBUG must be present"
    );
    assert_eq!(keys.len(), 3, "no duplicates");

    // Verify sorted order.
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "keys must be sorted");
}

/// list_keys with no schema returns only .env keys.
#[test]
fn test_collect_key_names_no_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(".env"), "ONLY_IN_ENV=value\n").expect("write .env");
    let keys = collect_key_names(dir.path());
    assert_eq!(keys, vec!["ONLY_IN_ENV"]);
}

/// list_keys with no schema and no .env returns an empty list.
#[test]
fn test_collect_key_names_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let keys = collect_key_names(dir.path());
    assert!(keys.is_empty());
}

// ── Story 2.3: describe inner function ──────────────────────────────────────

/// describe returns per-var metadata; current_value is "***" for set keys and
/// None for unset keys.
#[test]
fn test_describe_current_value_redacted_or_null() {
    let dir = make_project_dir();
    let vars = describe(dir.path());

    let db = vars
        .iter()
        .find(|v| v.key == "DATABASE_URL")
        .expect("DATABASE_URL must be present");
    let stripe = vars
        .iter()
        .find(|v| v.key == "STRIPE_KEY")
        .expect("STRIPE_KEY must be present");
    let debug = vars
        .iter()
        .find(|v| v.key == "APP_DEBUG")
        .expect("APP_DEBUG must be present");

    // DATABASE_URL is set in .env → must be "***"
    assert_eq!(
        db.current_value.as_deref(),
        Some("***"),
        "set key must be redacted"
    );
    // STRIPE_KEY and APP_DEBUG are not set → must be None
    assert!(
        stripe.current_value.is_none(),
        "unset sensitive key current_value must be null"
    );
    assert!(
        debug.current_value.is_none(),
        "unset key current_value must be null"
    );
}

/// describe returns correct metadata fields.
#[test]
fn test_describe_metadata_fields() {
    let dir = make_project_dir();
    let vars = describe(dir.path());

    let db = vars
        .iter()
        .find(|v| v.key == "DATABASE_URL")
        .expect("DATABASE_URL");
    assert_eq!(db.var_type, "url");
    assert!(db.required);
    assert!(!db.sensitive);
    assert_eq!(
        db.description.as_deref(),
        Some("Primary database connection string")
    );

    let stripe = vars
        .iter()
        .find(|v| v.key == "STRIPE_KEY")
        .expect("STRIPE_KEY");
    assert_eq!(stripe.var_type, "string");
    assert!(stripe.required);
    assert!(stripe.sensitive);
    assert_eq!(stripe.example.as_deref(), Some("sk_test_EXAMPLE"));

    let debug = vars
        .iter()
        .find(|v| v.key == "APP_DEBUG")
        .expect("APP_DEBUG");
    assert_eq!(debug.var_type, "bool");
    assert!(!debug.required);
    assert_eq!(debug.default.as_deref(), Some("false"));
}

/// describe output sorted by key.
#[test]
fn test_describe_sorted_by_key() {
    let dir = make_project_dir();
    let vars = describe(dir.path());
    let keys: Vec<&str> = vars.iter().map(|v| v.key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "describe output must be sorted by key");
}

/// describe returns empty list when no schema exists.
#[test]
fn test_describe_no_schema_returns_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vars = describe(dir.path());
    assert!(vars.is_empty());
}

// ── Story 2.3: leak-guard (FR15) ────────────────────────────────────────────

/// The raw secret value stored in .env must NOT appear anywhere in the
/// serialized describe output. This is the primary FR15 safety assertion.
#[test]
fn test_describe_no_raw_secret_in_output() {
    let fake_secret = "postgres://localhost/testdb";

    let dir = make_project_dir();
    let vars = describe(dir.path());

    let json = serde_json::to_string(&vars).expect("serialize");
    assert!(
        !json.contains(fake_secret),
        "raw secret value must NOT appear in describe output, got: {json}"
    );
}

/// The raw secret value must also not appear in collect_key_names output.
#[test]
fn test_list_keys_no_raw_value_in_output() {
    let fake_secret = "postgres://localhost/testdb";

    let dir = make_project_dir();
    let keys = collect_key_names(dir.path());

    let json = serde_json::to_string(&keys).expect("serialize");
    assert!(
        !json.contains(fake_secret),
        "raw secret value must NOT appear in list_keys output, got: {json}"
    );
}

// ── Story 2.4: Audit logging (NFR-S3 / FR16) ────────────────────────────────

/// `audit_message` produces exactly `"MCP <tool>"` and contains no value-like
/// content — documents the value-free guarantee for sec-ops grep patterns.
#[test]
fn test_audit_message_excludes_values() {
    let msg_list = audit_message("list_keys");
    let msg_describe = audit_message("describe_schema");

    assert_eq!(msg_list, "MCP list_keys");
    assert_eq!(msg_describe, "MCP describe_schema");

    // Must start with the grep-able prefix.
    assert!(
        msg_list.starts_with("MCP "),
        "message must start with 'MCP '"
    );
    assert!(
        msg_describe.starts_with("MCP "),
        "message must start with 'MCP '"
    );

    // Must contain no '=' characters (no KEY=VALUE content).
    assert!(
        !msg_list.contains('='),
        "audit message must not contain '='"
    );
    assert!(
        !msg_describe.contains('='),
        "audit message must not contain '='"
    );

    // Must not contain any of the characters typical of secret values.
    for ch in ['/', ':', '@', '$', '+'] {
        assert!(
            !msg_list.contains(ch),
            "audit message must not contain secret-like char '{ch}'"
        );
        assert!(
            !msg_describe.contains(ch),
            "audit message must not contain secret-like char '{ch}'"
        );
    }
}

// ── Story 2.4: Property / fuzz test — no raw secret leaks (NFR-S1) ──────────

use proptest::prelude::*;

/// Strategy: UPPER_SNAKE key names, 1–16 chars, starting with A-Z.
fn key_strategy() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Z][A-Z0-9_]{0,15}").expect("valid key regex")
}

/// Strategy: mixed high-entropy and arbitrary values.
fn value_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // High-entropy token-like values (base64-ish, API keys, etc.)
        proptest::string::string_regex("[A-Za-z0-9+/=_-]{8,64}").expect("valid value regex"),
        // Arbitrary printable values (may be short)
        ".{1,40}",
    ]
}

/// A generated (key, value) pair with both strategies.
fn kv_strategy() -> impl Strategy<Value = (String, String)> {
    (key_strategy(), value_strategy())
}

/// Write a temp project dir with the given (key, value) pairs.
///
/// Schema declares every key. `.env` assigns every key its value.
/// The `sensitive` flag alternates: even-indexed keys are sensitive.
fn write_temp_project(pairs: &[(String, String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");

    // Build .env.schema.toml
    let mut schema = String::new();
    for (i, (key, _value)) in pairs.iter().enumerate() {
        let sensitive = if i % 2 == 0 { "true" } else { "false" };
        schema.push_str(&format!(
            "[{key}]\ntype = \"string\"\nrequired = true\nsensitive = {sensitive}\n\n"
        ));
    }
    std::fs::write(dir.path().join(".env.schema.toml"), &schema).expect("write schema");

    // Build .env
    let mut dotenv = String::new();
    for (key, value) in pairs {
        // Escape newlines in value so the .env parser sees one entry per line.
        let escaped = value.replace('\n', "\\n").replace('\r', "\\r");
        dotenv.push_str(&format!("{key}={escaped}\n"));
    }
    std::fs::write(dir.path().join(".env"), &dotenv).expect("write .env");

    dir
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For any randomly generated set of (key, value) pairs, neither
    /// `describe` nor `collect_key_names` serialized output must contain any
    /// raw secret value of length >= 8 (NFR-S1).
    ///
    /// Short values (< 8 chars) are skipped to avoid false positives from
    /// coincidental substrings (e.g. "true", "url", type names).
    #[test]
    fn prop_no_raw_secret_leaks_through_mcp_tools(
        // Generate 1–8 unique-key pairs. Dedup by key.
        raw_pairs in proptest::collection::vec(kv_strategy(), 1..=8)
    ) {
        // Dedup by key (last value wins — matches BTreeSet/HashMap behaviour).
        let mut seen = std::collections::BTreeMap::new();
        for (k, v) in raw_pairs {
            seen.insert(k, v);
        }
        let pairs: Vec<(String, String)> = seen.into_iter().collect();

        let dir = write_temp_project(&pairs);

        // Invoke both inner functions and serialize their output.
        let desc_json = serde_json::to_string(&describe(dir.path()))
            .expect("describe serialization");
        let keys_json = serde_json::to_string(&collect_key_names(dir.path()))
            .expect("collect_key_names serialization");

        // Assert: no raw value of length >= 8 appears in either output.
        for (_key, value) in &pairs {
            if value.len() < 8 {
                // Skip short values: too many coincidental substring matches.
                continue;
            }
            prop_assert!(
                !desc_json.contains(value.as_str()),
                "raw value appeared in describe output!\nvalue={value:?}\njson={desc_json}"
            );
            prop_assert!(
                !keys_json.contains(value.as_str()),
                "raw value appeared in list_keys output!\nvalue={value:?}\njson={keys_json}"
            );
        }
    }
}
