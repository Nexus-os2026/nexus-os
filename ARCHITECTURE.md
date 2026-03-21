# Nexus OS Architecture

## Overview

Nexus OS is a **governed AI agent operating system** built on four architectural pillars:

1. **Sovereignty**: All computation and data remain on the user's machine
2. **Governance**: Every agent action is capability-gated, metered, audited, and revocable
3. **Evolution**: Agents improve autonomously through Darwinian selection
4. **Interoperability**: Standard protocols (A2A, MCP) enable cross-platform agent communication

This document describes the system architecture from kernel to UI.

---

## System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                        │
│         React/TypeScript · 50 Pages · Tauri 2.0 Shell           │
├─────────────────────────────────────────────────────────────────┤
│                        Command Layer                             │
│            397 Tauri Commands (Rust ↔ TypeScript IPC)            │
├─────────────────────────────────────────────────────────────────┤
│                     Orchestration Layer                           │
│          Nexus Conductor · A2A Protocol · MCP Protocol           │
├─────────────────────────────────────────────────────────────────┤
│                       Agent Layer                                │
│    53 Agents (L0-L6) · 47 Genomes · Darwinian Evolution         │
├─────────────────────────────────────────────────────────────────┤
│                     Governance Layer                              │
│  Capability ACL · HITL · Fuel Meter · PII · Output Firewall     │
├─────────────────────────────────────────────────────────────────┤
│                       Kernel Layer                               │
│  WASM Sandbox · Hash-Chain Audit · DID/Ed25519 · Key Store      │
├─────────────────────────────────────────────────────────────────┤
│                      Provider Layer                              │
│  Ollama · NVIDIA NIM (42 models) · OpenAI · Anthropic · Others  │
├─────────────────────────────────────────────────────────────────┤
│                     Infrastructure                               │
│  Tauri 2.0 · tokio async runtime · SQLite · OS Keychain         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Kernel

The kernel is the trust root of Nexus OS. Written entirely in Rust with zero `unsafe` blocks in governance-critical paths.

### WASM Sandboxing (nexus-sandbox)

Agent code executes inside wasmtime-powered sandboxes:

```rust
pub struct AgentSandbox {
    engine: wasmtime::Engine,
    store: wasmtime::Store<AgentState>,
    instance: wasmtime::Instance,
    fuel_limit: u64,
    memory_limit_bytes: usize,
}
```

**Key design decisions:**
- **wasmtime over wasmer**: Bytecode Alliance backing, superior security audit history, cranelift JIT
- **Fuel-based metering**: wasmtime's built-in fuel mechanism counts instructions executed, not wall-clock time — deterministic and reproducible
- **No WASI filesystem by default**: Agents cannot touch the filesystem unless granted explicit capability tokens
- **Speculative execution shadow**: High-risk agent actions are first executed in a shadow sandbox; only committed to real state after HITL approval

### Hash-Chained Audit (nexus-audit)

```rust
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: SystemTime,
    pub agent_did: String,
    pub action: AuditAction,
    pub capability_used: CapabilityToken,
    pub fuel_consumed: u64,
    pub previous_hash: [u8; 32],  // SHA-256 of previous entry
    pub signature: Ed25519Signature, // Agent's signature
}

impl AuditEntry {
    pub fn hash(&self) -> [u8; 32] {
        // SHA-256(id || timestamp || agent_did || action || ... || previous_hash)
    }
    
    pub fn verify_chain(entries: &[AuditEntry]) -> Result<(), AuditError> {
        // O(n) traversal verifying each entry's previous_hash
    }
}
```

**Storage**: Append-only SQLite with WAL mode. No UPDATE or DELETE operations permitted on audit tables. Database file integrity verified on startup.

### Agent Identity (nexus-identity)

```rust
pub struct AgentIdentity {
    pub did: DecentralizedIdentifier,  // did:nexus:agent:<uuid>
    pub keypair: Ed25519Keypair,
    pub created_at: SystemTime,
    pub capabilities: Vec<CapabilityToken>,
    pub trust_chain: Vec<TrustLink>,
}
```

**Key management:**
- Keys generated using OS CSPRNG (`getrandom` crate)
- Private keys stored in OS keychain (libsecret/Keychain/Credential Manager)
- Key rotation creates a new keypair linked to the same DID via trust chain
- OIDC-A JWT tokens issued for cross-system authentication

---

## Layer 2: Governance

The governance layer enforces policy between the kernel and agents. Every agent action passes through this layer — there is no bypass path.

### Capability-Based Access Control

