# Changelog

All notable changes to NEXUS OS are documented here.

## v5.0.0 - Production Ready

### Sandbox and Networking
- WASM-ready agent sandbox with memory limits, time limits, and governed host functions
- TCP transport for distributed nodes with length-framed messages, exponential backoff retry, and connection management

### CLI Completeness
- 24 CLI commands across 8 subsystems: agent, audit, cluster, marketplace, compliance, delegation, benchmark, finetune
- Structured JSON output mode on every command

### Desktop UI Overhaul
- Command Center with live agent grid, status indicators, fuel bars, and Start/Stop/Kill controls
- Audit Timeline with color-coded events, filtering, and federation cross-reference badges
- Marketplace Browser with search, verified badges, and install flow
- Compliance Dashboard with SOC2 control status cards and report generation
- Cluster Status with node health, heartbeat tracking, and quorum indicators
- Trust Dashboard with per-agent trust scores, autonomy levels, and promotion/demotion badges

### Documentation
- Architecture guide with layered system diagram
- SDK tutorial for building governed agents from scratch
- Deployment guide for single-node and cluster setups
- Security hardening checklist with 9 sections and verification commands
- Complete API reference for all public types across every crate

### End-to-End Integration Tests
- Full governance pipeline: manifest to capability check to fuel to audit to evidence bundle verification
- Circuit breaker failover with provider routing
- Marketplace publish, install, and tamper detection
- RBAC permission boundaries with SOC2 compliance report
- Adaptive governance promotion and demotion lifecycle
- Capability delegation with fuel limits and governed channel collaboration
- Cloud tenant lifecycle with API keys and usage metering

## v4.0.0 - Governed Distributed Agent Platform

### Phase 1: Hardening and Evidence
- Criterion benchmark suite across kernel, gateway, agent, and replay modules
- Replay evidence bundles with standalone 5-check verifier (hash-chain integrity, manifest match, fuel bounds, approval coverage, monotonic ordering)
- Circuit breaker state machine (Closed/Open/HalfOpen) for LLM provider fault tolerance
- Provider routing engine with 4 strategies: Priority, RoundRobin, LowestLatency, CostOptimized

### Phase 2: Distributed Governance
- Cross-node replication with heartbeat failure detection and delta/full-sync modes
- Quorum-backed execution engine with propose/vote/timeout lifecycle for multi-party consensus
- Federated audit chains with cross-node hash references and tamper detection
- Agent marketplace with Ed25519 manifest signature verification and provenance checking

### Phase 3: Ecosystem and Enterprise
- Plugin SDK: NexusAgent trait, governed AgentContext, ManifestBuilder, TestHarness for third-party agent development
- Enterprise RBAC with 6 roles (Owner/Admin/Operator/Developer/Viewer/Auditor) and glob-matched permissions
- SOC2 Type II compliance report generator mapping 5 controls (CC6.1-CC7.2) to Nexus OS audit primitives
- Cloud multi-tenancy with plan-based resource limits, SHA-256 hashed API keys, and usage metering

### Phase 4: Agent Intelligence
- Multi-agent collaboration: governed channels with rate limiting, orchestrator with capability-based task assignment, ACL-gated blackboard
- Capability delegation with transitive trust chains, cascade revocation, and fuel-bounded grants
- Adaptive governance with trust score computation, human-gated promotions, and automatic demotions
- Governed fine-tuning with safety check pipeline (PII/Harm/Accuracy/Alignment) and full audit trail

90 test suites. Zero failures. All features governed, audited, capability-gated.

## v3.0.0 - Human-Centric & Hardware-Hardened

- Autonomy Levels (L0-L5) with hard-gating at kernel chokepoints
- Human-in-the-Loop tiering (Tier0-Tier3) with two-person rule
- PII Redaction engine at LLM gateway boundary
- Hardware-backed security abstraction (TPM/Enclave/TEE stubs)
- Economic fuel model with monthly budgets + anomaly detection
- Safety supervisor with 3-strike halt + incident reports
- Kill gates with per-subsystem freeze/halt/escalation
- Distributed interface scaffolding (traits + local defaults)

## v2.0.0 - Autonomous Digital Worker Platform

