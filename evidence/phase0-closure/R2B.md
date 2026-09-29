# P0-LINUX-FINAL-R2B: raw-identifier Wasmtime component path

This note is evidence and claims for Architect review. It is not approval,
and it is not Phase Zero or Final Gate completion. Windows and macOS stay
deferred.

## Commit

| | |
|---|---|
| Start | `9126be753907a630fb38d721457fe1d88c78aefd` (tree `e160e280`, parent `c9e64eee`) |
| Commit | `f4ef200f6f2c1f5f4af9d635cbd3c99eacaba0e4` "test(security): close raw Wasmtime component path bypass" |
| Tree | `b67badf943a845599d84ff6bb7868c165f2dfce4` |
| Parent | `9126be753907a630fb38d721457fe1d88c78aefd` |

Changed file: `app/src-tauri/src/phase0_surface/fg_reliability/tests.rs`
only (`git diff --name-only 9126be75 f4ef200f`). No production,
documentation, manifest, lockfile, workflow, policy or dependency file
changed. The two security documents are unchanged; their invariant already
forbids the component module.

## The bypass

Rust raw identifiers let `r#component` name the same item as `component`. In
R2A, the direct-path check in `wasmtime_forbidden_uses` matched only
`rest.starts_with("::component")`. So `wasmtime::r#component::Val`,
`use wasmtime::r#component as component_api;` and
`::wasmtime::r#component::Func` were not classified as the component module.

The group checks were not affected, because in `r#component` the word
`component` follows `#`, which is a word boundary.

## Repair

A bounded helper replaces that one condition:

```rust
fn starts_with_component_segment(rest: &str) -> bool {
    ["::component", "::r#component"].iter().any(|segment| {
        rest.strip_prefix(segment)
            .is_some_and(|after| !after.bytes().next().is_some_and(ident_byte))
    })
}
```

- The helper matches the exact segment after a `wasmtime` root and checks the
  identifier boundary after it.
- `r#` is not stripped anywhere else, so raw strings are unaffected.
- All R2 and R2A behaviour is unchanged:
  - component paths, groups and aliases inside a group fail;
  - crate aliases and globs fail;
  - Cargo-metadata dependency renames fail;
  - core use is accepted;
  - comments are ignored;
  - `cfg`-disabled code stays in scope;
  - no parser or dependency was added.

## New probes

A new probe loop requires the `component:` rule for each of these raw
spellings:

- `use wasmtime::r#component::Val;`
- `fn f() { let _: wasmtime::r#component::Val = todo!(); }`
- `use ::wasmtime::r#component as component_api;`
- `use r#wasmtime::r#component::Func;`
- `use wasmtime :: r#component :: Val;`
- a multiline `wasmtime :: r#component :: Val`
- `#[cfg(any())]` followed by `use wasmtime::r#component::Val;`
- `use wasmtime::{r#component::Val};`
- `use wasmtime::{Engine, r#component as c};`

New accepted boundary case: `let x = wasmtime::r#componentx;`.

Every R2 and R2A probe is kept: 14 crate aliases, 2 globs and 15 component
spellings caught, and 9 accepted sources, now 10.

## NC7

Mutation: `#[cfg(any())]` followed by `use wasmtime::r#component::Val;`,
appended to `sdk/src/wasm_agent.rs`.

- The guard binary was built from the corrected source and run directly.
- **Result:** exit 101, `test result: FAILED`. The message was
  `…/sdk/src/wasm_agent.rs: ["component: wasmtime::component"] (Wasmtime's
  component module is not used while RUSTSEC-2026-0316 is accepted)`.
- **Restoration:** SHA-256 before the mutation was `1ed55dda…a07045`, which
  equals `git show HEAD:sdk/src/wasm_agent.rs`. With the mutation it was
  `fd183d82…cf3bb64`. After restoration it was `1ed55dda…a07045`, and `cmp`
  against Git matches.
- After restoration, a clean guard run exited 0.
- `Cargo.lock`'s SHA-256 is unchanged.
- `git status` showed only the test file.

## Local validation

- `git diff --check` and `cargo fmt --all -- --check`: clean.
- `cargo clippy -p nexus-desktop-backend --all-targets --all-features --locked -- -D warnings`:
  clean (the R2A form).
- The strengthened Wasmtime guard and the final trust-surface guard pass.
- `cargo test --workspace --locked -- p0_fg p0_r1 p0_002c5c`: 297 passed and
  0 failed.
  - This is 231 `p0_fg_*`, 11 `p0_fg1_*`, 8 `p0_r1_*` and 47 `p0_002c5c_*`.
  - It equals the R2A baseline, because no test function was added.

## Fast-local run

Run `36606156626`, #12, attempt 1:

- event `push`; tested commit `f4ef200f6f2c1f5f4af9d635cbd3c99eacaba0e4`,
  confirmed by the verify step in each job; conclusion **success**.
- **fast-linux** (success; steps 8–17 all successful):
  - fmt and clippy;
  - `cargo test --workspace --locked`: 7684 passed, 0 failed, 43 ignored, 0
    `FAILED` lines;
  - `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` and
    `p0_002c5c_final_trust_surface_guard_is_complete` both `ok`;
  - 231 `p0_fg_*`, 11 `p0_fg1_*`, 8 `p0_r1_*` and 47 `p0_002c5c_*`, all
    passed;
  - the live webview harness, dev and release (`ok` twice);
  - the trusted-entry JS tests (21 of 21) and the packaged Builder gate
    (13 passed);
  - the security gate: cargo-audit 0.22.1 and cargo-deny 0.19.6,
    `advisories ok, bans ok, licenses ok, sources ok`,
    `security-audit: passed`.
- **fast-frontend** (success): vitest 461 of 461 (103 files); tsc clean.
- **fast-python** (success): voice tests 27, OK.

## Refs (github)

| Ref | SHA |
|---|---|
| `repair/p0-linux-final-closure-r2` | `f4ef200f6f2c1f5f4af9d635cbd3c99eacaba0e4` |
| `rebuild/phase0-trust-boundary` (authoritative) | `71c47acbf3f8ee8210109587b3229f8d89067b6b` (unchanged) |
| `main` | `80640bba41e74c17abbf4eb71eafa88bd1ade8db` (unchanged) |

- No hosted CI was dispatched.
- No PR was opened, and nothing was merged or integrated.
- Nothing was amended, rebased, squashed or force-pushed.
- The phase was not advanced.
