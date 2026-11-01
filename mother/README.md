# Mother Nexus M1 + M2a: mission contract, coordinator core, broker consent

Status: M1 and M2a (mission consent in the broker; Mother waits for broker
approval) are on `main`. M2a-S (Stop and denial durability across
restarts) is a candidate on `mother/m2a-s-stop-durability`.
Headless Rust library only: no UI, no real research, no website building,
no model calls. It is not a passed gate. Every capability here is PLANNED
until its physical test passes on the installed build (`MOTHER.md` §1).

## What M1 is

Mother turns a human goal into a governed mission:

1. **MissionContract** (`contract.rs`): the Owner's words verbatim, Mother's
   restatement, the requested folder (a request to the broker, never path
   authority), a budget (time, lane calls, money = 0), forbidden effects,
   the missing-data questions with their answers, and pass criteria fixed
   before any work. A SHA-256 hash binds every field.
2. **Consent** (`consent.rs`): bound to the exact contract hash. A change to
   the contract voids it. A `Consent` cannot be built, cloned or
   deserialized outside the crate. M2a: the Owner is asked through the
   broker's native mission dialog (`ControlConfirmer::confirm_mission`,
   with a `MissionConfirmation`: the contract hash, a request label and
   Mother's plain-language summary). The broker records the answer in its
   evidence store and, only on a recorded yes, returns a `MissionApproval`
   for that hash and request. The coordinator makes a live consent only
   from that approval (`Consent::from_broker`), for the mission's current
   request and unchanged contract; a restart replays the verified journal.
3. **State machine** (`state.rs`): Draft → AwaitingConsent → Approved →
   Investigating → Building → Verifying → Explaining → Done, plus Stopped,
   Denied, Failed, Blocked and (M2a) AwaitingApproval. One deterministic
   table, used live and on replay. Stop works from every state.
4. **EffectPort** (`effect.rs`, `adapter/`): the only way Mother reaches the
   world, sealed to `GovernedPort`, the adapter over
   `crates/nexus-governed-control`. Each mission acts as the agent `mother`
   in an agent run of its own, under the Owner's standing grants.
5. **Lanes** (`lane.rs`): Researcher A/B, Skeptic, Builder, Verifier,
   Sentinel. Every call is journaled with the lane's identity, an
   invocation id and the hashes of its input and output. M1 has only
   deterministic lanes.
6. **Journal** (`journal.rs`): append-only and hash-chained. After an
   orderly shutdown (`Coordinator::close`) a restarted coordinator resumes
   every mission in its state; after a crash (M2a-S) every unfinished
   mission comes back held until the Owner resumes or stops it. A Stopped
   mission stays stopped; a tampered journal is refused.
7. **Walk example**: `cargo run -p nexus-mother --example walk`.

## Invariants and their oracles

The oracles were committed before the implementation (`MOTHER.md` §3.1).
Unit oracles are in `src/oracles/`; the real-broker, file-journal and
source-scan oracles are in `src/adapter/` (only the adapter module touches
the host, tests included).

