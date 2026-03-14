# CONDUCTOR_RECON.md — Exact API Surface for Building the Conductor Crate

> Generated from source. All signatures are copy-pasted from the actual Rust files.
> Use `nexus_kernel`, `nexus_sdk`, etc. as the crate names in `use` statements
> (Rust converts hyphens to underscores for module paths).

---

## 1. Workspace Structure

**Root `Cargo.toml` `[workspace] members`:**

```toml
[workspace]
resolver = "2"
members = [
  "agents/coder",
  "agents/designer",
  "agents/coding-agent",
  "agents/screen-poster",
  "agents/self-improve",
  "agents/social-poster",
  "agents/web-builder",
  "agents/workflow-studio",
  "kernel",
  "connectors/core",
  "connectors/web",
  "connectors/social",
  "connectors/messaging",
  "connectors/llm",
  "workflows",
  "cli",
  "research",
  "content",
  "analytics",
  "adaptation",
  "control",
  "factory",
  "marketplace",
  "self-update",
  "tests/integration",
  "app/src-tauri",
  "benchmarks",
  "distributed",
  "sdk",
  "enterprise",
  "cloud",
  "agents/collaboration",
  "protocols",
  "packaging/airgap",
]

[workspace.package]
edition = "2021"
version = "7.0.0"
license = "MIT"

[workspace.lints.rust]
unsafe_code = "forbid"
```

**Member → Crate name mapping:**

| Directory | `[package] name` | Rust import path |
|---|---|---|
| `agents/coder` | `coder-agent` | `coder_agent` |
| `agents/designer` | `designer-agent` | `designer_agent` |
| `agents/coding-agent` | `coding-agent` | `coding_agent` |
| `agents/screen-poster` | `screen-poster-agent` | `screen_poster_agent` |
| `agents/self-improve` | `self-improve-agent` | `self_improve_agent` |
| `agents/social-poster` | `social-poster-agent` | `social_poster_agent` |
| `agents/web-builder` | `web-builder-agent` | `web_builder_agent` |
| `agents/workflow-studio` | `workflow-studio-agent` | `workflow_studio_agent` |
| `kernel` | `nexus-kernel` | `nexus_kernel` |
| `connectors/core` | `nexus-connectors-core` | `nexus_connectors_core` |
| `connectors/web` | `nexus-connectors-web` | `nexus_connectors_web` |
| `connectors/social` | `nexus-connectors-social` | `nexus_connectors_social` |
| `connectors/messaging` | `nexus-connectors-messaging` | `nexus_connectors_messaging` |
| `connectors/llm` | `nexus-connectors-llm` | `nexus_connectors_llm` |
| `workflows` | `nexus-workflows` | `nexus_workflows` |
| `cli` | `nexus-cli` | `nexus_cli` |
| `research` | `nexus-research` | `nexus_research` |
| `content` | `nexus-content` | `nexus_content` |
| `analytics` | `nexus-analytics` | `nexus_analytics` |
| `adaptation` | `nexus-adaptation` | `nexus_adaptation` |
| `control` | `nexus-control` | `nexus_control` |
| `factory` | `nexus-factory` | `nexus_factory` |
| `marketplace` | `nexus-marketplace` | `nexus_marketplace` |
| `self-update` | `nexus-self-update` | `nexus_self_update` |
| `tests/integration` | `nexus-integration` | `nexus_integration` |
| `app/src-tauri` | `nexus-desktop-backend` | `nexus_desktop_backend` |
| `benchmarks` | `nexus-benchmarks` | `nexus_benchmarks` |
| `distributed` | `nexus-distributed` | `nexus_distributed` |
| `sdk` | `nexus-sdk` | `nexus_sdk` |
| `enterprise` | `nexus-enterprise` | `nexus_enterprise` |
| `cloud` | `nexus-cloud` | `nexus_cloud` |
| `agents/collaboration` | `nexus-collaboration` | `nexus_collaboration` |
| `protocols` | `nexus-protocols` | `nexus_protocols` |
| `packaging/airgap` | `nexus-airgap` | `nexus_airgap` |

---

## 2. Kernel Supervisor — Exact API

**Source:** `kernel/src/supervisor.rs`

### Imports

```rust
use crate::audit::{AuditTrail, EventType};
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
use crate::errors::AgentError;
use crate::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelViolation,
};
use crate::kill_gates::KillGateError;
use crate::lifecycle::{transition_state, AgentState};
use crate::manifest::AgentManifest;
use crate::permissions::{
    CapabilityRequest, PermissionCategory, PermissionHistoryEntry, PermissionManager,
};
use crate::policy_engine::PolicyEngine;
use crate::safety_supervisor::{KpiKind, SafetyAction, SafetySupervisor};
use crate::speculative::{SimulationResult, SpeculativeEngine};
use crate::time_machine::TimeMachine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
```

### Type Alias

```rust
pub type AgentId = Uuid;
```

### ExecutionMode

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    Native,
    Wasm { binary_path: PathBuf },
}
```

### AgentHandle

```rust
#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub autonomy_guard: AutonomyGuard,
    pub consent_runtime: ConsentRuntime,
    pub autonomy_level: u8,
    pub state: AgentState,
    pub remaining_fuel: u64,
    pub execution_mode: ExecutionMode,
}
```

### AgentStatus

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub id: AgentId,
    pub state: AgentState,
    pub remaining_fuel: u64,
}
```

### Supervisor

```rust
#[derive(Debug)]
pub struct Supervisor {
    agents: HashMap<AgentId, AgentHandle>,
    fuel_ledgers: HashMap<AgentId, AgentFuelLedger>,
    audit_trail: AuditTrail,
    safety_supervisor: SafetySupervisor,
    speculative_engine: SpeculativeEngine,
    permission_manager: PermissionManager,
    policy_engine: PolicyEngine,
    time_machine: TimeMachine,
}

impl Default for Supervisor {
    fn default() -> Self { Self::new() }
}
```

### Supervisor — All Public Methods

