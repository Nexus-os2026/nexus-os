# Nexus OS v7.0.0 — Codebase Reconnaissance Report

> Generated: 2026-03-14 | Read-only analysis — no code modified

---

## SECTION 1: PROJECT STRUCTURE

### Top-Level Directories

| Directory | Description |
|-----------|-------------|
| `adaptation/` | Self-improvement layer for governed strategy adaptation |
| `agents/` | 10 agent crates (coder, designer, coding-agent, screen-poster, self-improve, social-poster, web-builder, workflow-studio, collaboration, conductor) |
| `analytics/` | Engagement analytics and closed-loop feedback |
| `app/` | React/Tauri frontend (`app/src/`) + Tauri desktop backend (`app/src-tauri/`) |
| `benchmarks/` | Performance benchmarking suite |
| `cli/` | Command-line interface (clap-based) |
| `cloud/` | Cloud deployment scaffolding |
| `config/` | Configuration schemas |
| `connectors/` | 5 external integration connectors (core, web, social, messaging, llm) |
| `content/` | Content generation & scheduling layer |
| `control/` | Computer control automation (vision, input, capture) |
| `distributed/` | Distributed governance layer (audit chain, consensus, replication) |
| `docs/` | Documentation |
| `enterprise/` | Enterprise RBAC & compliance |
| `factory/` | Agent Factory (NL → agent scaffolding) |
| `kernel/` | Core runtime & governance engine (~50 modules) |
| `marketplace/` | Agent Marketplace (bundles, registry, trust, SQLite) |
| `packaging/` | Deployment packaging (airgap bundles) |
| `protocols/` | HTTP gateway, A2A, MCP, WebSocket (axum-based) |
| `research/` | Research pipeline for agent bootstrap |
| `sdk/` | Plugin SDK for agent development (wraps kernel) |
| `self-update/` | Secure self-update with TUF |
| `services/` | Microservices (voice server) |
| `tasks/` | Task tracking |
| `tests/` | Integration tests |
| `voice/` | Voice/audio processing (Python) |
| `website/` | Marketing website |
| `workflows/` | Workflow orchestration engine |

### Workspace Members (35 crates)

From root `Cargo.toml`:

```
agents/coder, agents/designer, agents/coding-agent, agents/screen-poster,
agents/self-improve, agents/social-poster, agents/web-builder,
agents/workflow-studio, agents/collaboration, agents/conductor,
kernel, connectors/core, connectors/web, connectors/social,
connectors/messaging, connectors/llm, workflows, cli, research,
content, analytics, adaptation, control, factory, marketplace,
self-update, tests/integration, app/src-tauri, benchmarks,
distributed, sdk, enterprise, cloud, protocols, packaging/airgap
```

### Crate Purposes (one line each)

| Crate | Purpose |
|-------|---------|
| `nexus-kernel` | Core runtime: supervisor, audit trail, fuel, autonomy, consent, permissions, firewall, HITL, speculative execution |
| `nexus-sdk` | Plugin SDK wrapping kernel; NexusAgent trait, AgentContext, Wasmtime sandbox, shadow sandbox |
| `nexus-cli` | CLI commands: create, test, package agents (clap-based) |
| `coder-agent` | Codebase scanning, code writing, test/fix loops, LLM codegen |
| `designer-agent` | UI design generation, component libraries, screenshot-to-code, design tokens |
| `coding-agent` | Governed coding agent runtime with capability checks and fuel budgets |
| `screen-poster-agent` | Vision-driven social posting with human approvals and stealth navigation |
| `self-improve-agent` | Self-improvement: outcome tracking, learning strategies, prompt optimization |
| `social-poster-agent` | Social content research, generation, review, and publishing pipeline |
| `web-builder-agent` | NL website generation with templates, 3D scenes, deploy helpers |
| `workflow-studio-agent` | Visual workflow runtime for DAG-based automation with AI nodes |
| `nexus-collaboration` | Multi-agent collaboration: channels, task orchestration, shared blackboard |
| `nexus-conductor` | Orchestrates multi-agent task execution from NL requests |
| `nexus-connectors-core` | Governed connector framework: rate limiting, idempotency, vault, validation |
| `nexus-connectors-web` | Web search, content extraction, Twitter/X integration |
| `nexus-connectors-social` | Facebook and Instagram publishing connectors |
| `nexus-connectors-messaging` | Discord, Slack, Telegram, WhatsApp bridge connectors |
| `nexus-connectors-llm` | LLM gateway: 6 providers, fuel enforcement, PII redaction, RAG, vector store |
| `nexus-workflows` | Workflow orchestration engine |
| `nexus-research` | Research pipeline for agent bootstrap |
| `nexus-content` | Content generation & scheduling |
| `nexus-analytics` | Engagement analytics |
| `nexus-adaptation` | Governed strategy adaptation |
| `nexus-control` | Computer control (screen capture, mouse/keyboard) |
| `nexus-factory` | Agent Factory (NL description → scaffolded agent) |
| `nexus-marketplace` | Agent marketplace with SQLite registry, ed25519 signing, trust scoring |
| `nexus-self-update` | Secure self-update with TUF framework |
| `nexus-distributed` | Distributed audit chain, consensus protocols, replication |
| `nexus-enterprise` | Enterprise RBAC & SSO |
| `nexus-cloud` | Cloud deployment scaffolding |
| `nexus-protocols` | HTTP gateway (axum), A2A, MCP server, WebSocket, JWT auth |
| `nexus-packaging-airgap` | Offline bundle creation and validation |
| `nexus-benchmarks` | Performance benchmarking |
| `nexus-integration-tests` | E2E integration tests |

### Frontend Structure (`app/src/`)

