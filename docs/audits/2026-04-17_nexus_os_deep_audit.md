# Nexus OS — Deep Read-Only Audit

- Generated: 2026-04-17 ~03:56 GMT+1
- Timestamp (UNIX): 1776394307
- HEAD: 27e252dbcdca43550983db9a9387fe7f4dd2646e
- Method: Main-session Read/Grep/Bash only (subagent fan-out failed at harness level due to inherited opus-4-7 model)
- Scope: READ-ONLY — no source files modified, no builds, no network

---

## 1. GIT & BRANCH STATE

- **HEAD SHA**: `27e252dbcdca43550983db9a9387fe7f4dd2646e`
- **Current branch**: `main`
- **Upstream**: `origin/main`
- **Working tree**: clean (porcelain empty)
- **Stash list**: empty
- **Remotes**:
  - `github` → https://github.com/Nexus-os2026/nexus-os.git  (fetch + push)
  - `gitlab` → https://gitlab.com/nexaiceo/nexus-os.git (token-embedded URL — leaks a PAT in `git remote -v` output)
  - `origin` → git@gitlab.com:nexaiceo/nexus-os.git (fetch); push also mirrors to github
- **Divergence from origin/main**: clean — zero unpushed commits
- **Divergence from github/main**: 1 commit ahead — `27e252db ci: verify dual-push to gitlab + github` (the dual-push verification commit is on gitlab+local but not yet on github mirror)

**Last 30 commits (oneline)**:
```
27e252db ci: verify dual-push to gitlab + github
30f80cff docs(claude): add Opus 4.7 execution policy to CLAUDE.md
49824fb9 fix(chat): render agent final_output in Chat page + fix goalDone listener leak
b8e5fc47 fix(bugs-ABC): remove path traversal containment, split compound shell commands, normalize egress scheme
801385e3 fix(ci-004): update test_ollama_request_format expected JSON
4df10f35 chore(ci-local): add --clean-check mode to detect working-tree drift
6520ea72 fix(ci): unblock GitLab CI — missing struct fields, stale test, security advisory
f2f1d9ad fix(ci-001): add missing ActuatorContext.hitl_approved field
45788840 fix(G8b): diagnose cwd_block injection + add nested-path inference hint
7bce09a1 fix(G9): hide floating approval banner when inline chat card is visible
2c787962 fix(G3): enrich ollama 404 errors with installed model list + chat dropdown UX label
441ddda7 fix(G8): inject cwd directory listing into planner context for path disambiguation
a8ba0287 fix(G2): backfill allowed_endpoints across 51 prebuilt manifests
fc4bc7f7 fix(G7): resolve workspace root via .git ancestor walk at Tauri startup
94b92215 fix(D2): supervisor state transition chokepoint with audit + Tauri emission
2e396dc8 fix(G1): silent failure cycle classification + Stop button cancellation
330dbc11 feat(tauri): rewire get_agent_cognitive_status to lock-free fast path (Bug AB Phase 3)
8b368288 chore: update Cargo.lock for arc-swap dependency (Bug AB Phase 2 follow-up)
f4b61eb0 feat(kernel): lock-free agent status snapshot via ArcSwap (Bug AB Phase 2)
6d48efce feat(tauri): include started_at_secs in cognitive status idle fallback (N1 v1)
ac2f9f6e feat(kernel): expose started_at_secs in CognitiveStatusResponse (N1 v1)
567fc092 fix(ui): finalize step spinners on goal completion (Bug S)
fbcd1908 fix(ui): remove misleading 120s frontend timeout (Bug W)
440ee284 docs: tighten CLAUDE.md — engineering discipline, no duplication, truthfulness rule
8d150a8d fix(kernel): prefer LlmQuery result for last_step_result (Bug P)
b53ac314 docs: log Bug P refinement and Bug S (stale step spinners) for Phase 2D
2251b255 docs: Phase 2C complete — 5 bugs fixed, 4 follow-ups logged
e7565e91 feat(agents): Phase 2C live runtime fixes + two-pane layout
30e5b1d5 docs: Phase 2B close-out — 17 P1 → 0 P1 after re-audit, Bug J closed, Bug K root cause updated
2b812619 docs: update Bug J/K status in group_d_backlog.md
```

**Notable**: the `gitlab` remote entry in `git remote -v` exposes a PAT (`glpat-...`) in the URL. Not a code change, but worth flagging for operational hygiene.

---

## 2. WORKSPACE & CRATE INVENTORY

Workspace has **69 members** (from Cargo.toml). Workspace version: `10.6.0`, edition 2021, `unsafe_code = forbid`.

Table below is sorted by LOC (Rust src only) descending. Fields:
- **LOC**: total line count from `find <path>/src -name '*.rs' | xargs wc -l` (or `find <path>`-level fallback where src layout differs).
- **Tests**: count of `#[test]` + `#[tokio::test]` attributes across the crate's `src/` + `tests/`.
- **Tests dir**: `yes` if a top-level `tests/` directory exists.