```rust
pub struct CapabilityToken {
    pub id: Uuid,
    pub agent_did: String,
    pub resource: ResourcePath,      // e.g., "filesystem:/workspace/output"
    pub permissions: Permissions,     // Read, Write, Execute, Delegate
    pub fuel_budget: u64,
    pub expires_at: Option<SystemTime>,
    pub requires_hitl: bool,
    pub issued_by: String,           // DID of issuer (kernel or delegating agent)
    pub signature: Ed25519Signature, // Issuer's signature — unforgeable
}
```

**Evaluation flow:**
1. Agent requests action
2. Kernel checks agent's capability tokens
3. If capability exists and is valid → check fuel budget
4. If fuel sufficient → check HITL requirement
5. If HITL required → await human approval (tokio::sync::Notify)
6. If approved → execute in sandbox, deduct fuel, write audit entry
7. If any check fails → deny, write denial audit entry

### Fuel Metering (nexus-fuel)

Fuel is a universal resource budget that prevents runaway agents:

| Operation | Fuel Cost |
|-----------|-----------|
| LLM inference (local) | 1,000 per request |
| LLM inference (cloud API) | 5,000 per request |
| Filesystem read | 100 per operation |
| Filesystem write | 500 per operation |
| Network request | 1,000 per request |
| Agent-to-agent message | 200 per message |
| WASM instruction execution | 1 per instruction |

Fuel is allocated per-agent, per-session. Exhaustion triggers graceful shutdown with audit entry.

### HITL Gates (nexus-hitl)

```rust
pub struct HitlGate {
    notify: Arc<tokio::sync::Notify>,
    decision: Arc<Mutex<Option<HitlDecision>>>,
}

impl HitlGate {
    pub async fn request_approval(&self, context: HitlContext) -> HitlDecision {
        // Send context to frontend via Tauri command
        // Await notification (no polling, no deadlock)
        self.notify.notified().await;
        self.decision.lock().unwrap().take().unwrap_or(HitlDecision::Deny)
    }
}
```

**Design choice**: `tokio::sync::Notify` over polling — zero CPU waste, no deadlock risk, instant response when user decides.

### PII Redaction (nexus-pii)

Multi-strategy detection:
1. **Regex patterns**: Email, phone, SSN, credit card, IP address
2. **Luhn validation**: Credit card number verification
3. **NER-based**: Named entity recognition for person names
4. **Custom rules**: User-configurable patterns via TOML

Redaction happens at three points:
- Before LLM provider receives the prompt
- Before audit trail records the action
- Before any output reaches the frontend

---

## Layer 3: Agents

### Agent Manifest

Every agent declares its identity, capabilities, and behavior in a TOML manifest:

```toml
[agent]
name = "coder-agent"
version = "1.0.0"
autonomy_level = 3  # L3: Supervised
description = "Autonomous coding agent with HITL for destructive operations"

[agent.identity]
did_method = "nexus"
key_algorithm = "Ed25519"

[agent.capabilities]
filesystem_read = ["/workspace/src/**"]
filesystem_write = ["/workspace/output/**"]
llm_providers = ["ollama/codellama", "anthropic/claude-sonnet"]
network = []
max_fuel = 1_000_000

[agent.hitl]
required_for = ["filesystem.write", "llm.provider_switch"]
timeout_seconds = 300
default_on_timeout = "deny"

[agent.genome]
id = "genome-coder-v3.2"
fitness_criteria = ["code_correctness", "test_pass_rate", "token_efficiency"]
evolution_eligible = true
```

### Autonomy Levels (L0–L6)

| Level | Kernel Enforcement |
|-------|--------------------|
| L0 Passive | All outputs are read-only. No write capabilities issued. |
| L1 Assistive | Suggestions generated but all actions require HITL approval. |
| L2 Conditional | Pre-approved action set. Anything outside triggers HITL. |
| L3 Supervised | Autonomous within declared capabilities. HITL for edge cases. |
| L4 Autonomous | Full autonomy within capability bounds. HITL only on escalation. |
| L5 Collaborative | Can delegate capabilities to other agents via Conductor. |
| L6 Evolving | Can modify own genome within evolution constraints. |

**Key insight**: Autonomy level is not "trust level." An L6 agent has the same governance constraints as L0 — it simply has additional evolution capabilities. All governance checks (capability ACL, fuel, HITL, audit) apply uniformly across all levels.

---

## Layer 4: Evolution (Nexus Darwin Core)

### Architecture

