# Changelog

All notable changes to NEXUS OS are documented here.

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
