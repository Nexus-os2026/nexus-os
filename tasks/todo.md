# Nexus OS Task Tracker

## Current Sprint
- [x] Wire real agent execution to Tauri backend (COMPLETE)
  - create_agent: parses AgentManifest from JSON, calls Supervisor.start_agent(), creates AgentIdentity with DID, audits with capabilities
  - start_agent: calls Supervisor.restart_agent() (stop → restart lifecycle), emits agent-status-changed event
  - stop_agent: calls Supervisor.stop_agent() triggering shutdown, emits agent-status-changed event
  - pause_agent / resume_agent: real Supervisor lifecycle transitions, emit events
  - list_agents: returns real data from Supervisor (capabilities, fuel_budget, fuel_remaining, state, DID, sandbox_runtime)
  - Real-time frontend updates via Tauri agent-status-changed event listener
  - Full governance active: capability checks, fuel metering, audit logging, permission checks during execution
- [x] Connect LLM gateway to actual Ollama/cloud providers — Complete LLM Management System (COMPLETE)
  - **6 LLM providers**: Ollama (local), OpenAI, DeepSeek, Google Gemini, Anthropic Claude, Mock fallback
  - OpenAI provider (connectors/llm/src/providers/openai.rs): OpenAI-compatible API, custom endpoint support
  - Gemini provider (connectors/llm/src/providers/gemini.rs): Google's OpenAI-compatible endpoint
  - **Smart Ollama error handling**: detects not installed vs not running vs no models, suggests exact command based on system RAM
  - **User-choosable providers**: Settings > LLM Providers section with enable/disable, API key management, connection testing
  - **4 routing strategies**: Priority, RoundRobin, LowestLatency, CostOptimized — configurable per-user
  - **Per-agent LLM assignment**: PermissionDashboard lets users assign specific provider per agent, or "Auto" for global routing
  - **Local-only mode**: per-agent toggle restricts to local providers for privacy-sensitive tasks
  - **Setup wizard**: when no LLM configured, detects system specs and recommends models with exact install commands
  - **Governance SLM routing**: warns when governance uses cloud LLM, suggests local model for privacy
  - `check_llm_status`: smart diagnostics (Ollama binary check, model count, latency, error hints, setup commands)
  - `get_llm_recommendations`: system-appropriate model suggestions based on RAM (phi3:mini < 8GB, llama3:8b 8-16GB, mixtral 16-32GB, llama3:70b 32GB+)
  - `set_agent_llm_provider`: per-agent provider assignment stored in encrypted config
  - `get_provider_usage_stats`: aggregate tokens/cost per provider from audit trail
  - `test_llm_connection`: sends test prompt and reports latency/success
  - LlmProviderEntry, AgentLlmAssignment types in kernel config (encrypted at rest)
  - ProviderSelectionConfig extended with openai_api_key, gemini_api_key fields
  - Full governance on all LLM calls: PromptFirewall, PII redaction, egress checks, fuel metering, audit trail
  - 1639 tests passing (5 new provider tests), zero regressions
- [x] Real system stats in header (CPU/RAM via sysinfo)
- [x] Smart capability detection case-insensitive fix
- [x] Agent button state management (disable Start when Running, etc)
- [x] Phase 7.4: Marketplace & Developer Toolkit (COMPLETE)
- [x] C.5: Policy Engine Integration Tests (COMPLETE)
  - 13 integration tests in kernel/tests/policy_engine_integration_tests.rs
  - TOML loading, allow/deny evaluation, deny-overrides-allow, conditional autonomy level + fuel cost checks
  - Time window storage, default-deny semantics, Supervisor end-to-end with consent tier override
  - Policy reload, invalid TOML rejection, wildcard principal matching, audit trail fail-closed verification

