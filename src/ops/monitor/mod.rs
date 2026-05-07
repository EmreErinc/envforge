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
    Manual,
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
            Self::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub source: EventSource,
    pub key: Option<String>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
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

pub fn init_event_bus(capacity: usize) {
    let (tx, _) = broadcast::channel(capacity.max(64));
    let _ = EVENT_BUS.set(tx);
    BUS_ENABLED.store(true, Ordering::SeqCst);
}

pub fn emit_event(event: RuntimeEvent) {
    if !BUS_ENABLED.load(Ordering::SeqCst) {
        return;
    }
    if let Some(tx) = EVENT_BUS.get() {
        let _ = tx.send(event);
    }
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
