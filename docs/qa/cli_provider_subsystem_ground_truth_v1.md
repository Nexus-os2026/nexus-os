# CLI provider subsystem — ground truth bugs
# Hand-documented by Suresh, April 9 2026
# Sealed reference for the CLI provider subsystem repair cycle.
#
# These bugs are visible FROM the Chat page but they are NOT Chat-specific.
# They are cross-cutting infrastructure bugs in the CLI provider wiring
# that spans: Settings page → provider state store → model enumerator →
# Chat page model selector → request routing layer. These bugs will also
# affect any other Nexus OS page that enumerates LLM providers
# (e.g. Nexus Builder, Nexus Code model config, Agents model config).
#
# This doc is separate from chat_page_ground_truth_v1.md because these
# bugs require subsystem-level repair, not page-level repair. Their scope
# is not limited to the Chat page even though that is where they were
# first observed.

## Confirmed bugs

CP-001: CLI provider detection logic broken
  Where: Settings page → LLM Providers tab → "CLI Providers (Local)"
    section, specifically the "Claude Code (Local CLI)" and "OpenAI
    Codex CLI (Local)" entries
  Symptom: Both CLI provider entries display "Not detected" status
    (orange "enabled but not found" text for Claude Code, plain "Not
    detected" for Codex CLI) even when the binaries are verifiably
    installed on the system. Specifically: the Codex CLI binary exists
    at /home/nexus/.npm-global/bin/codex (verified April 9 2026 during
    Phase 1.4 SG0 stop gate — which codex returned the path and
    codex --help produced full help output). The Re-detect button
    does not appear to improve the detection result.
  Expected: Detection logic should correctly identify the presence of
    installed CLI binaries. When Codex CLI is installed at
    /home/nexus/.npm-global/bin/codex, the Settings page should show
    "Detected" / "Configured" status. Similarly for Claude Code if
    installed.
  Hypothesis: Detection logic probably calls which <binary> or checks
    a hardcoded path that does not include the npm global bin directory
    ~/.npm-global/bin/ in its search path. Fix is to update the
    detection to use the user's actual PATH environment or to explicitly
    check the npm global bin location. The Re-detect button should
    re-run the corrected detection logic.

CP-002: Enabled CLI providers do not appear in Active Model selector
  Where: Chat page → Active Model dropdown (and likely any other model
    selector across Nexus OS — verify on Nexus Builder, Nexus Code,
    Agents page model config if applicable)
  Symptom: After enabling the Codex CLI provider toggle in Settings →
    LLM Providers and clicking Save Settings, the Chat page's Active
    Model dropdown does not include any Codex CLI entries. Filtering
    the dropdown by "gpt" shows an OPENAI section (GPT-4.1 Nano/Mini/
    etc. via OpenAI API) and an OPENROUTER section (various GPT
    variants via OpenRouter), but there is no CODEX CLI section or any
    entry routed through Codex CLI. The dropdown has no awareness of
    CLI provider state — it only enumerates API-based providers.
  Expected: When a CLI provider (Codex CLI or Claude Code) is enabled
    and detected, its available models appear as a distinct section in
    the Active Model dropdown with a section header like "CODEX CLI" or
    "LOCAL CLI". Selecting a model from this section routes subsequent
    chat requests through the CLI subprocess (codex exec), not through
    an API endpoint.
  Hypothesis: The Chat page model enumerator only iterates API-based
    providers. There is no enumeration path that reads CLI provider
    state from the settings store and includes them in the dropdown.
    Fix requires: (a) extending the provider enumeration to include CLI
    providers when enabled and detected, (b) adding a new section in
    the dropdown for CLI-sourced models, (c) wiring the Chat request
    router to actually invoke the codex subprocess when a CLI-sourced
    model is selected (this last part may itself be a separate missing
    piece — see CP-003).

CP-003: Provider backend labeling ambiguous
  Where: Chat page active conversation, model attribution labels
    (e.g. "GPT-4.1 Mini via OpenAI" appearing on each assistant
    response)
  Symptom: When a user has (or would have, once CP-002 is fixed)
    multiple backends that can serve the same model — e.g. GPT-4.1 Mini
    could be served via OpenAI API OR via Codex CLI (using ChatGPT Plus
    subscription) — the current UI labels model responses with only the
    provider name ("via OpenAI") without distinguishing which specific
    backend handled the request. The cost implications are very
    different: OpenAI API = per-token billing, Codex CLI = fixed
    ChatGPT Plus subscription with no per-token cost. A user seeing
    "GPT-4.1 Mini via OpenAI" has no way to know whether they just
    spent $0.00 (Codex CLI) or measurable money (OpenAI API). Quote
    from user testing: "im a bit confused is it codex cli or openai
    api key it is using?"
  Expected: Model attribution labels must be unambiguous about which
    specific backend was used. Design direction per Option X: when both
    OpenAI API and Codex CLI are enabled, they appear as two separate
    entries in the Active Model dropdown (e.g. "GPT-4.1 Mini (OpenAI
    API, 12 fuel)" and "GPT-4.1 Mini (Codex CLI, 0 fuel — ChatGPT
    Plus)"). Once selected, the assistant response label shows the
    specific backend, e.g. "GPT-4.1 Mini via Codex CLI" not just "via
    OpenAI". This aligns with the user's stated preference for explicit
    governance over automatic routing ("Never route Max plan or $150
    Claude.ai credits through nx autonomously — ToS violation, account
    ban risk").
  Hypothesis: The response labeling code uses the provider category
    (OpenAI, Anthropic, Ollama) as the display label rather than the
    specific backend identifier. Fix requires: (a) the model selector
    to treat "same model via different backend" as two distinct
    selectable entries (Option X resolution), (b) the response
    attribution to use the full backend identifier not just the
    provider category, (c) potentially a cost indicator in the
    attribution label so users can see at a glance whether a response
    cost them anything.
