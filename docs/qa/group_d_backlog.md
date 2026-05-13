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

- Bug AE — **CLOSED** (this commit) — Typed
  `AgentError::PublishFailed { reason, retryable, retry_after_secs }`
  and `SwarmError::PublishFailed { agent, reason, retryable,
  retry_after_secs }` shipped. `agents/social-poster/src/swarm_entry.rs`
  re-routes `PublishStatus::RateLimited`, `Failed`, and `AuthFailure`
  into the new `Err` variant; herald adapter maps cleanly through
  `map_agent_error`. Audit emission (`publish_complete` phase event)
  preserved across the new `Err` paths. V2 retry loop tracked as Bug
  BG (depends on AE + AF).

- Bug AF — **CLOSED** (this commit) — Persistent
  `IdempotencyManager` via internal storage swap shipped per
  `docs/architecture/decisions/0003_persistent_idempotency_store.md`.
  New `idempotency_cache` SQLite table + `lookup_idempotency` /
  `record_idempotency` helpers in nexus-persistence. New
  `IdempotencyManager::with_db(ttl, db)` ctor preserves the existing
  `::new` API; HashMap is now a fast cache, SQLite the durable
  source-of-truth on miss. `TwitterConnector::with_db` and
  `post_status_update_idempotent(…, request_id)` close Bug V's
  duplicate-tweet-on-retry gap. The 4 pre-AF consumers
  (facebook.rs, instagram.rs, sequential.rs, http_connector.rs)
  remain on `::new` because their construction sites lack
  `Arc<NexusDatabase>`; tracked as Bug BJ.

- Bug AG — **CLOSED** (this commit) — Subsumed by Bug AE — typed
  `SwarmError::PublishFailed` carries the retryable hint AG asked
  for.

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

- Bug AL — **CLOSED** (this commit) — Persistent swarm audit tail.
  Resolved per `docs/architecture/decisions/0002_swarm_audit_persistence.md`.
  New `swarm_audit_log` SQLite table on `nexus-persistence` (id,
  run_id, seq, event_kind, ticket_nonce, node_id, timestamp_secs,
  timestamp_nanos, payload_summary, previous_hash, current_hash,
  created_at) plus index on (run_id, seq). Helpers
  `record_swarm_audit`, `query_swarm_audit_by_run`,
  `last_swarm_audit_hash_for_run`, `verify_swarm_audit_chain`,
  and the genesis-hash constant exposed as
  `SWARM_AUDIT_GENESIS_HASH`. Per-row hash chain via SHA-256 over
  `(run_id|seq|event_kind|ticket_nonce|node_id|timestamp_secs|timestamp_nanos|payload_summary|previous_hash)`.
  The prior in-memory `Arc<Mutex<HashMap<Uuid, Vec<AuditEntry>>>>`
  on `SwarmInner` is dropped; the forwarder pulls AppState's
  `Arc<NexusDatabase>` and chains rows on every broadcast.
  `swarm_audit_tail` Tauri command grows optional `limit` /
  `offset` args (defaults 1000 / 0); existing zero-arg-other-than-
  run_id callers keep working. Wire shape `AuditEntry` gains
  `previous_hash` and `current_hash` strings. Audit-write failures
  log via eprintln but do NOT crash the swarm runtime.

  +5 persistence tests (round-trip, chain genesis, tamper detect,
  pagination, migration idempotency); existing 4 swarm tests
  unchanged; existing 11 SwarmAudit vitest specs unchanged
  (mocks updated to include the two new hash fields).
  Filed Bug BA (retention/prune), Bug BC (hash-inspection UI),
  Bug BD (`swarm_list_runs` + run-history dropdown).

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