## Completed
- [x] Phase 1: Hardening (benchmarks, replay evidence, circuit breakers)
- [x] Phase 2: Distributed (replication, quorum, federation, marketplace)
- [x] Phase 3: Ecosystem (SDK, enterprise, cloud)
- [x] Phase 4: Intelligence (collaboration, delegation, adaptive, fine-tuning)
- [x] Phase 5: Production (sandbox, networking, CLI, UI, docs, E2E tests)
- [x] Phase 6.1: Real Wasm Agent Sandboxing (COMPLETE)
  - WasmtimeSandbox implementing SandboxRuntime trait (wasmtime v27, store-per-agent isolation)
  - 6 governance-gated host functions delegating through AgentContext (nexus_log, nexus_emit_audit, nexus_llm_query, nexus_fs_read, nexus_fs_write, nexus_request_approval)
  - Fuel sync with AgentFuelLedger via deduct_wasm_fuel() (1 Nexus fuel = 10,000 wasmtime instructions, round-up)
  - SafetySupervisor kill_with_reason() integration for descriptive halt reasons
  - WasmAgent NexusAgent adapter with checkpoint/restore lifecycle
  - Ed25519 signature verification before wasm compilation (SignaturePolicy: RequireSigned/AllowUnsigned)
  - `nexus sandbox status` CLI command (25 total CLI commands)
  - Tauri desktop "Isolated (wasmtime)" badge + fuel usage indicator per agent
  - 50 new tests (492 total) with zero regressions
- [x] Phase 6.2: Speculative Execution — Shadow Simulation (COMPLETE - ALL GAPS CLOSED)
  - SpeculativeEngine in kernel/src/speculative.rs: fork_state, simulate, present_preview, commit/rollback
  - RiskLevel enum (Low/Medium/High/Critical) synthesized from HitlTier + AutonomyLevel
  - StateSnapshot: frozen agent state for isolated simulation
  - SimulationResult: predicted_changes (FileChange, NetworkCall, DataModification, LlmCall), resource_impact, risk_level
  - Auto-simulation: Tier2+ operations trigger simulation before approval request
  - Supervisor integration: require_consent_with_simulation(), approve/deny_consent_with_simulation()
  - simulation_for_request() accessor links simulations to approval requests
  - `nexus simulation status` CLI command (26 total CLI commands)
  - SpeculativePreview UI component with risk badge, change list, resource impact, approve/reject buttons
  - SimulationPreview TypeScript types (RiskLevel, ResourceImpact, ActionPreviewItem, SimulationPreview)
  - Gap 1 CLOSED: ShadowSandbox forking — disposable wasm sandbox clones AgentContext, runs in isolation, collects SideEffects
  - Gap 2 CLOSED: Recording mode — AgentContext.enable_recording() captures ContextSideEffect log instead of executing
  - Gap 3 CLOSED: Host function interception — SpeculativePolicy + check_speculation() gate all 4 host functions (llm_query, fs_read, fs_write, request_approval); Block returns -6, HumanReview returns -7, no policy = exact 6.1 behavior
  - Gap 4 CLOSED: ThreatDetector with 4 detection categories: (1) path traversal (../, /etc/, /sys/, /proc/), (2) prompt injection (12 patterns), (3) capability escalation (vs manifest), (4) excessive resource (>80% fuel budget). Wired into speculation interception — Dangerous escalates to Block, Suspicious escalates to HumanReview
  - 10 integration tests in sdk/tests/speculative_shadow_tests.rs exercising full pipeline
  - 562 total tests with zero regressions
