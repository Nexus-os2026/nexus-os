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
  | budget-entry-then-stop (lapse before the lane's first call) | `block:budget_time` (SB3C-F1) |
  | stop-then-budget (Stop engaged, lapse, then the caller asks) | `stop:owner` |
  | failed-, closing-lapse-then-stop (restriction with no cause, lapse, Stop) | `stop:owner`, as at `a299c0d9` |
  | revoked-lapse-then-stop (invocation revoked, lapse, Stop) | `stop:owner`, as at `a299c0d9` |
  | reply-lapse-then-stop, reply-lapse (lapse observed by the Replied guard) | `block:budget_time` |
  | poll-crossing-then-stop (the deadline crosses inside one broker poll, Stop lands while the attempt returns) | `block:budget_time` |
  | scope-only-then-stop, scope-entry-order-, scope-reply-order-then-stop (the scope refuses, budget not lapsed at the sample) | `stop:owner` |
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

## SB3C-F1 (SB3C-R5): the budget cause on the session route

The defect: a held chat on the session route met the budget deadline, the lane got `failed:model_timeout`,
and an Owner Stop that arrived while the lane was still running ended the mission `Stopped` / `stop:owner`.
Spec O3 requires the earlier cause; the ordinary route had it.

Observed cause (probe, restored by sha256). The earlier hypothesis, that the broker's B1f expiry latches a tag
on the shared File cell, was wrong:
- `HaltCell::is_engaged` on a session-scoped cell returns `!scope.live(word)`. B1f's mission deadlines make the
  scope report lapsed, so the cell read engaged while its word held no cause (`reason()` None, word `0x40`).
- `Caller::live()` was therefore false at the budget observation sites, the `BudgetTime` deny did not run, and
  nothing was latched until the Stop's `OwnerStop` (word `0x40` to `0xffff41`).

The fix, as repaired under SB3C-R14 to R16. `Caller::latch_lapse` writes the budget cause only if Mother's own
deadline sample is expired and no latched cause, Closing, Failed, Sealed or Emergency is in the cell word
(`HaltCell::word_restricted`, read-only), no invocation revocation, no emergency stop. The first-wins cell decides
a race with Stop. Mother takes that sample:

- at `CallDeadline::expired` and after the attempt returns: `note_lapse`, whose one sample is what it returns;
- at call entry and at the Replied guard: once, immediately before `live()`. No clock is read after the refusal.

`CallGate::expired` evaluates the scope first and then always the caller, so a timeout the broker returns from a
scope expiry is followed by Mother's own observation. The two are the same mission budget on the same clock.

SB3C-R17: the broker judges expiry at several sites (the polled deadline, request admission, the cancel poll that
defers to it), and admission's timeout never asks the caller. Every site ends the attempt at one exit: the transport
returns, no hold crossing it, before native recording. On `Err(Timeout)` only, the broker asks Mother's deadline once
there; that single exit is what covers every broker expiry site. A Q or backstop timeout has Mother's sample false.

The request deadline is not the mission budget. A session scope refuses for its own request deadline Q (300 s for
Run and Approve, from when the command was accepted) as well as for the mission budget, and Q writes nothing to the
word. A Q-only refusal therefore looks like a lapse from the cell alone, but Mother's sample is not expired, so it
does not latch budget and a later Stop stays `stop:owner`. SB3C-R16 replaced latching by elimination for this
reason: a probe with Q shortened showed a live Caller refused with the budget not lapsed.

How much is tested: `scope-only-then-stop` narrows the scope's mission limits below Mother's own, which reaches the
same scope refusal and the same state with no clock advance. Q itself is not driven: it is 300 s of real time, and
the broker's Q seam is compiled only for its own crate.

Limits of this unit. F1 fixes ordered schedules: a lapse that is observed before a Stop keeps the budget cause. If
a lapse and a Stop fall within adjacent clock reads, or the wall clock moves backward, the cause can still be
attributed either way. In every such case the mission still ends and the halt is unaffected. These are recorded
residuals, not repaired further.
A Stop between a broker timeout and that exit is the same residual. A refusal before the attempt exists (the scope
refused at attempt admission) never reaches the exit: it has no recording, and Mother's post-call sample follows it.

Schedules (SB3C-R14 (d)). A held lane waits for its explicit release, and an expired hold exits the case's child
process, so the coordinator's own budget boundary cannot satisfy these cases. In `stop-then-budget` the lane asks only
after Stop is engaged and the clock moved.

RED, each on the oracle commit against the production before its fix:

- `7bbe4ef5`: `budget-then-stop`, `budget-entry-then-stop` at `Stop arrives second`.
- `9bd53ad2` (against `bb4c5ec9`): `failed-` and `closing-lapse-then-stop` `Ok(Blocked)` for `Ok(Stopped)`;
  `reply-lapse-then-stop` `Ok(Stopped)` for `Ok(Blocked)`; `reply-lapse` `fail:lane_fail:builder`; the one-sample
  guard. With `a299c0d9`'s `model_calls.rs`, `failed-`, `closing-`, `revoked-lapse-then-stop` and `stop-then-budget`
  passed, which is where their `stop:owner` comes from.
- `03d9f467` (against `ee547a7e`): `poll-crossing-then-stop` `Ok(Stopped)` for `Ok(Blocked)`; `scope-entry-order-` and
  `scope-reply-order-then-stop` latched (`Some(Fence)`, want `None`). `scope-only-then-stop` passed there; its plant
  below is what shows it discriminates.
- R17 oracle commit (against `33ff3f65`): `admission-lapse-then-stop` `Ok(Stopped)` for `Ok(Blocked)`.
  `admission-scope-only-then-stop` passed there. Stop lands after recording returns: no seam inside it reaches Mother.

Plants on the final code (applied to the real path, restored by sha256, not committed). All are behaviour kills
unless stated:

| Plant | Mutation | Result |
|---|---|---|
| Stop-wins mapping restored | `file_finish_intent`: a budget `Failure` intent ends `Stop(Owner)` | Killed at `Stop arrives second` on the session cases and ordinary `c1_s6`; also the no-Stop literals |
| session latch removed (ordinary/session divergence) | guard is `live()` | Killed on the session cases and `reply-lapse`'s literal; ordinary `c1_s6` passes |
| latch after Stop could win | latch moved to `Caller::drop` | Killed at `Stop arrives second` on budget-then-stop, entry, reply. Masked elsewhere: ordinary `c1_s6` dies at `engaged && !terminal`, poll-crossing at a `within` wait, and the source guard also fires |
| entry latch removed | no latch at entry | Killed at budget-entry-then-stop only |
| reply latch removed | no latch at the Replied refusal | Killed at reply-lapse-then-stop and `reply-lapse`'s literal |
| `is_revoked` guard removed | condition deleted | Killed at `revoked-lapse-then-stop: Stop` |
| accessor bypassed | `reason().is_none()` for `word_restricted` | Killed at `failed-` and `closing-lapse-then-stop: Stop` |
| second clock sample | `expired` re-reads the deadline | Killed at poll-crossing `Stop arrives second`, and by the source guard |
| broker reorder undone | `CallGate::expired` caller first, short-circuit | Not killed on the final code: masked by the exit observation (killed at poll-crossing at `33ff3f65`) |
| sample after `live()`, entry | sample moved into the refusal | Killed at `scope-entry-order: nothing latched` |
| sample after `live()`, Replied | sample moved into the refusal | Killed at `scope-reply-order: nothing latched` |
| expired condition removed at the refusals | `latch_lapse(true)` at entry and Replied | Killed at `scope-only: nothing latched` and both order cases |
| exit observation removed | no call at the attempt's exit | Killed at `admission-lapse-then-stop`, and by the source guard |
| exit observation unconditional | called for every result | Source guard only: it would let a budget sample pre-empt an Owed record's cause, and no deterministic seam reaches that |
| expired condition removed in `note_lapse` | `latch_lapse(true)` | Killed at `admission-scope-only: Stop arrives second`; also kills many other cases |

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

`mother_end` / `budget` stays OPEN:F1: that sink is `CloseSeal::begin_mother_end` / `observe_mother_end`,
and `first/budget-then-stop` does not execute it (`m.close()` there is `FileSession::close`). SB3C-F1 does not
flip it.

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
