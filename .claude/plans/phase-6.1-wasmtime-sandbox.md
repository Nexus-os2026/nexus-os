# Phase 6.1 — Real Wasm Sandboxing with wasmtime

## Implementation Plan for Claude Code

**Goal:** Replace the abstract sandbox layer with real wasmtime-based Wasm execution, giving every plugin true memory isolation, fuel-metered execution, and governance-gated host functions.

**Constraint:** All 442 existing tests MUST remain green. We implement behind existing traits — no breaking changes upstream.

---

## Step 1: Add wasmtime dependencies

**Files to modify:**
- `crates/nexus-kernel/Cargo.toml` (or wherever the kernel crate lives)

**Action:**
```toml
[dependencies]
wasmtime = "27"
wasmtime-wasi = "27"
```

**Verify:** `cargo check` passes across workspace.

---

## Step 2: Create `WasmtimeRuntime` struct

**Files to create:**
- `crates/nexus-kernel/src/sandbox/wasmtime_runtime.rs`

**Modify:**
- `crates/nexus-kernel/src/sandbox/mod.rs` — add module, re-export

**Design:**

```rust
use wasmtime::{Engine, Module, Store, Linker};

pub struct WasmtimeRuntime {
    engine: Engine,
}

pub struct WasmtimeInstance {
    // Each plugin gets its OWN Store — critical for isolation
    // Store is NOT Send+Sync, so one per plugin, never shared
    store: Store<PluginState>,
    instance: wasmtime::Instance,
}

/// Per-plugin state held inside the Store
pub struct PluginState {
    pub plugin_id: String,
    pub fuel_budget: u64,
    pub memory_limit_bytes: usize,
    pub capabilities: Vec<String>,  // granted capabilities from RBAC
    pub kv_store: HashMap<String, Vec<u8>>,  // plugin-scoped storage
}
```

**Key decisions:**
- One `Engine` shared across all plugins (engines are thread-safe, compilation cache)
- One `Store<PluginState>` per plugin instance (NOT shared — this is the isolation boundary)
- `PluginState` carries the governance context so host functions can check permissions

---

## Step 3: Implement the existing `SandboxRuntime` trait

**Action:** Find the existing sandbox trait (likely in `sandbox/mod.rs` or `sandbox/traits.rs`) and implement it for `WasmtimeRuntime`.

**The trait implementation should cover:**
1. `load(wasm_bytes: &[u8]) -> Result<Module>` — compile Wasm bytes via `Engine::new()` + `Module::new()`
2. `instantiate(module, config) -> Result<Instance>` — create Store with PluginState, link host functions, instantiate
3. `execute(instance, function_name, args) -> Result<Output>` — call exported Wasm function with fuel metering
4. `teardown(instance)` — drop the Store, reclaim all memory

**Fuel metering setup in Store:**
```rust
let mut store = Store::new(&engine, plugin_state);
store.set_fuel(fuel_budget)?;         // set initial fuel
store.fuel_async_yield_interval(Some(10000)); // yield periodically for async
```

**Memory limits via wasmtime Config:**
```rust
let mut config = wasmtime::Config::new();
config.consume_fuel(true);
config.max_wasm_stack(512 * 1024);  // 512KB stack per plugin
// Memory limits enforced via resource limiter on Store
```

---

## Step 4: Build host function linker with governance gates

**Files to create:**
- `crates/nexus-kernel/src/sandbox/host_functions.rs`

**Host functions to expose (all governance-gated):**

| Function | Signature | Governance Check |
|---|---|---|
| `nexus_log` | `(level: i32, msg_ptr: i32, msg_len: i32)` | Always allowed |
| `nexus_kv_get` | `(key_ptr, key_len) -> (val_ptr, val_len)` | Requires `kv:read` capability |
| `nexus_kv_set` | `(key_ptr, key_len, val_ptr, val_len)` | Requires `kv:write` capability |
| `nexus_request_capability` | `(cap_ptr, cap_len) -> i32` | Checked against RBAC + governance |
| `nexus_get_time` | `() -> i64` | Always allowed (deterministic in replay mode) |
| `nexus_random` | `(buf_ptr, buf_len)` | Seeded deterministic in replay, real otherwise |

**Implementation pattern for each host function:**
```rust
linker.func_wrap("nexus", "nexus_kv_get", |mut caller: Caller<'_, PluginState>, key_ptr: i32, key_len: i32| -> Result<(i32, i32)> {
    let state = caller.data();
    
    // GOVERNANCE GATE — check capability before executing
    if !state.capabilities.contains(&"kv:read".to_string()) {
        return Err(anyhow!("Plugin '{}' lacks kv:read capability", state.plugin_id));
    }
    
    // Read key from Wasm memory
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let key = read_string_from_wasm(&memory, &caller, key_ptr, key_len)?;
    
    // Fetch from plugin-scoped KV
    let value = state.kv_store.get(&key).cloned().unwrap_or_default();
    
    // Write value back to Wasm memory
    write_bytes_to_wasm(&memory, &mut caller, &value)
})?;
```

**Critical:** Every host function checks `PluginState.capabilities` before executing. This connects directly to Phase 3 RBAC and Phase 4 adaptive governance.

---

## Step 5: Wire fuel metering into adaptive governance

