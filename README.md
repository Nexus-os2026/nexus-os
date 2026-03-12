[![CI](https://gitlab.com/nexaiceo/nexus-os/badges/main/pipeline.svg)](https://gitlab.com/nexaiceo/nexus-os/-/pipelines)
[![Version](https://img.shields.io/badge/version-7.0.0-blue.svg)](CHANGELOG.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-1941_passing-brightgreen.svg)](#)
[![Rust](https://img.shields.io/badge/rust-1.94+-orange.svg)](#)

# Nexus OS — Governed Deterministic Agent OS

> The world's first operating system where AI agents run with full governance, security, and transparency.

Nexus OS is a 130k+ line Rust operating system for AI agents where every action passes through capability checks, fuel metering, and cryptographic audit trails. Agents don't get trust by default — they earn it through track records, and lose it through violations.

## What Makes Nexus OS Different

- **Governed by Design** — Every agent action is audited, capability-checked, and fuel-metered. No action executes without governance approval.
- **Local-First AI** — Run LLMs locally via Ollama — your data never leaves your device. No cloud API keys required.
- **15 Built-In Apps** — Code Editor, Terminal, File Manager, AI Chat, Database Manager, API Client, and more.
- **Deterministic Execution** — WASM sandboxing ensures reproducible agent behavior across runs.
- **Enterprise-Ready Security** — EU AI Act compliance, PII redaction at LLM boundaries, SOC 2 Type II reporting, supply chain signing.
- **Zero Unsafe Rust** — `unsafe_code = "forbid"` enforced across all 33 workspace crates.

## Quick Start

### Prerequisites

- Rust 1.94+ (`rustup install stable`)
- Node.js 22+ (for the desktop frontend)
- Platform libraries for Tauri (see [Setup Guide](docs/SETUP.md))
- Optional: [Ollama](https://ollama.ai) (for local LLM inference)

### Build & Run

```bash
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os
cargo build --workspace
cd app && npm install && npm run build
cargo tauri dev
```

### First Steps

1. Launch Nexus OS — the Setup Wizard starts automatically
2. Connect Ollama — the wizard detects your hardware and helps install Ollama
3. Download a Model — pick a recommended model for your GPU/RAM
4. Assign Agents — configure which model powers each built-in agent
5. Start Building — open the Code Editor, Chat Hub, or Terminal

## Architecture

```
+---------------------------------------------------------------+
|                   Desktop UI (Tauri + React)                   |
|    15 Built-In Apps | Command Center | Setup Wizard            |
+---------------------------------------------------------------+
|                        CLI Layer                               |
|  nexus agent | nexus audit | nexus cluster | nexus marketplace |
+---------------------------------------------------------------+
|              Enterprise | Marketplace | Distributed            |
|   RBAC | SOC 2 | Signed Bundles | Quorum | TCP Replication     |
+---------------------------------------------------------------+
|                       SDK Layer                                |
|   NexusAgent Trait | AgentContext | ManifestBuilder | Sandbox  |
+---------------------------------------------------------------+
|                        Kernel                                  |
|  Supervisor | Autonomy (L0-L5) | Audit (hash-chained)         |
|  Fuel Metering | Delegation | HITL Consent | PII Redaction     |
+---------------------------------------------------------------+
```

- **33 workspace crates** — kernel, SDK, 9 agents, 5 connectors, distributed, enterprise, marketplace, CLI, desktop app
- **1,941 tests** — unit, integration, and property tests across all crates
- **WASM sandboxing** — wasmtime-based sandbox with host function governance bridge
- **Local LLM** — Candle-based inference + Ollama integration for GGUF models

## Features

| # | Category | Feature | Status |
|---|----------|---------|--------|
| 1 | Governance | Capability-based access control | Done |
| 2 | Governance | 6-level autonomy system (L0-L5) | Done |
| 3 | Governance | Fuel metering with anomaly detection | Done |
| 4 | Governance | Human-in-the-loop approval gates | Done |
| 5 | Governance | Adaptive trust scoring + auto-promotion/demotion | Done |
| 6 | Governance | Transitive delegation with cascade revocation | Done |
| 7 | Audit | Hash-chained append-only audit trail | Done |
| 8 | Audit | Deterministic replay with evidence bundles | Done |
| 9 | Audit | PII redaction at LLM gateway boundary | Done |
| 10 | Distributed | TCP transport with length-prefix framing | Done |
| 11 | Distributed | Audit replication across nodes | Done |
| 12 | Distributed | Quorum-based governance voting | Done |
| 13 | Distributed | SWIM-style membership protocol | Done |
| 14 | Enterprise | Role-based access control (RBAC) | Done |
| 15 | Enterprise | SOC 2 Type II compliance reporting | Done |
| 16 | Marketplace | Signed agent bundles with trust scoring | Done |
| 17 | Safety | Kill gates, zero unsafe Rust | Done |
| 18 | App | Code Editor — Monaco, 50+ languages, agent coding | Done |
| 19 | App | Design Studio — AI canvas, components, tokens | Done |
| 20 | App | Terminal — 30+ commands, governed execution | Done |
| 21 | App | File Manager — grid/list, drag-drop, vault | Done |
| 22 | App | Database Manager — SQL editor, visual builder, ERD | Done |
| 23 | App | API Client — request builder, governed vault | Done |
| 24 | App | Notes — rich markdown, templates, agent notes | Done |
| 25 | App | Email Client — IMAP/SMTP, threading, PII redaction | Done |
| 26 | App | Project Manager — kanban, sprints, burndown | Done |
| 27 | App | Media Studio — editor, AI generation, OCR | Done |
| 28 | App | System Monitor — CPU/RAM/GPU, per-agent stats | Done |
| 29 | App | App Store — Ed25519 verification, reviews | Done |
| 30 | App | AI Chat Hub — 9 models, comparison, voice mode | Done |
| 31 | App | Deploy Pipeline — 4 providers, environments, SSL | Done |

## SDKs

| SDK | Path | Language |
|-----|------|----------|
| Rust SDK | `sdk/` | Rust — `NexusAgent` trait, `AgentContext`, `ManifestBuilder` |
| TypeScript SDK | `sdk/typescript/` | TypeScript — browser/Node.js client |
| Python SDK | `sdk/python/` | Python — agent development and scripting |

## API Compatibility

Nexus OS exposes OpenAI-compatible and Anthropic-compatible API endpoints for local LLM inference:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | OpenAI-compatible chat (streaming supported) |
| `/v1/embeddings` | POST | OpenAI-compatible text embeddings |
| `/v1/models` | GET | List available models |
| `/v1/messages` | POST | Anthropic-compatible messages (streaming supported) |

## CLI

```bash
nexus agent list|start|stop|status      # Agent management
nexus audit show|verify|export          # Audit verification
nexus cluster status|join|leave         # Cluster operations
nexus marketplace search|install        # Agent marketplace
nexus compliance report|status          # Compliance reporting
nexus delegation grant|revoke|list      # Capability delegation
```

All commands support `--json` for structured output.

## Documentation

| Document | Description |
|----------|-------------|
| [Setup Guide](docs/SETUP.md) | Platform-specific installation and Ollama setup |
| [Architecture Guide](docs/ARCHITECTURE.md) | Layered design, module reference, governance pipeline |
| [SDK Tutorial](docs/SDK_TUTORIAL.md) | Build your first governed agent step-by-step |
| [Developer Guide](docs/DEVELOPER_GUIDE.md) | Contributing and development workflow |
| [API Reference](docs/API_REFERENCE.md) | Complete public type and function reference |
| [User Guide](docs/USER_GUIDE.md) | End-user guide for the desktop application |
| [Security Hardening](docs/SECURITY_HARDENING.md) | Production hardening checklist |
| [Deployment Guide](docs/DEPLOYMENT.md) | Single node and cluster setup |
| [Threat Model](docs/THREAT_MODEL.md) | Attack surfaces and mitigations |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and PR process.

## Security Invariants

These are enforced at the code level and must never be violated:

1. Every agent action goes through kernel capability checks
2. Fuel budget checked **before** execution, not after
3. Audit trail is append-only with hash-chain integrity
4. PII redaction at LLM gateway boundary
5. HITL approval mandatory for Tier 1+ operations
6. `unsafe_code = "forbid"` — zero unsafe Rust
7. All tests must pass before merging
8. Agents declare capabilities in TOML manifests

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).

## Built By

**Suresh Karicheti** (Creator & Builder) + **Claude** (Lead Architect)

> Don't trust. Verify.
