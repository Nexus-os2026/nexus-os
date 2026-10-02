# P2-V1-R3B-I3-I1-R2: maintenance succession and verification authority — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-custody-store-implementation`. A file in a commit cannot name
that commit, so the evidence here is bound to two other things:
- **the baseline**, bound to the repair base;
- **the candidate's results**, bound to the exact source bytes they
  validated (`validation/sources.SHA256SUMS`), run in a scratch Git checkout
  of a snapshot of the candidate's tree.

The validation rerun on the committed candidate is kept outside the
repository, in `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r2-work/post-commit/`. The
mission's final report gives it with its hashes.

## 1. Identity

| Item | Value |
|---|---|
| Repair base | `ed7c7088247badf87b3b1d483ee57360f867e2f1` (tree `08c20843335507210b9401239ad9c8472e980d66`, sole parent `72ffc4fcc0141e2e5ae77a927481017ca711ab7c`) |
| R5 design baseline | `7689e599ed8fc89ec7720d13869ef58cab767ecb` |
| Candidate | The commit containing this file. Sole parent: the repair base. Subject: `test(custody): bind succession to verified predecessor state` |
| Pre-commit snapshot | Tree `5b396d8303c0ebb0641661cd16885f4645ce4c36`: the worktree's tracked and new files when the candidate runs began, committed into a scratch Git checkout whose `HEAD^{tree}` equals it (`validation/candidate-runs.txt`). It differs from the candidate's tree only inside this evidence directory. Files added after the runs: the run outputs under `validation/`, `controls/`, `checks/` and `scope/`; the generated matrices; `scripts/scripts.adaptation.diff`, `scripts/adaptation_check.py` and `SHA256SUMS`. Files completed after them: `REPORT.md`, `normalization.tsv`, `matrices/maintenance-procedure-authority.md` and `matrices/module-self-review.md`. No source, test or document outside this directory, and no script the runs used, changed after the snapshot. The comparison with the commit is in the mission's final report. |

Changed paths, relative to the base:
- `crates/nexus-verifier-sandbox/tests/support/custody/store/`: modified
  `classify.rs`, `maintenance.rs` and `open.rs`; `mod.rs` (documentation
  only).
- `crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs` (modified).
- `docs/architecture/p2-custody-store-implementation-boundary.md`
  (modified: section 8 and the R2 notes).
- `docs/evidence/p2-v1-r3b-i3-i1-r2/` (new).

Byte-identical to the base (`checks/scope.txt`):
- `owner.rs`, `exchange.rs`, `recorder.rs`, `disposition.rs`, `faults.rs`,
  `format.rs`, `io.rs` and `sim.rs`;
- the custody core and codec (`core.rs`, `model.rs`, `codec.rs`), their
  tests, and `support/custody/mod.rs`;
- the cleanup observer and its regression, the live harness and the
  package-layout test;
- production `src/`, workflows, `AGENTS.md`, `CLAUDE.md`, the manifests,
  `Cargo.lock` and the toolchain;
- the R5 design, and every earlier evidence directory, R1's included.

`open.rs` and `mod.rs` are narrowly editable under the mission. `open.rs`
changed because the session's verification is the store's own scan, and
reporting every condition (finding F3) needs the scan itself to collect.
`mod.rs` gained documentation only.

## 2. The findings at the base

`baseline/r2_base_probes.rs` is a test target added to an isolated copy of
the base tree. `baseline/provenance.txt` shows the copy holds every one of
the base's 2972 blobs unchanged, plus only that file (SHA-256
`19d1886d…`). The probes reach the store only through
`custody::store::...` call sites, in safe Rust, from outside the store
module. The commands, exit codes and complete output are
`baseline/commands.txt`, `baseline/probe-build.txt` and
`baseline/probe-run.txt`. `baseline/base-store-sources.SHA256SUMS` hashes
the base's store sources.

