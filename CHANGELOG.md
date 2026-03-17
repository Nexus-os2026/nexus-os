# Changelog

All notable changes to NEXUS OS are documented here.

## v9.0.0 — Gen-3 Living OS (March 17, 2026)

12 new systems transforming Nexus OS from an agent framework into a living digital organism.

### New Systems
- **Agent DNA Genome** — genetic breeding, crossover, mutation, 47 genomes, evolve_population
- **Genesis Protocol** — agents create agents, gap detection, multi-generation, pattern memory
- **Consciousness Kernel** — agent internal states (confidence, fatigue, curiosity), empathic interface
- **Dream Forge** — overnight autonomous work, replay, experiment, consolidate, precompute, morning briefing
- **Temporal Engine** — parallel timeline forking, time-dilated sessions, checkpoint rollback
- **Immune System** — threat detection, antibody spawning, immune memory, adversarial arena, privacy scanner
- **Cognitive Filesystem** — semantic file understanding, knowledge graph, natural language queries
- **Agent Civilization** — parliament, economy, elections, dispute resolution, governance DAO
- **Sovereign Identity** — Ed25519 keys, ZK proofs, hardware-bound identity, agent passports
- **Distributed Mesh** — multi-machine consciousness, agent migration, shared knowledge
- **Self-Rewriting Kernel** — performance profiling, LLM-generated patches, sandboxed testing, auto-rollback
- **Computer Omniscience** — screen understanding, intent prediction, proactive assistance

### Visual Command Center
- Mission Control dashboard
- DNA Lab (breeding interface)
- Consciousness Monitor
- Dream Forge viewer
- Temporal Engine timeline viewer
- Immune Dashboard
- Knowledge Graph explorer
- Civilization governance panel
- Identity & Mesh management
- Self-Rewrite Lab

### Improvements
- Chat pipeline: end-to-end streaming with Ollama + NVIDIA NIM (Kimi K2 Instruct)
- 47/47 agents verified via smoke test
- Agent self-improvement proven (35/40 → 39/40)
- Output firewall false positives fixed
- Startup crash fixed (async agent loading)
- ENABLE_REAL_API guard removed
- Vite dev server fixed

### Stats
- 2,997+ Rust tests, 0 failures
- 14 Python end-to-end test scripts, 118/118 sub-tests passed
- 26 kernel modules
- 52 agents (47 prebuilt + 5 AI-generated)
- 47 agent genomes

## v8.1.0 — March 16, 2026

- Comprehensive audit fix — all pages wired, all systems verified, zero dead code
- 10 missing agents added (total 45 at that point)

## v8.0.0 — March 16, 2026

- Cognitive loop and HITL approval flow fully working
- Docker support, README refresh, 45 agents, production ready

## v7.0.0 - The Complete Operating System

When you open Nexus OS, you never need to leave it. 15 built-in governed applications, 33 desktop pages, every tool a developer needs — all running through the same capability-checked, fuel-metered, audit-logged governance kernel.

### Built-in Applications (Phase 7)

- **Code Editor** — Monaco editor with 50+ language support, file explorer, multi-tab editing, integrated governed terminal, Git integration (commit/push/pull/diff/branch), agent-assisted coding with 8 actions (Explain/Refactor/Fix/Test/Document/Optimize/Complete/Review), split view for agent suggestions, agent worker collaboration panel, cross-file search
- **Design Studio** — AI-powered canvas with drag-and-drop, Designer Agent generates layouts from natural language, 29 components across 5 categories, real-time React/HTML preview, export to code, 26 design tokens, version history with visual diffs
- **Terminal** — 30+ shell commands, 18 blocked patterns requiring Tier2+ HITL approval, warning system for risky commands, multi-pane with tab management, command history, smart agent suggestions, mock filesystem navigation, nexus CLI integration
- **File Manager** — Grid and list view, drag-and-drop operations, preview panel with syntax display, agent file operations with live progress, file permissions tied to agent capabilities, content search, encrypted vault for sensitive files, governed trash with recovery
- **Database Manager** — Connect to SQLite/PostgreSQL/MySQL, visual query builder with filters, governed agent read/write (DROP/TRUNCATE/DELETE blocked), query history with audit trail, data visualization charts (bar/pie/line), CSV/JSON export, schema viewer with ERD
- **API Client** — Request builder (7 HTTP methods), headers/body/auth configuration, JSON syntax highlighted response viewer, API collections, governed API key vault, rate limiting enforcement, all calls audit-logged
- **Notes App** — Rich markdown editor with live preview, folders and tags, agent auto-creates notes, research note dumps, link notes to agents/workflows/audit events, full-text search, export to PDF/markdown/docx, templates
- **Email Client** — IMAP/SMTP with Gmail/Outlook/custom, conversation threading, agent-drafted emails with HITL approval, email templates, smart categorization and priority, PII redaction before agent processing
- **Project Manager** — Kanban board with drag-and-drop, agent auto-creates tasks, sprint planning with agent-estimated complexity, time tracking with fuel correlation, burndown charts and velocity metrics, workflow automation triggers
- **Media Studio** — Image editor (crop/resize/9 filters/rotation/annotations), AI image generation with 8 styles, media library with 6 folders, OCR text extraction, before/after comparison slider, multi-format export (PNG/JPEG/WebP/SVG/PDF/AVIF)
- **System Monitor** — Real-time CPU/RAM/GPU/disk/network graphs, per-agent resource breakdown, process list with agent attribution, network traffic monitoring, fuel consumption over time, alert system for excessive resource use
- **App Store** — Featured agents with screenshots and reviews, one-click install with Ed25519 signature verification, user ratings and reviews, developer portal for publishing, automatic governed updates, dependency management
- **AI Chat Hub** — 9 models (Claude Opus/Sonnet/Haiku 4.5, GPT-4o/Mini, Gemini Pro/Flash, Llama 70B, Qwen 72B), side-by-side model comparison, agents join conversations, voice chat with Jarvis mode, image generation, code blocks with Run button, chat history search
- **Deploy Pipeline** — One-click build and deploy to Vercel/Netlify/Cloudflare/self-hosted, environment management (dev/staging/prod), one-click rollback, deploy logs with agent commentary, SSL certificate management, domain management, HITL required for production deploys
- **Learning Center** — 7 interactive courses with lesson tracking, 6 Rust code challenges with editor and hints, XP leveling system (Apprentice to Master), Self-Improve Agent shares learnings with confidence scores, community knowledge base, agent-generated video tutorials

### Governance Runs Through Everything

Every single app enforces the same governance model:
- Capability checks before every action
- Fuel metering with visual budget bars
- Audit trail logging for every operation
- HITL approval for dangerous actions
- Agent attribution on all automated activity
- PII redaction at data boundaries

### Stats

- 1,175 tests passing
- 33 workspace crates
- 33 desktop pages
- 15 built-in applications
- Zero unsafe Rust

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
