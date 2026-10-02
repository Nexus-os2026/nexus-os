# P2-V1-R3B-I3-P-R2 design-validation evidence

This directory validates revision R2 of the durable recorder and refusal-store design in `docs/architecture/p2-custody-durable-recorder-design.md`. It contains the R2 design model and the R2 reference check, run on the design candidate before it was committed.

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P-R2, design and design validation only |
| Design base | `34cb35d833ef9dd317708615211fee7d02d2eb78` (the R1 candidate) |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` (tree `cd197e047cb5a57e458b2af5d32f9c5406371301`) |
| R1 evidence | `docs/evidence/p2-v1-r3b-i3-p-r1/`, unchanged. The R2 model loads the R1 model from there, by path, and checks its SHA-256 before using it. |
| Run | On the development host; the UTC time and the Python version are in `validation.txt` |

## What this is, and what it is not

**It is a specification-validation artifact.**
- `design_checks.py` is a standard-library Python model of the R2 design, carried forward from the R1 model and corrected for the Architect's findings F1–F4 and section 8.
- It works on in-memory byte images and simulated state: kernel-visible, durable and pending bytes; writeback errors with per-description cursors; page reclaim apart from inode eviction; process death and power loss; directory metadata under ordered and per-directory persistence schedules; open-file-description locks; the recorder's exchange, fatal latch, failure delivery and store admission gate; `PROVISION` selection and revalidation; maintenance sessions.
- Before its own checks it re-runs each of the Architect's counterexamples on the **unchanged R1 model** and requires each to reproduce.
- It checks the design's protocols against the model's own semantics. Model-level negative controls restore incorrect behaviours, most of them R1's, and must fail the intended marked assertions.

**It is not:**
- the store, a maintenance session, an admission gate or any other implementation;
- a test of the Rust code, the kernel, ext4 or any device.

The model opens no store, calls no native operation, performs no privileged I/O, and causes no real crash or power loss. No Cargo command, live test binary, systemd or bus operation, mount, provisioning step or workflow ran.

**The record sequences and the core model are design artifacts.** Fixtures T1–T15 and the core model (`CoreSim`) were derived by reading the cited source paths at the source baseline. They are not Rust execution results. Conformance of the real core's journals (I9 in the design, §16.3) is left to the implementation mission.

## Files

| File | Role |
|---|---|
| `design_checks.py` | The R2 model: the re-run of the counterexamples on the R1 model, checks C00–C24, 48 negative controls, and the harness. Writes `coverage.json` with `--json`. |
| `reference_check.py` | Checks against Git objects of the source baseline every `file:line` reference in the design (115) and the model's 50 golden-vector literals. It also checks that the design's §15.2 crash matrix and coverage counts, its §15.3 operation table, and its §16.1 check table say exactly what the model computes, performs and runs. `--self-test` adds six in-memory corruptions, each of which must be reported. |
| `validation.txt` | The R1 baseline reproduction, the exact R2 commands with their complete output and exit statuses, the tested files' SHA-256, the development record, the scope checks, and the environment |
| `coverage.json` | Machine-readable results: the counterexample reproductions; each check's marker, requirement, mission items and result; each control's target and assertion; the computed crash matrix; the assertions changed from R1; the assumptions the model does not establish |
| `SHA256SUMS` | SHA-256 of the design document and of every other file here |

## Reproducing

From the repository root, in a clone that contains the source baseline commit and the R1 evidence:

```
python3 -B docs/evidence/p2-v1-r3b-i3-p-r2/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r2/reference_check.py --self-test
```

- **Exit status.** Each command exits 0 only on a complete pass. For the model, that means:
  - every counterexample reproduces on the unchanged R1 model;
  - every baseline check passes;
  - every negative control is caught by an assertion carrying its target check's marker;
  - the restored baseline passes;
  - no tool failure occurred.
- **Classifying failures.** The harness classifies anything other than an `AssertionError` as a tool failure, never as a caught control. That covers syntax, import and fixture errors. A control that fails an assertion without its target's marker is reported as `WRONG-MARKER`, not as caught.
- **Other copies.** `reference_check.py` reads the design document and the model next to it by default. `--repo`, `--doc` and `--model` point it at other copies, for example an extracted commit.

## Counterexamples re-run on the R1 model

| Id | Finding | What reproduces |
|---|---|---|
| R1-A | F1 | Worker loss with three records acknowledged and none issued: no failure delivered, admission still permitted |
| R1-B | F1 | A conflict on acknowledged record 1: latched, never delivered; R1's INV-4 contradicted |
| R1-C (five runs) | F1 | Zero, out-of-range and foreign submissions never delivered; a future sequence stored silently; a conflict beyond the next record leaving records pending for ever |
| R1-D | F1 | Loss between a durable reservation and its admission: admission permitted |
| R1-E | F1 | A record in flight published and acknowledged after a conflict latched |
| R1-F | F1 | Loss during the claim and during the seal left both states `None` |
| R1-F2 | F2 | The standalone verifier inside maintenance was Busy |
| R1-F3 | F3 | A startup that read `PROVISION` before a succession and locked afterwards became Ready on the predecessor while another became Ready on the successor |
| R1-F4a, b, c | F4 | A link before the directory sync had one F2 outcome (absent); the `LOCK` creation's root sync was not in the text and pool files were durable at creation; clean pages were not reclaimable with a descriptor open |
| R1-S8 | Section 8 | 5000 dispositions and 5000 revoked entries all examined |

## Coverage

R2 mission section 9 items, with the checks that exercise each:

| Item | Requirement | Check (marker) |
|---|---|---|
| 1 | Worker loss with no pending or unissued record | C18 `[fatal-total]` |
| 2 | Loss between durable reservation and admission | C18 `[fatal-total]` |
| 3 | An acknowledged-sequence conflict | C18 `[fatal-total]` |
| 4 | Zero, foreign, future and out-of-range submissions | C18 `[fatal-total]` |
| 5 | Fatal signal versus publication and admission ordering | C19 `[admission-fence]` |
| 6 | Unexpected acknowledgement outcomes | C18 `[fatal-total]` |
| 7 | Lock-aware verification without lock conversion or reacquisition | C20 `[session-verify]` |
| 8 | Exclusion of competitors during maintenance | C20 `[session-verify]` |
| 9 | `PROVISION` replacement between read and lock | C21 `[provision-selection]` |
| 10 | A stale predecessor startup after successor publication | C21 `[provision-selection]` |
| 11 | Metadata persistence before an explicit directory sync | C22 `[metadata-schedules]` |
| 12 | Administrative faults under the corrected schedules | C12 `[admin-crash]`, C22 `[metadata-schedules]` |
| 13 | Protocol and model step parity | C23 `[step-parity]` |
| 14 | Bounded aggregate scanning and reporting | C24 `[aggregate-bounds]` |

The R1 checks C00–C17 keep their identities, markers and requirements, and still cover R1's twelve mission items (C01–C12). `coverage.json` records both mappings.

**Negative controls.** Each restores one incorrect behaviour. The targets and the assertion each one tripped are in `validation.txt` and `coverage.json`. R1's 27 controls (NC01–NC17, with lettered variants) are kept with the same identities, targets and intent. The 21 new ones:

| Controls | Target | Behaviour restored |
|---|---|---|
| NC18a | C18 | No store admission gate: admission continues after a fatal condition (R1) |
| NC18b | C18 | R1's failure targeting: the cause's own sequence, only at that position |
| NC18c | C18 | A submission for an unissued future sequence stored silently (R1) |
| NC18d | C19 | Publication after the fatal latch (R1) |
| NC18e | C18 | An unexpected acknowledgement outcome ignored |
| NC18f | C18 | `record_failed` for an unissued record counted as delivered |
| NC19 | C19 | A health check followed by an unprotected admission call |
| NC20a–NC20f | C20 | In-session verification through a new shared lock (R1); a lock converted; a lock released around verification; Busy taken as success; authority from a flag; a verification skipped by reusing an earlier report |
| NC21a, NC21b | C21 | No post-lock revalidation (R1); revalidation through the descriptor kept from the read |
| NC22a, NC22b | C22 | Entries durable only through a directory sync (R1); no page reclaim with a descriptor open (R1) |
| NC23 | C23 | An undocumented directory sync in recycling |
| NC24a–NC24c | C24 | Bounds checked after the entries are examined; a startup authorized from a truncated report; retirement preconditions evaluated on a truncated report |

## Assertions changed from R1

Each change keeps the underlying requirement; the model prints the same list.
- **C03, C05:** recorder EIO and loss are read from the exchange's one fatal latch instead of R1's separate `failed` and `lost` fields. R1's eviction became inode eviction in the same scenarios; page reclaim with descriptors open is separate (C22).
- **C08:** the join-handle check is part of every apply step, so a vanished worker is latched at the first apply instead of at a separate call. The timing is stronger.
- **C09, C16:** restart images also include page reclaim, so the counts grew; every R1 assertion applies to every image.
- **C10:** the `PROVISION` grammar gained `revision`, with two more non-canonical variants.
- **C11, C12:** procedures run inside maintenance sessions. C12 runs under the ordered and per-directory schedules, checks recovery, and counts outcomes instead of crash points; its safety assertions are unchanged.
- **C15:** "nothing acknowledged at or after the failed sequence" became "nothing acknowledged beyond P", which covers every fatal cause.

No R1 assertion was deleted or weakened.

## Assumptions the model does not establish

- **Physical storage:** A-S1 flush honesty, A-S2 4096-byte containment, A-S3 no silent loss and A-S4 read stability (design §5). C04 shows that A-S2 is load-bearing.
- **Metadata ordering:** A-M1 (design §4.8). The ordered schedules use it; the per-directory schedules are an over-approximation that does not, and they show that the safety results do not depend on it.
- **Kernel semantics as documented, modelled and not tested:** errseq sampling at open and reporting per open file description; `flock` per open file description; non-blocking FIFO open; clean-page reclaim regardless of open descriptors.
- **Filesystem:** ext4 overwrites of written, unshared extents need no allocation.
- **Cryptography:** SHA-256 collision resistance.
- **Adversary:** domain A is outside the guarantees, so gate G-AUTH stays open.
- **Core and implementation:** `CoreSim` and the fixtures are source-derived. The implementation's mutex linearization, lock order and the real core's journals are left to the implementation mission.
- **Interface:** the maintenance session is a specified future interface; no executable exists or is authorized.

## Model simplifications

- Directory descriptors and component-by-component `O_PATH` walks are not modelled.
- Mutex discipline is a property for code review; each critical section is one atomic step of the interleaving enumerator.
- The per-directory schedules over-approximate ext4; the ordered schedules rest on A-M1.
- The core is represented by the fixtures, a model of `Ledger::acknowledge` and `Ledger::fail`, and the source-derived `CoreSim`.
- The model's verifier stops at the first refusal.

## Primary sources

The design's §1.4 lists them with versions:
- Linux man-pages 6.7 (Ubuntu `manpages` 6.7-2);
- the kernel errseq, vfs and FIEMAP documentation;
- Linux v6.17 `fs/open.c` and `Documentation/ABI/stable/sysfs-block`;
- PostgreSQL `data_sync_retry` (corroboration);
- `libc` 0.2.183.

## Output normalization

None. `validation.txt` holds the commands' standard output and standard error verbatim. The outputs contain no absolute paths, timings or other host-specific values beyond those listed in its header.