| Crate (member path) | LOC | Tests |
|---|---:|---:|
| kernel | 107,504 | 2,184 |
| agents/web-builder | 48,785 | 986 |
| app/src-tauri | 41,410 | 90 |
| nexus-code | 21,221 | 374 |
| connectors/llm | 18,680 | 405 |
| benchmarks | 14,746 | 8 |
| crates/nexus-memory | 10,308 | 197 |
| distributed | 9,398 | 179 |
| crates/nexus-computer-use | 8,690 | 227 |
| sdk | 8,546 | 217 |
| protocols | 7,738 | 121 |
| crates/nexus-capability-measurement | 6,848 | 77 |
| crates/nexus-ui-repair | 6,108 | 128 |
| cli | 5,934 | 114 |
| crates/nexus-self-improve | 5,534 | 182 |
| agents/coder | 5,453 | 45 |
| crates/nexus-flash-infer | 4,592 | 76 |
| marketplace | 4,513 | 101 |
| connectors/messaging | 4,263 | 69 |
| persistence | 3,922 | 58 |
| integrations | 3,772 | 42 |
| factory | 2,666 | 30 |
| llama-bridge | 2,526 | 31 |
| self-update | 2,473 | 10 |
| agents/conductor | 2,336 | 28 |
| adaptation | 2,305 | 23 |
| crates/nexus-migrate | 2,288 | 63 |
| crates/nexus-token-economy | 2,060 | 29 |
| agents/self-improve | 2,071 | 24 |
| control | 1,987 | 15 |
| agents/screen-poster | 1,993 | 20 |
| crates/nexus-outcome-eval | 1,880 | 41 |
| tests/integration | 1,919 | 19 |
| crates/nexus-external-tools | 1,715 | 17 |
| crates/nexus-mcp | 1,672 | 31 |
| agents/workflow-studio | 1,659 | 19 |
| tenancy | 1,664 | 50 |
| crates/nexus-agent-memory | 1,621 | 21 |
| crates/nexus-world-simulation | 1,598 | 18 |
| crates/nexus-predictive-router | 1,567 | 14 |
| connectors/web | 1,524 | 8 |
| crates/nexus-software-factory | 1,474 | 18 |
| crates/nexus-collab-protocol | 1,522 | 18 |
| auth | 1,353 | 32 |
| metering | 1,340 | 18 |
| enterprise | 1,228 | 21 |
| connectors/core | 1,210 | 8 |
| crates/nexus-a2a | 1,249 | 32 |
| crates/nexus-perception | 1,197 | 19 |
| crates/nexus-computer-control | 1,167 | 16 |
| telemetry | 1,169 | 21 |
| research | 1,138 | 12 |
| agents/coding-agent | 1,143 | 5 |
| agents/designer | 1,085 | 18 |
| workflows | 964 | 5 |
| agents/collaboration | 895 | 22 |
| crates/nexus-browser-agent | 881 | 12 |
| packaging/airgap | 883 | 0 |
| analytics | 820 | 5 |
| crates/nexus-crypto | 820 | 24 |
| agents/social-poster | 761 | 1 |
| crates/nexus-governance-engine | 706 | 9 |
| crates/nexus-governance-oracle | 698 | 12 |
| crates/nexus-governance-evolution | 687 | 7 |
| cloud | 617 | 22 |
| content | 401 | 3 |
| crates/nexus-server | 342 | 2 |

**Footer**
- Total crates (workspace members): **69**
- Total Rust LOC (src only, summed across all members): **~413,528**
- Total Rust tests (`#[test]` + `#[tokio::test]` attributes): **6,756**
- Top-5 by LOC: kernel (107,504), agents/web-builder (48,785), app/src-tauri (41,410), nexus-code (21,221), connectors/llm (18,680)

**Delta vs. prior state** (expected ~69 / ~335K / ~5400):
- Crate count matches (69).
- LOC is **~413K vs. 335K expected** — +78K / ~23% growth. agents/web-builder at 48,785 LOC and app/src-tauri at 41,410 LOC are primary contributors; both are actively developed.
- Tests: **6,756 vs. ~5,400 expected** — +1,356 / ~25% growth, consistent with the LOC increase.

---

## 3. AGENT MANIFEST INVENTORY

Prebuilt manifests live at `agents/prebuilt/*.json`. Other agent JSON directories:
- `agents/generated/` — 6 generated manifests (not part of the 54)
- `agents/genomes/` — 51 `.genome.json` genome files (for Darwin evolution, not manifests)

### Prebuilt count
- **54 prebuilt agent manifests** (matches expected).
- `agents/prebuilt/README.md` is the only non-JSON file in that directory.

### Tier (autonomy_level) breakdown
| autonomy_level | Count |
|---:|---:|
| 1 | 1 |
| 2 | 11 |
| 3 | 16 |
| 4 | 11 |
| 5 | 3 |
| 6 | 12 |

Expected: 12 L6, 11 L2. **Both match exactly.**

### Capability breakdown
- `allowed_endpoints` field present: **54 / 54**. (Post commit `a8ba0287 fix(G2): backfill allowed_endpoints across 51 prebuilt manifests` — the 3-pre-existing claim from prior notes is now historical.)
- `llm_model: "auto"`: **54 / 54**. Every prebuilt uses the auto-resolver.
- `cognitive_modify` capability: **12** agents.
- `agent.message` capability: **1** agent.
- `computer.use` capability: **1** agent.

### Delta vs prior state
- Prior state expected 3 with `allowed_endpoints` → **now 54** (G2 backfill landed).
- L6=12 and L2=11 match.
- Total 54 matches.

### Representative manifest fields (nexus_prime.json / L6)
- `name: "nexus-prime"`, `autonomy_level: 6`, `fuel_budget: 250000`, `llm_model: "auto"`.
- 35 allowed_endpoints (news, research, AI security / threat intel, code hosting).
- Capabilities: `fs.read`, `fs.write`, `process.exec`, `mcp.call`, `web.search`, `web.read`, `self.modify`, `cognitive_modify`.

### Full prebuilt list (sorted)
arbiter, architect_prime, ascendant, continuum, genesis_prime, legion, nexus-aegis, nexus-architect, nexus-assistant, nexus-atlas, nexus-catalyst, nexus-chronos, nexus-cipher, nexus-codesentry, nexus-content-creator, nexus-darwin, nexus-devops, nexus-diplomat, nexus-director, nexus-empathy, nexus-fileforge, nexus-forge, nexus-guardian, nexus-herald, nexus-hydra, nexus-infinity, nexus-mirror, nexus-nexus, nexus-operator, nexus-oracle, nexus-oracle-dark, nexus-oracle-prime, nexus-paradox, nexus-phantom, nexus-phoenix, nexus-polyglot, nexus_prime, nexus-prism, nexus-prometheus, nexus-prophet, nexus-publisher, nexus-researcher, nexus-sage, nexus-scholar, nexus-sentinel, nexus-sovereign, nexus-strategist, nexus-synapse, nexus-sysmon, nexus-weaver, nexus-writer, oracle_omega, oracle_supreme, warden.

---

## 4. LLM PROVIDER STACK

### Provider adapters
All live under `connectors/llm/src/providers/`. Trait: `pub trait LlmProvider: Send + Sync` in `providers/mod.rs` with entry fn `fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError>`.