```
app/src/
├── api/
│   └── backend.ts          — 117 invokeDesktop() wrappers to Tauri commands
├── components/
│   ├── agents/
│   │   └── CreateAgent.tsx  — 4-step agent creation wizard
│   ├── Layout.tsx           — Main app layout with sidebar
│   └── ...                  — Shared UI components
├── pages/                   — 41 page components (20,621 total lines)
│   ├── CodeEditor.tsx (1020)    ├── ModelHub.tsx (1139)
│   ├── Terminal.tsx (773)       ├── Documents.tsx (1132)
│   ├── FileManager.tsx (554)    ├── LearningCenter.tsx (809)
│   ├── Settings.tsx (736)       ├── AppStore.tsx (795)
│   ├── DatabaseManager.tsx (673)├── TimeMachine.tsx (752)
│   ├── NotesApp.tsx (585)       ├── SetupWizard.tsx (768)
│   ├── ApiClient.tsx (573)      ├── VoiceAssistant.tsx (670)
│   ├── PermissionDashboard.tsx  ├── DeployPipeline.tsx (667)
│   ├── SystemMonitor.tsx (491)  ├── ComplianceDashboard.tsx (475)
│   ├── AiChatHub.tsx (697)      ├── EmailClient.tsx (463)
│   ├── ProjectManager.tsx (681) ├── AgentBrowser.tsx (414)
│   ├── MediaStudio.tsx (678)    ├── PolicyManagement.tsx (380)
│   ├── DesignStudio.tsx (610)   ├── Marketplace.tsx (328)
│   ├── Chat.tsx (327)           ├── DeveloperPortal.tsx (328)
│   ├── Protocols.tsx (305)      ├── Audit.tsx (295)
│   ├── Agents.tsx (281)         ├── Firewall.tsx (264)
│   ├── Workflows.tsx (248)      ├── MarketplaceBrowser.tsx (236)
│   ├── DistributedAudit.tsx     ├── CommandCenter.tsx (134)
│   ├── Identity.tsx (120)       ├── AuditTimeline.tsx (99)
│   ├── TrustDashboard.tsx (92)  ├── ClusterStatus.tsx (90)
│   └── Dashboard.tsx (59)
├── App.tsx                  — Root component, routing, state management
└── main.tsx                 — Entry point
```

---

## SECTION 2: RUST BACKEND REALITY CHECK

### Tauri Commands: 226 Total

**226 Tauri commands** found in `app/src-tauri/src/main.rs` (lines 6113–9276).

**Breakdown: ~220 REAL, ~6 MOCK/STUB**

All commands are thin wrappers calling `super::function_name()` which delegates to real kernel/connector functions. Notable categories:

| Category | Count | Status |
|----------|-------|--------|
| Agent Management (create, start, stop, pause, resume, list) | 6 | ALL REAL |
| Chat & LLM (send_chat, chat_with_ollama, streaming) | 10 | ALL REAL |
| Hardware/Ollama (detect, check, pull, ensure, delete) | 10 | ALL REAL |
| Permissions (get, update, bulk, history, capability requests) | 5 | ALL REAL |
| Audit & Compliance (audit log, chain status, compliance) | 8 | ALL REAL |
| Marketplace (search, install, info, publish, my_agents) | 5 | ALL REAL |
| RAG Pipeline (index, search, chat, governance, semantic map) | 9 | ALL REAL |
| File Manager (list, read, write, create_dir, delete, rename) | 7 | ALL REAL |
| Database Manager (connect, query, list_tables) | 3 | ALL REAL |
| Notes App (list, get, create, save, delete) | 5 | ALL REAL |
| Email Client (list, save, delete) | 3 | ALL REAL |
| Terminal (execute, execute_approved) | 2 | ALL REAL |
| MCP Host (list_servers, add, remove, connect, call_tool) | 7 | ALL REAL |
| Time Machine (checkpoints, undo, redo, diff) | 7 | ALL REAL |
| Conductor (conduct_build, execute_tool, list_tools) | 5 | ALL REAL |
| Voice (start/stop/status/transcribe, load whisper) | 8 | ALL REAL |
| System Monitor (specs, live metrics) | 2 | ALL REAL |
| Economy & Billing (wallets, contracts, performance) | 10 | ALL REAL |
| Reputation (register, record, rate, get, top) | 7 | ALL REAL |
| Protocols Dashboard (status, requests, tools, cards) | 4 | 3 REAL, 1 STUB |
| All others (ghost protocol, nexus link, evolution, etc.) | 103 | ALL REAL |

**Known Stubs:**
- `get_protocols_requests()` → returns empty `Vec::new()` (line 8166)
- `voice_transcribe()` → falls back to stub when no engine available
- Some status commands return cached/static data when gateway not started

### Kernel Public Functions: 669+

**ALL pure in-memory logic.** The kernel has zero external I/O, zero database queries, zero file writes. Everything is `HashMap`, `Vec`, and struct-based state:

| Module | Functions | Nature |
|--------|-----------|--------|
| Supervisor | 85+ | Agent lifecycle, fuel ledgers, consent queue — all HashMap/Vec |
| Autonomy | 15+ | Level enums, guard checks — pure logic |
| Speculative | 20+ | Shadow simulation engine — in-memory fork/simulate/commit |
| Audit | 20+ | Hash-chain append to `Vec<AuditEvent>`, SHA-256 integrity |
| Fuel Hardening | 40+ | Cost models, anomaly detection, ledgers — struct fields |
| Consent | 50+ | HITL approval queue — `HashMap<String, ApprovalRequest>` |
| Permissions | 30+ | Capability grants — `HashMap<AgentId, HashSet<String>>` |
| Protocols | 60+ | A2A tasks, MCP tool registry — in-memory state machines |
| Orchestration | 30+ | Team coordination, message bus — in-memory |
| Safety | 25+ | KPI monitoring, risk assessment — in-memory |
| Kill Gates | 16+ | Circuit breakers — in-memory |
| Firewall | 15+ | Allowlist/blocklist, regex patterns — in-memory |
| Other (20+ modules) | 300+ | Identity, reputation, delegation, compliance — all in-memory |

