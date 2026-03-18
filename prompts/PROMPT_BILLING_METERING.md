# PROMPT: Billing & Usage Metering for Nexus OS

## Context
Enterprise deployments need to track agent resource consumption for internal chargeback, cost allocation, and usage reporting per team/workspace.

## Objective
Create a `nexus-metering` crate that tracks and reports resource consumption by agent, workspace, user, and time period.

## Implementation Steps

### Step 1: Create nexus-metering crate

### Step 2: Core types

```rust
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
}

pub enum ResourceType {
    LlmTokensInput { provider: String, model: String },
    LlmTokensOutput { provider: String, model: String },
    AgentFuelConsumed,
    SandboxComputeSeconds,
    StorageBytes,
    ApiCalls,
    IntegrationCalls { provider: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub period: TimePeriod,
    pub workspace_id: Option<String>,
    pub total_llm_tokens: u64,
    pub total_fuel_consumed: u64,
    pub total_compute_seconds: f64,
    pub total_api_calls: u64,
    pub total_storage_bytes: u64,
    pub cost_breakdown: Vec<CostLineItem>,
    pub top_agents: Vec<AgentUsageSummary>,
    pub trend: UsageTrend,
}

pub struct CostLineItem {
    pub category: String,
    pub quantity: f64,
    pub unit_cost: f64,
    pub total_cost: f64,
}
```

### Step 3: Cost estimation

```toml
[metering.cost_rates]
# LLM costs per 1M tokens (approximate)
ollama_input = 0.00     # Local, free
ollama_output = 0.00
openai_gpt4o_input = 2.50
openai_gpt4o_output = 10.00
anthropic_sonnet_input = 3.00
anthropic_sonnet_output = 15.00
nvidia_nim_input = 0.50  # Varies by model
nvidia_nim_output = 1.50

# Compute costs
sandbox_compute_per_hour = 0.10

# Storage costs
storage_per_gb_month = 0.023
```

### Step 4: Collection points

Instrument these locations to emit UsageRecords:
- LLM Router: token counts per request (input/output)
- Fuel Meter: fuel consumed per agent action
- WASM Sandbox: compute duration per execution
- Audit Store: storage growth
- Integration Router: external API calls

### Step 5: Aggregation

```rust
pub struct MeteringAggregator {
    /// Aggregate by time bucket
    pub async fn aggregate(
        &self,
        period: TimePeriod,
        group_by: GroupBy,
    ) -> Result<Vec<UsageReport>, MeteringError>;
}

pub enum TimePeriod {
    Hour,
    Day,
    Week,
    Month,
    Custom { start: DateTime<Utc>, end: DateTime<Utc> },
}

pub enum GroupBy {
    Workspace,
    User,
    Agent,
    Provider,
    ResourceType,
}
```

### Step 6: Tauri commands

```rust
#[tauri::command]
async fn metering_usage_report(state: State<'_, AppState>, period: TimePeriod, group_by: GroupBy) -> Result<Vec<UsageReport>, NexusError>

#[tauri::command]
async fn metering_workspace_usage(state: State<'_, AppState>, workspace_id: String, period: TimePeriod) -> Result<UsageReport, NexusError>

#[tauri::command]
async fn metering_export_csv(state: State<'_, AppState>, period: TimePeriod) -> Result<String, NexusError>

#[tauri::command]
async fn metering_set_budget_alert(state: State<'_, AppState>, workspace_id: String, threshold: BudgetAlert) -> Result<(), NexusError>
```

### Step 7: Budget alerts

```rust
pub struct BudgetAlert {
    pub workspace_id: String,
    pub metric: ResourceType,
    pub threshold: f64,
    pub period: TimePeriod,
    pub notification_channels: Vec<String>, // Integration IDs
}
```

When threshold exceeded → trigger integration notification (Slack, Teams, email).

### Step 8: Frontend

Create `frontend/src/pages/Usage/` with:
- Usage dashboard with charts (by workspace, agent, provider, time)
- Cost breakdown table
- Budget alert configuration
- CSV/PDF export
- Trend analysis (month-over-month)

### Step 9: Storage

Create `metering.db` SQLite database:
```sql
CREATE TABLE usage_records (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_did TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    quantity REAL NOT NULL,
    unit TEXT NOT NULL,
    cost_estimate_usd REAL,
    metadata TEXT
);

CREATE INDEX idx_usage_workspace_time ON usage_records(workspace_id, timestamp);
CREATE INDEX idx_usage_agent_time ON usage_records(agent_did, timestamp);
```

## Testing
- Unit test: Cost calculation accuracy
- Unit test: Aggregation by different groupings
- Unit test: Budget alert triggering
- Unit test: CSV export format

## Finish
Run `cargo fmt` and `cargo clippy` on `nexus-metering` crate only.
Do NOT use `--all-features`.