### Agent Platform Expansion
- Added `coder-agent` with project scanning, architecture analysis, context building, style-aware code writing, test execution, and iterative fix loops.
- Added `screen-poster-agent` with platform navigation maps, draft composition, approval-gated posting, comment interaction, engagement tracking, and stealth timing controls.
- Added `web-builder-agent` for natural-language website generation with reusable templates, Tailwind theme synthesis, and React Three Fiber scene generation.
- Added `workflow-studio-agent` with typed node system, DAG execution engine, checkpointing, retry/skip/halt strategies, parallel node execution, and workflow serialization/versioning.
- Added `self-improve-agent` with performance tracking, strategy learning, prompt optimization, encrypted knowledge storage, skill ratings, and rollback-capable improvement loop.
- Added `designer-agent` for design spec generation, component library generation, screenshot analysis, and design token output.

### Desktop and UX
- Shipped major desktop UI upgrade with neural background effects, holographic panels, richer data visualizations, voice orb, agent avatar system, and fast page transitions.
- Expanded command center navigation to include workflow studio and deeper agent visibility.
- Updated About, splash, and runtime version surfaces for `v2.0.0`.

### Quality and Integration
- Added cross-agent integration coverage in `tests/integration/tests/e2e_system_workflows.rs`:
  - Coding agent end-to-end workflow
  - Screen poster draft approval workflow
  - Website builder generation workflow
  - Workflow studio DAG execution workflow
  - Self-improvement learning workflow
- Re-verified workspace-wide formatting, linting, Rust tests, voice Python tests, and app build/test checks.

## v1.0.0 - Production Readiness (Phase 9)

### Documentation and Productization
- Complete user and developer documentation set.
- Expanded threat model with concrete attack surfaces and trust boundaries.
- Updated installation/quick-start guidance for Linux, macOS, and Windows.
- Added first end-to-end production agent (`social-poster`) with dry-run mode.

### Governance and Runtime
- Hardened governed execution model with capability checks and fuel controls.
- Hash-chained audit events integrated through runtime workflows.
- Stabilized cross-platform CI test reliability and deterministic behavior.

## v0.9.0 - CI/CD and Security Automation (Phase 8)

### CI/CD
- Multi-platform CI workflow for Linux, macOS, and Windows.
- Python voice test workflow integration.
- Release workflow for tagged builds with platform artifacts.

### Security Automation
- Scheduled and push-triggered security audit workflow.
- `cargo audit` and `cargo deny` integration for dependency and license review.

## v0.8.0 - Desktop Experience (Phase 7)

### Tauri Desktop App
- Functional desktop control plane with Chat, Agents, Audit, and Settings pages.
- Real backend command wiring for agent lifecycle, audit access, config, and chat.
- System tray actions for dashboard access and voice mode controls.

## v0.7.0 - Local Voice Runtime (Phase 6)

### Voice Pipeline
- Local-first voice stack (wake word, VAD, STT, TTS) integration path.
- CLI voice commands for start/test/model inspection.
- Runtime hooks for push-to-talk and Jarvis mode.

## v0.6.0 - Messaging and Remote Control (Phase 5)

### Telegram Bridge
- Real Telegram integration path for status/start/stop/approval/log commands.
- Device pairing and authorization model for remote control.
- Messaging bridge routing and command handling tests.

## v0.5.0 - Web and LLM Real Integrations (Phase 4)

### Web Intelligence
- Brave Search request pipeline with rate limiting.
- Web reader extraction with robots and timeout controls.
- X API integration surface with OAuth and rate-limit handling.

### LLM Gateway
- Multi-provider support with safe defaults and explicit real-API opt-in.
- Provider selection strategy (mock/local/cloud ordering).
- Cost, token, latency telemetry in governed gateway path.

## v0.4.0 - Configuration and Setup (Phase 3)

### Configuration
- Encrypted global config file support at `~/.nexus/config.toml`.
- Setup wizard and validation checks for external integrations.
- Config status checks for operator visibility.

## v0.3.0 - Workflow, Research, and Content Layers (Phase 2)

### Workflows and Research
- Sequential and resumable workflows with checkpoint support.
- Research pipeline for citation extraction and strategy synthesis.

### Content and Analytics
- Content generation and compliance modules.
- Metrics collection, evaluation, and reporting flows.
- Adaptation engine with authority-bound update controls.

## v0.2.0 - Connectors and Control Foundations (Phase 1)

### Connector Foundation
- Core connector framework with vault, idempotency, and rate limiting.
- LLM, web, social, and messaging connector scaffolding.

### Control Foundation
- Screen capture/control, browser automation, and vision loop primitives.

## v0.1.0 - Kernel Foundation (Phase 0)

### Initial Runtime
- Kernel runtime, supervisor, lifecycle, and manifest parsing.
- Capability and fuel governance primitives.
- Audit and privacy building blocks.
- Initial CLI and crate workspace baseline.
