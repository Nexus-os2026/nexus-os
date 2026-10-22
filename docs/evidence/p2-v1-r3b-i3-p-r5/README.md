# P2-V1-R3B-I3-P-R5 design-validation evidence

This directory validates revision R5 of the durable recorder and refusal store design in `docs/architecture/p2-custody-durable-recorder-design.md`. R5 defines the stable-completion storage contract and closes the design around it. It holds the R5 design model and the R5 reference check, run on the design candidate before it was committed.

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P-R5, design and design validation only |
| Design base | `91d91e5052805f432586b5f1f9a0e2e14ca1af87` (the R4 candidate, published and blocked) |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` (tree `cd197e047cb5a57e458b2af5d32f9c5406371301`) |
| Earlier evidence | `docs/evidence/p2-v1-r3b-i3-p-r1/`, `-r2/`, `-r3/` and `-r4/`, unchanged. The R5 model loads the R1 to R4 models from there by path, and checks each SHA-256 before using it. |
| Kernel sources | Linux v6.17, 35 cited files (§1.4 of the design), fetched from the v6.17 tag and verified by SHA-256 in `reference_check.py`. Every file R5 added was also compared byte for byte with `git.kernel.org`. |
| Run | On the development host; the UTC time and the Python version are in `validation.txt` |

## What this is, and what it is not

**It is a specification-validation artifact.**
- `design_checks.py` is a standard-library Python model of the R5 design, carried forward from the R4 model.
- It adds the storage below the filesystem, as design §5.5 to §5.9, §6.4 step 12 and §10.8 specify it:
  - **The observation.** What the store uid observes: the device's form, from the `/sys/dev/block` link; the kernel's registration of its cache (`queue/write_cache`, `queue/fua`); its transport and identity.
  - **Admission.** The trusted construction of a storage admission, and its check at the claim.
  - **Devices.** Admitted storage, where a completed write is stable and no flush is sent; a volatile write-back cache that receives flushes, with native or emulated FUA; and a volatile cache the kernel registered as absent.
  - **Failures.** Reported write failures, writes in flight, and the superblock's tail write with the abort's rewrite of it, in `JournalSim` and in the whole-store model.
- Before its own checks, it re-runs the reproductions in order: the Architect's counterexamples on the unchanged R1 and R2 models; the R4 findings on the unchanged R3 model; and the R5 findings, R4's checkpoint witness among them, on the unchanged R4 model. Each must reproduce.
- Model-level negative controls restore incorrect behaviours and must fail the intended marked assertions.

**It is not:**
- the store, an activation, a maintenance session or any other implementation;
- a kernel, a device, or a test of the Rust code, the kernel, ext4 or any device;
- a qualification of any host, controller, namespace or configuration.

**Nothing ran on a host or a device.**
- **No store operation.** The model opens no store, calls no native operation, performs no privileged I/O, and causes no real crash or power loss.
- **No storage was read.** It read no sysfs attribute, NVMe Identify data, cache setting or firmware revision.
- **Nothing was run or changed.** Nothing ran: no experiment, benchmark or fault injection; no raw-device access, root command, Cargo command or systemd or bus operation; no mount, provisioning step or workflow. No device cache, queue attribute, firmware setting, mount or kernel was changed.
- **The values are fixtures.** The storage values (the link, the attributes, the identities, the controller's report, the guest flag) are fixtures in the form Linux v6.17 prints them. They qualify nothing.
- **No live reproduction.** None of any finding is claimed.

**The record sequences, the core model, the journal model and the storage model are design artifacts.**
- Fixtures T1–T15 and `CoreSim` were derived by reading the cited source paths at the source baseline.
- The journal and storage behaviour was derived by reading the cited kernel sources.
- None of these is an execution result.

## Files

| File | Role |
|---|---|
| `design_checks.py` | The R5 model: the reproductions on the R1 to R4 models, checks C00–C32, 67 negative controls and the harness. Writes `coverage.json` with `--json`. |
| `reference_check.py` | See **What the reference check covers** below. |
| `validation.txt` | Everything needed to audit the run, listed in **What `validation.txt` holds** below. |
| `coverage.json` | Machine-readable results, listed in **What `coverage.json` holds** below. |
| `SHA256SUMS` | SHA-256 of the design document and of every other file here |

**What `validation.txt` holds:**
- the R4 baseline, reproduced first;
- the historical manifests (R1, R2, R3, R4), each checked against its own committed snapshot;
- the exact R5 commands, with complete output and exit statuses;
- the tested files' SHA-256;
- the primary-source record, with the R5 source traces;
- the development record;
- the scope checks and the protected refs;
- the environment.

**What `coverage.json` holds:**
- the reproductions on the R1, R2, R3 and R4 models;
- each check's marker, requirement, mission items (R1 to R5) and result;
- each control's target and assertion, and the snapshot checks of the composed-only controls;
- the crash matrix, the composed dimensions and traces, the activation operations and the activation-proof enumeration;
- the profile's required and excluded options;
- the storage rule's class, admitted cache values and identity attributes, its refused cases, the storage paths of design §5.8, and the storage-event composition;
- the assertion and model changes;
- the assumptions the model does not establish.

**What the reference check covers.** Against the Git objects of the source baseline:
- every Rust `file:line` reference in the design (115);
- the model's 50 golden-vector literals.

It checks that the design says exactly what the model computes, performs and runs:
- the §15.2 crash matrix and coverage counts;
- the §15.3 operation table;
- the §10.6 activation operations;
- the §15.4 composed dimensions, and (R5) its storage-event dimensions;
- the §10.7 enumeration counts;
- the §6.4 required and excluded options and journal inode;
- (R5) the §6.4 step 12 storage class, admitted cache values, identity prefix and identity attributes;
- the §7.5 `PROVISION` keys;
- the §16.1 check table and control count.

**Linux citations.**
- **Coverage.** Every Linux `file:line` citation in the design (204) must have an expectation, and every expectation must be cited.
- **Line checks.** With `--kernel DIR`, each citation's first and last line is checked against local copies of the 35 cited v6.17 files, each verified by SHA-256.
- **Self-test.** `--self-test` adds in-memory corruptions, each of which must be reported: fifteen of the document and one of the model's golden vectors; with `--kernel`, also one R4 and one R5 kernel expectation.
- **Its limit.** A successful line check is not a semantic proof.

## Reproducing

From the repository root, in a clone that contains the source baseline commit and the R1 to R4 evidence:

```
python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test [--kernel <dir>]
```

`<dir>` holds the 35 Linux v6.17 files listed in `reference_check.py` (`KERNEL_FILES`), under their source paths or with `/` replaced by `_`.

**Exit status.** Each command exits 0 only on a complete pass. For the model, that means all of the following:
- every R1 and R2 counterexample reproduces on its own unchanged model;
- every R4 finding reproduces on the unchanged R3 model;
- every R5 finding reproduces on the unchanged R4 model;
- every baseline check passes;
- every negative control is caught by an assertion carrying its target's marker;
- for NC-ACT-COMPOSE, the earlier snapshot checks C12, C20, C21 and C22 also still pass under the mutant; for NC-STORAGE-COMPOSED, C25, C26, C27 and C28 do;
- the restored baseline passes;
- no tool failure occurred.

**How failures are classified.**
- Anything other than an `AssertionError` is a tool failure, never a caught control.
- A control whose assertion lacks its target's marker is `WRONG-MARKER`.
- A composed-only control that also breaks a snapshot check is `NOT-COMPOSED-ONLY`.

## The R5 findings, re-run on the R4 model

| Id | What reproduces |
|---|---|
| R4-F2-WITNESS | On the unchanged R4 model, activation certifies with results 0, 0, 0, 0. After a checkpoint whose flush fails, the journal is not aborted and d1 does not survive a power loss. A later commit's successful flush keeps it. The witness is conditional, for a volatile write-back cache, and was derived, not observed. |
| R4-PATHS | R4's `JournalSim` models no superblock write outcome, no FUA mode, no failed or incomplete home write and no device without a write cache. Paths (b) and (d) of design §5.8 are outside it. |
| R4-FIXTURE | R4's fixture host is a device-mapper device (`253:1`, `dm-1`), on which R4's opening claimed. R5's opening refuses it as stacked storage. |

The R1 counterexamples (16), the R2 composed counterexample (3) and the R4 findings on the R3 model (3) are re-run on their own models, as in R4.

## Coverage

R5 mission items, with the checks that exercise each:

| Item | Requirement | Checks |
|---|---|---|
| 1 | A qualified stable-completion fixture activates; claims, records and acknowledgements work | C30 |
| 2 | Completed succession; prior history discoverable; disposition and retirement follow the existing authority | C30 |
| 3 | Volatile-completion storage not admitted; unknown or contradictory storage refused before any claim; no activation token for an unsupported configuration | C30, C32 |
| 4 | A spoofed or stale qualification cannot authorize; decoding manufactures no admission | C30 |
| 5 | A reported write failure never becomes an acknowledgement | C31 |
| 6 | An incomplete write never becomes a durable record | C31 |
| 7 | A loss of qualification discards no custody or history | C30, C32 |
| 8 | On admitted storage, R4's witness cannot lose required history | C28, C31 |
| 9 | The volatile paths kept separate; no invented abort; R4's witness retained | C29 |
| 10 | Composed administrative, process-death, startup, dependent-work and second-crash scenarios with storage events | C26, C32 |
| 11 | R4's 30 checks and 61 controls retained; each domain change documented | C26, C28, C29 |
| 12 | Stable completion shown necessary | C31 |

The R1 to R4 checks C00–C29 keep their identities, markers and requirements. C00–C27's output is identical to R4's. C28 and C29 change domain, as below. `coverage.json` records all five mappings.

**Safety and conformance are separate checks.**
- **Safety.** C28 (`[activation-proof]`) and C31 (`[stable-completion]`) ask whether the protocol is safe on the modelled journal and storage.
- **Conformance.** C29 (`[journal-conformance]`) asks whether the modelled journal follows the cited branches. It keeps v6.17's discarded statuses as they are; they still lose history on a volatile cache, and R5 admits no such storage (C30).

**The six new negative controls:**

| Control | Target | Behaviour restored |
|---|---|---|
| NC-STORAGE-VOLATILE | C30 | A volatile-completion profile admitted as stable: `queue/write_cache` alone decides |
| NC-STORAGE-CLAIM | C30 | An unverified storage claim or a stale qualification treated as authority |
| NC-STORAGE-COMPLETION | C31 | Durability published before the operation completes: a home write in flight counted as complete |
| NC-STORAGE-ERROR | C31 | A reported write error converted into a successful acknowledgement |
| NC-STORAGE-TAIL | C31 | Journal history discarded without durable home data or a retained log copy |
| NC-STORAGE-COMPOSED | C32 | The unsafe path restored: storage admitted on the Owner's attestation (R4's domain); C32's oracle must catch the lost history |

R1's 27 controls, R2's 21, R3's 6 and R4's 7 are kept with the same identities, targets and intent; every one of their assertions is identical to R4's output.

## Assertion and model changes

The model prints the full list.
- **Retained output.** Every retained check's detail line and every retained control's assertion is identical to the R4 output, except C28's and C29's.
- **C28, a domain change.**
  - **The old assumption.** A-S5: the continuations ran on a volatile cache with every flush honoured.
  - **The new domain.** Admitted storage, to which no flush is sent. The continuations add failed home and superblock writes, a write in flight and R4's failed flush (12 instead of 6).
  - **The historical counterexample, retained.** R4-F2-WITNESS, and R4's counterexample in C29.
  - **The replacement assertion.** Every certified dependency survives every continuation and a power loss.
  - **The unchanged safety requirement.** No certified activation's dependency is lost in domain H.
- **C29.** R4's "necessity of A-S5" experiment is kept with the same events on a volatile cache, relabelled "R4's counterexample, retained". The paths of design §5.8 and the admitted-storage conformance were added.
- **The fixture.** The host's device changed from a device-mapper device to an NVMe partition (R4-FIXTURE). `MountEnv` keeps the whole `/sys/dev/block` link and the storage attributes.
- **`PROVISION`.** It has the storage fields of design §7.5.

No R1, R2, R3 or R4 assertion was deleted or weakened.

## Assumptions the model does not establish

- **Physical storage:** A-S1 (stable completion, restated in R5), A-S2, A-S3 and A-S4 (design §5.1). C31 shows A-S1 necessary: on a cache the kernel registered as absent, an acknowledged generation is lost and nothing reports it. A-S5 is withdrawn.
- **Hardware truthfulness:** that the controller's report of no volatile write cache is true, and that no layer below the completion, a hypervisor included, holds acknowledged data in volatile memory. The store observes the kernel's registration, never the device.
- **Qualification facts:** the controller's Identify data, that the host is not a virtual machine guest, and the evidence for stable completion. Root establishes them (G-HOST); the store uid cannot observe them. Also the superblock facts (R4).
- **Metadata ordering:** A-M1. The per-directory schedules do not assume it.
- **Journal and block-layer behaviour:** A-M2 and the paths of design §5.5–§5.8, read from the Linux v6.17 sources and not tested. The running kernel's exact build must be qualified (G-HOST).
- **Write-error detection:** errseq's 19-bit counter (design §5.6); not modelled.
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
- Linux man-pages 6.7 (Ubuntu `manpages` 6.7-2), as in R4;
- the 35 cited Linux v6.17 files, whose SHA-256 values are in `reference_check.py` and `validation.txt`, `Documentation/block/writeback_cache_control.rst` among them;
- the kernel errseq and vfs documentation;
- `libc` 0.2.183, as in R4.

No vendor documentation, product data or device was consulted, and none is selected.

## Output normalization

None for outputs. `validation.txt` holds the commands' standard output and standard error verbatim, and the outputs contain no absolute paths. In the echoed command lines, paths outside the repository are written as `<scratch>`.
