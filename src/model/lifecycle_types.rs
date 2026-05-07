use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ─── Core Rule Types ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleRule {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub trigger: LifecycleTrigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<LifecycleCondition>,
    pub action: LifecycleAction,
    pub enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LifecycleRule {
    pub fn new(name: String, trigger: LifecycleTrigger, action: LifecycleAction) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description: String::new(),
            trigger,
            condition: None,
            action,
            enabled: true,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

// ─── Trigger Types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum LifecycleTrigger {
    Cron { expression: String },
    AgeExceeded { max_days: u32 },
    FileChange { paths: Vec<PathBuf> },
    PolicyViolation { policy: String },
    Composite { triggers: Vec<LifecycleTrigger>, operator: LogicalOp },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleCondition {
    pub operator: LogicalOp,
    pub conditions: Vec<ConditionExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConditionExpr {
    SecretMatches { pattern: String },
    HealthBelow { threshold: f64 },
    UnusedFor { days: u32 },
    TagMatches { tags: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOp {
    All,
    Any,
    Not,
}

// ─── Action Types ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum LifecycleAction {
    Create { template_id: Uuid },
    Rotate { strategy: RotationStrategy },
    Decommission { grace_days: Option<u32> },
    Notify { message: String },
    Composite { actions: Vec<LifecycleAction> },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RotationStrategy {
    #[default]
    Replace,
    DualWrite,
    BlueGreen,
    ProviderManaged,
}

// ─── State Machine Types ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Creating,
    Active,
    Rotating,
    PendingDeprecation,
    Deprecated,
    Decommissioned,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub timestamp: DateTime<Utc>,
    pub triggered_by: String,
    pub operation_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretLifecycle {
    pub key: String,
    pub state: LifecycleState,
    pub history: Vec<StateTransition>,
    pub last_rotation: Option<DateTime<Utc>>,
    pub rotation_count: u32,
    pub expiry: Option<DateTime<Utc>>,
}

impl SecretLifecycle {
    pub fn new(key: String) -> Self {
        Self {
            key,
            state: LifecycleState::Active,
            history: Vec::new(),
            last_rotation: None,
            rotation_count: 0,
            expiry: None,
        }
    }
}

// ─── Template Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretTemplate {
    pub id: Uuid,
    pub name: String,
    pub generator: SecretGenerator,
    pub target_paths: Vec<PathBuf>,
    pub rotation_policy: Option<RotationPolicy>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum SecretGenerator {
    Random { length: usize, chars: String },
    Provider { provider: String, path: String },
    Certificate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    pub strategy: RotationStrategy,
    pub interval_days: Option<u32>,
    pub notify_days_before: Option<u32>,
}

// ─── Event Types ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEvent {
    pub trigger_type: TriggerType,
    pub rule_id: Uuid,
    pub secret_key: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    Cron,
    Age,
    FileChange,
    Policy,
}

impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cron => write!(f, "cron"),
            Self::Age => write!(f, "age"),
            Self::FileChange => write!(f, "file-change"),
            Self::Policy => write!(f, "policy"),
        }
    }
}

// ─── Result Types ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResult {
    pub key: String,
    pub value_set: bool,
    pub template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateResult {
    pub key: String,
    pub success: bool,
    pub new_value_set: bool,
    pub strategy: RotationStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecommissionResult {
    pub key: String,
    pub success: bool,
    pub grace_period_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecommissionPlan {
    pub key: String,
    pub has_active_dependents: bool,
    pub recommended_grace_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub key: String,
    pub success: bool,
    pub snapshot_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub key: String,
    pub recovered: bool,
    pub errors: Vec<String>,
}

// ─── Operation Types ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleOperation {
    pub id: Uuid,
    pub operation_type: OperationType,
    pub key: Option<String>,
    pub status: OperationStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    Create,
    Rotate,
    Decommission,
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

// ─── Supporting Types ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicySeverity {
    Info,
    Warning,
    Error,
    Critical,
}

pub type SnapshotId = Uuid;
pub type RuleId = Uuid;

// ─── Snapshot Types ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: Uuid,
    pub key: String,
    pub masked_value: String,
    pub source_file: Option<PathBuf>,
    pub source_hash: Option<String>,
    pub state: LifecycleState,
    pub operation_type: OperationType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub meta: SnapshotMeta,
    pub value: Option<String>, // None if masked-only
}

// ─── Trigger Engine Context ─────────────────────────────

/// Context passed to trigger and condition evaluation.
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    pub project_dir: Option<PathBuf>,
    pub current_time: DateTime<Utc>,
    pub last_check: Option<DateTime<Utc>>,
}

// ─── State Event ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateEvent {
    CreateComplete,
    RotationRequested,
    RotationComplete,
    DeprecationRequested,
    GracePeriodExpired,
    DecommissionComplete,
    Failure { reason: String },
    Recovery,
}
