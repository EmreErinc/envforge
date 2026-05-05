use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Unique identifier for a context
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextId(pub String);

impl ContextId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ContextId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ContextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a namespace
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespaceId(pub String);

impl NamespaceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NamespaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a resource
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub String);

impl ResourceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// AI tool type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolType {
    ClaudeCode,
    GitHubCopilot,
    Cursor,
    Other(String),
}

impl ToolType {
    pub fn as_str(&self) -> &str {
        match self {
            ToolType::ClaudeCode => "Claude Code",
            ToolType::GitHubCopilot => "GitHub Copilot",
            ToolType::Cursor => "Cursor",
            ToolType::Other(s) => s,
        }
    }
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Isolation level for context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub enum IsolationLevel {
    None,
    Low,
    Medium,
    High,
    Strict,
}

impl IsolationLevel {
    pub fn as_str(&self) -> &str {
        match self {
            IsolationLevel::None => "None",
            IsolationLevel::Low => "Low",
            IsolationLevel::Medium => "Medium",
            IsolationLevel::High => "High",
            IsolationLevel::Strict => "Strict",
        }
    }
}

impl std::fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Context specification for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSpec {
    pub tool_type: ToolType,
    pub isolation_level: IsolationLevel,
    pub expires_at: Option<DateTime<Utc>>,
    pub parent_context: Option<ContextId>,
    pub metadata: HashMap<String, String>,
}

impl ContextSpec {
    pub fn new(tool_type: ToolType) -> Self {
        Self {
            tool_type,
            isolation_level: IsolationLevel::High,
            expires_at: None,
            parent_context: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.expires_at = Some(Utc::now() + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::hours(1)));
        self
    }

    pub fn with_parent(mut self, parent_context: ContextId) -> Self {
        self.parent_context = Some(parent_context);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Context state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub enum ContextState {
    Active,
    Suspended,
    Expired,
    Destroyed,
}

impl std::fmt::Display for ContextState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextState::Active => write!(f, "Active"),
            ContextState::Suspended => write!(f, "Suspended"),
            ContextState::Expired => write!(f, "Expired"),
            ContextState::Destroyed => write!(f, "Destroyed"),
        }
    }
}

/// Context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub id: ContextId,
    pub namespace: NamespaceId,
    pub tool_type: ToolType,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub parent_context: Option<ContextId>,
    pub state: ContextState,
    pub metadata: HashMap<String, String>,
}

impl Context {
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }
}

/// Namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub id: NamespaceId,
    pub context_id: ContextId,
    pub parent_namespace: Option<NamespaceId>,
    pub created_at: DateTime<Utc>,
    pub resources: HashMap<ResourceId, Resource>,
    pub isolation_level: IsolationLevel,
}

impl Namespace {
    pub fn new(context_id: ContextId, isolation_level: IsolationLevel) -> Self {
        Self {
            id: NamespaceId::new(),
            context_id,
            parent_namespace: None,
            created_at: Utc::now(),
            resources: HashMap::new(),
            isolation_level,
        }
    }

    pub fn with_parent(mut self, parent_namespace: NamespaceId) -> Self {
        self.parent_namespace = Some(parent_namespace);
        self
    }
}

/// Resource type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Secret,
    File,
    Network,
    Process,
    Other(String),
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Secret => write!(f, "Secret"),
            ResourceType::File => write!(f, "File"),
            ResourceType::Network => write!(f, "Network"),
            ResourceType::Process => write!(f, "Process"),
            ResourceType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    pub resource_type: ResourceType,
    pub name: String,
    pub value: String,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl Resource {
    pub fn new_secret(name: String, value: String) -> Self {
        Self {
            id: ResourceId::new(),
            resource_type: ResourceType::Secret,
            name,
            value,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

/// Secret bindings for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretBindings {
    pub context_id: ContextId,
    pub secrets: HashMap<String, ResourceId>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

impl SecretBindings {
    pub fn new(context_id: ContextId) -> Self {
        let now = Utc::now();
        Self {
            context_id,
            secrets: HashMap::new(),
            created_at: now,
            last_accessed: now,
        }
    }
}

/// Operation type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    Read,
    Write,
    Delete,
    Execute,
    Other(String),
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Read => write!(f, "Read"),
            OperationType::Write => write!(f, "Write"),
            OperationType::Delete => write!(f, "Delete"),
            OperationType::Execute => write!(f, "Execute"),
            OperationType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Access type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessType {
    Direct,
    Inherited,
    Delegated,
}

impl std::fmt::Display for AccessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessType::Direct => write!(f, "Direct"),
            AccessType::Inherited => write!(f, "Inherited"),
            AccessType::Delegated => write!(f, "Delegated"),
        }
    }
}

/// Context operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOperation {
    pub operation_type: OperationType,
    pub resource_id: Option<ResourceId>,
    pub access_type: AccessType,
    pub metadata: HashMap<String, String>,
}

impl ContextOperation {
    pub fn new(operation_type: OperationType, access_type: AccessType) -> Self {
        Self {
            operation_type,
            resource_id: None,
            access_type,
            metadata: HashMap::new(),
        }
    }
}

/// Violation severity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationSeverity::Low => write!(f, "Low"),
            ViolationSeverity::Medium => write!(f, "Medium"),
            ViolationSeverity::High => write!(f, "High"),
            ViolationSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Policy violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub policy_name: String,
    pub description: String,
    pub severity: ViolationSeverity,
}

/// Warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub warning_type: String,
    pub description: String,
    pub recommendation: String,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub violations: Vec<PolicyViolation>,
    pub warnings: Vec<Warning>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            violations: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_violation(mut self, violation: PolicyViolation) -> Self {
        self.valid = false;
        self.violations.push(violation);
        self
    }

    pub fn with_warning(mut self, warning: Warning) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Cleanup status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupStatus {
    Scheduled,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for CleanupStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanupStatus::Scheduled => write!(f, "Scheduled"),
            CleanupStatus::InProgress => write!(f, "InProgress"),
            CleanupStatus::Completed => write!(f, "Completed"),
            CleanupStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Scheduled cleanup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledCleanup {
    pub context_id: ContextId,
    pub expires_at: DateTime<Utc>,
    pub status: CleanupStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Cleanup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub context_id: ContextId,
    pub secrets_cleaned: usize,
    pub duration_ms: u128,
    pub status: CleanupStatus,
}

impl CleanupResult {
    pub fn new(context_id: ContextId, secrets_cleaned: usize, duration_ms: u128) -> Self {
        Self {
            context_id,
            secrets_cleaned,
            duration_ms,
            status: CleanupStatus::Completed,
        }
    }
}

/// Context statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    pub total_contexts: usize,
    pub active_contexts: usize,
    pub expired_contexts: usize,
    pub total_secrets_bound: usize,
    pub average_secrets_per_context: f64,
}

impl ContextStats {
    pub fn default() -> Self {
        Self {
            total_contexts: 0,
            active_contexts: 0,
            expired_contexts: 0,
            total_secrets_bound: 0,
            average_secrets_per_context: 0.0,
        }
    }
}

/// Context summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub context_id: ContextId,
    pub tool_type: ToolType,
    pub state: ContextState,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub secret_count: usize,
    pub isolation_level: IsolationLevel,
}

/// Context isolation errors
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Context not found: {0}")]
    NotFound(String),

    #[error("Invalid context state: {0:?}")]
    InvalidState(ContextState),

    #[error("Context expired")]
    Expired,

    #[error("Access denied")]
    AccessDenied,

    #[error("Secret not found")]
    SecretNotFound,

    #[error("Namespace not found")]
    NamespaceNotFound,

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
