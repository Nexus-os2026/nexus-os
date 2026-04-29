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

- Bug X — **CLOSED** (`da542c79`) — `target/` size hygiene. New
  `scripts/cleanup-target.sh` with --incremental | --aggressive |
  --report-only modes. ci-local.sh emits non-blocking WARNING when
  `target/` exceeds `NEXUS_TARGET_SIZE_LIMIT_GB` (default 200GB).
  Disk hygiene policy documented in CLAUDE.md.

- Architecture Q1 (Broker fate) — **DECIDED** (this commit) —
  Resolved per `docs/architecture/decisions/0001_broker_fate.md`.
  Status quo + doc fix on `BrokerAdapter` (it stays as a
  coordination-themed LLM capability, registered in production via
  the swarm registry; the misleading "wraps nexus-collaboration"
  doc comments in `crates/nexus-swarm/src/adapters/broker.rs` and
  `mod.rs` are corrected to match reality). The `agents/collaboration/`
  crate is **deleted as inert code** — 895 LOC, 22 unit tests, zero
  production callers. The `agents/conductor/` and
  `tests/integration/` Cargo.toml deps are removed; the
  `delegation_and_collaboration` integration test is renamed to
  `delegation_lifecycle` and the GovernedChannel/AgentMessage block
  is dropped (delegation half preserved). Orchestrator-style Broker
  is **deferred** until Phase 4c materializes — design patterns
  worth reimplementing (Blackboard with AccessLevel, GovernedChannel
  with send/receive governance gates, Task/SubTask lifecycle) are
  preserved in the ADR for future reference.

- Track C #3 (commit 2 of 2) — **CLOSED** (`8d5c5b4a`) — Frontend
  MediaRecorder + mic button on DirectorConsole. Closes Track C #3
  and Track C as a whole.

  - New `app/src/voice/wav_encoder.ts` — pure functions:
    `downsampleFloat32` (linear interpolation, identity below
    target rate), `floatTo16BitPCM` (clamping s16le conversion),
    `encodeWav` (44-byte RIFF/fmt/data header), and
    `encodeFloat32ChunksToWav` composing the full pipeline.
  - `app/src/voice/PushToTalk.ts` — desktop branch wired:
    AudioContext + ScriptProcessor capture mirroring
    `VoiceAssistant.tsx:534` precedent. Float32 chunks collected
    in `onaudioprocess`, downsampled to 16kHz mono, wrapped in
    WAV, handed to `transcribePushToTalk(audioBytes,
    16000)`. The "not yet wired" throw at line 80 is gone. The
    mock-mode SpeechRecognition path is preserved for browser
    dev. Permission errors mapped explicitly:
    `NotAllowedError → "Microphone permission denied"`,
    `NotFoundError → "No microphone detected"`,
    `NotReadableError → "Microphone is in use by another
    application"`. Setup-time rejection (start-fail) is buffered
    on the recorder and surfaced via `stopAndTranscribe`'s await
    chain — single consumer-facing await; no leaked unhandled
    rejection.
  - `app/src/components/swarm/DirectorConsole.tsx` — mic button
    inline before the Submit button. Pre-flight
    `voicePipelineHealth()` on mount; mic stays disabled with
    `title=` tooltip until the probe lands and reflects health
    afterward. Recording state: red `var(--nexus-danger)` pill
    with the existing `swarm-node-pulse` keyframe; respects
    `prefers-reduced-motion`. Click → start; second click or
    30s auto-cap → stop + transcribe. Captured transcript
    appended to the textarea with the documented separator
    semantics via the new `appendTranscript` pure helper
    (exported for unit testing). Cursor lands at the new end via
    `setSelectionRange` queued through `queueMicrotask`. Errors
    routed through the existing `setError(...)` span. The 30s
    cap is owned by DirectorConsole (single React-state owner),
    not by the recorder — surfaces "Recording auto-stopped at
    30s" while still consuming whatever was captured.
  - `app/tsconfig.json` — added `src/lib/swarm/__tests__` and
    `src/voice/__tests__` to the existing test-dir exclude list
    (matches precedent for sibling test dirs).
  - +11 wav_encoder tests + 6 PushToTalk desktop tests + 9 new
    DirectorConsole tests (5 mic + 4 appendTranscript helper) =
    +26 vitest specs. Test infra (MediaDevices + AudioContext
    stubs) is local to each test file — no changes to global
    `app/src/test/setup.ts`. Existing DirectorConsole tests
    untouched in behaviour; existing PushToTalk consumer in
    `App.tsx` still compiles (the `void recorder.startRecording()`
    + `await recorder.stopAndTranscribe()` shape is unchanged).

