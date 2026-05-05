# ADR 0005 — Publish Retry Loop

## Status

Accepted, 2026-05-05. Implementation scheduled across BK.1
(this ADR), BK.2 (decorator + impl, tracing-only emission),
BK.3 (NodeEvent emission). Test coverage extends in Bug BL
(scenarios E/F/G). Connector RetryPolicy rationalization
extends in Bug BM.

## Context

After Bug BG (commit `5013a5a5`), the Twitter publish path
threads `Arc<NexusDatabase>` through `HeraldAdapter` →
`SocialPosterEntry` → `RealPublishExecutor` →
`TwitterConnector`, and publish calls go through
`post_status_update_idempotent` with a per-call
`uuid::Uuid::new_v4()` request_id. Idempotency is durable.

What is missing: a structured retry loop that handles
transient publish failures (rate limiting, transient transport
errors, provider 5xx) without saturating the connector or
losing observability. Today the publish path issues exactly
one attempt; on a retryable error it propagates to the caller.

The connector already has its own `RetryPolicy` (3 attempts,
200ms × 2.0× backoff) at
`connectors/core/src/connector.rs:11-14`, but connector retry
is transport-only and does not honor provider hints
(`retry_after_secs`) or emit swarm-level observability
events. Stacking connector retry on top of a swarm-level
retry compounds attempts; Bug BM disables connector retry on
the publish path post-BK.

Two adjacent retry-shaped patterns exist in the workspace:

- `kernel/src/errors.rs:33-37` — `ErrorStrategy::Retry` is
  consumed only by workflow-studio; not applicable to publish.
- `crates/nexus-governance-oracle/src/timing.rs:25-36` —
  additive jitter on a constant-time ceiling; not a backoff
  curve.

Neither is reusable directly. BK adds a retry primitive at
the correct layer.

## Decision

### Location

Retry lives in a `PublishExecutor` decorator named
`RetryingPublishExecutor`, defined in
`agents/social-poster/src/retry.rs` (new file). The decorator
wraps `Arc<dyn PublishExecutor>` and implements
`PublishExecutor` itself, forwarding `credentials_present()`
unchanged and intercepting `publish(text)`.

Rejected alternatives:

1. **In the connector**
   (`connectors/web/src/twitter.rs`). Connector retry is
   transport-only and lacks the swarm-level context required
   for `retry_after_secs` honoring, idempotency-key reuse
   semantics, and `NodeEvent` emission. Bug BM disables
   connector retry on the publish path to eliminate
   compounding.

2. **In the coordinator**
   (`crates/nexus-swarm/src/coordinator.rs`). Publish
   semantics are agent-specific (`credentials_present`,
   `request_id` reuse, server hint handling). The coordinator
   stays generic; agents own their retry policy.

### Module Location

`RetryConfig` and `RetryingPublishExecutor` ship in
`agents/social-poster/src/retry.rs`. Cargo dep direction
prohibits placement in `crates/nexus-swarm`:

- `crates/nexus-swarm/Cargo.toml:24` declares
  `social-poster-agent = { path = "../../agents/social-poster" }`.
- `agents/social-poster/Cargo.toml:22` declares only
  `nexus-swarm-core`, not `nexus-swarm`.

`nexus-swarm-core` is the wrong layer (agent-execution
primitives, not publish-specific policy). A dedicated
`nexus-retry` crate is YAGNI at this scope.

### RetryConfig

```rust
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u8,           // 3
    pub initial_backoff_ms: u64,    // 200
    pub backoff_multiplier: f64,    // 2.0
    pub max_backoff_secs: u64,      // 60
    pub retry_after_cap_secs: u64,  // 300
    pub jitter_pct: f64,            // 0.20
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 200,
            backoff_multiplier: 2.0,
            max_backoff_secs: 60,
            retry_after_cap_secs: 300,
            jitter_pct: 0.20,
        }
    }
}
```

### Backoff Formula

For attempt N (1-indexed; first call = attempt 1, retries
follow at N=2, 3, ...):

