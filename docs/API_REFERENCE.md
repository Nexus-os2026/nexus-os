# API Reference

> Public types and functions for Nexus OS v4.0.0.

## Kernel (`nexus-kernel`)

### `supervisor`

```rust
pub type AgentId = Uuid;

pub struct AgentHandle {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub autonomy_guard: AutonomyGuard,
    pub consent_runtime: ConsentRuntime,
    pub autonomy_level: u8,
    pub state: AgentState,
    pub remaining_fuel: u64,
}

pub struct AgentStatus {
    pub id: AgentId,
    pub state: AgentState,
    pub remaining_fuel: u64,
}

pub struct Supervisor {
    // Private fields
}

impl Supervisor {
    pub fn new() -> Self;
    pub fn register(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError>;
    pub fn start(&mut self, id: AgentId) -> Result<(), AgentError>;
    pub fn stop(&mut self, id: AgentId) -> Result<(), AgentError>;
    pub fn kill(&mut self, id: AgentId) -> Result<(), AgentError>;
    pub fn status(&self, id: AgentId) -> Option<AgentStatus>;
    pub fn list_agents(&self) -> Vec<AgentStatus>;
    pub fn execute_action(&mut self, id: AgentId, action: &str) -> Result<(), AgentError>;
}
```

### `manifest`

```rust
pub struct AgentManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    pub autonomy_level: Option<u8>,
    pub consent_policy_path: Option<String>,
    pub requester_id: Option<String>,
    pub schedule: Option<String>,
    pub llm_model: Option<String>,
    pub fuel_period_id: Option<String>,
    pub monthly_fuel_cap: Option<u64>,
}

pub fn parse_manifest(input: &str) -> Result<AgentManifest, AgentError>;
```

**Capability Registry**: `web.search`, `web.read`, `llm.query`, `fs.read`, `fs.write`, `process.exec`, `social.post`, `social.x.post`, `social.x.read`, `messaging.send`, `audit.read`

**Validation Rules**:
- Name: 3-64 characters, alphanumeric + hyphens
- Fuel budget: 1 - 1,000,000
- Autonomy level: 0-5
- All capabilities must be in the registry

### `autonomy`

```rust
pub enum AutonomyLevel {
    L0, L1, L2, L3, L4, L5,
}

impl AutonomyLevel {
    pub fn as_str(self) -> &'static str;
    pub fn from_numeric(value: u8) -> Option<Self>;
    pub fn from_manifest(value: Option<u8>) -> Self;
    pub fn previous(self) -> Self;
}

pub struct AutonomyPolicyHooks {
    pub tool_call_min: AutonomyLevel,         // Default: L1
    pub multi_agent_min: AutonomyLevel,       // Default: L2
    pub self_modification_min: AutonomyLevel, // Default: L4
    pub distributed_min: AutonomyLevel,       // Default: L5
}

pub struct AutonomyGuard { /* private */ }

impl AutonomyGuard {
    pub fn new(level: AutonomyLevel) -> Self;
    pub fn with_hooks(level: AutonomyLevel, hooks: AutonomyPolicyHooks) -> Self;
    pub fn level(&self) -> AutonomyLevel;
    pub fn require_tool_call(&mut self, actor_id: Uuid, audit: &mut AuditTrail) -> Result<(), AutonomyError>;
    pub fn require_multi_agent(&mut self, actor_id: Uuid, audit: &mut AuditTrail) -> Result<(), AutonomyError>;
    pub fn require_self_modification(&mut self, actor_id: Uuid, audit: &mut AuditTrail) -> Result<(), AutonomyError>;
    pub fn require_distributed(&mut self, actor_id: Uuid, audit: &mut AuditTrail) -> Result<(), AutonomyError>;
    pub fn downgrade(&mut self, actor_id: Uuid, new_level: AutonomyLevel, action: &'static str, reason: &str, audit: &mut AuditTrail);
}

pub enum AutonomyError {
    Denied {
        required: AutonomyLevel,
        current: AutonomyLevel,
        action: &'static str,
        downgraded_to: Option<AutonomyLevel>,
    },
}
```

