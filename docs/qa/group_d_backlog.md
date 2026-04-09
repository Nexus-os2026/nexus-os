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

- Bug J — o3 Mini returns 400 from OpenAI HTTP provider in Chat page.
  Reproduced April 2026 on All Agents mode, direct send via Chat page.
  Error surface: "supervisor error: openai request failed with status 400".
  Same class as Bug C (GPT-5 400) which was fixed by rerouting to
  Codex CLI. o3 likely needs either (a) Codex CLI rerouting in
  chat_llm.rs provider selection, or (b) the OpenAI HTTP provider
  needs reasoning-model param shape: max_completion_tokens instead of
  max_tokens, no temperature field, no top_p. Not a GT ticket.

- Bug K — nexus-herald L3 agent with gemma4:e4b produces no tool calls.
  Reproduced April 2026. Agent shows "Running" with capabilities
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