- Track C #3 (commit 1 of 2) — **CLOSED** (`03277bba`) — Backend
  wiring for local STT. `voice/stt.py` gains a JSON-over-stdout CLI
  (`--health` and `transcribe <wav-path>`) consumed by the Tauri
  subprocess bridge. The legacy `transcribe_push_to_talk` model-load
  probe is replaced with a real transcribe path: validates 16kHz
  mono PCM, writes the bytes to a tempfile via
  `std::env::temp_dir() + Uuid`, spawns `python3 stt.py transcribe`
  from `/voice/`, parses the result JSON, and returns the typed
  `TranscribeResult { text, language, confidence, latency_ms,
  model }`. New `voice_pipeline_health` Tauri command emits the
  reachable/model/last_error tuple and mirrors it onto
  `VoiceRuntimeState.pipeline_health`. Audit invariant: every
  attempt — success OR failure — appends a hash-chained
  `EventType::ToolCall` row with payload
  `{action, transcript_sha256, audio_duration_ms, model,
  latency_ms}`. Transcript text is NEVER persisted; only the SHA-256
  of the text reaches the audit trail. Privacy invariant: local-only
  path; no cloud-STT fallback. Pipeline-down is surfaced loudly.
  +6 Python pytest specs (4 pass, 2 skip on machines without
  faster-whisper) + 4 Rust unit tests on validation, audit
  emission, and health. Frontend mic button lands in commit 2;
  this commit is unblocked-on-merge but not user-visible. Track C
  #3 itself remains open until commit 2 ships.

- Track C #2 — **CLOSED** (`f9155fa3`) — Cross-page swarm status
  indicator. New `app/src/components/swarm/SwarmStatusBadge.tsx`
  (floating top-right pill, fixed-position). Subscribes to
  `selectActiveRun`, `selectIsPlanPending`, and the new
  `selectRunProgress` factory selector for `{ done, total, running,
  failed }`. Direct `swarmBus` subscription captures
  `swarm_completed` / `swarm_cancelled` events to hold a 5-second
  terminal-state pill ("COMPLETED ✓" / "CANCELLED") before the badge
  hides — the store dispatcher clears `activeRun` synchronously on
  these events, so the hold has to be local state. Suppressed on
  `page === "agents"` (where richer chrome already exists). Reduced-
  motion respected via `window.matchMedia('(prefers-reduced-motion:
  reduce)')` — pulse animation skipped when set. Reuses the
  `nexus-topbar-chip` class for visual consistency. The
  `swarm-node-pulse` keyframe was promoted from
  `AgentNode.tsx`'s inline injection into
  `app/src/styles/nexus-design-system.css` so both components reuse
  the same CSS-defined animation. Mounted in `App.tsx` as a sibling
  of `<Sidebar>` and the main column inside `.nexus-shell`. Click →
  `setPage("agents")`. +1 selector + 4 selector tests + 9 component
  tests. Pill-only in v1; per-node hover/popover filed as Bug AN.

- Track C #1 — **CLOSED** (`2e984379`) — Swarm Audit Viewer page.
  New `app/src/pages/SwarmAudit.tsx` (read-only viewer for
  `swarm_audit_tail`), single-column layout, run-id picker
  (active-run default + paste override), filter chips for
  `event_kind`, node_id substring filter, client-side time-range
  filter (1m / 5m / 15m / 1h / All), expand-to-JSON rows.
  Rust-side: `AuditEntry` extended with `node_id: Option<String>`;
  per-variant extraction in `event_to_audit_entry`
  (`app/src-tauri/src/commands/swarm.rs`). TS-side: typed
  `AuditEventKind` union replaces the prior bare `string`; new
  `audit_kind_category.ts` mapper reuses the EventTape pill
  palette. Routing: new `swarm-audit` Page slug, NAV entry under
  MONITORING, lazy import + switch arm in `App.tsx`. +4 Rust unit
  tests on `event_to_audit_entry`; +15 vitest specs (4 on
  `auditKindCategory` exhaustiveness; 7 on the page component;
  4 on the pure `filterEntries` helper). Persistence + live-append
  filed as Bug AL / AM (out of v1 scope).

