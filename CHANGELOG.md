# Changelog

All notable changes to NEXUS OS are documented here.

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
