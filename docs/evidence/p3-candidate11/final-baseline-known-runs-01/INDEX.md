# Final-baseline known runs 01 — index

Mission P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. For the Sole Architect's review.

**Evidence only.** This packet records one fresh optimised build of frozen Candidate C, its complete nonignored library suite, five exact-final-baseline full runs (F-2, F-B, F-3, F-PT, F-1, in that order, one at a time) and the A4 comparison. It accepts nothing. D2A-1 remains unaccepted and unintegrated; nothing here completes Phase Three. No source, manifest or proof document was changed; no mutation, mutant witness, other configuration or hosted CI was run.

## Candidate C (frozen; `FREEZE-RECORD.txt`, `receipts/00-authentication.txt`)

| Item | Value |
|---|---|
| Commit | d5964ee5326c2733177307db6f507328023d361a |
| Tree | 51bac5fd335999cc995c0ab9f582a30244973ef0 |
| Single parent | 71264aa81a40b86189783facc8437e0427887c16 |
| protocol.rs / model_tests.rs / unit_tests.rs blobs | `a2df2841…` / `28085ce2…` / `55dc6640…` |
| Current-tree manifest ct-1 | blob `5d31cb45…`, sha256 `20bcc6fe…1a44279` |
| AD-1-R4 | sha256 `00ced6d5…5192b693` |
| Worktree | `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01`, detached at C; byte-for-byte equal to C before and after the build and every invocation (`tools/verify_tree.py`) |

## Build and pinned executable (`receipts/10-build`, `receipts/11-pin`)

