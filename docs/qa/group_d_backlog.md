# Group D Backlog — Side Issues Out of Scope

Bugs discovered during Chat page (Phase 1.5 Group D) diagnosis that
are NOT confirmed_miss GT tickets. These do not block Group D and
are not folded into GT fix prompts without explicit approval.

Fixed and committed during Group D (not in this backlog):
- Bug A — Settings Re-detect webview crash (commit 729628ab)
- Bug C — openai/gpt-5 returns 400 from OpenAI HTTP (commit 729628ab)

## Open

- Bug B — CLI detection does not persist across Settings page navigation.
  Needs Rust-side OnceLock cache in AppState. Non-fatal since Bug A
  fix made re-detect safe.

- Bug D — Claude CLI and Codex CLI models missing from Chat page model
  dropdown. Model registry function in chat_llm.rs does not enumerate
  CLAUDE_CODE_MODELS or CODEX_CLI_MODELS constants into the dropdown.

- Bug E — Agent workloads default to slow subscription-backed CLIs.
  nexus-herald on GPT-5 via Codex CLI hit 180s timeout because the
  agent needs many fast turns and Codex CLI is 3–30s per call. Needs
  either a warning, auto-select of faster model, or longer timeout
  in agent contexts.

- Bug F — Log spam. Two sources: (1) resolve_prebuilt_manifest_dir in
  chat_llm.rs is not memoized, prints 30x/sec during agent ops; fix
  is OnceLock wrapper. (2) 17 leftover CRASH-TRACE-NN eprintln lines
  across agents.rs (1 site) and cognitive.rs (16 sites) from an
  earlier crash investigation. Safe to delete.

- Bug G — Sub-agent delegation routing prefix leaks into visible
  message body (nexus-herald → nexus-sentinel case). Related to GT-009
  but distinct — explicit L3 agent delegation should still show a
  delegation trace somewhere, just not inline in the message body.

- Bug H — Orphaned /tmp/nexus-dev-server-test-* Vite processes leaking
  from the self-hosted GitLab Runner on every CI run. 10+ zombies
  accumulated. CI/runner cleanup bug.

- Bug I — Python voice pipeline crash loop when piper CLI is missing.
  journalctl showed hundreds of "EOF when reading a line" + "piper
  CLI not found" per minute. Python process burning CPU.

- Bug J — **FIXED** (748d99e8) — o4-mini corrected to o3-mini in nexus-code.
  Root cause: model ID typo `o4-mini` in nexus-code/src/llm/providers/mod.rs;
  OpenAI API returned 400 because o4-mini does not exist.
  Originally reproduced April 2026 on All Agents mode, direct send via Chat page.
  Same class as Bug C (GPT-5 400) which was fixed by rerouting to
  Codex CLI. o3 likely needs either (a) Codex CLI rerouting in
  chat_llm.rs provider selection, or (b) the OpenAI HTTP provider
  needs reasoning-model param shape: max_completion_tokens instead of
  max_tokens, no temperature field, no top_p. Not a GT ticket.

- Bug K — **ROOT CAUSE IDENTIFIED** (e57c5e06) — OllamaProvider uses /api/generate
  not /api/chat; extraction forward-compatible; endpoint switch is separate ticket.
  tool_calls extraction added across all providers but Ollama's /api/generate
  endpoint does not support tool calls. Fix requires switching to /api/chat with
  messages[] array format and tools[] parameter in the request body.
  Originally reproduced April 2026. Agent shows "Running" with capabilities
  web.search, web.read, fs.read, fs.write. User prompt: "what is the
  latest ai news today?". LLM responds with generic "I'm an LLM, I
  don't have real-time access" hallucination. Zero tool calls attempted.
  Two hypotheses: (1) gemma4:e4b too weak for tool-use — 4B-class
  models routinely ignore tool schemas; reliable tool calling needs
  8B+ local or frontier cloud. (2) Executor not injecting tool schema
  into Ollama request on small-model path. Diagnosis needs nexus-herald
  execution trace via Logs button. Related to but distinct from Bug E.
  Together Bug E + Bug K mean agent runtime has no viable default model
  on 62GB RAM + RTX 3070. Flag as Phase 1 blocker candidate once Group
  D closes. Not a GT ticket.

