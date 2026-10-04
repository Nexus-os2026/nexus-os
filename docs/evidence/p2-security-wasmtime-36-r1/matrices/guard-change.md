# The Phase Zero Wasmtime guard: why it changed, and how

## The conflict

`app/src-tauri/src/phase0_surface/fg_reliability/tests.rs`,
`p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api`, ended by requiring
`deny.toml` to hold exactly one RUSTSEC-2026-0316 exception whose reason names
"wasmtime 43.0.2", `get_typed_func`, the guard itself and "latent". The fresh
`cargo deny` run proves that exception stale on 36.0.17
(`security/pre-deny-edit-security-audit.txt`: `advisory-not-detected`).
`scripts/security-audit.sh` fails on a stale exception, and the mission
requires removing it. The guard and the mission's required removal cannot
both hold. The file is not in the mission's initial list, so the change is
surfaced here and in `REPORT.md`.

## What changed (and what did not)

| Part | Before | After |
|---|---|---|
| Scanner `wasmtime_forbidden_uses` (aliases, globs, component paths) | in force | unchanged; its doc comment no longer ties it to the 0316 exception |
| Its self-tests (aliases, component spellings, raw identifiers, cfg'd uses, safe core uses, comments, raw strings) | in force | unchanged |
| Production-source scan for aliases and component paths | in force | unchanged (assertion messages no longer say "while RUSTSEC-2026-0316 is accepted") |
| Cargo-metadata check: no renamed `wasmtime` dependency | in force | unchanged |
| The SDK sandbox's core imports and `get_typed_func::<(), ()>` | in force | unchanged |
| `WasmtimeSandbox` and `WasmAgent` stay latent-API needles | in force | unchanged |
| Async and component-model entry points (`call_async`, `instantiate_async`, `func_wrap_async`, `func_new_async`, `async_support`, `wasm_component_model`, `wasm_component_model_async`) in a production source that uses Wasmtime | not checked | rejected (new `wasmtime_async_uses`, with 7 detecting and 3 safe self-test probes) |
| `deny.toml` | exactly one RUSTSEC-2026-0316 exception, with its reasons | no exception entry may name `wasmtime`, RUSTSEC-2026-0316 or RUSTSEC-2026-0327; the entries must be found |

Every assertion that held before still holds, except the one that required a
Wasmtime exception to exist. That assertion is replaced by its opposite:
Wasmtime advisories may not be accepted at all. The guard keeps its name, so
the existing references to it in docs and dossiers stay valid.

## Its controls

`matrices/controls.md` (scripts/guard_controls.py): each mutation restores a
wrong state in a scratch copy of the candidate and the guard must fail on it:
a Wasmtime exception back in `deny.toml`; a `call_async` in a production
Wasmtime source; a `wasmtime::component` import; and an alias of the crate.
Every file is restored byte for byte.