| Invariant | Oracles |
|---|---|
| MN-01: zero file, network, process, model-tool or spend effects before consent | `oracles/mn01.rs`, `adapter/real_broker.rs` |
| Consent binds the exact hash; a changed contract voids it | `oracles/consent.rs` |
| Stop from every state: no new dispatch, durable across restart, never waits | `oracles/stop.rs`, `adapter/real_broker.rs` |
| Denial (the Owner's no, the broker's refusal) means zero effect | `oracles/deny.rs`, `adapter/real_broker.rs` |
| A planted "consent"/"approved" string grants nothing | `oracles/consent.rs` |
| Journal tampering is detected and refused | `oracles/journal.rs`, `adapter/file_tests.rs` |
| The state machine is the mission's table | `oracles/state.rs` |
| Lanes: identity, invocation id, input/output hashes, budgets | `oracles/lanes.rs` |
| Contract bounds; the hash binds every field; verbatim words | `oracles/contract.rs` |
| No host access outside the adapter; no kernel actuator or legacy stack | `adapter/scan.rs` |
| No production always-yes consent confirmer, in `mother/` or the broker (Owner condition) | `adapter/scan.rs` |
| The port is sealed; the adapter calls only the agent side of the broker | `oracles/structure.rs` |
| M2a: consent only from the broker's recorded yes for the exact hash and request; another hash, a replayed answer, an unrecorded yes or a broker refusal give none | `oracles/m2a_consent.rs`, broker `tests/m2a_mission_consent.rs` |
| M2a: waiting for approval: approve continues once, deny = zero effect and Denied, expiry = Blocked, Stop withdraws durably, restart never re-sends or approves | `oracles/m2a_waiting.rs`, `adapter/real_broker.rs` |
| M2a: Stop withdraws a pending request after the port is rebuilt; expiry by either clock | `oracles/m2a_rebuild.rs`, `adapter/real_broker.rs`, `adapter/expiry_tests.rs` |
| M2a-S: a Stop or denial that returns Ok holds across every restart; one whose append and poisoning both fail returns Err, nothing further runs, and after any restart every unfinished mission is held (no lane, no port call, no broker query) until the Owner resumes or stops it; a clean close resumes normally | `oracles/m2as_durability.rs`, `oracles/m2as_review.rs`, `oracles/m2as_resume.rs` |
| M2a-S: a call reserved before the break never reaches the world after it (fenced at the broker); a failed open still withdraws every journaled request; the class test over random sequences | `adapter/m2as_fence.rs`, `adapter/m2as_adopt.rs`, `adapter/m2as_model.rs` |

Coordinator rules the oracles pin:

- Nothing reaches the port, and no lane runs, before consent.
- An effect the broker leaves for the Owner's native approval puts the
  mission in AwaitingApproval, with the rest of its phase (the asking
  lane's remaining effect requests and the lanes still to run) journaled.
  While it waits no lane runs and nothing is sent. Each `advance` asks the
  port what became of the action (no lock held): approved and run at the
  broker, the phase goes on from where it paused, once; denied, the
  mission is Denied; expired, revoked, failed or unknown, it is Blocked;
  Mother's time budget running out ends the wait before the broker is
  asked. Every end of a wait without the action cancels the mission's run,
  which withdraws it. Stop while waiting does the same, durably.
- Mother refuses an effect before the port when it is forbidden, when it is
  spend (the money budget is zero), or when it is a file request without a
  requested folder or not relative to it. Any refusal blocks the mission.
- The Owner's emergency stop is checked before every lane and every effect.
- No coordinator lock is held while the Owner is asked, a lane runs or an
  effect runs. A result that comes after a Stop is dropped and journaled
  as overtaken.
- Every lane call counts against the call budget. The time budget runs from
  consent, measured on both the wall and the monotonic clock.
- An effect dispatched but not settled when Mother restarts blocks its
  mission (`effect_outcome_unknown`): it may have happened. A mission that
  dispatched any effect in its current phase before a restart is blocked
  too (`phase_interrupted`): re-running the phase could repeat it.
- If a journal append fails, the mission ends (or stops) in memory, the
  port is cancelled, and the journal is poisoned durably (a
  `.poisoned` marker beside the file): it opens nothing until the Owner
  reconciles it, so a Stop the journal could not record is never undone
  by a restart.
- M2a-S, when poisoning fails too: every session journals its opening
  before anything else it writes (at once on a journal whose last session
  closed cleanly), and only `close` of a coordinator that is not broken,
  with no call in flight, journals a clean close (it is refused too if the
  journal changed under it). A journal whose last session has no clean
  close (a crash, a failed append, a journal from before sessions) brings
  every unfinished mission back held (`unclean_shutdown`): advance,
  consent, submit and revise are refused; only the Owner's `resume`
  (journaled) or Stop moves it. The holds are journaled with the session's
  first record or its clean close, so opening stays read-only until then,
  and they hold across every restart until the Owner decides. A held
  waiting mission's request is handed back like any waiting one: Mother
  does not ask about it or approve it while held; the Owner's Stop
  withdraws it. Once the journal has failed, the coordinator acknowledges
  nothing: Stop is never Ok (a terminal state may be in memory only), and
  it neither asks the Owner for consent nor asks the broker about an
  approval. Replay refuses a session the live coordinator could not write
  (a clean close, or work, before the holds an unclean session requires).
- M2a-S, the fence: at the moment the journal fails (under the coordinator
  lock that marks it broken) the port is fenced: every broker run it holds
  is cancelled, so a call reserved before the break (an effect journaled
  dispatched, a waiting request) is refused by the broker itself, and the
  port opens no new run. After every outbound call (a perform, a lane, the
  Owner's consent dialog, an approval query) the result is discarded, and
  nothing is journaled for it, if the coordinator broke or the mission is
  held. Every journaled open request is handed back to the port before any
  session record is written, so an open that fails still withdraws it; the
  port's withdraw is an error for a handed-back request it does not hold
  while the broker holds it open. A hold after a resume is journaled again.
- M2a-S class test (`adapter/m2as_model.rs`): 500 seeded sequences on the
  real broker (more with `MOTHER_M2AS_SEQUENCES`), checking after every step
  that an acknowledged Stop or denial holds, that nothing reaches the world
  after a break, the holds, and that a failed open leaves no request open.
- Replay accepts records only in the order the live coordinator writes
  them (lane answers after their calls, no transition but Stop while an
  effect is unsettled, mission ids in sequence).

## Non-claims

- **M2a-S: `resume` is an API call, like `stop`.** It is the Owner's
  decision, journaled, but not confirmed through a broker dialog: the
  Mother screen (M2) must bind it to the Owner. Mother never calls it.
  It is the risky direction (it lets a mission whose Stop may have been
  lost carry on); Stop is the safe one.
- M2a-S: while a waiting mission is held, its request stays at the broker,
  where only the Owner's own native approval can run it. Mother does not
  ask about it or carry on from it until the Owner resumes the mission.
- M2a-S: the fence cancels broker runs under the coordinator lock, so the
  broker's cancellation record is written while that lock is held (Owner
  decision). An effect already executing inside the broker when the
  coordinator breaks sees its run's cancellation token; one past the point
  where it checks it may complete. The oracles pin every call reserved but
  not yet handed to the broker.
- M2a-S: the clean close is a record in the unkeyed chain. Someone with
  write access who cuts the journal back to a clean close (dropping a
  later Stop) is not detected; that is the existing truncation non-claim.
- **Consent is the broker's, the dialog is not built yet.** No desktop
  dialog implements `confirm_mission` in this mission (no UI): the walk
  example stands in for it at the terminal, and only test code fakes one.
  A confirmer that does not implement it declines every mission.
- The broker shows Mother's summary beside the hash; it does not check the
  summary against the hash (Mother builds both). It binds the Owner's
  answer to the hash and the request label.
- Consent is an extra gate, not authority: every effect still needs the
  Owner's broker grant, and an R2 effect needs the Owner's native approval.
- **Mother never approves or runs a waiting action.** The Owner approves
  and runs it from the interface (`authorize`, then `execute`); Mother
  learns only its state from the broker, not its output. An action the
  Owner authorized but has not run yet still counts as waiting.
- The identity of the action a mission waits on (its broker run and
  commitment, and the port's deadline for it) is journaled before the
  wait, and stays open in the journal until a closure record, which is
  written only after the port confirms the broker no longer holds the
  action open (Mother's and final, or not held at all). Every reopen
  hands each open action back to its port, whatever its mission's state;
  unless the mission still waits, it is withdrawn (the mission's run is
  cancelled) and closed once confirmed. So a Stop, denial, lapse or block
  interrupted before its withdrawal took effect is completed by the next
  reopen, and a rebuilt port can still ask about and withdraw a waiting
  action.
- Recovery is fail-closed, in one place (`Coordinator::recover`): any
  error in it (a journal append, a closure, a withdrawal the broker
  refused), and any append that failed in it even if a step did not
  report it, makes opening fail, after every journaled open request,
  waiting ones included, has been withdrawn. A poisoned journal then opens
  nothing until the Owner reconciles it.
- A request the Owner approved that is still executing when a terminal
  (or interrupted) mission is recovered is cancelled (its cancellation
  token is set) but not yet final, so its open marker is not closed: it is
  closed on a later reopen once the broker holds it final (for an
  interrupted mission, possibly already by its block in the same reopen). No oracle
  pins this on the real broker (it needs an execution in flight); a
  recording-port oracle scripts it.
- The ids are names, not authority: the port takes one back only from a
  live run of Mother's that no other mission holds (an action two
  missions name, compared by parsed id, is handed back to neither), and
  trusts its deadline no further than the action's remaining lifetime by
  the broker's wall-clock expiry. The broker's own monotonic deadline is
  not in its public view, so with a wall clock set back since the action
  was created that bound is looser. Still possible with write access to
  Mother's journal (it is unkeyed): swapping the actions of two live
  Mother missions, or naming one action in two
  missions, after which neither is handed back and a stopped mission's
  genuine request stays approvable until it expires (the Owner's emergency
  stop withdraws it).
- **Known limit: the adopted deadline (Owner decision, exact clamp in
  M2b).** A port that takes back a journaled action clamps its deadline to
  the action's remaining lifetime by the broker's wall-clock expiry, not to
  the broker's original monotonic deadline, which the broker's public view
  does not expose. With a wall clock set back since the action was created,
  the clamp is looser than the broker's deadline (the port may report
  Waiting a little longer; the broker still refuses an expired action). No
  broker change in M2a.
- **Known limit: the pre-journal window.** If Mother dies after the broker
  left an action waiting but before its identity was journaled, the
  mission is blocked on restart (`effect_outcome_unknown` before the
  settlement was journaled, `phase_interrupted` after it) and Mother
  cannot withdraw that action: it stays approvable until it expires or
  the Owner denies it. The Owner's emergency stop at the broker withdraws
  it (an oracle pins this: a request Mother never journaled is no longer
  approvable or executable after the emergency stop). A broker restart
  loses the action itself (it can no longer be approved), and the mission
  is Blocked (`approval_outcome_unknown`).
- The broker ends an expired action only when it next touches it. The
  adapter reports a waiting action as expired once its wall-clock expiry
  or the port's monotonic deadline has passed (the broker clock read before
  asking plus the action's lifetime, never later than the broker's own
  deadline, which a wall clock set back does not delay). The broker
  refuses to approve or start it then.
