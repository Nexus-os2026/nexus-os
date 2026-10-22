# Revocation: capacity and recovery (P2-V1-R3B-I3-I1-R3, mission sections 9 to 11)

Design sections 6.6 and 13.4: `dispositions/revoked/` holds at most 4096
entries, counted from the names before any entry is examined; revoked files
stay as evidence; after an interrupted revocation "the Owner completes the
revocation by unlinking the `dispositions/` name and syncing that
directory".

## What each procedure does, by state

Normal revocation is P-REVOKE (`maintenance::revoke`): construction and gate
read the same admissibility (`revocation_admissible`). R-REVOKE is
`maintenance::resume_revocation`: construction, gate and verify-after read
the same state (`interrupted_revocation`).

| State of the store | P-REVOKE (normal) | R-REVOKE (recovery) | Tests |
|---|---|---|---|
| Active disposition valid, `revoked/` below 4096, revoked name absent | Admissible: design section 15.3's 3 operations; `revoked/` gains one entry | Refuses: "no revoked artifact" | `m03`, `m08`, `r323` |
| `revoked/` at 4095 | Admissible; `revoked/` reaches 4096, which the store still opens | - | `r315` |
| `revoked/` at 4096 | Refuses before any operation (at construction, or at the gate when the last entry appears after it was built). Normal revocation is unavailable; no revoked entry is removed to make room | - | `r316` |
| The revoked name exists (earlier evidence) | Refuses before any operation, at construction or at the gate; the existing file keeps its inode and bytes | - | `r317` |
| The same binding, revoked again at another time | Admissible; both revoked files are kept, each with its own bytes | - | `r318` |
| The time is not a compact UTC time | Refuses | Refuses | `r320` |
| **Split**: one inode under the active and the revoked name, two links (the per-directory over-approximation) | Refuses: the active disposition has two links | 2 operations: unlink the active name (only while it is that inode), `fsync` `dispositions/`. Allowed with `revoked/` at 4096, since it adds no entry | `r319`, `r321` |
| **Completed**: the active name gone, the revoked file alone with one link | Refuses: no active disposition | 1 operation: `fsync` `dispositions/` (its durability is not known) | `r319`, `r3x4` |
| Another file under the revoked name; another incident's or root's disposition there; bytes that are not canonical; another mode; a third link; two links and no active name; no revoked file | - | Refuses before any operation; nothing changes | `r320` |
| The revoked file lost (per-directory family only: the removal from `dispositions/` kept, the addition to `revoked/` lost) | - | Refuses: "no revoked artifact"; the incident blocks | `r3x4` |

Verify-after:
- P-REVOKE: the generic one (section 8.2 of the implementation boundary),
  `r323`.
- R-REVOKE: the generic one, then the revocation must be complete (the active
  name gone, the revoked file alone, the same inode), `r324`.

## Capacity is never permission, and never conflated with recovery

- The count refuses a normal revocation at 4096, because a normal revocation
  adds an entry (`r316`).
- R-REVOKE adds no entry and never reads the count (`r321`; control
  NC-R3-REVOKE-CAPACITY-CONFLATED).
- No count ever authorizes anything; the store never removes revoked
  evidence to make room.

## The crash model (`r3x4`)

P-REVOKE's 4 crash points (0 to 3): F1, and F2 under every ordered and
per-directory schedule with pending data old and new. That is 30 states,
design section 15.2's P-REVOKE coverage. Each next opening reports
"published", "blocks" or "invalid", and the recovery reaches "blocks":

| Outcome | Recovery | Result |
|---|---|---|
| published | P-REVOKE again, at the same time | blocks |
| blocks, with the revoked file | R-REVOKE: the sync alone | blocks |
| blocks, the revoked file lost (per-directory only) | none possible: R-REVOKE refuses | blocks |
| invalid (the exact split) | R-REVOKE, and R-REVOKE interrupted at each of its own 3 crash points (F1; F2 under every schedule): each state is the split again or the completed revocation, and R-REVOKE once more reaches "blocks" | blocks |

The as-run counts are in `crash-model-tallies.md`.
