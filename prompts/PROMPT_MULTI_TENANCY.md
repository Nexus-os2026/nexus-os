# PROMPT: Multi-Tenancy for Nexus OS

## Context
Enterprise deployments need workspace isolation — different teams, departments, or customers each get their own isolated environment with separate agents, data, fuel budgets, and policies.

## Objective
Create a `nexus-tenancy` crate that adds workspace-based multi-tenancy to Nexus OS.

## Architecture

```
Organization
├── Workspace: Engineering
│   ├── Users: [alice, bob]
│   ├── Agents: [coder-agent, reviewer-agent]
│   ├── Fuel Budget: 10M/day
│   ├── Policies: L3 max autonomy
│   └── Audit Trail: isolated
├── Workspace: Research
│   ├── Users: [charlie, diana]
│   ├── Agents: [researcher-agent, analyst-agent]
│   ├── Fuel Budget: 50M/day
│   ├── Policies: L5 max autonomy
│   └── Audit Trail: isolated
└── Admin Workspace
    ├── Users: [admin]
    └── Access: all workspaces (read-only for audit)
```

## Implementation Steps

### Step 1: Create nexus-tenancy crate

```bash
cd crates
cargo new nexus-tenancy --lib
```

### Step 2: Core types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub admins: Vec<String>,          // User emails
    pub members: Vec<WorkspaceMember>,
    pub agent_limit: u32,
    pub fuel_budget_daily: u64,
    pub max_autonomy_level: u8,       // L0-L6
    pub allowed_providers: Vec<String>,
    pub data_isolation: DataIsolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub user_id: String,
    pub role: WorkspaceRole,          // Admin | Operator | Viewer | Auditor
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataIsolation {
    Full,      // Completely separate database files
    Logical,   // Shared database, row-level isolation via workspace_id
}
```

### Step 3: Storage isolation

For `DataIsolation::Full`:
- Each workspace gets its own SQLite database file
- Audit trails are physically separate
- Agent genomes stored per-workspace

For `DataIsolation::Logical`:
- Add `workspace_id TEXT NOT NULL` column to all tables
- All queries filter by `WHERE workspace_id = ?`
- Create composite indexes: `(workspace_id, ...)` for all existing indexes

### Step 4: Workspace-scoped operations

Modify the following to be workspace-aware:
- Agent deployment: `deploy_agent(workspace_id, agent_manifest)`
- Fuel allocation: `allocate_fuel(workspace_id, agent_did, amount)`
- Audit queries: `get_audit_trail(workspace_id, filters)`
- Capability grants: Scoped to workspace resources only
- LLM routing: Respect workspace `allowed_providers`

### Step 5: Tauri commands

```rust
#[tauri::command]
async fn workspace_create(state: State<'_, AppState>, config: WorkspaceConfig) -> Result<Workspace, NexusError>

#[tauri::command]
async fn workspace_list(state: State<'_, AppState>) -> Result<Vec<Workspace>, NexusError>

#[tauri::command]
async fn workspace_add_member(state: State<'_, AppState>, workspace_id: String, member: WorkspaceMember) -> Result<(), NexusError>

#[tauri::command]
async fn workspace_remove_member(state: State<'_, AppState>, workspace_id: String, user_id: String) -> Result<(), NexusError>

#[tauri::command]
async fn workspace_set_policy(state: State<'_, AppState>, workspace_id: String, policy: WorkspacePolicy) -> Result<(), NexusError>

#[tauri::command]
async fn workspace_usage(state: State<'_, AppState>, workspace_id: String) -> Result<WorkspaceUsage, NexusError>
```

### Step 6: Frontend

Create `frontend/src/pages/Workspaces/` with:
- Workspace list/grid view
- Workspace creation wizard
- Member management
- Policy configuration
- Usage dashboard per workspace

### Step 7: Cross-workspace admin

Admin users can:
- View all workspaces
- Read audit trails across workspaces (for compliance)
- Set global policies (max fuel, provider restrictions)
- Cannot modify workspace data (read-only cross-workspace)

## Testing
- Unit test: Workspace CRUD
- Unit test: Data isolation (logical mode)
- Unit test: Cross-workspace access denied for non-admins
- Integration test: Agent deployment scoped to workspace

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.
