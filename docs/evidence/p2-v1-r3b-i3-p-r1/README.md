# P2-V1-R3B-I3-P-R1 design-validation evidence

This directory validates the durable recorder and refusal-store design in `docs/architecture/p2-custody-durable-recorder-design.md` (revision R1). It contains a design model and a reference check, run on the design candidate before it was committed.

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P-R1, design and design validation only |
| Design base | `b23ae1ac040732c8332e8a45cc80b10734ca0b36` |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9` (tree `cd197e047cb5a57e458b2af5d32f9c5406371301`) |
| Run | On the development host; the UTC time and the Python version are in `validation.txt` |

## What this is, and what it is not

**It is a specification-validation artifact.**
- `design_checks.py` is a standard-library Python model of the design.
- It works on in-memory byte images and simulated state: kernel-visible, durable and pending bytes; writeback errors with per-description cursors; eviction; process death and power loss; directory entries; open-file-description locks.
- It checks the design's protocols against the model's own semantics.
- Model-level negative controls restore incorrect behaviours and must fail the intended assertions. Several of those behaviours are the first candidate's.

**It is not:**
- the store;
- a test of the Rust code, the kernel, ext4 or any device.

The model opens no store, calls no native operation, performs no privileged I/O, and causes no real crash or power loss. No Cargo command, live test binary or workflow ran.

**The record sequences are design fixtures.** Fixtures T1–T15 were derived by reading the cited source paths at the source baseline. They are not Rust execution results. Conformance of the real core's journals (I9 in the design, §16.3) is left to the implementation mission.

## Files

| File | Role |
|---|---|
| `design_checks.py` | The model, its checks C00–C17, the 27 negative controls and the harness. Writes `coverage.json` with `--json`. |
| `reference_check.py` | Verifies two things against Git objects of the source baseline: every `file:line` reference in the design, and the model's golden-vector literals. `--self-test` adds two in-memory corruptions that must be reported. |
| `validation.txt` | The exact commands, their complete output and exit statuses, the tested files' SHA-256, and the environment |
| `coverage.json` | Machine-readable results: each check's marker, requirement, mission item and result; each control's target and assertion; the assumptions the model does not establish |
| `SHA256SUMS` | SHA-256 of the design document and of every other file here |

## Reproducing

From the repository root, in a clone that contains the source baseline commit:

```
python3 -B docs/evidence/p2-v1-r3b-i3-p-r1/design_checks.py --json <output path>
python3 -B docs/evidence/p2-v1-r3b-i3-p-r1/reference_check.py --self-test
```

- **Exit status.** Each command exits 0 only on a complete pass. For the model, that means:
  - every baseline check passes;
  - every negative control is caught by an assertion carrying its check's marker;
  - the restored baseline passes;
  - no tool failure occurred.
- **Classifying failures.** The model's harness classifies anything other than an `AssertionError` as a tool failure, never as a caught control. That covers syntax, import and fixture errors.
- **Other copies.** `reference_check.py` reads the design document and the model next to it by default. `--repo`, `--doc` and `--model` point it at other copies, for example an extracted commit.

## Coverage

Mission section 5 items, with the check that exercises each:

| Item | Requirement | Check (marker) |
|---|---|---|
| 1 | Process death keeps the visible and durable state apart | C01 `[F1-volatile]` |
| 2 | F1, restart, F2 differs with and without an actual sync | C02 `[F1-F2-sync]` |
| 3 | A sync failure is not repaired by reopening and observing success | C03 `[errseq-reopen]` |
| 4 | Shared-region tear hazards, handled per the supported profile | C04 `[tear-containment]` |
| 5 | Worker failure keeps exclusion while custody is unresolved | C05 `[lock-retained]` |
| 6 | A competing writer is refused before any startup sync | C06 `[busy-before-sync]` |
| 7 | Duplicate delivery cannot block the owner or overflow state | C07 `[dup-bounded]` |
| 8 | Full or disconnected queues cannot hide a fatal recorder condition | C08 `[fatal-latched]` |
| 9 | A non-durable late incident yields uncertainty and refusal | C09 `[late-uncertain]` |
| 10 | Exact header, seal and slot consumption rejects malformed reserved bytes | C10 `[exact-bytes]` |
| 11 | Archival preserves bindings, or changes them explicitly | C11 `[archive-binding]` |
| 12 | Administrative crash points cannot install partial state or retire unresolved history | C12 `[admin-crash]` |

Further checks:

| Check | Marker | What it exercises |
|---|---|---|
| C00 | `[golden-codec]` | The model's frames reproduce the 50 golden version-1 record vectors |
| C13 | `[grammar-conformance]` | The §11.4 grammar on the 13 record fixtures, every prefix of each, and 32 mutations |
| C14 | `[safe-open]` | Special files refused unopened; ext4 identified by mount ID, never by magic |
| C15 | `[ack-durable]` | INV-1 and INV-4 under short writes, zero-progress writes, EINTR and sync EIO |
| C16 | `[no-false-resolution]` | INV-3 at every crash point of every fixture (eager and lazy recording, and a sync failure at each record) |
| C17 | `[arith-bounds]` | Claim exhaustion, configuration, size, `PROVISION`, incident and claim-gap bounds |

**Negative controls.** Each restores one incorrect behaviour. The targets and the assertion each one tripped are in `validation.txt` and `coverage.json`.

| Controls | Target | Behaviour restored |
|---|---|---|
| NC01 | C01 | Process death makes bytes durable |
| NC02 | C02 | Evidence reported as preserved without a sync |
| NC03 | C03 | A new description's sync treated as a certificate |
| NC04 | C04 | First-candidate geometry |
| NC05a, NC05b | C05 | Worker-owned lock descriptions; a duplicated lock description unlocked |
| NC06 | C06 | Sync and read before the lock |
| NC07 | C07 | Bounded submission queue with re-acknowledgement |
| NC08 | C08 | Failure delivered as a queued message |
| NC09 | C09 | Outstanding-only refusal |
| NC10a–NC10e | C10 | Unchecked reserved bytes, seal tail and padding; a checksum-valid invalid header taken as an abandoned claim; non-canonical decimals |
| NC11a, NC11b | C11 | A location-bound binding; a binding that ignores content |
| NC12a–NC12c | C12 | Temporary dispositions read; a `PROVISION` inode mismatch ignored; retirement without preconditions |
| NC13 | C13 | G16 removed |
| NC14a, NC14b | C14 | Blocking open before the type check; ext4 decided by magic |
| NC15a, NC15b | C15 | Durable published before the sync; a sync retried after EIO |
| NC16 | C16 | A visible `RunEnded` taken as resolution |
| NC17 | C17 | An unbounded claim counter |

## Assumptions the model does not establish

The model assumes, and does not establish, the following. It prints the same list.

- **Physical storage:** A-S1 flush honesty, A-S2 4096-byte containment, A-S3 no silent loss and A-S4 read stability (design §5). C04 shows that A-S2 is load-bearing: with a 16 KiB failure unit, a durable `ActionStarted` block can be lost silently.
- **Kernel semantics as documented, modelled and not tested:**
  - errseq sampling at open, and reporting per open file description;
  - `flock` per open file description;
  - non-blocking FIFO open.
- **Filesystem:** ext4 overwrites of written, unshared extents need no allocation.
- **Cryptography:** SHA-256 collision resistance.
- **Adversary:** domain A is outside the guarantees, so gate G-AUTH stays open.

## Model simplifications

- Directory descriptors and component-by-component `O_PATH` walks are not modelled.
- Mutex discipline is a property for code review.
- A rename is durable per directory, which is more conservative than ext4's journal.
- The core is represented by the fixtures and by a model of `Ledger::acknowledge` and `Ledger::fail`.
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