- [x] Phase 6.3: Local SLM Integration (COMPLETE)
  - LocalSlmProvider implementing LlmProvider trait with 3-param query(prompt, max_tokens, model)
  - ModelRegistry with discover/load/unload lifecycle, RAM check, task recommendation
  - GovernanceSlm with PII detection, prompt safety, capability risk assessment, content classification
  - GovernanceVerdict enum: Clean, PiiDetected, PromptUnsafe, HighRisk, Sensitive, Inconclusive
  - Governance-aware ProviderRouter: TaskType (General/Governance), route_task/route_governance prefers local-slm
  - ML-enhanced ThreatDetector: scan_side_effects_ml() alongside pattern-matching scan_side_effects()
  - MlScanner trait in SDK for governance model abstraction without circular deps
  - Feature flag `local-slm` for candle-core/candle-nn/candle-transformers/tokenizers/hf-hub/safetensors
  - CLI commands: `nexus model list/download/load/unload/status`, `nexus governance test` (32 total CLI commands)
  - Tauri desktop SlmStatusBadge: model loaded state, latency indicator, governance routing (LOCAL/CLOUD/FALLBACK)
  - 15 integration tests in connectors/llm/tests/local_slm_integration_tests.rs
  - 717 total tests with zero regressions
- [x] Phase 6.5: Visual Permission Dashboard (COMPLETE)
  - PermissionManager in kernel/src/permissions.rs: risk levels, categories, history, locking, bulk operations
  - 6 permission categories (filesystem, network, ai, system, social, messaging) with 11 capability keys
  - PermissionRiskLevel enum (Low/Medium/High/Critical) with admin-only enforcement for Critical
  - Supervisor integration: 7 new public methods (get/update/bulk/history/requests/lock/unlock)
  - Tauri commands: 5 permission endpoints wired to invoke_handler
  - PermissionDashboard React page: toggle switches, risk badges, bulk actions, history timeline, capability request modal
  - Optimistic UI updates with revert on failure, confirmation modals for High/Critical changes
  - 30 new tests (13 integration + 14 unit + 3 existing), 804 total tests with zero regressions
- [x] Phase 6.4: Distributed Immutable Audit (COMPLETE)
  - AuditBlock with SHA-256 content-addressable hash chain and Ed25519 signatures
  - ContentAddressedStore (in-memory) and FileAuditStore (file-backed) persistence
  - AuditChain with append_block, append_verified_block, verify_integrity, verify_chain, verify_event
  - TamperResult enum (Clean, ChainBroken, SignatureInvalid, SequenceGap, HashMismatch)
  - DevicePairingManager with Ed25519 keypair gen/persist, pairing codes, accept/revoke/list
  - GossipProtocol for block sync between paired devices with tamper detection
  - VerificationEngine for cross-device verification, chain integrity, SOC2 compliance reports
  - BlockBatchSink trait in kernel — events flow from AuditTrail through batcher into distributed AuditBlocks
  - 4 integration tests: 3-chain gossip sync, tamper detection, 3-of-3 verification, kernel→block UUID preservation
  - CLI commands: audit verify-chain/verify-event/distributed-status/compliance-report, device pair/list/revoke (39 total)
  - Tauri desktop DistributedAudit page: chain block visualization, device sync status, tamper alert banner
- [x] Full UI audit: 19 fixes across 12 files
- [x] Core agents always present with SYSTEM/CUSTOM badges
- [x] Smart capability auto-detection in agent factory
- [x] LLM model selector dropdown
- [x] Engineering Foundation Hardening (COMPLETE)
  - Fail-closed audit enforcement: 199 call sites fixed across 75+ files — `append_event` returns `Result<Uuid, AuditError>`, zero silent failures remain
  - `AuditError` type added to kernel with `BatcherPoisoned` variant; `From<AuditError>` impls for `AgentError`, `AdaptationError`, `MutationError`
  - Two-tier fix strategy: `?` for Result-returning functions, `.expect("audit: fail-closed")` for boundary/non-Result code
  - Node.js 20 → 22 LTS upgrade across all CI (`.gitlab-ci.yml`, `ci.yml`, `release.yml` — 5 references)
  - `rust-toolchain.toml` pinning stable channel with rustfmt, clippy, wasm32-wasip1 target for reproducible builds
  - `cargo-audit` CI job for known vulnerability scanning (RUSTSEC advisory database)
  - `cargo-deny` CI job for license compliance — allows MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, CDLA-Permissive-2.0
  - `deny.toml` moved from `.github/` to project root (standard location), old file removed
  - Security stage runs before test stage in GitLab CI pipeline
  - 4 wasmtime advisories (RUSTSEC-2025-0046, -0118, RUSTSEC-2026-0020, -0021) tracked for separate version bump
  - 905 tests pass, clippy clean, cargo deny clean
