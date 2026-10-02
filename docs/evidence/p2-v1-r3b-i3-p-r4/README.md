# P2-V1-R3B-I3-P-R4 design-validation evidence

This directory validates revision R4 of the durable recorder and refusal-store design in `docs/architecture/p2-custody-durable-recorder-design.md`. R4 covers the supported profile and the activation proof. The directory holds the R4 design model and the R4 reference check, run on the design candidate before it was committed.

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P-R4, design and design validation only |
| Design base | `b0f8437636d0bc7d38818e75df71d1bf516cff8e` (the R3 candidate) |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` (tree `cd197e047cb5a57e458b2af5d32f9c5406371301`) |
| Earlier evidence | `docs/evidence/p2-v1-r3b-i3-p-r1/`, `-r2/` and `-r3/`, unchanged. The R4 model loads the R1, R2 and R3 models from there by path, and checks each SHA-256 before using it. |
| Kernel sources | Linux v6.17, 21 files (§1.4 of the design), fetched from the v6.17 tag and verified by SHA-256 in `reference_check.py`. Three were also compared byte for byte with `git.kernel.org`. |
| Run | On the development host; the UTC time and the Python version are in `validation.txt` |

## What this is, and what it is not

**It is a specification-validation artifact.**
- `design_checks.py` is a standard-library Python model of the R4 design, carried forward from the R3 model.
- It adds:
  - the effective filesystem profile: the option listing, the journal's jbd2 name, the kernel identity and the Owner's qualification as root;
  - `JournalSim`, a small transaction model of the jbd2 and ext4 paths that activation depends on, as read from the Linux v6.17 sources:
    - transactions running, committing or completed, then committed or failed;
    - the abort and emergency read-only flags;
    - which flush sites are checked and which discard their status;
    - checkpointing, the log tail and recovery;
  - a corrected `fsync` model: a completed transaction's `fsync` returns 0 without the abort test.
- Before its own checks, it re-runs the Architect's counterexamples on the unchanged R1 and R2 models, and the R4 findings on the unchanged R3 model, and requires each to reproduce.
- Model-level negative controls restore incorrect behaviours and must fail the intended marked assertions.

**It is not:**
- the store, an activation, a maintenance session or any other implementation;
- a kernel, or a test of the Rust code, the kernel, ext4 or any device.

The model opens no store, calls no native operation, performs no privileged I/O, and causes no real crash or power loss. Nothing ran on a host: no directory sync, timestamp probe, profile read, state root, Cargo command, live test binary, systemd or bus operation, mount, provisioning step or workflow. The profile values (option listing, jbd2 names, kernel identity) are fixtures in the form Linux v6.17 prints them, not observations of any host. No live reproduction of any finding is claimed.

**The record sequences, the core model and the journal model are design artifacts.**
- Fixtures T1–T15 and `CoreSim` were derived by reading the cited source paths at the source baseline.
- The journal behaviour was derived by reading the cited kernel sources.
- None of these is an execution result.

## Files

| File | Role |
|---|---|
| `design_checks.py` | The R4 model: the reproductions on the R1, R2 and R3 models, checks C00–C29, 61 negative controls and the harness. Writes `coverage.json` with `--json`. |
| `reference_check.py` | See **What the reference check covers** below. |
| `validation.txt` | Everything needed to audit the run, listed in **What `validation.txt` holds** below. |
| `coverage.json` | Machine-readable results, listed in **What `coverage.json` holds** below. |
| `SHA256SUMS` | SHA-256 of the design document and of every other file here |

**What `validation.txt` holds:**
- the R3 baseline, reproduced first;
- the historical manifests (R1, R2, R3), each checked against its own committed snapshot;
- the exact R4 commands, with complete output and exit statuses;
- the tested files' SHA-256;
- the source traces of F1 to F4;
- the primary-source record;
- the development record;
- the scope checks;
- the environment.

**What `coverage.json` holds:**
- the reproductions;
- each check's marker, requirement, mission items and result;
- each control's target and assertion;
- the crash matrix;
- the composed dimensions and traces;
- the activation operations;
- the activation-proof enumeration;
- the profile's required and excluded options;
- the assertion and model changes;
- the assumptions the model does not establish.

**What the reference check covers.** Against the Git objects of the source baseline:
- every Rust `file:line` reference in the design (115);
- the model's 50 golden-vector literals.

Against the model, that the design says exactly what the model computes, performs and runs:
- the §15.2 crash matrix and coverage counts;
- the §15.3 operation table;
- the §10.6 activation operations;
- the §15.4 composed dimensions;
- the §10.7 enumeration counts;
- the §6.4 required and excluded options and journal inode;
- the §7.5 `PROVISION` keys;
- the §16.1 check table and control count.

Every Linux `file:line` citation in the design (121) must have an expectation, and every expectation must be cited. With `--kernel DIR`, each citation's first and last line is checked against local copies of the 21 cited v6.17 files, each verified by SHA-256. `--self-test` adds in-memory corruptions, each of which must be reported: eleven of the document, one of the model's golden vectors, and with `--kernel` one of a kernel expectation. A successful line check is not a semantic proof.

## Reproducing

From the repository root, in a clone that contains the source baseline commit and the R1, R2 and R3 evidence:

```
python3 -B docs/evidence/p2-v1-r3b-i3-p-r4/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r4/reference_check.py --self-test [--kernel <dir>]
```

`<dir>` holds the 21 Linux v6.17 files listed in `reference_check.py` (`KERNEL_FILES`), under their source paths or with `/` replaced by `_`.

**Exit status.** Each command exits 0 only on a complete pass. For the model, that means all of the following:
- every R1 and R2 counterexample reproduces on its own unchanged model;
- every R4 finding reproduces on the unchanged R3 model;
- every baseline check passes;
- every negative control is caught by an assertion carrying its target's marker;
- for NC-ACT-COMPOSE, the earlier snapshot checks C12, C20, C21 and C22 also still pass under the mutant;
- the restored baseline passes;
- no tool failure occurred.

**How failures are classified.**
- Anything other than an `AssertionError` is a tool failure, never a caught control.
- A control whose assertion lacks its target's marker is `WRONG-MARKER`.
- A composed-only control that also breaks a snapshot check is `NOT-COMPOSED-ONLY`.

## The R4 findings, re-run on the R3 model

| Id | What reproduces |
|---|---|
| R3-F4 | R3's probe state has no running or completed distinction. After the probe's transaction completed and a later commit aborted the journal, R3's `fsync_file` returns EIO; v6.17's `jbd2_complete_transaction` returns 0 (`journal.c:800-805`). |
| R3-F4-REACH | Over R3's 27 checks, R3's `fsync_file` ran 3484 times and never took that branch. No R3 result depended on it. |
| R3-PROFILE | R3's opening decides the profile from the pinned `mountinfo` strings, which omit the superblock's defaults. It claims on a host whose effective `nobarrier`, or whose external journal, the R4 opening refuses. |

The R1 counterexamples (16) and the R2 composed counterexample (3) are re-run on their own models, as in R3.

## Coverage

R4 mission items, with the checks that exercise each:

| Item | Requirement | Checks |
|---|---|---|
| 1 | The supported profile; each unsupported profile refused before the claim | C27 |
| 2 | The profile established from effective state: never from an absent string, a version prefix or an attestation | C27 |
| 3 | Unknown, contradictory or unqualified profile information refused | C27 |
| 4 | A qualified profile activates; established stores claim; completed successions work | C25, C27 |
| 5 | The transaction windows: running, committing or completed; an abort before, inside or after the probe's handle start | C28 |
| 6 | A failed dependency persistence never becomes activation, even behind later successes | C25, C28 |
| 7 | A harmless late error is not mislabelled as lost history | C28 |
| 8 | The completed-transaction return represented | C29 |
| 9 | Discarded flush statuses never replaced by invented aborts; A-S5 shown necessary | C29 |
| 10 | Checkpoint and log-tail interactions within the declared bounds | C28, C29 |
| 11 | Every R3 composed scenario preserves discoverability or refuses | C26 |

The R1, R2 and R3 checks C00–C26 keep their identities, markers and requirements, and their output is identical to R3's. `coverage.json` records all four mappings.

**Safety and conformance are separate checks.** C28 (`[activation-proof]`) asks whether the activation protocol is safe on the modelled journal: every certified activation has its dependencies committed, and they survive every later continuation in domain H and a power loss. C29 (`[journal-conformance]`) asks whether the modelled journal follows the cited branches.

**The seven new negative controls:**

| Control | Target | Behaviour restored |
|---|---|---|
| NC-PROF-JOURNAL | C27 | An external journal accepted |
| NC-PROF-MODE | C27 | An excluded mode in the effective listing accepted |
| NC-ACT-UNQUALIFIED | C27 | An unqualified profile authorizes: the profile taken from the pinned `mountinfo` strings and the attestation, the kernel never compared (R3) |
| NC-PROF-GUARD | C27 | The effective profile not checked again after the syncs (A4) |
| NC-PROOF-ORDER | C28 | The probe before the directory syncs |
| NC-FSYNC-COMPLETED | C29 | R3's `fsync` model restored |
| NC-ERR-DETECTED | C29 | A discarded flush status treated as detected |

R1's 27 controls, R2's 21 and R3's 6 are kept with the same identities, targets and intent.

## Assertion and model changes

The model prints the full list.
- **Retained output.** Every retained check's detail line and every retained control's assertion is identical to the R3 output.
- **F4 correction.** `fsync_file` follows `jbd2_complete_transaction`. No R3 check took the old branch.
- **Probe windows.** `touch()` models an abort landing between ext4's and jbd2's handle tests, and activation's hook sees the two probe windows. No retained check uses either.
- **Profile.** Every opening checks the effective profile, and `PROVISION` has a `kernel` field. The model's host is the qualified profile.

No R1, R2 or R3 assertion was deleted or weakened.

## Assumptions the model does not establish

- **Physical storage:** A-S1 to A-S5 (design §5). A-S5, flush success, is new in R4. C29 shows it is necessary: v6.17 discards the status of the checkpoint flush.
- **Metadata ordering:** A-M1. The per-directory schedules do not assume it.
- **Journal behaviour:** A-M2 as corrected (design §5.1), read from the Linux v6.17 sources and not tested. The running kernel's exact build must be qualified (G-HOST).
- **Qualification facts:** the superblock facts (an internal journal at inode 8, no `fast_commit`), which the store uid cannot read at runtime and the Owner keeps.
- **Kernel semantics as documented:**
  - errseq and `flock` per open file description;
  - non-blocking FIFO open;
  - clean-page reclaim regardless of open descriptors.
- **One filesystem:** the profile's requirement, which the model represents with one journal per host.
- **Cryptography:** SHA-256 collision resistance.
- **Adversary:** domain A stays outside the guarantees, and gate G-AUTH stays open.
- **Core and implementation:** the real core's journals (I9), the implementation's mutex linearization and lock order are left to the implementation mission.
- **Bounds:** the composed scenarios and the journal enumeration are bounded, not proofs.

## Primary sources

The design's §1.4 lists them with versions:
- Linux man-pages 6.7 (Ubuntu `manpages` 6.7-2), including `uname(2)` and `readlink(2)`, added for R4;
- the 21 Linux v6.17 files, whose SHA-256 values are in `reference_check.py` and `validation.txt`;
- the kernel errseq and vfs documentation;
- `libc` 0.2.183, which declares `fsync`, `fdatasync`, `futimens`, `utimensat`, `readlinkat` and `uname` for Linux.

## Output normalization

None for outputs: `validation.txt` holds the commands' standard output and standard error verbatim, and the outputs contain no absolute paths. In the echoed command lines, paths outside the repository are written as `<scratch>`.