```text
base_ms = initial_backoff_ms * backoff_multiplier^(N - 1)
jitter  = rand::thread_rng().gen_range(-jitter_pct ..= jitter_pct)
wait_ms = (base_ms * (1.0 + jitter))
            .clamp(0.0, (max_backoff_secs * 1000) as f64) as u64
```

Multiplicative jitter on the computed wait, ±20% by default.
Cap is applied after the multiplier and before the
`retry_after_secs` precedence rule. `rand = "0.8"` matches
`crates/nexus-governance-oracle/Cargo.toml:14`.

### `retry_after_secs` Precedence

When the inner publish returns
`AgentError::PublishFailed { retry_after_secs: Some(n), .. }`,
the decorator waits `min(n, retry_after_cap_secs)` seconds
instead of the computed backoff. The 300-second ceiling
prevents pathological hints from stalling a run. `None`
falls through to computed backoff.

Producer site for the hint:
`agents/social-poster/src/swarm_entry.rs:611-633`. Wire shape
at swarm-event layer:
`crates/nexus-swarm/src/error.rs:79-90`
(`SwarmError::PublishFailed`, post-AE). The decorator
classifies on `AgentError::PublishFailed`
(`crates/nexus-swarm-core/src/error.rs:21`), the trait return
type — distinct from `SwarmError::PublishFailed` at the
swarm-event layer.

### `request_id` Lifetime

A single `uuid::Uuid::new_v4()` is generated **at decorator
entry** and reused across every retry attempt of the same
logical publish call. Distinct publish calls generate
distinct ids.

This lifts the per-call generation that BG installed at
`agents/social-poster/src/swarm_entry.rs:202-220` up into
the decorator. Cross-references ADR 0003 (persistent
idempotency store) for cache-key semantics: an idempotency
cache hit on a retried request_id correctly short-circuits
to the cached result, which is the intended observable
behavior of Bug BL test scenario F.

### Error Classification

The decorator retries **only** when the inner publish
returns `AgentError::PublishFailed { retryable: true, .. }`.
Every other `AgentError` variant returns immediately.

This rule is fail-closed: any future `AgentError` variant
introduced without an ADR amendment is **non-retryable by
construction**. Adding a new retryable error class requires
a follow-up ADR. This prevents silent retry-surface drift
when producers add variants.

Empirical justification: the producer at
`agents/social-poster/src/swarm_entry.rs:611-633` maps
RateLimited / Failed / AuthFailure into `PublishFailed` with
appropriate `retryable` flags. Non-publish `AgentError`
variants (Cancelled, FuelExhausted, Internal) are not
generated by the publish path.

### NodeEvent Emission Contract

The decorator emits exactly one `SwarmEvent::NodeEvent` per
**retry** attempt — that is, when `attempt_num ≥ 2`. The
first attempt is the implicit baseline and emits no retry
event. This keeps `phase = "retry_attempt"` semantically
accurate (the first attempt is not a retry) and avoids
adding NodeEvent noise to the happy path.

Trigger: emitted **before** the sleep that precedes the
upcoming retry attempt (i.e., on retry-attempt START).

Variant: `SwarmEvent::NodeEvent`, verbatim from
`crates/nexus-swarm/src/events.rs:55-61`:

```rust
NodeEvent {
    r#ref: NodeRef,
    phase: String,            // "retry_attempt"
    payload: serde_json::Value,
    ticket_nonce: Uuid,
}
```

Payload schema:

```json
{
    "attempt_num": 2,
    "wait_secs": 0.412,
    "last_error_summary": "rate limited: retry after 1s"
}
```

- `attempt_num` (`u32`): 1-indexed, against overall
  attempts. Value is always ≥ 2 because the first attempt
  emits nothing.
- `wait_secs` (`f64`): the actual sleep about to occur,
  post-jitter, post-cap, post-`retry_after_secs` override.
- `last_error_summary` (`String`): the inner error rendered
  via `Display`, truncated at 200 chars with a Unicode
  ellipsis (`…`) suffix when truncated. The bound caps
  payload size on a hot broadcast channel; 200 chars covers
  typical Twitter-style error JSON envelopes.

Correlation: `ticket_nonce` on the event matches the publish
call's containing node. Consumers correlate by
`ticket_nonce` just as they do for every other `NodeEvent`.

