use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Core Event Types ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAccessEvent {
    pub secret_name: String,
    pub access_type: AccessType,
    pub accessor: AccessorInfo,
    pub timestamp: DateTime<Utc>,
    pub source: AccessSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<AccessContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedAccessEvent {
    pub id: Uuid,
    pub raw: RawAccessEvent,
    pub secret_id: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    pub risk_level: RiskLevel,
    pub enriched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessorInfo {
    pub id: String,
    pub accessor_type: AccessorType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

// ─── Enums ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessType {
    Read,
    Write,
    Delete,
    List,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessorType {
    User,
    Service,
    AiTool,
    CiCdPipeline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessSource {
    Proxy,
    AiGuard,
    Cli,
    Tui,
    Cicd,
    Provider,
    Changelog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeWindow {
    LastHour,
    Last24Hours,
    Last7Days,
    Last30Days,
    Last90Days,
    #[serde(untagged)]
    Custom(CustomTimeWindow),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTimeWindow {
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalPattern {
    Ephemeral,
    Constant,
    Periodic,
    Bursting,
    Declining,
    Growing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrelationType {
    Positive,
    Negative,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasteReason {
    Unused,
    Underused,
    Overprovisioned,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SavingsType {
    RemoveSecret,
    DowngradeProvider,
    ReduceScope,
    LimitAccess,
    Consolidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EffortLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Urgency {
    Informational,
    Recommended,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

// ─── Frequency Types ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePattern {
    pub pattern_type: String,
    pub description: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessFrequency {
    pub total: u64,
    pub per_day: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_hour: Option<u32>,
    #[serde(default)]
    pub distribution: Vec<FrequencyDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyDataPoint {
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyData {
    pub raw_count: u64,
    pub daily_average: f64,
    pub weekly_average: f64,
    pub monthly_average: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedFrequency {
    pub window: TimeWindow,
    pub frequency: FrequencyData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trend {
    pub direction: TrendDirection,
    pub slope: f64,
    pub r_squared: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correlation {
    pub secret_a: String,
    pub secret_b: String,
    pub coefficient: f64,
    pub correlation_type: CorrelationType,
}

// ─── Unused/Dormant Detection Types ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedSecret {
    pub secret_name: String,
    pub reason: String,
    pub days_since_last_access: u64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowUsageSecret {
    pub secret_name: String,
    pub access_count: u64,
    pub threshold: u64,
    pub period: TimeWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedTimeline {
    pub review_by: DateTime<Utc>,
    pub deprecate_by: DateTime<Utc>,
    pub remove_by: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationRecommendation {
    pub secret_name: String,
    pub reason: String,
    pub unused: UnusedSecret,
    pub timeline: SuggestedTimeline,
    pub dependent_count: u64,
}

// ─── Cost Estimation Types ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub total_monthly: f64,
    pub total_annual: f64,
    #[serde(default)]
    pub per_provider: Vec<ProviderCost>,
    #[serde(default)]
    pub per_secret: Vec<SecretCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCost {
    pub provider: String,
    pub monthly: f64,
    pub secret_count: u64,
    pub access_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretCost {
    pub secret_name: String,
    pub provider: String,
    pub monthly: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub scope: String,
    pub estimated_monthly: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WastedCost {
    pub amount: f64,
    pub reason: WasteReason,
    pub secret_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsRecommendation {
    pub description: String,
    pub estimated_savings: f64,
    pub savings_type: SavingsType,
    pub effort: EffortLevel,
}

// ─── Policy Recommendation Types ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeastPrivilegePolicy {
    pub secret_name: String,
    pub current_scope: String,
    pub recommended_scope: String,
    pub estimated_savings: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedPolicy {
    pub secret_name: String,
    #[serde(default)]
    pub allowed_hours: Vec<u32>,
    #[serde(default)]
    pub allowed_days: Vec<WeekdaySerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WeekdaySerde {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeReduction {
    pub secret_name: String,
    pub current_accessors: u64,
    pub target_accessors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyTightening {
    pub secret_name: String,
    pub anomaly_description: String,
    pub recommended_action: String,
    pub urgency: Urgency,
}

// ─── Aggregation Types ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateBucket {
    pub key: String,
    pub period_start: DateTime<Utc>,
    pub period_type: AggregatePeriod,
    pub access_count: u64,
    pub unique_accessors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatePeriod {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

// ─── Report Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    pub total_secrets: u64,
    pub total_events: u64,
    pub unused_count: u64,
    pub dormant_count: u64,
    pub active_count: u64,
    pub estimated_monthly_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsReport {
    pub id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub summary: AnalyticsSummary,
    #[serde(default)]
    pub unused: Vec<UnusedSecret>,
    #[serde(default)]
    pub low_usage: Vec<LowUsageSecret>,
    #[serde(default)]
    pub trends: Vec<Trend>,
    #[serde(default)]
    pub correlations: Vec<Correlation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostBreakdown>,
    #[serde(default)]
    pub recommendations: Vec<SavingsRecommendation>,
}

// ─── Config Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_events")]
    pub max_events: usize,
    #[serde(default = "default_true")]
    pub auto_aggregate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing_file: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_retention_days() -> u32 {
    90
}

fn default_max_events() -> usize {
    10000
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 90,
            max_events: 10000,
            auto_aggregate: true,
            pricing_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingEntry {
    pub provider: String,
    #[serde(default)]
    pub monthly_base: f64,
    #[serde(default)]
    pub per_secret: f64,
    #[serde(default)]
    pub per_access: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
}

fn default_currency() -> String {
    "USD".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PricingData {
    #[serde(default)]
    pub providers: Vec<PricingEntry>,
}
