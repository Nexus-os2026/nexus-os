# SB-3 D1: Mother composition oracles (SB3D-R1, R2; SB3C-R5)

Test-only. The route is unchanged. Base: C1 `bc5eb000`, plus a merge of C2 `64da66fd` for the SB3D-R1
Q2 `SessionPort::inner_for_test` seam only.

D1 lives in its own files so it does not collide with C2 (SB3D-R2):
- `mother/src/adapter/session_model_bridge_d_tests.rs`;
- its `mod` line at the end of `adapter/mod.rs`;
- this register.

Every case runs in a child process under the C1b runner, which bounds each case's wait.
`sb3_d1_schedules_run_in_child_processes` asserts that each test body is exactly one child-runner call.
It is the Mother-side counterpart of the broker watched-join guard, which scans only broker files.

## Oracles

Real C1 caller → SessionPort → session broker → fake peer.

- **O1** `sb3_unrecorded_reply_is_unusable`: recorded, owed and uncertain cases.
  - Owed and Uncertain are injected through the seam, because the native store is held EXCLUSIVE
    (SB3D-R1 Q1). The native debt itself is B's evidence: SB-3B2-C `seal/owed`, SB-3B2-M
    `settled:ok:uncertain`, and B1e `evidence_failure`.
  - The calls give `record_unavailable` then `invalidated`. The end is
    `block:broker_refused:evidence_unavailable`, with no LaneReturned, File dispatch or effect.
  - The charge is retained and there is one chat. The broker's own record is still `recorded`.
  - Later work and `close()` are refused with `Recording`.
- **O2** `sb3_late_content_never_projects`: invalidation at three Mother-reachable points (SB3D-R1
  Q3; the broker interior is B2's reply:after and record:before cells).
  - The points are: the held chat, the `model_replied` hook, and after the lane returned (final
    result admission).
  - There is no LaneReturned, File dispatch, accepted output or content in the journal. The native
    record stays conclusive.
- **O3** `sb3_first_terminal_cause_survives_model_failure`:

  | Case | Expected end |
  |---|---|
  | debt-then-stop | `block:broker_refused:evidence_unavailable` |
  | stop-then-debt | `stop:owner`, with recording debt kept as a separate fact |
  | budget-then-stop | **OPEN:SB3C-F1**: today `Stopped` / `stop:owner` |
  | emergency | `stop:emergency_stop` |
  | fail-then-stop | `fail:lane_fail:builder` |
  | stop-then-fail | `stop:owner` |

- **V1** `sb3_canaries_stay_out_of_model_evidence`: note, prompt, reply and error canaries, and
  their hex forms.
  - They are absent from the Mother journal, the whole session broker tree (SQLite, WAL, SHM, File
    store), Advance, errors, reasons, `close_obligations()` and `startup_report()`. This holds for the
    replied and owed cases.
  - The native store exists and holds the expected record.
- **V2** `sb3_positive_reply_uses_only_intended_memory_path`: recorded, failed, unrecorded and late
  cases.
  - The reply's content appears only on the intended paths: the private store and the File run's
    staging.
  - Every private entry is owner-only.
  - Failed, unrecorded and late replies leave no deliverable on any path.

## Known defect OPEN:SB3C-F1 (SB3C-R5)

A held chat on the session route meets the budget deadline. The broker's own mission expiry is seen
first, and the lane gets `failed:model_timeout` with the File cell already engaged. An Owner Stop that
then arrives while the lane is still running ends the mission `Stopped` / `stop:owner`.

The ordinary route keeps `Blocked` / `block:budget_time` for the same schedule. Without the Stop, the
session mission also ends `block:budget_time`.

The cell asserts today's behaviour exactly. The guard binds the tag to this one cell, and production
unit SB3C-F1 flips it.
