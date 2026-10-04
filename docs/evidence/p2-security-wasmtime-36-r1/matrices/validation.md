# Validation

## Environment

The runs used an isolated snapshot: the base `a7001311` plus the candidate's
four changed files, in a fresh scratch repository, tree
`58afc152e021d5a02878548d87b419f0f4c2235d` (`scope/validated-snapshot.txt`).
`scripts/validate.sh` mirrors `ci.yml`'s `test-linux`, `test-frontend` and
`test-python` lanes. Every command's exit is in `validation/commands.txt`
and its output in `validation/<step>.txt`. The copies are normalized
(ANSI and trailing whitespace stripped); each copy's raw and normalized
SHA-256 is in `validation/normalization.tsv`.

Toolchain: Rust 1.94.0 (`validation/toolchain.txt`). For the Builder
assembly only: the repository-pinned Node 24.21.0 and npm 11.19.0. The
official `node-v24.21.0-linux-x64.tar.gz` was verified against the SHA-256
pinned in `packaging/builder-toolchain/node-release.json` and used from the
work directory (CI uses `actions/setup-node`).

## Results

| Step | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | exit 0 |
| clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | exit 0, no warning |
| check (worktree, before the guard edit was final) | `cargo check --workspace --all-targets --all-features --locked` | exit 0, no warning (`validation/workspace-check-all-features.txt`); first compile against 36.0.17, with no source change |
| workspace tests | `cargo test --workspace --locked --exclude nexus-verifier-sandbox` | exit 0; 293 test binaries, 7,925 passed, 0 failed, 43 ignored |
| verifier sandbox, non-live | `cargo test -p nexus-verifier-sandbox --locked --lib --bins --test` (6 non-live targets) | exit 0; 8 binaries, 421 passed |
| live harness | `... --features development-toolchain --test phase2_live_sandbox --no-run` | built, not run |
| SDK (all features) | `cargo test -p nexus-sdk --locked --all-features` | exit 0; 220 passed (lib, `wasmtime_integration_tests`, `speculative_shadow_tests`, doc) |
| protocols | `cargo test -p nexus-protocols --locked` | exit 0; 135 passed (includes the Wasmtime engine test) |
| benchmarks | `cargo bench -p nexus-benchmarks --locked --no-run` | exit 0 (the Wasmtime bench builds) |
| Wasmtime guard | `... --lib -- phase0_surface::fg_reliability::tests::p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api --exact` | exit 0; 1 passed |
| Phase 0/1/2 guards | `cargo test -p nexus-desktop-backend --locked --lib -- phase0_surface phase1_tests phase2_tests coding_flow::verification` | exit 0; 140 passed |
| kernel Phase 2 | `cargo test -p nexus-kernel --locked --lib -- p2b_ p2e_input p2h_` | exit 0; 26 passed |
| webview boundary, dev | `xvfb-run ... cargo test -p nexus-desktop-backend --locked --test webview_boundary_live -- --ignored` | exit 0; `[live] tauri profile: dev`, `test webview_boundary_live ... ok` |
| Builder assembly | `node packaging/builder-toolchain/scripts/assemble.mjs --target x86_64-unknown-linux-gnu --out app/src-tauri/builder-toolchain` | exit 0 |
| Builder entry tests | `app/src-tauri/builder-toolchain/node/node --test packaging/builder-toolchain/test/entry.test.mjs` | exit 0 |
| packaged Builder toolchain | `NEXUS_BUILDER_TOOLCHAIN=packaged cargo test -p nexus-desktop-backend --locked --lib -- p0_002c4d2_assembled_ p0_002c4c3_packaged_` | exit 0; 13 passed (CI's required count) |
| frontend | `cd app && npm ci`; `npx tsc --noEmit`; `npm test`; `npx vite build` | exit 0 each; 105 test files, 483 tests passed; built |
| webview boundary, release | `(cd app && npm run build)`, then `xvfb-run ... --features tauri/custom-protocol --test webview_boundary_live -- --ignored` | exit 0; `[live] tauri profile: release`, `... ok` |
| Python voice | `pip install --require-hashes --no-deps -r voice/requirements-linux-py311.lock`; `pytest` | NOT RUN (see below) |

## Deviations, disclosed

- **The verifier sandbox's live suite was not run locally.**
  `phase2_live_sandbox`'s first step probes this host and, where the sandbox
  works, runs live cases that create transient systemd scopes. This host ran
  the full live suite for the runner account. Running it here would create
  scopes in the user manager, which this mission did not authorize. The crate
  is not in Wasmtime's graph (`audit/after-wasmtime-dependents.txt`). Its
  non-live targets ran and the live harness was built.
  - The counts reconcile with closure CI's `test-linux` on the base. CI's
    workspace test run reported 8,385 tests, including
    "39 live sandbox cases passed" on the hosted runner (the verifier
    doc-tests have none). Locally: 7,925 + 421 = 8,346 = 8,385 − 39.
- **The packaged Builder step was rerun.** The first run
  (`validation/builder-packaged-toolchain.first-run-without-switch.txt`)
  omitted CI's `NEXUS_BUILDER_TOOLCHAIN=packaged`, so the packaged tests
  were not compiled in and 0 ran. It was rerun with the switch, as CI sets
  it: 13 passed. `scripts/validate.sh` now carries the switch and the
  13-passed check. The first runner script also piped the webview
  harnesses into `grep -q`; it was stopped after `fmt` and fixed before the
  run recorded here.
- **The Python voice lane was not run locally.** Its locked requirements
  need Python 3.11 (`tflite-runtime==2.14.0` has no Python 3.12
  distribution: `validation/python-voice.txt`). This host has only 3.12.3,
  and no Python was installed for this mission. This candidate changes no
  Python input (`voice/` and its lockfile are unchanged). The lane passed
  in closure run 37191730167 on the base. GitHub-hosted CI is its gate for
  this candidate.

## Controls

`matrices/controls.md`.
