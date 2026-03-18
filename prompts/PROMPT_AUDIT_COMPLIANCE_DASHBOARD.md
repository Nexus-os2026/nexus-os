# PROMPT: Audit & Compliance Dashboard for Nexus OS

## Context
Enterprise security and compliance teams need a visual dashboard to explore audit trails, verify hash chain integrity, generate compliance reports, and monitor governance metrics in real-time.

## Objective
Enhance the existing Audit Viewer page and create a new Compliance Dashboard page with rich visualizations and export capabilities.

## Implementation Steps

### Step 1: Enhanced Audit Viewer

Upgrade `frontend/src/pages/AuditViewer/` with:

**Search & Filter:**
- Full-text search across audit entries
- Filter by: agent DID, action type, capability, time range, HITL decision, workspace
- Filter by: severity (info, warning, denied, error)
- Date range picker with presets (last 1h, 24h, 7d, 30d, custom)

**Timeline View:**
- Chronological timeline of agent actions
- Color-coded by action type (green=success, yellow=HITL pending, red=denied)
- Expandable entries showing full detail (capability, fuel, signature)
- Chain integrity indicator per entry (✓ verified / ✗ broken)

**Statistics Panel:**
- Total entries in time range
- Actions per agent (bar chart)
- Approval vs denial rates (pie chart)
- Fuel consumption over time (line chart)
- Top capabilities used (horizontal bar)

**Export:**
- JSON export (full detail)
- CSV export (tabular)
- PDF report generation (formatted compliance report)

### Step 2: Compliance Dashboard

Create `frontend/src/pages/Compliance/` with sections:

**1. Hash Chain Integrity**
- Last verification timestamp and result
- Chain length
- "Verify Now" button
- Verification history (graph of verification times)
- Alert if chain is broken

**2. EU AI Act Status**
- Article-by-article compliance checklist (from EU_AI_ACT_CONFORMITY.md)
- Visual progress bars per article
- Evidence links for each requirement
- Gap identification and remediation tracking

**3. SOC 2 Controls**
- Trust service criteria heatmap (green/yellow/red)
- Controls implemented vs partial vs planned
- Evidence collection status
- Remediation tracking

**4. Governance Metrics**
- HITL approval rate over time
- Capability denial rate (unauthorized access attempts)
- PII redaction volume
- Output firewall block rate
- Fuel exhaustion events
- Agent autonomy level distribution

**5. Security Events**
- Failed authentication attempts
- Capability escalation attempts
- Output firewall triggers
- Unusual fuel consumption patterns
- Sandbox resource limit hits

### Step 3: Tauri commands

```rust
#[tauri::command]
async fn audit_search(state: State<'_, AppState>, query: AuditSearchQuery) -> Result<AuditSearchResult, NexusError>

#[tauri::command]
async fn audit_statistics(state: State<'_, AppState>, period: TimePeriod) -> Result<AuditStatistics, NexusError>

#[tauri::command]
async fn audit_verify_chain(state: State<'_, AppState>) -> Result<ChainVerifyResult, NexusError>

#[tauri::command]
async fn audit_export_report(state: State<'_, AppState>, format: ExportFormat, period: TimePeriod) -> Result<String, NexusError>

#[tauri::command]
async fn compliance_eu_ai_act_status(state: State<'_, AppState>) -> Result<EuAiActStatus, NexusError>

#[tauri::command]
async fn compliance_soc2_status(state: State<'_, AppState>) -> Result<Soc2Status, NexusError>

#[tauri::command]
async fn compliance_governance_metrics(state: State<'_, AppState>, period: TimePeriod) -> Result<GovernanceMetrics, NexusError>

#[tauri::command]
async fn compliance_security_events(state: State<'_, AppState>, period: TimePeriod) -> Result<Vec<SecurityEvent>, NexusError>
```

### Step 4: Data types

```rust
pub struct AuditStatistics {
    pub total_entries: u64,
    pub entries_by_action: HashMap<String, u64>,
    pub entries_by_agent: HashMap<String, u64>,
    pub hitl_approvals: u64,
    pub hitl_denials: u64,
    pub hitl_timeouts: u64,
    pub capability_denials: u64,
    pub pii_redactions: u64,
    pub firewall_blocks: u64,
    pub total_fuel_consumed: u64,
}

pub struct GovernanceMetrics {
    pub hitl_approval_rate: f64,
    pub capability_denial_rate: f64,
    pub pii_redaction_rate: f64,
    pub firewall_block_rate: f64,
    pub avg_hitl_response_time_ms: f64,
    pub fuel_utilization_percent: f64,
    pub autonomy_distribution: HashMap<u8, u32>, // L0-L6 counts
}

pub struct ChainVerifyResult {
    pub verified: bool,
    pub chain_length: u64,
    pub verification_time_ms: u64,
    pub first_break_at: Option<u64>, // Entry index if broken
    pub last_verified_at: DateTime<Utc>,
}
```

### Step 5: Scheduled compliance checks

Run automatically in the background:
- Hash chain verification: every 6 hours
- Governance metrics aggregation: every 15 minutes
- Security event scan: every 5 minutes

Results cached and served from cache for dashboard performance.

## Testing
- Unit test: Audit search with various filters
- Unit test: Statistics aggregation accuracy
- Unit test: Chain verification (valid chain, broken chain)
- Unit test: Export format correctness (JSON, CSV)

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.
