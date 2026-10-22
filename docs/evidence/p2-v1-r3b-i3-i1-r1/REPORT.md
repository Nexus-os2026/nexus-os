# P2-V1-R3B-I3-I1-R1: store authority and closure — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-custody-store-implementation`. A file in a commit cannot name
that commit, so everything here is bound to two other things. The baseline
is bound to the repair base. The candidate's results are bound to the exact
source bytes they validated (`validation/sources.SHA256SUMS`), run in a
snapshot of the candidate's tree. The validation rerun on the committed
candidate is kept outside the repository, in
`/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r1-work/post-commit/`, and reported with
its hashes in the mission's final report.

## 1. Identity

| Item | Value |
|---|---|
| Repair base | `72ffc4fcc0141e2e5ae77a927481017ca711ab7c` (tree `6cc4a42f5650f35bdb8f3401517a66011b1fe791`, sole parent `7689e599ed8fc89ec7720d13869ef58cab767ecb`) |
| R5 design baseline | `7689e599ed8fc89ec7720d13869ef58cab767ecb` |
| Custody and codec baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` |
| Candidate | the commit containing this file: sole parent the repair base, subject `test(custody): bind store authority and sealing to owned state` |
| Pre-commit snapshot | tree `a6ef7af6c3187dbe108b8405a132e2e8eb9e0df1`: the worktree's tracked and new files before this evidence was written, archived into a scratch Git checkout whose `HEAD^{tree}` equals it (`validation/candidate-runs.txt`). Its sources are the candidate's (`validation/sources.SHA256SUMS`). It differs from the candidate's tree only in this evidence directory and in `docs/architecture/p2-custody-store-implementation-boundary.md`, which gained the succession finding of section 9 afterwards (documentation only). |

Changed paths (relative to the base):

- `crates/nexus-verifier-sandbox/tests/support/custody/store/`
  - modified: `disposition.rs`, `exchange.rs`, `maintenance.rs`, `mod.rs`,
    `open.rs`, `recorder.rs`;
  - new: `owner.rs`, `faults.rs`.
- `crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs` (modified).
- `docs/architecture/p2-custody-store-implementation-boundary.md` (modified).
- `docs/evidence/p2-v1-r3b-i3-i1-r1/` (new).

The custody core and codec (`core.rs`, `model.rs`, `codec.rs`) and their
tests, `support/custody/mod.rs`, the cleanup observer and its regression,
the live harness, the package-layout test, production `src/`, workflows,
governance, manifests, `Cargo.lock`, the toolchain, the R5 design, and all
earlier design and evidence are byte-identical to the base
(`checks/scope.txt`).

## 2. The counterexamples at the base

`baseline/r1_base_probes.rs` is a test target added to an isolated copy of
the base tree. `baseline/provenance.txt` shows that copy holds every one of
the base's 2743 blobs unchanged plus only that file. The probes reach the
store only through `custody::store::...` call sites of safe Rust, from outside
the store module. The first run is `baseline/probe-run.txt`; the rerun, with
its commands and exit codes, is in `baseline/rerun/`. All six reproduced:

| | What the base allowed | Observed |
|---|---|---|
| B1 | `Recorder::request_seal(&header, &fabricated_closed, 0)` | The custody refused closure (`Refused([LateIncidents(1), EvidencePending(1)])`), yet the fabricated `Closed { records: 6 }` was accepted, the worker wrote the seal, and the next owner started (Ready). |
| B2 | `request_seal` with another custody's genuine `Closed` | With equal counts (6 and 6) and the same generation, the foreign closure sealed this journal, and the next owner started. |
| B3 | `opened.decision.blocking.clear()`, then `opened.claim(...)` | The unedited start refused `PriorUnresolved (1)`; after the edit the claim was accepted (pool file 1, claim 2). |
| B4 | `opened.opening` and `opened.selection.digest` overwritten, then `claim_presenting(.., Some(other_admission), ..)` | Another opening's admission was accepted (pool file 0, claim 1). |
| B5 | `StoreValidator::new(&root, &edited_scan)` | With no disposition file in the store, a fabricated disposition validated, `apply_disposition` succeeded and the blocked run started. |
| B6 | `Recorder::exchange().acquire().state.*` | The latch was cleared. `durable_through = 1` made the core acknowledge a record never written. The claim and seal states were set with no seal block written. |

## 3. The corrected boundary

The base's public items let caller data stand in for retained state. R1
removes those items; it does not add a check beside them. The full
description is section 7 of
`docs/architecture/p2-custody-store-implementation-boundary.md`. In brief:

- **Ownership graph.** `owner::start_owner` is the only constructor of a
  `StoreOwner`. That value retains, together and privately:
  - the custody (built with every current incident as a prior, with
    dispositions applied only through the opening's own validator);
  - the recorder, with its exchange;
  - the claimed journal's header and worker;
  - the exclusion guard (`Arc<StoreGuard>`), shared only with the worker's
    thread, which keeps it until the worker ends.

  Custody operations are forwarded one by one. There is no `&mut Custody`,
  setter or field access from outside the store.
- **Closure to seal.** `StoreOwner::close(self)` calls the core's `close` on
  the retained custody:
  - on refusal, the same owner returns, with the custody, every owner it
    holds, the recorder, the worker and the guard;
  - on `Ok`, and only then, it requests once the seal of its own journal,
    with that closure's record count and the retained header.

  `ClosedStore` has no close or seal request. A second request is refused
  (`AlreadyRequested`). A latch withholds the seal after a successful
  closure.
- **Opening.** `Opened`'s fields are private: callers get borrows or copies.
  The claim is internal. It consumes the opening and derives its applied
  bindings and reasons from the opening's verified state. The start decides
  over the complete verified set, never the bounded report.
- **Dispositions.**
  - `exact_restatement` is a pure comparison.
  - `StoreValidator<'a>` exists only as a borrow of one opening's verified
    state, read under its lock. It cannot outlive the opening, and no
    constructor takes a `ScanResult`.
  - API provenance only: G-AUTH stays open.
