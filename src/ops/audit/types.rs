//! Unified audit event types for AI audit trail.
//!
//! Defines the unified [`AuditEvent`] type that bridges all audit sources
//! (AI Guard, Proxy, Sync, CLI, TUI, Hooks) into a single event model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

// ─── Identifiers ─────────────────────────────────────────────────

/// Unique identifier for each audit event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl Default for EventId {
    fn default() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl EventId {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session identifier — groups related audit events.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl Default for SessionId {
    fn default() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl SessionId {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Enums ───────────────────────────────────────────────────────

/// Classification of the audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    ContextCreated,
    SecretBound,
    SecretAccessed,
    SecretExposure,
    AccessDenied,
    SessionStarted,
    SessionEnded,
    SyncPush,
    SyncPull,
    ConfigChange,
    IntegrityAlert,
}

/// Which subsystem produced this event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventSource {
    AiGuard,
    Proxy,
    Sync,
    Cli,
    Tui,
    Hook,
}

/// Outcome of the audited action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventResult {
    Success,
    Failure(String),
    Denied(String),
    Warning(String),
}

// ─── Unified Event ───────────────────────────────────────────────

/// The unified audit event — single type for all audit sources.
///
/// Contains all context fields needed for querying, filtering,
/// chain of custody, and compliance reporting.  Never contains
/// secret **values** — only key names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub source: EventSource,
    pub result: EventResult,

    /// AI tool session context.
    pub session_id: Option<SessionId>,
    pub tool_type: Option<String>,

    /// Secret context (key name only — never the value).
    pub secret_key: Option<String>,
    pub operation: Option<String>,

    /// Cryptographic integrity fields (set by tamper-evident writer).
    pub prev_hash: Option<String>,
    pub entry_hash: String,

    /// Extensible metadata (file paths, user agents, message fragments).
    pub metadata: serde_json::Value,
}

impl AuditEvent {
    /// Create a new event with a fresh ID and current timestamp.
    #[must_use]
    pub fn new(event_type: EventType, source: EventSource, result: EventResult) -> Self {
        let id = EventId::new();
        let timestamp = Utc::now();
        Self {
            id,
            timestamp,
            event_type,
            source,
            result,
            session_id: None,
            tool_type: None,
            secret_key: None,
            operation: None,
            prev_hash: None,
            entry_hash: String::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Set session context.
    #[must_use]
    pub fn with_session(mut self, session_id: SessionId, tool_type: Option<String>) -> Self {
        self.session_id = Some(session_id);
        self.tool_type = tool_type;
        self
    }

    /// Set secret context.
    #[must_use]
    pub fn with_secret(mut self, key: String, operation: String) -> Self {
        self.secret_key = Some(key);
        self.operation = Some(operation);
        self
    }

    /// Add metadata key-value pair.
    pub fn add_metadata(&mut self, key: &str, value: serde_json::Value) {
        if let serde_json::Value::Object(ref mut map) = self.metadata {
            map.insert(key.to_string(), value);
        }
    }

    /// Compute the entry hash (SHA-256 of this event's content, excluding hashes).
    /// This is set by [`super::tamper::TamperEvidentWriter`].
    pub fn compute_entry_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.id.0.as_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        // Serialize enum variants as strings for stability
        hasher.update(format!("{:?}", self.event_type).as_bytes());
        hasher.update(format!("{:?}", self.source).as_bytes());
        if let Some(ref sid) = self.session_id {
            hasher.update(sid.0.as_bytes());
        }
        if let Some(ref tool) = self.tool_type {
            hasher.update(tool.as_bytes());
        }
        if let Some(ref key) = self.secret_key {
            hasher.update(key.as_bytes());
        }
        if let Some(ref op) = self.operation {
            hasher.update(op.as_bytes());
        }
        if let Some(ref prev) = self.prev_hash {
            hasher.update(prev.as_bytes());
        }
        hasher.update(
            serde_json::to_string(&self.result)
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(
            serde_json::to_string(&self.metadata)
                .unwrap_or_default()
                .as_bytes(),
        );
        bytes_to_hex(&hasher.finalize())
    }

    /// Copy an event, preserving all fields except hashes.
    #[must_use]
    pub fn duplicate_for_chain(&self) -> Self {
        let mut clone = self.clone();
        clone.prev_hash = None;
        clone.entry_hash = String::new();
        clone
    }
}

#[inline]
fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

// ─── Error ───────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialize error: {0}")]
    JsonSerialize(#[from] serde_json::Error),

    #[error("integrity verification failed for event {0}: hash chain broken")]
    IntegrityViolation(String),

    #[error("event not found: {0}")]
    EventNotFound(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),
}
