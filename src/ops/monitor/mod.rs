//! Monitor subsystem for real-time secret monitoring.
//!
//! Provides data types, error types, event bus for streaming secret access events,
//! health probes for infrastructure verification, and the fingerprinting subsystem
//! for AI tool behavioral analysis and trust management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use thiserror::Error;
use tokio::sync::broadcast;

// ─── Re-exports ──────────────────────────────────────────────────────────────

pub mod fingerprint;
pub mod health;

pub use fingerprint::{FingerprintGenerator, IdentityVerifier, TrustManager};

// ─── Tool Type ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolType {
    ClaudeCode,
    GitHubCopilot,
    Cursor,
    Codeium,
    Tabnine,
    Unknown(String),
}

impl ToolType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::GitHubCopilot => "github-copilot",
            Self::Cursor => "cursor",
            Self::Codeium => "codeium",
            Self::Tabnine => "tabnine",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

impl fmt::Display for ToolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for ToolType {
    fn from(s: &str) -> Self {
        match s {
            "claude-code" => Self::ClaudeCode,
            "github-copilot" => Self::GitHubCopilot,
            "cursor" => Self::Cursor,
            "codeium" => Self::Codeium,
            "tabnine" => Self::Tabnine,
            _ => Self::Unknown(s.to_string()),
        }
    }
}

impl From<String> for ToolType {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl Serialize for ToolType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from(s.as_str()))
    }
}

// ─── Monitor Event ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorEvent {
    pub tool_type: ToolType,
    pub secret_key: String,
    pub operation: String,
    pub timestamp: DateTime<Utc>,
}

// ─── Event Source ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    Proxy,
    AiGuard,
    Fence,
    Canary,
    Scanner,
    Provider,
    Reveal,
    Manual,
    UnsafeArgv,
    KeyProvisioning,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proxy => write!(f, "proxy"),
            Self::AiGuard => write!(f, "ai-guard"),
            Self::Fence => write!(f, "fence"),
            Self::Canary => write!(f, "canary"),
            Self::Scanner => write!(f, "scanner"),
            Self::Provider => write!(f, "provider"),
            Self::Reveal => write!(f, "reveal"),
            Self::Manual => write!(f, "manual"),
            Self::UnsafeArgv => write!(f, "unsafe-argv"),
            Self::KeyProvisioning => write!(f, "key-provisioning"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum SecuritySeverity {
    #[default]
    Info,
    Warn,
    Critical,
}

impl fmt::Display for SecuritySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub source: EventSource,
    pub key: Option<String>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub severity: SecuritySeverity,
}

// ─── Health Check ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    pub name: String,
    pub category: String,
    pub status: HealthStatus,
    pub message: String,
    pub latency_ms: Option<u64>,
}

// ─── Event Bus ────────────────────────────────────────────────────────────────

static EVENT_BUS: OnceLock<broadcast::Sender<RuntimeEvent>> = OnceLock::new();
static BUS_ENABLED: AtomicBool = AtomicBool::new(false);
static AUDIT_LOG_STARTED: AtomicBool = AtomicBool::new(false);

pub fn init_event_bus(capacity: usize) {
    let (tx, _) = broadcast::channel(capacity.max(64));
    let _ = EVENT_BUS.set(tx);
    BUS_ENABLED.store(true, Ordering::SeqCst);
}

pub fn emit_event(event: RuntimeEvent) {
    let redacted = redact_runtime_event(event);
    if let Some(tx) = EVENT_BUS.get() {
        let _ = tx.send(redacted.clone());
    }
    if AUDIT_LOG_STARTED.load(Ordering::SeqCst) {
        write_audit_entry(&redacted);
    }
}

/// Emit a security-classified event with explicit severity.
/// Convenience wrapper — sets source and auto-populates timestamp.
pub fn emit_security_event(
    source: EventSource,
    severity: SecuritySeverity,
    key: Option<&str>,
    message: impl Into<String>,
) {
    emit_event(RuntimeEvent {
        source,
        key: key.map(|s| s.to_string()),
        message: message.into(),
        timestamp: Utc::now(),
        severity,
    });
}

fn resolve_audit_log_path() -> std::path::PathBuf {
    crate::config::config_dir()
        .map(|d| d.join("audit.jsonl"))
        .unwrap_or_else(|_| std::path::PathBuf::from("audit.jsonl"))
}

fn write_audit_entry(event: &RuntimeEvent) {
    static AUDIT_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    let log_path = AUDIT_PATH.get_or_init(resolve_audit_log_path);

    if let Ok(mut json) = serde_json::to_string(event) {
        json.push('\n');
        if let Ok(meta) = std::fs::metadata(log_path) {
            if meta.len() > 10 * 1024 * 1024 {
                let old = log_path.with_extension("jsonl.old");
                let _ = std::fs::rename(log_path, &old);
            }
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        if let Ok(mut f) = opts.open(log_path) {
            use std::io::Write;
            let _ = f.write_all(json.as_bytes());
            let _ = f.flush();
        }
    }
}

pub fn start_persistent_audit_log() {
    AUDIT_LOG_STARTED.store(true, Ordering::SeqCst);
    init_event_bus(1024);
}

pub fn audit_log_path() -> Option<std::path::PathBuf> {
    Some(resolve_audit_log_path())
}

pub fn read_audit_entries(limit: usize) -> Result<Vec<RuntimeEvent>, String> {
    let path = resolve_audit_log_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read audit log: {}", e))?;
    let mut events: Vec<RuntimeEvent> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<RuntimeEvent>(line) {
            events.push(event);
        }
    }
    if events.len() > limit {
        events = events.split_off(events.len() - limit);
    }
    Ok(events)
}