### Phase 2B close-out notes (2026-04-12)
- Bug J (o3-mini): CLOSED — fixed in 748d99e8
- Bug K (nexus-herald gemma4 no tool calls): ROOT CAUSE UPDATED
  - Was: missing tool_call extraction in ollama.rs
  - Is: OllamaProvider::query() uses /api/generate not /api/chat
  - Extraction is forward-compatible (e57c5e06)
  - Fix requires switching to /api/chat with messages[] + tools[] — separate ticket
- LLM batch landed: smart default model detection, tool_calls across 7/8 providers, Ollama fallback preserved

### Phase 2C Live Runtime Audit (2026-04-12) — COMPLETE

**Verified live:** Backend cognitive loop, LLM batch integration, Ollama fallback,
IPC event delivery, two-pane Agents layout. Tested at 3 viewport sizes. Further polish issues (Bugs P, S) surfaced during post-commit testing; logged for Phase 2D.

**Bugs fixed this phase:**
- Bug L: agent-goal-completed event was being dropped by mountedRef guard — fixed
- Bug M-1: AgentGoal.description contained manifest text — added user_goal field
- Bug M-2: result_summary echoed description instead of LLM output — added last_step_result
- Bug N: AgentOutputPanel result text had no overflow — added max-height + scroll
- Bug N-2: Agents page buried output panel below fold — refactored to two-pane layout

**Bugs remaining (Phase 2D follow-up):**
- Bug O: gemma4:e2b too small for planner JSON output (model selection / grammar constraints needed)
- Bug P: AgentOutputPanel result prefers last step regardless of action type. When the last step is `file_create`, users see "wrote N bytes to /path/strategy_draft.txt" instead of the actual LLM-generated content. Fix: in `get_agent_status()` (crates/nexus-kernel/src/cognitive/loop_runtime.rs), change `last_step_result` to walk `state.steps.iter().rev()` and return the first step matching `action.type == "LlmQuery" && result.is_some()`. Fall back to current behaviour (any completed step with result) only if no LlmQuery step exists.
- Bug Q: "AGENT CONTROL // 4 ACTIVE" header block too tall (~140px chrome) — compress
- Bug R: Recent Runs section layout cleanup
- Bug S: Step list in AgentOutputPanel shows spinning loader icons on each step row even after the goal completes. The goal-completed handler sets `goalRunning: false` but doesn't flip individual `StepDetail.status` values from "running" to "succeeded". Either (a) update all step statuses to "succeeded" on goal-completed, (b) hide the step list below the RESULT block once the goal is complete, or (c) have the agent-cycle event stream the final step status before goal-completed fires. Option (a) is the cheapest fix — a single setGoalStepDetails() call in the agent-goal-completed listener in Agents.tsx.

**Bug K status unchanged:** OllamaProvider uses /api/generate not /api/chat.
Extraction logic is forward-compatible. Endpoint switch still separate ticket.

---

## Track B — Phase 1.5 / 4b governance + swarm bugs (2026-04)

> Note on bug-letter namespace collision: the Phase 2C / 2D close-out
> sections above use letters L/M/N/O/P/Q/R/S for *different* bugs. The
> letters in this section are **Track B** (governance + swarm
> infrastructure) — a separate per-phase scope. Commit messages use
> "Bug L" / "Bug M" etc. without the Track prefix because the commit
> SHA disambiguates. When in doubt, look up the SHA.

### Closed (with SHAs)

- Bug L — **CLOSED** (`f11e404d`) — Stale Haiku model id in nexus-code.
  `claude-haiku-4-20250514` → `claude-haiku-4-5-20251001`. Single-line
  string fix; no semantic change.