### LLM Provider Implementations: 6 Providers

| Provider | File | Real HTTP? | Gate |
|----------|------|-----------|------|
| **Ollama** | `connectors/llm/src/providers/ollama.rs` | YES — `reqwest::Client` to `localhost:11434` | Always available |
| **OpenAI** | `connectors/llm/src/providers/openai.rs` | YES — curl subprocess to `api.openai.com` | `ENABLE_REAL_API=1` + `OPENAI_API_KEY` |
| **Claude/Anthropic** | `connectors/llm/src/providers/claude.rs` | YES — `reqwest::blocking::Client` | `real-claude` feature flag + `ENABLE_REAL_API=1` + `ANTHROPIC_API_KEY` |
| **Google Gemini** | `connectors/llm/src/providers/gemini.rs` | YES — curl subprocess | `ENABLE_REAL_API=1` + `GEMINI_API_KEY` |
| **DeepSeek** | `connectors/llm/src/providers/deepseek.rs` | YES — curl subprocess | `ENABLE_REAL_API=1` + `DEEPSEEK_API_KEY` |
| **Mock** | `connectors/llm/src/providers/mock.rs` | NO — returns canned response | Always available (default fallback) |

All paid providers (OpenAI, Claude, Gemini, DeepSeek) are behind the `ENABLE_REAL_API=1` environment variable gate. Only Ollama and Mock work out of the box.

### Wasm Sandbox: REAL (Wasmtime)

**Location:** `sdk/src/wasmtime_sandbox.rs` (440+ lines)

- Uses **real wasmtime v42** (`wasmtime::Engine`, `wasmtime::Store`, `wasmtime::Module`)
- **Module validation:** `wasmtime::Module::validate(&engine, &bytecode)`
- **Memory isolation:** `StoreLimitsBuilder::new().memory_size(limit).build()` per agent
- **Stack limits:** 512 KB per agent
- **Fuel metering:** `store.set_fuel(units)` with 1 Nexus unit = 10,000 wasm fuel
- **Signature verification:** Optional Ed25519 via `ed25519-dalek` (defaults to RequireSigned)
- **Host function governance:** All host calls check `AgentContext` for capabilities
- **Module cache:** `ModuleCache` prevents recompilation of identical bytecode
- **Speculative execution:** Optional `SpeculativePolicy` intercepts host calls

**Verdict: REAL — not simulated.** Uses actual wasmtime APIs for isolation, metering, and execution.

### Audit System: IN-MEMORY ONLY

**Location:** `kernel/src/audit/mod.rs`

- Storage: `Vec<AuditEvent>` — unbounded in-memory array
- Hash chain: SHA-256 linking each event to previous (tamper-proof)
- **NO disk I/O by default**
- Optional `BlockBatchSink` trait for distributed persistence (caller implements)
- `verify_integrity()` validates entire chain in O(n)
- Genesis hash: `"0000...0000"`

**Verdict: Integrity is REAL (cryptographic hash chain), but persistence is NOT (memory only, lost on restart).**

### RAG System: REAL Vector Operations

**Location:** `connectors/llm/src/vector_store.rs` + `connectors/llm/src/rag.rs`

- **Real cosine similarity:** `cos_sim = (A · B) / (|A| * |B|)` — actual dot product + L2 norm
- **Real embeddings:** Calls `provider.embed(&chunk_texts, &model)` for actual vectors
- **Real chunking:** Document text split by format (markdown, code, plain text)
- **Real PII redaction:** `RedactionEngine.process_prompt()` before indexing
- **Content hashing:** SHA-256 for integrity verification
- **Storage:** In-memory `Vec<StoredEmbedding>` (not persisted to disk)

**Verdict: REAL vector math, but vectors live only in memory.**

### Trait Definitions and Implementations

| Trait | Purpose | Implementations |
|-------|---------|----------------|
| `LlmProvider` | LLM inference API | 6 (Ollama, OpenAI, Claude, Gemini, DeepSeek, Mock) |
| `BlockBatchSink` | Distributed audit storage | 2 (NoOpSink, AuditChain) |
| `ConsensusProtocol` | Distributed consensus | 1 (NoOpConsensus) |
| `EventReplicator` | Audit event replication | 1 (NoOpReplicator) |
| `NodeRegistry` | Node discovery | 0 (trait only) |
| `DiscoveryProtocol` | Peer discovery | 0 (trait only) |
| `MlClassifier` | Semantic boundary | 1 (TfIdfClassifier) |
| `NexusAgent` | Agent interface | 4+ (coding, social-poster, web-builder, etc.) |

---

## SECTION 3: FRONTEND REALITY CHECK

### Pages: 41 Total

**Pages with REAL Tauri invoke() calls (directly or via backend.ts): ~20**
**Pages with hardcoded mock/fallback data: ~21**