fn redact_runtime_event(mut event: RuntimeEvent) -> RuntimeEvent {
    event.message = redact_message(&event.message);
    event
}

fn redact_message(msg: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| regex::Regex::new(r"[A-Za-z0-9_\-]{24,}").expect("static regex compiles"));
    re.replace_all(msg, "[REDACTED]").into_owned()
}

pub fn subscribe_events() -> Option<broadcast::Receiver<RuntimeEvent>> {
    EVENT_BUS.get().map(|tx| tx.subscribe())
}

pub fn is_bus_enabled() -> bool {
    BUS_ENABLED.load(Ordering::SeqCst)
}

// ─── Fingerprint ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFingerprint {
    pub tool_type: ToolType,
    pub behavioral_signature: String,
    pub created_at: DateTime<Utc>,
    pub confidence: f64,
}

// ─── Trust ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrustScore {
    pub score: f64,
    pub confidence: f64,
    pub last_updated: DateTime<Utc>,
    pub sample_size: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrustEvent {
    PositiveVerification,
    NegativeVerification,
    SuspiciousBehavior,
    NormalBehavior,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerificationResult {
    Match,
    Mismatch { confidence: f64, divergence: String },
    InsufficientData,
    NoBaseline,
}

// ─── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrustConfig {
    pub positive_weight: f64,
    pub negative_weight: f64,
    pub suspicious_weight: f64,
    pub normal_weight: f64,
    pub decay_rate: f64,
    pub min_events: usize,
    pub confidence_threshold: f64,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            positive_weight: 0.1,
            negative_weight: -0.2,
            suspicious_weight: -0.3,
            normal_weight: 0.05,
            decay_rate: 0.01,
            min_events: 30,
            confidence_threshold: 0.5,
        }
    }
}

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("insufficient events for fingerprint generation: {0} provided, {1} required")]
    InsufficientEvents(usize, usize),

    #[error("fingerprint not found for tool: {0}")]
    FingerprintNotFound(String),

    #[error("trust score not found for tool: {0}")]
    TrustScoreNotFound(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

const MCP_REVERIFY_TTL_ENV: &str = "ENVFORGE_MCP_REVERIFY_TTL";
const MCP_REVERIFY_TTL_DEFAULT_SECS: u64 = 7 * 24 * 60 * 60;

/// Per-process re-verify state. Persisted by callers; the function is pure.
#[derive(Debug, Clone, Default)]
pub struct McpReverifyState {
    pub last_verify_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_known_bad: Vec<String>,
}

/// Outcome of one re-verify tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpReverifyOutcome {
    Skipped,
    Ok { server_count: usize },
    NewKnownBad { servers: Vec<String> },
    Failed { error_kind: String },
    NoLockfile,
}

/// Read TTL from env (`ENVFORGE_MCP_REVERIFY_TTL`); default 7 days.
pub fn mcp_reverify_ttl() -> std::time::Duration {
    std::env::var(MCP_REVERIFY_TTL_ENV)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or_else(|| std::time::Duration::from_secs(MCP_REVERIFY_TTL_DEFAULT_SECS))
}

/// Run one re-verify tick. Returns outcome; updates `state`.
///
/// Caller is responsible for persisting `state` across process restarts.
/// Pure modulo filesystem reads.
pub fn mcp_reverify_tick(
    now: chrono::DateTime<chrono::Utc>,
    state: &mut McpReverifyState,
) -> McpReverifyOutcome {
    use crate::ops::mcp_pin::resolver::ReputationLookup;
    use crate::ops::mcp_pin::{FsLockfileRepository, LockfileRepository};
    use crate::ops::mcp_reputation::{
        FsUserOverrideRepository, Tier, TierLookup, UserOverrideRepository,
    };
    use std::sync::Arc;

    let ttl = mcp_reverify_ttl();
    let should_run = state
        .last_verify_at
        .map(|t| (now - t).to_std().unwrap_or(std::time::Duration::ZERO) >= ttl)
        .unwrap_or(true);

    if !should_run {
        return McpReverifyOutcome::Skipped;
    }

    let path = std::path::PathBuf::from(".envforge/mcp.lock");
    let repo = FsLockfileRepository;
    if !repo.exists(&path) {
        state.last_verify_at = Some(now);
        return McpReverifyOutcome::NoLockfile;
    }

    let lockfile = match repo.load(&path) {
        Ok(l) => l,
        Err(_) => {
            return McpReverifyOutcome::Failed {
                error_kind: "lockfile_load_error".into(),
            };
        }
    };
    let override_repo: Arc<dyn UserOverrideRepository> =
        Arc::new(FsUserOverrideRepository::at_default());
    let tier_lookup = match TierLookup::new(override_repo) {
        Ok(t) => t,
        Err(_) => {
            return McpReverifyOutcome::Failed {
                error_kind: "feed_decode_error".into(),
            };
        }
    };

    let current_known_bad: Vec<String> = lockfile
        .servers
        .iter()
        .filter(|p| matches!(tier_lookup.lookup(&p.name), Tier::KnownBad { .. }))
        .map(|p| p.name.clone())
        .collect();

    let new_bad: Vec<String> = current_known_bad
        .iter()
        .filter(|s| !state.last_known_bad.contains(s))
        .cloned()
        .collect();

    state.last_verify_at = Some(now);
    let server_count = lockfile.servers.len();
    state.last_known_bad = current_known_bad;

    if new_bad.is_empty() {
        McpReverifyOutcome::Ok { server_count }
    } else {
        McpReverifyOutcome::NewKnownBad { servers: new_bad }
    }
}