- Bug M — **CLOSED** (`ad14c8c3`) — Governance ruleset hot-swap not
  reaching the live DecisionEngine. AppState's `Arc<Mutex<>>` was a
  swap surface that nothing propagated from. Engine now reads from
  `Arc<RwLock<GovernanceRuleset>>` shared with AppState; new
  `update_governance_ruleset(&self, new)` writes through the lock.
  Engine API change is additive (`with_shared_ruleset` constructor
  alongside existing `new`). +4 tests, +1 unit + 3 integration. No
  evolution producer wired yet (see Bug Z).

- Bug N — **CLOSED** (`bdb29cba`) — Versioned identity file format for
  `~/.nexus/oracle_identity.key`. Added 4-byte NXOK magic + version
  byte + 65-byte payload (V1, 70 bytes total). V0 → V1
  auto-migration on first start; future-version refuses to start;
  corrupt files refuse to start. Atomic write via tempfile + fsync +
  chmod + rename. +4 integration tests including real-startup
  migration test. Tempfile name keys on pid + uuid to prevent
  parallel-test rename races.

- Bug O — **CLOSED** (`dc8063e6`) — Session-scoped swarm caller
  identity. swarm_plan was generating a fresh `CryptoIdentity` per
  request. New file `~/.nexus/swarm_caller_identity.key` (NXSC magic,
  V1 format, 70 bytes) loaded at AppState init, exposed via
  `swarm_caller_identity()`. Architecturally separate from oracle
  identity (oracle = system trust root; caller = user/session
  identity, prepares for future multi-user). +6 integration tests
  including 65-byte oracle-V0-lookalike rejection. Atomic write
  duplicated from Bug N — see Bug AA.

- Bug X — **CLOSED** (this commit) — `target/` size hygiene. New
  `scripts/cleanup-target.sh` with --incremental | --aggressive |
  --report-only modes. ci-local.sh emits non-blocking WARNING when
  `target/` exceeds `NEXUS_TARGET_SIZE_LIMIT_GB` (default 200GB).
  Disk hygiene policy documented in CLAUDE.md.

Track B status as of Bug X: L/M/N/O/X all closed. **Track B complete.**

### Open

- Bug V — **PHASE 5 DEPENDENCY** — Real Twitter publish wiring in
  `social-poster-agent` SwarmEntry. Currently `dry_run: false` returns
  `publish_status: "deferred"` because `WebAgentContext` (governance/
  fuel) and Twitter API credentials aren't threaded into
  `AgentExecutionContext`. Phase 5 wires them.

- Bug W — **PHASE 5 DEPENDENCY** — Per-channel post-count surface
  missing from swarm context. `compliance::check_compliance(channel,
  recent_posts: 0)` is hard-coded to 0 in social-poster's swarm_entry.
  Compliance gate under-fires until the post-count surface is
  plumbed.

- Bug Y — **TEST HYGIENE** — `nexus-desktop-backend` lib_tests path
  (`OracleRuntime::start(test_ruleset)` in lib.rs:1756) creates a
  real `Persistent` runtime against `~/.nexus/oracle_identity.key`
  during `cargo test`. Every test run touches the production user's
  actual oracle identity file. Pre-dates Bug N; Bug N's atomic-write
  refactor just made the prior race more visible. Should switch to
  `IdentityMode::Ephemeral` or an explicit TempDir.

- Bug Z — **EVOLUTION PRODUCER** — `update_governance_ruleset` has no
  in-tree producer. The plumbing exists (Bug M); the trigger source
  doesn't. Future phase tracks where evolution decides to swap (likely
  after a Darwin-Core attack-arena verdict) and how it formats the new
  `GovernanceRuleset`.

- Bug AA — **WRITER DUPLICATION** — Atomic-write helper duplicated
  between `app/src-tauri/src/oracle_runtime.rs` (Bug N) and
  `app/src-tauri/src/swarm_caller_identity.rs` (Bug O). Tempfile +
  fsync + chmod + rename logic is byte-identical. Extract once a
  third consumer arrives or a writer-side fix needs to land in two
  places. Per Bug O preflight: readers diverge semantically (oracle
  accepts V0; swarm rejects 65-byte) so reader extraction is not on
  the table.
