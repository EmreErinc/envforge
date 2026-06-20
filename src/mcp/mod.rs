//! MCP (Model Context Protocol) server for EnvForge.
//!
//! This module is compiled only when the `mcp-server` feature is enabled.
//! It exposes an MCP server over stdio that allows AI agents to query
//! read-safe environment variable metadata without accessing secret values.
//!
//! Enable with:
//! ```toml
//! envforge = { version = "*", features = ["mcp-server"] }
//! ```

pub mod server;