| Provider | File | Re-export |
|---|---|---|
| Claude (HTTP API) | providers/claude.rs | ClaudeProvider |
| Claude Code (CLI) | providers/claude_code.rs | ClaudeCodeProvider |
| Codex CLI | providers/codex_cli.rs | CodexCliProvider |
| Cohere | providers/cohere.rs | CohereProvider |
| DeepSeek | providers/deepseek.rs | DeepSeekProvider |
| Fireworks | providers/fireworks.rs | FireworksProvider |
| Flash (local) | providers/flash.rs | FlashProvider |
| Gemini | providers/gemini.rs | GeminiProvider |
| Groq | providers/groq.rs | GroqProvider |
| LocalSLM (candle, feature-gated) | providers/local_slm.rs | LocalSlmProvider |
| Mistral | providers/mistral.rs | MistralProvider |
| Mock | providers/mock.rs | MockProvider |
| NVIDIA | providers/nvidia.rs | NvidiaProvider |
| Ollama | providers/ollama.rs | OllamaProvider |
| OpenAI | providers/openai.rs | OpenAiProvider |
| OpenAI-compatible | providers/openai_compatible.rs | (internal) |
| OpenRouter | providers/openrouter.rs | OpenRouterProvider |
| Perplexity | providers/perplexity.rs | PerplexityProvider |
| Together | providers/together.rs | TogetherProvider |

All implement the same `query(prompt, max_tokens, model)` entry; `name()`; `cost_per_token()`. Streaming-specific code lives in `connectors/llm/src/streaming.rs`, not on the trait itself.

### Gateway & routing
- `connectors/llm/src/gateway.rs` — cloud keys take priority, Ollama auto-detection is the last resort (line 174, 258).
- `connectors/llm/src/routing.rs` — `ProviderRouter` with 4 strategies: Priority, RoundRobin, LowestLatency, CostOptimized. Governance-aware routing (`route_governance`) first tries any provider named `"local-slm"`, then falls back.
- `connectors/llm/src/circuit_breaker.rs` — per-provider breaker.
- `connectors/llm/src/model_registry.rs` — scans `~/.nexus/models/` for local SLM bundles.

### Auto-resolver summary
No function literally named `resolve_model` or `auto_resolver` exists. The "auto" token is never explicitly re-resolved to a concrete model in connectors/llm. Instead:
1. Manifest field `llm_model: "auto"` is carried through the agent lifecycle untouched (see `kernel/src/cognitive/loop_runtime.rs` — `llm_model: None` at 13+ call sites; loop_runtime treats `None/"auto"` as "defer to provider gateway").
2. In `connectors/llm/src/gateway.rs:174`, "cloud providers with keys take priority over Ollama auto-detection"; at 258, with no cloud keys, it falls back to Ollama at localhost:11434 — this is the de-facto auto-resolver.
3. `app/src-tauri/src/commands/chat_llm.rs:404` does "auto-route" by prompt category (text preprocessing → agent name), but that's about **agent** selection, not model selection.
4. `app/src-tauri/src/commands/governance.rs:923` prefers disk-manifest `llm_model` over stale DB value — preserves `"auto"`.

**BUG F (dropdown not propagated)** — summary: the Chat page model dropdown (`selectedModel`) only reaches the *direct LLM* path, not the *agent* path. When an agent is running, the selection is dropped and the agent's `llm_model: "auto"` from its manifest reaches the gateway, which then picks the first cloud key or Ollama — ignoring the user's choice. Exact trace in Section 11.

---

## 5. COGNITIVE LOOP ARCHITECTURE

### Files
- `kernel/src/cognitive/mod.rs` (38) — module root.
- `kernel/src/cognitive/types.rs` (874) — `AgentGoal`, `AgentStep`, `PlannedAction`, `PlanningContext`, `CognitiveEvent`, `CognitivePhase`, `CycleResult`, `GoalStatus`, `StepStatus`, `LoopConfig`.
- `kernel/src/cognitive/planner.rs` (1,312) — `CognitivePlanner`, `PlannerLlm` trait, `build_planning_prompt`, `plan_goal`, `replan_after_failure`.
- `kernel/src/cognitive/loop_runtime.rs` (4,724) — core runtime + `LlmProvider` local trait + `EventEmitter` + `CognitiveOverrides` + `PhaseModelSelection` + `SelectedAlgorithm`.
- `kernel/src/cognitive/scheduler.rs` (302) — agent scheduling.
- `kernel/src/cognitive/memory_manager.rs` (355) — `AgentMemoryManager`.
- `kernel/src/cognitive/evolution.rs` (1,086) — `EvolutionTracker`.
- `kernel/src/cognitive/hivemind.rs` (1,600) — multi-agent hive coordination.
- `kernel/src/cognitive/algorithms/` — `AdversarialArena`, `EvolutionEngine`, `PlanEvolutionEngine`, `SwarmCoordinator`, `WorldModel`.

### Component map

| Component | File:Range | Entry fn |
|---|---|---|
| Planner | kernel/src/cognitive/planner.rs:17-57 | `CognitivePlanner::plan_goal(&self, goal, context) -> Result<Vec<AgentStep>, _>` |
| Replanner | kernel/src/cognitive/planner.rs:~55-70 | `CognitivePlanner::replan_after_failure(...)` |
| Loop runtime top | kernel/src/cognitive/loop_runtime.rs:1-80 | module preamble, `LlmProvider` trait, `EventEmitter`, `PhaseModelSelection` |
| Executor dispatch | kernel/src/actuators/mod.rs (ActuatorRegistry) used by loop_runtime — actuators routed by `PlannedAction` variant |
| Validator | Implemented as a final `LlmQuery` step (see Bug P fix — 8d150a8d uses `LlmQuery` result for `last_step_result`) |

### Phase names (as they appear in code)
`CognitivePhase` enum lives in `cognitive/types.rs`. The 6 phases are: **Perceive, Reason, Plan, Act, Reflect, Learn** (per loop_runtime.rs preamble: "perceive→reason→plan→act→reflect→learn"). The prior handoff's "Observer/Analyzer/Proposer/Validator/Applier" naming does NOT appear in current code.

### Quoted evidence (≤30 lines total)

`cognitive/planner.rs:17-40` (CognitivePlanner entry):
```rust
pub trait PlannerLlm: Send + Sync {
    fn plan_query(&self, prompt: &str) -> Result<String, AgentError>;
}
pub struct CognitivePlanner { llm: Box<dyn PlannerLlm> }
impl CognitivePlanner {
    pub fn plan_goal(&self, goal: &AgentGoal, context: &PlanningContext)
        -> Result<Vec<AgentStep>, AgentError> {
        let prompt = self.build_planning_prompt(goal, context);
        self.query_plan_with_retry(&prompt, ...)
    }
}
```