- Bug BA — **SWARM_AUDIT_LOG UNBOUNDED GROWTH** — Bug AL ships
  the `swarm_audit_log` SQLite table without retention or pruning
  (matches existing precedent — no audit-flavoured table in
  `persistence/src/lib.rs::migrate()` has TTL or vacuum logic).
  Realistic scale is dozens to a few hundred provider-touching
  events per run; long-lived deployments will accumulate. Add
  retention when storage exceeds ~10MB or row count exceeds
  ~100k. Suggested approach: a periodic `DELETE FROM
  swarm_audit_log WHERE created_at < ?` with the cutoff driven
  by config (`audit_retention_days`). Out of scope for v1.

- Bug BC — **HASH-CHAIN INSPECTION UI** — Bug AL stores
  `previous_hash` and `current_hash` per row; the SwarmAudit page
  receives them on the wire but does not render or verify them
  today. v2 affordances: (i) a small chain-integrity badge
  ("verified" / "broken at row N") via a new
  `swarm_audit_verify` Tauri command wrapping
  `verify_swarm_audit_chain`; (ii) click-to-show the SHA-256 hex
  in the expand-to-JSON row body; (iii) a "copy chain" export.
  Storage and verification primitives are already in place.

- Bug BD — **swarm_list_runs + RUN-HISTORY DROPDOWN** — Bug AL
  persists rows but the SwarmAudit page's run-id picker is still
  paste-only with active-run default. After persistence, paste-
  any-historical-run-id works, but discoverability is poor.
  Add a `swarm_list_runs(limit) -> Vec<RunSummary>` Tauri command
  (`{ run_id, started_at, event_count }`, derived via `SELECT
  DISTINCT run_id, MIN(created_at), COUNT(*) FROM swarm_audit_log
  GROUP BY run_id ORDER BY MIN(created_at) DESC LIMIT ?`) and a
  dropdown of recent runs alongside the paste input on
  `SwarmAudit.tsx`. UX iteration; deserves its own preflight.

- **BE** — `swarm_audit` forwarder logs write failures via eprintln only. In production builds where stderr is not captured, audit-write failures are silent. Add a metric/counter (e.g. `swarm_audit_write_failures_total`) that the SwarmAudit page or a future health endpoint can surface. Structural chain integrity survives missed writes (subsequent rows chain off last successful row), but a user depending on completeness needs visibility.

- **BG** — Twitter swarm-path `Arc<NexusDatabase>` threading. AF wired Twitter at the connector layer but deferred swarm-path threading. BG closes that deferral: thread `Arc<NexusDatabase>` through `HeraldAdapter` → `SocialPosterEntry` → `RealPublishExecutor` → `TwitterConnector::with_db`; swap publish path from `post_status_update` to `post_status_update_idempotent` with a per-publish UUID. Production gains persistent idempotency dedup with no behavior change visible to the swarm broadcast. Originally filed as "V2 retry loop" — preflight surfaced two orthogonal pieces: this threading work (BG) and the actual retry loop (split out to BK). BG is the threading piece only; BK consumes BG's idempotency wiring. Depends on: AE (typed errors, shipped), AF (persistent IdempotencyStore, shipped). Blocks: BK.

