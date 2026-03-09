# Lessons Learned

## Terminal Pasting
- Large heredoc blocks get corrupted when pasted into Linux terminal
- Use python scripts or nano editor for creating files with complex content
- Use simple dashes (-) not em dashes in git commit messages

## UI Issues
- Always make buttons actually do something - no dead buttons ever
- Agent IDs must be valid UUIDs, not string names
- When adding a dropdown selector, wire it all the way to the backend
- Loading indicators: only show ONE at a time
- Voice features need real Web Speech API, not just visual indicators
- Clear buttons must actually clear state
- Case-insensitive keyword matching for any text analysis
- Regex patterns for keyword detection must use /i flag and cover common variants (e.g., "Three Fiber" not just "react three fiber")
- Button state feedback: show transient text ("Starting...") for ~1s, disable when action is invalid for current state
- Always update "last action" display immediately on user interaction, don't wait for backend refresh
- Use flex-wrap on button rows, not fixed grid columns - buttons must never overflow their container
- Set overflow: hidden on cards to prevent content from escaping borders at any viewport width
- Mock/fallback responses must match the product identity - never say "I can't do X" for capabilities the product has

## Architecture
- Every agent action must go through kernel capability checks
- Fuel budget checked before execution, not after
- Audit trail is append-only - never modify events
- Mock data is okay for UI development but must be clearly labeled

## Wasm Sandboxing (Phase 6.1)
- Reconnaissance before implementation catches major trait mismatches (e.g., discovering Supervisor has `health_check()` not `list_agents()`) — saves hours of rework vs coding blind from a plan
- `Rc<RefCell<AgentContext>>` elegantly solves wasmtime Store ownership: closures capture Rc clones, borrow checker satisfied without unsafe code
- Delegating all host functions to existing `call_host_function()` keeps governance in one place — no reimplemented capability checks, fuel accounting, or audit logging in the sandbox layer
- Fuel ratio 1:10,000 (1 Nexus fuel = 10,000 wasmtime instructions) with round-up division prevents under-charging — `(consumed + ratio - 1) / ratio` ensures even 1 wasm instruction costs 1 Nexus fuel
- Separate `kill_with_reason()` from `kill()` gives the audit trail context for safety actions — "fuel exhausted", "signature rejected", "crash isolated" are far more useful than a bare kill event

## Speculative Execution (Phase 6.2)
- Risk classification is multi-dimensional (HitlTier × AutonomyLevel × KpiStatus × EscalationLevel) — don't assume a single enum exists, synthesize from existing systems
- Hook speculative simulation at the ApprovalRequired error boundary — this is the natural pause point between consent request creation and human review
- Keep simulation scope realistic: when underlying APIs are mock (file I/O, LLM), simulation should predict call patterns and fuel costs, not actual content diffs
- Don't mutate existing types (ApprovalRequest) when a side-channel accessor (simulation_for_request()) achieves the same goal without breaking PartialEq/serialization
- ConsentRuntime.policy_engine().required_tier(op) is the key to knowing whether an operation needs simulation — check tier before calling enforce_operation

## Speculative Execution Depth (Phase 6.2 Gaps)
- Always verify architecture depth, not just test count — 521 tests passing doesn't mean the speculation pipeline is complete if ShadowSandbox forking, recording mode, host function interception, and threat detection aren't wired end-to-end
- Two-layer speculation avoids wasm deadlock: full shadow execution (ShadowSandbox.fork/run_shadow/collect_results) happens at the orchestration layer BEFORE calling execute(), while per-call interception (SpeculativePolicy + ThreatDetector) runs inline inside host function callbacks. Trying to fork a ShadowSandbox from within a host function would re-enter wasm execution and deadlock
- Use `Option<SpeculativePolicy>` so `None` means zero overhead — existing tests and 6.1 behavior are completely unaffected
- ThreatDetector needs `ContextSideEffect` (has actual prompt text) not `SideEffect` (has only prompt_len) for injection detection — choose the right abstraction level for your scanner input

## Local SLM Integration (Phase 6.3)
- Recon caught 3-param query mismatch early: `LlmProvider::query(&self, prompt, max_tokens, model)` takes 3 params, not the 2-param `query(prompt, model)` some plans assumed — reading the trait definition before writing code prevents signature mismatches across 5+ implementing types
- Router vs gateway distinction matters: `ProviderRouter` handles multi-provider selection with circuit breakers and strategies; `LlmGateway` handles fuel accounting, redaction, and audit logging — they're separate concerns, don't conflate them
- Feature flags keep compile times manageable: gating candle/tokenizers/hf-hub behind `local-slm` means default builds don't pull ~100 heavy ML crates, and CI runs fast with `MockProvider` for all governance tests
- Data structures should always be available, runtime gated: `ModelConfig`, `GovernanceVerdict`, `GovernanceSlm` work without `local-slm` feature — only actual tensor loading and inference need the feature flag
- When writing integration tests that span feature gates, handle both paths: `load()` returns different errors with vs without `local-slm` (feature error vs file-not-found) — assert on the common property (is_err) not the specific message
- Pattern scanners and ML scanners complement each other: pattern matching is fast and deterministic (12 hardcoded injection phrases), ML catches subtle semantic attacks (social engineering, obfuscated manipulation) that patterns miss — test both independently and in combination

## Distributed Immutable Audit (Phase 6.4)
- Verify completion percentage by checking actual module coverage (gossip, verification, device pairing, kernel bridge, integration tests, CLI, UI), not just test count — a high test count can mask missing subsystems
- Reuse existing Transport trait infrastructure for gossip protocol instead of building new networking — LocalTransport enables deterministic testing of multi-device sync without sockets
- `pub(crate)` fields work for unit tests (same crate) but NOT for integration tests (separate crate) — if tests need field access, either make the field `pub` or provide a test-only method
- `LocalTransport::register_node()` must be called before `recv()` — forgetting this causes "node not found" errors in gossip tests
- Diverged chains for tamper testing: don't directly mutate `content_hash` on a private field; instead, build a chain with genuinely different events at the same sequence — produces naturally different hashes
- BlockBatchSink trait pattern prevents circular deps: kernel defines the interface, distributed crate implements it — kernel never depends on distributed

## CI / Workflows
- Always check for merge conflict markers after merging branches — leftover `<<<<<<< branch` / `>>>>>>> main` markers in YAML break CI silently
- The release.yml had unresolved merge conflict markers from the `ci/windows-artifact-fix` branch, causing duplicate steps and bare text in YAML that GitHub Actions rejects
- After any architecture migration (renaming crates, changing versions), grep all workflow files for old names/versions
- Validate YAML files locally before pushing: `python3 -c "import yaml; yaml.safe_load(open('file.yml'))"`
- Keep workflow files clean: each build job should have exactly one set of build/normalize/upload steps, not duplicates from both sides of a merge conflict
- Release workflows must only trigger on tag pushes (`push: tags: ["v*"]`), never on `push: branches: [main]` — otherwise every push to main triggers a failing release build
- Always verify workflow triggers after resolving merge conflicts — conflicts can corrupt the `on:` block
