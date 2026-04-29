# ADR 0003: Persistent IdempotencyStore via Internal Storage Swap

- **Status:** ACCEPTED
- **Date:** 2026-04-29
- **Deciders:** Suresh Karicheti (architect), Claude (technical advisor)
- **Supersedes:** None
- **Superseded by:** None

## Context

The Q4+Q5 coupled preflight (HEAD c14d0de3) confirmed:

1. `IdempotencyManager` at connectors/core/src/idempotency.rs is in-memory only (HashMap-backed). Process restart drops the entire cache. Cross-restart retry of a transient publish failure can produce duplicate posts.

2. `TwitterConnector` does NOT use `IdempotencyManager` (zero references in connectors/web/src/twitter.rs and zero in agents/social-poster/src/swarm_entry.rs). The active social path is the most exposed to duplicate-on-retry.

3. Bug AE (commit 96e0d18c) shipped typed `SwarmError::PublishFailed { retryable, retry_after_secs }` but the V2 retry loop (Bug BG) cannot deliver dedup-safe retry without a stable cross-process idempotency key.

4. Persistence patterns are well-established (Bug W social_publish_log, Bug AL swarm_audit_log). Both use CREATE TABLE IF NOT EXISTS in migrate() with helper functions on NexusDatabase.

## Decision

We make four coordinated changes:

1. **New SQLite table `idempotency_cache`** in persistence/src/lib.rs::migrate(). Mirrors social_publish_log pattern with `expires_at_ms` column for TTL support.

2. **Internal storage swap on `IdempotencyManager`.** Keep the existing public type and API. Add new ctor `with_db(ttl_seconds, db: Arc<NexusDatabase>)` that wires SQLite as durable backing. Existing `::new(ttl_seconds)` stays in-memory only. Cache pattern: HashMap (fast read), SQLite fallthrough on miss, write-through on record_completion.

3. **Wire Twitter to the new persistent store.** New consumer at the connector layer in connectors/web/src/twitter.rs. Closes Bug V's documented duplicate-tweet-on-retry gap.

4. **Upgrade existing 4 consumers opportunistically.** facebook.rs, instagram.rs, sequential.rs, http_connector.rs each use `::new` today. Where the construction site has access to `Arc<NexusDatabase>`, upgrade to `::with_db`. Where it doesn't, leave on `::new` and file a follow-up.

We do NOT pursue trait extraction. That would break public type signatures across 4 crates with no offsetting benefit; the internal-swap approach delivers the same persistence semantics with zero API churn.

We do NOT pursue a parallel `SqliteIdempotencyStore`. That doubles storage surface for runtime selection no caller needs.

## Consequences

### What we keep

- `IdempotencyManager` public type and method signatures.
- All 4 existing consumers compile and run unchanged (opt-in upgrade, not forced).
- Existing in-memory tests stay green.

### What we gain

- Cross-process idempotency dedup for Twitter (closes Bug V's gap).
- Cross-process idempotency dedup for upgraded consumers.
- Restart-resilient cache for opt-in consumers.

### What we change

- `IdempotencyManager` internal struct gains `Option<Arc<NexusDatabase>>` field.
- HashMap is now a fast cache, not the source of truth (when DB is configured).
- New `with_db` ctor on `IdempotencyManager`.

### Audit consequences

- **One-time:** new SQLite table + ~150 LOC of helpers + Twitter wiring + ADR + tests.
- **Recurring:** none expected. Lazy eviction keeps the table bounded under normal load. Bug BH (filed in this commit) tracks a unified retention helper if/when needed.

## Alternatives Considered

### Option A: Trait extraction

Extract `IdempotencyStore` trait with two impls (InMemory, Sqlite). All 4 consumers store `Box<dyn IdempotencyStore>`.

**Why rejected:** Breaks public type signatures across 4 crates with no offsetting benefit. The runtime polymorphism isn't needed; no caller wants to switch impls dynamically. Trait extraction is the right shape if there were 10+ consumers OR if multiple impls were needed simultaneously; neither is true.

### Option C: Parallel `SqliteIdempotencyStore` + migration flag

Both stores live; runtime switch.

**Why rejected:** Doubles storage surface for runtime selection no caller needs. Adds a config knob with no clear "when do I flip this" answer.

### Option D: Hot-load HashMap on startup

Load all unexpired SQLite entries into HashMap at construction.

**Why rejected:** With 4 consumers each constructing their own manager, hot-load means 4x memory duplication of the same SQLite table. Cleaner shape: HashMap is fast cache, miss-falls-through-to-SQLite. No hot-load needed.

## Trigger Conditions for Revisiting

Reopen this decision if:

1. **A second persistent IdempotencyStore impl is needed** (e.g., Redis-backed for multi-instance deployments). Trait extraction (Option A) becomes the right shape then.

2. **Performance under load shows fast-path cache misses dominate.** Today the HashMap is per-manager and warms over a single process lifetime. If runs frequently hit "cold cache → SQLite query" paths, hot-load on startup becomes worth its memory cost.

3. **Multi-tenant key collisions surface.** request_ids are UUIDs (statistically unique), but if a future use case shares request_ids across tenants intentionally, a `tenant_id` column on idempotency_cache becomes necessary.

4. **The retention table grows unbounded.** Lazy eviction at write-time caps growth in normal operation. If a workload writes faster than it reads, growth is unbounded between writes. Bug BH provides the unified retention helper if needed.