| Page | Lines | Invoke Calls | Status |
|------|-------|-------------|--------|
| CodeEditor.tsx | 1020 | file_manager_* (4 calls) | REAL + mock git/agent workers |
| Terminal.tsx | 773 | terminal_execute, terminal_execute_approved | REAL |
| FileManager.tsx | 554 | file_manager_* (7 calls) | REAL |
| NotesApp.tsx | 585 | notes_list, notes_save, notes_delete | REAL |
| DatabaseManager.tsx | 673 | db_connect, db_list_tables, db_execute_query | REAL |
| ApiClient.tsx | 573 | api_client_request | REAL |
| PermissionDashboard.tsx | 654 | get_agent_permissions, update_*, bulk_* | REAL (via backend.ts) |
| SystemMonitor.tsx | 491 | get_live_system_metrics (polling 2s) | REAL |
| Settings.tsx | 736 | check_llm_status, test_llm_connection, get_llm_recommendations | REAL (via backend.ts) |
| Documents.tsx | 1132 | index_document, search_documents, chat_with_documents | REAL (via backend.ts) |
| ModelHub.tsx | 1139 | search_models, download_model, list_local_models | REAL (via backend.ts) |
| SetupWizard.tsx | 768 | detect_hardware, check_ollama, run_setup_wizard | REAL (via backend.ts) |
| TimeMachine.tsx | 752 | time_machine_* (7 calls) | REAL (via backend.ts) |
| VoiceAssistant.tsx | 670 | voice_*, jarvis_*, transcribe_* | REAL (via backend.ts) |
| LearningCenter.tsx | 809 | start_learning, learning_agent_action | REAL (via backend.ts) |
| AppStore.tsx | 795 | marketplace_search, marketplace_install | REAL (via backend.ts) |
| ComplianceDashboard.tsx | 475 | get_compliance_status, get_compliance_agents | REAL (via backend.ts) |
| Agents.tsx | 281 | list_agents, start/stop/pause/resume_agent | REAL (via props from App.tsx) |
| Chat.tsx | 327 | send_chat, chat_with_ollama | REAL (via props from App.tsx) |
| EmailClient.tsx | 463 | email_list, email_save, email_delete | REAL (via backend.ts) |
| AiChatHub.tsx | 697 | — | MOCK: fallback models, hardcoded agent list |
| ProjectManager.tsx | 681 | project_list, project_save, project_delete | REAL (via backend.ts) + localStorage |
| DesignStudio.tsx | 610 | — | MOCK: COMPONENT_LIBRARY (23), DESIGN_TOKENS (50+) |
| MediaStudio.tsx | 678 | — | MOCK: INITIAL_ASSETS (12), FILTERS (9) |
| DeployPipeline.tsx | 667 | — | MOCK: hardcoded pipeline data |
| Marketplace.tsx | 328 | marketplace_search (fallback) | MIXED: real backend + FALLBACK_TEMPLATES (10) |
| CommandCenter.tsx | 134 | — | MOCK: MOCK_AGENTS (6) |
| TrustDashboard.tsx | 92 | — | MOCK: MOCK_TRUST (6 agents) |
| ClusterStatus.tsx | 90 | — | MOCK: MOCK_NODES (4 cluster nodes) |
| AuditTimeline.tsx | 99 | — | MOCK: MOCK_EVENTS (12 timeline events) |
| Protocols.tsx | 305 | — | MOCK: MOCK_STATUS, MOCK_TOOLS (8), MOCK_CARDS (3) |
| MarketplaceBrowser.tsx | 236 | — | MOCK: MOCK_LISTINGS (5 agents) |
| Workflows.tsx | 248 | — | MOCK: WORKFLOWS (4), RUN_HISTORY (10) |
| Identity.tsx | 120 | list_identities (fallback) | MIXED: real backend + mockIdentities() |
| Audit.tsx | 295 | get_audit_log (via backend.ts) | REAL but hardcoded AGENT_NAMES/COLORS |
| Dashboard.tsx | 59 | — | Read-only fuel display from props |
| Firewall.tsx | 264 | get_firewall_status, get_firewall_patterns | REAL (via backend.ts) |
| DeveloperPortal.tsx | 328 | — | Not fully wired |
| PolicyManagement.tsx | 380 | policy_list, policy_validate, policy_test | REAL (via backend.ts) |
| AgentBrowser.tsx | 414 | — | Not fully wired |
| DistributedAudit.tsx | 226 | — | Not fully wired |

### Tauri invoke() Calls in Frontend

**117 unique invoke wrappers** in `app/src/api/backend.ts`, covering all 226 Tauri commands.

**23 direct `invoke()` calls** in page .tsx files (mostly file manager, terminal, notes, database).

The remaining ~94 commands are called through `backend.ts` wrappers used across pages.

### Empty onClick Handlers

**Zero empty `() => {}` handlers found.** All click handlers have real logic (state updates, navigation, or invoke calls).

### Hardcoded Mock Data Arrays

| File | Constant | Items | Description |
|------|----------|-------|-------------|
| Marketplace.tsx | `FALLBACK_TEMPLATES` | 10 | Fake agent listings (SEO Writer, Bug Fixer, etc.) |
| CommandCenter.tsx | `MOCK_AGENTS` | 6 | Static agent list with fake fuel/status |
| AuditTimeline.tsx | `MOCK_EVENTS` | 12 | Fake timeline events |
| TrustDashboard.tsx | `MOCK_TRUST` | 6 | Fake trust scores |
| ClusterStatus.tsx | `MOCK_NODES` | 4 | Fake cluster nodes |
| Protocols.tsx | `MOCK_STATUS/TOOLS/CARDS` | 14 | Fake protocol data |
| DesignStudio.tsx | `COMPONENT_LIBRARY` | 23 | Fake UI components |
| DesignStudio.tsx | `DESIGN_TOKENS` | 50+ | Fake design tokens |
| MediaStudio.tsx | `INITIAL_ASSETS` | 12 | Fake media files |
| MediaStudio.tsx | `FILTERS` | 9 | Fake image filters |
| Workflows.tsx | `WORKFLOWS` | 4 | Fake workflow definitions |
| Workflows.tsx | `RUN_HISTORY` | 10 | Fake run history |
| MarketplaceBrowser.tsx | `MOCK_LISTINGS` | 5 | Fake marketplace listings |
| CodeEditor.tsx | `MOCK_GIT_*` | 8 | Fake git changes/log/agent workers |
| AiChatHub.tsx | `FALLBACK_MODELS` | 1 | Fallback mock model |
| AiChatHub.tsx | `AGENTS` | 4 | Hardcoded agent list |
| Audit.tsx | `AGENT_NAMES/COLORS` | 6 | Hardcoded agent display names |