| Item | Value |
|---|---|
| Command | `cargo test -p nexus-governed-control --locked --release --lib --no-run` (exactly), cwd the worktree |
| Environment (`env -i`) | `HOME=/home/nexus` `PATH=/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin` `CARGO_TARGET_DIR=/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target` `CARGO_TERM_VERBOSE=true` `CARGO_NET_OFFLINE=true` |
| Toolchain | rustc 1.94.0 (4a4ef493e 2026-03-02); `/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu/bin/rustc`; cargo 1.94.0 (85eff7c80 2026-01-15) |
| Repository Cargo configuration | unchanged: `[build] jobs = 8`; `[env] CARGO_PROFILE_TEST_DEBUG = "1"`; release profile opt-level 3, lto thin, codegen-units 16, strip symbols |
| Effective rustc flags of the test unit (build's own verbose message) | `-C opt-level=3 -C lto=thin -C codegen-units=16 -C strip=symbols --test`; no `--cfg` (no feature, no extra cfg); no debuginfo, debug-assertions, overflow-checks or panic flag (rustc defaults: off, off, off, unwind) |
| Result | exit 0, no signal; `Finished release profile [optimized]`; wall 91.86 s of 900 s; scope peak 2.74 GiB, swap peak 0, oom_kill 0 |
| Executable | `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` |
| sha256 / size | `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c` / 8,984,464 bytes |
| Inputs (rustc dep-info) | 49 `.rs` files, all under `crates/nexus-governed-control/src/`, each C's blob; none under `docs/` |

The digest was checked before and after every invocation: all equal the pin.

## Nonignored suite (`receipts/30-suite`) — CLASSIFICATION: PASS

`nexus_governed_control-2faa0b9718d3e6a3 --nocapture --test-threads=1`, no name filter: **test result: ok. 313 passed; 0 failed; 35 ignored; 0 measured; 0 filtered out; finished in 63.31s**. Exit 0, no signal; wall 63.31 s of 180 s; scope peak 0.22 GiB, swap 0, oom_kill 0.

- Listed by the executable: 348 tests; ignored 35 (the 34 of AD-1-R4 E.3 and `authority::tests::stop_measurement::rs01_t10_the_registry_stop_seen_from_outside_its_lock`); nonignored 313; executed 313; difference between listed nonignored and executed: none; duplicates: none; no ignored test executed.
- Five default explorations: X-1 PASS, 59,104 states; X-2 PASS, 10,020 states; X-3 PASS, 25,617 states; X-4 PASS, 3,106 states; X-5 PASS, 3,311 states.
- The 51 diagnostic tests of AD-1-R4 D (N.8): 51 of 51 ok (`receipts/30-suite/DIAGNOSTICS-51.txt`).
- Healthy witnesses: C1, C25 and C4 ok, each printing exactly its healthy line(s) of G.3(d) / G.5(C), with no positive marker, violation or fixture failure; C4's 20 step records hold G.6 against the frozen schedule (evidence commit B).
- 20 refusal lines, every one inside a `should_panic` refusal test; none elsewhere. 38 panic messages, all in tests written to panic or catch it.

## Full runs (one at a time; 16 GiB, zero swap, 45 minutes; `--exact --ignored --nocapture --test-threads=1`)

| Run | Test | Explorer | Measured keys / states (expanded) | Kf | Derived original states | Transitions (fault) | No-step | Admission evals / admitted | Wall | Scope peak | Outer class |
|---|---|---|---|---|---|---|---|---|---|---|---|
| F-2 | `full_f2` | original | 12,602,670 (12,602,670) | — | — (original explored) | 54,214,081 (3,558,228) | 3,206 | — | 67.44 s | 1.59 GiB | PASS — admissible unchanged-core original-explorer PASS |
| F-B | `gc_symmetry_fb` | composed | 3,775,373 (3,775,373) | 156,463 | 22,182,849 | 11,404,590 (954,390) | 10,821 | 3,775,373 / 310 | 51.57 s | 0.46 GiB | PASS — admissible unchanged-core composed-quotient PASS |
| F-3 | `gc_symmetry_f3` | composed | 20,431,851 (20,431,851) | 880,778 | 119,948,772 | 77,342,551 (6,034,170) | 13,996 | 20,431,851 / 3,182 | 338.97 s | 2.73 GiB | PASS — admissible unchanged-core composed-quotient PASS |
| F-PT | `gc_symmetry_fpt` | composed | 29,751,473 (29,751,473) | 1,698,028 | 173,414,754 | 113,252,841 (7,396,731) | 16,131 | 29,751,473 / 3,182 | 487.53 s | 3.85 GiB | PASS — admissible unchanged-core composed-quotient PASS |
| F-1 | `full_f1` | original | 46,524,336 (46,524,336) | — | — (original explored) | 209,297,440 (21,234,651) | 8,696 | — | 245.18 s | 5.42 GiB | PASS — admissible unchanged-core original-explorer PASS |

Every run: exit 0, no signal; swap peak 0; oom_kill 0; 24 property lines HOLD, E10 "not applicable here (worker verbs do not unwind)", D1 and L1 HOLD; all 90 BRANCHES labels listed (taken labels outside the listed set are recorded as printed); no refusal; no panic; executable and source identity equal before and after. For F-B, F-3 and F-PT **the original graph was not explored**: the derived counts are (N+1)(2K − Kf) with N = 2, conditional on the accepted proof (B(1)) and N. F-PT's reduced-key margin: 248,527 keys below 30,000,000. F-1 ran under the Owner-adopted 60,000,000 ceiling (AD-1-R4 A).

Informational only (AD-1-R4 I: historical results are not evidence): every printed count above equals the historical receipt of the same configuration — F-1 and F-2 at 244c5b23 (CAMPAIGN-01), F-3 at 77a146ca (GC-SYM-F3-01), F-PT and F-B at 2d83b74b (PT-FB-EXPERIMENT-01). F1-CAP60-01's F-1 at 243501f0 printed the same 46,524,336 states and 188,062,789 non-fault edges but 173,856 fewer fault edges; 243501f0 precedes the two SOURCE-R1/R2 model commits (49e29cd3, 244c5b23), whose subjects concern explored insert outcomes.

## A4 (`receipts/50-A4`)

**A4: SATISFIED** — F-3 PASS (True), F-PT PASS (True), no applicable property fails in F-PT, both admissible. Differences recorded (not required to be equal): test selected (--exact --ignored), header, bounds, harness result (raw), MEASURED quotient keys, MEASURED expanded keys, MEASURED transitions, MEASURED fault edges, MEASURED non-fault edges (L1's line), MEASURED no-step keys, MEASURED admission evaluations, MEASURED self-mirrored keys (Kf), DERIVED original states (N+1)(2K-Kf), conditional on B(1), explorer elapsed (s, as printed), process wall (s, /usr/bin/time), scope memory.peak (bytes), largest process max RSS (KiB), property L1, branch sets. Taken in F-PT only: 0 label(s); in F-3 only: 1. Not a mutation kill.

## Executor defects found and repaired during the mission (all in tools; none in Nexus; nothing re-run)

1. `supervise.py` v1 passed relative record paths into a scope with another cwd: the self-test failed (`development/selftest-v1-FAILED-relative-paths`). v2 made them absolute.
2. The v2 self-test's time-limit predicate expected exit 137; `timeout -s KILL` kills its own process group, so signal 9 is seen (`development/selftest-v2-FAILED-predicate`). Enforcement itself worked. v3 accepts either. The final self-test: all six checks OK.
3. `supervise.py`'s build step looked for cargo's non-verbose `Executable unittests src/lib.rs (…)` line; under `CARGO_TERM_VERBOSE=true` cargo prints ``Executable `…` `` instead, so it wrote no pin and reported BUILD NOT OK although the build exited 0. The build was **not** repeated: `tools/pin.py` pinned from the same unedited output (`receipts/11-pin`). Its informational dep-info line was garbled; `DEP-INFO.txt` replaces it.
4. `check_suite.py` v1: rustc 1.94 prints `thread '<name>' (<id>) panicked at`, which v1's pattern missed (its panic checks passed vacuously), and a 0.0 s wall time was read as missing (two listing checks failed). v2 fixed both and classified the same unedited outputs (`receipts/27-TOOLS-REVISION.txt`, `development/suite-check-v1-checker-defects`).

## Limitations

- The healthy witnesses' results come from the unfiltered suite run, as the mission requires. They were not invoked alone with `--exact` (AD-1-R4 F.2 / G.3(d) / G.5(C) describe that form for a type (e3) receipt); no such invocation was authorised here, so none was made.
- The original graphs of F-B, F-3 and F-PT were not explored; their original counts are derived, not measured.
- Every result is of the explored model and stays conditional on A-1, A-2 and AQ6 (AD-1-R4 J).
- Executor-written tools parse the output; they were frozen before use and exercised against historical outputs and synthetic negative controls (`receipts/24`, `26-checker-controls*.txt`).

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. The classifiers re-run read-only over the receipts (`tools/check_suite.py`, `tools/check_full.py <key>`, `tools/a4.py`) given the mission's evidence root path; `tools/final_gate.py` is the gate whose output is `receipts/60-final-gate.txt`.