```rust
impl Supervisor {
    pub fn new() -> Self
    pub fn with_policy_dir(dir: impl Into<PathBuf>) -> Self
    pub fn set_policy_engine(&mut self, engine: PolicyEngine)
    pub fn reload_policies(&mut self) -> Result<usize, crate::policy_engine::PolicyError>
    pub fn policy_engine(&self) -> &PolicyEngine
    pub fn time_machine(&self) -> &TimeMachine
    pub fn time_machine_mut(&mut self) -> &mut TimeMachine

    // ── Agent lifecycle ──
    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError>
    pub fn stop_agent(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn pause_agent(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn resume_agent(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn restart_agent(&mut self, id: AgentId) -> Result<(), AgentError>

    // ── Fuel & metrics ──
    pub fn record_llm_spend(
        &mut self,
        id: AgentId,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_units: u64,
    ) -> Result<(), AgentError>
    pub fn fuel_audit_report(&self, id: AgentId) -> Option<FuelAuditReport>
    pub fn health_check(&self) -> Vec<AgentStatus>
    pub fn get_agent(&self, id: AgentId) -> Option<&AgentHandle>
    pub fn audit_trail(&self) -> &AuditTrail

    // ── Safety ──
    pub fn record_subsystem_metric(
        &mut self,
        id: AgentId,
        kind: KpiKind,
        value: f64,
    ) -> Result<(), AgentError>
    pub fn subsystem_gate_status(&self, subsystem: &str) -> Option<crate::kill_gates::GateStatus>
    pub fn manual_freeze_subsystem(
        &mut self,
        id: AgentId,
        subsystem: &str,
        operator_id: &str,
    ) -> Result<(), AgentError>
    pub fn manual_unfreeze_subsystem(
        &mut self,
        id: AgentId,
        subsystem: &str,
        operator_id: &str,
        hitl_tier: u8,
    ) -> Result<(), AgentError>
    pub fn manual_halt_agent(
        &mut self,
        id: AgentId,
        operator_id: &str,
        reason: &str,
    ) -> Result<(), AgentError>

    // ── Autonomy gates ──
    pub fn require_tool_call(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn require_multi_agent(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn require_self_modification(&mut self, id: AgentId) -> Result<(), AgentError>
    pub fn require_distributed(&mut self, id: AgentId) -> Result<(), AgentError>

    // ── Consent ──
    pub fn require_consent(
        &mut self,
        id: AgentId,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError>
    pub fn approve_consent(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError>
    pub fn deny_consent(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError>
    pub fn pending_consent_requests(
        &self,
        id: AgentId,
    ) -> Result<Vec<ApprovalRequest>, AgentError>

    // ── Speculative execution + consent ──
    pub fn require_consent_with_simulation(
        &mut self,
        id: AgentId,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError>
    pub fn approve_consent_with_simulation(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError>
    pub fn deny_consent_with_simulation(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError>
    pub fn simulation_for_request(&self, request_id: &str) -> Option<&SimulationResult>
    pub fn pending_simulations(&self) -> Vec<(&str, &SimulationResult)>

    // ── Permission dashboard ──
    pub fn get_agent_permissions(
        &self,
        id: AgentId,
    ) -> Result<Vec<PermissionCategory>, AgentError>
    pub fn update_agent_permission(
        &mut self,
        id: AgentId,
        capability_key: &str,
        enabled: bool,
        changed_by: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError>
    pub fn bulk_update_agent_permissions(
        &mut self,
        id: AgentId,
        updates: &[(String, bool)],
        changed_by: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError>
    pub fn get_permission_history(
        &self,
        id: AgentId,
    ) -> Result<Vec<PermissionHistoryEntry>, AgentError>
    pub fn get_capability_requests(
        &self,
        id: AgentId,
    ) -> Result<Vec<CapabilityRequest>, AgentError>
    pub fn lock_agent_capability(
        &mut self,
        id: AgentId,
        capability_key: &str,
    ) -> Result<(), AgentError>
    pub fn unlock_agent_capability(
        &mut self,
        id: AgentId,
        capability_key: &str,
    ) -> Result<(), AgentError>
}
```

---

## 3. AgentManifest — Exact Fields

**Source:** `kernel/src/manifest.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub allowed_endpoints: Option<Vec<String>>,
    #[serde(default)]
    pub domain_tags: Vec<String>,
    #[serde(default)]
    pub filesystem_permissions: Vec<FilesystemPermission>,
}
```

### Supporting types

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsPermissionLevel {
    ReadOnly,
    ReadWrite,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemPermission {
    pub path_pattern: String,
    pub permission: FsPermissionLevel,
}
```

### Capability registry (valid capability strings)

```rust
const CAPABILITY_REGISTRY: [&str; 15] = [
    "web.search",
    "web.read",
    "llm.query",
    "fs.read",
    "fs.write",
    "process.exec",
    "social.post",
    "social.x.post",
    "social.x.read",
    "messaging.send",
    "audit.read",
    "rag.ingest",
    "rag.query",
    "mcp.call",
    "computer.control",
];
```

### Public methods on AgentManifest

```rust
impl AgentManifest {
    pub fn check_fs_permission(&self, path: &str, needs_write: bool) -> Result<(), String>
}
```

### Public free function

```rust
pub fn parse_manifest(input: &str) -> Result<AgentManifest, AgentError>
```

---

## 4. Audit Trail — Exact API

**Source:** `kernel/src/audit/mod.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    StateChange,
    ToolCall,
    LlmCall,
    Error,
    UserAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: u64,
    pub agent_id: Uuid,
    pub event_type: EventType,
    pub payload: Value,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditError {
    #[error("audit batcher mutex poisoned — audit integrity compromised")]
    BatcherPoisoned,
    #[error("audit event serialization failed — fail-closed, no hash without payload")]
    SerializationFailed,
}

#[derive(Debug, Clone)]
pub struct BatcherConfig {
    pub max_events: usize,
    pub max_age_secs: u64,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self { max_events: 50, max_age_secs: 10 }
    }
}

pub trait BlockBatchSink: Send + Sync {
    fn seal_batch(&mut self, events: Vec<AuditEvent>);
}