---

## SECTION 4: API AND PROTOCOL STATUS

### Axum HTTP Routes: 30 Routes

**Primary file:** `protocols/src/http_gateway.rs` (144KB, 3000+ lines)

| Method | Path | Real Work? | Details |
|--------|------|-----------|---------|
| GET | `/health` | YES | Uptime, agent counts, audit chain integrity, memory usage |
| GET | `/metrics` | YES | Prometheus exposition format |
| GET | `/auth/jwks` | YES | OIDC JWKS endpoint (Ed25519) |
| GET | `/a2a/agent-card` | YES | Returns A2A AgentCard from manifest |
| GET | `/v1/models` | YES | OpenAI-compatible model listing |
| GET | `/ws?token=<jwt>` | YES | WebSocket upgrade, broadcasts real events |
| POST | `/a2a` | YES | A2A task submission with real state machine |
| GET | `/a2a/tasks/{id}` | YES | Task status lookup |
| GET | `/mcp/tools/list` | YES | MCP tool listing with governance metadata |
| POST | `/mcp/tools/invoke` | YES | Tool invocation with capability/fuel/egress checks |
| GET | `/api/agents` | YES | List all agents from supervisor |
| POST | `/api/agents` | YES | Create agent with full kernel registration |
| POST | `/api/agents/{id}/start` | YES | Start agent |
| POST | `/api/agents/{id}/stop` | YES | Stop agent |
| GET | `/api/agents/{id}/status` | YES | Agent health query |
| GET/PUT | `/api/agents/{id}/permissions` | YES | Permission management |
| POST | `/api/agents/{id}/permissions/bulk` | YES | Bulk permission update |
| GET | `/api/audit/events` | YES | Paginated audit trail query |
| GET | `/api/audit/events/{id}` | YES | Single event lookup |
| GET | `/api/compliance/status` | YES | Compliance snapshot |
| GET | `/api/compliance/report/{id}` | YES | Transparency report generation |
| POST | `/api/compliance/erase/{id}` | YES | Cryptographic data erasure |
| GET | `/api/marketplace/search` | STUB | Returns empty `{agents: []}` |
| GET | `/api/marketplace/agents/{id}` | STUB | Returns hardcoded dummy data |
| POST | `/api/marketplace/install/{id}` | PARTIAL | Real local registration, marketplace lookup stubbed |
| GET | `/api/identity/agents` | YES | Lists agent DIDs with real Ed25519 keys |
| GET | `/api/identity/agents/{id}` | YES | Single agent identity |
| GET | `/api/firewall/status` | YES | Real pattern counts from kernel |
| POST | `/v1/chat/completions` | CONDITIONAL | Real if LLM provider injected, mock otherwise |
| POST | `/v1/messages` | CONDITIONAL | Anthropic-compatible, real if provider injected |

### A2A Protocol

**Status: REAL state machine**

- Task states: Submitted → Working → Completed/Failed/Canceled
- `can_transition_to()` enforces valid transitions
- `GovernanceContext` attached to every task (autonomy level, fuel budget, capabilities)
- `AgentCard` auto-generated from manifest with auth requirements based on autonomy level
- Protocol version: 0.2.1
- **Limitation:** Tasks stored in `HashMap` (memory only, not persisted)

### MCP Server

**Status: REAL tool invocation with governance**

- 11 tools derived from manifest capabilities
- Each tool has governance metadata: required_capabilities, min_autonomy_level, estimated_fuel_cost, requires_hitl, pii_redaction
- Invocation pipeline: resolve agent → capability check → fuel check → egress check → execute → fuel deduction → audit append
- **Limitation:** Tool execution is "governed mock" — checks all governance but actual tool effects are limited

### REST API

**Status: MOSTLY REAL**

- Agent lifecycle: fully connected to kernel supervisor
- Permissions: live modification of manifest capabilities
- Audit: real hash-chain queries with pagination
- Compliance: real snapshot and erasure
- Marketplace: STUBBED (returns empty/dummy data)
- All endpoints go through JWT auth middleware

### WebSocket

**Status: REAL event broadcasting**

- Events: `fuel_consumed`, `agent_status_changed`, `audit_event`, `compliance_alert`, `firewall_block`, `speculation_decision`
- Channel capacity: 256
- Lag warning when consumers can't keep up
- JWT authentication via query parameter

---

## SECTION 5: AGENT EXECUTION FLOW

### Agent Creation Flow

```
Frontend (CreateAgent.tsx)
  → 4-step wizard: name, capabilities (auto-detected), fuel budget, model
  → JSON manifest generated
  → invoke("create_agent", { manifestJson })
    ↓
Tauri Command (main.rs:7795)
  → super::create_agent(state, manifest_json)
    ↓
Backend Implementation (main.rs:284)
  → Parse AgentManifest from JSON
  → supervisor.lock()
  → supervisor.start_agent(manifest) → returns UUID
  → identity_mgr.get_or_create(agent_id) → DID + Ed25519 key
  → Store AgentMeta (name, last_action)
  → Log audit event (UserAction: create_agent)
    ↓
Kernel Supervisor (supervisor.rs:126)
  → Generate UUID v4
  → Parse AutonomyLevel from manifest (default L0)
  → Initialize ConsentRuntime (HITL)
  → Determine ExecutionMode (Wasm if "wasm.binary:" capability, else Native)
  → Create AgentHandle (manifest, autonomy guard, consent, fuel)
  → Insert into self.agents HashMap
  → Create AgentFuelLedger with BurnAnomalyDetector
  → Append audit events (fuel.period_set, autonomy.level_initialized)
  → Transition state: Created → Starting → Running
  → Consume fuel for supervisor.start
  → Create TimeMachine checkpoint
  → Return agent_id
```

