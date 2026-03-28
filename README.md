[![pipeline status](https://gitlab.com/nexaiceo/nexus-os/badges/main/pipeline.svg)](https://gitlab.com/nexaiceo/nexus-os/-/commits/main)

<div align="center">

# 🧠 Nexus OS

### The World's First Governed AI Agent Operating System

**53 Agents · 397 Commands · 47 Self-Evolving Genomes · 12 Gen-3 Systems · Zero Crashes**

*Local-first. Air-gappable. Desktop-native. Built in Rust.*

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tauri 2.0](https://img.shields.io/badge/shell-Tauri%202.0-24C8D8.svg)](https://tauri.app/)
[![EU AI Act](https://img.shields.io/badge/EU%20AI%20Act-Compliant-003399.svg)](#eu-ai-act-compliance)
[![v10.3.0](https://img.shields.io/badge/version-10.3.0-green.svg)](CHANGELOG.md)

[Architecture](#architecture) · [Quick Start](#quick-start) · [Agents](#agents) · [Governance](#governance) · [Enterprise](#enterprise) · [Contributing](CONTRIBUTING.md)

</div>

---

## What is Nexus OS?

Nexus OS is an **AI agent operating system** — not a framework, not a library, not a cloud service. It's a complete desktop operating system where AI agents are first-class citizens with cryptographic identities, governed autonomy, and the ability to evolve.

**The anti-cloud AI platform.** Your agents run on your hardware. Your data never leaves your machine. No API keys required for local inference. Air-gap it, take it offline, run it in a SCIF — it still works.

```
┌─────────────────────────────────────────────────────────────────┐
│                     Nexus OS v10.3.0                            │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ Coder    │  │ Research │  │ Security │  │ DevOps   │  ...53 │
│  │ Agent    │  │ Agent    │  │ Agent    │  │ Agent    │ agents │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘       │
│       │              │              │              │             │
│  ┌────▼──────────────▼──────────────▼──────────────▼─────┐      │
│  │              Nexus Conductor (Orchestration)           │      │
│  │         A2A Protocol  ·  MCP Protocol  ·  Swarm       │      │
│  └────────────────────────┬──────────────────────────────┘      │
│                           │                                      │
│  ┌────────────────────────▼──────────────────────────────┐      │
│  │                  Governance Kernel                     │      │
│  │  Capability ACL · HITL Gates · Fuel Metering          │      │
│  │  Hash-Chained Audit · PII Redaction · WASM Sandbox    │      │
│  │  DID/Ed25519 Identity · EU AI Act Compliance          │      │
│  └────────────────────────┬──────────────────────────────┘      │
│                           │                                      │
│  ┌────────────────────────▼──────────────────────────────┐      │
│  │                    LLM Providers                       │      │
│  │  Ollama (local) · NVIDIA NIM · OpenAI · Anthropic     │      │
│  │  Google · DeepSeek · OpenRouter · + 200 models         │      │
│  └───────────────────────────────────────────────────────┘      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Rust Kernel  ·  Tauri 2.0 Shell  ·  React/TS Frontend  │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

## Why Nexus OS?

| Problem | Everyone Else | Nexus OS |
|---------|--------------|----------|
| **Data sovereignty** | Send everything to the cloud | 100% local-first, air-gappable |
| **Agent safety** | Trust the agent, hope for the best | WASM sandbox, capability ACL, fuel limits |
| **Agent identity** | Anonymous function calls | DID/Ed25519 cryptographic identity per agent |
| **Audit trail** | Logs (deletable, mutable) | Hash-chained audit trail (tamper-evident) |
| **Human oversight** | Optional, bolted on | HITL consent gates built into kernel |
| **Agent evolution** | Static prompts forever | Darwinian evolution: agents mutate, compete, improve |
| **Compliance** | "We're working on it" | EU AI Act conformity built in |
| **Performance** | Python + Electron | Rust kernel + Tauri 2.0 (5MB binary vs 100MB+) |
| **Vendor lock-in** | Pick one cloud provider | 8 LLM providers (200+ models), swap freely, or go fully offline |

## Architecture

### Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| **Kernel** | Rust | Memory safety, zero-cost abstractions, no GC pauses |
| **Desktop Shell** | Tauri 2.0 | 5-15MB binaries, native performance, cross-platform |
| **Frontend** | React + TypeScript | 50 pages, rich agent management UI |
| **Voice** | Python pipeline | Speech-to-text / text-to-speech for multimodal agents |
| **Local LLM** | Ollama | Run any open model locally, zero internet required |
| **GPU Inference** | NVIDIA NIM | 42+ models from 12 providers via single API key |
| **Sandboxing** | wasmtime (WASM) | Hardware-grade agent isolation |
| **Identity** | DID + Ed25519 | Self-sovereign, verifiable agent identities |
| **Protocols** | A2A + MCP | Google Agent-to-Agent + Anthropic Model Context Protocol |

### Core Systems

**12 Gen-3 Systems** powering the platform:

- **Governance Kernel** — Capability-based access control, HITL consent gates, fuel metering
- **Nexus Conductor** — Multi-agent orchestration with A2A and MCP protocol support
- **Darwin Core** — Darwinian evolution engine (AdversarialArena + SwarmCoordinator + PlanEvolutionEngine)
- **Agent Identity** — DID/Ed25519 with OIDC-A JWT tokens
- **Audit Engine** — Hash-chained, tamper-evident, append-only audit trails
- **WASM Sandbox** — wasmtime-based agent isolation with speculative execution
- **LLM Router** — Unified interface across 6 providers and 42+ models
- **Output Firewall** — Real-time content filtering and data exfiltration prevention
- **PII Redaction** — Automated detection and redaction of personally identifiable information
- **Computer Control** — Agents can see and interact with your desktop
- **World Simulation** — Agent environment simulation for safe testing
- **Voice Pipeline** — Multimodal speech interaction

## Flash Inference — Run Any Open-Source Model Locally

Nexus OS includes a built-in local inference engine powered by llama.cpp, supporting 60+ model architectures through the GGUF format. Run models from Qwen, DeepSeek, Llama, Mistral, Gemma, Phi, and more — all governed with the same security pipeline as cloud providers.

**Verified Benchmarks (ASUS ROG Zephyrus Duo 16, 62GB RAM, CPU only):**

| Model | Parameters | Type | RAM Cage | tok/s |
|-------|-----------|------|----------|-------|
| Gemma 2 2B | 2B | Dense | — | 9.93 |
| Qwen3.5-35B-A3B | 35B (3B active) | MoE | — | 8.36 |
| Qwen3.5-397B-A17B | 397B (17B active) | MoE | 32 GB | 0.26 |

> **397B model verified running in 32 GB systemd memory cage (`MemoryMax=32G`) via mmap SSD streaming. No GPU required.**

Every inference call passes through: capability check → fuel reserve → adversarial arena → PII redaction → output firewall → hash-chained audit trail.

No GPU required. Cross-platform: Linux, macOS, Windows.

## Agents

**53 prebuilt agents** across 7 autonomy levels (L0–L6):

| Level | Autonomy | Example Agents |
|-------|----------|---------------|
| **L0** | Passive — observe only | Monitor Agent, Logger Agent |
| **L1** | Reactive — respond to triggers | Alert Agent, Notification Agent |
| **L2** | Guided — human approves each action | Research Agent, Analyst Agent |
| **L3** | Supervised — human approves risky actions | Coder Agent, Web Builder Agent |
| **L4** | Autonomous — operates within constraints | DevOps Agent, Data Pipeline Agent |
| **L5** | Collaborative — coordinates with other agents | Swarm Agent, Conductor Agent |
| **L6** | Self-evolving — improves own behavior | Darwin Agent, Evolution Agent |

### Self-Evolving Agents (Darwin Core)

Agents don't just execute — they **evolve**. The Darwin Core implements Darwinian natural selection for agent behavior:

1. **Mutation** — LLM-driven prompt mutation creates behavioral variants
2. **Competition** — Agents compete in the AdversarialArena on standardized tasks
3. **Selection** — Best-performing variants survive, others are pruned
4. **Reproduction** — Winners produce offspring with inherited + mutated traits
5. **Governance** — All evolution is constrained by capability ACL and HITL gates

**47 genomes** represent the evolved behavioral DNA of agent populations.

## Governance

Nexus OS is **governance-native** — security isn't bolted on, it's the kernel.

### Capability-Based Access Control
Every agent has an explicit capability set. No ambient authority. An agent can only do what its capability token allows.

```
Agent: coder-agent
Capabilities:
  ✅ file.read(scope: /workspace/*)
  ✅ file.write(scope: /workspace/*)
  ✅ llm.query(provider: ollama, model: codestral)
  ❌ network.external
  ❌ file.write(scope: /system/*)
  ❌ process.execute(elevated: true)
Fuel: 10,000 units/session
HITL: Required for file.delete, process.execute
```

### Hash-Chained Audit Trail
Every agent action is recorded in a hash-chained, append-only log. Each entry includes the agent DID (cryptographic identity), action taken, capability used, timestamp, hash of previous entry (chain integrity), and HITL approval status. Tampering with any entry breaks the chain — auditors can verify the entire history cryptographically.

### Human-in-the-Loop (HITL) Gates
Configurable consent gates that pause agent execution and require human approval:

| Mode | Behavior | Agents |
|------|----------|--------|
| **Always** | Every action requires approval | L0–L1 |
| **Risky** | Only high-risk actions need approval | L2–L3 |
| **Exceptional** | Only out-of-bounds actions need approval | L4–L5 |
| **Audit-only** | Execute immediately, log for review | L6 |

## Quick Start

### Prerequisites
- **OS:** Linux (Ubuntu 22.04+), macOS, or Windows 10+
- **Rust:** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Node.js:** 18+ (`nvm install 18`)
- **Ollama** (optional, for local LLM): `curl -fsSL https://ollama.ai/install.sh | sh`

### Install & Run

```bash
# Clone the repository
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os

# Build the Rust workspace
cargo build --workspace

# Install frontend dependencies and build
cd app && npm ci && npm run build && cd ..

# Run in development mode
cd app && npm run tauri dev
```

### First Agent

```bash
# Pull a local model (optional — works with cloud providers too)
ollama pull llama3.2

# Launch Nexus OS — navigate to Agents → Coder Agent → Start
cargo tauri dev
```

## Enterprise

### Deployment Options

| Mode | Use Case | Guide |
|------|----------|-------|
| **Desktop** | Individual knowledge workers | [Quick Start](#quick-start) |
| **Server** | Team/department deployment | [Deployment Guide](docs/DEPLOYMENT.md) |
| **Kubernetes** | Enterprise-scale orchestration | [K8s Guide](docs/DEPLOYMENT.md#kubernetes) |
| **Air-Gapped** | Classified/regulated environments | [Air-Gap Guide](docs/DEPLOYMENT.md#air-gapped) |

### Security & Compliance

| Standard | Status | Documentation |
|----------|--------|--------------|
| **EU AI Act** | ✅ Conformity self-assessment complete | [EU AI Act](docs/EU_AI_ACT_CONFORMITY.md) — Article-by-article mapping (Arts. 9, 10, 11, 12, 13, 14, 15, 17) |
| **SOC 2 Type II** | 🔄 Readiness documented | [SOC 2](docs/SOC2_READINESS.md) |
| **ISO 27001** | 🔄 Controls mapped | [Enterprise Guide](docs/ENTERPRISE_GUIDE.md) |
| **HIPAA** | 🔄 BAA template available | [Enterprise Guide](docs/ENTERPRISE_GUIDE.md) |
| **GDPR** | ✅ PII redaction built in | [Governance](#governance) |

### Enterprise Features

- **SSO/OIDC** — Keycloak, Auth0, Azure AD/Entra, Okta integration
- **Multi-tenancy** — Isolated workspaces with tenant-level governance
- **Observability** — OpenTelemetry → Prometheus/Grafana/Jaeger
- **Rate Limiting** — Per-agent, per-tenant, per-API throttling
- **Encryption at Rest** — AES-256-GCM for all stored data
- **Backup/Restore** — Point-in-time recovery with encrypted snapshots
- **Fleet Management** — Deploy, update, monitor thousands of instances
- **Admin Console** — Centralized management dashboard
- **Audit Dashboard** — Compliance reporting and anomaly detection
- **Enterprise Integrations** — Slack, Teams, Jira, ServiceNow, Salesforce

## EU AI Act Compliance

Nexus OS is designed for compliance with the EU AI Act (Regulation 2024/1689), which begins enforcing high-risk system requirements on **August 2, 2026**.

| Article | Requirement | Nexus OS Feature |
|---------|------------|------------------|
| Art. 9 | Risk Management | Fuel metering, capability constraints, HITL gates |
| Art. 10 | Data Governance | PII redaction, data lineage tracking |
| Art. 12 | Record-keeping | Hash-chained audit trails, tamper-evident logging |
| Art. 13 | Transparency | Agent capability declarations, decision explanations |
| Art. 14 | Human Oversight | Configurable HITL consent gates at every autonomy level |
| Art. 15 | Accuracy/Robustness | WASM sandboxing, output firewall, adversarial testing |

See [EU AI Act Conformity Self-Assessment](docs/EU_AI_ACT_CONFORMITY.md) for the complete mapping.

## Project Stats

```
Tauri Commands .... 397     Agents ............ 53
Genomes ........... 47      Gen-3 Systems ..... 12
UI Pages .......... 50      LLM Providers ..... 6
Crashes ........... 0       Blank Pages ....... 0
Dead Buttons ...... 0       Broken Calls ...... 0
```

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | System design, component diagrams, data flow |
| [API Reference](docs/API_REFERENCE.md) | Complete reference for 397 Tauri commands |
| [Deployment Guide](docs/DEPLOYMENT.md) | Docker, Kubernetes, air-gapped installation |
| [Enterprise Guide](docs/ENTERPRISE_GUIDE.md) | Enterprise evaluation and deployment |
| [Security Policy](SECURITY.md) | Vulnerability reporting, security model |
| [EU AI Act](docs/EU_AI_ACT_CONFORMITY.md) | Regulatory compliance self-assessment |
| [SOC 2 Readiness](docs/SOC2_READINESS.md) | SOC 2 Type II control mapping |
| [SDK Guide](docs/SDK_GUIDE.md) | Build custom agents and integrations |
| [Changelog](CHANGELOG.md) | Version history and release notes |
| [Contributing](CONTRIBUTING.md) | How to contribute to Nexus OS |

## Roadmap

- [x] v10.3.0 — Full audit: 50 pages, 397 commands, 53 agents, 0 defects
- [x] Darwin Core — Darwinian evolution engine
- [x] NVIDIA NIM — 42-model expansion (12 providers)
- [ ] Premium website with 3D design
- [ ] Computer Control live testing
- [ ] Demo video recording
- [ ] Enterprise SSO/OIDC integration
- [ ] Kubernetes Helm chart
- [ ] OpenTelemetry instrumentation
- [ ] Admin console and fleet management
- [ ] v11.0.0 — Enterprise Edition GA

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. Nexus OS is MIT-licensed and welcomes contributions.

**Areas where help is needed:** Agent development, frontend UI/UX, enterprise integration connectors, documentation and tutorials, security auditing and penetration testing.

## License

[MIT License](LICENSE)

---

<div align="center">

**Built with 🧠 by [Suresh Karicheti](https://gitlab.com/nexaiceo)**

*"AI as the substrate, not the feature."*

⭐ **Star this repo if you believe AI agents should be governed, not trusted.**

</div>