- [x] Phase 7.1: A2A + MCP Protocol Integration (COMPLETE)
  - A2A core types in kernel/src/protocols/a2a.rs: AgentCard, A2ATask, TaskStatus lifecycle, GovernanceContext, JSON-RPC 2.0, AgentCard::from_manifest auto-generation (all 11 capabilities → skills)
  - MCP governed tool server in kernel/src/protocols/mcp.rs: McpServer with capability check → fuel check → execute → fuel deduct → audit pipeline, GovernedTool/GovernedResource/GovernedPrompt types, 11 capability-to-tool mappings
  - GovernanceBridge in kernel/src/protocols/bridge.rs: unified gateway routing A2A tasks and MCP invocations through full governance pipeline (sender auth → capability check → fuel check → speculative simulation for Tier2+ → execute → audit)
  - CLI commands: `nexus protocols status`, `nexus protocols agent-card <name>`, `nexus protocols start --port` (42 total CLI commands)
  - Tauri desktop Protocols page: A2A/MCP server status cards, MCP Tool Registry table, Agent Cards grid with JSON preview, Recent Protocol Requests with governance decisions
  - 9 integration tests in kernel/tests/protocols_integration_tests.rs: Agent Card mapping, task lifecycle, tool discovery governance, capability+fuel enforcement, sender auth, MCP capability denial, speculative execution triggers, full audit coverage, no-bypass verification
  - 95+ unit tests across a2a.rs (30), mcp.rs (25), bridge.rs (16), router.rs (3), commands.rs (5)
- [x] Phase 7.2: Identity & Firewall (COMPLETE)
  - Ed25519 cryptographic identity per agent: AgentIdentity with DID derivation (did:key:z6Mk...), signing, verification, persistence via IdentityManager
  - EdDSA JWT token system: TokenManager with OIDC-A claims (iss, sub, aud, exp, iat, jti, scope, agent_did, delegator_sub), issue/validate/refresh/revoke, JWKS endpoint
  - PromptFirewall: InputFilter (20 injection patterns, homoglyph detection, context overflow, PII+SSN+passport redaction) + OutputFilter (JSON schema validation, exfiltration detection)
  - EgressGovernor: per-agent URL allowlisting, rate limiting (sliding 60s window), default deny, wired into GovernedLlmGateway and MCP tool invocation
  - Canonical patterns.rs: single source of truth for all security patterns (20 injection, 6 PII, 7 exfil, 3 sensitive paths, SSN/passport/IP regex)
  - CLI commands: `nexus identity show/list`, `nexus token issue`, `nexus firewall status/patterns` (47 total CLI commands)
  - Tauri desktop Identity page and Firewall page with pattern library
  - 14 integration tests in kernel/tests/identity_firewall_integration_tests.rs
  - 932 total tests with zero regressions
- [x] Phase 7.3: Compliance, Erasure & Provenance (COMPLETE)
  - CLI commands: `nexus compliance status` (real ComplianceMonitor), `nexus compliance classify <agent-id>` (EU AI Act risk tier), `nexus compliance erase-agent-data <agent-id>` (GDPR Article 17 cryptographic erasure), `nexus compliance retention-check` (retention policy enforcement), `nexus compliance provenance <agent-id>` (data lineage report) — 51 total CLI commands
  - Kernel compliance module: ComplianceMonitor (6 continuous checks), RiskClassifier (4 EU AI Act tiers), AgentDataEraser (legal hold + cryptographic erasure), RetentionPolicy (per-data-class periods + legal hold), ProvenanceTracker (data lineage with origin/transformation/delegation tracking), TransparencyReportGenerator (Article 13 reports with JSON + Markdown output)
  - Tauri desktop Compliance page: 6-tab layout (Overview with green/yellow/red status indicator + alerts, Risk Cards with per-agent EU AI Act classification, Transparency Report viewer with download, Erasure controls with confirmation dialog, Data Provenance table with classification + transformations, Retention policy settings with visual bars + enforcement trigger)
  - 12 integration tests in kernel/tests/compliance_integration_tests.rs: risk classification across all tiers, unacceptable agent rejection at spawn, Article 13 transparency report completeness, cryptographic erasure with key destruction, erasure proof event under system UUID, legal hold prevention + release, retention purge with legal hold exemption, full data lineage chain, delegation handoff provenance, missing identity detection, broken audit chain detection, multi-framework SOC2+EU AI Act+HIPAA+CA AB316 report
  - All tests pass with zero regressions
