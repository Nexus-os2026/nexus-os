# SB-3 D1: Mother composition oracles (SB3D-R1, R2; SB3C-R5)

Test-only. The route is unchanged. Base: main `7f406d2a` (C1 and C2), merged into the earlier D1
branch that had started on C2's `64da66fd` Q2 seam.

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
  - Later work and `close()` are refused with `Recording`. No reply content rests in the journal or
    the private store.
- **O2** `sb3_late_content_never_projects`: invalidation at three Mother-reachable points (SB3D-R1
  Q3; the broker interior is B2's reply:after and record:before cells).
  - The points are: the held chat, the `model_replied` hook, and after the lane returned (final
    result admission).
  - There is no LaneReturned, File dispatch, accepted output, or content in the journal or the private
    store. The native record stays conclusive.
- **O3** `sb3_first_terminal_cause_survives_model_failure`:

  | Case | Expected end |
  |---|---|
  | debt-then-stop | `block:broker_refused:evidence_unavailable` |
  | stop-then-debt | `stop:owner`, with recording debt kept as a separate fact |
  | budget-then-stop | `block:budget_time` (SB3C-F1) |
  | unnoticed lapse then Stop or Stop-all, charge boundary, entry, reply arm, poll crossing, final admission | `block:budget_time` |
  | revoked invocation, then lapse, then Stop | `block:budget_time` (revocation is not in the word) |
  | Stop then lapse; Failed or Closing, then lapse, then Stop | `stop:owner` |
  | scope-only (Q stand-in) then Stop, no running budget | `stop:owner` |
  | emergency, then lapse, then Stop | `stop:emergency_stop` |
  | reply lapse, no Stop | `fail:lane_fail:builder` (no budget cause observed) |
  | budget (no Stop) | `block:budget_time` |
  | emergency | `stop:emergency_stop` |
  | fail-then-stop | `fail:lane_fail:builder` |
  | stop-then-fail | `stop:owner` |

- **V1** `sb3_canaries_stay_out_of_model_evidence`: note, prompt, reply, error and peer-metadata
  (`/api/version`) canaries, their hex forms and their I6 content digests (spec 6.6 also prohibits
  content hashes).
  - They are absent from the Mother journal, the whole session broker tree (SQLite, WAL, SHM, File
    store), Advance, errors, reasons, `close_obligations()` and `startup_report()`. This holds for the
    replied and owed cases.
  - The private store holds the approved contract (the note) and no prompt, reply, error or metadata.
  - The native store exists and holds the expected record.
- **V2** `sb3_positive_reply_uses_only_intended_memory_path`: recorded, failed, rejected (the
  transport refuses an unfinished reply), unrecorded and late cases.
  - The reply's content appears only on the intended paths: the private store and the File run's
    staging.
  - Every private entry, the staging folders and every staged file holding the accepted bytes are
    owner-only. The broker's own `broker/files/ledger.db-wal` is `0644` inside the `0700` folder; that
    is the existing SQLite mode, recorded here and not claimed.
  - Failed, unrecorded and late replies leave no deliverable on any path.

## SB3C-F1 (SB3C-R18): Owner Stop decides budget versus Stop

Once Mother's budget lapses, a session-scoped File cell reads engaged with nothing in its word, so nothing records the
lapse and a later Owner Stop used to win. Earlier repairs tried to observe the lapse at each Caller and broker site;
each review found another site (the last was the charge boundary, where the meter refuses `Stale`).

The rule, in `FileSession::engage_owner_stop`, on both Owner Stop paths and before `OwnerStop` is engaged: if the cell
is session-scoped, its word records no cause and no Closing, Failed, Sealed or Emergency state, the emergency stop is
not set and Mother's budget has lapsed, the time budget is latched first. Stop then proceeds as normal. Any other case
is a plain Stop.

- The reading is `FileLife::check_budget`: two atomics set once at consent, and the clock. They equal the Caller's
  values: `mono_ref` is the consent reading with nothing spent, and the only rebase is at coordinator open, before any
  File session exists. Budget time does not pause while awaiting approval. Before consent both deadlines are
  `u64::MAX`, so no budget is running.
- Stop never waits: atomics and clock reads only, and no lock is held across the latch.
- Q and the backstop are never read. A scope-only lapse leaves Mother's budget unexpired, so Stop stays plain.
- A mission already ended keeps its state and end. Its cell may record the time budget, as any touch already does.

The ordinary route is unchanged: an unobserved lapse there still loses to Stop (`ordinary-lapse-then-stop`). Without a
Stop nothing changes on either route: an unobserved lapse is not given a budget cause (`reply-lapse`).

Limits. A lapse and a Stop inside the one reading can go either way, and so can an emergency or a Failed landing
between the word read and the latch; the first-wins cell decides and the halt is unaffected. Q is not driven (300 s of
real time): narrowed scope limits stand in for it.

No production line of the earlier F1 branch is kept. `note_lapse`, `latch_lapse`, `word_restricted`, the samples
before `live()`, the broker reorder and the exit call are all absent, and `model_calls.rs` differs from `a299c0d9` only
by `cfg(test)` hooks.

Expected values that change from `a299c0d9`, all from `stop:owner` / `Stopped`: `budget-then-stop` (was tagged OPEN),
and the schedules new here, whose `a299c0d9` result was also `stop:owner`: unnoticed lapse, charge, entry, reply arm,
poll crossing, final admission, and revoked then lapse. Revocation is not recorded in the word, so R18 (a) does not
withhold the budget. No other expected value in the suite changed.

RED at `55cd9796` against `a299c0d9`: nine cases above with `Ok(Stopped)` or `stop:owner` for the budget, and
`lapse-stop-never-waits` with `Some(Stop)` for `Some(TimeBudget)`. The rest passed there.

Plants on the final code (real path, restored by sha256, not committed):

| Plant | Mutation | Result |
|---|---|---|
| rule removed | no budget check at Stop | Behaviour kill on every budget case above |
| Q reading | rule uses `is_engaged` (scope) instead of Mother's budget | Behaviour kill at `scope-only-` and `admission-scope-only-then-stop` |
| word ignored | accessor checks only the scope | Behaviour kill at `failed-` and `closing-lapse-then-stop` |
| ordinary route | accessor drops the scope check | Behaviour kill at `ordinary-lapse-then-stop` |
| Stop made to wait | rule takes the coordinator's lock first | Behaviour kill at `lapse-stop-never-waits` (the engagement wait) |
| Stop-all skips the rule | plain engagement | Behaviour kill at `unnoticed-lapse-then-stop-all` |
| emergency read removed | `emergency_stopped` dropped | Not killed. The session route sets the word's Emergency cause first, and the session fixture has no handle to the registry stop; kept because R18 (a) names it |

## The R8 acceptance sinks (SB3D-R3)

Three more oracles, each case in a child process, real C1 caller to SessionPort to fake peer. Each
restriction is the session primitive the ingress map names: `emergency` (CR, LF and CRLF reduce to it),
`SessionIngress::fail`, `accept(Quit)`, Quit then emergency for `ClosingRestricted`, and the mission
deadline on the shared clock. The session state is asserted at the sink.

- `sb3_restriction_before_the_record_is_classified`: restriction lands while the admitted chat is held.
  `failed:model_cancelled` (`failed:model_timeout` for budget). `failed:` is produced only by
  `classify`'s recorded-error arm, so the sink ran.
- `sb3_restriction_at_the_post_record_recheck`: restriction at the `model_replied` hook, which fires only
  for a recorded `Replied`, between the record and the recheck. Gives `invalidated`; with no restriction,
  `ok` and the exact reply (the positive that rules out blanket denial).
- `sb3_restriction_after_the_recheck_or_the_append`: emergency after the lane received the reply (nothing
  accepted), and emergency after the `LaneReturned` line, at the next journal line (the answer stands
  once, no File published).

### Register update

Before: 226 cited, 2 executed, 240 N/A, OPEN D1 94, D2 627, F1 1. After: 226 cited, 33 executed, 240 N/A,
OPEN D1 0, D2 690, F1 1. Total 1,190.

Cells that became `x:` (31; `t` = `..._before_the_record_is_classified`, `r` = `..._at_the_post_record_recheck`,
`a` = `..._after_the_recheck_or_the_append`, `o` = `sb3_unrecorded_reply_is_unusable`):

| Sink | Lifecycle | Ingress | Fault | Schedule |
|---|---|---|---|---|
| `record_first_classify` | Normal `o`; Emergency, Failed, ClosingNormal, ClosingRestricted `t` | CR, LF, CRLF, budget, quit, fatal `t` | none, record_failed, record_unknown `o` | before `t`; after `r` |
| `post_record_recheck` | Normal, Emergency, Failed, ClosingNormal, ClosingRestricted `r` | CR, LF, CRLF, budget, quit, fatal `r` | none `r` | before `r`; after `a` |
| `lane_result_admission` | none | none | none | after `a` |

record_failed and record_unknown at `classify` are the Owed and Uncertain statuses injected at the adapter
(SB3D-R1 Q1): the status is `classify`'s own input there. Native debt stays B's evidence.

Moved to `open:D2` (63), each with why D1 did not execute it at that boundary:

| Cells | Reason |
|---|---|
| classify and recheck, lifecycle SealedFinalizing, ClosedClean, ClosedUnclean (6) | No attempt can be in flight after the seal; reaching the sink there is D2's H5 work. |
| classify and recheck, ingress `original_cell_unscheduled`, `repeated`, `EOF`, `partial_Ctrl_D`, `master_close`, `read_failure`, `stop_all` (14) | No session-level test performs that reduction (ingress map OPEN:D2, or no evidence); D1 adds none. |
| classify and recheck, ingress `mission_stop`, `door_fence` (4) | The reductions to `SessionDoor::cancel_run` and `halt` are OPEN:D2 under SB3C-R12; D1 does not prove them. |
| classify and recheck, ingress `grant_revocation`, `grant_expiry` (4) | The original File grant cannot be revoked or expired from Mother with existing seams. |
| classify, fault poison, panic (2) | `classify` holds no lock and is pure; no seam injects these at it. |
| recheck, fault record_failed, record_unknown (2) | `Caller::call` returns `RecordUnavailable` before the recheck, so it is not reached on these values. |
| recheck, fault poison, panic (2) | No Mother-reachable seam at the recheck. |
| admission, lifecycle Emergency to ClosedUnclean (7) and all 17 ingress values | The restriction is refused at `file_boundary` before the `LaneReturned` step. The sink's own append is not reached, so claiming it would be a neighbouring point. |
| admission, fault decline, record_failed, record_unknown, poison, panic (5) | Not reached at the append: `model_obligations` refuses debt first; no decline or poison seam. |

`mother_end` / `budget` stays OPEN:F1.

### RED record (before the data and check changes)

With the new controls added and the verifier's logic and data untouched: `sb3d_r3_d1_citation_controls_are_rejected`
failed with 20 controls RED (the 14 `x:` cases wanted their specific errors and got the old "only a C2-new oracle";
`open:D1` x3, `#[ignore]` on a D1 oracle, a D1 name that is not a test, and a D1 test as a retained `c:` were all
accepted) and `sb3d_r3_d1_cells_are_executed_or_owned` failed (executed 2, `open:D1` 94). After the data and logic
changes all seven verifier tests pass.

### Plants (each applied to the real code, restored by sha256, not committed)

| Plant | Mutation | Result |
|---|---|---|
| unrecorded reply projected | `classify`: Owed releases the reply | Killed at O1 `owed: unusable` |
| late content projected | `Caller::call`: `if self.live()` removed from the Replied arm | Killed at O2 `replied: never reaches the lane` |
| late result admitted | LaneReturned step: the pending-lane condition removed | Killed at O2 `the advance is refused`, one assertion before `no LaneReturned`; V2 `late: no LaneReturned` also fails |
| first cause lost | `file_latch_terminal`: intent not latched | Killed in O3 at `Stop arrives second`, before the literal; O1's end literal also fails |
| canary in evidence, as its digest | reply in the native `model_digest` detail (one edit; I6 seals it to `model_digest_sha256`) | Killed at V1 `REPLY leaked (digest)`, after SB3D-R4 |
| canary in evidence, raw | the same, and the I6 seal skipped for that key | Killed at V1 `REPLY leaked` |
| rejected reply reaches the lane | transport `done` check removed | Killed at V2 `rejected: no usable reply` |
| recorded failure not classified | `Caller::call`: `Failed` mapped to `Invalidated` | Killed at `classified as recorded failure` |

Two first attempts survived. The one-edit canary plant survived the first V1, and that was a real gap
(review f7/review-sb3d1-1, SB3D-R4): I6 replaced the reply with its digest, and that digest of the reply stayed
in the native evidence, which spec 6.6 also prohibits. V1 searched only plaintext and hex, so it did not see it.
V1 now searches each canary's I6 digest (the public `Digest::of` over I6's domain, the domain checked against
the source) and kills that plant. The rejected plant passed because the case's body also lacked the eval
counts and a later check refused it; the body was corrected.

### V1 limits

- V1 does not inspect `HostSession::inventory()` or an export renderer. `close_obligations()` and
  `startup_report()` are narrower observations.
- The digest search matches a digest of exactly one canary. A digest over a larger text that contains a canary
  (a whole error body, a concatenation) is not detected.

### Verifier limits

The verifier shows that a D1 `x:` is a `#[test]` in that file at a cell its table lists, and nothing more:
it does not run the test or show it reaches the sink (register 14.10, 14.13). `is_test` reads source text and
the D1 module's `#[cfg]` lines are outside it. The literal tables and these docs share one audit origin.