- Bug V — **CLOSED** (`fdfcd132`) — Real Twitter publish wiring in
  `social-poster-agent`'s swarm path. New `PublishExecutor` async
  trait (`credentials_present` + `publish`); production
  `RealPublishExecutor` wraps `TwitterConnector::post_status_update`
  in `tokio::task::spawn_blocking`. `WebAgentContext` is constructed
  per-call (not carried on the entry, per locked decision #2).
  `PublishStatus` extended with `Published { post_id, url }`,
  `RateLimited { retry_after_secs }`, `AuthFailure`,
  `CredentialsMissing`, `Failed { reason }`. `record_publish` trait
  amended to accept `post_id: Option<String>`; new `post_id TEXT`
  column on `social_publish_log` (idempotent ALTER). Cred check
  short-circuits at `CredentialsMissing` instead of falling through
  to the connector's mock mode. `record_publish` runs only on
  confirmed `Published` (Bug AH catalogues the partial-failure hole).
  Phase emit set: dry_run / blocked = 6 phases (W's terminal
  "publishing" emission dropped); credentials_missing = 7;
  published / publish-error = 9 (adds checking_credentials,
  publishing, publish_complete). +13 unit tests on the entry, +2
  on the in-memory state (post_id round-trip), +2 SQLite
  integration tests (post_id round-trip + migration idempotency).
  Existing W tests touched: 2 lib tests updated for V's superseded
  Deferred contract; 1 nexus-swarm cross-crate test updated for
  same; 1 phase-sequence assertion updated (W's 7-phase →
  V's 6-phase dry-run shape).

- Bug W — **CLOSED** (`6922787c`) — Per-channel post-count surface
  for Herald's compliance gate. New `social_publish_log` SQLite table
  in `nexus-persistence` (platform, account_id, published_at,
  content_hash, indexed on the composite key). New
  `social-poster-agent::publish_state::PublishStateHandle` async
  trait with `InMemoryPublishState` (Mutex<HashMap>) and
  `SqlitePublishState` (Arc<NexusDatabase>) impls. `SocialPosterEntry`
  now holds `Arc<dyn PublishStateHandle>`; the hard-coded
  `check_compliance(_, 0)` is replaced with a real read on
  `(platform, account_id)` keyed by new `ChannelKey`. 24h trailing
  window (see Bug AC). `dry_run` reads count, does NOT increment;
  `record_publish` is wired but only V's real publish path will call
  it. HeraldAdapter takes the handle in its constructor; production
  wiring opens its own `NexusDatabase::open(default_db_path())` from
  the swarm `OnceLock` initializer (the static path has no AppState
  reference). +4 unit tests on the entry, +5 integration tests on the
  SQLite impl, +5 trait-level tests on the in-memory impl.

Track B status as of Bug V: L/M/N/O/X/W/V all closed. **Phase 5
complete.** **Track C complete** — items 1 (Swarm Audit Viewer),
2 (cross-page swarm status indicator), and 3 (DirectorConsole mic
input) all closed. Architecture Q1 (Broker) reopens next. Bug
AB/AC/AD (filed against W), Bug AE/AF/AG/AH/AI/AJ/AK (filed against
V), Bug AL/AM (Track C #1), Bug AN/AO (Track C #2), Bug AP/AS
(Track C #3 commit 1), and Bug AT/AU/AV/AW (Track C #3 commit 2)
remain open as backlog.

### Open

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

- Bug AB — **LEGACY PATH** — `SocialPosterAgent::run` in
  `agents/social-poster/src/lib.rs` (the non-swarm execution path)
  still calls `check_compliance(platform, slot as usize)` with a
  loop-index slot, not a real post count. Bug W only rewired the
  swarm path through `SocialPosterEntry`; the legacy path remains
  on its synthetic counter. Either retire `SocialPosterAgent::run`
  in favour of the swarm path or thread the same
  `PublishStateHandle` through it. Filed during Bug W to keep the
  W diff scoped.

- Bug AC — **HARDCODED WINDOW** — `COMPLIANCE_WINDOW: Duration =
  24h` is a `const` in `agents/social-poster/src/swarm_entry.rs`.
  Platform-specific limits (X = 3-hour rolling, IG = daily, FB =
  daily) are encoded inside `nexus_content::compliance` but the
  *window over which we count* is single-valued in W. Pull the
  window from `manifest.toml` per platform once V wires real
  publishing — until V exists, a single trailing-24h window is the
  conservative read for all three platforms.

- Bug AD — **DUPLICATE DB HANDLE** — Bug W's swarm `OnceLock`
  initializer in `app/src-tauri/src/commands/swarm.rs::state()`
  opens its own `NexusDatabase::open(default_db_path())` because the
  static path has no `AppState` reference. AppState already holds an
  `Arc<NexusDatabase>` against the same file; SQLite WAL keeps the
  two connections coherent, but it's wasteful and creates two
  independent connection pools. Thread AppState (or just its
  `Arc<NexusDatabase>`) into the swarm initializer so the same
  handle is reused. Filed during Bug W as a v1 → v2 follow-up.

- Bug AE — **TYPED PUBLISH ERROR** — V maps publish failures into
  `SwarmError::AgentInternal { agent, detail }` with a string-shaped
  detail (locked decision #7); coordinator retries / dashboards have
  to parse strings to recover the failure shape. Add
  `SwarmError::PublishFailed { agent, reason, retryable }` (mirrored
  on swarm-core's `AgentError` so `map_agent_error` routes it
  cleanly) so the coordinator can act on rate-limit / auth /
  transport without parsing. Scope: `nexus-swarm` and
  `agents/social-poster` only.

- Bug AF — **SQLITE-BACKED IDEMPOTENCY** — V relies on Twitter's
  own ~1h duplicate-status detection as the cross-restart dedupe
  backstop (locked decision #3). The in-memory
  `nexus-connectors-core` `IdempotencyManager` would solve in-process
  retries but loses state on restart. Build a `IdempotencyStore` trait
  with a SQLite impl (mirroring W's `PublishStateHandle` shape) so
  cross-restart dedupe doesn't depend on Twitter's behavior. Wire V's
  publish path through it before any callers actually retry.

- Bug AG — **RETRYABLE HINT LOST IN STRINGIFICATION** — V's flat
  error mapping (`PublishStatus::Failed { reason }`) loses the
  retryable signal that exists in the connector's underlying error
  flavour. Subsumed by Bug AE if `PublishFailed { ... retryable }`
  lands; otherwise add a parallel `retryable: bool` field on the
  `Failed` variant. Today the swarm UI cannot distinguish "5xx —
  try again" from "content rejected — never retry" without parsing
  the reason string.

- Bug AH — **RESERVE→CONFIRM ATOMICITY ON record_publish** — V
  fires `record_publish` only after a confirmed publish success
  (locked decision #5). Two partial-failure modes still
  under-count: (1) Twitter accepts the post but the connector's
  response parse fails — V's branch sees an error and skips
  `record_publish` even though the post landed; (2) `record_publish`
  itself fails after the publish lands — V logs and returns
  `Published`, leaving the audit row missing. Both windows are
  rare. Closing them needs a reserve→confirm pattern: write a
  pending row before publish, confirm it post-success, expire it on
  failure. Out of scope for V; tracked here for V+1.

- Bug AI — **REMOVE DEAD `PublishStatus::Deferred`** — V supersedes
  the Deferred variant. It's retained one revision so any external
  serializers / dashboards keyed on `"deferred"` get a deprecation
  window (locked decision #6). After one rev of V dogfooding, drop
  the variant + its label.

- Bug AJ — **HARDCODED PUBLISH FUEL BUDGET** —
  `PUBLISH_FUEL_BUDGET = 50` is a `const` on
  `RealPublishExecutor` in
  `agents/social-poster/src/swarm_entry.rs`. The connector charges
  10 fuel per `post_status_update` so 50 = a 5x ceiling — but
  there is no governance path. Should derive from `ctx.budget` (so
  swarm-level fuel governs the per-call ceiling) or be made
  configurable via `agents/social-poster/manifest.toml`. Drifts
  silently if the connector's per-call cost ever changes. Filed
  during Bug V as a v1 follow-up.

- Bug AK — **CREDENTIAL VAULT MIGRATION** — Twitter OAuth1 keys
  (and future social creds) are stored as plaintext strings in
  `kernel/src/config.rs::SocialConfig` and read at connector
  construction by
  `connectors/web/src/twitter.rs::load_twitter_credentials`. Move
  them to the encrypted vault scaffold at
  `connectors/core/src/vault.rs` so secrets are not on disk in
  plaintext. Touches `kernel/src/config.rs` (drop the four `x_*`
  fields or transitionally point them at the vault),
  `connectors/web/src/twitter.rs::load_twitter_credentials` (read
  from vault instead of TOML), and `app/src/pages/Settings.tsx`
  (frontend save path now writes to the vault, not the config
  file). Originally bundled with Bug AE's typed error work in V's
  draft; split out so the security migration is independently
  schedulable.

- Bug AL — **SWARM AUDIT TAIL PERSISTENCE** — The audit tail
  surfaced by `swarm_audit_tail` is held in
  `Arc<Mutex<HashMap<Uuid, Vec<AuditEntry>>>>`
  (`app/src-tauri/src/commands/swarm.rs:68`) — pure in-memory, lost
  on desktop restart. The Swarm Audit Viewer (Track C #1) works
  for the life of one process; users who close the app expecting
  the tail to be there will be surprised. Migrate to a
  SQLite-backed store mirroring W's `social_publish_log` pattern
  (new table on `nexus-persistence`, typed helpers, async
  read/write through `PublishStateHandle`-style trait). Required
  to make Track C #1's UX hold up across restarts.

- Bug AM — **SWARM AUDIT VIEWER LIVE-APPEND** — Track C #1's page
  fetches `swarm_audit_tail` once on mount and on manual refresh.
  It does NOT subscribe to `swarm:event` and project new audit
  entries into the visible list. After a run completes, users
  must click Refresh to see the final audit rows. Add an
  optional live-append path: subscribe to the event stream while
  the page is mounted, project incoming `SwarmEvent`s through the
  same `event_to_audit_entry` shape (or a frontend twin), and
  merge by `seq`. v1 chose one-shot for snapshot semantics;
  live-append is the v2 freshness affordance. Also re-evaluate
  whether `swarm_audit_tail` needs a backend `audit-update` event
  for clean cross-tab consistency.

- Bug AN — **SWARM STATUS BADGE EXPANSION-ON-HOVER** — Track C #2
  ships a pill-only indicator: it shows aggregate state ("RUNNING ·
  3/7") and click navigates to `/agents`. A v2 affordance is a
  hover/click expansion popover that surfaces per-node mini-status
  (running list, failed list, recent oracle denials) without
  forcing the user off their current page. Builds on the existing
  `selectActiveNodes` (frozen-sentinel cached, currently running
  only) plus a small popover positioning helper. Pill behaviour and
  click-to-jump unchanged.

- Bug AO — **BADGE PULSE COLOR DRIFT** — `SwarmStatusBadge`
  ring-pulse uses `AgentNode`'s legacy `#38bdf8` (sky-blue) while
  the chip foreground/border use `var(--nexus-accent)` `#4af7d3`
  (mint cyan). Two cyan tones reading as a color bug at small
  sizes. Reconcile to a single token before broader design polish.

- Bug AP — **DEAD WEBSOCKET VOICE BRIDGE** —
  `services/voice/nexus_voice/voice_server.py` exposes a websocket
  bridge on `127.0.0.1:9876` (`voice_server.py:6–12`) that is
  **never started by any Tauri command** (repo-wide grep on
  `nexus_voice` against `app/src-tauri/src/` returns zero). The
  active subprocess bridge in Track C #3 commit 1 targets `/voice/`
  exclusively. `services/voice/voice_engine.py:18` is also explicit
  that its transcription is stubbed. Delete `services/voice/`
  entirely in a separate cleanup commit; `requirements.txt` there
  pins `websockets >= 12.0` for nothing. Out of scope for Track C
  #3 — file as standalone hygiene.

- **AS** — voice/tests/test_real_backends.py::test_synthesize_to_wav_executes_command fails when run in the full suite. Pre-existing breakage in TTS (piper) synthesis testing, unrelated to STT or Track C #3. Pinned for triage; not blocking voice/STT work.

- Bug AT — **macOS NSMicrophoneUsageDescription** — Track C #3
  commit 2 ships frontend mic capture via
  `navigator.mediaDevices.getUserMedia`. Linux/X11 (Suresh's dev
  environment) doesn't enforce mic permission at the OS level, so
  the prompt is a no-op. macOS REQUIRES an
  `NSMicrophoneUsageDescription` string in the Tauri-generated
  `Info.plist` for `getUserMedia` to even surface a permission
  prompt — without it, the call fails silently. Add the bundle
  config in `app/src-tauri/tauri.conf.json` (Tauri 2 has a
  `bundle.macOS.entitlements` / `infoPlist` slot) before any
  macOS bundle ships. Out of scope today; file for the next
  cross-platform pass.

- Bug AU — **MIGRATE ScriptProcessor → AudioWorklet** — Track C
  #3 commit 2 captures audio via
  `audioContext.createScriptProcessor(4096, 1, 1)` matching the
  `VoiceAssistant.tsx:534` precedent. ScriptProcessor is
  deprecated; the modern equivalent is AudioWorklet (worklet
  file, separate worker context, message-port for sample
  delivery). Works on WRY/webkitgtk today but should be hardened
  before browser engines drop ScriptProcessor entirely. Track C
  #3 commit 2 will need migrating; `VoiceAssistant.tsx`'s
  legacy capture path too.

- Bug AV — **INSERT-AT-CURSOR FOR TRANSCRIPT** — Track C #3 commit
  2 ships append-at-end semantics: the transcribed text always
  lands at the end of the textarea regardless of where the user's
  cursor was pre-recording. v2 should respect `selectionStart` /
  `selectionEnd` and insert at cursor (or replace selection if
  one exists). The `appendTranscript` helper would be
  generalised to `insertTranscript(currentText, cursor,
  transcript)`. Low priority; v1 ships append-only.

- Bug AW — **BASE64 audio_bytes WIRE FORMAT** — Track C #3 commit
  1 sends `audio_bytes: Vec<u8>` as a JSON array of integers
  (Tauri 2 default serialization). At 16kHz mono s16le that's
  ~32KB/s PCM, ~6–8MB JSON for a 30s recording. The 30s cap
  keeps us under the Tauri 2 default ~8MB IPC ceiling, but the
  encoding bloat is wasteful. If recording duration ever extends
  (or the user-flow demands continuous capture), switch to a
  base64 string field on the wire. Tauri 2's binary IPC is in
  beta; revisit once stable.

- **AX** — `tests/integration/tests/full_agent_flow.rs::test_full_agent_flow` fails at HEAD due to file-path expectation drift in the actuator output. Test expects `~/.nexus/agents/<uuid>/workspace/test.txt`; actuator writes to `~/.nexus/agents/test.txt`. Pre-existing failure verified via git stash test, NOT introduced by Architecture Q1 commit. ci-local 7/7 still passes (this specific test path apparently not exercised by rust-tests-full). Triage and fix as standalone follow-up.

- **AY** — `app/src/pages/__tests__/TokenEconomy.test.tsx::renders heading after load` uses the same `/Token Economy/i` regex pattern that matches the loading state's "Loading token economy..." text. Test currently passes (only asserts presence, not interaction), but the assertion is satisfied by the loading-state DOM rather than the loaded DOM — so the test name is a partial lie. Tighten the regex or change the assertion to a post-load-only target. Not blocking; the test is green.

- **AZ** — Codebase-wide audit for tests using `/<heading>/i` regex patterns where the loading state text contains the heading substring (e.g. "Loading X..."). The TokenEconomy fix in this commit addresses one instance; similar latent races may exist in other page tests. Sweep `app/src/pages/__tests__/` and `app/src/components/**/__tests__/` for the pattern.
