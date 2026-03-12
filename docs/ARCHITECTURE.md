# Nexus OS Architecture Guide

> Version 7.0.0 | Don't trust. Verify.

## Overview

Nexus OS is a governed AI agent operating system written in Rust. Every agent action passes through capability checks, fuel metering, and audit logging before execution. The system enforces the principle: **no action without governance**.

## Layered Architecture

```
+---------------------------------------------------------------+
|                     Desktop UI (Tauri + React)                 |
|  15 Built-In Apps | Command Center | Setup Wizard | 33 Pages  |
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

Tauri desktop shell with React/TypeScript frontend. 33 pages including 15 built-in apps.

### Core Pages

| Page | Description |
|------|-------------|
| Command Center | Live agent grid with status, fuel, autonomy controls |
| Audit Timeline | Chronological audit events with federation cross-references |
| Marketplace Browser | Search, install, and verify agent packages |
| Compliance Dashboard | SOC 2 control status with evidence tracking |
| Cluster Status | Node health, heartbeat monitoring, quorum status |
| Trust Dashboard | Per-agent trust scores with promotion/demotion indicators |
| Setup Wizard | Hardware detection, Ollama setup, model download, agent configuration |

### Built-In Apps (Phase 7)

| App | Description |
|-----|-------------|
| Code Editor | Monaco-based, 50+ languages, agent-assisted coding |
| Design Studio | AI canvas, component library, design tokens |
| Terminal | 30+ commands, governed execution, HITL blocking |
| File Manager | Grid/list view, drag-drop, encrypted vault |
| Database Manager | SQL editor, visual query builder, ERD schema |
| API Client | Request builder, governed vault, rate limiting |
| Notes | Rich markdown, templates, agent auto-notes |
| Email Client | IMAP/SMTP, threading, PII redaction |
| Project Manager | Kanban, sprints, burndown charts |
| Media Studio | Image editor, AI generation, OCR |
| System Monitor | CPU/RAM/GPU graphs, per-agent resource tracking |
| App Store | Ed25519 verification, reviews, publishing |
| AI Chat Hub | 9 models, comparison mode, voice/Jarvis mode |
| Deploy Pipeline | 4 providers, environments, SSL/domains |
| Learning Center | Courses, code challenges, XP leveling |

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

## Crate Dependency Graph

```
                         app (Tauri desktop)
                              |
                         cli (24 commands)
                        /     |     \
                enterprise  marketplace  distributed
                    |         |    |        |
                    +---------+----+--------+
                              |
                   connectors (LLM, web, social, messaging, control)
                              |
                     +--------+--------+
                     |                 |
                  agents (9)         sdk
                     |                 |
                     +---------+-------+
                               |
                            kernel (governance root)
```

**Dependency rule:** Agents depend on `nexus-sdk`, never on `nexus-kernel` directly. The SDK re-exports kernel types via `nexus_sdk::prelude::*`.

## Data Flow

### Agent Action Flow

```
User/Trigger
    |
    v
Agent.execute()
    |
    v
AgentContext.require_capability("web.search")
    |
    v
Kernel Supervisor
    |--- Capability Check (manifest registry)
    |--- Fuel Check (budget remaining?)
    |--- Autonomy Gate (level sufficient?)
    |--- HITL Gate (Tier1+ needs approval)
    |
    v
Execute Action
    |
    v
AuditTrail.append_event() --- hash-chained, append-only
    |
    v
Fuel Ledger.deduct() --- anomaly detection
    |
    v
Result returned to agent
```

### LLM Query Flow (Local)

```
Agent requests LLM query
    |
    v
PII Redaction (gateway boundary)
    |
    v
Ollama / Candle inference (local)
    |
    v
Response returned (never leaves device)
```

### Distributed Consensus Flow

```
Node A proposes governance change
    |
    v
QuorumPropose message (TCP, length-prefix framed)
    |
    v
Nodes B, C, D receive and vote
    |
    v
Quorum reached? --- Yes: apply change, replicate audit event
                 --- No: reject proposal
```

## Key Design Decisions

1. **Kernel as trust root** — All governance flows through the kernel. No bypass path exists. This makes the security surface auditable.

2. **Fuel before execution** — Fuel is checked *before* an action runs, not after. An agent that exhausts its budget mid-action would leave the system in an inconsistent state.

3. **Hash-chained audit** — Each audit event includes the hash of the previous event. Tampering with any event breaks the chain, making corruption detectable.

4. **PII redaction at boundary** — PII is stripped before data reaches the LLM, not after. This is a design constraint, not a feature flag.

5. **TOML manifests** — Agent capabilities are declared statically in TOML, not requested dynamically. The kernel rejects any action not declared in the manifest.

6. **SDK indirection** — Agents use the SDK, not the kernel directly. This allows the kernel to evolve its internals without breaking agent code.

7. **Zero unsafe Rust** — `#![forbid(unsafe_code)]` is set workspace-wide. All memory safety is guaranteed by the compiler.

8. **Local-first AI** — LLM inference runs on the user's device via Ollama/Candle. No data leaves the machine unless the user explicitly configures a cloud endpoint.

## Security Invariants

These invariants are enforced at the code level and must never be violated:

1. Every agent action goes through kernel capability checks
2. Fuel budget checked **before** execution, not after
3. Audit trail is append-only with hash-chain integrity
4. PII redaction at LLM gateway boundary
5. HITL approval mandatory for Tier 1+ operations
6. `unsafe_code = "forbid"` — zero unsafe Rust
7. All tests must pass before merging
8. Agents declare capabilities in TOML manifests