| | What the base allowed | Observed (`baseline/probe-run.txt`) |
|---|---|---|
| B-S1 | Succession with no verification | The predecessor held an unresolved, undispositioned incident. `successor()` with no verification was Ok (32 steps) and ran. `PROVISION` then selected the successor (revision 2) with the statement "predecessor fully dispositioned". A fresh opening saw 0 current and 0 blocking incidents, without the predecessor's binding, and a fresh owner STARTED (Ready) on the successor. The predecessor's pool file is unchanged (UnsealedAction, refused). |
| B-S2 | A free-form statement | The predecessor's verification refused, Invalid ("disposition … root"). The succession still ran, and the successor recorded "predecessor fully dispositioned", which does not name the condition. |
| B-S3 | No verify-after | All 32 steps returned Ok. The session's events end with `Reselect(2)`: no verification after it. The probe's own verification of the selected successor refused, `MaintenanceIncomplete("journals/.tmp-00000")`. |
| B-S4 | A stale verification | The verify-before saw 1 current incident, dispositioned. A revocation then moved the disposition to `revoked/`. `successor()` after that was Ok and ran, and a fresh owner STARTED on the successor. |
| B-S5 | Capacity loses the report | With 4 claim gaps and 1 Malformed pool file, at `incident_limit` 4, the result was `Err(Capacity("5 current incidents"))`. With claim 100 alone, it was `Err(Capacity("99 claim gaps"))`. The `Err` carries `{refused, revision}` only: no classes, bindings, facts, disposition status or history. |
| B-S6 | Invalid loses all but the first condition | With two conditions, only the first was reported (`disposition … root`). With that one removed, the second appeared (`duplicate generation`). |

## 3. The corrected boundary

The full description is section 8 of
`docs/architecture/p2-custody-store-implementation-boundary.md`. In brief:

- **The session's own complete verification** (F3, F6).
  - **Owner openings are unchanged.** An owner's opening and the standalone
    verifier still stop at the first condition, with the same refusal.
  - **Sessions collect.** The scan meets every refusing check through a
    findings collector. In-session verification records every condition it
    can establish, each with a class and a canonical, printable name.
  - **Classes.** Invalid (determinate, accepted by name), MaintenanceIncomplete
    (recover first), Capacity (a count, never permission), and Indeterminate.
    Indeterminate stops the scan, and the verification is then not complete.
  - **Claim gaps.** They are enumerated while there are at most as many as the
    store could ever hold dispositions for (4096). Beyond that they are
    counted, and no complete set exists.
  - **Retained, bound, taken once.** The session retains the verification
    (`Verification`, a private type), bound to its mutation count, its
    opening, and the root id, revision, digest and state name of its
    selection. It is taken once. A caller gets `Assessment` copies, which
    authorize nothing.
- **Gate and verify-after** (F4, F6).
  - Every session procedure has a gate before its first step and before any
    operation. The gate refuses if any step of the session ran since the
    procedure was built.
  - Its in-session verify-after runs within its last step. A procedure is
    complete only after the verify-after; a verify-after clears the
    verification.
  - The generic verify-after fails on lost authority or a remaining leftover
    temporary. Succession's is strict.
- **Succession** (F1, F2).
  - **Verify-before.** It consumes a complete verification with:
    - every claim gap enumerated;
    - no leftover;
    - every current incident exactly dispositioned, over the complete set;
    - no archived incident without its disposition.
  - **Owner acceptance.** The Owner's acceptance must equal the verified set
    of Invalid condition names (sorted). An omission, an addition or a
    repetition refuses.
  - **The statement.** It is generated from the verified set, or reads
    `none`. It is at most 512 bytes and never truncated.
  - **A new root id**, and a gate on lock and `PROVISION`.
  - **Order.** The 29 operations of R5 section 15.3 (3 more per further pool
    file). The successor's lock is adopted before its `PROVISION` is
    published, and both stores' locks are held until the session ends. Then
    re-selection, and a strict verify-after: the successor, as selected,
    verifies with no condition and no incident.
  - **Failure.** Nothing is rolled back. A failed verify-after leaves the
    succession not complete, and the selected successor stays as evidence.
- **The audit** (F6): `matrices/maintenance-procedure-authority.md`.
  - P-DISP now restates only a verified incident: its facts come from the
    verification, or from an archived journal's bytes. The Owner gives only
    words.
  - P-PROV and R-REPUBLISH refuse while `PROVISION` exists, or while another
    root may hold history.
  - P-ARCH runs over Capacity again (R5 section 13.10).
  - Every session procedure gained its gate and verify-after.
  - The deliberate non-repairs are listed with their R5 grounds.
