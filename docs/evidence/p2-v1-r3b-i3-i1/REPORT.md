# P2-V1-R3B-I3-I1 evidence: the custody store implementation

This directory is the evidence for the Rust custody store, the durable recorder and refusal store specified by revision R5 of `docs/architecture/p2-custody-durable-recorder-design.md`. The store is implemented as test infrastructure and validated against the real custody core. What the implementation is and is not, and what stays open, is in `docs/architecture/p2-custody-store-implementation-boundary.md`.

## Candidate

| Item | Value |
|---|---|
| Branch | `review/p2-v1-custody-store-implementation` |
| Parent (base) | `7689e599ed8fc89ec7720d13869ef58cab767ecb`, the R5 design commit |
| Custody and codec source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` |
| Candidate | The single commit that adds this directory. Its SHA did not exist when these files were written. |
| Run | On the development host, before the commit, with the candidate's changes uncommitted. `controls/*/status-at-run.txt` records the worktree status each control run required and kept. |

## What was run

| Step | Command or script | Result |
|---|---|---|
| Store tests | `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store` | 78 passed: 75 store tests and the core's 3 commitment-boundary tests |
| Codec tests | `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec` | 21 passed |
| Core tests | `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core` | 47 passed |
| Clippy | `cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_store --test phase2_custody_codec --test phase2_custody_core -- -D warnings` | exit 0, no findings |
| Cleanup observation | `cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation` | 36 passed |
| Live harness | `cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run` | built, not executed |
| Formatting | `cargo fmt --all -- --check` | exit 0 |
| Whitespace | `git diff --check` | exit 0 |
| Store tests, with output | the store command with `-- --nocapture --test-threads=1` | 78 passed; the traces are in `traces.txt` |
| Store mutation controls | `scripts/store_controls.py` | 90 counted, all as required (below) |
| I2-R1 controls, rerun | `scripts/i2r1_controls_rerun.py` | 32 counted, all as required; 4 informational, each as expected |
| R5 design model | `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json <scratch>/design_checks.json` | PASS. The JSON it wrote is byte-identical to the committed `p2-v1-r3b-i3-p-r5/coverage.json` (SHA-256 `d88bc7cb140ecd702ef2ae398f9dfb113ef5bc4c874c722e3786ca2b8b7e96c8`). |
| R5 reference check | `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test --kernel <scratch>/kernel` | PASS, every self-test corruption reported |
| Historical manifests | `scripts/manifests.py`: each R1 to R5 `SHA256SUMS`, checked against its own snapshot | PASS (`validation/historical-manifests.txt`) |
| Scope | `scripts/scope.py <checkout> 7689e599ed8fc89ec7720d13869ef58cab767ecb` | PASS (`scope/scope-check.txt`) |

`validation/commands.txt` lists every validation command with its exit status and start time. Each command's complete output is in `validation/`. The toolchain is in `validation/toolchain.txt`: rustc 1.94.0, cargo 1.94.0, clippy 0.1.94, rustfmt 1.8.0, Python 3.12.3, host kernel Linux 6.17.0-35-generic.

## Store mutation controls

`scripts/store_controls.py` holds 90 counted controls. Each restores one wrong behaviour in the store's own code, in `support/custody/store/`. A control counts only if:

- every edit's anchor occurs exactly once;
- the store target compiles;
- the one intended test, run alone (`--exact`), fails;
- the last panic on the test's own thread carries the control's marker, and every required text;
- every mutated file is written back. All 17 files the script hashes, including the protected core, model and codec and their tests, are byte-identical before each control and after each restoration.

The worktree status had to equal `controls/store/status-at-run.txt` before the run and after every control. The protected custody core is never mutated by this script.

| Category | Counted | Result |
|---|---|---|
| implementation-safety (the N- controls are the 3 that exercise the Linux primitives) | 69 | all caught at their marker, all restored |
| simulator-source-conformance | 12 | all caught at their marker, all restored |
| authority-api-surface (source or type guards, not behaviour) | 9 | all caught at their marker, all restored |

The categories are never added into one figure. The run is in `controls/store/`: `run.jsonl` (one line per control, then the summary), `summary.json`, `run.meta` (start and end), and `logs/` (complete output per control).

Designing the controls and the trial runs before the counted one led to these corrections, all retained in the candidate:

- **A vacuous guard.** Test `o10`'s Clone and Default probe could never fail. It is now an autoref-specialization probe that is checked on `u8` first.
- **Assertions without markers.** Several assertions and `expect` messages carried no marker. They now carry the marker of the requirement they check.
- **New tests.** Tests were added where a control had no detector:
  - `c04`: claim exhaustion at 2^63;
  - `o13`: the preservation sync survives power loss;
  - `f09`: the disposition validator's exactness;
  - `r10`: the worker's W5 recheck;
  - `m10`: retirement from the complete set;
  - `a01`: the authority surface in the source.
- **A runner bug.** The runner read the first panic of a test, which can be a fixture's deliberate panic. It now reads the last panic on the test's own thread.

No test was deleted, no assertion weakened and nothing ignored.

## I2-R1 custody and codec controls, rerun

`scripts/i2r1_controls_rerun.py` is the I2-R1 runner: `docs/evidence/p2-v1-r3b-i2-r1/i2-r1/controls/negative_controls.py` at `8e2c46c00cd4071bbae41815d1d1cd0c4814655f` (`evidence/p2-v1-r3b-i2-r1`), SHA-256 `df5c06e881542d51a976e0590f07a542b3c8cea72919e06355d4612342043764`. `scripts/i2r1_controls_rerun.diff` shows exactly how it differs:

- `mod.rs`'s expected hash is the I3-I1 file's, `ff6379ed837328a4404d2092ef6d2a0b660781d848b6ddc8818be6b2f6b440d1`, because the I3-I1 file wires the store and documents it. The other five envelope files are byte-identical to I2-R1's and are checked as before.
- An optional `--expected-status FILE` replaces the requirement of a clean checkout.

The controls themselves are unchanged. Results:

- all 32 counted controls compiled, failed their intended test with their marker, and were restored. These are the eight I2A codec and integration controls, the 22 custody controls and the two canonical request-receipt controls, `NC-REQUEST-DEBUG` and `NC-REQUEST-ALTERED-DIGEST`.
- The four informational runs matched their recorded expectations: `c13` passes under both new mutations, `c16` fails under the Debug-digest mutation (its source guard) and passes under the altered-digest one.

These controls mutate the protected core, as I2-R1 established. The mutation is transient and authorized for these controls only. Every file was restored and verified by SHA-256. The run is in `controls/i2r1-rerun/`.

## Coverage

`coverage-matrix.md` and `coverage-matrix.json` are generated by `scripts/coverage_matrix.py`. They link the R5 invariants (INV-1 to INV-21), the R5 model's 67 design controls, the Rust tests with their markers, and the counted Rust controls. The generator checks that every listed test carries one of its invariant's markers, and that every control ran as required.

- **Design controls with a counted Rust control: 66.** Four of them (NC05a, NC05b, NC19, NC21b) have only an authority-api-surface counterpart: a source or type guard, because no deterministic test can interleave them, or because the Rust types exclude them.
- **Historical Python-model control: 1.** NC04, the first candidate's container geometry, has no Rust form. S-TEAR-SPANS is a related simulator control. It counts for tear containment only, not for the geometry decision.
- **Rust-specific controls with no design control: 21.** Among them are the closed real entry, the admission's construction, the native primitives, the error-sequence limitation, poisoning, worker loss and the seal.

## Traces and composed recovery

`traces.txt` extracts the figures the tests print.

- **Real-core traces.**
  - The every-kind journal.
  - T1, T3 and T14.
  - The three T8 variants: a late owner whose incident record was never submitted, taken but not written, or durable but not published. Each is UnsealedAction and refused.
- **The activation windows.** The counts are (50, 259200, 51820, 207380, 48404, 32460, 621840), equal to design section 10.7.
- **The crash matrix's coverage.** It equals design section 15.2 for all eight procedures.
- **Composed recovery.**
  - 883 first crashes, 459 runs with dependent work and 2,425 second crashes.
  - Every second crash ends with the generation in the fresh owner's decision.
  - Refused before work: Unprovisioned 367, MaintenanceIncomplete 32, PriorUnresolved 25.
- **Storage composed.** 240 openings, 35 runs on admitted storage, 1,117 second crashes, 180 openings on storage that is not admitted, and 33 losses of qualification.

## Scope and refs

- **`scope/scope-check.txt`.** Every changed path lies in the mission's envelope. The protected files (the core, model and codec and their tests, the cleanup observer and its regression, the live harness and package-layout tests, the manifests, `Cargo.lock`, the design) and trees (production `src`, `.github`, the R1 to R5 evidence) are byte-identical to the base.
- **`scope/changed-paths.txt`.** The staged change set.
- **`scope/refs-before-commit.txt`.** The ten protected refs, read from the remote before the commit, each at its expected SHA. The candidate branch did not exist on the remote then.

## Normalization

Outputs are otherwise verbatim. Absolute paths were replaced as follows:

- the checkout by `<checkout>`;
- the scratch directory by `<scratch>`, and the kernel copies by `<scratch>/kernel`;
- the Cargo home by `<cargo-home>`.

Blank lines at the end of a file were removed. Thread and process identifiers in panic lines are as printed.

Two files keep trailing spaces byte for byte, and `.gitattributes` exempts exactly these two from Git's whitespace checks:

- `scripts/i2r1_controls_rerun.diff`: a unified diff's empty context lines are a single space.
- `validation/store-tests-nocapture.txt`: a test's result line ends in a space where other output interleaved.

The repository's `.gitignore` ignores `*.log`. The control logs here were added explicitly, as I2-R1's were.

## Limitations and gates

- **Nothing is qualified.**
  - Every host, profile, kernel and storage value the tests use is a fixture.
  - The simulator is a model of the design's reading of the Linux v6.17 sources. It is not a kernel.
  - No host, filesystem, kernel build or device was observed or qualified.
- **The native tests are narrow.** They ran unprivileged beneath `CARGO_TARGET_TMPDIR`. They injected no I/O error and no power loss, and prove neither durability of real storage nor native cleanup.
- **The real configured-store entry is closed.** No real store was opened, provisioned, synced, locked or claimed.
- **Assumptions stay assumptions.**
  - A-S1 to A-S4, A-M1 and A-M2 remain assumptions.
  - A-S5 remains rejected.
  - The finite error-sequence counter is a documented limitation, not a detected condition.
- **The external gates stay open.**
  - G-AUTH, the store uid's authority, is unresolved.
  - G-HOST, G-LIVE, G-NATIVE and G-PWR remain.
- **Not done here.** No integration, live validation, authentication, hardware proof or Phase Two completion is claimed.

## Files

| Path | Content |
|---|---|
| `REPORT.md` | This report |
| `.gitattributes` | Exempts the two verbatim files above from whitespace checks |
| `SHA256SUMS` | SHA-256 of every file here, of the boundary document, the design and every candidate source file |
| `coverage-matrix.md`, `coverage-matrix.json` | The coverage matrix |
| `traces.txt` | Real-core traces and composed figures |
| `scripts/store_controls.py` | The store mutation controls and their runner |
| `scripts/i2r1_controls_rerun.py`, `scripts/i2r1_controls_rerun.diff` | The I2-R1 runner as rerun, and its diff from I2-R1's |
| `scripts/coverage_matrix.py` | The matrix generator |
| `scripts/validate.sh` | The validation commands |
| `scripts/manifests.py`, `scripts/scope.py` | The manifest and scope checks |
| `controls/store/` | The counted store control run |
| `controls/i2r1-rerun/` | The I2-R1 control rerun |
| `validation/` | Every validation command's output, the toolchain, the R5 model and reference outputs, and the historical manifests |
| `scope/` | The scope check, the changed paths and the ref readback |

## Reproducing

From a clean checkout of the candidate:

```
python3 docs/evidence/p2-v1-r3b-i3-i1/scripts/store_controls.py <checkout> <log dir>
python3 docs/evidence/p2-v1-r3b-i3-i1/scripts/i2r1_controls_rerun.py <checkout> <log dir>
bash docs/evidence/p2-v1-r3b-i3-i1/scripts/validate.sh <checkout> <output dir>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test --kernel <dir>
python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/manifests.py <checkout> docs/evidence/p2-v1-r3b-i3-p-r1 docs/evidence/p2-v1-r3b-i3-p-r2 docs/evidence/p2-v1-r3b-i3-p-r3 docs/evidence/p2-v1-r3b-i3-p-r4 docs/evidence/p2-v1-r3b-i3-p-r5
python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/scope.py <checkout> 7689e599ed8fc89ec7720d13869ef58cab767ecb
```

Two notes on reproduction:

- **Run the control scripts alone.** They mutate and restore files in the checkout, so nothing else may build, test or edit there while they run. Without `--expected-status` they require a clean checkout.
- **Kernel files.** `<dir>` holds the Linux v6.17 files that `reference_check.py` lists and verifies by SHA-256.
