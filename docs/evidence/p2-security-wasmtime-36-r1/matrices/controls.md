# Controls

## Guard mutation controls (`scripts/guard_controls.py`, `controls/summary.json`)

These ran in the validated snapshot. The guard
`p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` passes unmutated
(`controls/baseline.log`). Each control then restores one wrong state, runs
the guard alone (`--exact`), requires it to fail on the control's own
marker, and restores the file byte for byte. Source mutations are
`#[cfg(any())]` items, so every crate still compiles while the guard's text
scan sees them.

| Control | Mutation | File | Marker | Result |
|---|---|---|---|---|
| GC-01-WASMTIME-EXCEPTION-RETURNS | a RUSTSEC-2026-0327 Wasmtime exception added to `deny.toml` | `deny.toml` | a Wasmtime advisory is accepted | detected (exit 101) |
| GC-02-CALL-ASYNC-IN-PRODUCTION | `TypedFunc::call_async` named in production | `sdk/src/wasmtime_sandbox.rs` | Wasmtime's async and component-model APIs are not used | detected (exit 101) |
| GC-03-COMPONENT-IMPORT-IN-PRODUCTION | `use wasmtime::component::Val;` | `sdk/src/module_cache.rs` | Wasmtime's component module is not used | detected (exit 101) |
| GC-04-CRATE-ALIAS-IN-PRODUCTION | `use wasmtime as wt;` | `sdk/src/wasm_agent.rs` | the wasmtime crate is not aliased or renamed | detected (exit 101) |

Every control compiled and failed on its marker. All four files are
identical after (SHA-256), and the snapshot's Git status is unchanged (empty
before and after): `RESULT: PASS`.

The guard's in-test self-checks also run on every execution:
- the existing alias, component and safe-source probes;
- the new async/component-model probes: 7 detected and 3 safe.

## Scanner controls

- The stale-exception rule detects what it must. With the base's 11
  exceptions on the repaired lockfile, `cargo deny` failed with exactly the
  six stale ones (`matrices/security-gates.md`, section 1).
- The evaluator self-check: `scripts/eval_wasmtime_advisories.py` on 43.0.2
  reports exactly the four Wasmtime advisories the fresh scanners report for
  43.0.2 (`matrices/rustsec-facts.md`).

## Compile control

The core-only build (`matrices/reachability.md`): with Wasmtime's
`component-model` and `async` features compiled out, the three Wasmtime
users compile with all targets and the SDK's tests pass. This is the
compile-level counterpart of GC-02 and GC-03.
