# Succession: ordinary and recovery operations (P2-V1-R3B-I3-I1-R3, mission sections 6 to 8)

## Which path a succession takes

Read at authorization and again at the succession's gate, before any
operation (`maintenance::successor`, with `kept_copy` and
`state_root_absent`):

| `<PROVISION_PATH>.predecessor-<old root id>` | Temporaries | Successor root | Result | Tests |
|---|---|---|---|---|
| absent (case A) | none | absent | Ordinary: design section 15.3's 29 operations (one pool file), unchanged | `r301`, `m08` |
| exactly the selected `PROVISION` (case B): regular, `root:root 0444`, one link, opened by identity, its complete bytes the bytes the session selected | none | absent | Repeat: step 2a skipped, operations 7 to 29 (23 for one pool file). The copy is never recreated, rewritten or linked over | `r302`, `r313` |
| anything else (case C): a byte changed, missing or added; another owner, mode or link count; a directory, FIFO or symbolic link | any | any | Refused before any operation; the copy is left as it is | `r303`, `r304`, `r314` |
| any | `<PROVISION_PATH>.predecessor.tmp` or `<PROVISION_PATH>.tmp` | any | Refused until R-LEFTOVER removes it | `r305` |
| absent or exact | none | exists | Refused until R-SUCCESSOR removes it | `r306` |

A copy or a root that appears after authorization makes the gate refuse,
with no operation (`r314`).

## The operations

Labels and object names as design section 15.3; the rows are the
implementation boundary's section 9.3, which the tests parse.

| Procedure | Operations (one pool file) |
|---|---|
| P-SUCCESSOR, ordinary | 29: step 2a (6), step 2b (18), step 2c (5) |
| P-SUCCESSOR, repeat | 23: step 2b (18), step 2c (5) |
| R-SUCCESSOR, complete step 2b | 11: unlink `journals/pool`; fsync_dir `journals`; rmdir `dispositions/revoked`; fsync_dir `dispositions`; rmdir `root/journals`, `root/dispositions`, `root/archive`; unlink `root/LOCK`; fsync_dir `root`; rmdir `parent/root`; fsync_dir `parent` |
| R-SUCCESSOR, partial root | Only the objects present are removed; a directory none of whose entries is removed is not synced; `fsync_dir` `parent` always ends it |
| R-SUCCESSOR, root already gone | 1: fsync_dir `parent` |

Each further pool file adds 3 operations to step 2b, and one `unlink` to
R-SUCCESSOR (before the one `fsync_dir` of `journals`).

## What R-SUCCESSOR requires (mission section 7, preconditions 1 to 12)

| # | Precondition | Where it is read | Tests |
|---|---|---|---|
| 1 | `PROVISION` selects the predecessor, and is the bytes the session selected | `holds_selection`; the digest check | `r309` |
| 2 | The session's lock is retained and its selection revalidates | `holds_selection` (construction and gate) | `r309`, `r310` |
| 3 | The candidate is not the selected store (state name or root id) | `unreferenced` | `r309` |
| 4 | The exact intended target: this store's parent, uid and gid, after an interrupted succession of the selected store (its `PROVISION` kept exactly) | `unreferenced`, `succession_interrupted` | `r309` |
| 5 | Not referenced: not the recorded predecessor; no kept copy for its root id; no kept copy's root id, state root or predecessor naming it; every kept copy readable and parsed | `unreferenced` | `r309` |
| 6 | Only what step 2b makes: the root, `LOCK`, `journals/`, pool files, `dispositions/`, `revoked/`, `archive/` | `inspect_incomplete` | `r306`, `r308` |
| 7 | Each with step 2b's type, owner, mode and link count | `inspect_incomplete` | `r308` |
| 8 | Each pool file at most its size; `LOCK` empty | `inspect_incomplete` | `r307`, `r308` |
| 9 | Every pool byte zero, read in full | `inspect_incomplete` | `r307` |
| 10 | No unexpected entry (bounded listings, exact names) | `inspect_incomplete` | `r308` |
| 11 | `LOCK_EX \| LOCK_NB` on the candidate's `LOCK` and pool files, retained until the procedure ends | `inspect_incomplete` | `r310` |
| 12 | Any ambiguity refuses: an error other than `ENOENT`, a replaced inode, a listing over its bound | throughout | `r307` to `r310` |

Then the removal: by name, bottom-up, each entry only while it is the inode
found, in the directory found (`remove_known`); never recursive (`r308`);
each changed directory synced, the parent last, so the absence is durable
(`r306`). The verify-after verifies the store and requires the candidate
gone (`r306`). Its steps lapse the session's verification, so the repeat
needs a fresh one (`r312`).

## The crash model

- `r3x1`: the ordinary succession's 30 crash points, 400 states (design
  section 15.2's P-SUCCESSOR coverage). Each state is the successor, with
  the predecessor's `PROVISION` kept, or the predecessor. Its recovery
  (R-LEFTOVER, R-SUCCESSOR where the root exists, a fresh verification, the
  succession again) reaches the successor. Step 2a is skipped exactly when
  a copy was kept, and that copy keeps its inode.
- `r3x2`: the repeat's 24 crash points (after F1 at operation 6). Every
  state reaches the successor; the copy keeps its inode and bytes
  throughout.
- `r3x3`: R-SUCCESSOR's crash points, for one pool file (12) and for two
  (13). Every state keeps the predecessor selected and recovers: R-SUCCESSOR
  again (the parent's sync alone exactly when the root is gone), then the
  succession.

The as-run counts of each path are in `crash-model-tallies.md`.