- **Exchange.** It is internal to the store, with its state, guard, sink,
  recorder, worker and handle; callers read copies.
  - The fault interface (`faults`) can only latch (monotonically), poison,
    interleave or stop the real worker, inject what the sink refuses or
    already holds, panic the next sink, or re-acknowledge an already-durable,
    unapplied record.
  - `recorder::native_wiring` is a Linux-only check on a test's own file and
    an exchange of its own, with no seal.
- **Maintenance sessions.**
  - Archival, recycling, retirement and the recycling's resumption consume
    the session's own latest verification, never a caller's name, report or
    refusal. One verification authorizes one procedure, and it lapses when
    any step of the session's procedures starts.
  - The `PROVISION` rewrite is internal to recycling, retirement and
    re-qualification. `resume_recycle`'s saved report remains Owner input
    (R5 section 13.8), checked against the session's own refusal and reads.

## 4. What each counterexample meets now

| | Behaviour (test) | API/type guard (probe) |
|---|---|---|
| B1 | In each T8 window, and with every record durable and acknowledged, the custody refuses closure. The same owner returns with its late owner and lock (Busy), no seal is requested or written, and the next owner is refused (`b01`). | `P-B1-CLOSURE-DATA`, `P-SEAL-AGAIN`, `P-CLOSE-TWICE` |
| B2 | Two stores with the same generation and equal counts: one closure seals only its own journal; the other journal stays unsealed and blocks (`b02`). | `P-B2-CUSTODY-SWAP`, `P-B2-HEADER-SWAP` |
| B3 | Edited copies of the report, decision, scan and selection change nothing; the start still refuses (`b03`). With 66 incidents (64 listed), dispositions for the listed ones leave the two unlisted ones blocking (`b04`). | `P-B3-DECISION-EDIT`, `P-B3-SCAN-EDIT`, `P-B3-CLAIM-CALL` |
| B4 | Each opening's identity, selection and admission are its own; copies rebind nothing; each owner claims with its own admission (`b05`, `o10`). | `P-B4-IDENTITY-EDIT`, `P-B4-OPENING-MINT`, `P-B4-ADMISSION-FORGE` |
| B5 | A fabricated exact disposition validates nothing, here or in a caller's own custody. So does another store's exact disposition, and a published one with another count. Only the exact one published here lets the owner run, and the header lists its file's reason (`b06`, `f09`). | `P-B5-VALIDATOR-FROM-COPY` |
| B6 | A status copy cleared of its latch clears nothing: the gate refuses, the first cause stays, nothing is published after the latch. No acknowledgement of a non-durable record, and no injection of the sink's next record (`b07`, `r02`). | `P-B6-EXCHANGE-TYPE`, `P-B6-WORKER-TYPE` |
| Sessions | No decision without a verification. One verification, one procedure. A verification older than a revocation authorizes nothing (`b08`). | `P-SESSION-VERIFIED`, `P-PROVISION-REWRITE` |
| Exclusion | An owner dropped while its worker's write is in flight leaves the store excluded (Busy) until the thread ends (`b09`). | — |

