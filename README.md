[![pipeline status](https://gitlab.com/nexaiceo/nexus-os/badges/main/pipeline.svg)](https://gitlab.com/nexaiceo/nexus-os/-/commits/main)

<div align="center">

# Nexus OS

### The Governed Agentic AI Operating System

**65 Crates | 658 Commands | 84 Pages | 5,029 Tests | 10/10 OWASP | Zero Stubs**

*Local-first. Air-gappable. Post-quantum ready. Built in Rust.*

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-317K%20lines-orange.svg)](https://www.rust-lang.org/)
[![Tauri 2.0](https://img.shields.io/badge/shell-Tauri%202.0-24C8D8.svg)](https://tauri.app/)
[![Tests](https://img.shields.io/badge/tests-5%2C029%20passing-brightgreen.svg)](#post-audit-status)
[![OWASP](https://img.shields.io/badge/OWASP%20Agentic-10%2F10-brightgreen.svg)](#security--governance)
[![v10.5.0](https://img.shields.io/badge/version-10.5.0-green.svg)](CHANGELOG.md)

[Architecture](#architecture) | [Quick Start](#quick-start) | [Features](#features) | [Audit Status](#post-audit-status) | [Docs](docs/)

</div>

---

Nexus OS is an AI agent operating system where agents are first-class citizens with cryptographic identities, governed autonomy, and the ability to evolve. It runs entirely on your hardware — no cloud dependency, no data leaving your machine, air-gappable. Every action is hash-chained, every decision auditable, every agent sandboxed.

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Nexus OS v10.5.0                              │
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │ Coder    │  │ Research │  │ Security │  │ DevOps   │  ...54     │
│  │ Agent    │  │ Agent    │  │ Agent    │  │ Agent    │  agents    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘            │
│       │              │              │              │                  │
│  ┌────▼──────────────▼──────────────▼──────────────▼─────┐           │
│  │              Nexus Conductor (Orchestration)           │           │
│  │       A2A Protocol  ·  MCP Protocol  ·  Swarm          │           │
│  └───────────────────────┬────────────────────────────────┘           │
│                          │                                            │
│  ┌───────────────────────▼────────────────────────────────┐           │
│  │                   Governance Kernel                     │           │
│  │  Capability ACL · HITL Gates · Fuel Metering            │           │
│  │  OWASP 10/10 · Ed25519 Consent · PII Redaction          │           │
│  │  Hash-Chain Audit · WASM Sandbox · Cedar Policies        │           │
│  └───────────────────────┬────────────────────────────────┘           │
│                          │                                            │
│  ┌───────────────────────▼────────────────────────────────┐           │
│  │                    LLM Providers (15)                    │           │
│  │  Ollama · OpenAI · Claude · Gemini · Groq · DeepSeek    │           │
│  │  NVIDIA NIM · OpenRouter · Mistral · Cohere · Fireworks  │           │
│  │  Together · Perplexity · Flash (llama.cpp) · + Mock      │           │
│  └────────────────────────────────────────────────────────┘           │
│                                                                      │
│  Rust Kernel (317K LOC) · Tauri 2.0 Shell · React/TS Frontend (64K)  │
└──────────────────────────────────────────────────────────────────────┘
```

## Why Nexus OS?

| Problem | Everyone Else | Nexus OS |
|---------|--------------|----------|
| **Data sovereignty** | Send everything to the cloud | 100% local-first, air-gappable |
| **Agent safety** | Trust the agent, hope for the best | WASM sandbox, capability ACL, fuel limits |
| **Agent identity** | Anonymous function calls | Ed25519 cryptographic identity per agent |
| **Audit trail** | Logs (deletable, mutable) | Hash-chained audit trail (tamper-evident) |
| **Human oversight** | Optional, bolted on | HITL consent gates built into kernel |
| **Agent evolution** | Static prompts forever | Darwinian evolution: agents mutate, compete, improve |
| **Compliance** | "We're working on it" | EU AI Act conformity, OWASP Agentic 10/10 |
| **Performance** | Python + Electron | Rust kernel + Tauri 2.0 (5MB binary vs 100MB+) |
| **Vendor lock-in** | Pick one cloud provider | 15 LLM providers, 200+ models, swap freely or go offline |
| **Security standard** | Ad-hoc | OWASP Agentic Top 10 — all 10 defenses with 62 tests |

## Features

### Governance and Security
- **OWASP Agentic Top 10** — All 10 defenses implemented and tested (62 dedicated tests)
- **Ed25519 consent signing** — Tier2+ approvals are cryptographically non-repudiable
- **Post-quantum ready** — Ed25519 + X25519 with ML-DSA/ML-KEM roadmap (nexus-crypto)
- **Prompt firewall** — 20 injection patterns, PII redaction, output filtering
- **Cedar-inspired policies** — Declarative capability rules with formal evaluation
- **HITL consent gates** — 4 tiers (Tier0-3) with configurable approval counts
- **Hash-chained audit** — Tamper-evident, append-only, cryptographically verifiable
- **Checkpoint-rollback** — 3-level recovery: memory, execution, side-effect compensation

### Agent Intelligence
- **Cognitive loop** — Perceive, reason, plan, act, reflect, learn cycle
- **Darwin Core** — Adversarial arena, swarm coordination, evolutionary strategies
- **Capability measurement** — 4-vector scoring with gaming detection
- **Predictive routing** — Model selection optimized for latency, cost, and task complexity
- **Memory subsystem** — 4 types (working, episodic, semantic, procedural) with SQLite, GC, rollback, ACL
- **54 prebuilt agents** across 7 autonomy levels (L0-L6)

### Protocols and Integration
- **MCP** — JSON-RPC 2.0 client with subprocess tool discovery
- **A2A** — Agent-to-agent discovery, task delegation, status tracking
- **OpenAI-compatible REST API** — Chat completions, tool calls, SSE streaming
- **Messaging adapters** — Slack, Discord, Matrix, Telegram, WhatsApp, Webhook
- **Migration tool** — Import from CrewAI and LangGraph

### Infrastructure
- **Token economy** — Wallets, fuel metering, delegation contracts, tier gating
- **Software factory** — Project creation, build pipeline, quality gates
- **Marketplace** — Ed25519 signed packages with verification pipeline
- **World simulation** — Virtual filesystem sandbox with dry-run and risk assessment
- **Collaboration protocol** — Multi-agent sessions with voting and consensus

### Desktop and Local-First
- **Flash inference** — llama.cpp via FFI, GGUF loading, speculative decoding
- **15 LLM providers** — All with real HTTP calls, automatic failover
- **Voice pipeline** — Push-to-talk, Whisper transcription, Web Speech API
- **Browser automation** — Playwright integration with URL allowlist and financial blocking
- **Computer control** — Screen capture, input execution, governance-gated at L4+
- **84 frontend pages** — Full management UI for every subsystem

## Flash Inference — Run Any Open-Source Model Locally

Built-in local inference via llama.cpp, supporting 60+ model architectures through GGUF format. Run Qwen, DeepSeek, Llama, Mistral, Gemma, Phi, and more — all governed with the same security pipeline as cloud providers.

| Model | Parameters | Type | RAM Cage | tok/s |
|-------|-----------|------|----------|-------|
| Gemma 2 2B | 2B | Dense | — | 9.93 |
| Qwen3.5-35B-A3B | 35B (3B active) | MoE | — | 8.36 |
| Qwen3.5-397B-A17B | 397B (17B active) | MoE | 32 GB | 0.26 |

> **397B model verified running in 32 GB systemd memory cage via mmap SSD streaming. No GPU required.**

Every inference call passes through: capability check, fuel reserve, adversarial arena, PII redaction, output firewall, hash-chained audit trail.

## Quick Start

### Prerequisites
- **OS:** Linux (Ubuntu 22.04+), macOS, or Windows 10+
- **Rust:** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Node.js:** 18+ (`nvm install 18`)
- **Ollama** (optional): `curl -fsSL https://ollama.ai/install.sh | sh`

### Build and Run

```bash
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os

# Build workspace
cargo build --workspace

# Build frontend
cd app && npm ci && npm run build && cd ..

# Run in development mode
cd app && npm run tauri dev
```

## Post-Audit Status

Independent audit completed 2026-03-31. Every metric verified by inspecting source code.

| Category | Score | Evidence |
|----------|------:|---------|
| Feature completeness | 10/10 | 23 features, all real implementations, 0 stubs |
| Rust test coverage | 9/10 | 4,687 tests, 0 failures, every crate tested |
| Frontend test coverage | 9/10 | 342 tests, 84/84 pages (100% coverage) |
| Build health | 10/10 | fmt clean, clippy clean, 0 compile warnings |
| Error handling | 9/10 | 0 production unwrap/expect in kernel + Tauri |
| Code organization | 9/10 | 31K monolith split into 18 domain modules |
| Command wiring | 10/10 | 0 phantom commands, 0 unwired frontends |
| Security posture | 10/10 | OWASP 10/10, Ed25519 signing, unsafe_code = forbid |
| **Overall** | **9.5/10** | |

```
Tauri Commands .... 658     Agents ............ 54
Rust Crates ....... 65      Frontend Pages .... 84
Rust Tests ........ 4,687   Frontend Tests .... 342
LLM Providers ..... 15      OWASP Score ....... 10/10
Production TODOs .. 0       Production Stubs .. 0
```

## Security and Governance

### OWASP Agentic Top 10 — Complete Coverage

| # | Risk | Defense | Tests |
|---|------|---------|------:|
| 1 | Goal Hijacking | GoalIntegrityGuard — SHA-256 hash + drift detection | 7 |
| 2 | Tool Poisoning | ToolPoisoningGuard — output scan + rate limit + audit | 5 |
| 3 | Privilege Escalation | PrivilegeEscalationGuard — L4+ hard-gate | 5 |
| 4 | Delegated Trust | DelegationNarrowing — capability subset enforcement | 5 |
| 5 | Injection Cascade | CascadeGuard — inter-agent scan + chain depth | 5 |
| 6 | Memory Poisoning | MemoryWriteValidator — sanitize + rate limit | 8 |
| 7 | Supply Chain | RuntimePackageVerifier — Ed25519 load-time verification | 7 |
| 8 | Cascading Failures | CircuitBreakerManager — Closed/Open/HalfOpen | 5 |
| 9 | Insecure Logging | SecureLogger — PII/credential redaction + hash chain | 5 |
| 10 | Insufficient Monitoring | AnomalyMonitor — spike detection + auto-suspend | 5 |

### Capability-Based Access Control

```
Agent: coder-agent (L3)
Capabilities:
  file.read(scope: /workspace/*), file.write(scope: /workspace/*)
  llm.query(provider: ollama, model: codestral)
Denied:
  network.external, file.write(scope: /system/*), process.execute(elevated: true)
Fuel: 10,000 units/session
HITL: Required for file.delete, process.execute
```

## Project Structure

```
nexus-os/
├── kernel/          109K lines — governance, cognitive loop, actuators, audit
├── app/src-tauri/     30K lines — 658 Tauri commands across 18 domain modules
├── app/src/           64K lines — 84 React pages, 342 tests
├── connectors/        23K lines — LLM (15 providers), messaging (6 channels), web, social
├── crates/            52K lines — memory, crypto, A2A, MCP, measurement, simulation, ...
├── agents/            18K lines — 8 agent crates + 54 prebuilt manifests
├── sdk/                8K lines — agent-facing API wrapping kernel
├── distributed/        9K lines — P2P, ghost protocol, mesh
├── protocols/          7K lines — HTTP gateway, OpenAI-compat API, MCP client
├── marketplace/        4K lines — Ed25519 signing, SQLite registry
├── enterprise/         7K lines — auth, tenancy, integrations, metering, telemetry
└── cli/                6K lines — CLI tools, packager
```

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](ARCHITECTURE.md) | System design, component diagrams, data flow |
| [Security Policy](SECURITY.md) | Vulnerability reporting, security model |
| [Compliance](COMPLIANCE.md) | EU AI Act, SOC 2, NIST, Singapore AI Governance |
| [Threat Model](THREAT_MODEL.md) | Adversarial threat analysis |
| [Privacy Design](PRIVACY_DESIGN.md) | Privacy-by-design principles |
| [Changelog](CHANGELOG.md) | Version history and release notes |
| [Contributing](CONTRIBUTING.md) | How to contribute |

## Roadmap

- [x] v10.5.0 — Post-audit hardening: 9.5/10 audit score, OWASP 10/10
- [x] v10.4.0 — Agent memory, PQC crypto, migration tool, OpenAI-compat API
- [x] v10.3.0 — Full audit: 54 agents, 655 commands, 84 pages
- [x] Darwin Core — Darwinian evolution engine with adversarial arena
- [x] Flash Inference — llama.cpp integration, 397B model verified
- [ ] Docker + Helm chart for server/K8s deployment
- [ ] SOC 2 Type II / NIST 800-53 formal certification
- [ ] Governed Self-Improvement (capstone — agents improve the OS itself)
- [ ] Research paper: formal verification of governance properties

## License

[MIT License](LICENSE)

---

<div align="center">

**Built by [Suresh Karicheti](https://gitlab.com/nexaiceo)**

</div>
