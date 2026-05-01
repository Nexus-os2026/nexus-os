# ADR 0004: Credential Vault Facade

- **Status:** ACCEPTED — Phase 1 shipped 2026-05-01
- **Date:** 2026-04-30
- **Deciders:** Suresh Karicheti (architect), Claude (technical advisor)
- **Supersedes:** None (retires `connectors/core/src/vault.rs` in Phase 1)
- **Superseded by:** None
- **Related:** ADR 0001 (Broker fate), ADR 0002 (Audit persistence), ADR 0003 (Persistent IdempotencyStore)

## Context

Bug AK was opened against credential storage. The Q3 / Bug AK preflight (HEAD `d98c4fe8`) established four facts that drive this decision:

1. `connectors/core/src/vault.rs` (188 LOC, 2 tests) has exactly **one** in-tree consumer (`http_connector.rs`). It uses AES-256-GCM with a caller-supplied 32-byte `VaultUserKey` per call. No persistence, no master key, no capability gate, no migration path.

2. The de-facto credential plane is three already-deployed systems:
   - Environment variables (read by ~80 files).
   - `~/.nexus/config.toml`, envelope-encrypted by `NEXUS_ENCRYPTION_KEY` (`kernel/src/config.rs`).
   - OS keyring via `keyring` v3 — production for four `nexus-swarm` LLM provider modules: `anthropic`, `openai`, `openrouter`, `huggingface`.

3. `vault.rs` has existed alongside these for the project lifetime and achieved 1-of-80 adoption. Its "caller supplies user key per call" model is the reason — usable only with operator ceremony nothing else in the tree performs.

4. Bug AK's surface ask is `SocialConfig` plus LLM `api_key` fields. Phase 1 scope is bounded to ~16 files. Phases 2 / 3 are filed as new bugs (AK-2, AK-3).

The four `nexus-swarm` providers already use OS keyring directly. Re-routing them through a facade is a behavior change beyond the original AK ask: it potentially exposes them to env-var override semantics they never see today. We must decide deliberately whether that override applies — and to which providers.

## Decision

Promote credential storage to a kernel-level facade with pluggable backends. Retire `connectors/core/src/vault.rs`.

### Location

New module: `kernel/src/secrets/`, alongside `crypto.rs` and `identity/credentials.rs`. Trait-based facade. Backends gated by feature flags.

### Public API (sketch)

```rust
#[async_trait]
pub trait SecretBackend: Send + Sync {
    async fn get(&self, scope: &str, name: &str)
        -> Result<Zeroizing<String>, SecretError>;
    async fn set(&self, scope: &str, name: &str,
        value: Zeroizing<String>) -> Result<(), SecretError>;
    async fn delete(&self, scope: &str, name: &str)
        -> Result<(), SecretError>;
    async fn list(&self, scope: &str)
        -> Result<Vec<String>, SecretError>;
}

pub struct SecretsFacade {
    backends: Vec<Box<dyn SecretBackend>>,
    capabilities: Arc<CapabilityRegistry>,
    audit: Arc<AuditTrail>,
    env_override_providers: Vec<String>,
    resolve_log_seen: Mutex<HashSet<String>>,
}
```

### Backends

Four backends ship in Phase 1:

1. `EnvVar` — reads from process env. Read-only; `set()` returns `BackendReadOnly`.
2. `OSKeyring` — `keyring` v3 (linux-native / apple-native / windows-native).
3. `SqliteEnvelope` — new `secrets` table in `NexusDatabase`, AES-256-GCM with a per-domain key derived via HKDF-SHA-256 (domain separator `nexus.secrets.v1`) from the kernel's existing `EncryptionKey` (see "Master key sourcing" below).
4. `Memory` — test fixtures only.

### Master key sourcing

The `SqliteEnvelope` backend does NOT read `NEXUS_ENCRYPTION_KEY` directly. It reuses the existing `kernel::crypto::EncryptionKey::from_config(&config.encryption)` path that already powers config envelope encryption and database-file encryption-at-rest. That path supports two key sources, both already in tree:

- **Primary: env** — `EncryptionConfig.key_source = "env"` (default), value read from the env var named in `EncryptionConfig.key_env` (default `NEXUS_ENCRYPTION_KEY`). 64-char hex string is decoded as raw 32 bytes; otherwise SHA-256-hashed as a passphrase.
- **Fallback: file** — `EncryptionConfig.key_source = "file"`, value read from `EncryptionConfig.key_file` (a path; raw 32 bytes or passphrase). Designed for Docker/K8s secret mounts.