### Agent Execution (What happens when "started")

**Critical finding: Agents are PASSIVE state machines, not background processes.**

- `start_agent()` transitions state to Running but doesn't spawn a thread/process
- No continuous execution loop
- Agents only execute when explicitly triggered:
  - Chat: `send_chat()` → selects provider → sends single LLM query
  - Conductor: `conduct_build()` → orchestrates multi-step pipeline
  - Learning/Research/Build: session-based, user-driven step-by-step
- The `schedule` field in manifests (cron expressions) is parsed but never activated

**LLM calls ARE real** (when configured):
- `chat_with_ollama()` → streams tokens from local Ollama with 50ms throttle
- `send_chat()` → routes through GovernedLlmGateway with fuel checks

**Wasm execution IS real** (when triggered):
- WasmtimeSandbox loads real .wasm bytecode
- Host functions check capabilities and consume fuel
- But no built-in mechanism to auto-run wasm agents

### Agent Output Flow

```
LLM Response received
  → GovernedLlmGateway records fuel spend
  → PII redaction at gateway boundary
  → Response returned to Tauri command
  → Emitted as Tauri event (chat-token, conduct-progress)
  → Frontend receives via event listener or invoke return
  → Displayed in Chat/Build/Research UI
```

### Supervisor / Multi-Agent Coordination

**Status: REAL but triggered manually**

- `TeamOrchestrator` creates teams with roles (Planner, Executor, Auditor)
- `TeamMessageBus` routes inter-agent messages
- `Conductor` orchestrates: Planner → Dispatcher → Monitor
- `Collaboration` crate provides shared blackboard
- `DelegationChain` tracks authority transfers
- **All in-memory.** No automatic coordination — requires explicit API calls.

### Governance Flow

**ConsentRuntime: REAL blocking**
- Operations enqueue `GovernedOperation` with HitlTier
- `require_operation()` returns error until approved
- `approve_operation()` / `deny_operation()` resolve pending
- **Gap:** No UI widget for approving pending consent requests

**AutonomyGuard: REAL gatekeeping**
- `require_tool_call()` checks L2+ required
- `require_multi_agent()` checks L3+ required
- `require_self_modification()` checks L4+ required
- Returns `AgentError` on insufficient level

**SpeculativeEngine: REAL shadow simulation**
- `fork_state()` creates shadow copy
- `simulate()` runs governance checks without executing
- `commit()` / `rollback()` based on simulation result
- Only triggers for Tier2+ operations

---

## SECTION 6: DATABASE AND PERSISTENCE

### Databases

| Database | Location | Engine | Status |
|----------|----------|--------|--------|
| Marketplace Registry | `~/.nexus/marketplace.db` | SQLite (rusqlite) | REAL — tables: agents, reviews, versions |
| Notes | `~/.nexus/notes/` | JSON files on disk | REAL — each note is a JSON file |
| Emails | `~/.nexus/emails/` | JSON files on disk | REAL — each email is a JSON file |
| Projects | `~/.nexus/projects/` | JSON files on disk | REAL — each project is a JSON file |
| NexusConfig | `~/.nexus/config.toml` | TOML file on disk | REAL — hardware, LLM settings |

### What Is Persisted vs In-Memory Only

| Data | Persisted? | Storage |
|------|-----------|---------|
| Marketplace agents, reviews, versions | YES | SQLite |
| Notes, emails, projects | YES | JSON files |
| NexusConfig (hardware, LLM settings) | YES | TOML file |
| Agent manifests (registered agents) | NO | In-memory HashMap |
| Audit trail events | NO | In-memory Vec |
| Fuel ledgers | NO | In-memory struct |
| Agent execution state | NO | In-memory enum |
| Permissions/capabilities | NO | In-memory HashMap |
| Consent queue | NO | In-memory HashMap |
| Vector store (RAG embeddings) | NO | In-memory Vec |
| Time machine checkpoints | NO | In-memory Vec |
| Speculative simulations | NO | In-memory HashMap |

**Critical gap:** All agent state, audit history, fuel records, permissions, and governance state are lost on application restart.

### Marketplace Registry

- **Schema:** 3 tables (agents, reviews, versions) with indexes
- **Population:** Empty on first access; populated via `nexus marketplace publish`
- **No pre-loaded catalog.** Frontend uses `FALLBACK_TEMPLATES` array as placeholder.

---

## SECTION 7: EXTERNAL INTEGRATIONS