Existing consumers do not break on the new phase string.
Verified at:

- `app/src-tauri/src/commands/swarm.rs:254-265` (formats
  `phase={phase}`; no match on phase value).
- `tests/integration/src/swarm_harness/correlation.rs:97,111`
  (extracts `run_id` / `ticket_nonce`; ignores phase).
- `tests/integration/src/swarm_harness/events.rs:95` (static
  label mapping; ignores phase).
- `crates/nexus-swarm/tests/context_tests.rs:105-115`
  (asserts `phase == "plan"` on events its own producer
  emits; unaffected).

### Budget.wall_ms

The retry decorator is intended to consume `Budget.wall_ms`
for each sleep. **At HEAD `5013a5a5`, the coordinator does
not mutate `Budget` — `Budget::try_consume` has zero call
sites in the workspace (PB-2)**. BK.2 therefore performs no
budget arithmetic; the decorator sleeps without
consultation. Once PB-2 lands, the decorator will call

```rust
budget.try_consume(BudgetCost {
    wall_ms: slept_ms,
    ..Default::default()
})?
```

before each sleep, and an exhausted result will abort the
retry loop with a typed error. This ADR pins intent without
overstating today's behavior.

### Implementation Phasing

- **BK.1** (this commit): ADR only. Doc-only diff.
- **BK.2**: `RetryingPublishExecutor` + `RetryConfig` +
  classifier + tests. Emission stub uses `tracing` only.
  Wires the decorator into `SocialPosterEntry::new`
  construction;
  `SocialPosterEntry::with_publish_executor` test seam
  unchanged.
- **BK.3**: NodeEvent emission. Resolves the implementation
  question of how the decorator obtains an event emitter
  handle. Two documented candidates:
    1. Per-run decorator construction with captured
       emitter (decorator built inside execute(); no trait
       change).
    2. Trait extension to accept a context parameter
       carrying an `EventEmitter` handle (touches
       `PublishExecutor` signature; both impls update;
       all callers update).
  Selection deferred to a BK.3 preflight that establishes
  the publish() call-site shape and per-run emitter
  availability.

### Cargo Adjustments (BK.2)

- Add `rand = "0.8"` to `agents/social-poster/Cargo.toml`,
  matching `crates/nexus-governance-oracle/Cargo.toml:14`.
  Workspace-internal precedent: BG's `nexus-persistence`
  addition.
- Extend tokio features on
  `agents/social-poster/Cargo.toml:27` to include `"time"`
  (currently `["rt", "rt-multi-thread", "macros", "sync"]`).
  Required for `tokio::time::sleep`.

## Consequences

### Positive

- Transient publish failures auto-recover within the run.
- Provider hints (`retry_after_secs`) are honored, capped
  at 300s.
- Idempotency-key reuse + retry compose correctly: a retry
  after a network hiccup that did succeed server-side
  short-circuits via the idempotency cache.
- Observability: each retry attempt surfaces as a
  `NodeEvent` with `attempt_num`, `wait_secs`, and a
  truncated error summary.
- Connector retry compounding eliminated post-Bug BM.

### Negative

- `text: String` is cloned per attempt. Bounded at
  ~280 chars for tweets; not a hot path.
- One extra `Box::new` + `Box::pin` per attempt due to
  `#[async_trait]` dyn dispatch. Network-bound; negligible.
- Decorator is in-memory; an in-flight retry is lost if
  the process restarts mid-loop. State-across-restart is
  V3.

### Neutral

- `Budget.wall_ms` accounting is intent-only at HEAD;
  activates when PB-2 lands.

## Out of Scope

- **State-across-restart**: durable retry queue. Deferred
  to V3.
- **Connector RetryPolicy on the publish path**: owned by
  Bug BM (~10 LOC; disables connector retry post-BK ship).

## Test Scenarios (Bug BL Placeholders)

- **E** — Retryable failure → retry → success: inner
  produces `Err(PublishFailed{retryable:true})` then
  `Ok(_)`; assert one `NodeEvent` with `attempt_num=2` and
  a single returned `TweetResult`.
