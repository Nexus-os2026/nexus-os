# Stage 4 — native, packaged-Builder, frontend and Python lanes

Every step below is a step of `.github/workflows/ci-fast-local.yml` at Candidate C (sha256 `583311ba…53a3`). Its exact
command is quoted in `04-FAST-LOCAL-MATRIX.tsv`. **None of these steps was executed by this mission, and no receipt
of any of them exists at Candidate C.** No step is described as equivalent to a local substitute.

The local inventory (`raw/04-local-tool-inventory.txt`, read-only version queries, 2026-10-16T15:06:27Z):

| Workflow requirement | Local machine | Consequence |
|---|---|---|
| Node.js 24.21.0 (`actions/setup-node`, Builder assembly; also the Node on PATH for later fast-linux steps) | `/usr/bin/node` v20.20.2; no version manager | unavailable without download or installation |
| Node.js "22" (fast-frontend) | v20.20.2 | unavailable |
| Python 3.11 (`actions/setup-python`) | python3 3.12.3; no python3.11 | unavailable |
| ripgrep 14.1.1 (installed by `cargo install` into the runner tool cache) | `/usr/bin/rg` 14.1.0 | not the pinned version; installing needs crates.io |
| xvfb-run; webkit2gtk-4.1 2.52.6, javascriptcoregtk-4.1 2.52.6, libsoup-3.0 3.4.4, gtk+-3.0 3.24.41, glib-2.0 2.80.0 | present | the system prerequisites exist |
| Runner tool cache `/home/github-runner/actions-runner/_work/.nexus-fast-cache` | owned by `github-runner`; permission denied | not usable, and not to be borrowed |

## fast-linux — native and packaged-Builder steps

| # | Step | Status | Exact reason |
|---|---|---|---|
| 10 | Live webview boundary harness (item D) | **NOT DISCHARGED** | Needs a dev-profile test build of `nexus-desktop-backend` (`--test webview_boundary_live -- --ignored` under `xvfb-run`). Its dependency closure is 983 of the 1,026 resolved packages, 58 of the 72 workspace members (`raw/04-desktop-backend-closure.txt`, offline `cargo metadata`), i.e. almost the whole workspace test build, which took 31,998,849,024 B in VALIDATION-02. After this mission's Clippy, 24,145,510,400 B were available; above the 16 GiB floor that leaves about 6.5 GiB. Not attempted. Reusing VALIDATION-02's retained target would modify another mission's evidence target and is not permitted. In Stage B, `webview_boundary_live` printed `ignored (pass -- --ignored to run)`. |
| 11 | Setup Node.js 24.21.0 | **NOT DISCHARGED** | The action downloads Node 24.21.0 (network); it is absent locally. |
| 12 | Assemble Nexus Builder toolchain (`node packaging/builder-toolchain/scripts/assemble.mjs --target x86_64-unknown-linux-gnu --out app/src-tauri/builder-toolchain`) | **NOT DISCHARGED** | Needs Node 24.21.0 and the pinned Node archive, whose digest the script checks. It would also write `app/src-tauri/builder-toolchain` into the checkout. |
| 13 | Trusted Builder entry tests (packaged Node) | **NOT DISCHARGED** | Depends on step 12's packaged Node. |
| 14 | Linux package inspection negative controls (`node --test packaging/verifier-toolchain/test/inspect-deb.test.mjs`) | **NOT DISCHARGED** | The workflow runs it with Node 24.21.0 on PATH. A run under the local Node 20.20.2 would be a different test environment and would not discharge the step, so it was not made. |
| 15 | Packaged Builder toolchain verification and governed launch (`NEXUS_BUILDER_TOOLCHAIN=packaged cargo test -p nexus-desktop-backend --locked --lib -- p0_002c4d2_assembled_ p0_002c4c3_packaged_`, expecting `test result: ok. 13 passed`) | **NOT DISCHARGED** | Depends on step 12, and needs a `nexus-desktop-backend` lib test build (the disk constraint of step 10). |
| 16 | Live webview boundary harness, release profile (item D) | **NOT DISCHARGED** | `cd app && npm ci && npm run build` (network), then `cargo test -p nexus-desktop-backend --locked --features tauri/custom-protocol --test webview_boundary_live -- --ignored` (disk). |

## fast-frontend job (Node 22)

| # | Step | Status | Exact reason |
|---|---|---|---|
| 18 | Checkout | NOT DISCHARGED | the job was not run |
| 19 | Verify the tested commit and a clean checkout | discharged by the same commands (`receipts/42-worktree`) | job-independent check on the same commit |
| 20 | Setup Node.js "22" | **NOT DISCHARGED** | download (network); absent locally |
| 21 | npm advisory gate tests (`node --test scripts/ci/npm-advisory-gate.test.mjs`) | **NOT DISCHARGED** | needs Node 22 |
| 22 | npm advisory gate (`node scripts/ci/npm-advisory-gate.mjs`) | **NOT DISCHARGED** | runs a fresh `npm audit` of every audited npm root against the registry (network) |
| 23 | Install frontend dependencies (`cd app && npm ci`) | **NOT DISCHARGED** | registry access (network). A ~2.0 GiB local npm cache exists, but `npm ci --offline` would be a different command. |
| 24 | TypeScript check (`npx tsc --noEmit`) | **NOT DISCHARGED** | depends on 20 and 23 |
| 25 | Run frontend tests (`npm test`) | **NOT DISCHARGED** | depends on 20 and 23 |
| 26 | Build frontend (`npx vite build`) | **NOT DISCHARGED** | depends on 20 and 23 |

## fast-python job (Python 3.11)

| # | Step | Status | Exact reason |
|---|---|---|---|
| 27 | Checkout | NOT DISCHARGED | the job was not run |
| 28 | Verify the tested commit and a clean checkout | discharged by the same commands (`receipts/42-worktree`) | job-independent |
| 29 | Setup Python 3.11 | **NOT DISCHARGED** | absent locally; the action downloads it |
| 30 | Install voice dependencies (`python -m venv …`; `pip install --require-hashes --no-deps -r voice/pip-bootstrap.lock` and `-r voice/requirements-linux-py311.lock`; `pip check`) | **NOT DISCHARGED** | package index access (network); Python 3.11 required |
| 31 | Run Python voice tests (`CUDA_VISIBLE_DEVICES="" … python -m pytest -v`) | **NOT DISCHARGED** | depends on 29 and 30 |

## What would discharge these steps (each needs an Owner decision; none is requested as a workaround)

1. A network boundary for exactly: the Node 24.21.0 and Node 22 downloads (or the pinned archive), the npm registry
   (`npm ci`, `npm audit`), the Python 3.11 download and the hash-pinned pip install, crates.io for ripgrep 14.1.1,
   and the two advisory-database fetches; **or** an Owner-authorised run of the workflow itself on the self-hosted
   runner (currently inactive and disabled) for this exact commit.
2. Disk capacity for the native lanes: roughly a whole-workspace test build (~32 GB measured in VALIDATION-02) above
   the 16 GiB floor, **or** an explicit authorisation to reuse or remove an existing build target (not given here).
3. Alternatively, a ruling on whether the full Fast Local gate is a D2A-1 module-acceptance requirement or a later
   Candidate 11 integration requirement (contract 3.1 places "the Fast Local gate on the exact commit, started by
   dispatch" in the D2A-1 mission's own process).