| Service | File | Real HTTP? | Authentication | Default Status |
|---------|------|-----------|----------------|----------------|
| **Ollama** | `connectors/llm/src/providers/ollama.rs` | YES (`reqwest::Client`) | None (localhost) | Available if installed |
| **OpenAI** | `connectors/llm/src/providers/openai.rs` | YES (curl subprocess) | `OPENAI_API_KEY` | Gated: `ENABLE_REAL_API=1` |
| **Claude/Anthropic** | `connectors/llm/src/providers/claude.rs` | YES (`reqwest::blocking`) | `ANTHROPIC_API_KEY` | Gated: `real-claude` feature + `ENABLE_REAL_API=1` |
| **Google Gemini** | `connectors/llm/src/providers/gemini.rs` | YES (curl subprocess) | `GEMINI_API_KEY` | Gated: `ENABLE_REAL_API=1` |
| **DeepSeek** | `connectors/llm/src/providers/deepseek.rs` | YES (curl subprocess) | `DEEPSEEK_API_KEY` | Gated: `ENABLE_REAL_API=1` |
| **Twitter/X** | `connectors/web/src/twitter.rs` | YES (OAuth 1.0a signing) | `TWITTER_*` env vars | Real OAuth implementation |
| **Brave Search** | `connectors/web/src/search.rs` | YES | `BRAVE_API_KEY` | Gated |
| **Telegram** | `connectors/messaging/src/telegram.rs` | YES | `TELEGRAM_BOT_TOKEN` | Gated |
| **Facebook** | `connectors/social/src/facebook.rs` | Structured but no HTTP | Access token | Partially implemented |
| **Instagram** | `connectors/social/src/instagram.rs` | Structured but no HTTP | Access token | Partially implemented |
| **Discord** | `connectors/messaging/src/discord.rs` | Structured but no HTTP | Bot token | Partially implemented |
| **Slack** | `connectors/messaging/src/slack.rs` | Structured but no HTTP | OAuth token | Partially implemented |
| **WhatsApp** | `connectors/messaging/src/whatsapp.rs` | Structured but no HTTP | API key | Partially implemented |
| **GitHub** | `connectors/core/src/github_connector.rs` | Structured but no HTTP | PAT | Partially implemented |

### Ollama Auto-Detection

**YES, it works:**
- `check_ollama` command → `OllamaProvider::check_health()` → HTTP GET `http://localhost:11434`
- `is_ollama_installed` → runs `ollama --version` subprocess
- `ensure_ollama` → starts Ollama if not running
- `pull_model` → streams model download with progress events
- `chat_with_ollama` → streaming inference with 50ms token throttle

---

## SECTION 8: TEST COVERAGE MAP

### Total Tests: 2,189 (excluding target/)

| Crate | Tests | Key Areas Tested |
|-------|-------|-----------------|
| kernel | 929 | Supervisor, audit chain integrity, fuel metering, autonomy guards, consent, permissions, speculative execution, kill gates, firewall, protocols, compliance, delegation, time machine |
| connectors | 304 | LLM gateway fuel enforcement, PII redaction, RAG pipeline, provider routing, web search, messaging |
| sdk | 217 | Wasmtime sandbox, shadow sandbox, speculative policies, module cache, agent trait |
| distributed | 168 | Audit chain immutability, consensus, replication, node identity |
| agents | 132 | Coder scanning/codegen, designer components, self-improve, web-builder, workflow DAGs, conductor orchestration |
| cli | 114 | Agent creation, coding agent start, self-improve, SDK workflows |
| marketplace | 84 | SQLite registry, ed25519 signing, publish/search/install pipeline |
| app | 46 | Tauri command wiring |
| factory | 30 | Agent scaffolding from NL |
| adaptation | 23 | Strategy adaptation |
| cloud | 22 | Cloud scaffolding |
| enterprise | 21 | RBAC |
| tests/integration | 18 | Full pipeline, E2E workflows, governance benchmarks, dry runs |
| protocols | 16 | HTTP gateway, A2A, MCP |
| packaging | 15 | Airgap bundles |
| control | 15 | Computer control |
| research | 12 | Research pipeline |
| self-update | 10 | Mutation kill gates |
| workflows | 5 | Workflow engine |
| analytics | 5 | Analytics |
| content | 3 | Content generation |

### Test Quality Assessment

**Testing real functionality (not just mocks):**
- Audit chain: creates 50+ events, verifies hash integrity, tampers events to test detection
- Fuel metering: creates GovernedLlmGateway, enforces budget caps, validates violation reporting
- Marketplace: real SQLite writes, ed25519 signing, search queries
- Wasmtime: loads real bytecode, validates fuel consumption, tests memory limits
- Permissions: grants, revokes, bulk updates, history tracking

**Tests using mock providers:**
- LLM tests use `MockProvider` / `ScriptedProvider` (predetermined responses)
- No tests hit real external APIs by default (gated behind `ENABLE_REAL_API=1`)
- Agent tests validate governance logic but not actual LLM inference

**Critical path tests:**
- `acceptance_criteria_tests.rs` — meta-test enforcing ≥400 kernel tests
- `fuel_hardening_phase5.rs` — fuel budget cap enforcement
- `kill_gates_phase7.rs` — emergency shutdown gates
- `safety_supervisor_phase6.rs` — safety assessment
- `protocols_integration_tests.rs` — A2A/MCP protocol correctness
- `wasmtime_integration_tests.rs` — real sandbox execution

---

## SECTION 9: CONFIGURATION

### Config Files

| File | Format | Controls |
|------|--------|----------|
| `~/.nexus/config.toml` | TOML | Hardware profile, LLM provider, API keys, model preferences |
| `agents/*/manifest.toml` | TOML | Agent capabilities, fuel budget, autonomy level, schedule, model |
| `audit.toml` | TOML | Audit configuration |
| `deny.toml` | TOML | Dependency audit rules |
| `rust-toolchain.toml` | TOML | Rust version pinning |
| `app/src-tauri/tauri.conf.json` | JSON | Tauri app config (window, bundle, permissions) |
| `app/tsconfig.json` | JSON | TypeScript config |
| `app/vite.config.ts` | TS | Vite bundler config |
| `.cargo/config.toml` | TOML | Cargo build settings |

### Environment Variables

