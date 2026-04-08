# Chat page — ground truth bugs
# Hand-documented by Suresh, April 9 2026
# Sealed reference for nexus-ui-repair Phase 1.5 first run on the Chat page.
# Do not edit after the scout's first run on Chat — this file is the
# tiebreaker for false-positive / false-negative measurement.
#
# Chat page has three sub-views (tabs): Chat, Compare, History.
# This doc catalogues Chat-specific bugs only. CLI provider subsystem
# bugs (which are visible from Chat but are cross-cutting infrastructure
# issues) are catalogued separately in cli_provider_subsystem_ground_truth_v1.md.

## Confirmed bugs

GT-001: Compare tab non-functional — 404 endpoint
  Where: Compare sub-tab of the Chat page, "Compare Responses" action
  Symptom: User selects two models (Model A, Model B), enters a prompt,
    clicks "Compare Responses". Both model result panels display the
    following error verbatim:
    "supervisor error: failed to parse JSON response: trailing characters
    at line 1 column 5
    Raw response (first 200 chars): 404 page not found"
    The Compare feature is completely unusable — no successful comparison
    has ever been observed.
  Expected: Both models return real responses to the prompt, displayed
    side-by-side in the two result panels. User can visually compare the
    outputs.
  Hypothesis: Compare feature calls a backend endpoint that does not
    exist, returning an HTML 404 page. The supervisor's JSON parser then
    chokes on the HTML. Fix is likely either implementing the missing
    endpoint in the Rust backend, or correcting the frontend to call the
    correct endpoint if one already exists under a different route.

GT-002: Chat tab — no per-message edit action
  Where: Chat sub-tab active conversation message bubbles
  Symptom: User cannot edit any message (user-sent or model-sent) after
    it has been sent/received. No edit button, no click-to-edit, no
    context menu with edit option. Messages are immutable once in the
    conversation.
  Expected: User can edit any user-sent message (to fix typos, refine
    prompt, retry with different wording). Ideally also edit model
    responses for annotation purposes.
  Hypothesis: Per-message edit action not yet implemented in the message
    bubble component. Likely needs an edit icon in each message bubble
    that opens an inline editor, plus a Tauri command to persist the
    edit back to the conversation store.

GT-003: Chat tab — no per-message delete action
  Where: Chat sub-tab active conversation message bubbles
  Symptom: User cannot delete an individual message from a conversation.
    The only deletion action is the top-right trash icon, which deletes
    the entire conversation (confirmed behavior per user testing). There
    is no way to remove a single bad prompt or a single low-quality
    response while keeping the rest of the conversation intact.
  Expected: User can delete any individual message from a conversation.
    The conversation continues with the remaining messages intact.
  Hypothesis: Per-message delete action not yet implemented in the
    message bubble component. Same component as GT-002 — both actions
    should probably be added together as part of a per-message action
    menu (edit + delete).

GT-004: Compare tab — no "New Compare" action
  Where: Compare sub-tab, top of the result area
  Symptom: After running a comparison, user has no way to start a fresh
    comparison. The "+New" button at the top of the left sidebar creates
    a new Chat conversation, not a new Compare run. Compare tab has no
    equivalent action.
  Expected: Compare tab has its own "New Compare" button that resets
    the prompt textarea and clears any existing comparison result,
    ready for a fresh run. Alternatively, the +New button in the left
    sidebar should be context-aware — create a new Chat conversation
    when Chat tab is active, create a new Compare run when Compare tab
    is active.
  Hypothesis: The +New button is hardcoded to create Chat conversations
    and does not dispatch differently based on active tab. Fix is either
    adding a dedicated Compare action button, or making +New tab-aware.

GT-005: Compare tab — no "Clear" action
  Where: Compare sub-tab, result area
  Symptom: Once a comparison has produced output (including error output
    as in GT-001), there is no way to clear the result and reset the
    textarea without navigating away from the Compare tab entirely.
  Expected: Compare tab has a "Clear" button that empties the prompt
    textarea, removes any displayed comparison result, and resets the
    model selector state (optional — resetting selectors is a design
    choice).
  Hypothesis: Compare tab has no state-management actions at all.
    Related to GT-004 — both are symptoms of Compare tab being built as
    a static form rather than as a stateful sub-view.