```
┌─────────────────────────────────────────┐
│           Nexus Darwin Core              │
│                                          │
│  ┌──────────────┐  ┌─────────────────┐  │
│  │  Adversarial  │  │     Swarm       │  │
│  │    Arena      │  │  Coordinator    │  │
│  └──────┬───────┘  └───────┬─────────┘  │
│         │                   │            │
│  ┌──────▼───────────────────▼─────────┐  │
│  │      Plan Evolution Engine         │  │
│  │  (Mutation · Crossover · Selection)│  │
│  └──────────────────┬────────────────-┘  │
│                     │                    │
│  ┌──────────────────▼────────────────┐   │
│  │       Fitness Evaluator           │   │
│  │  (Automated scoring + LLM judge)  │   │
│  └───────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

### Evolution Pipeline

1. **Population Initialization**: N agents with diverse genomes deployed on identical tasks
2. **Adversarial Arena**: Agents compete head-to-head; outputs scored by automated metrics + LLM judge
3. **Fitness Evaluation**: Multi-criteria scoring (correctness, efficiency, safety, token usage)
4. **Selection**: Top K genomes survive; bottom performers are culled
5. **Mutation**: LLM-driven prompt mutation — not random character flipping, but semantic variation
6. **Crossover**: Successful strategies from different genomes are recombined
7. **Swarm Coordination**: Top genomes are shared across agent populations
8. **HITL Checkpoint**: Every N generations, evolved genomes require human review before deployment

**Governance constraint**: Evolution is not unbounded. Genomes cannot evolve capabilities beyond their agent's manifest. The fitness function includes a governance compliance score — agents that bypass safety mechanisms are penalized, not rewarded.

---

## Layer 5: Orchestration (Nexus Conductor)

The Conductor manages multi-agent workflows:

```rust
pub struct ConductorPlan {
    pub id: Uuid,
    pub agents: Vec<AgentAssignment>,
    pub dependencies: DirectedAcyclicGraph<TaskId>,
    pub timeout: Duration,
    pub fuel_budget: u64,
    pub hitl_checkpoints: Vec<TaskId>,
}
```

**Features:**
- DAG-based task scheduling with dependency resolution
- Parallel execution of independent tasks
- Fuel budget shared across all agents in a plan
- A2A protocol for cross-framework agent communication
- MCP protocol for tool/resource access
- Automatic retry with exponential backoff
- Plan-level HITL checkpoints at configurable intervals

### Protocol Support

**A2A (Agent-to-Agent)**: Google's open protocol for inter-agent communication. Enables Nexus agents to collaborate with agents from other frameworks.

**MCP (Model Context Protocol)**: Anthropic's protocol (now Linux Foundation) for connecting agents to tools and data sources. Nexus agents can use any MCP-compatible tool.

---

## Layer 6: Presentation

### Tauri 2.0 Shell

397 Tauri commands bridge the Rust backend to the React/TypeScript frontend:

```rust
#[tauri::command]
async fn agent_execute(
    state: State<'_, AppState>,
    agent_did: String,
    task: TaskInput,
) -> Result<TaskResult, NexusError> {
    // 1. Resolve agent identity
    // 2. Verify capabilities
    // 3. Check fuel budget
    // 4. Execute in sandbox
    // 5. Write audit entry
    // 6. Return result
}
```

**Why Tauri over Electron:**
- 10 MB binary vs. 100+ MB
- Rust backend vs. Node.js (memory safety, performance)
- Native OS webview vs. bundled Chromium
- Lower memory footprint (100-200 MB vs. 500 MB+)

### Frontend Architecture

```
frontend/
├── src/
│   ├── pages/           # 50 pages
│   │   ├── AgentHub/
│   │   ├── GenomeLab/
│   │   ├── Conductor/
│   │   ├── AuditViewer/
│   │   ├── Governance/
│   │   ├── Identity/
│   │   ├── LLMRouter/
│   │   ├── ComputerControl/
│   │   ├── WorldSimulation/
│   │   ├── VoiceInterface/
│   │   ├── PluginMarketplace/
│   │   └── ComplianceCenter/
│   ├── components/      # Shared UI components
│   ├── hooks/           # Tauri command hooks
│   ├── store/           # State management
│   └── utils/           # Helpers
```

---

## Provider Layer

### Flash Inference (nexus-llama-bridge + nexus-flash-infer)

Local model inference powered by llama.cpp, supporting 60+ GGUF model architectures with zero external runtime dependencies.

```
┌─────────────────────────────────────────────────────┐
│                  Flash Inference                      │
│                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ Model       │  │ Memory       │  │ Auto       │  │
│  │ Catalog     │  │ Budget Mgr   │  │ Config     │  │
│  └──────┬──────┘  └──────┬───────┘  └─────┬──────┘  │
│         │                │                 │          │
│  ┌──────▼────────────────▼─────────────────▼──────┐  │
│  │            nexus-flash-infer                    │  │
│  │   (quantization selection, context sizing,     │  │
│  │    thread tuning, batch optimization)          │  │
│  └────────────────────┬───────────────────────────┘  │
│                       │                               │
│  ┌────────────────────▼───────────────────────────┐  │
│  │            nexus-llama-bridge                   │  │
│  │   (Rust FFI → llama.cpp C API)                 │  │
│  └─────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