- [x] Phase 7.4: Marketplace & Developer Toolkit (COMPLETE)
  - SQLite-backed marketplace registry (SqliteRegistry) replacing in-memory HashMap: agents/reviews/versions tables, publish/search/install/update/rate/get_agent methods, configurable DB path (~/.nexus/marketplace.db), auto-migration
  - 6-step verification pipeline (verification_pipeline.rs): signature_check, manifest_validation, sandbox_test, security_scan, capability_audit, governance_check with Approved/ConditionalApproval/Rejected verdicts
  - CLI 6 marketplace subcommands (search/install/publish/info/my-agents/uninstall) wired to real SQLite registry via router.rs
  - CLI developer toolkit: `nexus create` (6 templates), `nexus test` (3-phase lifecycle), `nexus package` (Ed25519 signed bundles with In-Toto attestation)
  - Tauri desktop 5 marketplace commands registered in invoke_handler, backed by same SQLite DB
  - MarketplaceBrowser page: real backend search, agent cards with ratings/downloads/price/capability badges, detail modal with reviews/versions/install
  - DeveloperPortal page: drag-drop .nexus-agent upload, animated 6-step verification pipeline, verdict display, published agent stats
  - 8 marketplace integration tests + 7 CLI SDK workflow tests covering full create→test→package→publish→install roundtrip
  - Stripe deferred to 7.5 — paid agents show price but install free during beta