### `lifecycle`

```rust
pub enum AgentState {
    Created, Starting, Running, Paused, Stopping, Stopped, Destroyed,
}

pub fn transition_state(from: AgentState, to: AgentState) -> Result<AgentState, AgentError>;
pub fn is_valid_transition(from: AgentState, to: AgentState) -> bool;
```

**Valid Transitions**:
- Created -> Starting
- Starting -> Running
- Running -> Paused, Stopping
- Paused -> Running, Stopping
- Stopping -> Stopped
- Stopped -> Destroyed

### `errors`

```rust
pub enum AgentError {
    FuelExhausted,
    FuelViolation { violation: FuelViolation, reason: String },
    InvalidTransition { from: AgentState, to: AgentState },
    ManifestError(String),
    CapabilityDenied(String),
    ApprovalRequired { request_id: String },
    SupervisorError(String),
    KeyDestroyed(String),
}

pub enum ErrorStrategy {
    Retry { max_attempts: u8 },
    Skip,
    Escalate,
}

pub fn on_error(error: &AgentError) -> ErrorStrategy;
```

### `delegation`

```rust
pub struct DelegationConstraints {
    pub max_fuel: u64,           // Default: 1000
    pub max_duration_secs: u64,  // Default: 3600
    pub max_depth: u8,           // Default: 1
    pub require_approval: bool,  // Default: false
}

pub struct DelegationGrant {
    pub id: Uuid,
    pub grantor: Uuid,
    pub grantee: Uuid,
    pub capabilities: Vec<String>,
    pub constraints: DelegationConstraints,
    pub chain: Vec<Uuid>,
    pub created_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub fuel_used: u64,
}

pub enum DelegationError {
    CapabilityNotOwned(String),
    DepthExceeded,
    Expired,
    Revoked,
    FuelExhausted,
    NotFound,
}
```

### `adaptive_policy`

```rust
pub enum RunOutcome {
    Success,
    Failed { reason: String },
    PolicyViolation { violation: String },
}

pub struct AgentTrackRecord {
    pub agent_id: Uuid,
    pub total_runs: u64,
    pub successful_runs: u64,
    pub failed_runs: u64,
    pub policy_violations: u64,
    pub approval_overrides: u64,
    pub fuel_efficiency: f64,
    pub last_violation_at: Option<u64>,
    pub trust_score: f64,
}

pub enum AutonomyChange {
    Promote { from: u8, to: u8 },
    Demote { from: u8, to: u8, reason: String },
    NoChange,
}

pub struct AdaptivePolicy {
    pub agent_id: Uuid,
    pub base_autonomy: u8,
    pub current_autonomy: u8,
    pub promotion_threshold: f64,
    pub demotion_threshold: f64,
    pub max_autonomy: u8,
    pub cooldown_after_violation_secs: u64,
}
```

**Trust Score Calculation**: `trust_score = (successful_runs / total_runs) * (1.0 - violations * 0.2)`, clamped to [0.0, 1.0].

### `fuel_hardening`

```rust
pub enum FuelViolation {
    OverBudget,
    OverMonthlyCap,
    AnomalousBurn,
}

pub struct AgentFuelLedger { /* private */ }
pub struct BudgetPeriodId(pub String);
pub struct BurnAnomalyDetector { /* private */ }
pub struct FuelAuditReport { /* private */ }
```

### `consent`

```rust
pub struct ApprovalRequest { /* details */ }
pub struct ConsentRuntime { /* private */ }
pub struct GovernedOperation { /* private */ }
```

### `replay`

```rust
// In kernel::replay
pub mod bundle;   // Evidence bundle creation
pub mod format;   // Replay format specification
pub mod verifier; // Deterministic replay verification
```