**Key design decisions:**
- **llama.cpp over other runtimes**: Widest architecture support (60+ models), active development, pure C/C++ with no Python dependency
- **Memory budget manager**: Automatically selects quantization level and context size based on available system RAM — prevents OOM on constrained machines
- **No Ollama dependency**: Direct FFI to llama.cpp eliminates the need for an external runtime process
- **Full governance pipeline**: FlashProvider implements the same LlmProvider trait as cloud providers — every inference call passes through capability check, fuel metering, adversarial arena, PII redaction, output firewall, and hash-chained audit

**Verified performance (CPU only, 62GB RAM):**
| Model | Parameters | Type | tok/s |
|-------|-----------|------|-------|
| Gemma 2 2B | 2B | Dense | 9.93 |
| Qwen3.5-35B-A3B | 35B (3B active) | MoE | 8.36 |

### LLM Router

The LLM Router manages model selection, fallback, and load balancing:

```rust
pub struct LlmRouter {
    providers: Vec<Box<dyn LlmProvider>>,
    routing_strategy: RoutingStrategy,
    fallback_chain: Vec<ProviderId>,
    rate_limiter: RateLimiter,
}

pub enum RoutingStrategy {
    Priority,           // First available in order
    RoundRobin,        // Distribute evenly
    CostOptimized,     // Cheapest provider first
    LatencyOptimized,  // Fastest provider first
    Capability,        // Match model to task requirements
}
```

### NVIDIA NIM Integration

42+ models accessible via single `nvapi-` API key:

| Provider | Notable Models |
|----------|---------------|
| NVIDIA | Nemotron, Llama-3.1-Nemotron-Ultra |
| DeepSeek | V3.1 Terminus |
| GLM | GLM-4.7 |
| Kimi | K2 |
| Mistral | Mistral Large, Codestral |
| Meta | Llama 3.3 70B |
| Google | Gemma 2 |
| Microsoft | Phi-4 |
| And more | 12 providers total |

---

## Data Flow

### Agent Execution Flow

```
User Action
    │
    ▼
Tauri Command (frontend → Rust)
    │
    ▼
Agent Resolution (DID lookup)
    │
    ▼
Capability Check ──── DENY → Audit(denied) → Error Response
    │
    ▼ (PASS)
Fuel Check ─────── INSUFFICIENT → Audit(fuel_exhausted) → Error Response
    │
    ▼ (PASS)
HITL Check ─────── REQUIRED → Await Approval ── DENIED → Audit(hitl_denied)
    │                                              │
    ▼ (NOT REQUIRED or APPROVED)                   ▼
WASM Sandbox Execution                          Error Response
    │
    ▼
Output Firewall ── BLOCKED → Audit(output_blocked) → Sanitized Response
    │
    ▼ (PASS)
PII Redaction
    │
    ▼
Audit Entry (hash-chained, signed)
    │
    ▼
Response to Frontend
```

---

## Performance Characteristics

| Metric | Value | Notes |
|--------|-------|-------|
| Cold start (desktop app) | ~2s | Tauri native, not Electron |
| Agent sandbox creation | ~5ms | wasmtime JIT compilation cached |
| Capability check | <1ms | In-memory token lookup |
| Audit entry write | ~2ms | SQLite WAL mode |
| HITL gate latency | 0ms (system) | User response time is variable |
| Hash chain verification | ~1ms per 1000 entries | SHA-256 batch verification |

---

## Design Principles

1. **Default deny**: Agents start with zero capabilities. Everything is opt-in.
2. **Governance is not optional**: There is no way to disable the governance layer.
3. **Local first**: Network access is a capability, not a default.
4. **Audit everything**: If it happened, there's a signed, hash-chained record.
5. **Evolve safely**: Agent evolution is bounded by the same governance as execution.
6. **Verify, don't trust**: Cryptographic verification at every layer boundary.