- **The tests** (F5). The 84 adapted tests publish dispositions only for
  verified incidents, with Owner words. They authorize successions only
  after an asserted in-session verification and with an exact acceptance.
  - The R1 recount check (`b06`) now plants the second disposition file as
    disk data.
  - `b08` stops its revocation after the first step. A completed
    procedure's verify-after now also clears the verification, which would
    otherwise mask R1's lapse at a step's start (section 6).

## 4. What each finding meets now

| | Behaviour (tests) | Behavioural controls | API/type guards |
|---|---|---|---|
| B-S1 / F1 | Refused before any write while one bound incident (or archived one) lacks a disposition (`v01`, `v16`, `v11`); authorized once all have one (`v01`, `v02`) | NC-SUCC-NO-VERIFY, NC-SUCC-MISSING-DISPOSITION, NC-SUCC-UNDISPOSITIONED-HISTORY | P-VERIFICATION-FORGE, P-VERIFICATION-TRANSPLANT, P-TAKE-ASSESSMENT, P-SESSION-VERIFIED |
| B-S2 / F2 | Exact acceptance; generated statement; 512-byte bound (`v08`, `v09`, `v20`) | NC-SUCC-OMIT-INVALID, NC-SUCC-INVENT-INVALID, NC-SUCC-REPEAT-INVALID, NC-SUCC-STATEMENT-CALLER, NC-SUCC-STATEMENT-TRUNCATED | — |
| B-S3 / F4 | The successor verified in-session after re-selection; a failing verify-after leaves it not complete and later openings refuse (`v02`, `v10`); every session procedure ends in its verify-after (`v19`, `v21`, `v24`) | NC-SUCC-NO-POSTVERIFY, NC-SUCC-POSTVERIFY-IGNORED, NC-SUCC-VERIFY-OLD-ROOT, NC-DISP-NO-POSTVERIFY, NC-LEFTOVER-AFTER-ACCEPTED | — |
| B-S4 | A verification older than a step, or used once already, authorizes nothing; a succession overtaken after authorization is refused by its gate before any operation (`v04`, `v12`) | NC-SUCC-STALE, NC-SUCC-REUSE-AUTH, NC-SUCC-NO-GATE, NC-SUCC-MUTATE-BEFORE-GATE, NC-SUCC-GATE-PROVISION | — |
| B-S5 / F3 | Capacity with a complete proof authorizes; beyond the gap bound, refused (`v06`, `v07`); archival over capacity (`v18`) | NC-SUCC-CAPACITY-BYPASS, NC-GAP-UNBOUNDED, NC-SUCC-CAPACITY-REFUSED, NC-ARCHIVE-CAPACITY | — |
| B-S6 / F3 | Every condition, in order, once; an owner's opening unchanged; Indeterminate refuses (`v08`, `v14`, `v20`, `v23`) | NC-VERIFY-FIRST-ONLY, NC-OWNER-COLLECTS, NC-ENTRY-STAT-DETERMINATE, NC-REVOKED-BOUND-INDETERMINATE, NC-NAMES-EXAMINED-TWICE | — |
| F6 | A disposition restates only a verified incident (`v15`, `v16`); provisioning never replaces a store (`v17`); a successor is a new root of a recovered predecessor (`v22`); both locks held (`v13`) | NC-DISP-UNREPORTED, NC-DISP-FACTS-NOT-VERIFIED, NC-DISP-ARCHIVED-NAME-ONLY, NC-PROV-OVER-EXISTING, NC-REPUBLISH-OVER-EXISTING, NC-PROV-OVER-HISTORY, NC-SUCC-LEFTOVER, NC-SUCC-SAME-ROOT, NC-SUCC-DUAL-LOCK, NC-VERIFY-AFTER-AUTHORIZES | — |

`matrices/successor-requirements.md` maps every requirement of the mission's
sections 8, 9, 11, 12 and 13 (S01 to S18, A to F) to its tests, controls and
guards, each with its as-run result.

