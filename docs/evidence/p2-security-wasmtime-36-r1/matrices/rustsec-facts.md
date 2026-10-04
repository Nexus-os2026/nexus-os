# RustSec facts for Wasmtime 36.0.17

## Sources

- RustSec advisory database: a fresh clone of `https://github.com/RustSec/advisory-db`
  at commit `ef6173cbc5c50ec8166f9a5b28f07834144373ee` (2026-10-03T10:14:03+02:00).
  This is the same commit both scanners fetched for the final security gate
  (`security/final-security-audit.txt`).
- crates.io version metadata for `wasmtime` (`security/crates-io-wasmtime.json`,
  the two versions' entries).

## Wasmtime 36.0.17 (crates.io)

| Fact | Value |
|---|---|
| MSRV (`rust_version`) | 1.86.0 (the workspace pins Rust 1.94.0) |
| Yanked | no |
| Published | 2026-10-02T16:48:19Z, with 49.0.2 (the coordinated RUSTSEC-2026-0327 release) |
| Newest 36.x | yes (36.0.0 to 36.0.17 exist) |
| Checksum | `0707f327a5821aa76c254fa661bd582d6e209c3176b6cca1d87484b2338d2dbb` (equal to the `Cargo.lock` entry) |
| License | Apache-2.0 WITH LLVM-exception (allowed by `deny.toml`) |

## The advisories the mission names

| Advisory | Patched | Unaffected | 36.0.17 |
|---|---|---|---|
| RUSTSEC-2026-0327 (Wasmtime component async-lifted callback result count is unvalidated, causing a native stack buffer overflow) | >= 48.0.4, < 49.0.0; >= 49.0.2 | < 39.0.0 | unaffected |
| RUSTSEC-2026-0222 (Stores can mix up type indices between engines) | >= 24.0.12, < 25.0.0; >= 36.0.13, < 37.0.0; >= 46.0.2, < 47.0.0; >= 47.0.3 | — | patched |
| RUSTSEC-2026-0269 (Filesystem sandbox escape when paths or symlinks contain trailing slashes) | >= 24.0.13, < 25.0.0; >= 36.0.14, < 37.0.0; >= 46.0.3, < 47.0.0; >= 47.0.4 | — | patched |
| RUSTSEC-2026-0316 (Dynamic record lifting can allocate beyond the hostcall fuel limit) | >= 36.0.16, < 37.0.0; >= 48.0.3, < 49.0.0; >= 49.0.1 | — | patched |
| RUSTSEC-2026-0114 (Panic when allocating a table exceeding the size of the host's address space) | >= 36.0.8, < 37.0.0; >= 43.0.2, < 44.0.0; >= 44.0.1 | < 30.0.0 | patched |

## Every RustSec advisory for the `wasmtime` crate

`scripts/eval_wasmtime_advisories.py` evaluates each advisory's `[versions]`
ranges against a version. Run against 43.0.2, it reports exactly the
four advisories the fresh scanners report for 43.0.2: RUSTSEC-2026-0222,
-0269, -0316 (accepted in the base `deny.toml`) and -0327 (the closure
failure). That match is the evaluator's own check
(`security/wasmtime-43.0.2-advisories.json`). Against 36.0.17 it reports
none of the 49 (`security/wasmtime-36.0.17-advisories.json`).

This covers the `wasmtime` crate only. The whole graph (every Wasmtime-family
crate: cranelift, pulley, winch, wasmtime-internal-*, wasm and wit crates) is
covered by the fresh scanners (`matrices/security-gates.md`): no finding names
any of them.

| Advisory | Date | 36.0.17 | Patched | Unaffected | Title |
|---|---|---|---|---|---|
| RUSTSEC-2021-0110 | 2021-09-17 | patched | >= 0.30.0 | — | Multiple Vulnerabilities in Wasmtime |
| RUSTSEC-2022-0016 | 2022-03-31 | patched | >= 0.35.2; >= 0.34.2, < 0.35.0 | < 0.34.0 | Use after free with `externref`s and epoch interruption in Wasmtime |
| RUSTSEC-2022-0075 | 2022-11-10 | patched | >= 1.0.2, < 2.0.0; >= 2.0.2 | — | Bug in pooling instance allocator |
| RUSTSEC-2022-0076 | 2022-11-10 | patched | >= 1.0.2, < 2.0.0; >= 2.0.2 | — | Bug in Wasmtime implementation of pooling instance allocator |
| RUSTSEC-2022-0095 | 2022-06-27 | patched | >= 0.38.1 | — | Miscompilation of `i8x16.swizzle` and `select` with v128 inputs |
| RUSTSEC-2022-0096 | 2022-02-17 | patched | >= 0.33.1, < 0.34.0; >= 0.34.1 | — | Invalid drop of VMExternRef from partially-initialized instances in the pooling instance allocator |
| RUSTSEC-2022-0097 | 2022-11-07 | patched | >= 1.0.2, < 2.0.0; >= 2.0.2 | — | Out of bounds write in `wasmtime_trap_code` C API function |
| RUSTSEC-2022-0098 | 2022-11-05 | patched | >= 1.0.2, < 2.0.0; >= 2.0.2 | — | Data leakage between instances in the pooling allocator |
| RUSTSEC-2022-0099 | 2022-03-28 | patched | >= 0.35.2; >= 0.34.2, < 0.35.0 | — | Use after free with `externref`s and epoch interruption in Wasmtime |
| RUSTSEC-2022-0100 | 2022-07-12 | patched | >= 0.38.2 | — | Use After Free with `externref`s in Wasmtime |
| RUSTSEC-2022-0101 | 2022-07-05 | patched | >= 0.38.2 | — | Miscompilation of constant values in division on AArch64 |
| RUSTSEC-2022-0102 | 2022-11-05 | patched | >= 1.0.2, < 2.0.0; >= 2.0.2 | — | Out of bounds read/write with zero-memory-pages configuration |
| RUSTSEC-2023-0090 | 2023-03-02 | patched | >= 4.0.1, < 5.0.0; >= 5.0.1, < 6.0.0; >= 6.0.1 | — | Guest-controlled out-of-bounds read/write on x86\_64 |
| RUSTSEC-2023-0091 | 2023-09-05 | patched | >= 10.0.2, < 11.0.0; >= 11.0.2, < 12.0.0; >= 12.0.2 | — | Miscompilation of wasm `i64x2.shr_s` instruction with constant input on x86\_64 |
| RUSTSEC-2023-0092 | 2023-04-21 | patched | >= 6.0.2, < 7.0.0; >= 7.0.1, < 8.0.0; >= 8.0.1 | — | Undefined Behavior in Rust runtime functions |
| RUSTSEC-2023-0093 | 2023-03-03 | patched | >= 4.0.1, < 5.0.0; >= 5.0.1, < 6.0.0; >= 6.0.1 | — | Miscompilation of `i8x16.select` with the same inputs on x86\_64 |
| RUSTSEC-2024-0438 | 2024-11-02 | patched | >= 24.0.2, < 25.0.0; >= 25.0.3, < 26.0.0; >= 26.0.1 | — | Wasmtime doesn't fully sandbox all the Windows device filenames |
| RUSTSEC-2024-0439 | 2024-10-03 | patched | >= 21.0.2, < 22.0.0; >= 22.0.1, < 23.0.0; >= 23.0.3, < 24.0.0; >= 24.0.1, < 25.0.0; >= 25.0.2 | < 19.0.0 | Race condition could lead to WebAssembly control-flow integrity and type safety violations |
| RUSTSEC-2024-0440 | 2024-10-02 | patched | >= 21.0.2, < 22.0.0; >= 22.0.1, < 23.0.0; >= 23.0.3, < 24.0.0; >= 24.0.1, < 25.0.0; >= 25.0.2 | < 21.0.0 | Runtime crash when combining tail calls with stack traces |
| RUSTSEC-2024-0441 | 2024-04-02 | patched | >= 19.0.1 | < 19.0.0 | Panic when using a dropped extenref-typed element segment |
| RUSTSEC-2025-0046 | 2025-07-18 | patched | >= 34.0.2; >= 33.0.2, < 34.0.0; >= 24.0.4, < 25.0.0 | < 10.0.0 | Host panic with `fd_renumber` WASIp1 function |
| RUSTSEC-2025-0112 | 2025-07-18 | unaffected | >= 38.0.3 | < 38.0.0 | Possible host crash with host-to-wasm component intrinsics |
| RUSTSEC-2025-0118 | 2025-11-11 | patched | >= 38.0.4; >= 37.0.3, < 38.0.0; >= 36.0.3, < 37.0.0; >= 24.0.5, < 25.0.0 | — | Unsound API access to a WebAssembly shared linear memory |
| RUSTSEC-2026-0006 | 2026-01-26 | patched | >= 41.0.1; >= 40.0.3, < 41.0.0; >= 36.0.5, < 37.0.0; < 29.0.0 | — | Wasmtime segfault or unused out-of-sandbox load with `f64.copysign` operator on x86-64 |
| RUSTSEC-2026-0020 | 2026-02-24 | patched | >= 24.0.6, < 25.0.0; >= 36.0.6, < 37.0.0; >= 40.0.4, < 41.0.0; >= 41.0.4 | — | Guest-controlled resource exhaustion in WASI implementations |
| RUSTSEC-2026-0021 | 2026-02-24 | patched | >= 24.0.6, < 25.0.0; >= 36.0.6, < 37.0.0; >= 40.0.4, < 41.0.0; >= 41.0.4 | — | Panic adding excessive fields to a `wasi:http/types.fields` instance |
| RUSTSEC-2026-0022 | 2026-02-24 | unaffected | >= 40.0.4, < 41.0.0; >= 41.0.4 | < 39.0.0 | Panic when dropping a `[Typed]Func::call_async` future |
| RUSTSEC-2026-0085 | 2026-04-09 | patched | >= 24.0.7, < 25.0.0; >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Panic when lifting `flags` component value |
| RUSTSEC-2026-0086 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Host data leakage with 64-bit tables and Winch |
| RUSTSEC-2026-0087 | 2026-04-09 | patched | >= 24.0.7, < 25.0.0; >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Wasmtime segfault or unused out-of-sandbox load with `f64x2.splat` operator on Cranelift x86-64 |
| RUSTSEC-2026-0088 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Data leakage between pooling allocator instances |
| RUSTSEC-2026-0089 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Host panic when Winch compiler executes `table.fill` |
| RUSTSEC-2026-0090 | 2026-04-09 | unaffected | >= 43.0.1 | < 43.0.0 | Use-after-free bug after cloning `wasmtime::Linker` |
| RUSTSEC-2026-0091 | 2026-04-09 | patched | >= 24.0.7, < 25.0.0; >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Out-of-bounds write or crash when transcoding component model strings |
| RUSTSEC-2026-0092 | 2026-04-09 | patched | >= 24.0.7, < 25.0.0; >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Panic when transcoding misaligned component model UTF-16 strings |
| RUSTSEC-2026-0093 | 2026-04-09 | patched | >= 24.0.7, < 25.0.0; >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Heap OOB read in component model UTF-16 to latin1+utf16 string transcoding |
| RUSTSEC-2026-0094 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Improperly masked return value from `table.grow` with Winch compiler backend |
| RUSTSEC-2026-0095 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Wasmtime with Winch compiler backend may allow a sandbox-escaping memory access |
| RUSTSEC-2026-0096 | 2026-04-09 | patched | >= 36.0.7, < 37.0.0; >= 42.0.2, < 43.0.0; >= 43.0.1 | — | Miscompiled guest heap access enables sandbox escape on aarch64 Cranelift |
| RUSTSEC-2026-0114 | 2026-04-30 | patched | >= 36.0.8, < 37.0.0; >= 43.0.2, < 44.0.0; >= 44.0.1 | < 30.0.0 | Panic when allocating a table exceeding the size of the host's address space |
| RUSTSEC-2026-0222 | 2026-07-31 | patched | >= 24.0.12, < 25.0.0; >= 36.0.13, < 37.0.0; >= 46.0.2, < 47.0.0; >= 47.0.3 | — | Stores can mix up type indices between engines |
| RUSTSEC-2026-0223 | 2026-07-31 | unaffected | >= 46.0.2, < 47.0.0; >= 47.0.3 | < 46.0.0 | Preemption and traps during bulk operations enable breaking internal VM state |
| RUSTSEC-2026-0268 | 2026-08-20 | unaffected | >= 46.0.3, < 47.0.0; >= 47.0.4 | < 46.0.0 | Guest controlled-size host heap allocation through WASIp3 streams |
| RUSTSEC-2026-0269 | 2026-08-20 | patched | >= 24.0.13, < 25.0.0; >= 36.0.14, < 37.0.0; >= 46.0.3, < 47.0.0; >= 47.0.4 | — | Filesystem sandbox escape when paths or symlinks contain trailing slashes |
| RUSTSEC-2026-0315 | 2026-09-24 | unaffected | >= 48.0.3, < 49.0.0; >= 49.0.1 | < 47.0.0 | `call_ref` and exception `catch` can drop some fuel accounting, leading to exponential fuel amplification |
| RUSTSEC-2026-0316 | 2026-09-24 | patched | >= 36.0.16, < 37.0.0; >= 48.0.3, < 49.0.0; >= 49.0.1 | — | Dynamic record lifting can allocate beyond the hostcall fuel limit |
| RUSTSEC-2026-0325 | 2026-10-02 | unaffected | >= 48.0.4, < 49.0.0; >= 49.0.2 | < 47.0.0 | Mis-typed WebAssembly tag imports can lead to GC heap corruption |
| RUSTSEC-2026-0326 | 2026-10-02 | unaffected | >= 48.0.4, < 49.0.0; >= 49.0.2 | < 47.0.0 | Rooting for GC values live across `try_call` may be missing, causing GC heap corruption |
| RUSTSEC-2026-0327 | 2026-10-02 | unaffected | >= 48.0.4, < 49.0.0; >= 49.0.2 | < 39.0.0 | Wasmtime component async-lifted callback result count is unvalidated, causing a native stack buffer overflow |

Affected on 43.0.2: RUSTSEC-2026-0222, RUSTSEC-2026-0269, RUSTSEC-2026-0316, RUSTSEC-2026-0327. Affected on 36.0.17: none.

## No new HIGH or CRITICAL finding

No advisory in the fresh database matches 36.0.17. The fresh `cargo audit`,
run with no exceptions over the repaired lockfile, reports exactly the five
non-Wasmtime vulnerabilities that `deny.toml` still accepts, and no
Wasmtime-family finding of any kind (`security/final-cargo-audit-no-ignores.json`).
The "BLOCKED — WASMTIME 36.0.17 IS NOT AN ACCEPTABLE SECURITY TARGET" stop
does not apply.