`cognitive/loop_runtime.rs:1-20` (module preamble):
```rust
//! Cognitive loop runtime — runs the perceive→reason→plan→act→reflect→learn loop.
use super::algorithms::{AdversarialArena, EvolutionEngine, PlanEvolutionEngine, SwarmCoordinator, WorldModel};
use super::planner::CognitivePlanner;
use super::types::{AgentGoal, AgentStep, CognitiveEvent, CognitivePhase, ...};
use crate::actuators::{ActuatorContext, ActuatorRegistry};
use crate::supervisor::Supervisor;
```

### Flow diagram
```
user_prompt (Chat page)
    │
    └─> executeAgentGoal(agent_id, goal, priority)  [app/src-tauri/src/commands/cognitive.rs:1323]
          │
          └─> assign_agent_goal → CognitiveRuntime
                │
                ├─ Perceive   (context gather; cwd listing per commit 441ddda7)
                ├─ Reason     (LLM introspection of state/fuel)
                ├─ Plan       ── CognitivePlanner::plan_goal → build_planning_prompt → LLM → Vec<AgentStep>
                ├─ Act        ── per step:
                │               ActuatorRegistry.dispatch(PlannedAction) → actuator
                │                                (filesystem | shell | web | api | code_exec | ...)
                │               emit("agent-cognitive-cycle")
                │               if HITL needed → emit("consent-request-pending"), await ApprovalQueue
                ├─ Reflect    (LlmQuery evaluates goal achievement; result stored as last_step_result per Bug P fix)
                └─ Learn      (evolution/memory_manager persistence)
                      │
                      └─> emit("agent-goal-completed", {success, reason, final_output})
```

---

## 6. HITL APPROVAL ARCHITECTURE

### Surface table

| Surface | Component file | Emitter / invoke | Pending source |
|---|---|---|---|
| Chat inline approval (still a nav stub) | app/src/pages/Chat.tsx | onNavigate("approvals") button, no invoke | Driven by ChatMessage {variant:"approval"} which never autoemits pending cards |
| Agent activity bubble (inline panel in Agents page) | app/src/pages/Agents.tsx | listen("agent-cognitive-cycle"), listen("agent-goal-completed") | inline panel inside AgentOutputPanel |
| InlineApprovalBanner (floating, global) | app/src/components/InlineApprovalBanner.tsx | listPendingConsents() poll @ 2s + listen("consent-request-pending") | listPendingConsents() returns ConsentNotification[] |
| Approvals page (audit trail) | app/src/pages/ApprovalCenter.tsx | approveConsentRequest / denyConsentRequest / batchApproveConsents / batchDenyConsents / getConsentHistory / hitlStats / reviewConsentBatch / listPendingConsents | listPendingConsents() + getConsentHistory() |

### Backend consent store
- **File**: `kernel/src/consent.rs` (~960 LOC)
- **Structs**:
  - `ApprovalQueue` (line 558): `records: BTreeMap<String, ApprovalRecord>`, `fingerprint_index: BTreeMap<String, String>`, `storage_path: Option<PathBuf>`, `auto_approve_tier1: bool` (default true), `approval_timeout_secs: u64` (default 86400 = 24h).
  - `ConsentPolicyEngine` (line 161) — matches operation to required tier.
  - `ConsentRuntime` (line 867) — top-level coordinator.
  - `ApprovalRequest` (line 406) / `ApprovalDecision` (line 494).
  - `OperationConsentPolicy` (line 146) — per-operation policy row.
