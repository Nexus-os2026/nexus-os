# Wasmtime reachability and API usage (Phase A, read-only, before any edit)

Established on the base `a7001311` before any change. The search covers every
tracked file (`git grep` over all 5,034 tracked files, `docs/evidence`
excluded): 51 files mention Wasmtime (`audit/files-mentioning-wasmtime.txt`);
110 lines name a `wasmtime` path in Rust (`audit/wasmtime-path-uses.txt`).

## Declarations

| Where | Declaration |
|---|---|
| `Cargo.toml` `[workspace.dependencies]` | `wasmtime = "43.0.2"` (now `"36.0.17"`), default features |
| `sdk/Cargo.toml` | `wasmtime.workspace = true` |
| `protocols/Cargo.toml` | `wasmtime.workspace = true` |
| `benchmarks/Cargo.toml` | `wasmtime.workspace = true` |

No other manifest declares Wasmtime, WASI (`wasmtime-wasi`, `wasi-common`) or
another Wasm runtime. No dependency is renamed (the existing guard checks
Cargo's effective metadata). `wasmtime-wasi` and `wasm-compose` are absent
from the repaired lockfile (`cargo tree -i` reports no such package).

## Who uses it

| Use | Files |
|---|---|
| Production | `sdk/src/wasmtime_sandbox.rs`, `sdk/src/wasmtime_host_functions.rs`, `sdk/src/module_cache.rs`, `sdk/src/wasm_agent.rs`, `sdk/src/shadow_sandbox.rs` (plus re-exports of SDK types in `sdk/src/lib.rs`) |
| Tests and benches | `sdk/tests/wasmtime_integration_tests.rs`, `sdk/tests/speculative_shadow_tests.rs`, `protocols/tests/web_api_integration_tests.rs`, `benchmarks/benches/phase67_bench.rs`, the in-file `#[cfg(test)]` modules of the SDK files |
| Guards | `app/src-tauri/src/phase0_surface/fg_reliability/tests.rs` (`p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api`) |
| Text only | `cli/src/lib.rs`, `cli/src/router.rs` (display strings), `sdk/src/sandbox.rs` (a comment) |

`nexus-protocols` and `nexus-benchmarks` use Wasmtime only in tests and
benches. Their production code does not use it.

## The API surface production Nexus uses (core Wasm only)

| Wasmtime API | Where |
|---|---|
| `Config::new`, `consume_fuel`, `epoch_interruption`, `max_wasm_stack` | sandbox, module cache, shadow sandbox, tests |
| `Engine::new`, `Engine::default`, `increment_epoch` | sandbox, module cache, agent |
| `Module::new`, `Module::validate` | module cache, agent |
| `Linker::new`, `func_wrap` (7 host functions), `instantiate` | host functions, sandbox |
| `Store::new`, `limiter`, `set_fuel`, `get_fuel`, `set_epoch_deadline`, `data`/`data_mut` | sandbox |
| `StoreLimits`, `StoreLimitsBuilder` (`memory_size` and the rest) | sandbox, host functions |
| `Instance::get_typed_func::<(), ()>` (`_start`, `nexus_main`), `TypedFunc::call` | sandbox |
| `Caller`, `Extern::Memory`, `Memory::data`/`data_size` | host functions, sandbox |
| `wasmtime::Error` | module cache |

## What production Nexus does not use

| Surface | Use | Evidence |
|---|---|---|
| 1. Core Wasm | yes | the table above |
| 2. Component Model (`wasmtime::component`, component `Linker`, `Val`, `Func`) | none | no path, group, glob or alias in any production source (the existing guard's scanner); the core-only compile below |
| 3. Async (`call_async`, `instantiate_async`, `func_wrap_async`, `func_new_async`, `async_support`; component async) | none | no such name in any production source that uses Wasmtime (the new guard check); the core-only compile below |
| 4. WASI | none | no WASI crate in any manifest or the lockfile |
| Module serialization (`serialize`/`deserialize`) | none | no call |

## Compile evidence: the core-only build

The text guards are backed by a compile. In a disposable copy of the
candidate (outside the repository), the workspace pin was changed to
`{ version = "36.0.17", default-features = false, features = ["cranelift",
"runtime", "std"] }`. This compiles out the `component-model`, `async` and
`component-model-async` features and every API they gate. Then:

- `cargo check -p nexus-sdk -p nexus-protocols -p nexus-benchmarks
  --all-targets`: exit 0 (`audit/core-only-experiment-check.txt`). The
  resolved Wasmtime features were only `cranelift`, `runtime`, `std` and their
  implied internals; nothing re-enabled the component model or async.
- `cargo test -p nexus-sdk`: exit 0, 183 + 10 + 26 + 1 passed
  (`audit/core-only-experiment-sdk-tests.txt`).

No Nexus code in the three crates that use Wasmtime (production, tests or
benches) needs any component-model or async API. This is evidence only. The
candidate keeps Wasmtime's default features (`matrices/hardening.md`).

## Dependency paths into the workspace

`cargo tree -i wasmtime --workspace --all-features -e normal,build,dev`,
before (`audit/before-tree-inverse-wasmtime.txt`) and after
(`audit/after-tree-inverse-wasmtime.txt`). The same 25 workspace packages
depend on Wasmtime in both (`audit/after-wasmtime-dependents.txt`), through
`nexus-sdk`, `nexus-protocols` and `nexus-benchmarks`. They include
`nexus-desktop-backend`, `nexus-cli`, `nexus-conductor`, `nexus-integration`,
`nexus-swarm` and the agents. `nexus-kernel` and `nexus-verifier-sandbox` are
not among them.

## Compiled-in Wasmtime features

| | Features (resolved, `--all-features`) |
|---|---|
| 43.0.2 (31) | `audit/before-wasmtime-features.txt`: includes `component-model`, `component-model-async`, `async` |
| 36.0.17 (25) | `audit/after-wasmtime-features.txt`: includes `component-model`, `async`; not `component-model-async`, which is not a 36.x default feature |

Removed with the change: `anyhow`, `backtrace`, `compile-time-builtins`,
`component-model-async`, `debug`, `futures`. None added.

## Unchanged Wasmtime-using sources

The five production SDK files are byte-identical to the base
(`audit/sdk-wasmtime-sources.sha256.txt`). The workspace compiles against
36.0.17 without any source change, so no compatibility repair was made.