| Variable | Purpose | Required? |
|----------|---------|-----------|
| `ENABLE_REAL_API` | Gate for all paid LLM providers (set to "1") | For external LLM |
| `ANTHROPIC_API_KEY` | Claude API authentication | For Claude |
| `ANTHROPIC_URL` | Custom Claude endpoint | No (defaults to api.anthropic.com) |
| `OPENAI_API_KEY` | OpenAI authentication | For OpenAI |
| `OPENAI_URL` | Custom OpenAI endpoint | No |
| `GEMINI_API_KEY` | Google Gemini authentication | For Gemini |
| `GEMINI_URL` | Custom Gemini endpoint | No |
| `DEEPSEEK_API_KEY` | DeepSeek authentication | For DeepSeek |
| `DEEPSEEK_URL` | Custom DeepSeek endpoint | No |
| `OLLAMA_URL` | Custom Ollama endpoint | No (defaults to localhost:11434) |
| `BRAVE_API_KEY` | Brave Search API | For web search |
| `TELEGRAM_BOT_TOKEN` | Telegram bot auth | For Telegram |
| `JWT_SECRET` | HTTP gateway JWT signing | For API server |
| `LLM_PROVIDER` | Default LLM provider selection | No |
| `NEXUS_CONFIG_PATH` | Custom config file path | No |
| `NEXUS_CONFIG_KEY` | Config encryption key | No |
| `NEXUS_HTTP_ADDR` | HTTP gateway bind address | No |
| `NEXUS_CORS_ORIGINS` | Allowed CORS origins | No |
| `NEXUS_SELF_IMPROVE_DIR` | Self-improve working directory | No |
| `HOME` | User home directory (for ~/.nexus/) | Yes |

### CLI Commands

| Command | Function | Status |
|---------|----------|--------|
| `nexus create <template>` | Scaffold new agent (6 templates: basic, data-analyst, web-researcher, code-reviewer, content-writer, file-organizer) | REAL |
| `nexus test` | Test agent in sandbox | REAL |
| `nexus package` | Package into signed .nexus-agent bundle | REAL |
| `nexus conduct <prompt>` | Run Conductor orchestration | REAL |
| `nexus self-improve` | Run self-improvement loop | REAL |

---

## SECTION 10: SUMMARY SCORECARD

### Counts

| Metric | Total | Real | Mock/Stub | % Real |
|--------|-------|------|-----------|--------|
| Tauri commands | 226 | ~220 | ~6 | 97% |
| Frontend pages | 41 | ~20 with real invokes | ~21 mock/placeholder | 49% |
| Backend.ts invoke wrappers | 117 | 117 | 0 | 100% |
| Axum API routes | 30 | 26 | 4 | 87% |
| External integrations | 14 | 6 fully real | 8 partial/stubbed | 43% |
| Kernel public functions | 669+ | 669+ | 0 | 100% |
| Tests | 2,189 | ~2,100 test real logic | ~89 mock-only | 96% |
| Workspace crates | 35 | 35 | 0 | 100% |

### Overall "Realness" Assessment

| Layer | Score | Notes |
|-------|-------|-------|
| Kernel governance logic | **95%** | All in-memory but fully functional: audit, fuel, consent, permissions, speculative |
| LLM integration | **75%** | 6 real providers, but most gated behind env vars; Ollama works OOTB |
| Wasm sandbox | **90%** | Real wasmtime, but no pre-built .wasm agents to run |
| Frontend-to-backend wiring | **60%** | 117 wrappers defined, but many pages still use hardcoded mock data |
| Data persistence | **15%** | Only marketplace (SQLite) + notes/emails/projects (JSON files); everything else lost on restart |
| External service integrations | **40%** | Ollama real; Twitter OAuth real; most social/messaging are structural stubs |
| Multi-agent coordination | **70%** | Real state machines and governance, but manual triggering only |
| Protocol compliance (A2A/MCP) | **85%** | Well-implemented specs with governance, but in-memory state |

**Weighted overall: ~60% real**

### Top 10 Critical Gaps for Making Agents Actually Work

1. **No persistence layer.** All agent state, audit trails, fuel records, permissions, and governance state are lost on restart. Need SQLite/sled for core state.

2. **Agents are passive.** No background execution loop, no scheduler, no continuous running. Agents only do work when explicitly triggered by user action. Need a task scheduler and execution runtime.

3. **No real agent-to-LLM loop.** Chat sends one message, gets one response. No iterative planning/execution/reflection cycle like OpenAI Swarm, CrewAI, or AutoGPT. Need an agentic loop (plan → act → observe → repeat).

4. **HITL approval has no UI.** ConsentRuntime blocks operations and enqueues approval requests, but there's no frontend widget for humans to review and approve/deny pending requests.

5. **20+ frontend pages use hardcoded mock data.** CommandCenter, AuditTimeline, TrustDashboard, ClusterStatus, Protocols, Workflows, DesignStudio, MediaStudio all show fake data.

6. **Marketplace is empty.** No pre-loaded agent catalog. Frontend falls back to `FALLBACK_TEMPLATES`. The HTTP API marketplace endpoints are stubbed.

7. **Social/messaging connectors are structural stubs.** Facebook, Instagram, Discord, Slack, WhatsApp have type definitions and method signatures but no actual HTTP client calls.

8. **No real-time event streaming to frontend.** WebSocket is implemented on the API server side but the Tauri desktop app uses polling/invoke, not WebSocket. Agent status changes aren't pushed to UI.

9. **Cron/schedule field parsed but never activated.** Agent manifests support `schedule = "*/10 * * * *"` but there's no cron executor. Agents can't run on a schedule.

10. **RAG vectors not persisted.** Document embeddings live in `Vec<StoredEmbedding>` in memory. Re-indexing required after every restart. Need a vector database (qdrant, chroma, or at minimum SQLite with vector extension).