## 5. Regression results (snapshot `5b396d83`; `validation/`)

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store` | 0 | 111 passed: 108 store tests and the core's 3 commitment-boundary tests. The store tests are 75 from I3-I1 and 9 R1 regressions, adapted to the R2 interface, plus 24 R2 regressions, `v01` to `v24`. |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec` | 0 | 21 passed |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core` | 0 | 47 passed |
| `cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_store --test phase2_custody_codec --test phase2_custody_core -- -D warnings` | 0 | no warning |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation` | 0 | 36 passed |
| `cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run` | 0 | built, never run |
| `cargo fmt --all -- --check` | 0 | clean |
| `git diff --check` | 0 | clean |
| store tests again, `--nocapture --test-threads=1` | 0 | 111 passed (the traces) |

- **What the tests touch.** The native tests `n01` to `n08` run inside the
  store target, unprivileged, on files they create beneath
  `CARGO_TARGET_TMPDIR`. Nothing opens a real store, qualifies a host, sysfs
  or a device, mounts, performs privileged I/O, uses a bus or a runner, or
  runs a live sandbox.
- **Toolchain.** rustc and cargo 1.94.0, clippy 0.1.94, rustfmt 1.8.0,
  Python 3.12.3, host kernel 6.17.0-35-generic (`validation/toolchain.txt`).
- **Before the commit.** `git diff --cached --check` ran on the staged
  candidate. Its result is in the mission's final report.

## 6. Controls, by kind, never added together

- **Behavioural store controls** (`controls/store/`,
  `matrices/behavioural-controls.md`). Each control restores one wrong
  behaviour. It must compile and fail its one intended test, on the
  assertion that carries its marker and required text. Every file must then
  be restored byte for byte (SHA-256), with the checkout's status unchanged.
  Results, all as required:
  - simulator and source conformance: 12 of 12;
  - implementation safety: 69 of 69 (3 exercise the Linux primitives);
  - authority and API surface: 8 of 8;
  - R1 authority binding: 10 of 10;
  - R2 maintenance authority: 38 of 38. These are new mutations, each
    reopening one maintenance defect:
    - verify-before: NC-SUCC-NO-VERIFY, NC-SUCC-STALE, NC-SUCC-REUSE-AUTH,
      NC-VERIFY-AFTER-AUTHORIZES;
    - the complete set: NC-SUCC-MISSING-DISPOSITION,
      NC-SUCC-REPORT-TRUNCATION, NC-SUCC-UNDISPOSITIONED-HISTORY,
      NC-SUCC-LEFTOVER;
    - Capacity: NC-SUCC-CAPACITY-BYPASS, NC-GAP-UNBOUNDED,
      NC-SUCC-CAPACITY-REFUSED, NC-ARCHIVE-CAPACITY;
    - Invalid acceptance and the statement: NC-SUCC-OMIT-INVALID,
      NC-SUCC-INVENT-INVALID, NC-SUCC-REPEAT-INVALID,
      NC-SUCC-STATEMENT-CALLER, NC-SUCC-STATEMENT-TRUNCATED;
    - the collecting verification: NC-VERIFY-FIRST-ONLY, NC-OWNER-COLLECTS,
      NC-ENTRY-STAT-DETERMINATE, NC-REVOKED-BOUND-INDETERMINATE,
      NC-NAMES-EXAMINED-TWICE;
    - gate, order and locks: NC-SUCC-NO-GATE, NC-SUCC-MUTATE-BEFORE-GATE
      (caught by the trace assertion), NC-SUCC-GATE-PROVISION,
      NC-SUCC-DUAL-LOCK, NC-SUCC-SAME-ROOT;
    - verify-after: NC-SUCC-NO-POSTVERIFY, NC-SUCC-POSTVERIFY-IGNORED,
      NC-SUCC-VERIFY-OLD-ROOT, NC-DISP-NO-POSTVERIFY,
      NC-LEFTOVER-AFTER-ACCEPTED;
    - the audit's repairs: NC-DISP-UNREPORTED, NC-DISP-FACTS-NOT-VERIFIED,
      NC-DISP-ARCHIVED-NAME-ONLY, NC-PROV-OVER-EXISTING,
      NC-REPUBLISH-OVER-EXISTING, NC-PROV-OVER-HISTORY.

  NC-SUCC-FOREIGN has no behavioural call site (section 9). Its intention is
  carried by the API/type guard P-VERIFICATION-TRANSPLANT.
