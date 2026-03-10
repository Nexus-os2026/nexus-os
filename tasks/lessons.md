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
- Feature-gated trait imports: when tests call trait methods on concrete structs behind `#[cfg(feature = "...")]`, the trait `use` must also be feature-gated with the same cfg — otherwise clippy reports unused-import without the feature, or missing-method with it. Use `#[cfg(any(feature = "a", feature = "b"))]` when multiple features share the import
- Pattern scanners and ML scanners complement each other: pattern matching is fast and deterministic (12 hardcoded injection phrases), ML catches subtle semantic attacks (social engineering, obfuscated manipulation) that patterns miss — test both independently and in combination

## Distributed Immutable Audit (Phase 6.4)
- Verify completion percentage by checking actual module coverage (gossip, verification, device pairing, kernel bridge, integration tests, CLI, UI), not just test count — a high test count can mask missing subsystems
- Reuse existing Transport trait infrastructure for gossip protocol instead of building new networking — LocalTransport enables deterministic testing of multi-device sync without sockets
- `pub(crate)` fields work for unit tests (same crate) but NOT for integration tests (separate crate) — if tests need field access, either make the field `pub` or provide a test-only method
- `LocalTransport::register_node()` must be called before `recv()` — forgetting this causes "node not found" errors in gossip tests
- Diverged chains for tamper testing: don't directly mutate `content_hash` on a private field; instead, build a chain with genuinely different events at the same sequence — produces naturally different hashes
- BlockBatchSink trait pattern prevents circular deps: kernel defines the interface, distributed crate implements it — kernel never depends on distributed

## Visual Permission Dashboard (Phase 6.5)
- Unicode escapes (`\u{1F512}`) in JSX text content cause TS1351 — wrap in expression `{"\u{1F512}"}` or use the JS escape form `"\uD83D\uDD12"`
- `replace_all` edits can corrupt unrelated occurrences of the same string — always check side effects when replacing a common pattern globally
- `#[allow(clippy::too_many_arguments)]` is acceptable for kernel methods that need agent_id + manifest + capability_key + enabled + changed_by + reason + audit_trail — splitting into a builder pattern would over-engineer an internal API
- `map_or(false, |x| ...)` should be `is_some_and(|x| ...)` in modern Rust — clippy catches this as `unnecessary_map_or`
- Permission system design: keep PermissionManager stateless for capabilities (reads from manifest), stateful only for locks and history — avoids sync issues between manager state and actual manifest capabilities
- Critical permissions need role-based gating (admin vs user) — don't let regular users enable `process.exec` or other Critical-risk capabilities
- Optimistic UI updates need careful revert logic — store previous state, apply change immediately, revert on backend error

## Protocol Integration (Phase 7.1)
- Sync kernel / async edge separation: keep all kernel types (GovernanceBridge, McpServer, A2ATask) fully synchronous; only the HTTP gateway crate uses tokio+axum. This prevents async infecting core governance logic.
- Capability inference from free-text A2A payloads requires keyword matching as a best-effort strategy — no structured capability field in the A2A spec. Fall back to "llm.query" for unrecognized text.
- MCP tool-level denials (CapabilityDenied from McpServer::invoke_tool) get audited in the MCP server's own AuditTrail, not the bridge's — integration tests must account for this dual-trail architecture when asserting audit event counts.
- GovernanceContext.audit_hash is `Option<String>` not `String` — set to `None` initially, then `Some(event_id.to_string())` after the audit event is appended. This avoids empty string sentinel values.
- When adding nav items to the UI sidebar, check for duplicate icons — "⬡" was already used by marketplace-browser; protocols got "⌬" instead.
- Pre-existing test failures on main (e.g., `test_context_building` in coder-agent due to missing `github_connector.rs`) should be verified via git stash/pop cycle before assuming your changes caused them.