- **F** — Idempotency cache hit on retry: inner produces
  a retryable error on attempt 1, then on attempt 2 the
  underlying publish reuses the same `request_id` and
  short-circuits via the idempotency store; assert
  `request_id` reuse and successful result.
- **G** — Non-retryable failure → no retry: inner produces
  `Err(PublishFailed{retryable:false})`; assert zero
  retry `NodeEvent` events and propagation of the original
  error.

## Open Implementation Questions (BK.2/BK.3)

Resolved at the architectural level by this ADR. Remaining
questions are mechanism-level and will be answered by their
respective sub-commit preflights:

1. **Emitter threading** (BK.3): how the decorator obtains
   an event emitter handle. Two candidates documented above.
2. **Test seam shape** (BK.2): `ScriptableExecutor` inline
   in `agents/social-poster` `#[cfg(test)] mod tests` is
   the working assumption; revisited if Bug BL's
   `ScriptedCapability` shape converges.

## Cross-References

- **ADR 0003** — Persistent idempotency store: BK relies
  on the idempotency cache surviving across retried
  `request_id`s.
- **ADR 0004** — Credential vault facade:
  `credentials_present()` forwarding semantics unchanged.
- **Bug BG** (commit `5013a5a5`) — `Arc<NexusDatabase>`
  threading enabling per-call idempotency. BK lifts
  `request_id` generation from the connector layer to the
  decorator.
- **Bug BM** — Disable connector RetryPolicy on publish
  path (~10 LOC).
- **Bug BL** — Phase B harness scenarios E/F/G.
- **PB-2** — `Budget::try_consume` not called by
  coordinator.
- **PB-3 / PB-4** — `SwarmCompleted` payload + error
  semantics (orthogonal to BK; flagged in case retry
  exhaustion later surfaces a
  `SwarmCompleted`-vs-`SwarmAborted` ambiguity).

## Amendment 1 (2026-05-05) — Trait Boundary and Classifier Mechanism

ADR 0005 originally specified that the retry decorator would
classify on `AgentError::PublishFailed { retryable: true }`.
BK.2 preflight revealed that `PublishExecutor::publish` returns
`Result<TweetResult, KernelAgentError>` (kernel `AgentError`,
not `swarm_core::AgentError`). Kernel `AgentError` does not
carry a `PublishFailed` variant; rate-limit information is
encoded in `SupervisorError(String)` payloads and decoded by
the existing `classify_publish_error` helper at
`agents/social-poster/src/swarm_entry.rs:261`.

**Decision.** BK.2 reuses `classify_publish_error` at the
decorator boundary. The classifier:

- On `KernelAgentError::SupervisorError(msg)` → call
  `classify_publish_error(&msg)`. Match on returned
  `PublishStatus`:
  - `RateLimited { retry_after_secs }` → retry, honor hint.
  - All other `PublishStatus` variants → return error
    (non-retryable).
- On any other `KernelAgentError` variant → return error
  (non-retryable by construction).

This preserves the fail-closed property of the original ADR
text. The narrative reference to `AgentError::PublishFailed`
in this ADR describes the swarm-event-layer producer behavior
(already in place at
`agents/social-poster/src/swarm_entry.rs:631-647`), not the
decorator's classifier input.

**Deferred.** Migrating `PublishExecutor::publish`'s return
type from `KernelAgentError` to `swarm_core::AgentError` (so
the decorator classifies on a typed `PublishFailed` variant
directly) is filed as follow-up bug
**BN-RETRY-CLASSIFY-MIGRATION**. Out of scope for BK.

## Amendment 2 (2026-05-05) — Trait Mechanism for request_id Reuse

`PublishExecutor::publish(&self, text: String) -> Result<...>`
cannot carry a `request_id` parameter without a trait change.
BK.2 adds a sibling method with a default-impl forwarding
pattern:

```rust
async fn publish_with_request_id(
    &self,
    text: String,
    request_id: Uuid,
) -> Result<TweetResult, KernelAgentError>;

async fn publish(
    &self,
    text: String,
) -> Result<TweetResult, KernelAgentError> {
    self.publish_with_request_id(text, Uuid::new_v4()).await
}
```

