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
IPC event delivery, two-pane Agents layout. Tested at 3 viewport sizes.

**Bugs fixed this phase:**
- Bug L: agent-goal-completed event was being dropped by mountedRef guard — fixed
- Bug M-1: AgentGoal.description contained manifest text — added user_goal field
- Bug M-2: result_summary echoed description instead of LLM output — added last_step_result
- Bug N: AgentOutputPanel result text had no overflow — added max-height + scroll
- Bug N-2: Agents page buried output panel below fold — refactored to two-pane layout

**Bugs remaining (Phase 2D follow-up):**
- Bug O: gemma4:e2b too small for planner JSON output (model selection / grammar constraints needed)
- Bug P: AgentOutputPanel shows "wrote N bytes" when last step is file_create — should prefer last llm_query step's result
- Bug Q: "AGENT CONTROL // 4 ACTIVE" header block too tall (~140px chrome) — compress
- Bug R: Recent Runs section layout cleanup

**Bug K status unchanged:** OllamaProvider uses /api/generate not /api/chat.
Extraction logic is forward-compatible. Endpoint switch still separate ticket.