**Files to modify:**
- `crates/nexus-kernel/src/governance/` — wherever adaptive governance lives

**Design:**
- Governance tier determines fuel budget:
  - `Tier::Minimal` → 1,000,000 fuel units (fast operations only)
  - `Tier::Standard` → 10,000,000 fuel units
  - `Tier::Extended` → 100,000,000 fuel units
  - `Tier::Unlimited` → u64::MAX (admin plugins only)
- When a plugin exhausts fuel, wasmtime traps with `FuelExhausted`
- We catch this, log it to the audit trail, and return a governed error
- If a plugin repeatedly exhausts fuel, adaptive governance can downgrade its tier

**Integration point:**
```rust
// Before executing a plugin
let fuel = governance.fuel_budget_for(plugin_id, action)?;
store.set_fuel(fuel)?;

// After execution
let remaining = store.get_fuel()?;
let consumed = fuel - remaining;
governance.record_fuel_usage(plugin_id, consumed)?;
audit.log_execution(plugin_id, consumed, result)?;
```

---

## Step 6: Create test plugin (.wasm binary)

**Files to create:**
- `test-plugins/hello-plugin/Cargo.toml`
- `test-plugins/hello-plugin/src/lib.rs`

**The test plugin should be a simple Rust → Wasm library:**
```rust
// Compiled with: cargo build --target wasm32-wasi --release

#[no_mangle]
pub extern "C" fn hello() -> i32 {
    // Call host function to log
    unsafe { nexus_log(0, "Hello from sandboxed plugin!".as_ptr(), 28); }
    42 // return value
}

extern "C" {
    fn nexus_log(level: i32, msg_ptr: *const u8, msg_len: i32);
}
```

**Build command:**
```bash
cd test-plugins/hello-plugin
cargo build --target wasm32-wasi --release
# Output: target/wasm32-wasi/release/hello_plugin.wasm
```

**Include the compiled .wasm in test fixtures** so CI doesn't need the wasm32 target.

---

## Step 7: Integration tests

**Files to create:**
- `crates/nexus-kernel/tests/sandbox_wasmtime_tests.rs`

**Test cases (minimum 8 tests):**

1. **`test_load_valid_wasm`** — loading a valid .wasm module succeeds
2. **`test_load_invalid_wasm`** — loading garbage bytes returns clean error
3. **`test_execute_hello_plugin`** — execute hello(), get return value 42
4. **`test_host_function_log`** — plugin calls nexus_log, verify log output captured
5. **`test_capability_denied`** — plugin without `kv:read` tries nexus_kv_get, gets governance error
6. **`test_capability_granted`** — plugin with `kv:read` reads successfully
7. **`test_fuel_exhaustion`** — plugin with low fuel budget (100 units) runs infinite loop, gets trapped cleanly without affecting kernel
8. **`test_memory_isolation`** — two plugins running simultaneously cannot read each other's memory
9. **`test_plugin_crash_isolation`** — one plugin panics/traps, other plugins and kernel continue unaffected
10. **`test_fuel_usage_recorded`** — after execution, governance layer has accurate fuel consumption record

---

## Step 8: Update CLI and desktop UI

**CLI (`nexus sandbox status`):**
- Show runtime type: `wasmtime v27`
- Show active plugins with fuel usage stats
- Show capability grants per plugin

**Desktop UI (Tauri):**
- Plugin list page should show sandbox status: `Isolated (wasmtime)` badge
- Fuel usage as a progress bar per plugin
- This prepares for 6.5 (visual permission dashboard)

---

## Verification Checklist

Before marking 6.1 complete:

- [ ] `cargo test` — all 442 existing tests pass (ZERO regressions)
- [ ] New wasmtime tests pass (minimum 8, target 10+)
- [ ] Test plugin compiles to .wasm and runs in sandbox
- [ ] Host functions are governance-gated (not hardcoded allow)
- [ ] Fuel metering integrates with adaptive governance
- [ ] Plugin crash does NOT crash kernel
- [ ] `nexus sandbox status` CLI command works
- [ ] Audit trail logs all sandboxed executions
- [ ] Update `tasks/todo.md` with completion status
- [ ] Log any surprises in `tasks/lessons.md`

---

## Claude Code Commands to Use

```
/plan          — Have Claude Code read sandbox.rs, NexusAgent trait, 
                 governance layer, then confirm this plan maps correctly
                 
/implement-roadmap — Execute steps 1-8 sequentially

/fix-bug       — If wasmtime API mismatches our trait design

/lessons       — After completion, log what we learned
```

---

## Architecture Notes for Claude Code Context

- **One Engine, many Stores** — Engine is the compilation cache (thread-safe, share it). Store is per-plugin (NOT thread-safe, never share).
- **wasmtime::Store is !Send** — each plugin execution should happen on the thread that created its Store, or use `wasmtime::Store::new()` fresh per invocation.
- **Deterministic replay** — `nexus_get_time` and `nexus_random` must return recorded values during replay mode. Wire this through PluginState.
- **Future-proofing for 6.2** — Speculative execution will clone a plugin's state, run it in a throwaway sandbox, inspect results, then decide. Design PluginState to be `Clone`.

---

*Phase 6.1 is the foundation. Once plugins run in real sandboxes with real isolation, 6.2–6.5 build on solid ground. Let's go, brother.*