`RealPublishExecutor`, `StubExecutor`, and
`RetryingPublishExecutor` implement `publish_with_request_id`
directly. Existing callers of `.publish(text)` (e.g.,
`SocialPosterEntry::execute` at `swarm_entry.rs:569`, the 13
test sites using `with_publish_executor`) continue to compile
unchanged; the trait's default impl generates a fresh UUID per
call. The retry decorator's `publish_with_request_id`
propagates the parameter across all retry attempts, satisfying
the request_id reuse contract.

Object safety preserved: both methods take `&self`, no
generics on methods, no associated types in return position.

## Amendment 3 (2026-05-05) — record_publish Single-Call Confirmation

The BK.2 preflight raised a concern that idempotency cache
replays under retry could cause `record_publish` to be called
twice with the same `tweet_id`. This concern was reviewed and
dismissed: the decorator's retry loop is internal to a single
`self.publish_executor.publish(...)` invocation at
`agents/social-poster/src/swarm_entry.rs:569`.
`SocialPosterEntry::execute` calls `record_publish` exactly
once per logical publish, in the `Ok` arm of the match on the
final `publish_result`. Intermediate retry attempts do not
surface to `execute`.

No deduplication contract is required.

## Amendment 4 (2026-05-05) — Per-Run Decorator Construction with Captured Emitter

ADR 0005 originally listed two BK.3 mechanism candidates for
emitter threading: (1) per-run decorator construction with
captured emitter, (2) trait extension. BK.3 selects Option
(1) and pins the construction approach.

**Circular dependency finding.** `NodeRef` is defined in
`crates/nexus-swarm/src/events.rs:25-30`, not in
`nexus-swarm-core`. `agents/social-poster/Cargo.toml:22`
declares only `nexus-swarm-core` (not `nexus-swarm`), and
adding `nexus-swarm` to social-poster's deps would create a
cycle (`crates/nexus-swarm/Cargo.toml:24` declares
`social-poster-agent` as a dependency). The decorator
therefore cannot construct `NodeRef` directly.

**Indirection through emit_phase.** The decorator calls
`EventEmitter::emit_phase("retry_attempt", payload)` on
the captured `Arc<dyn EventEmitter>`. The
`EventEmitter` impl handles `NodeRef` construction
internally (CoordinatorEmitter does this at
`crates/nexus-swarm/src/emitter.rs:43-46`; RecordingEmitter
discards it). This decouples social-poster from
`NodeRef` shape changes and avoids the dep cycle.

A follow-up bug **BO-NODEREF-RELOCATE** is filed to
consider moving `NodeRef` into `nexus-swarm-core` so future
agent-side observability can construct events directly. Out
of scope for BK.

**Construction pattern: lazy wrap in execute().**
`SocialPosterEntry` stores the un-wrapped
`Arc<dyn PublishExecutor>` plus an
`Option<RetryConfig>` field. The presence/absence of the
config IS the wrap signal:

- `SocialPosterEntry::new` sets
  `retry_config: Some(RetryConfig::default())`. Production
  retries are wrapped per-run with
  `Arc::clone(&ctx.emit)`.
- `SocialPosterEntry::with_publish_executor` sets
  `retry_config: None`. Test-injected stubs run exactly
  once per `execute()` call with no decorator interposed.

`BK.2`'s eager wrap at construction time is removed; a
single wrap site (inside `execute()` at
`agents/social-poster/src/swarm_entry.rs:596`) eliminates
the double-wrap risk.

**Emission timing.** The decorator emits
`emit_phase("retry_attempt", payload)` BEFORE
`tokio::time::sleep(wait).await`, so observers receive the
retry signal at attempt-START time. `emit_phase` is
non-blocking in practice: CoordinatorEmitter swallows
broadcast send failures (fire-and-forget,
`crates/nexus-swarm/src/emitter.rs:53-59`); RecordingEmitter
acquires a `tokio::sync::Mutex` (uncontended in test
fixtures).

**Tracing fallback.** `tracing::info!` continues to fire
unconditionally, regardless of emitter presence. Logs
remain available even when the decorator is constructed
without an emitter (e.g., direct test instantiation via
`RetryingPublishExecutor::new`).