---

## SDK (`nexus-sdk`)

### `NexusAgent` Trait

```rust
pub trait NexusAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError>;
    fn shutdown(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn checkpoint(&self) -> Result<Vec<u8>, AgentError>;   // Default: empty vec
    fn restore(&mut self, data: &[u8]) -> Result<(), AgentError>; // Default: no-op
}

pub struct AgentOutput {
    pub status: String,
    pub outputs: Vec<serde_json::Value>,
    pub fuel_used: u64,
}
```

### `AgentContext`

```rust
pub struct AgentContext { /* private */ }

impl AgentContext {
    pub fn new(agent_id: Uuid, capabilities: Vec<String>, fuel_budget: u64) -> Self;
    pub fn agent_id(&self) -> Uuid;
    pub fn fuel_remaining(&self) -> u64;
    pub fn fuel_budget(&self) -> u64;
    pub fn audit_trail(&self) -> &AuditTrail;
    pub fn approval_records(&self) -> &[ApprovalRecord];
    pub fn require_capability(&self, capability: &str) -> Result<(), AgentError>;
    pub fn llm_query(&mut self, prompt: &str, max_tokens: u32) -> Result<String, AgentError>;
    pub fn read_file(&mut self, path: &str) -> Result<String, AgentError>;
    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), AgentError>;
    pub fn request_approval(&mut self, description: &str) -> ApprovalRecord;
}

pub struct ApprovalRecord {
    pub description: String,
    pub requested_at: u64,
}
```

**Fuel Costs**:
| Operation | Cost |
|-----------|------|
| `llm_query` | 10 |
| `read_file` | 2 |
| `write_file` | 8 |

### `ManifestBuilder`

```rust
pub struct ManifestBuilder { /* private */ }

impl ManifestBuilder {
    pub fn new(name: &str) -> Self;
    pub fn version(self, version: &str) -> Self;
    pub fn capability(self, capability: &str) -> Self;
    pub fn fuel_budget(self, budget: u64) -> Self;
    pub fn autonomy_level(self, level: u8) -> Self;
    pub fn build(self) -> Result<AgentManifest, AgentError>;
}
```

### `TestHarness`

```rust
pub struct TestHarness { /* private */ }

impl TestHarness {
    pub fn new() -> Self;
    pub fn with_capabilities(self, capabilities: Vec<String>) -> Self;
    pub fn with_fuel(self, fuel: u64) -> Self;
    pub fn build_context(self) -> AgentContext;
}
```

### `Sandbox`

```rust
pub struct SandboxConfig { /* configurable limits */ }
pub struct InProcessSandbox { /* private */ }
pub struct SandboxRuntime { /* private */ }

pub enum SandboxError { /* variants */ }
pub type SandboxResult<T> = Result<T, SandboxError>;

pub trait HostFunction { /* host call interface */ }
pub enum HostCallResult { /* success/error */ }
```

---

## Distributed (`nexus-distributed`)

### `tcp_transport`