## Engineering Foundation Hardening
- Execution credibility matters as much as architectural vision — 199 silent `let _ = audit.append_event(...)` failures across 75+ files proved that fail-closed is non-negotiable; a single missed audit event means an unrecorded agent action, violating the core "Don't trust. Verify." invariant
- `append_event` was originally infallible (returned `Uuid`), but `BatcherHandle::push_event` silently swallowed mutex poisoning via `if let Ok(...)` — the real silent failure was one layer deeper than the surface `let _ =` pattern
- Two-tier fix strategy for audit propagation: `?` for functions returning `Result` with compatible error types, `.expect("audit: fail-closed")` for boundary/non-Result code — both prevent silent failures, but `?` is preferred because it allows graceful error handling upstream
- Bulk mechanical changes across 75+ files need verification at every step — subagents claiming edits doesn't mean the edits persisted; always `grep` for remaining patterns after bulk operations
- Dependency governance (cargo-audit + cargo-deny) catches vulnerabilities before production — 4 wasmtime WASI advisories (host panics, unsound memory access, resource exhaustion) were discovered immediately upon adding cargo-audit to CI
- License compliance is not optional: Tauri dependencies pull MPL-2.0 (cssparser) and CDLA-Permissive-2.0 (webpki-roots) — both are permissive but would fail a strict MIT-only policy; `deny.toml` makes this explicit and auditable
- `rust-toolchain.toml` eliminates "works on my machine" drift — pinning stable channel with explicit components and targets ensures CI and local builds use identical toolchains
- Run `cargo fmt` after any bulk code modification — automated search-and-replace doesn't respect rustfmt's line-breaking rules for chained method calls

## CI / Workflows
- Always check for merge conflict markers after merging branches — leftover `<<<<<<< branch` / `>>>>>>> main` markers in YAML break CI silently
- The release.yml had unresolved merge conflict markers from the `ci/windows-artifact-fix` branch, causing duplicate steps and bare text in YAML that GitHub Actions rejects
- After any architecture migration (renaming crates, changing versions), grep all workflow files for old names/versions
- Validate YAML files locally before pushing: `python3 -c "import yaml; yaml.safe_load(open('file.yml'))"`
- Keep workflow files clean: each build job should have exactly one set of build/normalize/upload steps, not duplicates from both sides of a merge conflict
- Release workflows must only trigger on tag pushes (`push: tags: ["v*"]`), never on `push: branches: [main]` — otherwise every push to main triggers a failing release build
- Always verify workflow triggers after resolving merge conflicts — conflicts can corrupt the `on:` block

## Identity & Firewall (Phase 7.2)
- Adding a new field to a widely-used struct (e.g., `allowed_endpoints` on `AgentManifest`) causes E0063 in ~15+ files — use `Option<T>` with `#[serde(default)]` for backward compatibility, then batch-fix all struct literals
- Conditional governance checks avoid breaking existing code: `if self.egress_governor.has_policy(agent_id)` ensures agents without egress policies (most test agents) aren't default-denied by the new EgressGovernor
- Consolidate security patterns into one canonical module early — scattered copies in defense.rs, bridge.rs, and prompt_firewall.rs diverge over time and make pattern updates error-prone
- EdDSA (Ed25519) is simpler than ES256 (ECDSA P-256) for JWTs when ed25519-dalek is already a dependency — custom base64url + JWT encode avoids pulling in the `jsonwebtoken` crate
- Integration tests should exercise the full pipeline end-to-end (identity → token → firewall → egress → audit) rather than testing components in isolation — catches wiring issues that unit tests miss
- Rate limiting with sliding windows needs per-endpoint tracking — one endpoint hitting the limit shouldn't block other allowed endpoints for the same agent

## Compliance, Erasure & Provenance (Phase 7.3)
- CLI commands that call kernel modules (e.g., `compliance_status` using real `ComplianceMonitor`) provide more useful output than returning static JSON — worth the extra import even for demo/mock endpoints
- Cryptographic erasure proof events must be logged under `Uuid::nil()` (system agent), not the erased agent — otherwise the proof event itself would be subject to future agent erasure
- Legal hold is a cross-cutting concern: both `AgentDataEraser` and `RetentionPolicy` must independently respect holds — erasure blocks immediately, retention skips held agents during purge
- Tabbed UI layouts scale better than single-page dashboards for compliance: overview, risk cards, reports, erasure, provenance, and retention are distinct workflows that don't need to be visible simultaneously
- `ProvenanceTracker::rebuild_from_audit()` enables lineage recovery from any audit trail backup — design data provenance events to be fully reconstructable from their payload fields alone
- Multi-framework compliance reports (SOC2 + EU AI Act + HIPAA + CA AB316) should be generated from a single `FullReportConfig` to ensure consistent agent snapshots and audit trail state across all framework sections

