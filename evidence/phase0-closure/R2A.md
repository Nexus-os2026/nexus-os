# P0-LINUX-FINAL-R2A: Architect review findings on R2

These are claims to verify, not approval. Nothing here declares the Final
Gate or Phase Zero complete. Windows and macOS stay deferred.

## Commit

| | |
|---|---|
| Start | `c9e64eee64a69f5a61167dedc4717f036a6fb50f` (tree `1baf6fc0`, parent `b8402283`) |
| Commit | `9126be753907a630fb38d721457fe1d88c78aefd` "test(security): close R2 architect review findings" |
| Tree | `e160e280f44da042d4d933d56e62f295bfcdde4d` |
| Parent | `c9e64eee64a69f5a61167dedc4717f036a6fb50f` |

Changed files (the three allowed):

- `app/src-tauri/src/phase0_surface/fg_reliability/tests.rs`
- `docs/security/phase0-final-gate-dossier.md`
- `docs/security/phase0-c5-authority-inventory.md`

No production, manifest, lockfile, workflow, policy or dependency file
changed (`git diff --name-only c9e64eee 9126be75` lists only these three).

## Finding A: cross-file Wasmtime alias bypass

The invariant now pinned by
`p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api`, while
RUSTSEC-2026-0316 is accepted:

- **No component module.** Production Nexus does not use Wasmtime's
  `component` module. Every R2 check remains: direct, spaced, multiline,
  grouped and nested paths, and `component as …` inside a group.
- **No crate alias or glob.** Production Nexus does not alias, rename or
  glob-import the wasmtime crate.
  - The alias declaration itself is refused, whether or not the file names
    `component`. This covers `[pub] use wasmtime as …`,
    `[pub] use wasmtime::{self as …}` and `[pub] extern crate wasmtime as …`,
    with raw identifiers, `::wasmtime`, `{wasmtime as …}` and `as _`
    included.
  - `wasmtime::*` and `wasmtime::{*}` are refused unconditionally.
- **No dependency rename.** No workspace package renames its `wasmtime`
  dependency.
  - This is read from Cargo's effective metadata, not from manifest text.
  - The command is `cargo metadata --no-deps --format-version 1 --locked
    --manifest-path <workspace>/Cargo.toml`, run with `env!("CARGO")`: the
    Cargo that built the test, not one the caller selects.
  - The guard checks the metadata before trusting it:
    - format version 1;
    - `workspace_root` equals this workspace;
    - `packages` covers every workspace member;
    - the `rename` field is present on each dependency entry;
    - at least one wasmtime dependency exists.
  - Today there are three: `nexus-sdk`, `nexus-protocols` (dev) and
    `nexus-benchmarks`, all unrenamed.
  - No dependency was added; `serde_json` is already a dependency of the
    desktop crate.
- **Core use allowed.** Direct core-Wasm use stays allowed:
  `Engine`/`Linker`/`Module`/`Store` and `get_typed_func`.
- **Scope of the claim.** This is a Nexus non-use and reachability
  invariant, not proof that wasmtime 43.0.2 is generally safe.

Probes that must be caught:

- **14 crate aliases:**
  - `use wasmtime as wt;`
  - `pub use wasmtime as wt;`
  - `pub(crate) use wasmtime as wt;`
  - `use wasmtime::{self as wt};`
  - `pub use wasmtime::{self as wt, Engine};`
  - `extern crate wasmtime as wt;`
  - `pub extern crate wasmtime as wt;`
  - `use r#wasmtime as wt;`
  - `pub use wasmtime as r#wt;`
  - `use ::wasmtime as wt;`
  - `use {wasmtime as wt};`
  - a multiline `pub use wasmtime as wt`
  - `#[cfg(any())] pub use wasmtime as wt;`
  - `use wasmtime as _;`
- **2 globs:** `pub use wasmtime::*;` and `use wasmtime::{Engine, *};`
- **15 R2 component-module spellings:** unchanged from R2.

Sources that must be accepted (9):

- the core import;
- `get_typed_func`;
- `wasmtime::Val::I32`;
- a line comment;
- a block comment;
- a commented-out alias;
- `mod my_component`;
- an item rename, `use wasmtime::{Engine as WasmEngine, Store};`
- `extern crate wasmtime;`

## Negative controls

Each control ran the built guard binary directly, then restored the file and
compared it byte for byte with `git show HEAD:<file>`.

| Control | Mutation | Result |
|---|---|---|
| NC5 | `#[cfg(any())]` `pub use wasmtime as wt;` appended to `sdk/src/wasm_agent.rs` | exit 101: `["alias: wasmtime as wt"] (the wasmtime crate is not aliased or renamed while RUSTSEC-2026-0316 is accepted)` |
| NC6 | `sdk/Cargo.toml` line 16, `wasmtime.workspace = true` → `wt = { package = "wasmtime", version = "43.0.2" }` (same package and version) | exit 101: `"nexus-sdk": wasmtime dependency renamed to "wt" (the wasmtime crate is not aliased or renamed …)` |
| NC1 (R2, rerun) | cfg-disabled `use wasmtime::{component::{Val}};` | exit 101: component module |
| NC2 (R2, rerun) | `use wasmtime::{Engine as _ProbeEngine, component as component_api};` | exit 101: component module |

- For NC6, `cargo metadata --locked` succeeded, and `Cargo.lock` stayed
  unchanged: a rename is not recorded in the lockfile.