GT-006: History tab — verified working (baseline, not a bug)
  Where: History sub-tab
  Symptom: NONE — this entry documents correct behavior so the scout's
    first run can confirm the baseline. History tab displays all
    conversations with title, model name, message count, and timestamp.
    Per-row location pin and trash icons are visible and appear
    actionable.
  Expected: History tab continues to show all conversations (Chat and
    eventually Compare runs per GT-007 resolution). Scout finding the
    History tab broken would be a false positive.
  Hypothesis: N/A — this is the baseline entry. If the scout flags
    History as broken, investigate scout reasoning before assuming the
    page regressed.

GT-007: Compare/Chat state leakage in Conversations sidebar
  Where: Left sidebar "CONVERSATIONS" list, visible on both Chat and
    Compare tabs
  Symptom: The Conversations list in the left sidebar shows the same
    entries regardless of which tab is active. On the Compare tab, the
    list shows Chat conversations (e.g. "Hello", "WhaT's your model
    name?") that were created in the Chat tab and have no relevance to
    Compare. There is no separate list of past Compare runs — either
    Compare runs are not being persisted, or they are being persisted
    but not surfaced anywhere in the UI.
  Expected: Compare tab should have its own tab-scoped history of past
    Compare runs displayed in the Conversations sidebar (Option 7a).
    Each Compare run is persisted with a type discriminator field (e.g.
    kind: "chat_conversation" | "compare_run") so the UI can filter the
    list by active tab. The unified History tab continues to show all
    persisted items (Chat conversations + Compare runs) with type
    labels.
  Hypothesis: The conversation store has no "kind" field on entries,
    and the sidebar list component does not filter by active tab. Fix
    spans: (a) add kind enum to conversation store schema, (b) persist
    Compare runs as kind=compare_run entries, (c) filter sidebar list
    by active tab, (d) update History tab to show kind labels.

GT-008: Fuel per message display not visually prominent
  Where: Chat tab message input area footer ("5 fuel/msg" in small
    green text below the input) and page footer bottom-right ("~5 fuel
    per message")
  Symptom: Fuel cost is displayed in small, low-contrast text that is
    easy to miss. A user sending a message may not notice the cost,
    particularly when switching between free local Ollama models and
    paid cloud providers (OpenRouter, OpenAI API, Anthropic API). This
    is a real UX risk — a user could accidentally send expensive
    messages because the cost indicator is not salient.
  Expected: Fuel cost is displayed prominently near the send button as
    a visible cost badge (e.g. a colored pill showing fuel cost directly
    adjacent to the send arrow icon). The badge should be large enough
    to read at a glance and should visually distinguish free (0 fuel)
    from cheap (1-12 fuel) from expensive (50+ fuel) requests, possibly
    through color coding (green for free, yellow for cheap, red for
    expensive).
  Hypothesis: Design-level issue, not a wiring bug. Fix is a component
    redesign — likely add a new FuelBadge component and place it in the
    send button row of the message input area. Scout may flag this as
    Ambiguous since the current text is technically present and correct
    just not prominent — consider marking it as Ambiguous in the
    classification so Claude Code receives it as a design-improvement
    ticket rather than a hard-broken ticket.

GT-009: Chat tab incorrectly prepends agent routing prefix in Direct LLM mode
  Where: Chat sub-tab, every assistant response, when agent selector is
    set to "All Agents" (Direct LLM mode)
  Symptom: Every assistant response in an active Chat conversation is
    prepended with the literal text "*Routing to nexus-nexus
    (general)...*" on its own line before the actual response content.
    This occurs even when the user has explicitly selected "All Agents"
    in the top agent dropdown, which according to UI convention should
    mean "no specific agent, direct model access with no middleware".
  Expected: When the agent selector is set to "All Agents" / Direct LLM
    mode, assistant responses should contain only the model's actual
    response text with no routing prefix, no agent invocation wrapper,
    and no middleware markers. The UI behavior should match the
    setting: "All Agents" means "no agents", not "routed through a
    default agent called nexus-nexus".
  Hypothesis: The Chat tab's message handler routes all outgoing
    messages through an agent middleware layer unconditionally, and the
    "All Agents" option in the dropdown does not actually disable agent
    routing — it probably selects a default "general" agent named
    nexus-nexus. Fix is to make "All Agents" a genuine bypass that
    skips the agent middleware entirely and calls the selected model
    directly. The routing prefix is also a leaked debug/trace message
    that should not appear in user-visible output regardless of mode.