## Marketplace & Developer Toolkit (Phase 7.4)
- SQLite `INSERT OR REPLACE` is the simplest upsert for marketplace agents — avoids checking existence before insert, and the primary key (package_id) naturally deduplicates
- `PermissionRiskLevel` doesn't implement `Ord` — use `.fold()` with a rank closure instead of `.max()` when comparing risk levels across capability lists
- Verification pipeline should use fold-based verdict escalation (Approved < ConditionalApproval < Rejected) — each check can only raise the verdict, never lower it
- Test agent templates against the kernel's actual CAPABILITY_REGISTRY (11 capabilities) — using capabilities like `screen.capture` or `input.keyboard` that aren't registered causes silent test failures
- `verified_publish_sqlite()` should reject on `Verdict::Rejected` but allow `ConditionalApproval` through — conditional agents get listed with flags for human review
- Frontend marketplace pages need graceful degradation: call real Tauri backend when available, fall back to mock data arrays for browser-only preview mode — use `hasDesktopRuntime()` guard
- Integration tests spanning CLI → marketplace → kernel should use in-memory SQLite (`open_in_memory()`) for speed and isolation — avoid temp file cleanup issues across parallel test execution
- The scaffold → test → package → publish pipeline validates each stage feeds into the next: scaffold produces valid manifests (kernel parser), test runner exercises capabilities (SDK context), packager creates signed bundles (Ed25519 + attestation), publisher runs verification (6 checks)

## Ignored Tests
- Never use `#[ignore]` as a workaround for tests that "might not work in CI" when cargo/git/shell tools are available — always verify first by running with `--ignored`, and only leave `#[ignore]` if there is a genuine reason (e.g., performance benchmark with timing sensitivity)
- `rust,ignore` in doctests means the example won't even compile-check — use `rust,no_run` instead when you want compile checking without execution. A bare `use nexus_sdk::prelude::*;` compiles fine as a doctest
- The governance overhead benchmark (`governance_benchmark.rs`) is legitimately `#[ignore]` — it's timing-sensitive, CPU-intensive, and requires `NEXUS_PERF=1` opt-in. This is the correct pattern for performance benchmarks

## Web API Integration Tests (Phase 7.5)
- Integration tests (in `protocols/tests/`) cannot access private struct fields like `GatewayState.inner` — use the REST API itself (e.g., GET /api/agents) to retrieve agent IDs and verify state, which also tests the API more realistically
- When testing auth rejection across many endpoints, iterate with `router.clone().oneshot()` — axum routers are cloneable and each `oneshot()` consumes the router
- WebSocket integration tests need a real TCP listener (`TcpListener::bind("127.0.0.1:0")`) because `oneshot()` doesn't support protocol upgrades — use `tokio_tungstenite::connect_async` against the ephemeral port
- Graceful shutdown testing should verify observable outcomes (health endpoint returns agents_active=0) rather than peeking at internal state — this is both more realistic and avoids private field access
- Marketplace search handler returns `{"results": [...]}` not `{"agents": [...]}` — always read the handler source to confirm response shape before writing assertions
- The `metrics` crate global recorder can only be installed once per process — integration tests that don't need real metrics should test the fallback path (metrics: None) instead of trying to install a recorder

## Dependency License Compliance
- When adding new heavy dependencies (wasmtime, reqwest, hyper, hf-hub, metrics-exporter-prometheus), always run `cargo deny check licenses` locally before pushing — transitive deps like `aws-lc-sys` bring non-obvious licenses (OpenSSL) that aren't in the default allow list
- The `aws-lc-sys` crate (pulled in by rustls → aws-lc-rs) uses the OpenSSL license — this is an FSF-approved permissive license but must be explicitly allowed in deny.toml
- After adding any new Cargo dependency, run `cargo deny check` as part of the pre-push checklist alongside fmt/clippy/test

## Test Portability
- Tests must only use commands from the TerminalExecutor ALLOWLIST (cargo, npm, pip, git, python, node, npx) — `echo`, `true`, `ls` etc. are NOT in the allowlist and will fail with CommandBlocked
- Prefer `git --version` over `cargo --version` in CI-portable tests — git is available on virtually every CI runner image, while cargo requires a Rust toolchain