- A clean run of the guard afterwards exited 0.
- After every control, `git status` showed only the three R2A files.
- The R2 vault controls NC3 and NC4 concern `fg_secrets/tests.rs`, which R2A
  does not change.

## Finding B: P0-FG1 current status

The current-status text now states directly, without bracketed corrections:
P0-FG1/J1 is integrated at `71c47acb`, its designated post-integration
hosted validation (run #108) passed, J1 does not await integration or
Architect review, and P0-FG1-R1's duplicate-name repair is integrated.

**Dossier:**
- the header paragraph;
- the Summary row J;
- the item J heading and "Status";
- the Architect decision's build-debt clause ("was then build debt …
  repaired since");
- the naming-repair heading;
- "Decision needed", rewritten as "Decisions (as asked at C5C, and their
  current state)";
- the `nexus-ui-repair` repair;
- the duplicate binary name (heading, and a "Status" replacing "the item
  stays open until …");
- the R2 correction note.

**Inventory:**
- §10.6;
- the §10.8 shipped list ("Shipped (at C5C)", the protocols rename, "At C5C
  … was a Final-Gate blocker", P0-FG1);
- the §10.10 repair point 4 ("Unchanged (at C5C)");
- the §10.10 classification: J1 moved to "Resolved since C5C", and the
  remaining list headed "Unresolved Final-Gate items at C5C";
- §11.2 row J;
- §11.6 (no longer says FG1 is "not declared complete");
- §11.9.

Historical statements kept deliberately, because they are labelled history:
- the dossier's "At C5C (historical)" table row J ("**Blocker** at C5C",
  with where it stands now);
- "Before P0-FG1";
- the dated hosted-run descriptions (#103–#106);
- the §10.8 C5C recount;
- the Architect's P0-FG1 Windows-fallback decision.

J2–J5 remain not declared complete until the closure candidate is integrated
and validated.

A mechanical scan of both documents found no remaining current-state
statement that P0-FG1 is review or integration pending, that P0-FG1-R1 is
repair or integration pending, or that J1 is an unresolved blocker. The scan
looked for any paragraph naming FG1, J1 or `nexus-server` that also says
pending, blocker or validation branch. Every remaining hit is labelled "at
C5C", or is the non-claim "not a Linux Phase Zero blocker" for request 7.

The Wasmtime claims in both documents are narrowed to the invariant above.
The earlier wording, "in any spelling … an alias followed by `component`",
was replaced.

## Validation

Local, before the commit:

- `git diff --check` clean; `cargo fmt --all -- --check` clean.
- clippy on the desktop crate (`--all-targets --all-features -D warnings`)
  clean.
- The strengthened guard and the final trust-surface guard pass.
- `cargo test --workspace --locked -- p0_fg p0_r1 p0_002c5c`: 297 passed, 0
  failed.

Fast-local run `36599471858`, #11, attempt 1:

- event `push`; tested commit
  `9126be753907a630fb38d721457fe1d88c78aefd`, confirmed by the verify step in
  each job; conclusion **success**.
- **fast-linux**, steps 1–17 all successful:
  - fmt and clippy (`--workspace --all-targets --all-features -D warnings`);
  - `cargo test --workspace --locked` (unfiltered): 293 summaries, 7684
    passed, 0 failed, 43 ignored;
  - the live webview boundary harness, dev and release (`webview_boundary_live
    ... ok` twice);
  - the Builder assembly, trusted-entry JS (21 of 21) and the packaged
    Builder gate (13 passed);
  - the security gate: cargo-audit 0.22.1 and cargo-deny 0.19.6, 11 accepted
    IDs, `advisories ok, bans ok, licenses ok, sources ok`,
    `security-audit: passed`.
- **fast-frontend:** vitest 461 of 461 (103 files); tsc clean; the vite build
  succeeds.
- **fast-python:** the hash-pinned voice lock installs, and the voice tests
  run 27, OK.

No hosted run was dispatched (neither `ci.yml` nor `ci-portability.yml`),
and #110 was not rerun.

## Totals recomputed from fast-local #11

| Selection | Count |
|---|---|
| `p0_fg_*` | 231 |
| `p0_fg1_*` | 11 |
| `p0_r1_*` | 8 |
| `p0_002c5c_*` | 47 |
| `phase0_surface::fg_*` modules | 82 (`fg_approval` 17, `fg_egress` 18, `fg_reliability` 13, `fg_secrets` 11, `fg_standalone` 12, `fg_webview` 11) |
| Packaged Builder gate (separate step) | 13 |

- 231 `p0_fg_*` + 13 packaged = 244 selected checks, all passed.
- Every test that ran passed: 0 `FAILED` lines.
- The strengthened Wasmtime guard, the vault-scope guard and the final
  trust-surface guard each report `ok`.

## Final refs (github)

| Ref | SHA |
|---|---|
| `repair/p0-linux-final-closure-r2` | `9126be753907a630fb38d721457fe1d88c78aefd` (c9e64eee, then 9126be75) |
| `rebuild/phase0-trust-boundary` (authoritative) | `71c47acbf3f8ee8210109587b3229f8d89067b6b` (unchanged) |
| `main` | `80640bba41e74c17abbf4eb71eafa88bd1ade8db` (unchanged) |

No PR was opened, nothing was merged, and nothing was amended, rebased,
squashed or force-pushed.