The OS keyring is **not** a master-key source, even though it is a backend. Using it as both would create a circular bootstrap (the facade would need the master key to read the master key). Documented as a deliberate non-option.

The HKDF derives the SqliteEnvelope key from `EncryptionKey.bytes()` with domain separator `nexus.secrets.v1`. Plaintext `EncryptionKey` bytes never enter the SqliteEnvelope row format; only nonce + ciphertext do.

**Bootstrap on fresh install.** Phase 1 ships a `SecretsFacade::ensure_master_key_available()` helper invoked on first construction. Behavior:

1. If `config.encryption.enabled = true` and the configured source resolves, use it.
2. Else (a fresh install: `enabled = false` by default), generate a 32-byte key with `OsRng`, write it to `~/.nexus/master.key`, restrict permissions, and flip the config to `encryption = { enabled = true, key_source = "file", key_file = "~/.nexus/master.key" }`. Save the (envelope-encrypted) config.
3. Log an INFO line: `master key initialized: source=file path=~/.nexus/master.key`.

**Cross-platform permissions.** On Unix, `set_restrictive_permissions` (already in `kernel/src/config.rs`) sets `0o600`. On Windows, the existing helper is a no-op and the file inherits NTFS DACLs from the parent directory, which on a default user profile is reasonable but not strictly user-only. Phase 1 ships the Unix path as-is and, on Windows, emits a startup `WARN`: `master.key Windows ACL hardening deferred to AK-5; file is currently protected only by user-profile location`. AK-5 (Windows ACL hardening via `windows-acl` or equivalent) is filed; the `windows-acl` crate is NOT in the workspace at HEAD and adding a new dep is out of scope for this commit.

This bootstrap path is taken at most once per install and is recorded in the audit trail.

**Rotation.** Out of scope for Phase 1. The existing `kernel::crypto::rotate_encryption_key(old, new, data_dir)` already re-encrypts every `.db` file under `data_dir`; the new `secrets` table lives inside `nexus.db` and is therefore covered automatically. A future "rotate the SqliteEnvelope domain key without rotating the master key" capability is filed as Bug AK-4 and explicitly deferred.

`SqliteEnvelope` schema:

```sql
CREATE TABLE secrets (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    nonce BLOB NOT NULL,        -- 12 bytes
    ciphertext BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope, name)
);
```

### Lookup order — the `env_override_providers` gate

The chain order on `get_secret(scope, name)` depends on whether `scope` is present in the configured `env_override_providers` list.

- **Scope ∈ `env_override_providers` (env-first):**
  `EnvVar → OSKeyring → SqliteEnvelope → Memory → NotFound`

- **Scope ∉ `env_override_providers` (keyring-first; default for the four LLM providers):**
  `OSKeyring → EnvVar → SqliteEnvelope → Memory → NotFound`

`SqliteEnvelope` and `Memory` always trail. Tests use `Memory`; production never sees it.

### New config field

```toml
[credential_facade]
env_override_providers = ["codex_cli", "ollama"]
```

- Type: `Vec<String>` of provider / scope names.
- Default: `["codex_cli", "ollama"]`.
- The four LLM providers (`anthropic`, `openai`, `openrouter`, `huggingface`) are **NOT** in the default. They remain keyring-first until explicitly opted in by an operator.

Rationale for the default: `codex_cli` and `ollama` are conventionally env-driven (the upstream tools document env vars as the configured source). The four LLM providers are conventionally keyring-driven on this project (precedent set by `crates/nexus-swarm/src/providers/*`), and their keys gate paid API usage where stale env vars cause silent misrouting (see Consequences).

### Startup diagnostic

On the **first** successful `get_secret` per `(provider, process_lifetime)` pair, the facade emits exactly one INFO-level log line:

```
credential resolved: <provider> source=<env|keyring|config>
```

`source=config` corresponds to `SqliteEnvelope`. `Memory` is never logged (test only). Deduplication state lives in `SecretsFacade::resolve_log_seen` (a `Mutex<HashSet<String>>`); not persisted across processes.

### Migration test

A test in `kernel/src/secrets/tests.rs` (or alongside whichever module owns the resolver) MUST assert the env_override default. Required shape:

- Set `ANTHROPIC_API_KEY=stale` in the test environment.
- Configure a valid keyring entry for `anthropic` via the test-shim keyring backend (e.g. value `sk-live-keyring`).
- With default `env_override_providers = ["codex_cli", "ollama"]`, call `facade.get_secret("anthropic", "api_key")`.
- Assert the returned value is the keyring entry, **not** the env var.
- Add a sibling test that flips `env_override_providers` to `["anthropic"]` and asserts the env var wins.

### Capability scope

Per-namespace gating. Five top-level namespaces:

- `llm.*`     — provider API keys
- `social.*`  — posting / messaging tokens
- `oauth.*`   — OAuth `client_secret`s (higher trust)
- `infra.*`   — backend service keys
- `system.*`  — kernel-internal; never agent-readable

Capability strings: `secret:read:<namespace>.*`, `secret:write:<namespace>.*`.

**Phase 1 enforcement: log-only mode pending capability ledger (AK-2).** Phase 1 ships the capability strings AND wires every facade resolve through the audit log with the capability string attached. Phase 1 does NOT surface a HITL prompt — the gate is "log-only, allow-with-audit" until the `AgentExecutionContext` capability ledger lands in Phase 2 (Bug AK-2). Critically, every audit row records the resolved-from source (`env` | `keyring` | `sqlite`) so a future ledger replay can detect any provider that resolved from an unexpected source.

Per-connector (caller-identity) gating is deferred — it requires runtime crate identity as its own kernel primitive and its own ADR.

### Audit

All `get` / `set` / `delete` go through `nexus_kernel::audit::AuditTrail` on the ADR 0002 hash-chained persistence path. Plaintext values are NEVER logged. The existing `vault.rs` test asserting plaintext absence in audit JSON is preserved and extended to the facade.

### Zeroize

Direct dep: `zeroize = "1"`.
- `get_secret` returns `Zeroizing<String>`.
- Internal decrypt buffers in `SqliteEnvelope` are wrapped in `Zeroizing<Vec<u8>>`.

### Tauri surface

Replace per-provider commands (`save_provider_api_key`, etc.) with a unified set:

```rust
#[tauri::command] async fn vault_set(scope: String, name: String, value: String) -> Result<(), String>;
#[tauri::command] async fn vault_get(scope: String, name: String) -> Result<String, String>;
#[tauri::command] async fn vault_list(scope: String) -> Result<Vec<String>, String>;
#[tauri::command] async fn vault_delete(scope: String, name: String) -> Result<(), String>;
```

`Settings.tsx` collapses to one IPC pair per credential row. `chat_llm.rs::save_provider_api_key` is marked deprecated and routed to `vault_set` in Phase 1; deletion is filed as a Phase-2 follow-up.

### Migration (Phase 1, hard cutover)

One-shot startup migration: `kernel/src/secrets/migrate.rs`.

For each known credential field in `NexusConfig` (Phase 1 scope):

1. Open a single SQLite transaction on `NexusDatabase`.
2. Inside the transaction:
   a. Read field from config; if non-empty, `record_secret(scope, name, nonce, ciphertext)`.
   b. Verify-read each one: `lookup_secret(scope, name)` MUST decrypt to the source plaintext.
   c. Bump the `schema_version` row to mark migration complete.
3. Commit the transaction. On any error inside the transaction, ABORT — the SQLite rollback leaves both the `secrets` table and `schema_version` untouched, the config file is untouched, and the next boot retries.
4. ONLY AFTER the successful commit: clear the migrated `SocialConfig` fields in the in-memory `NexusConfig` and re-save the (envelope-encrypted) config file.
5. **One acceptable inconsistency.** If the config-file re-save in step 4 fails AFTER the DB commit succeeded: the next boot sees `schema_version` already bumped, skips re-migration, and consumers read from the facade (which has the data). The on-disk config still has the original `SocialConfig` field values, but they are dead — no consumer reads them. Log a `WARN` (`config-file re-save failed after migration commit; on-disk SocialConfig fields are stale but unused`) and continue. The facade is authoritative once `schema_version` is bumped.

Idempotent — runs at most once per install (gated by the `schema_version` row).

### `vault.rs` retirement

Deleted in the Phase 1 commit. `http_connector.rs` migrated to the facade in the same diff. `pub mod vault;` removed from `connectors/core/src/lib.rs`. No deprecation cycle — `nexus-connectors-core` is workspace-internal with one in-tree consumer.

### PQC

AES-256-GCM stays. Grover halves effective security to ~128 bits; acceptable per current NIST PQC posture. CryptoIdentity wiring is filed as a future hook for per-agent secret isolation; out of Phase 1 scope.