- **R1 to R2 mapping** (`matrices/control_mapping.md`), generated from both
  scripts:
  - 92 controls unchanged;
  - 7 adapted, each with its anchor moved to the equivalent defect point.
    I-PRESERVATION-SKIPPED and I-PROVISION-INODE-IGNORED moved because the
    scan's checks now meet the findings collector. I-SESSION-NEW-LOCK,
    I-SESSION-SHARED, I-SESSION-RELOCK and I-VERIFY-CACHED moved because
    `verify_inner` and `verify` now go through it. R1-VERIFICATION-REUSED
    moved because the retained verification is now a bound `Verification`.
    Each keeps its id, test, marker, required text and intention;
  - 38 added.

  A-ADMISSION-UNBOUND stays remapped to API/type guards, as in R1. No
  compile failure is counted as a behavioural detection.
  `scripts/scripts.adaptation.diff` is the adaptation.
- **One control was masked by an R2 protection, and the test was
  strengthened.** In the first isolated full run,
  R1-VERIFICATION-NEVER-LAPSES (procedure steps no longer lapse the
  verification) left `b08` passing:
  - `b08` ran its revocation to completion;
  - the revocation's R2 verify-after then cleared the verification anyway.

  `b08` now stops the revocation after its first step, where the lapse at a
  step's start is the only protection left, and checks the completed case
  separately. The control itself is unchanged and is detected.
- **The I2-R1 suite** (`controls/i2r1-rerun/`). I3-I1's runner was rerun
  unchanged: the six files it pins are byte-identical to I3-I1's. Results:
  - 32 of 32 counted controls as required;
  - 4 informational runs.

  Every result is identical to R1's recorded rerun
  (`controls/i2r1-rerun/comparison-with-r1.txt`, from
  `scripts/compare_i2r1.py`).
- **API/type guards** (`controls/api-probes/`, `matrices/api-type-guards.md`).
  Compile-time probes from outside the boundary. Results, all as required:
  - 18 privacy guards: R1's 15, and R2's P-VERIFICATION-TRANSPLANT,
    P-VERIFICATION-FORGE and P-TAKE-ASSESSMENT. Each fails with exactly its
    expected `E0451`, `E0603`, `E0616` or `E0624` error.
  - Self-check: each probe builds once its access is reopened in the scratch
    copy, whose sources are then restored and verified. P-SESSION-VERIFIED
    and P-VERIFICATION-TRANSPLANT reopen both the `verified` field and its
    type. In R2 the field's type is private as well, a second guard.
  - 1 ownership guard: `close` twice is `E0382`, and closing the returned
    owner builds.
  - A positive control (the ordinary public use of the R2 interface builds)
    and a harness check (a deliberate `E0308` is seen).
- **R5 design-control coverage** (`matrices/coverage-matrix.md`). It is
  regenerated with I3-I1's generator, unchanged. Of 67 design controls, 66
  have a counted Rust mutation and 1 is historical (Python model only), as
  in I3-I1 and R1. Its title still reads I3-I1 because the generator is
  unchanged.
- **Successor requirement coverage** (`matrices/successor-requirements.md`).
  Every mapped test, control and guard is as required: 141 mapped items, none missing.
- **The maintenance procedure authority matrix**
  (`matrices/maintenance-procedure-authority.md`) and **the complete-module
  self-review** (`matrices/module-self-review.md`). These are the section
  10 and section 16 audits.

## 7. Repository checks (`checks/`)

- **R5 design model.**
  `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json ...`
  exits 0:
  - R1's counterexamples 16 of 16 reproduced;
  - R2's composed counterexample 3 of 3;
  - R4's and R5's findings on the R3 and R4 models 3 of 3 each;
  - baseline 33 of 33;
  - negative controls 67 of 67 caught;
  - restored 33 of 33.

  Its JSON is byte-identical to the committed `p2-v1-r3b-i3-p-r5/coverage.json`
  (`cmp` exit 0).