```rust
pub struct ConnectionConfig {
    pub connect_timeout_secs: u64,   // Default: 5
    pub read_timeout_secs: u64,      // Default: 10
    pub max_retries: u32,            // Default: 5
    pub base_retry_delay_ms: u64,    // Default: 1000
    pub max_retry_delay_ms: u64,     // Default: 30000
}

pub struct RetryPolicy { /* private */ }

impl RetryPolicy {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_retries: u32) -> Self;
    pub fn next_delay(&self, retry_count: u32) -> std::time::Duration;
    pub fn should_retry(&self, retry_count: u32) -> bool;
}

pub enum MessageType {
    Heartbeat,
    AuditSync,
    QuorumPropose,
    QuorumVote,
    ReplicationFull,
    ReplicationDelta,
    AuthChallenge,
    AuthResponse,
}

pub struct WireMessage {
    pub message_id: Uuid,
    pub sender_node_id: String,
    pub message_type: MessageType,
    pub timestamp: u64,
    pub payload: Vec<u8>,
}

pub fn frame_message(msg: &WireMessage) -> Vec<u8>;
pub fn read_framed_message(reader: &mut impl std::io::Read) -> Result<WireMessage, TcpTransportError>;

pub struct TcpTransportManager { /* private */ }

impl TcpTransportManager {
    pub fn new(config: ConnectionConfig) -> Self;
    pub fn bind(&mut self, addr: &str) -> Result<(), TcpTransportError>;
    pub fn connect(&mut self, node_id: &str, addr: &str) -> Result<(), TcpTransportError>;
    pub fn accept(&mut self) -> Result<String, TcpTransportError>;
    pub fn send(&mut self, node_id: &str, msg: &WireMessage) -> Result<(), TcpTransportError>;
    pub fn recv(&mut self, node_id: &str) -> Result<WireMessage, TcpTransportError>;
    pub fn broadcast(&mut self, msg: &WireMessage) -> Vec<(String, Result<(), TcpTransportError>)>;
    pub fn disconnect(&mut self, node_id: &str);
    pub fn reconnect(&mut self, node_id: &str, addr: &str) -> Result<(), TcpTransportError>;
    pub fn connected_nodes(&self) -> Vec<String>;
}

pub enum TcpTransportError {
    Bind(String),
    Connect(String),
    Send(String),
    Receive(String),
    Serialization(String),
    NotConnected(String),
    NoListener,
}
```

### `membership`

SWIM-style membership protocol for node discovery and failure detection.

### `quorum`

Quorum-based voting system for distributed governance decisions.

### `replication`

Audit event replication across cluster nodes.

### `transport`

Abstract transport trait for pluggable networking backends.

---

## Enterprise (`nexus-enterprise`)

### `rbac`

Role-based access control for multi-tenant deployments.

### `compliance`

SOC 2 Type II compliance reporting with control tracking and evidence collection.

---

## Marketplace (`nexus-marketplace`)

### `registry`

Agent package registry with search, publish, and discovery.

### `package`

Signed bundle format for distributing agents.

### `manifest_verify`

Cryptographic verification of agent package manifests.

### `trust`

Publisher trust scoring based on package history and community feedback.

### `scanner`

Security scanning of agent bundles before installation.

### `install`

Safe agent installation with sandboxed verification.

---

## CLI (`nexus-cli`)

### `commands`

```rust
pub enum CliCommand {
    // Agent management
    AgentList,
    AgentStart { agent_id: String },
    AgentStop { agent_id: String },
    AgentStatus { agent_id: String },

    // Audit
    AuditShow,
    AuditVerify,
    AuditExport { format: String },
    AuditFederationStatus,

    // Cluster
    ClusterStatus,
    ClusterJoin { seed: String },
    ClusterLeave,

    // Marketplace
    MarketplaceSearch { query: String },
    MarketplaceInstall { package_id: String },
    MarketplaceUninstall { package_id: String },

    // Compliance
    ComplianceReport,
    ComplianceStatus,

    // Delegation
    DelegationGrant { grantor: String, grantee: String, capabilities: Vec<String> },
    DelegationRevoke { grant_id: String },
    DelegationList,

    // Benchmarks
    BenchmarkRun,
    BenchmarkReport,

    // Fine-tuning
    FinetuneCreate { config: String },
    FinetuneApprove { job_id: String },
    FinetuneStatus { job_id: String },
}

pub struct CliOutput {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CliOutput {
    pub fn ok(message: impl Into<String>) -> Self;
    pub fn ok_with_data(message: impl Into<String>, data: serde_json::Value) -> Self;
    pub fn err(message: impl Into<String>) -> Self;
}
```

### `router`

```rust
pub fn route(command: CliCommand) -> CliOutput;
```

Dispatches any `CliCommand` to the appropriate handler and returns structured output.
