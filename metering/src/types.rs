//! Core types for usage metering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Resource types
// ---------------------------------------------------------------------------

/// Classification of a metered resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceType {
    LlmTokensInput { provider: String, model: String },
    LlmTokensOutput { provider: String, model: String },
    AgentFuelConsumed,
    SandboxComputeSeconds,
    StorageBytes,
    ApiCalls,
    IntegrationCalls { provider: String },
}

impl ResourceType {
    /// Human-readable category name for grouping.
    pub fn category(&self) -> &str {
        match self {
            Self::LlmTokensInput { .. } | Self::LlmTokensOutput { .. } => "llm",
            Self::AgentFuelConsumed => "fuel",
            Self::SandboxComputeSeconds => "compute",
            Self::StorageBytes => "storage",
            Self::ApiCalls => "api",
            Self::IntegrationCalls { .. } => "integration",
        }
    }

    /// Default unit label.
    pub fn default_unit(&self) -> &str {
        match self {
            Self::LlmTokensInput { .. } | Self::LlmTokensOutput { .. } => "tokens",
            Self::AgentFuelConsumed => "fuel_units",
            Self::SandboxComputeSeconds => "seconds",
            Self::StorageBytes => "bytes",
            Self::ApiCalls | Self::IntegrationCalls { .. } => "calls",
        }
    }
}

// ---------------------------------------------------------------------------
// Usage record
// ---------------------------------------------------------------------------

/// A single metered usage event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub workspace_id: String,
    pub user_id: String,
    pub agent_did: String,
    pub resource_type: ResourceType,
    pub quantity: f64,
    pub unit: String,
    pub cost_estimate_usd: Option<f64>,
    /// Arbitrary key-value metadata.
    pub metadata: Option<serde_json::Value>,
}

impl UsageRecord {
    pub fn new(
        workspace_id: impl Into<String>,
        user_id: impl Into<String>,
        agent_did: impl Into<String>,
        resource_type: ResourceType,
        quantity: f64,
    ) -> Self {
        let unit = resource_type.default_unit().to_string();
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            workspace_id: workspace_id.into(),
            user_id: user_id.into(),
            agent_did: agent_did.into(),
            resource_type,
            quantity,
            unit,
            cost_estimate_usd: None,
            metadata: None,
        }
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_estimate_usd = Some(cost);
        self
    }

    pub fn with_metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = Some(meta);
        self
    }
}

// ---------------------------------------------------------------------------
// Time period & grouping
// ---------------------------------------------------------------------------

/// Period for aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimePeriod {
    Hour,
    Day,
    Week,
    Month,
    Custom {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

/// Dimension to group reports by.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupBy {
    Workspace,
    User,
    Agent,
    Provider,
    ResourceType,
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

/// Aggregated usage report for a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub period: TimePeriod,
    pub group_key: Option<String>,
    pub total_llm_tokens: u64,
    pub total_fuel_consumed: u64,
    pub total_compute_seconds: f64,
    pub total_api_calls: u64,
    pub total_storage_bytes: u64,
    pub cost_breakdown: Vec<CostLineItem>,
    pub top_agents: Vec<AgentUsageSummary>,
    pub trend: UsageTrend,
}

/// One line in a cost breakdown table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLineItem {
    pub category: String,
    pub quantity: f64,
    pub unit_cost: f64,
    pub total_cost: f64,
}

/// Per-agent usage summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsageSummary {
    pub agent_did: String,
    pub total_records: u64,
    pub total_cost: f64,
}

/// Month-over-month trend data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTrend {
    pub previous_period_cost: f64,
    pub current_period_cost: f64,
    pub change_percent: f64,
}

// ---------------------------------------------------------------------------
// Budget alerts
// ---------------------------------------------------------------------------

/// Configurable budget alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub id: Uuid,
    pub workspace_id: String,
    pub metric: ResourceType,
    pub threshold: f64,
    pub period: TimePeriod,
    /// Integration IDs for notification (Slack, Teams, email, etc.).
    pub notification_channels: Vec<String>,
    pub enabled: bool,
}

impl BudgetAlert {
    pub fn new(
        workspace_id: impl Into<String>,
        metric: ResourceType,
        threshold: f64,
        period: TimePeriod,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            workspace_id: workspace_id.into(),
            metric,
            threshold,
            period,
            notification_channels: Vec::new(),
            enabled: true,
        }
    }
}