- [x] Phase 7.5: Web API Integration Tests (COMPLETE)
  - 11 integration tests in protocols/tests/web_api_integration_tests.rs exercising the full HTTP gateway stack
  - REST API: list agents (multi-agent, correct fields), create agent (UUID validation), permissions CRUD (GET/PUT/POST bulk), compliance status (all ComplianceMonitor fields), marketplace search (query + list-all)
  - Security: all 10+ /api/* endpoints verified to reject unauthenticated, invalid JWT, and wrong-key JWT requests
  - WebSocket: valid JWT connects, receives 3 event types (AgentStatusChanged, FuelConsumed, ComplianceAlert) with correct data
  - Health endpoint: all 12 fields validated (status, version, agents_registered, tasks_in_flight, started_at, uptime_seconds, agents_active, total_tests_passed, audit_chain_valid, compliance_status, memory_usage_bytes, wasm_cache_hit_rate)
  - Metrics endpoint: Prometheus text format validation (# comments + nexus_ metric lines)
  - Graceful shutdown: 5 agents registered → shutdown within 5s timeout → agents_active=0 verified via health endpoint
  - WASM module cache: cache miss → cache hit performance, different modules cached separately, clear works
- [x] **PHASE 7 COMPLETE** — Full production-ready web API stack with A2A+MCP protocols, EdDSA JWT identity, prompt firewall, EU AI Act compliance, SQLite marketplace, developer SDK, and comprehensive integration test coverage

## Phase 8: Nexus Builder v2 — Beyond Lovable

> **Goal:** Transform the Build tab into a full split-pane visual builder that is
> visibly more advanced than Lovable. Chat on the left, live preview on the right,
> real-time agent progress, time-machine undo, governed deployment.
> 
> **Why this wins:** Lovable is 1 LLM call → iframe. Nexus Builder is multi-agent
> Conductor orchestration with real-time progress, governed audit trail, time-machine
> undo, remix classification, and local-first privacy. Built by Claude the brain.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Build Tab (view === "build")                 │
├──────────────────────────┬──────────────────────────────────────────┤
│   LEFT: Chat + Progress  │   RIGHT: Live Preview + Tools           │
│   (40% width)            │   (60% width)                           │
│                          │                                          │
│  ┌────────────────────┐  │  ┌──────────────────────────────────┐   │
│  │ Agent Progress Bar  │  │  │  [📱 375] [📱 768] [🖥 100%]  │   │
│  │ ████████░░ 65%     │  │  │  [↻ Refresh] [<> Code] [Deploy] │   │
│  │ 🔨 WebBuilder done │  │  ├──────────────────────────────────┤   │
│  │ 🎨 Designer active │  │  │                                  │   │
│  │ 🔧 Fixer queued    │  │  │       Live iframe preview        │   │
│  └────────────────────┘  │  │       of generated website        │   │
│                          │  │                                  │   │
│  ┌────────────────────┐  │  │       (auto-refreshes as          │   │
│  │ Chat Messages      │  │  │        files are written)         │   │
│  │                    │  │  │                                  │   │
│  │ User: I want to    │  │  ├──────────────────────────────────┤   │
│  │ sell t-shirts...   │  │  │  Files: index.html | styles.css  │   │
│  │                    │  │  │  [⟲ Time Machine: 3 checkpoints] │   │
│  │ AI: Building now!  │  │  └──────────────────────────────────┘   │
│  └────────────────────┘  │                                          │
│                          │                                          │
│  ┌────────────────────┐  │                                          │
│  │ [Make header blue] │  │                                          │
│  │        [Send ➤]    │  │                                          │
│  └────────────────────┘  │                                          │
├──────────────────────────┴──────────────────────────────────────────┤
│ Status: qwen3.5:4b · 3 agents · 2500 fuel · checkpoint #3          │
└─────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

| File | Changes | Complexity |
|------|---------|------------|
| `app/src/pages/AiChatHub.tsx` | Split Build view into 2-pane, preview state, progress tracking, deploy UI, code viewer | HIGH |
| `app/src/pages/ai-chat-hub.css` | New CSS for split pane, preview toolbar, progress bar, file tabs, deploy modal, code view | MEDIUM |
| `agents/conductor/src/lib.rs` | Emit Tauri events during task execution (plan, agent-start, agent-done, file-written) | MEDIUM |
| `app/src-tauri/src/commands/tools_infra.rs` | Pass AppHandle for event emission, add file-read + serve commands | MEDIUM |
| `app/src/types.ts` | New event types for streaming build progress | LOW |
| `app/src/api/backend.ts` | Add readBuildFile helper | LOW |

### Implementation Tasks

#### Phase 8.1: Split-Pane Layout + Static Preview
- [ ] 8.1.1 Restructure Build view: left chat pane (40%) + right preview pane (60%)
- [ ] 8.1.2 Add builder state: `previewHtml`, `previewFiles`, `buildOutputDir`, `buildPhase`, `previewMode`
- [ ] 8.1.3 Render iframe in right pane with srcdoc (inline HTML from generated files)
- [ ] 8.1.4 Preview toolbar: responsive toggles (Mobile 375px / Tablet 768px / Desktop 100%)
- [ ] 8.1.5 File tabs bar below preview showing generated files
- [ ] 8.1.6 Code view toggle: switch between live preview and raw source code
- [ ] 8.1.7 CSS classes: `.nb-split`, `.nb-chat`, `.nb-preview`, `.nb-toolbar`, `.nb-iframe`, `.nb-tabs`

#### Phase 8.2: Real-Time Build Progress Streaming
- [ ] 8.2.1 Conductor `run()`: emit Tauri events — `builder:plan-ready`, `builder:agent-start`, `builder:file-written`, `builder:agent-done`, `builder:complete`
- [ ] 8.2.2 `conduct_build` in tools_infra.rs: pass AppHandle to conductor for event emission
- [ ] 8.2.3 Frontend: listen for builder events, update progress in real-time
- [ ] 8.2.4 Agent progress panel: show each agent status (queued → active → done) with role icons
- [ ] 8.2.5 Progress bar: animate 0-100% based on completed/total tasks
- [ ] 8.2.6 Auto-refresh preview when `builder:file-written` fires for HTML/CSS/JS

#### Phase 8.3: Iteration & Remix Loop
- [ ] 8.3.1 After build: keep preview live, chat input active for changes
- [ ] 8.3.2 User types change → LLM modifies specific files → re-render preview
- [ ] 8.3.3 RemixEngine classification badge: Cosmetic (instant) / Minor / Major / Structural
- [ ] 8.3.4 Time Machine panel: list checkpoints, click to restore any previous version
- [ ] 8.3.5 Iteration counter: "Version 3 of 5 changes"

#### Phase 8.4: Deploy & Export
- [ ] 8.4.1 "Deploy" button in toolbar (visible after successful build)
- [ ] 8.4.2 Deploy modal: Local preview (xdg-open) / Export ZIP / Deploy to hosting
- [ ] 8.4.3 Governed deployment: audit event, fuel deduction, HITL gate for production
- [ ] 8.4.4 Share: local network preview URL via built-in HTTP server

#### Phase 8.5: Polish — What Makes This Better Than Lovable
- [ ] 8.5.1 Skeleton loading animation in preview during build
- [ ] 8.5.2 Console panel: build logs, agent communications, errors
- [ ] 8.5.3 Agent DNA badges on file tabs (which agent generated each file)
- [ ] 8.5.4 Governance sidebar: fuel consumed, audit trail, permissions used
- [ ] 8.5.5 Template gallery: e-commerce, portfolio, landing page, dashboard, blog

### What Makes This Better Than Lovable

| Feature | Lovable | Nexus Builder |
|---------|---------|---------------|
| Build engine | Single LLM call | Multi-agent Conductor (WebBuilder + Coder + Designer + Fixer) |
| Progress | Spinner → done | Real-time agent-by-agent with task breakdown |
| Undo/rollback | Git-style | Time Machine with instant checkpoint restoration |
| Change classification | None | Cosmetic/Minor/Major/Structural with time estimates |
| Governance | None | Full audit trail, fuel metering, HITL gates |
| Agent transparency | Hidden | See which agent built which file, autonomy levels |
| Security | Trust the cloud | WASM sandbox, PII redaction, capability-gated agents |
| Privacy | Cloud only | Local-first — your machine, your models, your data |
| Deploy options | Vercel only | Local / ZIP / multi-provider deploy |
| Cost | $20/month | Free (local models) or bring your own API key |

### Complexity Estimate

- **Phase 8.1**: ~350 lines TSX + ~200 lines CSS — split layout + preview
- **Phase 8.2**: ~120 lines Rust + ~180 lines TSX — event streaming + progress
- **Phase 8.3**: ~250 lines TSX — iteration with remix + time machine
- **Phase 8.4**: ~180 lines TSX + modal — deploy options
- **Phase 8.5**: ~250 lines TSX/CSS — polish and superiority features
- **Total**: ~1530 lines across 6 files

### Tests Needed
- [ ] Build view renders split pane at various screen sizes
- [ ] Preview iframe loads generated HTML via srcdoc
- [ ] Responsive toggles change iframe container width
- [ ] Progress bar updates in real-time during build
- [ ] File tabs list all generated files
- [ ] Code view shows raw source
- [ ] Iteration changes refresh preview
- [ ] Time Machine restore works
- [ ] Deploy modal opens correctly
- [ ] `npm run build` passes
- [ ] `cargo check -p nexus-conductor -p nexus-desktop-backend` passes
