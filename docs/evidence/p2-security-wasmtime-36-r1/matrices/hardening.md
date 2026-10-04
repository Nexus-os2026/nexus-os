# Optional attack-surface hardening: investigated, not applied

## The question

Can Nexus declare Wasmtime with `default-features = false` and only the core
features it needs, to compile out the unused component-model and async code?

## What the investigation found

- On 36.0.17 the default features already exclude `component-model-async`
  (43.0.2's resolved set included it). The code RUSTSEC-2026-0327 affects is
  absent on this line in any case (unaffected below 39.0.0).
- With `default-features = false, features = ["cranelift", "runtime", "std"]`,
  `nexus-sdk`, `nexus-protocols` and `nexus-benchmarks` all compile with all
  targets, and the SDK's tests pass: 183 + 10 + 26 + 1
  (`audit/core-only-experiment-*.txt`, a disposable copy, not the candidate).
  So Nexus needs no component-model or async API.

## Why it is not in this candidate

- The SDK test run covers the API surface, not runtime behaviour that
  depends on dropped defaults. For example, `wat` (text modules passed to
  `Module::new`), `cache`, `parallel-compilation`, `pooling-allocator`,
  `gc`, `threads`, `profiling` and `coredump` change behaviour or
  performance rather than types.
- Feature selection is unified across the whole workspace graph (25 crates,
  the desktop included). Proving the exact required set would need a full
  workspace and desktop validation of its own.
- The security repair does not need it: 36.0.17 is unaffected by
  RUSTSEC-2026-0327 with its default features.

## Recommendation (future, separately approved)

Pin Wasmtime with `default-features = false` and an explicit minimal set
(starting from `cranelift`, `runtime`, `std`, plus whichever of `wat`,
`cache`, `parallel-compilation` and `pooling-allocator` Nexus is shown to
rely on). Then extend `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api`
to require that `component-model` and `async` stay disabled in Cargo's
resolved features. Validate the full workspace and desktop.
