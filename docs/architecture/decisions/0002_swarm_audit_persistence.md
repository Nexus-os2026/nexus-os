# ADR 0002: Swarm Audit Persistence — Separate SQLite Table with Per-Row Hash Chain

- **Status:** ACCEPTED
- **Date:** 2026-04-29
- **Deciders:** Suresh Karicheti (architect), Claude (technical advisor)
- **Supersedes:** None
- **Superseded by:** None

## Context

Track C #1 (commit 2e984379) shipped the `SwarmAudit` page reading from the `swarm_audit_tail` Tauri command, which pulled from an in-memory `Arc<Mutex<HashMap<Uuid, Vec<AuditEntry>>>>` on `SwarmInner`. Process restart dropped everything. Bug AL was filed to make the tail survive restarts.

Preflight against HEAD `3acedc5b` revealed:

1. **The kernel `AuditTrail` is also in-memory** (`kernel/src/audit/mod.rs:187` — `events: Vec<AuditEvent>`). The hash-chain semantics are in-memory; the persistent `audit_events` SQLite table at `persistence/src/lib.rs:1009` is fed by a separate codepath (agent lifecycle), not by the live `AuditTrail::append_event`. Mirroring the kernel's in-memory chain does NOT add persistence — the swarm audit would still drop on restart.

2. **Bug W's `social_publish_log` is the established persistence pattern** (`persistence/src/lib.rs:1328` table DDL inside `migrate()`'s `execute_batch`; `:1845–1880` typed helpers; `:1351` `add_column_if_missing` for additive ALTER). The pattern is tested, idempotent, and integrates cleanly with WAL-mode SQLite.

3. **The kernel's chain has a single-writer assumption** (`pub fn append_event(&mut self, ...)`). Sharing it across swarm + kernel producers would force a global mutex on every audit emission and lose the swarm's per-run partitioning.

4. **Schema unification is lossy.** The kernel's `AuditEvent` shape (`event_id, timestamp, agent_id, event_type, payload, previous_hash, hash`) and the swarm's `AuditEntry` (`seq, event_kind, ticket_nonce, node_id, timestamp, payload_summary`) share zero identical fields. Folding swarm rows into the kernel table requires flattening every swarm-specific field into an opaque JSON blob inside `detail_json` — losing indexed columns for the SwarmAudit page's filter chips (event_kind, node_id).

## Decision

We persist swarm audit rows in a new dedicated SQLite table `swarm_audit_log` with per-row hash-chain columns, mirroring the Bug W pattern. The in-memory `HashMap` is dropped entirely; all reads and writes go through `nexus-persistence`.

### Schema

`swarm_audit_log` columns: `id` (auto-increment), `run_id`, `seq`, `event_kind`, `ticket_nonce`, `node_id` (nullable), `timestamp_secs`, `timestamp_nanos`, `payload_summary`, `previous_hash`, `current_hash`, `created_at`. Index on `(run_id, seq)`.

### Hash chain

Per-row SHA-256 over `run_id|seq|event_kind|ticket_nonce|node_id_or_empty|timestamp_secs|timestamp_nanos|payload_summary|previous_hash`. First row of a run uses `SWARM_AUDIT_GENESIS_HASH` (sixty-four zero hex). The chain is per-run, not global — preserves the existing partition.

### Tauri command shape

`swarm_audit_tail(run_id: Uuid, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<AuditEntry>, String>`. Defaults: 1000 / 0. Backwards-compatible with Track C #1 callers passing only `run_id`.

### Wire format

`AuditEntry` gains `previous_hash: String` and `current_hash: String`. Additive change; existing TS consumers compile without modification.

### Pure replacement

The forwarder loop (`ensure_forwarder` in `commands/swarm.rs`) is rewired: drop the in-memory HashMap append, call `last_swarm_audit_hash_for_run` to get the chain link, call `record_swarm_audit` to persist. Audit-write failures `eprintln!` and continue — the swarm runtime must not crash on a SQLite hiccup.

## Consequences

### What we gain