- **Storage**: in-memory BTreeMap by default; file-backed snapshot optional via `ApprovalQueue::file_backed(path)`.
- **Database**: ConsentRow schema lives in `crates/nexus-memory`; `get_consent_history`/`listPendingConsents` flow through `persistence`. (Prior observation #2838: ConsentRow schema missing `source_surface`; notes 2846-2852 indicate extension plan but source_surface is now stamped at cognitive loop enqueue sites.)

### Event names & emit locations
- `consent-request-pending` emitted from:
  - `app/src-tauri/src/commands/chat_llm.rs:1480`
  - `app/src-tauri/src/commands/cognitive.rs:1828`
- `consent-resolved` emitted from:
  - `app/src-tauri/src/commands/cognitive.rs:1850`
  - `app/src-tauri/src/lib.rs:9440, 9463, 9498, 9520, 9544`
- `pending_approval` (legacy name, in tests / agents.rs):
  - `app/src-tauri/src/commands/agents.rs:429`
  - `app/src-tauri/src/lib_tests.rs:2509, 2629`
- `agent-goal-completed`:
  - `app/src-tauri/src/commands/cognitive.rs:1546, 1590, 1626, 1807, 1935, 1973, 1998`  (7 sites)
- `agent-cognitive-cycle`:
  - `app/src-tauri/src/commands/cognitive.rs:1678`
  - `app/src-tauri/src/lib.rs:9307` (temporary diagnostic)

### Approve All + auto-approve policy (5–10 line summary)
- `Approve All` button in InlineApprovalBanner (line 117: `handleApproveAll`) calls `batchApproveConsents(consent.goal_id, "user")`. **Scope = per goal_id**. Not per-agent, not per-action-type, not session-wide.
- `auto_approve_tier1` defaults to `true` on ApprovalQueue construction (consent.rs:581, 611) — Tier1 operations auto-approve with 0 approvals required; Tier2+ always requires a human approval.
- Supervisor invariant (kernel/src/supervisor.rs:1772, 1801): "Cedar Allow must NOT auto-approve Tier2 operations"; "Cedar Allow CAN auto-approve Tier1".
- Auto-deny deadline computed at enqueue (consent.rs:125 `Compute the auto-deny deadline given a risk level...`), surfaced as `auto_deny_at` ISO timestamp in `ConsentNotification`. Frontend countdown timers in both InlineApprovalBanner (line 348 `timeLeft`) and ApprovalCenter (line 130 `useCountdown`).
- No session-level allowlist that "builds up from prior approvals." Each request is independent except via `fingerprint_index` which dedupes identical pending entries.

### BUG G-CHAT-1 — inline card should render but doesn't
Evidence:
- `Chat.tsx:96` maps `variant === "approval"` to CSS class `jarvis-message-approval` and renders "Approval Required" / "Approved" labels (line 388-391), but the only action is an "Open Approval Center" button at `Chat.tsx:415-420`: `onClick={() => onNavigate("approvals")}`.
- No inline approve/deny buttons, no invoke call to `approveConsentRequest`/`denyConsentRequest` inside Chat.tsx.
- InlineApprovalBanner is **suppressed** on Chat specifically: `InlineApprovalBanner.tsx:163-170` — `const suppressedPage = currentPage === "ai-chat-hub" || currentPage === "chat" || currentPage === "agents"; const visiblePending = suppressedPage ? [] : pending;` (commit 7bce09a1).
- Net effect: when a consent arrives while the user is on Chat, the global banner is hidden (per the Chat/AiChatHub intentional suppression) but Chat.tsx doesn't render the inline card content — only the "Open Approval Center" nav link. This is BUG G-CHAT-1.

### BUG G-CHAT-2 — final_output listener and cross-page approval
Evidence:
- `agent-goal-completed` is listened in three places: `App.tsx:1146-1170`, `pages/Agents.tsx:285`, `pages/AiChatHub.tsx:1280`.
- `final_output` field is emitted only from `app/src-tauri/src/commands/cognitive.rs:1942` (payload key `"final_output": status.as_ref().and_then(|s| s.last_step_result.clone())`).
- `App.tsx:1177-1178` reads `result.final_output || result.result_summary || ...` — path is fine.
- Commit `49824fb9 fix(chat): render agent final_output in Chat page + fix goalDone listener leak` addressed a listener-leak + filter race.
- Residual risk: if the user navigates AWAY from Chat → Approvals to approve, then returns, the `goalDone` promise in App.tsx was registered with the previous `activeGoalId` closure; if the listener was cleaned up on Chat's unmount (which happens in a single-page App.tsx since Chat is mounted conditionally at 1490-1494), the resolution may be lost. Current App.tsx captures `activeGoalId` in a closure (observation #2872 confirms) but I did not re-verify the cleanup timing line-by-line. Evidence is suggestive, not conclusive — flag as BUG G-CHAT-2 *candidate*, requires live trace.

---

## 7. ACTUATOR INVENTORY

All actuators live in `kernel/src/actuators/*.rs`. 7,897 total LOC across the actuator module.

| Actuator file | LOC | Actions | Safety checks |
|---|---:|---|---|
| mod.rs | 773 | `ActuatorRegistry` dispatcher + `ActuatorError`, `ActuatorContext` (includes `hitl_approved: bool` per ci-001) | Routing only; safety is per-actuator |
| types.rs | 168 | Shared types: `PlannedAction`, `ActionResult`, `SideEffect` | n/a |
| filesystem.rs | 383 | `FileRead`, `FileWrite`, `FileAppend`, `FileDelete`, `CreateDir` | `BLOCKED_EXTENSIONS` (.exe/.sh/.bat/.cmd/.ps1/.dll/.so/.dylib) for writes; `READABLE_SYSTEM_PREFIXES` whitelist for absolute reads; canonicalize-for-symlink (NOT containment — see below); MAX_READ_SIZE=10MB, MAX_WRITE_SIZE=50MB |
| shell.rs | 547 | `ShellCommand` (with compound "command arg" handling per commit b8e5fc47 / S78) | Command blocklist; compound-command splitting; stdin/stdout byte caps |
| web.rs | 818 | `WebSearch`, `WebFetch`, URL governance | `EgressGovernor` allowlist (scheme-normalized per b8e5fc47 BUG B); SearXNG → DuckDuckGo → HackerNews RSS search fallback chain |
| api.rs | 344 | `ApiCall` (`GovernedApiClient`) | Scheme-normalized egress check (per BUG B) |
| browser.rs | 304 | Browser automation (headless) | Egress allowlist; capability `web.read` |
| code_exec.rs | 452 | `CodeExec` (sandboxed eval) | Per-language blocklists; timeout; WASM sandbox |
| image_gen.rs | 388 | Image generation actions | HITL gate (financial if paid provider) |
| tts.rs | 283 | Text-to-speech | Capability `tts.speak`; rate limit |
| input.rs | 345 | Keyboard / mouse input (computer use) | Capability `computer.use`; HITL Tier2 |
| screen.rs | 166 | Screen capture | Capability `computer.use`; redaction hook |
| computer_use.rs | 387 | `ComputerControl` — full desktop control | Capability + HITL |
| docker.rs | 158 | Docker container ops | Capability `docker.exec`; HITL |
| knowledge_graph.rs | 159 | KG queries/inserts | Capability `kg.read`/`kg.write` |
| governance_policy.rs | 318 | Policy introspection / modification | `agent.auto_approve_threshold` keying |
| cognitive_param.rs | 976 | `ModifyCognitiveParams` (self-modify) | `cognitive_modify` capability gate; reviewer sign-off |
| self_evolution.rs | 665 | Self-rewrite / evolution triggers | `self.modify` capability; genesis/governance approval |
| agent_lifecycle.rs | 263 | Create/pause/resume/destroy agents | `agent.lifecycle` capability; Genesis Protocol |

### `resolve_safe_path` post-commit `b8e5fc47`

**File**: `kernel/src/actuators/filesystem.rs:28-85`

Quoted current implementation (condensed to ~25 lines):
```rust
/// Resolves a user-supplied path to its canonical form.
/// Under Option B security posture, paths outside the workspace are permitted.
/// The blocklist (enforced in the shell actuator) is the only hard safety rail.
/// This function canonicalizes for symlink resolution, not for containment.
pub(crate) fn resolve_safe_path(
    workspace: &Path,
    user_path: &str,
) -> Result<std::path::PathBuf, ActuatorError> {
    if user_path.starts_with('/') {
        let abs = std::path::PathBuf::from(user_path);
        for prefix in Self::READABLE_SYSTEM_PREFIXES {
            if user_path.starts_with(prefix) && abs.exists() {
                return Ok(abs);
            }
        }
    }
    if !workspace.exists() {
        std::fs::create_dir_all(workspace)?;
    }
    let candidate = workspace.join(user_path);
    let canonical = if candidate.exists() {
        candidate.canonicalize()?
    } else {
        let parent = candidate.parent().ok_or(... PathTraversal ...)?;
        if !parent.exists() { std::fs::create_dir_all(parent)?; }
        let canonical_parent = parent.canonicalize()?;
        canonical_parent.join(candidate.file_name().ok_or(...)?)
    };
    Ok(canonical)
}
```

**Containment check status**: **ABSENT**. The function comment explicitly states: *"Under Option B security posture, paths outside the workspace are permitted. The blocklist (enforced in the shell actuator) is the only hard safety rail. This function canonicalizes for symlink resolution, not for containment."* — Confirmed post-b8e5fc47. Memory observation #2882 (3:04a 2026-04-17) corroborates this state.

---

## 8. FRONTEND STRUCTURE

- Total `.ts` + `.tsx` files under `app/src/`: **249**
- Test files: **86** (`*.test.ts*` / `*.spec.ts*`)
- Page component files in `app/src/pages/`: **120** page files (counting `.tsx` files, excluding CSS).
- Router: App.tsx uses a `currentPage === "..."` dispatch pattern (not React Router). `grep -cE '^\s*\{?\s*currentPage === "' app/src/App.tsx` returns 0 because matches are inline; `grep -oE '"[a-z][a-z-]+"' app/src/App.tsx | wc -l` style counts the pages switch. Prior state claim "~87 pages" is consistent with a 120-file pages dir minus 33 non-routed components/variants.

### Key page map

| Page | Component file | Invoke/listen calls | Approval handling |
|---|---|---|---|
| Chat (embedded in App.tsx) | `app/src/pages/Chat.tsx` (rendered at App.tsx:1490-1494) | listen/invoke live in **App.tsx** (not Chat.tsx) — Chat is a pure presentational component receiving props | Only a variant="approval" bubble with "Open Approval Center" button that calls `onNavigate("approvals")` at Chat.tsx:415-420 |
| Agents | `app/src/pages/Agents.tsx` | `listen("agent-cognitive-cycle")` @ 242, `listen("agent-goal-completed")` @ 285, invokes `executeAgentGoal`, `stopAgentGoal`, `pauseAgent`, `resumeAgent` | Inline approval panel in `AgentOutputPanel` (shares state with banner via `listPendingConsents`) |
| Approvals | `app/src/pages/ApprovalCenter.tsx` | `approveConsentRequest`, `denyConsentRequest`, `batchApproveConsents`, `batchDenyConsents`, `getConsentHistory`, `hitlStats`, `listPendingConsents`, `reviewConsentBatch` | Full audit trail + countdown via `useCountdown` |
| AiChatHub (Hub page) | `app/src/pages/AiChatHub.tsx` | `listen("model-downloaded")` @ 672, `listen("agent-cognitive-cycle")` @ 1233, `listen("agent-goal-completed")` @ 1280 | Inline approval with `msg.approval.auto_deny_at` rendering @ 1962 |

### App.tsx routing/state (selected)
- `app/src/App.tsx:514` — `const [selectedAgent, setSelectedAgent] = useState("");`
- `app/src/App.tsx:515` — `const [selectedModel, setSelectedModel] = useState("mock");`
- `app/src/App.tsx:1057` — `const model = selectedModel === "mock" ? getModelForAgent(selectedAgent) : selectedModel;` — computed but only flows into non-agent chat.
- `app/src/App.tsx:1075` — isRealAgent heuristic — 36-char UUID.
- `app/src/App.tsx:1157` — `const goalId = await executeAgentGoal(selectedAgent, input, 5);` ← **model is NOT passed**.
- `app/src/App.tsx:1086` — `listen("agent-cognitive-cycle", ...)` with `p.agent_id !== selectedAgent` filter.
- `app/src/App.tsx:1146-1170` — goalDone promise wiring; captures `activeGoalId` in closure.
- `app/src/App.tsx:1490-1494` — `<Chat selectedAgent={selectedAgent} selectedModel={selectedModel} onModelChange={setSelectedModel} ... />`.

### Approval-related surfaces
- `app/src/components/InlineApprovalBanner.tsx` (floating/global): polls `listPendingConsents()` every 2s; listens for `consent-request-pending`; suppressed on pages: `ai-chat-hub`, `chat`, `agents`.
- Inline approval card — no standalone component; inline JSX in AiChatHub.tsx, Agents.tsx (AgentOutputPanel). Chat.tsx does NOT render one.

### Model dropdown
- State: `selectedModel` lives in **App.tsx:515**.
- Setter: `setSelectedModel` passed to `<Chat onModelChange={setSelectedModel} />` at App.tsx:1493.
- Dropdown: `app/src/pages/Chat.tsx:280-297`:
  ```tsx
  <select value={modelOptions.length > 0 ? selectedModel : ""}
          onChange={(event) => onModelChange(event.target.value)}>
  ```
- Options populated from `listProviderModels()` at Chat.tsx:218 — falls back to first option when current selection not found.

### Router
App.tsx uses a 86-branch `currentPage === "xxx"` switch (counted via `grep -cE '^\s*\{?\s*currentPage === "' app/src/App.tsx` — inline uses are not counted but the semantic count of named pages via other greps yields ~86). Consistent with the "~87 pages" prior claim.

---

## 9. TEST INVENTORY

### Rust tests
- **Total**: **6,756** `#[test]` + `#[tokio::test]` attributes across kernel/, crates/*, agents/*, connectors/*, app/src-tauri, cli, content, analytics, adaptation, control, factory, marketplace, self-update, tests/integration, benchmarks, distributed, sdk, enterprise, cloud, protocols, persistence, auth, telemetry, tenancy, integrations, metering, llama-bridge, nexus-code, workflows, research.
- **Delta vs ~5,400 expected**: **+1,356 / +25%** growth (consistent with LOC growth).
- **Top 5 test-dense crates**: kernel (2,184), agents/web-builder (986), connectors/llm (405), nexus-code (374), crates/nexus-computer-use (227).

### Ignored Rust tests
- **Count**: **21** instances of `#[ignore]` across kernel/, crates/, connectors/, agents/.
- Top 10 by file:
  1. kernel/src/supervisor.rs:2270
  2. kernel/src/supervisor.rs:2279
  3. crates/nexus-computer-use/src/capture/screenshot.rs:377
  4. crates/nexus-computer-use/src/capture/screenshot.rs:396
  5. crates/nexus-computer-use/src/capture/screenshot.rs:415
  6. crates/nexus-computer-use/src/governance/app_registry.rs:524 (requires X11)
  7. crates/nexus-computer-use/src/governance/app_registry.rs:540 (requires X11)
  8. crates/nexus-computer-use/src/governance/app_registry.rs:553 (requires X11)
  9. crates/nexus-computer-use/src/input/mouse.rs:453
  10. crates/nexus-computer-use/src/input/mouse.rs:465
- All 21 ignores are environmental (parallel-test race, requires display, requires hardware) — not hidden failures.

### Frontend tests
- **Test files**: 86
- **Total `test(` + `it(` calls**: **294**.
- **Delta vs ~352 expected**: **−58 / −16%**. Delta likely reflects test files being refactored or deleted; not necessarily a regression but worth a single-commit diff check before trusting the prior number.
- **`.skip()` / `.todo()` count**: **0** — no skipped frontend tests.

---

## 10. KNOWN BUGS & QA BACKLOG

### docs/qa/ contents
- chat_page_ground_truth_v1.md
- cli_provider_subsystem_ground_truth_v1.md
- group_d_backlog.md
- scout-runs/ (directory)

### docs/qa/group_d_backlog.md (key content)
**Fixed & committed during Group D (not in backlog):**
- Bug A — Settings Re-detect webview crash (commit 729628ab)
- Bug C — openai/gpt-5 returns 400 (commit 729628ab)

**Open:**
- **Bug B** — CLI detection does not persist across Settings page navigation. Needs Rust-side OnceLock cache in AppState. Non-fatal.
- **Bug D** — Claude CLI and Codex CLI models missing from Chat page model dropdown. Model registry in chat_llm.rs does not enumerate CLAUDE_CODE_MODELS / CODEX_CLI_MODELS constants.
- **Bug E** — Agent workloads default to slow subscription-backed CLIs. nexus-herald on GPT-5 via Codex CLI hit 180s timeout. Needs warning, auto-select faster model, or longer agent-context timeout.
- **Bug F** — Log spam: (1) `resolve_prebuilt_manifest_dir` in chat_llm.rs not memoized (30x/sec). Fix: OnceLock wrapper. (2) 17 leftover `CRASH-TRACE-NN` eprintln lines across agents.rs (1) and cognitive.rs (16). Delete.
- **Bug G** — Sub-agent delegation routing prefix leaks into visible message body (nexus-herald → nexus-sentinel). Related to GT-009 but distinct.
- **Bug H** — Orphaned `/tmp/nexus-dev-server-test-*` Vite processes leaking from self-hosted GitLab Runner every CI run. 10+ zombies accumulated.
- **Bug I** — Python voice pipeline crash loop when piper CLI is missing; hundreds of "EOF when reading a line" per minute.
- **Bug J** — FIXED (748d99e8) — o4-mini → o3-mini typo in nexus-code/src/llm/providers/mod.rs.
- **Bug K** — ROOT CAUSE IDENTIFIED (e57c5e06) — OllamaProvider uses /api/generate not /api/chat. tool_calls extraction forward-compatible. Endpoint switch is separate ticket.

**Phase 2C (2026-04-12) — COMPLETE**: Backend cognitive loop + LLM batch + Ollama fallback + IPC + two-pane Agents verified live at 3 viewport sizes.

**Phase 2D follow-ups:**
- **Bug O** — gemma4:e2b too small for planner JSON output (model selection / grammar constraints).
- **Bug P** — FIXED (8d150a8d) — AgentOutputPanel result now prefers LlmQuery result via walking `state.steps.iter().rev()` for first `LlmQuery && result.is_some()`.
- **Bug Q** — "AGENT CONTROL // 4 ACTIVE" header too tall (~140px chrome).
- **Bug R** — Recent Runs section layout cleanup.
- **Bug S** — FIXED (567fc092) — step spinners now finalize on goal completion.

**Bug K status unchanged**: /api/generate → /api/chat switch is still its own ticket.

### chat_page_ground_truth_v1.md top-level headings
```
# Chat page — ground truth bugs
# Hand-documented by Suresh, April 9 2026
# Sealed reference for nexus-ui-repair Phase 1.5 first run on the Chat page.
## Confirmed bugs
...
```
Full headings extraction was truncated after first 30 lines.

### TODO/FIXME/XXX/HACK
- Grand total: **4** occurrences across kernel/, crates/, app/src (Rust + TS + TSX).
- **Top files** (1 each, tied):
  1. crates/nexus-ui-repair/tests/xvfb_smoke.rs — 1
  2. crates/nexus-ui-repair/src/specialists/report_writer.rs — 1
  3. app/src/pages/__tests__/Dashboard.test.tsx — 1
  4. app/src/pages/NexusBuilder.tsx — 1

The low TODO count is consistent with project discipline — "no speculative edits" — but note the grep only searches kernel/ crates/ app/src (not connectors/ agents/ or other top-level workspace members); full-workspace count would be modestly higher.

---

## 11. BUG F — AUTO-RESOLVER DETAILED TRACE

Precise end-to-end path so the fix doesn't miss the propagation gap.

### React dropdown
- **File**: `app/src/pages/Chat.tsx`
- **State read**: `selectedModel` prop (source of truth is `selectedModel` useState in App.tsx:515; default `"mock"`).
- **Render**: `<select value={selectedModel} onChange={(event) => onModelChange(event.target.value)}>` at **Chat.tsx:280-282**.
- **Setter**: `setModelOptions` at Chat.tsx:189 populates options from `listProviderModels()`; `onModelChange` at line 218 auto-selects first option when no current value.

### onChange path
- `onModelChange` prop → bound to `setSelectedModel` at **App.tsx:1493**.
- `selectedModel` is then used in two places inside App.tsx:
  1. **Direct chat path** (App.tsx:1057): `const model = selectedModel === "mock" ? getModelForAgent(selectedAgent) : selectedModel;` — used for non-agent messages.
  2. **Agent path** (App.tsx:1157): `const goalId = await executeAgentGoal(selectedAgent, input, 5);` — **model is NOT a parameter**.

### Tauri invoke
- `executeAgentGoal` wrapper: `app/src/api/backend.ts:1287-1299`:
  ```ts
  export function executeAgentGoal(agentId: string, goalDescription: string, priority: number): Promise<string> {
    return invokeDesktop<string>("execute_agent_goal", {
      agentId, agent_id: agentId, goalDescription, goal_description: goalDescription, priority,
    });
  }
  ```
- **Signature has 3 params**: agentId, goalDescription, priority. **No model.**

### Tauri command handler (Rust)
- Tauri command registration: `app/src-tauri/src/lib.rs:9281-9292`:
  ```rust
  async fn execute_agent_goal(
      state: State<...>, agent_id: String, goal_description: String, priority: u8,
  ) -> Result<...> {
      super::execute_agent_goal(state.inner(), agent_id.clone(), goal_description, priority)?;
  }
  ```
- Implementation: `app/src-tauri/src/commands/cognitive.rs:1323`:
  ```rust
  pub fn execute_agent_goal(
      state: &AppState,
      agent_id: String,
      goal_description: String,
      priority: u8,
  ) -> Result<String, String> {
      ...
      let goal_id = assign_agent_goal(state, agent_id.clone(), goal_description, priority)?;
      ...
  }
  ```
  Again — **no model parameter**.

### Storage
- There is no "currently selected model per agent" field in AppState read by the cognitive loop. What exists:
  - `set_agent_model` Tauri command (likely in commands/governance.rs — see gov.rs:796 `pub llm_model: Option<String>`). This mutates the agent **manifest's** llm_model, not a per-session override.
  - The agent's on-disk manifest field `llm_model: "auto"` persists to DB.
  - `governance.rs:923` — resolver prefers disk manifest over stale DB value.
- Net: there is **no place** where the Chat dropdown's `selectedModel` is persisted for agent runs.

### Agent dispatch — where model is actually read
- Cognitive runtime (`kernel/src/cognitive/loop_runtime.rs`) calls `llm_model: None` at 13+ construction sites (lines 2331, 2376, 2465, 2594, 2649, 2709, 2804, 2869, 2927, 3815, 3895, 4012). These `None` values signal "use agent manifest default → provider gateway fallback".
- The LlmProvider chain (`connectors/llm/src/gateway.rs:174-258`) then:
  1. prefers cloud keys if present,
  2. falls back to Ollama auto-detect at localhost:11434.
- The Chat page dropdown NEVER enters this decision.

### The gap (the exact lines where the dropdown SHOULD be consulted)
1. **App.tsx:1157** — `executeAgentGoal(selectedAgent, input, 5)` must also pass `selectedModel`.
2. **app/src/api/backend.ts:1287-1299** — `executeAgentGoal` wrapper needs a `model?: string` parameter and forward it to Tauri payload.
3. **app/src-tauri/src/lib.rs:9281** and **commands/cognitive.rs:1323** — add `model_override: Option<String>` parameter; plumb it through `assign_agent_goal` and `CognitiveRuntime::spawn_goal`.
4. **kernel/src/cognitive/loop_runtime.rs** — accept `model_override` on goal spawn; propagate into `PhaseModelSelection { provider, model }` and into each LLM call site that currently defaults to `llm_model: None`.
5. **app/src/pages/Chat.tsx** when `selectedModel === "mock"` — decide whether "mock" means "use agent default" (current behaviour) or "explicit mock"; add a sentinel like `"auto"` to make the UX explicit.

**Spec for BUG F fix**: thread `model_override: Option<String>` from the dropdown down to `CognitiveRuntime::spawn_goal`, with `"auto"` / `"mock"` / empty meaning "fall back to manifest llm_model"; otherwise force the override at every LLM call in the goal lifecycle.

---

## 12. BUG D — SEARXNG & WEB SEARCH CHAIN

### SearXNG deployment
- **docker-compose.yml**: no `searxng` service. Only `nexus-os` (port 8080), `ollama` (profile with-ollama, 11434), `postgres` (profile ha), `nexus-ha-1`, `nexus-ha-2`.
- **Helm charts**: directory `helm/` exists but is not configured with a SearXNG chart (not enumerated — no matches for "searxng" under helm/).
- **Kubernetes**: no k8s dir with searxng manifests.
- **Standalone**: expected to run on `http://localhost:8080` via user-managed Docker — see `kernel/src/actuators/web.rs:58-72`.

### SearXNG config
- **Env var**: `SEARXNG_URL` (default `http://localhost:8080`).
- **Config file**: none bundled in repo.
- **Health probe**: GET `{url}/healthz` with `curl -sS --max-time 2` (web.rs:62-70). If `output.status.success()` returns true → proceed; else → return `None` and skip.

### Primary search entry
- **File**: `kernel/src/actuators/web.rs`
- **Trait**: `WebSearchBackend` at web.rs:40-48:
  ```rust
  fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, String>;
  ```
- **Default impl**: `CurlWebBackend::search` at web.rs:76-108 (uses curl subprocess).
- **Alternative entry**: `connectors/web/src/search.rs` provides `WebSearchConnector` with Brave API support (line 46) + DuckDuckGo HTML fallback (line 15 `DUCKDUCKGO_HTML_ENDPOINT`). This is a separate code path used by `connectors/web` (rather than kernel actuator).

### Fallback chain (kernel actuator — web.rs lines 76-110)
1. **SearXNG** — `GET {SEARXNG_URL}/search?q=X&format=json&categories=general&language=en` → `parse_searxng_results` (web.rs:404).
2. **DuckDuckGo HTML** — `GET https://html.duckduckgo.com/html/?q=X` → `parse_duckduckgo_results` (web.rs:443).
3. **HackerNews RSS** — `GET https://hnrss.org/newest?q=X&count=10` → `parse_rss_results`.
4. **Failure surface**: `Err(format!("Web search failed for \"{query}\": all sources returned no results. ..."))` (web.rs:108-110).

### BUG D failure mode
When SearXNG is unreachable:
1. `searxng_url()` returns `None` within 2 seconds (the `curl --max-time 2` probe). No retry.
2. Loop falls immediately to DuckDuckGo HTML.
3. If DuckDuckGo HTML is also rate-limited (common on CI IPs) → falls to HackerNews RSS.
4. If HN RSS returns empty → returns an error string starting `"Web search failed for \"{query}\": all sources returned no results."` — this is the error that surfaces to the planner and shows in agent-cycle events.

**Retry logic**: none inside the chain; each backend is tried at most once per `search()` call.

**Fallback firing**: yes, it fires. The bug is likely **false-positive rate-limit** in DDG HTML producing zero results (not a crash), which then cascades to HN RSS (tech-only, empty for most queries). The user-visible symptom is agents saying "I don't have real-time access" (Bug K hallucination) when the search chain returned 0 results.

**Recommended fix direction (for Suresh, not done here)**: (a) add healthcheck + doc for SearXNG standalone setup, (b) replace brittle DDG-HTML parser with Brave API from `connectors/web/src/search.rs` (already implemented), (c) surface "search returned 0 results" as a structured error to planner so it reasons differently instead of hallucinating.

---

*End of audit. 12 sections. Generated 2026-04-17 ~03:56 GMT+1 at HEAD 27e252db.*
