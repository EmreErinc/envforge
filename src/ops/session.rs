use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Utc;

use crate::model::{
    AiTool, Session, SessionConfig, SessionError, SessionId, SessionState, SessionSummary,
};

/// Thread-safe in-memory session manager.
pub struct SessionManager {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Start a new session.
    pub fn create_session(
        &self,
        tool_type: AiTool,
        ttl_seconds: u64,
    ) -> Result<Session, SessionError> {
        let now = Utc::now();
        let expires = now + chrono::Duration::seconds(ttl_seconds as i64);
        let session = Session {
            id: SessionId::new(),
            tool_type,
            state: SessionState::Active,
            created_at: now,
            expires_at: expires,
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::Internal(format!("Failed to lock sessions: {}", e)))?;

        sessions.insert(session.id.as_str().to_string(), session.clone());
        Ok(session)
    }

    /// Stop (expire) a session by ID.
    pub fn stop_session(&self, id: &str) -> Result<Session, SessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| SessionError::Internal(format!("Failed to lock sessions: {}", e)))?;

        let session = sessions
            .get_mut(id)
            .ok_or_else(|| SessionError::NotFound(id.to_string()))?;

        session.state = SessionState::Expired;
        Ok(session.clone())
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Option<Session> {
        let sessions = self.sessions.lock().ok()?;
        sessions.get(id).cloned()
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        let sessions = match self.sessions.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        sessions
            .values()
            .map(|s| SessionSummary {
                id: s.id.clone(),
                tool_type: s.tool_type,
                state: s.state,
                created_at: s.created_at,
                expires_at: s.expires_at,
                remaining_seconds: s.remaining_seconds(),
            })
            .collect()
    }

    /// Remove expired sessions.
    pub fn cleanup_expired(&self) -> usize {
        let mut sessions = match self.sessions.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };

        let before = sessions.len();
        sessions.retain(|_, s| !s.is_expired());
        before - sessions.len()
    }

    /// Count active (non-expired) sessions.
    pub fn active_count(&self) -> usize {
        let sessions = match self.sessions.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };

        sessions.values().filter(|s| !s.is_expired()).count()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect which AI tool is running in the current process context.
pub fn detect_ai_tool() -> AiTool {
    let env = std::env::vars().collect::<HashMap<String, String>>();

    if env.contains_key("CLAUDE_CODE") || env.contains_key("ANTHROPIC_API_KEY") {
        return AiTool::ClaudeCode;
    }
    if env.contains_key("CURSOR") || env.contains_key("CURSOR_APP") {
        return AiTool::Cursor;
    }
    if env.contains_key("GITHUB_COPILOT") || env.contains_key("COPILOT") {
        return AiTool::GitHubCopilot;
    }

    // Check parent process name heuristics (best-effort).
    if let Ok(cmdline) = std::fs::read_to_string("/proc/self/cmdline") {
        let lowered = cmdline.to_lowercase();
        if lowered.contains("claude") {
            return AiTool::ClaudeCode;
        }
        if lowered.contains("cursor") {
            return AiTool::Cursor;
        }
        if lowered.contains("copilot") {
            return AiTool::GitHubCopilot;
        }
    }

    AiTool::Unknown
}

/// Parse a human-readable duration string into seconds.
/// Supports: 1h, 30m, 8h, 24h, 7d, 1w.
pub fn parse_ttl(ttl: &str) -> Result<u64, String> {
    let trimmed = ttl.trim();
    if trimmed.is_empty() {
        return Ok(SessionConfig::default().default_ttl_seconds);
    }

    let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit_str: String = trimmed.chars().skip_while(|c| c.is_ascii_digit()).collect();

    let num: u64 = num_str
        .parse()
        .map_err(|_| format!("Invalid TTL number in '{}'", trimmed))?;

    match unit_str.as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => Ok(num),
        "m" | "min" | "mins" | "minute" | "minutes" => Ok(num * 60),
        "h" | "hr" | "hrs" | "hour" | "hours" => Ok(num * 3600),
        "d" | "day" | "days" => Ok(num * 86400),
        "w" | "week" | "weeks" => Ok(num * 604800),
        "" => Ok(num * 3600), // bare number = hours
        _ => Err(format!("Unknown TTL unit '{}' in '{}'", unit_str, trimmed)),
    }
}

/// Format seconds as a human-readable duration.
pub fn format_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return "expired".to_string();
    }
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if secs > 0 {
        parts.push(format!("{}s", secs));
    }

    if parts.is_empty() {
        "0s".to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ttl() {
        assert_eq!(parse_ttl("1h").unwrap(), 3600);
        assert_eq!(parse_ttl("30m").unwrap(), 1800);
        assert_eq!(parse_ttl("1d").unwrap(), 86400);
        assert_eq!(parse_ttl("1w").unwrap(), 604800);
        assert_eq!(parse_ttl("3600").unwrap(), 3600 * 3600); // bare number = hours
        assert_eq!(parse_ttl("").unwrap(), 3600); // default
    }

    #[test]
    fn test_session_lifecycle() {
        let manager = SessionManager::new();
        let session = manager.create_session(AiTool::ClaudeCode, 3600).unwrap();
        assert_eq!(session.tool_type, AiTool::ClaudeCode);
        assert_eq!(session.state, SessionState::Active);
        assert!(session.remaining_seconds() > 0);

        let found = manager.get_session(session.id.as_str());
        assert!(found.is_some());

        let stopped = manager.stop_session(session.id.as_str()).unwrap();
        assert_eq!(stopped.state, SessionState::Expired);

        let list = manager.list_sessions();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, SessionState::Expired);
    }

    #[test]
    fn test_cleanup_expired() {
        let manager = SessionManager::new();
        let s1 = manager.create_session(AiTool::Cursor, 1).unwrap();
        let s2 = manager.create_session(AiTool::ClaudeCode, 3600).unwrap();

        // s1 expires after 1 second
        std::thread::sleep(std::time::Duration::from_secs(2));

        let removed = manager.cleanup_expired();
        assert_eq!(removed, 1);

        assert!(manager.get_session(s1.id.as_str()).is_none());
        assert!(manager.get_session(s2.id.as_str()).is_some());
    }

    #[test]
    fn test_tool_from_str() {
        assert_eq!("claude-code".parse::<AiTool>().unwrap(), AiTool::ClaudeCode);
        assert_eq!("cursor".parse::<AiTool>().unwrap(), AiTool::Cursor);
        assert_eq!("copilot".parse::<AiTool>().unwrap(), AiTool::GitHubCopilot);
        assert!("unknown".parse::<AiTool>().is_err());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(0), "expired");
        assert_eq!(format_duration(-1), "expired");
    }
}
