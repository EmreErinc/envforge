//! Input value object for resolver: one server's section of an MCP config.

use std::collections::HashMap;

use serde::Deserialize;

use crate::ops::mcp_pin::types::Transport;

/// One server's section of an MCP config file (e.g. one entry in
/// `~/.cursor/mcp.json`'s `mcpServers` map).
///
/// Parsed from MCP config JSON; consumed by `Resolver::resolve`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct McpConfigFragment {
    pub name: String,

    #[serde(default)]
    pub command: Option<String>,

    #[serde(default)]
    pub args: Option<Vec<String>>,

    #[serde(default)]
    pub transport: Option<Transport>,

    #[serde(default)]
    pub url: Option<String>,

    /// MCP config may carry env hints; not consumed by this unit.
    /// Reserved for Unit 005 cross-feature usage.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

impl McpConfigFragment {
    pub fn effective_transport(&self) -> Transport {
        self.transport.unwrap_or(Transport::Stdio)
    }
}
