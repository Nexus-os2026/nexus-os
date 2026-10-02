# P2-V1-R3B-I3-P-R3 design-validation evidence

This directory validates revision R3 of the durable recorder and refusal-store design in `docs/architecture/p2-custody-durable-recorder-design.md`: durable activation and composed recovery. It contains the R3 design model and the R3 reference check, run on the design candidate before it was committed.

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P-R3, design and design validation only |
| Design base | `8f8882521535db82a6ba08867c95518040a71cd2` (the R2 candidate) |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` (tree `cd197e047cb5a57e458b2af5d32f9c5406371301`) |
| Earlier evidence | `docs/evidence/p2-v1-r3b-i3-p-r1/` and `docs/evidence/p2-v1-r3b-i3-p-r2/`, unchanged. The R3 model loads the R1 and R2 models from there, by path, and checks each SHA-256 before using it. |
| Run | On the development host; the UTC time and the Python version are in `validation.txt` |

## What this is, and what it is not

**It is a specification-validation artifact.**
- `design_checks.py` is a standard-library Python model of the R3 design, carried forward from the R2 model.
- It adds the journal behaviour that activation depends on, as read from the Linux v6.17 sources the design cites:
  - a directory sync through a new descriptor;
  - a read-only superblock;
  - a commit that fails silently in the commit thread;
  - emergency read-only;
  - the timestamp probe.
- It adds owner and session activation, and composed scenarios that join administrative procedures, owner work and repeated crashes.
- Before its own checks, it re-runs the Architect's counterexamples on the unchanged R1 and R2 models, and requires each to reproduce.
- Model-level negative controls restore incorrect behaviours and must fail the intended marked assertions.

**It is not:**
- the store, an activation, a maintenance session or any other implementation;
- a test of the Rust code, the kernel, ext4 or any device.

The model opens no store, calls no native operation, performs no privileged I/O, and causes no real crash or power loss. Nothing ran on a host: no directory sync, timestamp probe, state root, Cargo command, live test binary, systemd or bus operation, mount, provisioning step or workflow.

**The record sequences, the core model and the journal model are design artifacts.**
- Fixtures T1–T15 and `CoreSim` were derived by reading the cited source paths at the source baseline.
- The journal behaviour was derived by reading the cited kernel sources.
- None of these is an execution result.

## Files

| File | Role |
|---|---|
| `design_checks.py` | The R3 model: re-runs of the counterexamples on the R1 and R2 models, checks C00–C26, 54 negative controls, and the harness. Writes `coverage.json` with `--json`. |
| `reference_check.py` | See **What the reference check covers** below. |
| `validation.txt` | Everything needed to audit the run: the R2 baseline reproduced first; the composed counterexample on the R2 model; the historical manifests checked against their own snapshots; the exact R3 commands with complete output and exit statuses; the tested files' SHA-256; the primary-source record; the development record; the scope checks; and the environment. |
| `coverage.json` | Machine-readable results: the counterexample reproductions; each check's marker, requirement, mission items and result; each control's target and assertion; the computed crash matrix; the composed dimensions and representative traces; the activation operations; the assertion and model changes; the assumptions the model does not establish |
| `SHA256SUMS` | SHA-256 of the design document and of every other file here |

**What the reference check covers.** Against the Git objects of the source baseline:
- every `file:line` reference in the design (115);
- the model's 50 golden-vector literals.

Against the model, that the design says exactly what the model computes, performs and runs:
- the §15.2 crash matrix and coverage counts;
- the §15.3 operation table;
- the §10.6 activation operations;
- the §15.4 composed dimensions;
- the §16.1 check table.

With `--kernel DIR`, it also checks every kernel citation against local copies of the cited v6.17 files, verified by SHA-256. `--self-test` adds eight in-memory corruptions, each of which must be reported.

## Reproducing

From the repository root, in a clone that contains the source baseline commit and the R1 and R2 evidence:

```
python3 -B docs/evidence/p2-v1-r3b-i3-p-r3/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r3/reference_check.py --self-test [--kernel <dir>]
```

`<dir>` holds the twelve Linux v6.17 files listed in `reference_check.py` (`KERNEL_FILES`), under their source paths or with `/` replaced by `_`.

**Exit status.** Each command exits 0 only on a complete pass. For the model, that means all of the following:
- every R1 and R2 counterexample reproduces on its own unchanged model;
- every baseline check passes;
- every negative control is caught by an assertion carrying its target's marker;
- for NC-ACT-COMPOSE, the earlier snapshot checks C12, C20, C21 and C22 also still pass under the mutant;
- the restored baseline passes;
- no tool failure occurred.

**How failures are classified.**
- Anything other than an `AssertionError` is a tool failure, never a caught control.
- A control whose assertion lacks its target's marker is `WRONG-MARKER`.
- A composed-only control that also breaks a snapshot check is `NOT-COMPOSED-ONLY`.

## Counterexamples re-run on the R2 model

| Id | What reproduces |
|---|---|
| R2-W-ASIS | As is, the R2 model rejects the claim on the successor. Its worker rechecks identity (W5) through the fixture's first root, which is an unmodelled transition: design W5 uses the opened root's journals descriptor. |
| R2-W | A successor's `PROVISION` was visible but not durable. An owner claimed, acknowledged an `ActionStarted` and admitted an action. In 10 of 14 power-loss outcomes, a fresh owner was Ready on the predecessor with that generation outside the decision. |
| R2-P | In the same case for a first `PROVISION`, the store refuses as Unprovisioned. R2's recovery premise ("holds no history") is false, and following it gives a fresh, Ready store without that history. |

The R1 counterexamples (16) are re-run on the R1 model, as in R2.

## Coverage

R3 mission items, with the checks that exercise each:

| Item | Requirement | Checks |
|---|---|---|
| 1 | The root-changing witness | C25, C26 |
| 2 | The initial-provisioning variant; an absent selection never permits a fresh replacement | C25, C26 |
| 3 | Activation of established stores and completed transitions | C25 |
| 4 | Activation failures before any claim | C25 |
| 5 | Process death or power loss during activation | C25 |
| 6 | Stale readers and competing owners | C25 |
| 7 | Persistence before and after explicit sync, both families | C22, C26 |
| 8 | Persistence domains: one filesystem verified; independent directories over-approximated | C25, C26 |
| 9 | A second crash after dependent records became durable | C26 |
| 10 | Safe conservative refusals and their recoveries | C12, C26 |
| 11 | Every administrative procedure composed with dependent work | C26 |
| 12 | A silent journal commit failure before activation | C25 |
| 13 | The verifier never activates; sessions activate before verifying | C25 |

The R1 and R2 checks C00–C24 keep their identities, markers and requirements; `coverage.json` records all three mappings.

**The six new negative controls**, each targeting C25 unless noted:

| Control | Behaviour restored |
|---|---|
| NC-ACT-VISIBLE | A freshly revalidated, visible selection treated as activated |
| NC-ACT-ORDER | Claim, acknowledgement and admission before the activation |
| NC-ACT-ERROR | A failed or uncertain activation sync treated as success |
| NC-ACT-PROBE | No certification probe: a sync that returns 0 after a silent commit failure is trusted |
| NC-ACT-REVALIDATE | The identity and selection revalidation around activation omitted |
| NC-ACT-COMPOSE (target C26) | The whole R2 protocol: no activation, and a fresh root after Unprovisioned. The snapshot checks C12, C20, C21 and C22 still pass under it; only the composed check detects it. |

R1's 27 controls and R2's 21 are kept with the same identities, targets and intent.

## Assertion and model changes

The model prints the full list.
- **Retained output.** Every retained control's assertion, and every retained check's detail line except C23's, is identical to the R2 output, although owners and sessions now activate. C23 now also checks the R3 re-publication recovery: 11 procedures and recoveries instead of 10.
- **Worker binding (model correction).** The worker's identity recheck now uses the root the owner opened, as design W5 specifies.
- **Lock bookkeeping (model correction).** `flock` ownership survives a world clone.
- **Generation redraw (model correction).** The redraw now includes archive headers, as design §7.7 specifies.

No R1 or R2 assertion was deleted or weakened.

## Assumptions the model does not establish

- **Physical storage:** A-S1 to A-S4 (design §5).
- **Metadata ordering:** A-M1. The per-directory schedules do not assume it.
- **Journal behaviour:** A-M2 (design §5.1), read from the Linux v6.17 sources and not tested. The running kernel must be qualified (G-HOST).
- **Kernel semantics as documented:**
  - errseq and `flock` per open file description;
  - non-blocking FIFO open;
  - clean-page reclaim regardless of open descriptors.
- **One filesystem:** the profile's requirement, which the model represents with one journal per host.
- **Cryptography:** SHA-256 collision resistance.
- **Adversary:** domain A stays outside the guarantees, and gate G-AUTH stays open.
- **Core and implementation:** the real core's journals (I9), the implementation's mutex linearization and lock order are left to the implementation mission.
- **Bounds:** the composed scenarios are a bounded enumeration, not a proof.

## Primary sources

The design's §1.4 lists them with versions:
- Linux man-pages 6.7 (Ubuntu `manpages` 6.7-2): `fsync(2)`, `open(2)` and `utimensat(2)`;
- the twelve Linux v6.17 files cited for A-M2, whose SHA-256 values are in `reference_check.py` and `validation.txt`;
- the kernel errseq and vfs documentation;
- `libc` 0.2.183, which declares `fsync`, `fdatasync`, `futimens` and `utimensat` for Linux.

## Output normalization

None for outputs: `validation.txt` holds the commands' standard output and standard error verbatim, and the outputs contain no absolute paths. In the echoed command lines, paths outside the repository are written as `<scratch>`.