- **BH** — Unified retention/prune helper. Both `swarm_audit_log` (Bug BA) and `idempotency_cache` (this commit's lazy eviction) are candidates for a shared retention layer. Today both use append-only-with-ad-hoc-cleanup; a `RetentionPolicy` trait + a single sweep helper would centralize TTL logic. Defer until storage growth empirically justifies it.

- **BJ** — Upgrade IdempotencyManager consumers to `with_db`. The following consumers stay on `::new` after Bug AF because their construction sites lack `Arc<NexusDatabase>`: `connectors/social/src/facebook.rs:31`, `connectors/social/src/instagram.rs:32`, `workflows/src/sequential.rs:56`, `connectors/core/src/http_connector.rs:57`. Threading the DB handle through requires multi-crate dependency changes (each `::new()` is the public ctor; callers in `connectors/social/src/lib.rs`, `connectors/core/src/github_connector.rs:28`, `workflows/src/sequential.rs:54-55`, and `tests/integration/tests/full_pipeline.rs:120` likewise lack DB). Address as part of broader DI refactor or alongside whichever crate's next major work.

- **BI** — `.gitignore` hygiene for Claude session machinery. Bug AF's commit added targeted entries (`.claude/scheduled_tasks.lock`, `.claude/tasks/`, `.claude/settings.local.json`) rather than a bare `.claude/` because the existing skill carve-outs (`!.claude/skills/claude-mem/` etc.) cannot survive an excluded parent. If future Claude session artifacts surface that aren't covered, append targeted entries; do not collapse to a bare directory rule.

- **BK** — **CLOSED (resolved)** — commits `4cabed46` (BK.1 ADR 0005 publish_retry_loop), `8ad2b9bc` (BK.2 retry decorator + impl), `c02eca5f` (BK.3 retry_attempt NodeEvent emission). ADR: `docs/architecture/decisions/0005_publish_retry_loop.md` with amendments 1–4. Tests: 8 unit tests in `agents/social-poster` plus 3 integration scenarios (BL.3a/BL.3b). Original entry: V2 retry loop with backoff and observability. Depends on BG. PublishExecutor decorator consuming `SwarmError::PublishFailed { retryable, retry_after_secs }`, exponential backoff with jitter (200ms × 2.0^n, capped at 60s, ±20%), `retry_after_secs` respected with 300s ceiling, `max_attempts=3` by default. Emits `NodeEvent("retry_attempt", { attempt_num, wait_secs, last_error_summary })` on each attempt start. ADR 0005 (publish_retry_loop) published as part of the BK commit. ~250–400 LOC plus ADR.

- **BL** — **CLOSED (resolved)** — commits `276dc67e` (BL.1 publish-path primitives), `be497f4f` (BL.2 `with_publish_capability` builder method), `36af4551` (BL.3a scenario E retryable→success), `50a80e5a` (BL.3b scenarios F + G idempotency replay + non-retryable). Coverage: ADR 0005's E/F/G test placeholders satisfied at the swarm-event layer, complementing BK.2/BK.3 unit-level coverage. Per-scenario wall-clock: E=1.07s, F=1.07s, G=0.07s. Original entry: Phase B harness scenarios E/F/G for retry coverage. Depends on BK. Adds `ScriptedCapability` primitive (~50 LOC), `with_idempotency_db` builder method (~40 LOC), and three scenarios: E (retryable failure → retry → success), F (idempotency hit → short-circuit), G (non-retryable failure → no retry). ~350 LOC total. Locks retry behavior with regression-proof tests.

- **BM** — **CLOSED (not-needed at HEAD)** — commit `83d8075b` (ADR 0005 Amendment 5). Finding: TwitterConnector has no connector-layer retry to disable. The "3 × 3 = 9 attempts" compounding described in the original BM filing was a faulty premise — the `connectors/core` `RetryPolicy` struct exists but `TwitterConnector` does not consume it. The swarm-layer `RetryingPublishExecutor` is the sole retry surface on the publish path. Re-open trigger: a future change that adds a framework `Connector` impl to `TwitterConnector` or adopts the `connectors/core` `RetryPolicy` abstraction on the publish path. Original entry: Disable connector RetryPolicy on publish path. Depends on BK. Connector's existing `RetryPolicy{3, 200ms, 2.0×}` retains for transport-level transient failures; publish-path retries owned by the BK executor decorator. Prevents double-retry compounding to 9 attempts. ~10 LOC config edit. Single commit.

- **BQ** — Tauri swarm command surface uses module-level OnceLock with its own NexusDatabase connection rather than AppState's db. Documented at `swarm.rs:111-113`; WAL-mode tolerates the duplicate connection but it's a redundant handle on the same file. Resolution shape: refactor ~10 Tauri command signatures to take `state: tauri::State<AppState>` and reach the shared db. Out of BG scope (BG would not reorganize AppState construction). Filed because BG's UNI #3 chose the smaller-diff path; this captures the leftover work. Depends on: nothing structural.

- **BN-RETRY-CLASSIFY-MIGRATION** — Migrate `PublishExecutor::publish` return type to `swarm_core::AgentError`. Filed 2026-05-07 during BK.2. Severity: Low (current string-parsing classifier works correctly; migration would be cleaner architecture). Context: `PublishExecutor::publish_with_request_id` returns `Result<TweetResult, KernelAgentError>` (kernel `AgentError`, not `swarm_core::AgentError`). Kernel `AgentError` has no `PublishFailed` variant; the BK retry decorator's `classify_for_retry` matches on `KernelAgentError::SupervisorError(msg)` and routes through the existing `classify_publish_error` helper (string-parses the message for "rate limited", etc.) to determine retryability. `swarm_core::AgentError` has a typed `PublishFailed { reason, retryable, retry_after_secs }` variant that would let the decorator match directly on `retryable: true` instead of parsing strings. Resolution: change `PublishExecutor::publish` and `PublishExecutor::publish_with_request_id` to return `Result<TweetResult, swarm_core::AgentError>`; migrate the classifier in `agents/social-poster/src/retry.rs::classify_for_retry` to match on `AgentError::PublishFailed { retryable: true, .. }` directly. `RealPublishExecutor` and `StubExecutor` impls update; the existing `classify_publish_error` helper can be retired or moved to the producer site (where `SocialPosterEntry::execute` constructs the typed variant from `PublishStatus` today). Reference: ADR 0005 Amendment 1 documents the current string-parsing approach and the deferred migration.

- **BO-NODEREF-RELOCATE** — Move `NodeRef` from `nexus-swarm` to `nexus-swarm-core`. Filed 2026-05-08 during BK.3. Severity: Low (no current production impact; enables future cleaner decorator-style observability). Context: `NodeRef` is currently defined in `crates/nexus-swarm/src/events.rs:25-30`. `agents/social-poster` declares only `nexus-swarm-core` as a dep (`Cargo.toml:22`), not `nexus-swarm`, because `nexus-swarm` depends on `social-poster-agent` (`Cargo.toml:24`) — a cycle would form if the agent imported the parent crate. BK.3 worked around this by routing `NodeEvent` emission through `EventEmitter::emit_phase` rather than letting the decorator construct `NodeEvent` variants directly. The `EventEmitter` impl (`CoordinatorEmitter` or `RecordingEmitter`) handles `NodeRef` construction internally. Future agent-side decorators that want to emit structured events without the `emit_phase` indirection would face the same cycle. Resolution: move `NodeRef` declaration from `crates/nexus-swarm/src/events.rs:25-30` to `nexus-swarm-core` (it's a 2-field POD: `run_id` + `node_id`). Re-export from `nexus-swarm` so existing callers compile unchanged. Touches all `NodeRef` construction sites: `crates/nexus-swarm/src/events.rs:153` (test), `crates/nexus-swarm/src/emitter.rs:44`, `crates/nexus-swarm/src/coordinator.rs:133, 245, 253, 295, 323, 409, 418`, `app/src-tauri/src/commands/swarm.rs:542, 667`. Mechanical re-export move. No structural dependencies.

- **BP-BENCH-CI** — `benchmarks/phase67_bench.rs` has zero ci-local test coverage; the wasmtime API path is only exercised by `cargo bench`, which is not part of any ci-local job. If wasmtime API changes ever break only the bench path (as could have happened during AK-13's wasmtime 42 → 43.0.2 bump), ci-local will not catch it. Resolution shape: add a minimal `#[test]`-marked smoke that compiles and runs one bench scenario end-to-end, OR add an explicit `cargo bench --no-run` invocation to a ci-local job (cheaper, no runtime cost). Surfaced during AK-13 closure. Priority: low (hygiene), but blast-radius grows with every wasmtime-touching ticket.

- **AK-FUTURE-WASMTIME-44** — Wasmtime 44.0.1 is available; AK-13 deliberately stopped at 43.0.2 per minimum-delta spec. Future bump rides the `[workspace.dependencies]` pin pattern established in AK-13: one-line edit in workspace `Cargo.toml`, then `cargo update -p wasmtime` and re-run ci-local. Trigger: 30+ days of 44.x upstream stability OR a new advisory targeting 43.x. No current advisory pressure (RUSTSEC-2026-0114 cleared by AK-13 commit 918c8d9c). Priority: informational; do not act preemptively.

- **BR-INPUT-RACE-REFACTOR** — `kernel/src/actuators/input.rs` rate limiter uses a process-global `static` `input_timestamps()` (`Mutex<VecDeque<u64>>`), creating shared-state races between concurrent tests that mutate or read the queue. Surface fix shipped this commit: `#[serial_test::serial]` markers on all affected tests, matching AK-8's prescribed Phase 1.5 pattern. Root fix: refactor the rate limiter to be context-bound (per-actuator-instance, or owned by `ActuatorContext`), eliminating the process-global anti-pattern entirely. Removes the need for test serialization and improves multi-tenant isolation (relevant when AK-14 per-OS-user keyring isolation lands). Failure surfaced on pipeline #2522852881 (commit 27fb8bd0). Diagnosis: pass/fail/pass pattern across 918c8d9c/27fb8bd0/05b03165 on the same GitLab runner; race triggered only under cgroup scheduling pressure. Priority: medium (architectural cleanup; not blocking). Related: AK-8 (same class, env-var variant).

### Phase 1.5 (must complete before Commit 5)

These items were filed during Bug AK Phase 1 and MUST land before Commit 5 of the AK series (Tauri command surface) so that the unified `vault_*` commands have well-defined semantics and a parallel-safe test suite.

- **AK-7** — `set_secret()` write semantics not yet documented at the trait level. Today the round-trip test passes only because the `OsKeyring` stub returns `BackendNotConfigured` and writes fall through to sqlite. Once Commit 4 lands the real keyring impl, `set()` write order is undefined. Required before Commit 5: document the write contract on the `SecretBackend` trait ("set writes to the first backend that returns `Ok`; backends earlier in the chain that return `BackendReadOnly` or `BackendNotConfigured` are skipped"); add a unit test asserting set-then-get round-trips through the highest-precedence writable backend; add a test asserting `set` never double-writes to multiple backends.

- **AK-8** — Env-mutation test races. `resolve_log_seen_dedups_per_provider` flaked under `cargo test` parallelism because another test mutated `ANTHROPIC_API_KEY` in the same process. Fix in Commit 1 was local (rewrote the dedup test to use `MockKeyring` instead of env). Required before Commit 5: ripgrep the `nexus-kernel` test suite for `std::env::set_var` and `std::env::remove_var`; serialize all such tests via a test mutex (`serial_test` crate or hand-rolled `Mutex<()>`); document the convention in the `kernel/src/secrets/tests.rs` module header so future contributors don't reintroduce the race.

  AK-8 addendum (filed Commit 2 close): When AK-8's serial-test infrastructure lands, add an end-to-end test that exercises `kernel::secrets::global::install` via `kernel::startup::run_migrations` on a fresh process-equivalent harness. Commit 2's `run_migrations` test deliberately bypasses `install` to avoid `OnceLock` poisoning during parallel test runs; the install path itself is currently uncovered by integration tests.

- **AK-9** — Commit 1 commit message must explicitly disclose that `OsKeyring` is a stub returning `BackendNotConfigured` in this commit; the real keyring backend lands in Commit 4 alongside the `keyring` v3 dep approval. Note that this stub is safe for Commits 1–3 because no consumer requires keyring resolution at runtime in those commits (`SocialConfig` and `http_connector` both resolve via sqlite or env). Surface this in the eventual Commit 1 commit message draft.

- **AK-11** — Migrate `NexusConfig.llm.providers: Vec<LlmProviderEntry>` per-entry `api_key` fields to the `SecretsFacade`. Each entry has a user-defined `id` (e.g. `"my-openai-1"`) and its own `api_key: String`. Variable count, dynamic content. Requires a hierarchical scope/name scheme — candidate: vault scope `"llm.providers"`, name = `entry.id` — and runtime hydration on read since IDs are not statically known. Out of Phase 1 scope (Bug AK Commit 3 covers the six static `LlmConfig.<provider>_api_key` fields only). Touches: `kernel/src/config.rs::LlmProviderEntry`, the Tauri provider-list commands that read and write the providers vector, `connectors/llm/src/gateway.rs` per-entry resolution path. Depends on: nothing structural — current `SecretsFacade::set_secret` / `get_secret` already accepts arbitrary `(scope, name)` pairs.

- **AK-12** — Mid-phase migration extension gotcha. Bug AK Commit 3 chose `v1-extended` for `schema_versions[credential_vault_v1]` so the SocialConfig (Commit 2) and LLM (Commit 3) clearings share one Phase 1 atomic gate. On installs that ran Commit 2 before Commit 3 deployed, `schema_version = 1` is already set, the migration is skipped on next boot, and the LLM fields stay in `~/.nexus/config.toml` until the operator manually deletes the `schema_versions` row (recovery operation, not a startup path). Production impact at HEAD is zero — Commits 2 and 3 ship in the same Phase 1 cutover and no install runs Commit 2 alone. File a forward-only "extend Phase 1" helper if a future commit adds further fields to `collect_phase1_fields` after a Phase 1 migration has already shipped to operators. Candidate API: `db.bump_schema_extension(v1, ext_id)` with per-extension idempotency rows. Defer until empirically needed.

- **AK-13** — **CLOSED** (shipped 918c8d9c). Wasmtime bumped 42 to 43.0.2 via `cargo update -p wasmtime`, clearing RUSTSEC-2026-0114. `[workspace.dependencies]` pin established for `wasmtime` in workspace `Cargo.toml`; future advisory bumps are single-line edits. Cranelift transitive bump 0.129 to 0.130 (source-compatible across the major). `cargo check --workspace --all-targets` clean, ci-local 7/7, `cargo audit` confirms RUSTSEC-2026-0114 absent. Wasmtime 44.0.1 available; deferred per minimum-delta spec (future ticket). Original entry: Bump Wasmtime past `RUSTSEC-2026-0114`. April 2026 Wasmtime advisory batch flagged via transitive dep. Wasmtime typically patches its advisories promptly. Identify which workspace dep pulls Wasmtime, find the minimum upgrade that clears the advisory, verify no breakage. The `deny.toml` ignore expires `2026-05-30`. If unresolved by expiry, `cargo-deny` re-fails and forces re-prioritization. Not blocking Phase 1.

- **AK-14** — Per-OS-user keyring isolation. `KEYRING_USER` is hardcoded to `"nexus"` in `kernel/src/secrets/backend_keyring.rs`. Multi-user installs on the same host share keyring entries. Not a problem at HEAD (single-operator install). Becomes one in shared-host deployments. Resolution shape: derive the keyring user from the OS user identity at facade construction time; scope service strings accordingly. Depends on: nothing structural.

- **AK-15** — **CLOSED** (this commit). Audit wiring on `SecretsFacade` shipped. `kernel/src/secrets/mod.rs` appends a hash-chained event for every `get_secret` / `set_secret` / `delete_secret` / `list_secrets` call via `nexus_kernel::audit::AuditTrail`. Payload shape locked: `{event, scope, name?, result, capability="log_only", resolved_from?}`. Plaintext values never enter the JSON; `agent_id` is `Uuid::nil()` until AK-2 lands the capability ledger. Regression test `ak15_audit_records_ops_without_plaintext_leak` reinstates the deleted `vault.rs` invariant. ADR 0004 §Audit corrected to reflect in-memory + retention persistence (the ADR-0002 reference was an overclaim; `swarm_audit_log` is a different per-run audit). AK-16 filed for SQLite-backed kernel audit persistence follow-up.

- **AK-16** — SQLite-backed kernel audit persistence. AK-15 wired `SecretsFacade` to the kernel in-memory `AuditTrail` with retention archival. ADR 0002's `swarm_audit_log` is shape-incompatible (per-run partitioned). A kernel-wide `kernel_audit_events` SQLite table with hash-chain integrity validation and a query API would persist credential and other kernel-level events across restarts. Filed because ADR 0004 §Audit originally claimed persistence; AK-15's correction documents the gap. Not blocking; design + ADR + implementation warrant their own commit series.

- **AK-17** — `AuditTrail` Mutex contention under high concurrent secret access. AK-15 wires `Arc<Mutex<AuditTrail>>`; every facade op takes the lock briefly for `append_event`. Today's sequential agent flows don't contend. Bug CA's brain-bypass loop (many agents fanning out simultaneously) may expose contention. Resolution shape: channel-based audit pipeline (mpsc to a dedicated audit-writer task) decouples facade hot path from append. Not blocking Phase 1; file before CA work begins. Trigger: any benchmark or production observation showing >1ms median latency on `facade.get_secret` under load.

## Phase B follow-ups

- **PB-1** — `tests/integration/src/full_agent_flow.rs` lib-test failure independent of Phase B. `cargo test -p nexus-integration` (no filter) fails on `full_agent_flow::test_full_agent_flow` even with B.1 changes stashed. Per-binary runs all pass (17/17 across the integration test binaries). Suspected: environment-path bug in `full_agent_flow.rs`'s `HOME_ENV_GUARD` interaction with new test ordering — the agent writes `test.txt` to `$HOME/.nexus/agents/test.txt` but the assertion expects the workspace-prefixed path `$HOME/.nexus/agents/<agent-id>/workspace/test.txt`. Triage in a separate commit; do not bundle into Phase B.

- **PB-2** — Coordinator does not consume budget. `SwarmCoordinator::execute_loop` never calls `budget.try_consume`. `BudgetUpdate` events emit constant `tokens_remaining` every iteration. This invalidates policies that depend on budget approaching its limit (e.g. `BudgetSoftLimitApproach` high-risk category). Either consumption is missing or `BudgetUpdate` is misnamed. Architectural; needs investigation before any policy that depends on budget arithmetic ships. Discovered by Phase B.2 scenario investigation.

- **PB-3** — `SwarmCompleted` event carries no summary payload. At HEAD the variant is `SwarmCompleted { run_id }`. Subscribers wanting completed / failed / cancelled counts must replay the full broadcast stream. Mid-stream subscribers lose prior events and cannot reconstruct from the terminal event alone. Resolution shape: either (a) add `summary: SwarmSummary` to `SwarmCompleted`, or (b) emit a sibling `SwarmFinalized { run_id, summary }` event after `SwarmCompleted`. Out of Phase B scope but worth filing — Phase B's harness derives summary metrics from event counts as a workaround.

- **PB-4** — `SwarmCompleted` emitted on coordinator-level error path. When the coordinator detects plan drift (or any error in `execute_loop`), it returns `Err` but the spawn block sets `cancelled=false` and emits `SwarmCompleted` as the terminal event. `NodeFailed("(coordinator)")` fires first with the actual reason, but `SwarmCompleted` as the trailing event suggests success to subscribers reading the broadcast as a state machine. Resolution shape: emit `SwarmCancelled` (`cancelled=true`) on `Err` paths, OR introduce a `SwarmAborted { reason }` variant for coordinator-level failure. Architectural; affects every consumer of the broadcast. Discovered by Phase B.3 scenario D investigation.

- **PB-5** — `drain_events_until_terminal` terminal set may miss coordinator-level `NodeFailed`. The harness drains until `SwarmCompleted | SwarmCancelled`. On coordinator error today, both `NodeFailed("(coordinator)")` and `SwarmCompleted` fire in the same task before timeout, so the drain works. Latent flake: if task scheduling shifts, the drain could time out waiting for `SwarmCompleted` while `NodeFailed` is the actual terminal. Resolution depends on PB-4: if PB-4 introduces a new terminal variant for coordinator error, the terminal set updates then. Otherwise, add `NodeFailed("(coordinator)")` to the terminal set explicitly.