#[derive(Debug, Clone, Default)]
pub struct AuditTrail {
    events: Vec<AuditEvent>,
    batcher: BatcherHandle, // private
}
```

### AuditTrail — All Public Methods

```rust
impl AuditTrail {
    pub fn new() -> Self
    pub fn enable_distributed_audit(
        &mut self,
        config: BatcherConfig,
        sink: Box<dyn BlockBatchSink>,
    )
    pub fn flush_batcher(&self) -> Result<(), AuditError>
    pub fn sealed_batch_count(&self) -> u64
    pub fn pending_batch_count(&self) -> usize
    pub fn append_event(
        &mut self,
        agent_id: Uuid,
        event_type: EventType,
        payload: Value,
    ) -> Result<Uuid, AuditError>
    pub fn events(&self) -> &[AuditEvent]
    pub fn verify_integrity(&self) -> bool
}
```

---

## 5. Error Types

**Source:** `kernel/src/errors.rs`

```rust
use crate::audit::AuditError;
use crate::fuel_hardening::FuelViolation;
use crate::lifecycle::AgentState;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentError {
    #[error("fuel budget exhausted")]
    FuelExhausted,
    #[error("fuel violation '{violation:?}': {reason}")]
    FuelViolation {
        violation: FuelViolation,
        reason: String,
    },
    #[error("invalid state transition from '{from}' to '{to}'")]
    InvalidTransition { from: AgentState, to: AgentState },
    #[error("manifest error: {0}")]
    ManifestError(String),
    #[error("capability denied: '{0}' is not allowed")]
    CapabilityDenied(String),
    #[error("approval required: request_id='{request_id}'")]
    ApprovalRequired { request_id: String },
    #[error("supervisor error: {0}")]
    SupervisorError(String),
    #[error("key '{0}' has been destroyed")]
    KeyDestroyed(String),
    #[error("audit failure: {0}")]
    AuditFailure(#[from] AuditError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorStrategy {
    Retry { max_attempts: u8 },
    Skip,
    Escalate,
}

pub fn on_error(error: &AgentError) -> ErrorStrategy
```

---

## 6. LLM Gateway — Exact API

**Source:** `connectors/llm/src/gateway.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeContext {
    pub agent_id: Uuid,
    pub capabilities: HashSet<String>,
    pub fuel_remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleEvent {
    pub agent_id: Uuid,
    pub prompt_hash: String,
    pub response_hash: String,
    pub model_name: String,
    pub response_text: String,
    pub token_count: u32,
    pub cost: f64,
    pub cost_units: u64,
    pub latency_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderSelectionConfig {
    pub provider: Option<String>,
    pub ollama_url: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
}

impl ProviderSelectionConfig {
    pub fn from_env() -> Self
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFuelBudgetConfig {
    pub period_id: BudgetPeriodId,
    pub cap_units: u64,
}

pub fn select_provider(config: &ProviderSelectionConfig) -> Box<dyn LlmProvider>

#[derive(Debug)]
pub struct GovernedLlmGateway<P: LlmProvider> {
    provider: P,
    audit_trail: AuditTrail,
    oracle_events: Vec<OracleEvent>,
    redaction_engine: RedactionEngine,
    semantic_boundary: SemanticBoundary,
    input_filter: InputFilter,
    egress_governor: EgressGovernor,
    fuel_model: FuelToTokenModel,
    default_period_id: BudgetPeriodId,
    fuel_ledgers: HashMap<Uuid, AgentFuelLedger>,
    safety_supervisor: SafetySupervisor,
}
```

### GovernedLlmGateway — All Public Methods

```rust
impl<P: LlmProvider> GovernedLlmGateway<P> {
    pub fn new(provider: P) -> Self
    pub fn with_redaction_policy(provider: P, policy: RedactionPolicy) -> Self
    pub fn register_agent_egress(&mut self, agent_id: Uuid, allowed_endpoints: Vec<String>)
    pub fn register_agent_egress_with_limit(
        &mut self,
        agent_id: Uuid,
        allowed_endpoints: Vec<String>,
        rate_limit_per_min: u32,
    )
    pub fn set_default_period(&mut self, period_id: impl Into<String>)
    pub fn set_model_cost(&mut self, model: impl Into<String>, cost: ModelCost)
    pub fn configure_agent_budget(&mut self, agent_id: Uuid, config: AgentFuelBudgetConfig)
    pub fn fuel_audit_report(&self, agent_id: Uuid) -> Option<FuelAuditReport>
    pub fn safety_mode(&self, agent_id: Uuid) -> OperatingMode
    pub fn query(
        &mut self,
        agent: &mut AgentRuntimeContext,
        prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError>
    pub fn query_with_origin(
        &mut self,
        agent: &mut AgentRuntimeContext,
        prompt: &str,
        max_tokens: u32,
        model: &str,
        origin: ContentOrigin,
    ) -> Result<LlmResponse, AgentError>
    pub fn audit_trail(&self) -> &AuditTrail
    pub fn audit_trail_mut(&mut self) -> &mut AuditTrail
    pub fn oracle_events(&self) -> &[OracleEvent]
    pub fn redaction_metrics(&self) -> &RedactionMetrics
    pub fn redaction_zero_pii_leakage_kpi(&self) -> bool
}
```

---

## 7. LLM Provider Trait

**Source:** `connectors/llm/src/providers/mod.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub output_text: String,
    pub token_count: u32,
    pub model_name: String,
    pub tool_calls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub model_name: String,
    pub token_count: u32,
}

pub trait LlmProvider: Send + Sync {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>;
    fn name(&self) -> &str;
    fn cost_per_token(&self) -> f64;
    fn is_paid(&self) -> bool {
        self.cost_per_token() > 0.0
    }
    fn requires_real_api_opt_in(&self) -> bool {
        false
    }
    fn estimate_input_tokens(&self, prompt: &str) -> u32;
    fn endpoint_url(&self) -> String {
        format!("provider://{}", self.name())
    }
    fn embed(&self, _texts: &[&str], _model: &str) -> Result<EmbeddingResponse, AgentError>;
}
```

### Box<T> impl (enables `Box<dyn LlmProvider>`)

```rust
impl<T: LlmProvider + ?Sized> LlmProvider for Box<T> {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>
    fn name(&self) -> &str
    fn cost_per_token(&self) -> f64
    fn is_paid(&self) -> bool
    fn requires_real_api_opt_in(&self) -> bool
    fn estimate_input_tokens(&self, prompt: &str) -> u32
    fn endpoint_url(&self) -> String
    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError>
}
```

---

## 8. Ollama Provider

**Source:** `connectors/llm/src/providers/ollama.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaProvider {
    base_url: String,
    streaming_timeout_secs: u32,
    request_timeout_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaPullProgress {
    pub status: String,
    pub total: u64,
    pub completed: u64,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>) -> Self
    pub fn with_streaming_timeout(mut self, secs: u32) -> Self
    pub fn with_request_timeout(mut self, secs: u32) -> Self
    pub fn from_env() -> Self
    pub fn base_url(&self) -> &str
    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest
    pub fn health_check(&self) -> Result<bool, AgentError>
    pub fn list_models(&self) -> Result<Vec<OllamaModel>, AgentError>
    pub fn chat_stream<F>(
        &self,
        messages: &[Value],
        model: &str,
        on_token: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str)
    pub fn pull_model<F>(&self, model_name: &str, mut on_progress: F) -> Result<String, AgentError>
    where
        F: FnMut(&str, u64, u64)
}

impl LlmProvider for OllamaProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>
    fn name(&self) -> &str  // returns "ollama"
    fn cost_per_token(&self) -> f64  // returns 0.0
    fn endpoint_url(&self) -> String
    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError>
}
```

---

## 9. Collaboration — Blackboard and Channel

**Source:** `agents/collaboration/src/blackboard.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
    Owner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub key: String,
    pub value: Value,
    pub owner: Uuid,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlackboardError {
    AccessDenied,
    KeyNotFound,
}

#[derive(Debug)]
pub struct Blackboard {
    pub entries: HashMap<String, BlackboardEntry>,
    pub acl: HashMap<(Uuid, String), AccessLevel>,
}

impl Default for Blackboard {
    fn default() -> Self { Self::new() }
}

impl Blackboard {
    pub fn new() -> Self
    pub fn grant_access(&mut self, agent_id: Uuid, key: &str, level: AccessLevel)
    pub fn write(&mut self, agent_id: Uuid, key: &str, value: Value) -> Result<(), BlackboardError>
    pub fn read(&self, agent_id: Uuid, key: &str) -> Result<&Value, BlackboardError>
    pub fn delete(&mut self, agent_id: Uuid, key: &str) -> Result<(), BlackboardError>
    pub fn list_keys(&self, agent_id: Uuid) -> Vec<String>
}
```

**Source:** `agents/collaboration/src/channel.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub message_type: String,
    pub payload: Value,
    pub timestamp: u64,
    pub requires_ack: bool,
}

impl AgentMessage {
    pub fn new(
        from: Uuid,
        to: Uuid,
        message_type: &str,
        payload: Value,
        requires_ack: bool,
    ) -> Self
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    WrongSender,
    MessageTypeNotAllowed(String),
    RateLimitExceeded,
    InsufficientFuel { required: u64 },
}

#[derive(Debug)]
pub struct GovernedChannel {
    pub id: Uuid,
    pub sender: Uuid,
    pub receiver: Uuid,
    pub allowed_message_types: Vec<String>,
    pub max_messages_per_minute: u32,
    pub fuel_cost_per_message: u64,
}

impl GovernedChannel {
    pub fn new(
        sender: Uuid,
        receiver: Uuid,
        allowed_message_types: Vec<String>,
        max_messages_per_minute: u32,
        fuel_cost_per_message: u64,
        initial_fuel: u64,
    ) -> Self
    pub fn send(&mut self, msg: AgentMessage) -> Result<(), ChannelError>
    pub fn recv(&mut self) -> Option<AgentMessage>
    pub fn messages_sent(&self) -> usize
    pub fn fuel_remaining(&self) -> u64
    pub fn audit_trail(&self) -> &AuditTrail
}
```

---

## 10. Agent Collaboration — Orchestrator

**Source:** `agents/collaboration/src/orchestrator.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub estimated_fuel: u64,
    pub status: SubTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub description: String,
    pub subtasks: Vec<SubTask>,
    pub assignments: HashMap<Uuid, Uuid>,  // subtask_id → agent_id
}

impl Task {
    pub fn get_subtask(&self, subtask_id: Uuid) -> Option<&SubTask>
}

#[derive(Debug)]
pub struct Orchestrator {
    pub agents: HashMap<Uuid, Vec<String>>,  // agent_id → capabilities
}

impl Default for Orchestrator {
    fn default() -> Self { Self::new() }
}

impl Orchestrator {
    pub fn new() -> Self
    pub fn register_agent(&mut self, id: Uuid, capabilities: Vec<String>)
    pub fn decompose(
        &self,
        description: &str,
        subtask_specs: Vec<(&str, Vec<String>, u64)>,
    ) -> Task
    pub fn assign(&self, task: &mut Task) -> usize
    pub fn complete_subtask(&self, task: &mut Task, subtask_id: Uuid) -> bool
    pub fn fail_subtask(&self, task: &mut Task, subtask_id: Uuid) -> bool
    pub fn is_complete(&self, task: &Task) -> bool
}
```

---

## 11. Factory — Intent Parser

**Source:** `factory/src/intent.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    ContentPosting,
    FileBackup,
    Research,
    Monitoring,
    SelfImprove,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedIntent {
    pub task_type: TaskType,
    pub platforms: Vec<String>,
    pub schedule: String,
    pub content_topic: String,
    pub raw_request: String,
}

pub struct IntentParser<P: LlmProvider> {
    // fields private
}

impl<P: LlmProvider> IntentParser<P> {
    pub fn new(provider: P, model_name: &str, fuel_budget: u64) -> Self
    pub fn parse(&mut self, request: &str) -> Result<ParsedIntent, AgentError>
    pub fn audit_oracle_count(&self) -> usize
}
```

---

## 12. Existing Agent Libs — Public APIs

### `agents/web-builder/src/lib.rs`

```rust
pub mod codegen;
pub mod deploy;
pub mod interpreter;
pub mod preview;
pub mod styles;
pub mod templates;
pub mod threejs;
```

### `agents/coder/src/lib.rs`

```rust
pub mod analyzer;
pub mod context;
pub mod editor;
pub mod fix_loop;
pub mod git;
pub mod init;
pub mod scanner;
pub mod terminal;
pub mod test_runner;
pub mod watcher;
pub mod writer;
```

### `agents/designer/src/lib.rs`

```rust
pub mod component_lib;
pub mod generator;
pub mod screenshot_to_code;
pub mod tokens;
```

---

## 13. Web Builder Internals

### `agents/web-builder/src/codegen.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChange {
    Create(String, String),        // (path, content)
    Modify(String, String, String), // (path, old_content, new_content)
    Delete(String),                 // (path)
}

pub fn generate_website(spec: &WebsiteSpec) -> Result<Vec<FileChange>, AgentError>
```

### `agents/web-builder/src/templates.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateCategory {
    Hero,
    Features,
    Testimonials,
    Pricing,
    Contact,
    Navigation,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub category: TemplateCategory,
    pub label: String,
    pub component_source: String,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateEngine {
    templates: HashMap<String, TemplateDefinition>,
}

impl TemplateEngine {
    pub fn new() -> Self
    pub fn get(&self, id: &str) -> Option<&TemplateDefinition>
    pub fn by_category(&self, category: TemplateCategory) -> Vec<&TemplateDefinition>
    pub fn render_component(&self, id: &str, title: &str, body: &str) -> Option<String>
}

pub fn default_template_engine() -> TemplateEngine
```

### `agents/web-builder/src/preview.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewHandle {
    pub url: String,
    pub command: String,
    pub project_path: String,
}

pub fn start_preview(project_path: impl AsRef<Path>) -> Result<PreviewHandle, AgentError>
```

### `agents/web-builder/src/threejs.rs`

```rust
pub fn generate_3d_scene(spec: &ThreeDSpec) -> String
pub fn scene_component_name(model: &str) -> String
```

### `agents/web-builder/src/interpreter.rs` (WebsiteSpec — used by codegen)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Framework {
    React,
    Vue,
    StaticHtml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSpec {
    pub name: String,
    pub layout: String,
    pub sections: Vec<SectionSpec>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSpec {
    pub kind: SectionKind,
    pub template_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionKind {
    Header,
    Hero,
    Features,
    Testimonials,
    Pricing,
    Menu,
    Contact,
    Footer,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSpec {
    pub colors: Vec<String>,
    pub fonts: Vec<String>,
    pub mood: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub name: String,
    pub props_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreeDSpec {
    pub model: String,
    pub animation: String,
    pub position: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationSpec {
    pub trigger: String,
    pub animation_type: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsiteSpec {
    pub pages: Vec<PageSpec>,
    pub theme: ThemeSpec,
    pub components: Vec<ComponentSpec>,
    pub three_d_elements: Vec<ThreeDSpec>,
    pub animations: Vec<AnimationSpec>,
    pub responsive: bool,
    pub framework: Framework,
}

pub struct DesignInterpreter { /* private */ }

impl Default for DesignInterpreter {
    fn default() -> Self { Self::new() }
}

impl DesignInterpreter {
    pub fn new() -> Self
    pub fn interpret(&mut self, description: &str) -> Result<WebsiteSpec, AgentError>
}

pub fn interpret(description: &str) -> Result<WebsiteSpec, AgentError>
```

### `agents/web-builder/src/deploy.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployProvider {
    Local,
    GitHubPages,
    Vercel,
    Netlify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub provider: DeployProvider,
    pub url: String,
    pub command: String,
}

pub struct Deployer { /* private */ }

impl Default for Deployer {
    fn default() -> Self { Self::new() }
}

impl Deployer {
    pub fn new() -> Self
    pub fn deploy_to(
        &mut self,
        provider: DeployProvider,
        project: impl AsRef<Path>,
    ) -> Result<DeploymentResult, AgentError>
    pub fn audit_events(&self) -> &[AuditEvent]
}

pub fn deploy_to(
    provider: DeployProvider,
    project: impl AsRef<Path>,
) -> Result<DeploymentResult, AgentError>
```

---

## 14. Coder Agent Internals

### `agents/coder/src/scanner.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Toml,
    Json,
    Yaml,
    Markdown,
    Shell,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub language: Language,
    pub last_modified: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub recent_commits: Vec<GitCommit>,
    pub contributors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMap {
    pub root_path: String,
    pub file_tree: Vec<FileEntry>,
    pub languages: HashMap<Language, usize>,
    pub entry_points: Vec<String>,
    pub config_files: Vec<String>,
    pub test_files: Vec<String>,
    pub total_lines: usize,
    pub git_info: Option<GitInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScannerConfig {
    pub depth_limit: usize,
    pub max_file_size_bytes: u64,
}

impl Default for ScannerConfig {
    fn default() -> Self  // depth_limit: 10, max_file_size_bytes: 1_048_576
}

pub fn scan_project(path: impl AsRef<Path>) -> Result<ProjectMap, AgentError>
pub fn scan_project_with_config(
    path: impl AsRef<Path>,
    config: ScannerConfig,
) -> Result<ProjectMap, AgentError>
pub fn detect_language(path: &Path, shebang_line: Option<&str>) -> Language
pub fn cargo_manifests(project_map: &ProjectMap) -> HashSet<String>
```

### `agents/coder/src/analyzer.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    RustWorkspace,
    RustCrate,
    NodeMonorepo,
    NodeProject,
    PythonPackage,
    GoModule,
    Polyglot,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiSurfaceEntry {
    pub file: String,
    pub symbol: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureReport {
    pub project_type: ProjectType,
    pub module_dependencies: HashMap<String, Vec<String>>,
    pub design_patterns: Vec<String>,
    pub test_frameworks: Vec<String>,
    pub api_surface: Vec<ApiSurfaceEntry>,
    pub dependency_graph: HashMap<String, Vec<String>>,
    pub llm_summary: Option<String>,
}

pub fn analyze(project_map: &ProjectMap) -> Result<ArchitectureReport, AgentError>
pub fn detect_project_type(project_map: &ProjectMap) -> ProjectType
```

### `agents/coder/src/writer.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChange {
    Create(String, String),
    Modify(String, String, String),
    Delete(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamingConvention {
    SnakeCase,
    CamelCase,
    PascalCase,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfile {
    pub indent_width: usize,
    pub uses_tabs: bool,
    pub naming_convention: NamingConvention,
    pub comment_style: String,
    pub import_organization: String,
    pub error_handling_pattern: String,
}

pub struct CodeWriter { /* private */ }

impl Default for CodeWriter {
    fn default() -> Self { Self::new() }
}

impl CodeWriter {
    pub fn new() -> Self
    pub fn write_code(
        &mut self,
        context: &CodeContext,
        task: &str,
    ) -> Result<Vec<FileChange>, AgentError>
    pub fn audit_events(&self) -> &[AuditEvent]
}

pub fn write_code(context: &CodeContext, task: &str) -> Result<Vec<FileChange>, AgentError>
pub fn detect_style(project_map: &ProjectMap) -> Result<StyleProfile, AgentError>
```

### `agents/coder/src/test_runner.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestFramework {
    Cargo,
    Npm,
    Pytest,
    Go,
    Jest,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestError {
    pub test_name: String,
    pub error_message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub stack_trace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    pub framework: TestFramework,
    pub passed: usize,
    pub failed: usize,
    pub errors: Vec<TestError>,
    pub stdout: String,
    pub stderr: String,
}

pub fn detect_test_framework(project: &ProjectMap) -> TestFramework
pub fn run_tests(project_path: impl AsRef<Path>) -> Result<TestResult, AgentError>
```

### `agents/coder/src/fix_loop.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixResult {
    Success {
        iterations: u32,
        applied_changes: usize,
        last_result: TestResult,
        audit_events: Vec<AuditEvent>,
    },
    MaxIterationsReached {
        iterations: u32,
        remaining_errors: Vec<TestError>,
        last_result: TestResult,
        audit_events: Vec<AuditEvent>,
    },
}

pub trait TestExecutor {
    fn run_tests(&mut self, project_path: &Path) -> Result<TestResult, AgentError>;
}

pub trait ErrorFixer {
    fn propose_fixes(
        &mut self,
        project_path: &Path,
        errors: &[TestError],
        iteration: u32,
    ) -> Result<Vec<FileChange>, AgentError>;
}

pub struct FrameworkTestExecutor;

impl TestExecutor for FrameworkTestExecutor {
    fn run_tests(&mut self, project_path: &Path) -> Result<TestResult, AgentError>
}

pub struct LlmErrorFixer { /* private */ }

impl Default for LlmErrorFixer {
    fn default() -> Self { Self::new() }
}

impl LlmErrorFixer {
    pub fn new() -> Self
}

impl ErrorFixer for LlmErrorFixer {
    fn propose_fixes(
        &mut self,
        _project_path: &Path,
        errors: &[TestError],
        iteration: u32,
    ) -> Result<Vec<FileChange>, AgentError>
}

pub fn fix_until_pass(
    project_path: impl AsRef<Path>,
    changes: Vec<FileChange>,
    max_iterations: u32,
) -> Result<FixResult, AgentError>

pub fn fix_until_pass_with(
    project_path: impl AsRef<Path>,
    changes: Vec<FileChange>,
    max_iterations: u32,
    executor: &mut dyn TestExecutor,
    fixer: &mut dyn ErrorFixer,
) -> Result<FixResult, AgentError>
```

### `agents/coder/src/context.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextFile {
    pub path: String,
    pub relevance_score: f64,
    pub reason: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeContext {
    pub task_description: String,
    pub files: Vec<ContextFile>,
    pub truncated_files: usize,
    pub total_chars: usize,
}

pub fn build_context(
    project_map: &ProjectMap,
    task_description: &str,
) -> Result<CodeContext, AgentError>
```

---

## 15. SDK — Agent Trait and Context

**Source:** `sdk/src/lib.rs` and submodules

### Re-exports from SDK

```rust
pub use agent_trait::{AgentOutput, NexusAgent};
pub use context::{AgentContext, ContextSideEffect};
pub use manifest::ManifestBuilder;
pub use module_cache::{ContentHash, ModuleCache};
pub use sandbox::{
    HostCallResult, HostFunction, InProcessSandbox, SandboxConfig, SandboxError, SandboxResult,
    SandboxRuntime,
};
pub use shadow_sandbox::{SafetyVerdict, ShadowResult, ShadowSandbox, SideEffect, ThreatDetector};
pub use testing::TestHarness;
pub use wasm_agent::WasmAgent;
pub use wasm_signature::{SignaturePolicy, SignatureVerification};
pub use wasmtime_host_functions::{SpeculativeDecision, SpeculativePolicy};
pub use wasmtime_sandbox::{WasmAgentState, WasmtimeSandbox};

// Kernel re-exports
pub use nexus_kernel::audit;
pub use nexus_kernel::autonomy;
pub use nexus_kernel::config;
pub use nexus_kernel::consent;
pub use nexus_kernel::errors;
pub use nexus_kernel::fuel_hardening;
pub use nexus_kernel::kill_gates;
pub use nexus_kernel::lifecycle;
pub use nexus_kernel::manifest as kernel_manifest;
pub use nexus_kernel::redaction;
pub use nexus_kernel::resource_limiter;
pub use nexus_kernel::supervisor;
```

### NexusAgent Trait

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub status: String,
    pub outputs: Vec<Value>,
    pub fuel_used: u64,
}

pub trait NexusAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError>;
    fn shutdown(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn checkpoint(&self) -> Result<Vec<u8>, AgentError>;
    fn restore(&mut self, _data: &[u8]) -> Result<(), AgentError>;
}
```

### AgentContext

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContextSideEffect {
    LlmQuery {
        prompt: String,
        max_tokens: u32,
        fuel_cost: u64,
    },
    FileRead { path: String, fuel_cost: u64 },
    FileWrite {
        path: String,
        content_size: usize,
        fuel_cost: u64,
    },
    ApprovalRequest { description: String },
    AuditEvent { payload: serde_json::Value },
    ToolExec {
        tool_name: String,
        input_json: String,
    },
}

#[derive(Debug, Clone)]
pub struct AgentContext { /* fields private */ }

impl AgentContext {
    pub fn new(agent_id: Uuid, capabilities: Vec<String>, fuel_budget: u64) -> Self
    pub fn with_filesystem_permissions(mut self, permissions: Vec<FilesystemPermission>) -> Self
    pub fn set_filesystem_permissions(&mut self, permissions: Vec<FilesystemPermission>)
    pub fn filesystem_permissions(&self) -> &[FilesystemPermission]
    pub fn agent_id(&self) -> Uuid
    pub fn fuel_remaining(&self) -> u64
    pub fn fuel_budget(&self) -> u64
    pub fn capabilities(&self) -> &[String]
    pub fn audit_trail(&self) -> &AuditTrail
    pub fn audit_trail_mut(&mut self) -> &mut AuditTrail
    pub fn approval_records(&self) -> &[ApprovalRecord]
    pub fn enable_recording(&mut self)
    pub fn disable_recording(&mut self)
    pub fn is_recording(&self) -> bool
    pub fn side_effects(&self) -> &[ContextSideEffect]
    pub fn drain_side_effects(&mut self) -> Vec<ContextSideEffect>
    pub fn record_side_effect(&mut self, effect: ContextSideEffect)
    pub fn require_capability(&self, capability: &str) -> Result<(), AgentError>
    pub fn llm_query(&mut self, prompt: &str, max_tokens: u32) -> Result<String, AgentError>
    pub fn read_file(&mut self, path: &str) -> Result<String, AgentError>
    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), AgentError>
    pub fn request_approval(&mut self, description: &str, agent_provided: bool) -> ApprovalRecord
    pub fn deduct_wasm_fuel(&mut self, units: u64)
    pub fn reserve_fuel(&mut self, cost: u64) -> Result<FuelReservation, AgentError>
    pub fn commit_reservation(&mut self, token: CommittedReservation)
    pub fn cancel_reservation(&mut self, token: CancelledReservation)
    pub fn return_leaked_reservation(&mut self, amount: u64)
    pub fn fuel_reserved(&self) -> u64
}
```

### ManifestBuilder

```rust
pub struct ManifestBuilder { /* fields private */ }

impl ManifestBuilder {
    pub fn new(name: &str) -> Self
    pub fn version(mut self, version: &str) -> Self
    pub fn capability(mut self, capability: &str) -> Self
    pub fn fuel_budget(mut self, budget: u64) -> Self
    pub fn autonomy_level(mut self, level: u8) -> Self
    pub fn build(self) -> Result<AgentManifest, AgentError>
}
```

### TestHarness

```rust
pub struct TestHarness { /* fields private */ }

impl Default for TestHarness {
    fn default() -> Self { Self::new() }
}

impl TestHarness {
    pub fn new() -> Self
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self
    pub fn with_fuel(mut self, fuel: u64) -> Self
    pub fn with_agent_id(mut self, agent_id: Uuid) -> Self
    pub fn with_filesystem_permissions(mut self, perms: Vec<FilesystemPermission>) -> Self
    pub fn build_context(self) -> AgentContext
}
```

### SandboxRuntime trait and InProcessSandbox

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub memory_limit_bytes: usize,
    pub execution_timeout_secs: u64,
    pub allowed_host_functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostFunction {
    LlmQuery { prompt: String, max_tokens: u32 },
    FsRead { path: String },
    FsWrite { path: String, content: String },
    RequestApproval { description: String },
}

impl HostFunction {
    pub fn name(&self) -> &str
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCallResult {
    Success { output: String },
    CapabilityDenied { function: String },
    FuelExhausted,
    TimedOut,
    MemoryExceeded,
    Error { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub completed: bool,
    pub outputs: Vec<String>,
    pub fuel_used: u64,
    pub host_calls_made: u64,
    pub killed: bool,
    pub kill_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    AlreadyKilled,
    ConfigError(String),
}

pub trait SandboxRuntime {
    fn execute(&mut self, agent_code: &[u8], ctx: &mut AgentContext) -> SandboxResult;
    fn kill(&mut self) -> Result<(), SandboxError>;
    fn memory_usage(&self) -> usize;
    fn elapsed_secs(&self) -> u64;
}

pub struct InProcessSandbox { /* private */ }

impl InProcessSandbox {
    pub fn new(config: SandboxConfig) -> Self
    pub fn check_limits(&mut self) -> Result<(), HostCallResult>
    pub fn call_host_function(
        &mut self,
        func: HostFunction,
        ctx: &mut AgentContext,
    ) -> HostCallResult
    pub fn simulate_memory_usage(&mut self, bytes: usize)
    pub fn is_killed(&self) -> bool
    pub fn kill_reason(&self) -> Option<&str>
}
```

---

## 16. Autonomy and Consent

### Autonomy (`kernel/src/autonomy.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum AutonomyLevel {
    #[default]
    L0,
    L1,
    L2,
    L3,
    L4,
    L5,
}

impl AutonomyLevel {
    pub fn as_str(self) -> &'static str
    pub fn from_numeric(value: u8) -> Option<Self>
    pub fn from_manifest(value: Option<u8>) -> Self
    pub fn previous(self) -> Self
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyGuard {
    level: AutonomyLevel,
    hooks: AutonomyPolicyHooks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPolicyHooks {
    pub tool_call_min: AutonomyLevel,
    pub multi_agent_min: AutonomyLevel,
    pub self_modification_min: AutonomyLevel,
    pub distributed_min: AutonomyLevel,
}

impl Default for AutonomyPolicyHooks {
    fn default() -> Self {
        Self {
            tool_call_min: AutonomyLevel::L1,
            multi_agent_min: AutonomyLevel::L2,
            self_modification_min: AutonomyLevel::L4,
            distributed_min: AutonomyLevel::L5,
        }
    }
}

impl Default for AutonomyGuard {
    fn default() -> Self { Self::new(AutonomyLevel::L0) }
}

impl AutonomyGuard {
    pub fn new(level: AutonomyLevel) -> Self
    pub fn with_hooks(level: AutonomyLevel, hooks: AutonomyPolicyHooks) -> Self
    pub fn level(&self) -> AutonomyLevel
    pub fn require_tool_call(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError>
    pub fn require_multi_agent(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError>
    pub fn require_self_modification(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError>
    pub fn require_distributed(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
    ) -> Result<(), AutonomyError>
    pub fn downgrade(
        &mut self,
        actor_id: Uuid,
        new_level: AutonomyLevel,
        action: &'static str,
        reason: &str,
        audit_trail: &mut AuditTrail,
    )
    pub fn require_level_with_policy(
        &mut self,
        actor_id: Uuid,
        audit_trail: &mut AuditTrail,
        default_required: AutonomyLevel,
        action: &'static str,
        policy_engine: Option<&PolicyEngine>,
    ) -> Result<(), AutonomyError>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AutonomyError {
    #[error("autonomy denied action ...")]
    Denied {
        required: AutonomyLevel,
        current: AutonomyLevel,
        action: &'static str,
        downgraded_to: Option<AutonomyLevel>,
    },
}
```

### Consent (`kernel/src/consent.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HitlTier {
    Tier0,
    Tier1,
    Tier2,
    Tier3,
}

impl HitlTier {
    pub fn as_str(self) -> &'static str
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GovernedOperation {
    ToolCall,
    TerminalCommand,
    SocialPostPublish,
    SelfMutationApply,
    MultiAgentOrchestrate,
    DistributedEnable,
    TimeMachineUndo,
}

impl GovernedOperation {
    pub fn as_str(self) -> &'static str
    pub fn display_label(self) -> &'static str
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn from_operation_tier(operation: GovernedOperation, tier: HitlTier) -> Self
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub operation: GovernedOperation,
    pub agent_id: String,
    pub payload_hash: String,
    pub requested_by: String,
    pub required_tier: HitlTier,
    pub created_seq: u64,
    pub display_summary: String,
    pub display_args: Vec<(String, String)>,
    pub risk_level: RiskLevel,
    pub raw_view: String,
}

impl ApprovalRequest {
    pub fn from_operation(
        id: String,
        operation: GovernedOperation,
        agent_id: String,
        payload_hash: String,
        requested_by: String,
        required_tier: HitlTier,
        created_seq: u64,
    ) -> Self
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone)]
pub struct ConsentRuntime {
    policy_engine: ConsentPolicyEngine,
    approval_queue: ApprovalQueue,
    requester_id: String,
    cedar_engine: Option<PolicyEngine>,
}

impl Default for ConsentRuntime { ... }

impl ConsentRuntime {
    pub fn new(
        policy_engine: ConsentPolicyEngine,
        approval_queue: ApprovalQueue,
        requester_id: String,
    ) -> Self
    pub fn from_manifest(
        consent_policy_path: Option<&str>,
        requester_id: Option<&str>,
        default_requester: &str,
    ) -> Result<Self, AgentError>
    pub fn set_cedar_engine(&mut self, engine: PolicyEngine)
    pub fn cedar_engine(&self) -> Option<&PolicyEngine>
    pub fn policy_engine(&self) -> &ConsentPolicyEngine
    pub fn policy_engine_mut(&mut self) -> &mut ConsentPolicyEngine
    pub fn pending_requests(&self) -> Vec<ApprovalRequest>
    pub fn enforce_operation(
        &mut self,
        operation: GovernedOperation,
        agent_id: Uuid,
        payload: &[u8],
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError>
    pub fn approve(
        &mut self,
        request_id: &str,
        approver_id: &str,
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError>
    pub fn deny(
        &mut self,
        request_id: &str,
        approver_id: &str,
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError>
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsentError {
    ApprovalRequired { request_id: String, operation: GovernedOperation, required_tier: HitlTier },
    RequestDenied { request_id: String },
    RequestNotFound(String),
    DuplicateApprover { request_id: String, approver_id: String },
    ApproverNotAllowed { request_id: String, approver_id: String },
    SelfApprovalRejected { request_id: String, approver_id: String },
    QueueStorage(String),
    PolicyDenied { reason: String },
    AuditFailed(String),
    TimedOut { request_id: String, timeout_secs: u64 },
    NoApproversConfigured { request_id: String },
    PayloadIntegrityFailed { request_id: String },
}
```

---

## 17. Delegation

**Source:** `kernel/src/delegation.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConstraints {
    pub max_fuel: u64,
    pub max_duration_secs: u64,
    pub max_depth: u8,
    pub require_approval: bool,
}

impl Default for DelegationConstraints {
    fn default() -> Self {
        Self {
            max_fuel: 1000,
            max_duration_secs: 3600,
            max_depth: 1,
            require_approval: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DelegationError {
    #[error("grantor does not own capability: {0}")]
    CapabilityNotOwned(String),
    #[error("delegation depth exceeded")]
    DepthExceeded,
    #[error("delegation grant expired")]
    Expired,
    #[error("delegation grant revoked")]
    Revoked,
    #[error("delegated fuel budget exhausted")]
    FuelExhausted,
    #[error("delegation grant not found")]
    NotFound,
}

#[derive(Debug)]
pub struct DelegationEngine {
    grants: HashMap<Uuid, DelegationGrant>,
    agent_capabilities: HashMap<Uuid, Vec<String>>,
}

impl Default for DelegationEngine {
    fn default() -> Self { Self::new() }
}

impl DelegationEngine {
    pub fn new() -> Self
    pub fn register_agent(&mut self, id: Uuid, capabilities: Vec<String>)
    pub fn has_capability(&self, agent_id: Uuid, capability: &str) -> bool
    pub fn delegate(
        &mut self,
        grantor: Uuid,
        grantee: Uuid,
        capabilities: Vec<String>,
        constraints: DelegationConstraints,
    ) -> Result<DelegationGrant, DelegationError>
    pub fn consume_delegated_fuel(
        &mut self,
        grant_id: Uuid,
        amount: u64,
    ) -> Result<(), DelegationError>
    pub fn revoke(&mut self, grant_id: Uuid) -> Result<(), DelegationError>
    pub fn expire_grants(&mut self) -> Vec<Uuid>
    pub fn active_grants_for(&self, agent_id: Uuid) -> Vec<&DelegationGrant>
}
```

---

## 18. Time Machine

**Source:** `kernel/src/time_machine.rs`

```rust
#[derive(Debug, Clone)]
pub enum TimeMachineError {
    CheckpointNotFound(String),
    UndoFailed(String),
    RedoFailed(String),
    Io(String),
    CapacityExceeded(usize),
    EmptyHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEntry {
    FileWrite { path: String, before: Option<Vec<u8>>, after: Vec<u8> },
    FileDelete { path: String, before: Vec<u8> },
    FileCreate { path: String, after: Vec<u8> },
    AgentStateChange { agent_id: String, field: String, before: Value, after: Value },
    ConfigChange { key: String, before: Value, after: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub label: String,
    pub timestamp: u64,
    pub agent_id: Option<String>,
    pub changes: Vec<ChangeEntry>,
    pub undone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    RestoreFile { path: String, content: Option<Vec<u8>> },
    DeleteFile { path: String },
    RestoreAgentState { agent_id: String, field: String, value: Value },
    RestoreConfig { key: String, value: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMachineConfig {
    pub max_checkpoints: usize,
    pub max_file_size_bytes: u64,
    pub auto_checkpoint: bool,
}

impl Default for TimeMachineConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 200,
            max_file_size_bytes: 10_485_760, // 10 MB
            auto_checkpoint: true,
        }
    }
}

pub struct CheckpointBuilder { /* private */ }

impl CheckpointBuilder {
    pub fn new(label: &str, agent_id: Option<String>, max_file_size: u64) -> Self
    pub fn record_file_write(&mut self, path: &str, before: Option<Vec<u8>>, after: Vec<u8>)
    pub fn record_file_create(&mut self, path: &str, after: Vec<u8>)
    pub fn record_file_delete(&mut self, path: &str, before: Vec<u8>)
    pub fn record_agent_state(&mut self, agent_id: &str, field: &str, before: Value, after: Value)
    pub fn record_config_change(&mut self, key: &str, before: Value, after: Value)
    pub fn change_count(&self) -> usize
    pub fn build(self) -> Checkpoint
}

#[derive(Debug)]
pub struct TimeMachine {
    config: TimeMachineConfig,
    checkpoints: Vec<Checkpoint>,
    redo_stack: Vec<Checkpoint>,
}

impl Default for TimeMachine {
    fn default() -> Self { Self::new(TimeMachineConfig::default()) }
}

impl TimeMachine {
    pub fn new(config: TimeMachineConfig) -> Self
    pub fn begin_checkpoint(&self, label: &str, agent_id: Option<String>) -> CheckpointBuilder
    pub fn commit_checkpoint(
        &mut self,
        checkpoint: Checkpoint,
    ) -> Result<(String, usize), TimeMachineError>
    pub fn undo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError>
    pub fn redo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError>
    pub fn undo_checkpoint(
        &mut self,
        id: &str,
    ) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError>
    pub fn list_checkpoints(&self) -> &[Checkpoint]
    pub fn get_checkpoint(&self, id: &str) -> Option<&Checkpoint>
    pub fn checkpoint_count(&self) -> usize
    pub fn config(&self) -> &TimeMachineConfig
}
```

---

## 19. CLI Structure

**Source:** `cli/src/lib.rs` and `cli/src/commands.rs`

### Pattern: Clap derive macros

```rust
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "nexus", about = "NEXUS OS command-line interface")]
pub struct Cli {
    #[command(subcommand)]
    pub command: TopLevelCommand,
}
```

### All Top-Level Subcommands

```rust
#[derive(Debug, Subcommand)]
pub enum TopLevelCommand {
    Create { name: String, #[arg(short, long, default_value = "basic")] template: String, #[arg(short, long)] output_dir: Option<String> },
    Test { path: String },
    Package { path: String },
    Agent { #[command(subcommand)] command: AgentCommand },
    Sandbox { #[command(subcommand)] command: SandboxCommand },
    Simulation { #[command(subcommand)] command: SimulationCommand },
    Voice { #[command(subcommand)] command: VoiceCommand },
    Setup { #[arg(long)] check: bool },
    SelfImprove { #[command(subcommand)] command: SelfImproveCommand },
    Model { #[command(subcommand)] command: ModelCommand },
    Governance { #[command(subcommand)] command: GovernanceCommand },
    Policy { #[command(subcommand)] command: PolicyCommand },
    Protocols { #[command(subcommand)] command: ProtocolsCommand },
    Marketplace { #[command(subcommand)] command: MarketplaceCommand },
}
```

### Nested: AgentCommand (example)

```rust
#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    Create { manifest: String },
    Start { agent_id: String, #[arg(long)] dry_run: bool },
    Stop { agent_id: String },
    Pause { agent_id: String },
    Resume { agent_id: String },
    Destroy { agent_id: String },
    List,
    Logs { agent_id: String },
    Audit { agent_id: String },
}
```

### Example handler wiring

```rust
pub fn execute_agent_command(command: AgentCommand) -> Result<String, String> {
    match command {
        AgentCommand::Create { manifest } => create_agent_from_path(Path::new(&manifest))
            .map_err(|error| format!("Failed to create agent: {error}")),
        AgentCommand::Start { agent_id, dry_run } => start_agent(agent_id.as_str(), dry_run),
        AgentCommand::Stop { agent_id } => Ok(format!("Agent '{agent_id}' stopped successfully")),
        AgentCommand::Pause { agent_id } => Ok(format!("Agent '{agent_id}' paused successfully")),
        AgentCommand::Resume { agent_id } => Ok(format!("Agent '{agent_id}' resumed successfully")),
        AgentCommand::Destroy { agent_id } => Ok(format!("Agent '{agent_id}' destroyed successfully")),
        AgentCommand::List => Ok("Listing all registered agents".to_string()),
        AgentCommand::Logs { agent_id } => Ok(format!("Showing logs for agent '{agent_id}'")),
        AgentCommand::Audit { agent_id } => Ok(format!("Showing audit trail for agent '{agent_id}'")),
    }
}
```

### Unified CliCommand enum (`cli/src/commands.rs`)

All commands are also representable via a flat `CliCommand` enum for programmatic use. Key variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    AgentList,
    AgentStart { name: String },
    AgentStop { name: String },
    AgentStatus { name: String },
    AuditShow { count: usize },
    AuditVerify,
    AuditExport { run_id: Uuid, path: String },
    ClusterStatus,
    ClusterJoin { addr: String },
    ClusterLeave,
    MarketplaceSearch { query: String },
    MarketplaceInstall { name: String },
    MarketplaceUninstall { name: String },
    MarketplacePublish { bundle_path: String },
    DelegationGrant { grantor: Uuid, grantee: Uuid, capabilities: Vec<String> },
    DelegationRevoke { grant_id: Uuid },
    DelegationList { agent_id: Uuid },
    SandboxStatus,
    SimulationStatus,
    ModelList,
    ModelDownload { model_id: String },
    ModelLoad { model_id: String },
    ModelUnload,
    ModelStatus,
    PolicyList,
    PolicyShow { policy_id: String },
    PolicyValidate { file: String },
    PolicyTest { file: String, principal: String, action: String, resource: String },
    PolicyReload,
    GovernanceTest { task_type: String, input: String },
    ProtocolsStatus,
    ProtocolsAgentCard { agent_name: String },
    ProtocolsStart { port: u16 },
    IdentityShow { agent_id: Uuid },
    IdentityList,
    FirewallStatus,
    FirewallPatterns,
    // ... and more
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliOutput {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CliOutput {
    pub fn ok(message: impl Into<String>) -> Self
}
```

---

## 20. Desktop App — Tauri Commands

**Source:** `app/src-tauri/src/main.rs`

### Pattern

- Commands use `#[tauri::command]` macro
- State is injected via `tauri::State<'_, AppState>`
- Window events via `tauri::Window`
- Parameters JSON-deserialized from frontend, return types JSON-serialized
- All commands registered in `tauri::generate_handler![...]`

### Example command handlers

```rust
#[tauri::command]
fn list_agents(state: tauri::State<'_, AppState>) -> Result<Vec<AgentRow>, String>

#[tauri::command]
fn create_agent(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    manifest_json: String,
) -> Result<String, String>

#[tauri::command]
fn start_agent(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<(), String>

#[tauri::command]
fn get_audit_log(
    state: tauri::State<'_, AppState>,
    agent_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AuditRow>, String>

#[tauri::command]
fn send_chat(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<ChatResponse, String>
```

### Frontend invocation pattern

```typescript
// From React/TypeScript:
const agents = await invoke<AgentRow[]>("list_agents");
const id = await invoke<string>("create_agent", { manifestJson: json });
await invoke("start_agent", { agentId: id });
```

### Registered commands (partial list — 140+ total)

```rust
builder.invoke_handler(tauri::generate_handler![
    list_agents,
    create_agent,
    start_agent,
    stop_agent,
    pause_agent,
    resume_agent,
    get_audit_log,
    send_chat,
    get_config,
    save_config,
    start_jarvis_mode,
    stop_jarvis_mode,
    jarvis_status,
    transcribe_push_to_talk,
    get_agent_permissions,
    update_agent_permission,
    get_permission_history,
    bulk_update_permissions,
    marketplace_search,
    marketplace_install,
    marketplace_info,
    marketplace_publish,
    policy_list,
    policy_validate,
    policy_test,
    time_machine_list_checkpoints,
    time_machine_undo,
    time_machine_redo,
    terminal_execute,
    factory_create_project,
    factory_build_project,
    factory_test_project,
    execute_tool,
    list_tools,
    mcp_host_list_servers,
    mcp_host_add_server,
    mcp_host_connect,
    mcp_host_call_tool,
    // ... 100+ more handlers
])
```

---

## Appendix: Designer Agent APIs

### `agents/designer/src/generator.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    Page,
    Sidebar,
    Header,
    Main,
    CardGrid,
    ChartArea,
    Footer,
    Section,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutNode {
    pub id: String,
    pub kind: LayoutKind,
    pub children: Vec<LayoutNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignComponent {
    pub name: String,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypographySpec {
    pub display_font: String,
    pub body_font: String,
    pub mono_font: String,
    pub base_size_px: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacingSpec {
    pub base_unit_px: u8,
    pub scale: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignSpec {
    pub layout_tree: LayoutNode,
    pub components: Vec<DesignComponent>,
    pub colors: Vec<String>,
    pub typography: TypographySpec,
    pub spacing: SpacingSpec,
    pub svg_mockup: String,
    pub react_component: String,
}

pub struct DesignGenerator { /* private */ }

impl Default for DesignGenerator {
    fn default() -> Self { Self::new() }
}

impl DesignGenerator {
    pub fn new() -> Self
    pub fn generate_design(&mut self, description: &str) -> Result<DesignSpec, AgentError>
}

pub fn generate_design(description: &str) -> Result<DesignSpec, AgentError>
```

### `agents/designer/src/component_lib.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandGuide {
    pub brand_name: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub neutral_color: String,
    pub spacing_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedComponent {
    pub name: String,
    pub react_tsx: String,
    pub storybook_story: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentLibrary {
    pub brand_guide: BrandGuide,
    pub components: Vec<GeneratedComponent>,
    pub dark_mode: bool,
    pub responsive: bool,
    pub accessibility_notes: Vec<String>,
}

pub fn generate_library(brand_guide: &BrandGuide) -> ComponentLibrary
```