## 5. Regression results (snapshot `a6ef7af6`; `validation/`)

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store` | 0 | 87 passed: 84 store tests (75 from I3-I1, adapted to the R1 interface, and 9 R1 regressions `b01` to `b09`), plus the core's 3 commitment-boundary tests |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec` | 0 | 21 passed |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core` | 0 | 47 passed |
| `cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_store --test phase2_custody_codec --test phase2_custody_core -- -D warnings` | 0 | no warning |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation` | 0 | 36 passed |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run` | 0 | built, never run |
| `cargo fmt --all -- --check` | 0 | clean |
| `git diff --check` | 0 | clean |
| store tests again, `--nocapture --test-threads=1` | 0 | 87 passed (the traces) |

The native tests `n01` to `n08` run inside the store target, unprivileged,
on files they create beneath `CARGO_TARGET_TMPDIR`. Nothing opens a real
store, qualifies a host, sysfs or a device, mounts, performs privileged I/O,
uses a bus or a runner, or runs a live sandbox. Toolchain: rustc and cargo
1.94.0, Python 3.12.3, host kernel 6.17.0-35-generic
(`validation/toolchain.txt`).

## 6. Controls, by kind, never added together

- **Behavioural store controls** (`controls/store/`,
  `matrices/behavioural-controls.md`). Each restores one wrong behaviour.
  Each must compile, fail its one intended test on the assertion carrying
  its marker, and have every file restored byte for byte, with the
  checkout's status unchanged. Results, all as required:
  - simulator and source conformance: 12 of 12;
  - implementation safety: 69 of 69 (3 exercise the Linux primitives);
  - authority and API surface: 8 of 8;
  - R1 authority binding: 10 of 10. These are new mutations reopening each
    behavioural R1 boundary: refused closure seals, start from the report,
    unchecked validator, invented applied reason, undurable fault
    acknowledgement, next-record injection, replaced latch, reused
    verification, verification that never lapses, guard not kept.
- **I3-I1 to R1 mapping** (`matrices/control_mapping.md`), generated from
  both scripts:
  - 86 controls unchanged;
  - 3 adapted. `I-VALIDATOR-INEXACT` and `A-WORKER-LOCK` got new anchors at
    the same defect. `I-SEAL-WHILE-LATCHED` got new anchors; and because the
    owner's close now requests the seal itself, its first detecting
    assertion is `r08`'s failed-claim case, so its required text changed;
  - 1 remapped: `A-ADMISSION-UNBOUND` has no behavioural call site left
    (nothing accepts a caller's admission). It is carried by the API/type
    guards and neither run nor counted as a behavioural control;
  - 10 added.

  No compile failure is counted as a behavioural detection. The adaptation
  diff is `scripts/store_controls.adaptation.diff`.
- **The I2-R1 suite** (`controls/i2r1-rerun/`). I3-I1's runner was rerun
  unchanged: the six files it pins are byte-identical to I3-I1's. Results:
  32 of 32 counted controls as required, and 4 informational runs, every
  result identical to I3-I1's recorded rerun
  (`controls/i2r1-rerun/comparison-with-i3-i1.txt`, from
  `scripts/compare_i2r1.py`). No runner adaptation was needed, so there is
  no diff.
- **API/type guards** (`controls/api-probes/`,
  `matrices/api-type-guards.md`). Compile-time probes from outside the
  boundary:
  - 15 privacy guards. Each fails with exactly its expected `E0603`,
    `E0616`, `E0624` or `E0451` error. Each builds once that one access is
    reopened in the scratch copy (self-check), whose sources are then
    restored and verified;
  - 1 ownership guard: `close` twice is `E0382`, and closing the returned
    owner builds;
  - a positive control (ordinary public use builds) and a harness check (a
    deliberate `E0308` is seen).
- **R5 design-control coverage** (`matrices/coverage-matrix.md`), regenerated
  with I3-I1's generator, unchanged. Of 67 design controls, 66 have a counted
  Rust mutation and 1 is historical (Python model only), as in I3-I1. Its
  title still reads I3-I1 because the generator is unchanged.

## 7. Repository checks (`checks/`)

- **R5 design model.**
  `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json ...`
  exits 0, and its JSON is byte-identical to the committed
  `p2-v1-r3b-i3-p-r5/coverage.json` (`cmp` exit 0).
- **R5 reference self-check.**
  `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test --kernel <dir>`
  passes. It checks 204 kernel citations in 35 files, each file verified by
  SHA-256, and every self-test corruption is reported. The kernel inputs
  (Linux v6.17 copies) are kept at
  `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r1-work/kernel/`; their SHA-256 list
  has SHA-256 `5ed404adfb87c3054481454e30617808aefdb725cb05c774830213998b597005`.
- **Historical manifests.** Each manifest (P-R1 to P-R5, and I3-I1) matches
  its own snapshot, and the evidence inside each directory is unchanged.
- **Scope.** Every changed path lies in the envelope, every protected file
  and tree is unchanged, and there is no `.gitattributes` anywhere.
- **Refs.** `scope/refs-before-commit.txt` holds the ten protected refs and
  the candidate branch, locally and on `github`, before the commit.
  `scope/publication-preflight.txt` holds the workflow triggers and the
  hooks.

## 8. Provenance and normalization

- **Raw outputs.** They are kept outside the repository in
  `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r1-work/` (`baseline/`,
  `pre-commit/runs/`, `pre-commit/checks/`).
- **Normalization.** Each copy here went through `scripts/normalize.py`:
  - trailing spaces, tabs and carriage returns are removed from each line;
  - blank lines at the end are removed;
  - exactly one final newline is kept.

  Nothing else is rewritten. `normalization.tsv` lists every copied file
  with its raw and normalized SHA-256 and byte counts: 196 copied files are
  unchanged and 9 were normalized. No `.gitattributes` exemption is used.
- **The adaptation diff.** `scripts/store_controls.adaptation.diff` is one
  of the 9. Its blank context lines are empty lines instead of a single
  space. GNU `patch` applies it to I3-I1's `store_controls.py` and
  reproduces this directory's `scripts/store_controls.py` byte for byte.
- **Logs.** `*.log` files are force-added; the ignore rules are unchanged.
- **Builds.** The candidate runs used a scratch Git checkout of the snapshot
  (`candidate_runs.sh`) and a target directory outside the repository. The
  mutating runs restored and verified every file. The checkout's status was
  empty before and after each step.

## 9. Limitations and open gates

- **A safe-API boundary only.** It holds within this crate's safe code, and
  does not defend against `unsafe` code in the same process.
- **Core types stay public.** `Closed`, `ValidatedDisposition`,
  `Custody::new` and `DispositionValidator` remain public. A caller can run a
  custody of its own; the store's guarantees concern the custody it retains.
- **Provisioning and re-publication** (R5 section 13.2) are Owner acts
  without a session. Re-publication does not itself refuse an existing
  `PROVISION`; that precondition rests on the Owner, as R5 states.
- **Open finding: succession.** Successor provisioning does not check the R5
  section 13.6 precondition. The predecessor's in-session verification must
  show every bound incident dispositioned, and the statement must name each
  store-level Invalid condition. I3-I1 never checked it either. A check
  needs a design reading: the session's verification yields no report beside
  an Invalid or Capacity refusal, and section 13.10 names succession as a
  resolution of Capacity. It is left for the Architect.
- **Gates.** G-AUTH, G-HOST, G-LIVE, G-NATIVE and G-PWR stay open. A-S5
  stays rejected. The errseq limitation stays documented, not detected.
- **Out of scope.** Nothing here authenticates storage, proves native
  cleanup or power-loss behaviour, qualifies a host, or is live acceptance,
  integration or Phase Two completion. No hash or stored record is used as
  authentication or as native-cleanup proof.

## 10. Reproducing

From a clean Git checkout of the candidate whose `HEAD^{tree}` is the
candidate's tree:

```
bash docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/candidate_runs.sh <checkout> <out>
bash docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/checks.sh <repository> <out> <kernel dir>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/matrices.py <store summary.json> <i2r1 summary.json> <api_probes.json> <out>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/control_mapping.py docs/evidence/p2-v1-r3b-i3-i1/scripts/store_controls.py docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/store_controls.py <out> <store summary.json>
python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/coverage_matrix.py docs/evidence/p2-v1-r3b-i3-p-r5/coverage.json docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/store_controls.py <store summary.json> crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs <out>
```

The control and probe scripts mutate and restore files in the checkout, so
run them alone. The baseline is reproduced on an isolated copy of the base
tree with `baseline/r1_base_probes.rs` added as
`crates/nexus-verifier-sandbox/tests/`, using the commands in
`baseline/rerun/commands.txt`.

## 11. Contents

| Path | Content |
|---|---|
| `baseline/` | The B1 to B6 probe source, its provenance, the base tree listing, the first build and run, and the rerun with its commands |
| `validation/` | The section 11 commands' outputs, the toolchain, the validated sources' SHA-256, and the candidate runs' log |
| `checks/` | The R5 model and reference outputs, the historical manifests, and the scope check |
| `controls/store/` | The counted store controls: the summary and one log per control |
| `controls/i2r1-rerun/` | The I2-R1 rerun: the summary and its logs |
| `controls/api-probes/` | The API/type probes: results, a table, and one log per build |
| `matrices/` | The behavioural matrix, the API/type matrix, the I3-I1 to R1 mapping, and the R5 coverage matrix |
| `scope/` | The ref readback and the publication preflight |
| `scripts/` | The R1 control script and its adaptation diff, the probes, the mapping, the matrices, the I2-R1 comparison, validation, checks, candidate runs, scope and normalization |
| `normalization.tsv` | Every copied output: raw and normalized SHA-256 |
| `SHA256SUMS` | Every file of this directory but itself |
