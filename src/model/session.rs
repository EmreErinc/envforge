use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// AI coding tool type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub enum AiTool {
    ClaudeCode,
    GitHubCopilot,
    Cursor,
    Unknown,
}

impl AiTool {
    pub fn as_str(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => "Claude Code",
            AiTool::GitHubCopilot => "GitHub Copilot",
            AiTool::Cursor => "Cursor",
            AiTool::Unknown => "Unknown",
        }
    }

    pub fn env_var_hint(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => "CLAUDE_CODE",
            AiTool::GitHubCopilot => "GITHUB_COPILOT",
            AiTool::Cursor => "CURSOR",
            AiTool::Unknown => "",
        }
    }
}

impl std::fmt::Display for AiTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AiTool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace(['-', '_', ' '], "").as_str() {
            "claudecode" => Ok(AiTool::ClaudeCode),
            "githubcopilot" | "copilot" => Ok(AiTool::GitHubCopilot),
            "cursor" => Ok(AiTool::Cursor),
            _ => Err(format!(
                "Unknown AI tool '{}'. Supported: claude-code, cursor, copilot",
                s
            )),
        }
    }
}

/// Session lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub enum SessionState {
    Active,
    Expired,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Active => write!(f, "Active"),
            SessionState::Expired => write!(f, "Expired"),
        }
    }
}

/// A session representing an AI tool invocation context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub tool_type: AiTool,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at || matches!(self.state, SessionState::Expired)
    }

    pub fn remaining_seconds(&self) -> i64 {
        let now = Utc::now();
        if now > self.expires_at {
            0
        } else {
            (self.expires_at - now).num_seconds()
        }
    }
}

/// Summary for listing sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub tool_type: AiTool,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub remaining_seconds: i64,
}

/// Default configuration for sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub default_ttl_seconds: u64,
    pub auto_cleanup: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 3600, // 1 hour
            auto_cleanup: true,
        }
    }
}

/// Error type for session operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Invalid tool type: {0}")]
    InvalidTool(String),

    #[error("Session expired: {0}")]
    Expired(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
