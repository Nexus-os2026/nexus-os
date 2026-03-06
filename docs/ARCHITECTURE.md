# Nexus OS Architecture Guide

> Version 5.0.0 | Don't trust. Verify.

## Overview

Nexus OS is a governed AI agent operating system written in Rust. Every agent action passes through capability checks, fuel metering, and audit logging before execution. The system enforces the principle: **no action without governance**.

## Layered Architecture

```
+---------------------------------------------------------------+
|                     Desktop UI (Tauri + React)                 |
|  Command Center | Audit Timeline | Marketplace | Compliance   |
+---------------------------------------------------------------+
|                          CLI Layer                             |
|  nexus agent | nexus audit | nexus cluster | nexus marketplace|
+---------------------------------------------------------------+
|                       Enterprise Layer                         |
|         RBAC | Compliance Reporting | SSO (stub)               |
+---------------------------------------------------------------+
|                      Marketplace Layer                         |
|     Package Registry | Trust Scoring | Manifest Verification   |
+---------------------------------------------------------------+
|                     Distributed Layer                          |
|   TCP Transport | Replication | Quorum | Membership | Node     |
+---------------------------------------------------------------+
|                       SDK Layer                                |
|  NexusAgent Trait | AgentContext | ManifestBuilder | Sandbox   |
+---------------------------------------------------------------+
|                        Kernel Layer                            |
|  Supervisor | Autonomy | Audit | Fuel | Delegation | Privacy  |
+---------------------------------------------------------------+
```

## Kernel (`kernel/`)

The kernel is the trust root. All governance decisions flow through it.

### Core Modules

| Module | Purpose |
|--------|---------|
| `supervisor` | Agent lifecycle management. Registers agents, transitions state, enforces fuel and capability checks on every action. |
| `autonomy` | Six-level autonomy system (L0-L5). `AutonomyGuard` gates actions by level. Violations trigger automatic downgrade. |
| `audit` | Append-only audit trail with hash-chain integrity. Every event is cryptographically linked to its predecessor. |
| `manifest` | TOML manifest parser and validator. Enforces capability registry, fuel limits, and name constraints. |
| `fuel_hardening` | Fuel ledger with per-period tracking, monthly caps, burn anomaly detection, and violation reporting. |
| `lifecycle` | Agent state machine: Created -> Starting -> Running -> Paused -> Stopping -> Stopped -> Destroyed. |
| `delegation` | Transitive capability delegation with constraints (max fuel, duration, depth). Cascade revocation. |
| `adaptive_policy` | Trust scores computed from run history. Automatic promotion/demotion of autonomy levels. |
| `consent` | Human-in-the-loop approval gates. `GovernedOperation` requires explicit consent for sensitive actions. |
| `privacy` | PII detection and redaction at the LLM gateway boundary. |
| `redaction` | Pattern-based content redaction before data leaves the system. |
| `replay` | Evidence bundle generation and verification. Deterministic replay of agent actions. |
| `safety_supervisor` | KPI monitoring with automatic safety actions when thresholds are breached. |
| `kill_gates` | Emergency kill switches for halting agent execution. |
| `hardware_security` | Hardware security module integration (TPM/HSM stubs). |
| `orchestration` | Multi-agent orchestration with role assignment and messaging. |
| `config` | System configuration management. |

### Governance Pipeline

Every agent action follows this pipeline:

```
Agent requests action
    |
    v
1. Capability Check --- Does the manifest allow this?
    |
    v
2. Fuel Check --------- Is there budget remaining?
    |
    v
3. Autonomy Gate ------ Is the autonomy level sufficient?
    |
    v
4. HITL Approval ------ Does this need human approval? (L1/L2)
    |
    v
5. Execute Action
    |
    v
6. Audit Event -------- Append to hash-chained audit trail
    |
    v
7. Fuel Deduction ----- Deduct cost from remaining budget
```

### Autonomy Levels

| Level | Name | Behavior |
|-------|------|----------|
| L0 | Inert | No actions permitted |
| L1 | Suggest | Agent suggests, human decides |
| L2 | Act-with-approval | Agent acts after human approves |
| L3 | Act-then-report | Agent acts, reports after |
| L4 | Autonomous-bounded | Fully autonomous, anomaly-triggered review |
| L5 | Full autonomy | Kernel override only |

### Capability Registry

Agents declare capabilities in their TOML manifest. The kernel validates each capability against the registry:

```
web.search    web.read      llm.query     fs.read       fs.write
process.exec  social.post   social.x.post social.x.read messaging.send
audit.read
```

Any capability not in the registry is rejected at manifest parse time.

## SDK (`sdk/`)

The SDK provides the developer interface for building governed agents.

### NexusAgent Trait

All agents implement the `NexusAgent` trait:

```rust
pub trait NexusAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError>;
    fn shutdown(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;
    fn checkpoint(&self) -> Result<Vec<u8>, AgentError>;
    fn restore(&mut self, data: &[u8]) -> Result<(), AgentError>;
}
```

### AgentContext

`AgentContext` provides capability-gated, fuel-metered operations:

- `require_capability(cap)` - Check capability before acting
- `llm_query(prompt, max_tokens)` - Query LLM (10 fuel)
- `read_file(path)` - Read file (2 fuel)
- `write_file(path, content)` - Write file (8 fuel)
- `request_approval(description)` - Request HITL approval

Every operation emits an audit event automatically.

### ManifestBuilder

Fluent builder for constructing agent manifests programmatically:

```rust
let manifest = ManifestBuilder::new("my-agent")
    .version("1.0.0")
    .capability("llm.query")
    .capability("fs.read")
    .fuel_budget(5000)
    .autonomy_level(2)
    .build()?;
```

### Sandbox

In-process sandbox for running untrusted agents with configurable memory and time limits. Host functions bridge the sandbox to kernel governance.

## Distributed Layer (`distributed/`)

Cross-node governance replication and consensus.

| Module | Purpose |
|--------|---------|
| `tcp_transport` | Real TCP networking with length-prefix framing, exponential backoff reconnection |
| `transport` | Transport trait abstraction |
| `replication` | Audit event replication across nodes |
| `quorum` | Quorum-based voting for governance decisions |
| `membership` | SWIM-style membership protocol |
| `node` | Node identity and configuration |

### Wire Protocol

Messages use 4-byte big-endian length-prefix framing. Message types:

- `Heartbeat` - Node liveness
- `AuditSync` - Audit event replication
- `QuorumPropose` / `QuorumVote` - Consensus
- `ReplicationFull` / `ReplicationDelta` - State sync
- `AuthChallenge` / `AuthResponse` - Node authentication

## Enterprise Layer (`enterprise/`)

| Module | Purpose |
|--------|---------|
| `rbac` | Role-based access control for multi-tenant deployments |
| `compliance` | SOC 2 Type II compliance reporting and control tracking |

## Marketplace (`marketplace/`)

| Module | Purpose |
|--------|---------|
| `registry` | Agent package registry with search and discovery |
| `package` | Signed bundle format for agent distribution |
| `manifest_verify` | Cryptographic manifest verification |
| `trust` | Publisher trust scoring |
| `scanner` | Security scanning of agent bundles |
| `install` | Safe installation with sandboxed verification |

## Desktop UI (`app/`)

Tauri desktop shell with React/TypeScript frontend.

### Pages

| Page | Description |
|------|-------------|
| Command Center | Live agent grid with status, fuel, autonomy controls |
| Audit Timeline | Chronological audit events with federation cross-references |
| Marketplace Browser | Search, install, and verify agent packages |
| Compliance Dashboard | SOC 2 control status with evidence tracking |
| Cluster Status | Node health, heartbeat monitoring, quorum status |
| Trust Dashboard | Per-agent trust scores with promotion/demotion indicators |

## CLI (`cli/`)

Full command-line interface covering all subsystems:

```
nexus agent list|start|stop|status
nexus audit show|verify|export|federation-status
nexus cluster status|join|leave
nexus marketplace search|install|uninstall
nexus compliance report|status
nexus delegation grant|revoke|list
nexus benchmark run|report
nexus finetune create|approve|status
```

All commands support `--json` for structured output.

## Workspace Crates

| Crate | Path | Purpose |
|-------|------|---------|
| `nexus-kernel` | `kernel/` | Core governance runtime |
| `nexus-sdk` | `sdk/` | Agent development SDK |
| `nexus-distributed` | `distributed/` | Cross-node networking |
| `nexus-enterprise` | `enterprise/` | RBAC and compliance |
| `nexus-marketplace` | `marketplace/` | Agent marketplace |
| `nexus-cli` | `cli/` | Command-line interface |
| `nexus-cloud` | `cloud/` | Cloud deployment scaffolding |
| `nexus-benchmarks` | `benchmarks/` | Performance benchmarks |
| `nexus-connectors-*` | `connectors/` | External service connectors (core, web, social, messaging, LLM) |
| `nexus-workflows` | `workflows/` | Workflow engine |
| `nexus-analytics` | `analytics/` | Telemetry and analytics |
| `nexus-adaptation` | `adaptation/` | Self-improvement primitives |
| `nexus-control` | `control/` | Control plane |
| `nexus-factory` | `factory/` | Agent factory |
| `nexus-self-update` | `self-update/` | Self-update mechanism |
| `nexus-research` | `research/` | Research utilities |
| `nexus-content` | `content/` | Content management |

## Agents

Built-in agents live in `agents/`:

| Agent | Purpose |
|-------|---------|
| `coder` | Code generation and modification |
| `designer` | UI/UX design generation |
| `coding-agent` | Advanced coding assistance |
| `screen-poster` | Screen capture and posting |
| `self-improve` | System self-improvement |
| `social-poster` | Social media management |
| `web-builder` | Web application building |
| `workflow-studio` | Visual workflow creation |
| `collaboration` | Multi-agent collaboration |

## Security Invariants

These invariants are enforced at the code level and must never be violated:

1. Every agent action goes through kernel capability checks
2. Fuel budget checked **before** execution, not after
3. Audit trail is append-only with hash-chain integrity
4. PII redaction at LLM gateway boundary
5. HITL approval mandatory for Tier 1+ operations
6. `unsafe_code = "forbid"` - zero unsafe Rust
7. All tests must pass before merging
8. Agents declare capabilities in TOML manifests