- **R5 reference self-check.**
  `python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test --kernel <dir>`
  passes. It checks 204 kernel citations in 35 files, each file verified by
  SHA-256, and every self-test corruption is reported. The kernel inputs
  (Linux v6.17 copies) are R1's, kept at
  `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r1-work/kernel/`; their SHA-256 list
  has SHA-256 `5ed404adfb87c3054481454e30617808aefdb725cb05c774830213998b597005`,
  as in R1.
- **Historical manifests.** Each manifest matches its own snapshot, and the
  evidence inside each directory is unchanged: P-R1 to P-R5, I3-I1 and R1
  (snapshot `ed7c7088`). That is 433 entries, all OK.
- **Scope.** Every changed path lies in the mission's envelope. Every
  protected file and tree is unchanged, including `owner.rs`, `exchange.rs`,
  `recorder.rs`, `disposition.rs`, `faults.rs` and R1's evidence. There is
  no `.gitattributes` anywhere (`checks/scope.txt`, taken before the
  commit).
- **The adaptation diff.** `checks/adaptation-diff-apply.txt` shows
  `scripts/scripts.adaptation.diff`, as committed, turning R1's scripts
  into this directory's R2 copies byte for byte.
- **Refs, triggers and hooks.** `scope/preflight-before-commit.txt` gives,
  before the commit:
  - the ten protected refs and the candidate branch, locally and on
    `github`;
  - every workflow's push trigger: none matches
    `review/p2-v1-custody-store-implementation`;
  - the active hooks: none;
  - the commit identity.

## 8. Provenance and normalization

- **Raw outputs.** They are kept outside the repository, in
  `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r2-work/`: `baseline/`,
  `candidate-runs/`, `checks-final/`, `matrices/`, `scope/` and `raw/`. The
  assembly script is `assemble.sh`.