- Audit tail survives desktop restart. The SwarmAudit page can paste any historical run-id and load its full tail.
- Tamper-evidence at the row level via SHA-256 chain. `verify_swarm_audit_chain(run_id)` walks the chain and returns the first divergent index, or `None` if intact.
- Pagination at the command level. Default 1000 limit shields the wire from large runs.
- Backwards-compatible Tauri shape. Track C #1 frontend keeps working; the two new wire fields are additive.

### What we lose

- One-time data loss on cutover. The in-memory `HashMap` from previous-process runs is gone forever — there is no migration path. Acceptable per the original Bug AL framing (today's behavior already discards on restart).
- 5 LOC of "fast path" — the in-memory append is replaced by a SQLite write per event. Phase G perf check confirms the throughput is acceptable.

### What we defer

- **Retention / pruning** (Bug BA). Unbounded growth in v1; matches the existing precedent (no audit-flavoured table has TTL today). Address when storage exceeds ~10MB or row count exceeds ~100k.
- **Hash-chain inspection UI** (Bug BC). Storage primitives are in place; rendering integrity badges and "verify chain" buttons is a separable v2.
- **`swarm_list_runs` + run-history dropdown** (Bug BD). The page works without it (paste any run-id); discoverability iteration deserves its own preflight.
- **Live-append from `swarm:event`** (Bug AM, pre-existing). One-shot fetch + manual refresh remains the v1 freshness story.

### What we preserve

- Per-process `seq: u64` atomic counter on `AuditEntry` for wire compat with Track C #1's `key={entry.seq}` React usage. The SQLite auto-increment id is a separate identity; the two diverge across restarts but both are exposed.
- `AuditEventKind` typed union (Track C #1) — unchanged. No new event kinds needed for persistence.
- `auditKindCategory` mapper — unchanged.

## Alternatives Considered

### Option A: Mirror the kernel `AuditTrail`

Wire swarm events into the existing in-memory `AuditTrail::append_event` path. Share the chain.

**Why rejected:** the kernel chain is also in-memory (preflight Q2 confirmed). It does not add persistence. To get persistence, we'd still need the SQLite layer — Option A is Option B with extra coupling. Plus the single-writer mutex would serialize swarm + kernel emissions; plus the schema is lossy (swarm-specific fields flatten into JSON).

### Option C: Dual-write shim (HashMap + SQLite)

Keep the in-memory HashMap for fast reads; also persist to SQLite for durability.

**Why rejected:** two sources of truth. Reads from the HashMap could lag writes to SQLite, or vice versa. The SwarmAudit page would need to merge them. Latent inconsistency; no real win because SQLite-WAL reads are fast enough.

### Option D: Skip hash chain

Persist rows without `previous_hash` / `current_hash` columns. Defer chain integrity to a future commit.

**Why rejected:** the storage cost of the two columns is ~64 bytes per row; the verification helper is ~10 LOC. Adding the chain later requires backfilling existing rows with synthesized hashes (which lose tamper-evidence on the historical rows). Cheaper to do it right the first time.

## Trigger Conditions for Revisiting

Reopen this decision if any of the following become true:

1. **Storage growth exceeds ~10MB or row count exceeds ~100k** without a retention pass. At that point, Bug BA matters and the schema may need a compaction column (e.g. `archived_at: Option<i64>`).

2. **A multi-tenant audit requirement lands** where one user's rows must be isolated from another's at the file-system level. Today, the single global SQLite file is shared across all runs/users — fine for single-user desktop, broken for multi-tenant. Would need either per-tenant DB files or a `tenant_id` column.

3. **Cross-process / cross-machine chain unification** becomes a requirement (e.g. distributed swarms or replicated audit). The current per-run hash chain is local to one SQLite file; cross-process unification needs the existing `BlockBatchSink` pattern from `kernel/src/audit/mod.rs:54`.

If none of these become true, the decision stays. Bug BC (chain inspection UI) and Bug BD (list-runs surface) iterate the UX without changing this storage decision.