## Consequences

### Semantics change for the four LLM providers

`anthropic`, `openai`, `openrouter`, `huggingface` previously read OS keyring directly via per-module `keyring::Entry::new(SERVICE, USER)` calls. After this ADR they read through the facade. The default `env_override_providers = ["codex_cli", "ollama"]` keeps them keyring-first, so **no runtime behavior change for existing users on first deploy** — the plumbing is uniform; the precedence is preserved.

### Opt-in path

Operators wanting env-var precedence for an LLM provider must add the provider name to `env_override_providers` in `~/.nexus/config.toml`. No code change, no rebuild — config-only.

### Subtle fallback expansion (Refinement C)

Prior to this ADR, a keyring miss for `anthropic` / `openai` / `openrouter` / `huggingface` returned `NotFound` immediately — direct `keyring::Entry::new` provided no fallback path. Under the facade, a keyring miss for these four providers falls through to `EnvVar → SqliteEnvelope → Memory → NotFound`. Operators who relied on keyring-miss-as-fail-closed for these providers should audit their environment for stale `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `OPENROUTER_API_KEY` / `HUGGINGFACE_API_KEY` values before deploy. AK-6 is filed for an opt-in "strict keyring" mode that disables fallback per provider.

### Risk that prompted the opt-in default

A stale `ANTHROPIC_API_KEY` (or peer) in shell env, with the facade defaulting to env-first across the board, could:

- silently route `vision_judge` escalation in `nexus-ui-repair` to a wrong account,
- blow the documented $10 cost ceiling on `nexus-ui-repair` runs against a chargeable account the operator forgot,
- fail opaquely against a revoked key while the keyring holds a valid one.

Defaulting to keyring-first for the four LLM providers neutralizes all three modes. Operators with intentional env-var workflows opt in explicitly.

### Positive

- Single credential plane. One mental model.
- Capability-gated reads close the current no-real-gate hole.
- Hash-chained audit on every credential access.
- Headless deployments (Docker, CI) work without OS keychain via `SqliteEnvelope`.
- `vault.rs`'s 1-of-80 adoption problem is structurally fixed.

### Negative

- Hard cutover migration must succeed first try. Mitigated by verify-read + abort-on-mismatch + a test that exercises every Phase-1 field.
- Capability namespace boundaries become security boundaries (`llm.*` reaches all LLM keys). Acceptable because LLM agents already have model access; the secret is the access mechanism, not the privilege.
- Tauri command surface changes; one-time UI churn in `Settings.tsx`.
- Adds `zeroize` as a direct dep (already transitive via `aes_gcm`).

### Phase 1 ships as a six-commit series (Amendment 3)

Phase 1 lands as a series of commits inside one PR / branch, NOT a single 25-file commit. Each commit must independently compile and pass crate-scoped gates; the full `ci-local.sh` runs only on the final commit. Commit boundaries:

1. **Kernel secrets foundation** — `kernel/src/secrets/{mod,backend_env,backend_keyring,backend_sqlite,backend_memory,migrate,tests}.rs` (new), `kernel/src/lib.rs`, `kernel/src/config.rs` (`CredentialFacadeConfig` only), `persistence/src/lib.rs` (`secrets` table + helpers + tests). No callers yet; workspace must compile.
2. **SocialConfig migration (atomic 3-file unit)** — `kernel/src/config.rs` (migration invocation in `load_config`), `connectors/web/src/twitter.rs` (facade consumer), `agents/social-poster/src/swarm_entry.rs` (cred-presence check). Splitting breaks compilation mid-series.
3. **`connectors/core` vault retirement** — delete `vault.rs`, drop `mod` from `lib.rs`, migrate `http_connector.rs` to the facade.
4. **`nexus-swarm` provider re-routing (semantics-change commit)** — anthropic / openai / openrouter / huggingface. Commit message carries the semantics-change disclosure.
5. **Tauri command surface** — new `secrets.rs` commands; deprecate `save_provider_api_key`.
6. **Frontend wire-through (optional, defer if non-trivial)** — `Settings.tsx` IPC swap. If non-trivial, file as Phase 1.5.

Each commit's gate budget is the modified crates only. Never `cargo build --all-features`. Never `cargo test --workspace` inside Claude Code.

### Test mocking strategy for OSKeyring

If the `keyring` v3 crate exposes a built-in `mock` feature on the version pinned in `Cargo.toml`, use it directly. If not, `backend_keyring.rs` introduces an internal trait `KeyringBackend` with two impls: `OsKeyring` (production) and `MockKeyring` (test-only behind `#[cfg(test)]`). The trait is module-private — no public surface, no API obligation. This isolates the choice from external callers and lets us swap it later without an ADR.

