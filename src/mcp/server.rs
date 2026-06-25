//! EnvForge MCP server — read-safe tools.
//!
//! Boots an MCP server over stdio, completes the initialize handshake, and
//! serves two read-safe tools that expose env-var metadata to AI agents without
//! ever returning raw secret values.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! envforge::mcp::server::serve_stdio().await
//! # }
//! ```

use std::path::Path;

use rmcp::{
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use serde::Serialize;

use crate::ops::{
    dotenv::parse_dotenv,
    monitor::{emit_event, EventSource, RuntimeEvent, SecuritySeverity},
    redact::redact_for_label,
    schema::parse_schema,
};

/// Build an audit message for an MCP tool invocation.
///
/// The message contains ONLY the static tool name — no env data, no key values,
/// no secrets. Sec-ops greps `"MCP "` to find all MCP access events.
///
/// # Examples
///
/// ```
/// # use envforge::mcp::server::audit_message;
/// assert_eq!(audit_message("list_keys"), "MCP list_keys");
/// assert_eq!(audit_message("describe_schema"), "MCP describe_schema");
/// ```
#[must_use]
pub fn audit_message(tool: &str) -> String {
    format!("MCP {tool}")
}

// ── Inner functions (pure, take explicit dir — testable without cwd mutation) ──

/// Collect the union of key names from schema variables and `.env` entries,
/// sorted and deduplicated. No values are included.
pub fn collect_key_names(dir: &Path) -> Vec<String> {
    let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // (a) Schema variable names.
    let schema_path = {
        let mut found = None;
        for name in [".env.schema.toml", ".env.schema"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                found = Some(candidate);
                break;
            }
        }
        found
    };
    if let Some(path) = schema_path {
        if let Ok(schema) = parse_schema(&path) {
            for key in schema.variables.keys() {
                keys.insert(key.clone());
            }
        }
    }

    // (b) Keys present in `.env`.
    let dotenv_path = dir.join(".env");
    if let Ok(entries) = parse_dotenv(&dotenv_path) {
        for entry in entries {
            keys.insert(entry.key);
        }
    }

    keys.into_iter().collect()
}

/// Per-variable metadata returned by `describe_schema`.
///
/// `current_value` is `Some("***")` when the key is set in `.env`, or `None`
/// when it is absent. The raw value is never included.
#[derive(Debug, Serialize)]
pub struct VarDesc {
    pub key: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub required: bool,
    pub default: Option<String>,
    pub example: Option<String>,
    pub description: Option<String>,
    pub sensitive: bool,
    pub current_value: Option<String>,
}

/// Collect per-variable metadata for all schema variables.
///
/// `current_value` is the redacted label `"***"` when the key is present in
/// `.env`, otherwise `null`. Raw values are never read or returned.
pub fn describe(dir: &Path) -> Vec<VarDesc> {
    // Determine which keys are set in `.env` (keys only — values discarded).
    let dotenv_path = dir.join(".env");
    let set_keys: std::collections::HashSet<String> = parse_dotenv(&dotenv_path)
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.key)
        .collect();

    // Load schema; if none, return empty list.
    let schema_path = {
        let mut found = None;
        for name in [".env.schema.toml", ".env.schema"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                found = Some(candidate);
                break;
            }
        }
        found
    };
    let schema = match schema_path.as_deref().and_then(|p| parse_schema(p).ok()) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut vars: Vec<VarDesc> = schema
        .variables
        .into_iter()
        .map(|(key, var)| {
            // current_value: redacted label if set, null if not. Never the raw value.
            let current_value = if set_keys.contains(&key) {
                Some(redact_for_label("", var.sensitive))
            } else {
                None
            };
            VarDesc {
                key,
                var_type: var.var_type.display().to_owned(),
                required: var.required,
                default: var.default,
                example: var.example,
                description: var.description,
                sensitive: var.sensitive,
                current_value,
            }
        })
        .collect();

    vars.sort_by(|a, b| a.key.cmp(&b.key));
    vars
}

/// Handler struct for the EnvForge MCP server.
///
/// Provides server metadata, capabilities, and two read-safe tools that expose
/// environment-variable metadata to AI agents without ever returning raw values.
pub struct EnvForgeMcp;

#[tool_router]
impl EnvForgeMcp {
    /// Return the set of env-var key NAMES known to this project.
    ///
    /// Sources: schema variable names ∪ keys present in `.env`. Sorted,
    /// deduplicated. No values are included.
    #[tool(
        name = "list_keys",
        description = "Return the set of env-var key names known to this project (no values). Sources: schema variables ∪ .env keys. Sorted, deduplicated."
    )]
    async fn list_keys(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        emit_event(RuntimeEvent {
            source: EventSource::Manual,
            key: None,
            message: audit_message("list_keys"),
            timestamp: chrono::Utc::now(),
            severity: SecuritySeverity::Info,
        });
        let dir = std::env::current_dir().map_err(|e| {
            rmcp::ErrorData::internal_error(
                format!("cannot determine working directory: {e}"),
                None,
            )
        })?;
        let keys = collect_key_names(&dir);
        let payload = serde_json::json!({ "keys": keys });
        let text = serde_json::to_string(&payload).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("JSON serialization failed: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    /// Return per-key metadata for schema variables.
    ///
    /// For each variable: key, type, required, default, example, description,
    /// sensitive, and current_value — where current_value is `"***"` (redacted)
    /// if the key is present in `.env`, otherwise `null`. Raw values are never
    /// returned.
    #[tool(
        name = "describe_schema",
        description = "Return per-key metadata for schema variables. current_value is '***' if set in .env, null otherwise. Raw secret values are never returned."
    )]
    async fn describe_schema(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        emit_event(RuntimeEvent {
            source: EventSource::Manual,
            key: None,
            message: audit_message("describe_schema"),
            timestamp: chrono::Utc::now(),
            severity: SecuritySeverity::Info,
        });
        let dir = std::env::current_dir().map_err(|e| {
            rmcp::ErrorData::internal_error(
                format!("cannot determine working directory: {e}"),
                None,
            )
        })?;
        let variables = describe(&dir);
        let payload = serde_json::json!({ "variables": variables });
        let text = serde_json::to_string(&payload).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("JSON serialization failed: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for EnvForgeMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("envforge", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "EnvForge MCP server — read-safe environment variable metadata for AI agents.",
            )
    }
}

/// Start the EnvForge MCP server over stdio and wait until the client disconnects.
///
/// This function blocks until the connection is closed or an error occurs.
///
/// # Errors
///
/// Returns an error if the MCP handshake fails or the server task panics.
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let service = EnvForgeMcp
        .serve(stdio())
        .await
        .map_err(|e| format!("MCP server failed to initialize: {e}"))?;
    service
        .waiting()
        .await
        .map_err(|e| format!("MCP server task error: {e}"))?;
    Ok(())
}
