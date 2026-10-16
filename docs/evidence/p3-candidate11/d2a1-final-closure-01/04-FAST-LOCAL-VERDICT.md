# Stage 4 — Fast Local verdict

## FAST_LOCAL_NOT_FULLY_DISCHARGED

The Fast Local gate is the whole of `.github/workflows/ci-fast-local.yml` at Candidate C (sha256 `583311ba…53a3`): 31
steps in three jobs (`04-FAST-LOCAL-MATRIX.tsv`). Contract 3.1 (line 125, not superseded by AD-1-R4 or AD-1-R5)
places "the Fast Local gate on the exact commit, started by dispatch" in D2A-1's process. No dispatch was authorised
or made.

### Discharged or evidenced at exact Candidate C

| Step | Evidence | Note |
|---|---|---|
| Verify the tested commit and a clean checkout (all three jobs) | this mission, `receipts/42-worktree` | the workflow's own commands |
| Rust 1.94.0 present; toolchain and Tauri system libraries | `receipts/42-worktree` | the workflow's own commands |
| Check formatting: `cargo fmt --all -- --check` | CLOSURE-PACKET-01 receipt (reused; exact command, exact source) | PASS |
| Run clippy: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **this mission**, `receipts/43b-clippy-all-features-r2` | **PASS**, 0 warnings — the previously missing exact Rust check |
| Run tests: `cargo test --workspace --locked` | VALIDATION-02 Stage B (reused; exact command, exact source) | PASS, 8,764 / 0 / 78 — **with an environment deviation:** ripgrep 14.1.0 on PATH, not the workflow's 14.1.1 |

### Not discharged (exact missing requirements)

| # | Step | Missing requirement |
|---|---|---|
| 6 | Provide ripgrep 14.1.1 | crates.io access for `cargo install ripgrep --version 14.1.1 --locked` (network), or a provided 14.1.1 binary |
| 10 | Live webview boundary harness (dev) | ~whole-workspace test-build disk (31,998,849,024 B measured for VALIDATION-02's target) above the 16 GiB floor; xvfb is present |
| 11–15 | Node 24.21.0; Builder assembly; trusted entry tests; package inspection; packaged Builder verification | Node 24.21.0 and its pinned archive (network); then desktop-backend lib-test disk |
| 16 | Live webview harness, release profile | `npm ci` (network), `npm run build`, custom-protocol desktop-backend test-build disk |
| 17 | Security gate (cargo-audit 0.22.1, cargo-deny 0.19.6) | fresh advisory-database fetches (network); scanners present at the pinned versions |
| 18, 20–26 | the fast-frontend job | Node 22 (network), `npm audit` and `npm ci` (network), then tsc, vitest and vite build |
| 27, 29–31 | the fast-python job | Python 3.11 (network) and the hash-pinned pip install (network), then pytest |

Five Rust/checkout steps and four verification steps are evidenced; the native, packaged-Builder, security, frontend
and Python lanes are not. **No partial suite is called a full PASS.** No workflow log or status was reproduced or
simulated.

### Owner decision required to discharge the remainder

One of:

1. **Authorise the Fast Local workflow itself** on the self-hosted runner (inactive and disabled now) for the exact
   commit `d5964ee5…`, as contract 3.1 describes (dispatch in Manual permission mode); or
2. **Authorise a bounded network boundary** (Node 24.21.0 and 22 downloads, the npm registry, Python 3.11 and pip,
   crates.io for ripgrep, the two advisory databases) **and the disk** for the native lanes (about 32 GB above the
   16 GiB floor, which needs either freed capacity or an explicit decision to remove or reuse an existing target); or
3. **Rule** that the full Fast Local gate belongs to Candidate 11 integration rather than to D2A-1 module acceptance,
   and record what D2A-1 acceptance requires instead.

Until one of these is decided, Fast Local remains **NOT FULLY DISCHARGED**.