### Neutral

- The VC system (`kernel/src/identity/credentials.rs`) and the secrets facade remain separate. Documented as intentional.

## Alternatives Considered

1. **Extend `connectors/core/src/vault.rs` with the ADR 0003 IdempotencyStore swap pattern.**
   Rejected: ADR 0003 solved persistence; vault's problem is master-key UX. SQLite alone leaves "caller supplies VaultUserKey per call" intact, which is precisely why nothing adopted it.

2. **Trait extraction inside `connectors/core/`.**
   Rejected: the semantic owner is the kernel. Capability gating belongs at the kernel boundary; locating the facade in connectors forces back-references from kernel governance into a lower layer.

3. **Dual-read (vault wins, `config.toml` fallback indefinitely).**
   Rejected: optional adoption produces non-adoption (vault.rs precedent). Solo founder + single dev box at HEAD means the population dual-read protects is one operator who can re-enter keys if migration aborts.

4. **Env-first across the board (no `env_override_providers` gate).**
   Rejected for the documented stale-env-var risk on LLM keys. Default keyring-first for high-trust scopes.

5. **Per-connector capability scope.**
   Rejected for Phase 1: requires runtime crate identity as a kernel primitive (its own ADR).

6. **Bridge with the VerifiableCredential system.**
   Rejected: different threat models (bearer secret vs signed public claim), different lifecycles, no current consumer needs the bridge.

## Implementation phasing

- **Commit 1:** `SecretsFacade` trait + scaffolding + backends. `OsKeyringBackend` is a STUB returning `BackendNotConfigured`. Real `keyring` crate dependency and live backend land in Commit 4.
- **Commit 2:** `kernel::startup::run_migrations`, `OnceLock<Arc<SecretsFacade>>` global, `RealPublishExecutor` and `SocialPosterEntry` facade threading, Twitter and search consumer rewire.
- **Commit 3:** LLM provider migration — 6 fields, vault scope `"llm"`, 4 nexus-swarm provider re-routes. Phase 1 aggregate is 10 fields total (4 SocialConfig + 6 LLM). The 6 env-only providers in `ProviderSelectionConfig` (groq / mistral / together / fireworks / perplexity / cohere) have no NexusConfig backing and need no migration — the facade's `EnvBackend` already serves them. Per-entry `LlmProviderEntry` api_key fields (variable count, dynamic IDs) are deferred to Bug AK-11.
- **Commit 4:** real OS keyring backend — `keyring` v3 direct dep on `nexus-kernel` with `linux-native` / `apple-native` / `windows-native` features (no D-Bus / libsecret runtime dependency on Linux). Central `service_string("nexus.<scope>.<name>")` scheme + single `KEYRING_USER = "nexus"` constant in `kernel/src/secrets/backend_keyring.rs`. Soft-error policy: every keyring failure other than `NoEntry` returns `SecretError::BackendNotConfigured`, which the facade dispatch loop treats as fall-through (env → keyring → sqlite → memory). CI runners without a kernel keyring service skip the backend automatically. Live round-trip test gated `#[ignore]`; run manually on a desktop session.
- **Commit 5:** `vault.rs` deletion + `http_connector` migration. Phase 1 complete.

## Trigger Conditions for Revisiting

- Crate identity becomes a runtime primitive — revisit per-connector capability scope.
- A feature requires presenting a VC through the same path as an API key — revisit VC bridge.
- `nexus-crypto` Phase 1 (PQC) lands and ML-KEM-encrypted secrets become available — revisit cipher choice.
- Phase 2 (Bug AK-2) lands — re-examine namespace taxonomy with OAuth and messaging in scope.
- More than one operator on a single install — revisit per-user secret isolation.
- More than two providers requesting env-first via `env_override_providers` — re-examine whether the default should flip.
- AK-4 (rotation of the SqliteEnvelope domain key without master-key rotation) lands — re-examine the master-key sourcing section.
- AK-5 (Windows ACL hardening for `~/.nexus/master.key` via `windows-acl` or platform equivalent) lands — re-examine cross-platform permissions documentation.
- AK-6 (opt-in "strict keyring" mode disabling env / sqlite / memory fallback per provider) lands — re-examine the lookup-order guarantees in this ADR.
