# Stage 4 — Rust validation at Candidate C

Source: Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`.
Workflow: `.github/workflows/ci-fast-local.yml` at C, blob `fa4f732535701d9b3d67a92593eea0d303df2a83`, sha256
`583311baaedd2b02320539e48a1949305e1a7a38e1dbec065e4f3174d36753a3`. Toolchain: rustc 1.94.0 (4a4ef493e 2026-03-02),
cargo 1.94.0 (85eff7c80 2026-01-15), clippy 0.1.94, rustfmt 1.8.0-stable. `Cargo.lock` at C: blob `1af53a60`, sha256
`6ed84a9aeba4a542b1820c53d4ba0f4899cdfa819380aaa8649c3b5d31add979` (unchanged; nothing was resolved or updated).

## 1. The four Rust steps of the fast-linux job

| Workflow step | Exact command | Evidence | Result | Exact-source identity | Deviations from the workflow (reported, not called equivalent) |
|---|---|---|---|---|---|
| Check formatting | `cargo fmt --all -- --check` | CLOSURE-PACKET-01 `FAST-LOCAL-3.1-RECEIPT.txt`, `receipts/50-fmt-gate-and-run.json`, `52-time-v.txt`, `53`/`54` (packet MANIFEST `372d1bc1…7f3f`, verified 48/48) | **PASS**: exit 0; stdout and stderr empty (0 bytes each); 4.38 s | worktree VALIDATION-02 byte-exact to C before and after (5,364 tracked entries, 97,246,341 bytes; tracked-stat files identical) | `env -i` with `CARGO_TERM_COLOR=never` and `CARGO_NET_OFFLINE=true` (workflow: `always`, unset). Wrapped in `time -v` and `timeout 900`, not in a memory-limited scope (rustfmt only; max RSS 143,860 KiB). Not rerun here. |
| Run clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **this mission**, `receipts/43b-clippy-all-features-r2` (raw sha256 `00495e2bcd527821aae5a6a9fea78a9d61d8e6b799a700cab51c37ce4c43b241`, 893 lines) | **PASS**: exit 0 by a normal exit; 0 warning and 0 error diagnostics; one `Finished` line (`dev` profile, 6m 11s); 18 of 18 checks | fresh mission-owned worktree byte-exact to C before; after: tracked bytes exact, ignored outputs `target/` and the declared `app/src-tauri/permissions/` only | `CARGO_BUILD_JOBS=2` (mission limit; the repository's `.cargo/config.toml` sets 8), `CARGO_TERM_COLOR=never`, `CARGO_NET_OFFLINE=true`; a fresh target instead of the runner's persistent cache; run as this mission's single invocation. Feature-gated crates were checked (candle-core/nn/transformers, tokenizers, hf-hub, safetensors, nexus-flash-infer, nexus-llama-bridge), so `--all-features` took effect. |
| Run tests | `cargo test --workspace --locked` | VALIDATION-02 `receipts/30-stage-B-workspace-test` (raw sha256 `ce11ea91537fc899292d3496e84a88de393dea01d7ebc65878a428d0d7639533`; manifest `36f10bf9…b822`, verified 177/177) | **PASS**: exit 0; recomputed here from the raw output: 305 libtest result lines, all `ok`; 8,764 passed, 0 failed, 78 ignored; 237 `Running` + 70 `Doc-tests` units; custom harness `39 live sandbox cases passed`; 0 `... FAILED` lines (`raw/04-stageB-recount.txt`) | source exact C before and after (declared generated `app/src-tauri/permissions/` only) | **ripgrep:** the workflow puts ripgrep 14.1.1 on PATH ("Privacy regressions exercise both real search backends"); Stage B's PATH reached `/usr/bin/rg` **14.1.0**. Rust resolved through the rustup proxy (`~/.cargo/bin`) and `rust-toolchain.toml` (1.94.0), not the toolchain directory directly. `CARGO_BUILD_JOBS=2`, `CARGO_TERM_COLOR=never`, `CARGO_NET_OFFLINE=true`. A separate target, not after the all-features Clippy in the same target. memory.peak 16,384 B above memory.max (section 3). |
| (VALIDATION-02 Stage A, not a workflow step) | `cargo clippy --workspace --all-targets --locked -- -D warnings` | VALIDATION-02 `receipts/20-stage-A-clippy` (raw `8566254291d9…6754`) | PASS, 352.02 s | exact C | **omits `--all-features`; it is not the workflow's Clippy step** and discharges nothing on its own. It is superseded for the workflow step by this mission's all-features run. |

## 2. The default suite (contract 8.2) inside the workspace tests

`cargo test --workspace --locked` runs the five default explorations unoptimised, with nothing ignored. In Stage B each
of `x1_…` to `x5_…` printed `... ok` exactly once (its report is captured by libtest and not printed). The
nexus-governed-control lib binary reported `313 passed; 0 failed; 35 ignored; … finished in 161.16s`: that wall time
bounds the five X explorations together below 8.2's 180-second ceiling. Individual unoptimised X durations are **not
measured**. The optimised (e1) X receipts with counts, property lines and branches are VALIDATION-02 Stage C
(`02-CONFIGURATIONS.tsv`).

## 3. The Stage B memory observation (classification)

| Field | Value (VALIDATION-02 Stage B, `descriptor.json`) |
|---|---|
| memory.max | 17,179,869,184 B (16 GiB) |
| memory.peak | 17,179,885,568 B: **16,384 B (4 pages of 4 KiB) above memory.max** |
| memory.events | max 4,595; oom 0; oom_kill 0 |
| swap | memory.swap.max 0; swap peak 0 |
| result | exit 0, completed, not timed out |

Executor classification: **OBSERVATION — a transient cgroup v2 charge above `memory.max` during kernel reclaim, with
the limit enforced (4,595 reclaim events, no OOM, no swap).** cgroup v2 charges memory before reclaiming it, and its
documentation states that usage may exceed the limit temporarily. The peak is a high-water mark, not a sustained
usage. The mid-run composition (anon 0.63 GiB, file 14.74 GiB) was recorded only in an earlier session transcript, not
in a sealed root, and is not relied on here. The measurement is not rewritten. **Architect ratification of this
classification remains pending** (it was docketed earlier as F-2 item 3).

## 4. What is not claimed

- No Fast Local PASS. Four Rust steps are evidenced; the native, packaged-Builder, security, frontend and Python steps
  are not (`04-FAST-LOCAL-MATRIX.tsv`, `04-FAST-LOCAL-VERDICT.md`).
- Local runs are not the hosted or self-hosted workflow. No GitHub Actions dispatch was made.
- Every model result is conditional on A-1 and A-2; nothing here establishes SQLite durability or real execution.