- A restart after an approved action but before its phase ended blocks the
  mission (`phase_interrupted`): re-running the phase could ask again.
- The rest of a paused phase is journaled as data: the lane's effect
  requests as it wrote them (URLs, methods, headers, bodies, tool and
  connector input), before the broker has checked them. The broker refuses
  credential headers when a request reaches it, but a lane that put a
  secret in a request would have it written to Mother's journal.
- Anchoring Mother's journal (and its consent entries) in the broker's
  evidence store is deferred to Gate E.
- The journal chain is unkeyed. It detects edits, removals, reordering,
  insertion and a torn last line. It does not detect someone with write
  access to Mother's home who rewrites the whole chain, or cuts entries off
  its end. Anchoring the head in the broker's evidence store is later work.
- The journal append is made under the coordinator lock: it is the
  transition's commit (a bounded local write), so the journal's order is the
  state machine's.
- The broker has no file or spend route: the adapter refuses both
  (`no_broker_route`). M1 writes no files for a mission.
- The journal file takes no exclusive lock: two coordinators on one file
  would interleave entries and the next open refuses the journal. A torn
  last line after a crash, or a poisoned journal, refuses the journal with
  no automatic recovery: both fail closed and need the Owner.
- After a restart, the time budget is measured from the wall clock at
  consent (a clock set earlier than the consent counts the budget as
  spent; a clock set back but still later than it is trusted).
- The source scans (`adapter/scan.rs`) are guards, not proofs: they match
  text (for instance, the confirmer check looks for `ControlConfirmer for`
  blocks, `stdin` and a literal `true` body).
- `GovernedPort::cancel` cannot report a failed run cancellation to its
  caller. The adapter counts each one (`cancel_failures`).
- Lane calls are counted as model calls; no model is called in M1.
- Mother depends on `nexus-governed-control`, which itself depends on the
  kernel. Mother uses no kernel actuator and none of the legacy stack.
- Path validation, workspace grants and the broker's process-group
  ownership are not OS sandboxes. Mother adds none and claims none.
- Linux only. Windows and macOS are not claimed.

## Run the walk

```text
cargo run -p nexus-mother --example walk
```

Each time it asks (a stand-in for the broker's mission dialog, showing what
the broker would show), type the first 8 characters of the contract hash to
consent. In run 2, answer "no" when the Deny step asks. The walk opens a
fresh home under the system's temporary directory and prints its path.
