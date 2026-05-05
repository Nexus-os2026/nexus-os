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