- **Normalization.** Each copy here went through `scripts/normalize.py`
  (R1's, unchanged in substance):
  - trailing spaces, tabs and carriage returns are removed from each line;
  - blank lines at the end are removed;
  - exactly one final newline is kept.

  Nothing else is rewritten: no assertion message, command, exit status or
  semantic output. `normalization.tsv` lists every copied file with its raw
  and normalized SHA-256 and byte counts. Of 260 copied files, 252 are
  unchanged and 8 were normalized. No
  `.gitattributes` exemption is used, and `.gitignore` is unchanged.
- **The adaptation diff.** `scripts/scripts.adaptation.diff` is the unified
  diff from each R1 script to its R2 copy. Its blank context lines lost
  their single space in normalization. `patch` still applies it to R1's
  scripts and reproduces this directory's copies byte for byte
  (`checks/adaptation-diff-apply.txt`).
- **Logs.** The `*.log` files here are force-added, as in R1. The ignore rules
  are unchanged.
- **Builds.**
  - **Isolated runs.** The candidate runs used a scratch Git checkout of the
    snapshot and an initially empty target directory outside the repository,
    used by nothing else. The mutating runs restored and verified every
    file, and the checkout's status was empty before and after each step.
  - **A discarded run.** An earlier candidate run shared its target
    directory with a diagnostic checkout of the same tree. Cargo hashes a
    path package relative to its workspace root, so both checkouts used the
    same artifacts. The copy had also kept the files' older modification
    times, so cargo treated the diagnostic checkout's last build, made with
    a control's mutation in place, as fresh. One store test then failed,
    with that mutation's message. The run was stopped and discarded. Every
    run since uses its own empty target directory.
  - **A masked control.** The first full run with an isolated target found
    one control not as required: R1-VERIFICATION-NEVER-LAPSES (section 6).
    `b08` was then adapted, the snapshot retaken, and every run repeated.
    The figures in sections 5 and 6 are from that repeat only.

## 9. Limitations and open items

- **A safe-API boundary only.** It holds within this crate's safe code. It
  does not defend against `unsafe` code in the same process.
- **No behavioural call site for the selection binding.** The
  verification's binding to its selection and opening cannot be reached
  behaviourally: the session's mutation count lapses the verification
  first. It is defence in depth, shown by API/type guards
  (P-VERIFICATION-TRANSPLANT is NC-SUCC-FOREIGN's intention).
- **P-PROV's step 6.** The standalone verifier, run as the store uid, stays
  the Owner's act: the root procedure cannot act as another identity
  (`matrices/maintenance-procedure-authority.md`).
- **Observed, not repaired: a repeated succession.** After a crash past R5
  section 13.6 step 2a, the repeat refuses with EEXIST. R5 section 15.2
  skips 2a when the kept copy matches the current `PROVISION`. This is
  fail-closed, unchanged since I3-I1, and has no test. It is outside the
  authority findings and is reported for the Architect.
- **`dispositions/revoked/` has no bound in P-REVOKE** (R5 gives none). A
  store pushed over it is Invalid, and succession can accept that condition
  by name.
- **Nothing to authorize, by design.** Several roots that may hold history,
  or more claim gaps than the store could hold dispositions for, leave the
  succession unauthorized. Both are for the Architect.
- **Gates.** G-AUTH, G-HOST, G-LIVE, G-NATIVE and G-PWR stay open, and A-S5
  stays rejected. The errseq limitation stays documented, not detected.
- **Out of scope.** Nothing here authenticates storage, proves native cleanup
  or power-loss behaviour, or qualifies a host. Nothing here is live
  acceptance, integration or Phase Two completion.

## 10. Reproducing

From a clean Git checkout of the candidate whose `HEAD^{tree}` is the
candidate's tree, with `CARGO_TARGET_DIR` set to an empty directory used by
nothing else:

```
bash docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/candidate_runs.sh <checkout> <out>
bash docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/checks.sh <repository> <out> <kernel dir>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/matrices.py <store summary.json> <i2r1 summary.json> <api_probes.json> <out>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/control_mapping.py docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/store_controls.py docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/store_controls.py <out> <store summary.json>
python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/coverage_matrix.py docs/evidence/p2-v1-r3b-i3-p-r5/coverage.json docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/store_controls.py <store summary.json> crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs <out>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/requirements.py <store summary.json> <api_probes.json> <validation/store-tests.txt> <out>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/compare_i2r1.py docs/evidence/p2-v1-r3b-i3-i1-r1/controls/i2r1-rerun/summary.json <i2r1 summary.json>
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/preflight.py <repository> github <candidate branch SHA> <out file>
```

The control and probe scripts mutate and restore files in the checkout, so
run them alone. The baseline is reproduced on an isolated copy of the base
tree with `baseline/r2_base_probes.rs` added under
`crates/nexus-verifier-sandbox/tests/`, using the commands in
`baseline/commands.txt`.

## 11. Contents

| Path | Content |
|---|---|
| `baseline/` | The B-S1 to B-S6 probe source, its provenance, the base tree listing, the base store sources' SHA-256, and the build and run with their commands and exit codes |
| `validation/` | The section 17 commands' outputs, the toolchain, the validated sources' SHA-256, and the candidate runs' log |
| `checks/` | The R5 model and reference outputs, the historical manifests, the scope check, and the adaptation diff's application |
| `controls/store/` | The counted store controls: the summary and one log per control |
| `controls/i2r1-rerun/` | The I2-R1 rerun: the summary, its logs, and the comparison with R1's rerun |
| `controls/api-probes/` | The API/type probes: results, a table, and one log per build |
| `matrices/` | The behavioural matrix, the API/type matrix, the R1 to R2 mapping, the R5 coverage matrix, the successor requirement coverage, the maintenance procedure authority matrix, and the complete-module self-review |
| `scope/` | The ref readback, workflow triggers, hooks and identity before the commit |
| `scripts/` | The R2 control script, probes, mapping, matrices, requirements, I2-R1 comparison, validation, checks, candidate runs, scope, preflight and normalization, and the R1-to-R2 adaptation diff |
| `normalization.tsv` | Every copied output: raw and normalized SHA-256 |
| `SHA256SUMS` | Every file of this directory but itself |
