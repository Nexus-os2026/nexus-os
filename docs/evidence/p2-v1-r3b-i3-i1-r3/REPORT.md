# P2-V1-R3B-I3-I1-R3: administrative crash-recovery closure — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-custody-store-implementation`. A file in a commit cannot name
that commit, so the evidence here is bound to two other things:
- **the baseline**, bound to the repair base;
- **the candidate's results**, bound to the exact source bytes they
  validated (`validation/candidate/sources.SHA256SUMS`), run in a scratch
  Git checkout of a snapshot of the candidate's tree.

The validation rerun on the committed candidate is kept outside the
repository, in `/home/nexus/NEXUS/p2-v1-r3b-i3-i1-r3-work/post-commit/`. The
mission's final report gives it with its hashes.

`ERRATA.md` corrects one statement of the R2 evidence (its `.gitattributes`
sentence). The R2 evidence directory is not modified.

## 1. Identity

| Item | Value |
|---|---|
| Repair base | `66a245c183b04ff1e2ae30fbbd065a57381df09c` (tree `13c689357c5781ebd048f4cee5081028329a729d`, sole parent `ed7c7088247badf87b3b1d483ee57360f867e2f1`) |
| R5 design baseline | `7689e599ed8fc89ec7720d13869ef58cab767ecb`; custody and codec baseline `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` |
| Candidate | The commit containing this file. Sole parent: the repair base. Subject: `test(custody): close administrative crash recovery` |
| Pre-commit snapshot | Tree `6350c192bee87e3523b3f5f7115c6ceaf983d506`: the worktree's tracked and new files when the final candidate runs began, committed into a scratch Git checkout whose `HEAD^{tree}` equals it (`validation/candidate-runs.txt`, `validation/candidate-snapshot.txt`). `validation/candidate-snapshot-files.SHA256SUMS` hashes its sources, documents and evidence files. It differs from the candidate's tree only inside this evidence directory. After the snapshot, two of its 22 evidence files changed: `matrices/module-self-review.md` (its snapshot reference) and `scripts/assemble.sh` (the earlier attempt's outputs, section 7). Every other file of this directory was added: the run outputs, the generated matrices, `scripts/scripts.adaptation.diff`, `validation/normalization.tsv`, `REPORT.md` and `SHA256SUMS`. No file outside this directory, and no script the runs used, differs from the snapshot. The comparison with the commit is in the mission's final report. |

Changed paths, relative to the base:
- `crates/nexus-verifier-sandbox/tests/support/custody/store/`: modified
  `maintenance.rs`, `io.rs` and `sim.rs`; `mod.rs` (documentation only).
- `crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs` (modified).
- `docs/architecture/p2-custody-store-implementation-boundary.md`
  (modified: section 9, and the R3 notes in sections 1, 2 and 8.5).
- `docs/evidence/p2-v1-r3b-i3-i1-r3/` (new).

Byte-identical to the base (`validation/checks/scope.txt`):
- `owner.rs`, `exchange.rs`, `recorder.rs`, `disposition.rs`, `faults.rs`,
  `format.rs`, `open.rs` and `classify.rs`;
- the custody core and codec (`core.rs`, `model.rs`, `codec.rs`), their
  tests, and `support/custody/mod.rs`;
- the cleanup observer and its regression, the live harness and the
  package-layout test;
- production `src/`, workflows, `AGENTS.md`, `CLAUDE.md`, the manifests,
  `Cargo.lock` and the toolchain;
- the R5 design, and every earlier evidence directory, R2's included.

No new store submodule, no new dependency, and no change to NXCD, the
journal or the container format.

## 2. The findings at the base

`baseline/r3_base_probes.rs` is a test target added to an isolated copy of
the base tree. `baseline/provenance.txt` shows the copy holds every one of
the base's 3250 blobs unchanged, plus only that file (SHA-256
`b95804b6…`). The probes reach the store only through `custody::store::...`
call sites, in safe Rust, from outside the store module. The commands, exit
codes and complete output are `baseline/commands.txt`,
`baseline/probe-build.txt` and `baseline/probe-run.txt` (11 passed: the 8
probes and the core's 3). Each probe prints what it observed and asserts
only that. `baseline/base-store-sources.SHA256SUMS` hashes the base's store
sources, and `baseline/base-tree.txt` lists the base tree.

| | What the base allowed | Observed (`baseline/probe-run.txt`) |
|---|---|---|
| R3-B1 | No repeat after step 2a | An authorized succession, F1 after its operation 6: `PROVISION` and its kept copy (durably), the copy equal to the verified `PROVISION`, no successor root. R-LEFTOVER (1 step) and a fresh verification succeeded. The same succession again: `Err("13.6/2a (step 4): linkat: Exist")` after 3 of 32 steps (create, write, fsync of a new `predecessor.tmp`), which it left behind. `PROVISION` still selected the predecessor. |
| R3-B2 | No recovery of an incomplete successor root | F1 after operation 23 (two pool files): the successor root held `LOCK` (1001:1001 0600, empty), the directories (0:0 0755) and `j00000.journal` (1001:1001 0600, 73728 bytes, all zero). The predecessor stayed selected and opened. The repeat failed the same way, and the root stayed. The I/O trait declared no directory removal; `io.rs`'s only `AT_REMOVEDIR` was the Linux fixture helper; maintenance named no removal of a successor root. |
| R3-B3 | A non-zero incomplete root, unexplained | As R3-B2, with a non-zero byte in pool file 0: `roots_with_history` listed the successor root, and after every available procedure it still existed. |
| R3-B4 | A wrong kept copy found by a failing link | A copy one byte off the verified `PROVISION`: the succession ran step 2a and failed at its `link` (`EEXIST`) after 3 of 32 steps, leaving `predecessor.tmp`. The mismatched copy was unchanged. |
| R3-B5 | No recovery of a split revocation | Under one of 2 per-directory schedules, both names held one inode with two links. The owner's opening and the in-session verification refused Invalid ("link count 2"), and R-LEFTOVER changed nothing. P-REVOKE again (another time) returned `Ok(())` and left two revoked names, each with two links: still Invalid. Maintenance named no revocation recovery. |
| R3-B6 | P-REVOKE over `revoked/`'s bound | With 4096 accepted entries, P-REVOKE returned `Ok(())` (3 operations, complete). `revoked/` then held 4097 entries; its verify-after and the next owner's opening refused Invalid ("enumeration bound: dispositions/revoked"). |
| R3-B7 | P-REVOKE replacing revoked evidence | A revocation at a time whose revoked name existed: the name then named another file, with other bytes. `revoked/` held one entry, the second publication's; the first revoked file was gone. |
| R3-B8 | No crash matrix for these recoveries | The repeat after F1 at each of the 30 crash points of a succession: points 0 to 3 completed (29 steps); points 4 to 27 failed at step 2a's `link` (`EEXIST`); at 28 and 29 the successor was already selected. The base had no successor cleanup or revocation recovery to interrupt. |

## 3. Successor recovery

The full description is section 9 of
`docs/architecture/p2-custody-store-implementation-boundary.md`, and
`matrices/successor-operations.md`. In brief:

- **The kept copy decides step 2a** (`successor`, `kept_copy`), at
  authorization and at the gate, before any operation:
  - case A, no copy: the ordinary succession, design section 15.3's 29
    operations, unchanged (`r301`);
  - case B, exactly the selected `PROVISION` (regular, `root:root 0444`,
    one link, opened by identity, complete bytes): step 2a is skipped (23
    operations) and the copy is never recreated, rewritten or linked over
    (`r302`, `r313`);
  - case C, anything else: refused before any operation, and left as it is
    (`r303`, `r304`, `r314`).

  A leftover `predecessor.tmp` or `PROVISION.tmp` refuses until R-LEFTOVER
  (`r305`), and an existing successor root refuses until R-SUCCESSOR
  (`r306`).
- **R-SUCCESSOR** (`recover_incomplete_successor`) removes an interrupted
  succession's incomplete root. It requires, read afresh at construction and
  at its gate:
  - the session's lock and revalidated selection, and `PROVISION` the bytes
    it selected;
  - a candidate of this store (parent, uid, gid), referenced by nothing:
    not the selected store, not its recorded predecessor, and named by no
    kept copy, whether by file name or by the copy's recorded root id,
    state root or predecessor;
  - an interrupted succession of the selected store (its `PROVISION` kept
    exactly);
  - only what step 2b makes, as step 2b makes it, every pool file zero in
    full;
  - `LOCK_EX | LOCK_NB` on `LOCK` and every pool file, held to the end.

  It then removes those objects by identity, bottom-up, never recursively,
  and syncs each changed directory, the parent last (11 operations for one
  pool file). It verifies after, and the candidate must be gone. A crashed
  cleanup is run again; with the root already gone it syncs the parent
  alone. Tests: `r306` to `r312`.
- **`StoreIo::remove_dir`** (R3): `unlinkat(at, name, AT_REMOVEDIR)`,
  descriptor-relative, one empty directory, the name checked as every
  other call's. The native primitive is tested only on the test's own
  entries beneath `CARGO_TARGET_TMPDIR` (`n09`); the simulator models it as
  a pending half of the parent (`s06`). This is not a filesystem
  qualification.
- **The crash model.** `r3x1`: all 400 states of P-SUCCESSOR's crash model
  (design section 15.2's coverage) are the successor or recover to it.
  `r3x2` covers the repeat's own 24 crash points (338 states), and `r3x3`
  R-SUCCESSOR's own crash points with one and two pool files (237 states).
  Every state recovers, and the kept copy keeps its inode
  (`matrices/crash-model-tallies.md`).

## 4. Revocation

`matrices/revocation-capacity-recovery.md` has the state-by-state table.

- **Normal revocation** (`revoke`, now `Result`), before any operation, at
  construction and at its gate:
  - the time must be a compact UTC time;
  - the active disposition is read by the opening's rules;
  - `revoked/` is counted from its names, and at 4096 normal revocation is
    unavailable (`r315`, `r316`);
  - the revoked name must be absent, so revoked evidence is never replaced
    (`r317`, `r318`).

  The operations are design section 15.3's three, unchanged (`m08`). Its
  verify-after runs (`r323`).
- **R-REVOKE** (`resume_revocation`) proves the exact split: the revoked
  file is this store's canonical disposition for the binding, `root:root
  0444`, the same inode as the active name, two links. It unlinks only the
  active name, then syncs `dispositions/` (`r319`). On a completed
  revocation it repeats only the sync. It refuses anything else (`r320`),
  never republishes, adds no entry, and so applies at the bound (`r321`).
  Its verify-after requires completion (`r324`).
- **The crash model.** `r3x4`: all 30 states of P-REVOKE's crash model, and
  R-REVOKE's own crash points from each split state, reach "blocks". In 2
  states, only under the per-directory over-approximation, the revoked
  file is lost and nothing remains to complete (section 10).

## 5. Design section 15.2, row by row

`matrices/r5-15.2-recovery-matrix.md` has each row with its verbatim design
text, implementation, tests and controls. No row is BLOCKED.

| Outcome | Status |
|---|---|
| unprovisioned, after provisioning began | IMPLEMENTED + TESTED: R-REPUBLISH when a `PROVISION.tmp` was left, otherwise provisioning at a new root (`r3x5`: all 338 states reach "fresh"). Removing an all-zero root is an EXPLICIT EXTERNAL OWNER STEP: the design makes it optional ("may"), no store or session exists before a `PROVISION`, and design section 15.3 names no such procedure |
| maintenance | IMPLEMENTED + TESTED (R2's `m09`; R3's `r305`, `r3x1`, `r3x6`) |
| lost, during recycling | IMPLEMENTED + TESTED (R2's `m09`) |
| invalid, after an interrupted revocation | IMPLEMENTED + TESTED (R3: R-REVOKE; `r319` to `r324`, `r3x4`) |
| unsupported, before re-qualification is visible | IMPLEMENTED + TESTED (`r3x6`: all 52 states reach revision 2) |
| predecessor, before the successor's `PROVISION` is visible or durable | IMPLEMENTED + TESTED (R3: the repeat and R-SUCCESSOR; `r301` to `r314`, `r3x1` to `r3x3`) |

The first table of section 15.2 (each procedure's crash outcomes, and the
coverage counts) is unchanged, and `m09` still asserts it.

## 6. Regressions

`matrices/requirements-r301-r328.md` maps each of R301 to R328 to its tests,
its behavioural controls and its API/type guards, with each one's as-run
result. Every requirement is covered: every listed test passed, and every
listed control and guard was as required
(`matrices/requirements-r301-r328.json`).

The store target holds 142 tests: 139 store tests and the core's 3.
- R1 and earlier: 84 (adapted in R2).
- R2: 24 (`v01` to `v24`).
- R3: 31, being 23 regressions (`r301` to `r328`), 6 crash-model recoveries
  (`r3x1` to `r3x6`), the simulator's `s06` and the native `n09`. R325
  (`b01` to `b09`), R326 (`v01` to `v24`) and R327 (`o01`) are carried by
  existing tests, unchanged.

The section 19 commands on the snapshot (`validation/candidate/commands.txt`)
all exited 0:
- store 142 passed, codec 21, core 47, cleanup observation 36;
- clippy with `-D warnings`;
- the live harness built and was not run (`--no-run`);
- `cargo fmt --all -- --check` and `git diff --check`;
- the store tests again with `--nocapture --test-threads=1`, which print
  the crash-model tallies.

The repository checks (`validation/checks/`) all passed:
- the R5 design model, its coverage equal to R5's;
- the R5 reference self-test against the SHA-256-pinned Linux v6.17 files
  (204 citations in 35 files);
- every historical evidence manifest, R2's included;
- the scope check;
- the adaptation check.

## 7. Controls

Behavioural controls and API/type guards are reported apart, and never
added into one figure.

**The store's 179 counted controls**, run serially on the snapshot
(`controls/store/`, `matrices/behavioural-controls.md`): all 179 as
required. Each applied exactly, compiled, failed its intended test at the
assertion carrying its marker (and its required text), and was restored
byte for byte; the checkout's status was unchanged after each.

| Category | Counted | As required | Of which R3's |
|---|---|---|---|
| administrative-recovery | 35 | 35 | 35 |
| authority-api-surface | 9 | 9 | 1 |
| authority-binding (R1) | 10 | 10 | 0 |
| implementation-safety | 71 | 71 | 2 |
| maintenance-authority (R2) | 38 | 38 | 0 |
| simulator-source-conformance | 16 | 16 | 4 |

R2's 137 (`matrices/r2-to-r3-control-mapping.md`): 135 unchanged, the same
edits, test, marker and checks. Two were adapted, each keeping its id,
category, test, marker and required text, because an R3 check had come to
mask R2's mutation (`REANCHORED_R3` in `scripts/store_controls.py`):

- **NC-SUCC-MUTATE-BEFORE-GATE.** It moves the gate after the first
  operation. R3's P-REVOKE gate rereads its admissibility, so with every
  procedure's gate moved, `v12`'s helper revocation refused after its own
  rename, before the succession `v12` examines. The mutation is now
  confined to the succession's gate, which is S17's subject.
- **NC-SUCC-SAME-ROOT.** It removes the new-root-id check. R3 also refuses
  a successor whose state root exists, and `v22` passes the predecessor's
  own layout. The equivalent defect point removes both checks.

R3's 42: 35 administrative-recovery controls, one authority-api-surface
guard (a recursive I/O operation), the simulator's `rmdir` and same-file
rename rules (4), and the native removal (2). Every requirement R301 to
R324 has at least one behavioural control
(`matrices/requirements-r301-r328.md`): R3's own for each, except R313,
whose controls are R2's of the succession's verify-after. A
native control never reaches beyond the test's own fixture: `n09` tries
the names that cannot leave the fixture first.

**The I2-R1 suite**, rerun with I3-I1's unchanged runner
(`controls/i2r1-rerun/`): 32 counted, all as required, and 4 informational
runs; identical, control by control, to R2's rerun
(`controls/i2r1-rerun/comparison-with-r2.txt`).

**API/type guards** (reported apart; `controls/api-probes/`,
`matrices/api-type-guards.md`): 28 results, all as required:
- 25 compile-time probes (R2's 18, R3's 7). Each fails to build with
  exactly its expected error, and builds once that access is reopened.
  R3's show that the session's hold check and every decision and removal
  helper of the R3 recoveries are private to the store;
- the ownership guard `P-CLOSE-TWICE`;
- the positive control, now with the two R3 recoveries;
- the harness check.

**The earlier attempt** (`controls/earlier-attempt/`). The complete run
before the final one used the same sources (snapshot tree `ad142aaf…`). It
found the two masked R2 controls above (177 of 179 as required), and three
R3 probes whose self-check did not build: reopening a function that
returns a private type needs that type reopened too, as R2's
`P-VERIFICATION-TRANSPLANT` reopens its type. The controls and the probes'
reopen edits were corrected, and the final run passed. Two runs before
that were stopped partway: once to add the controls of R301, R304, R311,
R315, R318, R322 and R323, and once to replace two placeholders in section
9.6 of the boundary document. Their partial outputs were discarded, and no
result here comes from them.

## 8. Evidence

| Path | Content |
|---|---|
| `REPORT.md` | This report |
| `ERRATA.md` | The R2 `.gitattributes` sentence, corrected with exact paths and blob ids |
| `baseline/` | The R3-B1 to R3-B8 reproduction at the base: the probe target, its provenance, commands and complete output |
| `validation/candidate/` | The section 19 commands on the snapshot: each command's complete output, the toolchain, and `sources.SHA256SUMS` |
| `validation/candidate-runs.txt`, `validation/candidate-snapshot*` | The run log, and the snapshot's identity and files |
| `validation/checks/` | The repository checks, and the adaptation check |
| `validation/normalization.tsv` | Every copied output's raw and normalized SHA-256 |
| `controls/store/` | Each store control's complete log, `summary.json` and the run's output |
| `controls/i2r1-rerun/` | The I2-R1 suite, rerun with I3-I1's unchanged runner, and its comparison with R2's run |
| `controls/api-probes/` | Each probe's build logs and `api_probes.json` |
| `controls/earlier-attempt/` | The run before the final one: its log, snapshot, store summary and API results, and the five logs that led to the adaptations (section 7) |
| `matrices/` | The R5 section 15.2 matrix; the succession's and the revocation's matrices; the crash-model tallies; R301 to R328; the R2-to-R3 control mapping; the behavioural controls; the API/type guards; the module self-review |
| `scope/` | The preflights (start, before the commit) |
| `scripts/` | Every script, R2's adapted copies with their diff, and R3's own |
| `SHA256SUMS` | Every other file of this directory |

Outputs were copied in through `scripts/normalize.py`. The only
transformation is that trailing whitespace and trailing blank lines are
removed, so that `git diff --check` accepts them without a `.gitattributes`
exemption; each copy's raw and normalized SHA-256 is in
`validation/normalization.tsv`. No `.gitattributes` exemption was added, and
`.gitignore` is unchanged.

## 9. Publication preflight

`scope/preflight-before-commit.txt`, before the commit:
- the ten protected refs and the candidate branch at their expected SHAs,
  locally and on `github`;
- no workflow whose push trigger could match
  `review/p2-v1-custody-store-implementation`;
- no active local hook;
- the commit identity: Suresh Karicheti <nexaiceo@gmail.com>.

RESULT: PASS. `scope/preflight-start.txt` is the same check at the start
of R3, made with R2's script (its header says R2).

## 10. Limits

- **Simulated storage and a fixture host.** Every recovery runs against
  them. The native directory removal is exercised only on the test's own
  entries beneath `CARGO_TARGET_TMPDIR`. Nothing here qualifies a host, a
  filesystem or a device, proves recovery after a physical power loss, or
  proves native cleanup.
- **The per-directory over-approximation can lose a revoked file.** In 2 of
  P-REVOKE's 30 crash states, the removal from `dispositions/` persists and
  the addition to `revoked/` does not: the incident blocks, and R-REVOKE
  has nothing to complete. Under A-M1 the rename is atomic, and this state
  does not occur.
- **A successor root with a non-zero byte stays.** R-SUCCESSOR refuses it,
  since it may hold history. The succession can be repeated at another root
  id; what such a root holds is for the Architect.
- **An all-zero root after an interrupted provisioning stays** until the
  Owner removes it, which the design leaves optional (section 5).
- **P-REVOKE's window.** The revoked name's absence is read at construction
  and at the gate. Between the gate and the `rename`, only a root process
  acting outside every procedure could create it, and design section 13.1
  makes that unsupported.
- **A safe-API boundary only**, as in R1 and R2.
- G-AUTH, G-HOST, G-LIVE, G-NATIVE and G-PWR stay open, and A-S5 remains
  rejected. There is no authentication, host qualification, physical
  power-loss proof, native-cleanup proof, live acceptance or integration.
  Phase Two is not complete.
