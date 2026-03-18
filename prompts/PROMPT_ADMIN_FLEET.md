# PROMPT: Admin Console & Fleet Management for Nexus OS

## Context
Enterprise IT teams need a centralized admin console to manage Nexus OS deployments across an organization — deploying agents, managing users, setting policies, and monitoring fleet health.

## Objective
Create admin dashboard pages in the frontend and corresponding Tauri commands for centralized management.

## Implementation Steps

### Step 1: Frontend pages

Create these pages under `frontend/src/pages/Admin/`:

**1. Admin Dashboard (Overview)**
- Total active agents across all workspaces
- Fleet health: instances online/offline/degraded
- Fuel consumption trends (last 24h, 7d, 30d)
- HITL approval queue (pending approvals across all workspaces)
- Recent security events (capability denials, firewall blocks)
- System alerts

**2. User Management**
- User list with roles, workspaces, last active timestamp
- Add/remove users
- Assign roles (Admin, Operator, Viewer, Auditor)
- Assign users to workspaces
- User activity log

**3. Agent Fleet**
- All deployed agents across all workspaces
- Agent status: running / idle / stopped / error
- Agent version, autonomy level, fuel remaining
- Bulk actions: stop all, update all, redeploy
- Agent deployment wizard

**4. Policy Editor**
- Global policies (max autonomy level, allowed providers, fuel limits)
- Workspace-level policy overrides
- Policy version history (who changed what, when)
- Policy templates (Strict, Balanced, Permissive)

**5. Compliance Dashboard**
- EU AI Act self-assessment status
- SOC 2 controls status
- Audit trail statistics
- Hash chain verification status (last verified, next scheduled)
- PII redaction statistics
- HITL approval/denial ratios

**6. System Health**
- Instance list (for server/hybrid mode)
- CPU, memory, disk usage per instance
- LLM provider health (latency, error rates)
- Database size and growth trends
- Backup status (last backup, next scheduled)

### Step 2: Tauri commands

```rust
// Admin dashboard
#[tauri::command]
async fn admin_overview(state: State<'_, AppState>) -> Result<AdminOverview, NexusError>

// User management
#[tauri::command]
async fn admin_users_list(state: State<'_, AppState>) -> Result<Vec<UserDetail>, NexusError>

#[tauri::command]
async fn admin_user_create(state: State<'_, AppState>, user: CreateUserRequest) -> Result<UserDetail, NexusError>

#[tauri::command]
async fn admin_user_update_role(state: State<'_, AppState>, user_id: String, role: UserRole) -> Result<(), NexusError>

#[tauri::command]
async fn admin_user_deactivate(state: State<'_, AppState>, user_id: String) -> Result<(), NexusError>

// Fleet management
#[tauri::command]
async fn admin_fleet_status(state: State<'_, AppState>) -> Result<FleetStatus, NexusError>

#[tauri::command]
async fn admin_agent_deploy(state: State<'_, AppState>, workspace_id: String, manifest: AgentManifest) -> Result<DeployResult, NexusError>

#[tauri::command]
async fn admin_agent_stop_all(state: State<'_, AppState>, workspace_id: String) -> Result<u32, NexusError>

#[tauri::command]
async fn admin_agent_bulk_update(state: State<'_, AppState>, agent_dids: Vec<String>, update: AgentUpdate) -> Result<BulkUpdateResult, NexusError>

// Policy management
#[tauri::command]
async fn admin_policy_get(state: State<'_, AppState>, scope: PolicyScope) -> Result<Policy, NexusError>

#[tauri::command]
async fn admin_policy_update(state: State<'_, AppState>, scope: PolicyScope, policy: Policy) -> Result<(), NexusError>

#[tauri::command]
async fn admin_policy_history(state: State<'_, AppState>, scope: PolicyScope) -> Result<Vec<PolicyChange>, NexusError>

// Compliance
#[tauri::command]
async fn admin_compliance_status(state: State<'_, AppState>) -> Result<ComplianceStatus, NexusError>

#[tauri::command]
async fn admin_compliance_export(state: State<'_, AppState>, format: ExportFormat) -> Result<String, NexusError>
```

### Step 3: Data types

```rust
pub struct AdminOverview {
    pub total_agents: u32,
    pub active_agents: u32,
    pub total_users: u32,
    pub active_users: u32,
    pub workspaces: u32,
    pub fuel_consumed_24h: u64,
    pub hitl_pending: u32,
    pub security_events_24h: u32,
    pub system_health: SystemHealth,
}

pub struct FleetStatus {
    pub agents: Vec<AgentStatus>,
    pub total_running: u32,
    pub total_idle: u32,
    pub total_stopped: u32,
    pub total_error: u32,
}

pub struct AgentStatus {
    pub did: String,
    pub name: String,
    pub workspace_id: String,
    pub autonomy_level: u8,
    pub status: AgentRunState,
    pub fuel_remaining: u64,
    pub last_active: DateTime<Utc>,
    pub uptime_seconds: u64,
}

pub enum AgentRunState {
    Running,
    Idle,
    Stopped,
    Error(String),
    AwaitingHitl,
}
```

### Step 4: RBAC enforcement

All admin commands must verify `UserRole::Admin`:
```rust
fn require_admin(session: &AuthenticatedUser) -> Result<(), NexusError> {
    if session.role != UserRole::Admin {
        return Err(NexusError::Forbidden("Admin role required".into()));
    }
    Ok(())
}
```

Auditor role gets read-only access to compliance and audit endpoints.

### Step 5: Audit all admin actions

Every admin action creates an audit entry:
```rust
AuditAction::AdminUserCreated { user_id, role }
AuditAction::AdminPolicyChanged { scope, changes }
AuditAction::AdminAgentDeployed { workspace_id, agent_did }
AuditAction::AdminBulkAction { action, count }
```

## Testing
- Unit test: Admin role enforcement
- Unit test: Policy CRUD
- Unit test: Fleet status aggregation
- Unit test: Audit entry creation for admin actions

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.
