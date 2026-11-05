# Mother Nexus M1 + M2a: mission contract, coordinator core, broker consent

Mother includes the coordinator, the separate desktop host, and this branch's
Linux terminal Hands candidate. Hands uses deterministic lanes and calls no
model. M2d-D is implementation and automated-test evidence, not an accepted
capability. Every capability remains PLANNED until the Owner tests the
installed build (`MOTHER.md` §1). The historical sections below describe the
layered M1–M2c interfaces; the File-only limits in **Run Hands** take precedence
for this terminal process.

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
   Sentinel. Every call is journaled with the lane's role, the digest of
   its identity, an invocation id and the hashes of its input and output.
   M1 has only deterministic lanes.
6. **Journal** (`journal.rs`): append-only and hash-chained. After an
   orderly shutdown (`Coordinator::close`) a restarted coordinator resumes
   every mission in its state; after a crash (M2a-S) every unfinished
   mission comes back held until the Owner resumes or stops it. A Stopped
   mission stays stopped; a tampered journal is refused. P0: format 2,
   evidence only (the table below); a v1 journal is refused and left as
   it is.
7. **Private store** (`private.rs`, `adapter/private_file.rs`, P0): the
   contract and a paused phase's rest, content addressed and hash checked,
   in `<home>/private/<kind>/<hex>` (directories 0700, files 0600). Every
   directory is durably linked (it and its parent synced) and every blob
   synced and renamed into place before the journal names it by
   reference; a failed sync fails the call. The rule is stateless: at
   open the store's directory is made canonical and its chain fixed (it,
   every directory above it that is private to Mother, and one more), and
   every put syncs the whole chain, so a directory's link is durable
   whoever created it and however its path was spelled. Every put checks
   the store's and the kind's directories (a directory, not a link, no
   access for others, the store's owner). Nothing in it is authority.
8. **Closed codes** (`evidence.rs`, P0): every reason, outcome and class
   Mother records or returns is a code of a closed registry; the detail
   behind one is the Owner's `WorkingDetail`, in memory only.
9. **Walk example**: `cargo run -p nexus-mother-host --example walk` (M2c: the walk lives in the host crate, which alone builds the broker).

## Invariants and their oracles

The oracles were committed before the implementation (`MOTHER.md` §3.1).
Unit oracles are in `src/oracles/`; the real-broker, file-journal and
source-scan oracles are in `src/adapter/` (only the adapter module touches
the host, tests included).

| Invariant | Oracles |
|---|---|
| MN-01: zero file, network, process, model-tool or spend effects before consent | `oracles/mn01.rs`, `adapter/real_broker.rs` |
| Consent binds the exact hash; a changed contract voids it | `oracles/consent.rs` |
| Stop from every state: no new dispatch, durable across restart. Stop never waits for a lane or for an effect in flight. It still waits for its own journal operations, for in-memory locks other calls hold, and for its broker records, which it writes after the cancellation has taken effect and while holding no lock | `oracles/stop.rs`, `adapter/real_broker.rs`, `adapter/m2c_halt.rs` (M4) |
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
| M2c: Mother acts only through a broker-issued door, bound to her agent and to the runs it opened or adopted; nothing reachable from the port leads to the front door, the authority, the issuer or an owner API | `lib.rs` (A1 compile_fail), broker `lib.rs` (A1), broker `governed/door_tests.rs` (A2, A3), `adapter/scan.rs` (A8) |
| M2c: every door asks: the Owner's native yes is needed for every issuance of a door, in a new process or in the same one; no stored state decides it; a no or an unrecorded yes gives no door; live doors and memberships are bounded and reclaimed. (A coordinator reopened on an existing port reuses that port's door; the walk asks for a fresh door at every run and restart.) | broker `governed/door_tests.rs` (A7) |
| M2c: consent asks through the door's bound confirmer, once, with the exact request | `adapter/m2c_door.rs` (A4), broker `governed/door_tests.rs` (A4) |
| M2c: resume asks: a held mission resumes only after the Owner's native dialog naming its contract hash, once per request (the table below) | `adapter/m2c_door.rs` (A5, A6), broker `governed/door_tests.rs` (A5, A6-f) |
| M2a-S: after a break, no effect dispatched after the break executes; a failed open still withdraws every journaled request; the class test over random sequences | `adapter/m2as_fence.rs`, `adapter/m2as_adopt.rs`, `adapter/m2as_model.rs`, `adapter/m2as_gate.rs`, `adapter/m2as_nowait.rs`, `adapter/m2as_sweep.rs` |
| P0: records keep only evidence (the policy below): contract fields, a paused rest, lane reasons, port classes, question ids and broker ids with a canary appear in no journal byte, broker record or returned error (scanned raw, in every encoded form and in fragments); the private store holds them | `adapter/p0_privacy.rs` (P0-O1–O4, O9, O19, O22), `adapter/p0_api.rs` (O9, O19, O23, I2, I3), `lib.rs` (closed error forms, compile_fail) |
| P0: private content that is missing or does not verify (content id, size, contract hash) withdraws the mission's journaled requests first, then stops it; content grants nothing; a v1 journal is refused byte for byte | `adapter/p0_privacy.rs` (P0-O5–O8), `adapter/p0_api.rs` (O8) |
| P0: the private store's directories are durable before a reference to its content is journaled (every new directory and its parent synced; a failed sync fails `put`, and nothing is journaled; every put syncs the link of every directory Mother may have created, whoever created it and however its path was spelled); every put checks the store's and the kind's directories | `adapter/p0_dirsync.rs`, `adapter/p0_privdir.rs`, `adapter/p0_chain.rs` |
| M2c: a break halts the port's door at the broker (invariant (b) below): an effect already past the port's fenced check meets the halt at every admission step; the halt is in memory, under one broker lock at a time, each taken even if poisoned; its records are written by the call that made them, after it released the coordinator and port locks, or counted (`halt_records`) | broker `governed/halt_tests.rs` (B1–B18, R1–R6; R6 is CR8's property, an Executing member that a fully written halt leaves unsettled), `adapter/m2c_halt.rs` (M1–M8, A6-g), `adapter/m2c_crash.rs` (MC1–MC4, CR1–CR7) |
| M3-A: the page Verifier (`verifier.rs`) is pure and deterministic: V0 to V2 gate in order, V3 to V5 then always run in full, and a pass needs all six; each of V3 (S1a's facts report) and V4 (the second reader) refuses what the other was made to pass; the reader's texts equal hand-written fixtures, html5ever's and S1a's own, and syntax it does not know fails closed; reports hold codes and booleans only | `oracles/m3a_verifier.rs` (M3-O1–O9), `oracles/m3a_s_o4.rs` (M3-O6, 2,000 pages), web-builder `tests/m3a_reader_html5.rs` (M3-O6, html5ever), kernel `inert_page/tests/m3_o16_facts_report.rs` (M3-O16) |

M2c: how a held mission resumes. The coordinator reserves a request
under its lock (replacing any earlier one; the mission's generation
moves), the broker asks the Owner through the door with no Mother lock
held and records the answer, and the coordinator takes it under its lock
only if that request is still current; it is consumed once. A resume
takes effect only when `Resumed` is journaled; the broker's record of the
Owner's yes resumes nothing.

| What lands, and when | Outcome |
|---|---|
| A second request is reserved while the first is in its dialog | the first one's answer is stale (its request was replaced, the generation moved); only the second can count, once |
| A Stop of the mission before the take (in the dialog, during the broker's record, or before the journal write) | the Stop clears the request and the hold: the answer is discarded and the call returns `Stopped` |
| A break (journal failure) before the take | the coordinator is broken: the take refuses |
| A fence of the port or a halt of its door outside a break (the public port fence, or poison recovery, which halts the door outside the coordinator lock), after the broker's post-record check and before the take | the take may journal `Resumed`: the Owner's recorded yes exists, and the fenced port and its halted door refuse every effect of the mission (A6-g, on the port fence and on a door halt) |
| An emergency stop after the broker's post-record check | the take may journal `Resumed`; every effect of the mission is then refused by the broker. Resuming a held mission permits no effect by itself |
| Anything after the take | acts on a resumed mission, as on any other |

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
  lock that marks it broken) the port is fenced. M2c: under the port's
  mutex it sets `fenced`, halts the port's door at the broker, then sweeps
  every registered run through the door's cancellation (a backstop), and
  returns the records all of that made, unwritten. The coordinator keeps
  them in its own state, takes them out before it releases its lock, and
  writes them with no Mother lock held, after the poison marker was tried
  (`with_lock`). A waiting request on a cancelled run is refused by the
  broker itself, and the port keeps no new run.
- M2c, invariant (b): after the fence's halt returns, the door's runs admit
  no new commitment to Executing; no run is inserted into the door's
  group; an action already Executing has its run's token set and stops at
  its actuator's next check; and the halt did no evidence I/O. The halt's
  first step (under the broker's runs lock) alone refuses all admission;
  its second (the commitments lock, taken alone) waits for any admission
  in progress, which is what the return-time claim needs. The fence does
  not wait for in-flight actions (Owner decision). A panic inside the
  halt skips its remaining steps. Admission refusal survives it once the
  halt has passed its runs barrier (its first step). The fence catches the
  panic and goes on with its backstop sweep, which covers the port's
  registered runs only, so later cleanup (another member's commitments,
  leases or pending effects) may remain incomplete; the panic is then
  resumed. Each broker lock is taken even if poisoned; records a panic
  left unwritten are counted lost, never silently dropped. Every broker call the port makes (opening a run, an action, an
  approval query, adopting, a settled check, a cancellation, a finish)
  checks `fenced` first and is refused once it is set. A call that checked
  just before the break is not waited for, even one stalled where no
  cancellation reaches it (a DNS lookup): it is interrupted where the
  broker can (its run is cancelled, so the broker refuses a step it has not
  begun, and an action executing on it sees its run's cancellation token);
  M2c: it then meets the halt (below). Every run registration checks `fenced` in the same
  critical section as the fence: it comes before the fence's (which
  cancels the run) or after (and cancels the run itself, never giving it
  to the mission). The
  Owner's consent and resume dialogs and the emergency-stop read do not
  check the fence: they produce no effect, and an answer that comes after
  the break is discarded. Both dialogs go through the port's door (M2c:
  Mother holds no broker Authority handle). A fenced port's withdraw does not retry a cancellation
  the fence could not make; it reports an error. After every outbound call (a perform, a lane, the
  Owner's consent dialog, an approval query) the result is discarded, and
  nothing is journaled for it, if the coordinator broke or the mission is
  held. Every journaled open request is handed back to the port before any
  session record is written, so an open that fails still withdraws it; the
  port's withdraw asks the broker for every handed-back request's own run
  and state, and is an error while the broker holds one open on any other
  run than the one it just cancelled (one two missions claim is recorded, not
  given back, so an open that would leave it approvable fails closed). A
  fence is final: a coordinator opened again after a break needs a new
  port. A hold after a resume is journaled again.
- M2a-S class test (`adapter/m2as_model.rs`): 500 seeded sequences on the
  real broker (more with `MOTHER_M2AS_SEQUENCES`), checking after every step
  that an acknowledged Stop or denial holds, invariant (b) (checked as: no
  request of the generated sequences reaches the world after a break), the
  holds, and that a failed open leaves no request open.
- Replay accepts records only in the order the live coordinator writes
  them (lane answers after their calls, no transition but Stop while an
  effect is unsettled, mission ids in sequence).

## Non-claims

- M3-A: the page Verifier is not wired yet (M3-D adds its lane and its
  input). Its second reader is independent only as a text extraction
  within S1a's accepted language; nothing is rendered, the 375 px and
  1366 px views stay OPEN, and the page's other sentences are not checked.
- **M2c: `resume` asks the Owner through the broker's held-mission
  dialog** (the confirmer the host bound to the door), naming the
  mission's contract hash and the request; the broker records the answer.
  Mother never calls it herself. The desktop has no native
  `confirm_held_mission` or `confirm_agent_door` dialog yet: until a
  desktop mission adds them, both answer no (no door, no resume). This
  fails closed.
- M2a-S: while a waiting mission is held, its request stays at the broker,
  where only the Owner's own native approval can run it. Mother does not
  ask about it or carry on from it until the Owner resumes the mission.
- M2c: the fence halts and cancels under the coordinator lock, in memory
  only; the broker's records are written after that lock and the port's
  are released (no lock held). An effect already executing inside the
  broker when the coordinator breaks sees its run's cancellation token;
  one past the point where it checks it may complete. Without waiting, the broker can open a
  run for a call that checked `fenced` just before the break; the port
  cancels it as it registers (it is recorded at the broker, cancelled). The oracles pin every call reserved but
  not yet handed to the broker.
- M2a-S: the clean close is a record in the unkeyed chain. Someone with
  write access who cuts the journal back to a clean close (dropping a
  later Stop) is not detected; that is the existing truncation non-claim.
- **Consent is the broker's.** The desktop still denies every dialogue.
  Hands uses the original session-bound terminal confirmer with fresh codes.
  The isolated legacy walk retains its three original dialogue categories.
  A confirmer
  that does not implement a category declines it.
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
- **Closed by M2c, for admission: the in-flight window.** An effect
  already past the port's fenced check before a break meets the door's
  halt at the broker: at opening a run (a run opened just before the halt
  may come back permanently cancelled, and every call on it is refused),
  at `prepare`, approval, `authorize`, `complete`, the lease fill and the
  credential re-checks. An action already Executing stops only
  cooperatively, at its actuator's next check: a request already sent, the
  gap between egress's last check and its send, a blocking DNS lookup
  under way, a tool's filesystem effects before its spawn check, and
  anything a process did before it was killed (reaping allows up to 5 s)
  are not undone. The credential release is not one atomic boundary.
  Cancellation is not containment.
- **Closed by M2c, for locks: the fence can block on the store.** The
  fence and Stop take effect before any evidence write, and no
  coordinator, port or broker lock is held across one. The call that broke
  the journal and Mother's Stop still return only after writing their own
  records. A stalled store also blocks every other evidence writer,
  because of the store's one mutex, as before.
- **Closed by M2c: the sweep guarantees attempts.** The guarantee is now
  the door's halt: in-memory steps, each under one broker lock taken alone
  and even if poisoned. Admission is refused from its first step, and the
  return-time claim needs its second. The sweep is a backstop.
- **Known limit: lost history.** A halt's records that were never written
  (a panic, a crash before the write, a coordinator left poisoned) are
  lost history, not lost safety: the halt lives in memory. They are
  counted (`ControlStatus.halt_records`: pending, recorded, owed,
  uncertain, lost). After a restart a commitment is `unsettled` exactly
  when its `Started` is durable and its end is not, whatever became of the
  halt's records, and recovery is then required. A door halt is not
  persisted: after any restart broker memory is empty, and every door asks
  the Owner again.
- **Known limit: lease ends write no record.** The halt (and a door's
  cancellation) ends its runs' credential leases in memory; no evidence
  phase exists for a lease end, so none is written (M2c adds no new
  public phase for it). The lease's run cancellation and its revoked
  commitments are recorded.
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
- P0, what each store may keep (design §5.1):

  | Store | Allowed | Not allowed (where it goes instead) |
  |---|---|---|
  | Mother journal | ids, digests, sizes, counts, kinds, roles, states, times, the closed codes of `evidence.rs` | the contract (private store, `contract`); a paused phase's rest (private store, `paused_phase`); lane reasons (dropped; the Owner's working detail) |
  | Mother-reachable errors | closed codes, indices, counts | detail (`WorkingDetail`, memory only) |

  The coding ledger and the broker's evidence are closed by P0-B and P0-C.
  The private store is the only place Mother keeps working content. It is
  not encrypted, has no retention or purge, and v1 journals are refused,
  not migrated or scrubbed. Plain SHA-256 digests of low-entropy values
  (a lane's name) can be guessed. A blob that is missing or does not
  verify stops its mission (`private_content_unavailable`) after its
  requests are withdrawn; write access to Mother's home can therefore stop
  missions, never grant anything.
- `Advance` and `LaneReport` print no lane text, lane reason or
  explanation in `Debug`; their fields still carry them for the Owner's
  display.
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
cargo run -p nexus-mother-host --example walk
```

The walk opens a fresh home under the system's temporary directory and
prints its path. It stands in, at the terminal, for the broker's three
dialogs, showing what the broker would show. Each dialogue shows a fresh random
8-character code on its `Answer code:` line. Wait for `INPUT READY`, then type
that exact code and press Return to approve. Every dialogue needs its own code;
anything else is a no.

- **The door** ("Let this agent act?") shows the agent, the door id and any
  advisory lines. Use its displayed answer code. It is asked every time
  Mother starts: at the start of each run and at every restart.
- **A mission's consent** ("Start this mission?") shows the full contract
  hash, request and summary. Use its displayed answer code. In run 2,
  answer no when the Deny step asks.
- **A held mission's resume** ("Resume this held mission?") shows the mission,
  its contract hash, the request, why it is held and its summary. Use this
  dialogue's newly displayed answer code.

Run 3 consents to a mission, advances it once, and exits without shutting
down. On restart the mission is held; a yes to the resume dialog lets it
finish, a no leaves it held, and the walk says which.

M2c: Mother's own crate builds no broker. The host crate
(`mother/host`, `nexus-mother-host`, not published) holds `Home`: the broker,
the journal, the Owner's confirmer, `door()` (asks every time) and the
Owner's side, `owner()`, which is never handed to Mother.

## Run Hands (M2d-D candidate; Linux only)

Hands creates one approved top-level `.txt` or checked `.html` file in an
empty, canonical folder you select. It never replaces a file, runs a shell,
opens a browser or calls a model. The broker validates size, content, extension,
mount topology, ownership and reserved directories; Linux alone does not make
storage supported. Home and its contents are reserved, as are the broker and
identity stores. Symlink selection and a nonempty folder refuse. Choose private
locations beneath your own home, outside this repository.

Build in this worktree, using the mission's Cargo slot:

```bash
slot=/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/53f6c814-4d9f-4dfc-bcb5-ab55762fc5cc/scratchpad/cargo-slot.sh
"$slot" cargo build -p nexus-mother-host --bin mother-hands --locked --target-dir target
install -d -m 700 "$HOME/.local/share/mother-hands-demo" "$HOME/mother-hands-empty"
target/debug/mother-hands \
  --home "$HOME/.local/share/mother-hands-demo" \
  --workspace "$HOME/mother-hands-empty" \
  --name hello.txt --text $'Hello from Mother.\n'
```

Both input and output must be real terminals; do not pipe or redirect them.
All four named arguments are required. `--text` is the exact UTF-8 argument,
including its final newline; it is visible in the launching command line and
may be retained by your shell history. Journal, ledger and evidence keep codes,
counts and digests; working content belongs in the private store and broker
candidate staging. This is not encryption or a disk-forensics guarantee.

Type `run`, then answer each Files grant, Mother door and mission-consent
dialogue with its own eight-character lowercase hexadecimal code. The grant
lasts 300 seconds. When `FILE PENDING` appears, type `approve`; inspect all
facts and the complete fenced content before typing the new code. Every answer
is exact. `no` is an explicit decline; whitespace, an empty line, a wrong code
or an overlong line fails the protocol and ends the host without approval.
A code inside content is data. Codes are random, checked against the
fixed display for collisions, and regenerated per dialogue.

Wait for `INPUT READY` before typing. The reader invalidates complete queued
answers and partial lines after the entire display is written and flushed.
A stale partial line is discarded through its newline. Stop, Quit and Ctrl-D
remain active across that boundary. There is one dialogue at a time; display,
flush, input and boundary failures deny and end the host. Door, grant and undo
dialogues allow 120 seconds; mission dialogues allow
300. These limits start at the original command acceptance, including queue
and display time. File approval also respects both original backend deadlines.
A new display code never refreshes request authority. Input accumulation and
queues are bounded. Terminal
echo, canonical input and newline translation are disabled while Hands owns
the terminal; orderly exit restores the prior modes. Ctrl-C is not intercepted.

| Command | Meaning |
|---|---|
| `run` | Start the one File mission allowed in this process. |
| `approve` | Ask for approval of this mission's exact pending action, then execute only if authorized. |
| `status` | Show session restriction, actual kernel/broker Stop and worker receipts. |
| `evidence` | Show bounded recent evidence, store health and resource/recording obligations; not a complete chain export. |
| `detail` | Consume the Owner-only in-memory working detail during normal operation. |
| `stop` | Deny through the original session cell immediately, then record that exact event off the reader. |
| `lift-for-undo` | Unavailable in every Hands state; no dialogue. |
| `undo` | Before emergency, separately approve removal of the exact eligible same-session file, once. Full bytes, SHA-256 and inode identity are shown; Owner edits refuse. |
| `cleanup` | Before emergency, request cleanup of a fixed list of returned disposable entries. Report settled, owed, excluded, in-flight and unknown facts. |
| `quit` | Quiesce, settle and close within the original three-second deadline. It never invokes undo. |

There is one original broker-issued session before input starts, one File
mission per process, and no replacement coordinator, adoption or recycled
eligibility. Commands retain their acceptance identity, target and deadline
through queues, dialogue and all child operations. A missing, busy or changed
approval/undo target refuses; a later target cannot repair an old command.

After emergency only status, evidence and Quit remain available; repeated Stop
still ingresses until sealing. No fresh undo, cleanup, lift or mission work can
start. Normal undo is available only before the first emergency. An already
handed-off kernel call may finish its documented internal work, including
compensation and internal cleanup. Each subsequent broker call needs a new
admission against its original request. Removing emergency undo does not close
the native handoff window; no new kernel observer or kernel change is claimed.

The reader performs memory-only Stop ingress and never waits on broker,
coordinator, output or cleanup. Up to four owned workers perform synchronous
Stop recording; additional presses still perform ingress and retain pending
recording obligations. Free worker slots drain them individually.
`STOP REQUEST RECEIVED`, corroborated kernel/broker halt, and
`STOP RECORDING COMPLETED` are different receipts. Worker return alone does
not establish durable recording; inspect health for owed/uncertain records.

A write already past the final native authorization check may publish once.
The host reports it honestly. Emergency does not grant permission for undo.
The exact uncertainty notice is:

> A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.

Startup verifies the kernel ledger, Mother history and primary evidence before
workspace selection or work. It does not sweep staging, restore permission,
recreate acknowledgement capabilities or restore undo. Unresolved or invalid
sources keep the Home report-only. A repeated start does not certify cleanup
or permit retrying an uncertain write. Facts absent from durable evidence are
reported as unavailable.

Normal Quit withdraws the exact pending action and truthfully settles a
nonterminal mission before cleanup. Cleanup preserves the published Owner
file and intentionally retained recovery material. Emergency close cannot
start new cleanup. Clean close requires joined producers/workers, complete
resource/recording/acknowledgement accounting, an atomic seal, durable Mother
then primary end records, terminal restoration and acknowledged final delivery.
After sealing, retained handles refuse and ingress returns `SessionEnded`.

`SESSION CLOSED recorded` is the final receipt; zero exit additionally requires
its successful delivery and renderer finalization before the same deadline.
A blocked append remains Unknown. Failure can leave one or both end records
durable while the full close fails; their statuses are reported separately.
EOF/read error engages emergency before nonzero shutdown. No exited process
promises eventual cleanup or recording. The exact notice is:

> UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.

FileJournal synchronizes both journal and parent directory, including existing
entries. Synchronization failure is returned rather than acknowledged.

### Install and record the exact build (Owner procedure)

This procedure is documented for the Owner; the builder does not install or
claim a physical test. Build the reviewed commit from a clean worktree. An
explicit target directory prevents confusion with `CARGO_TARGET_DIR`.

```bash
"$slot" cargo build --release -p nexus-mother-host --bin mother-hands --locked \
  --target-dir target/mother-hands-release
git rev-parse HEAD
sha256sum target/mother-hands-release/release/mother-hands
install -d -m 700 "$HOME/.local/bin" "$HOME/.local/share/mother-hands-receipts"
install -m 755 target/mother-hands-release/release/mother-hands "$HOME/.local/bin/mother-hands"
sha256sum "$HOME/.local/bin/mother-hands"
```

Keep the commit SHA, both matching hashes, build command, date, OS/filesystem,
selected Home/workspace paths and observed exit status in a receipt under
`$HOME/.local/share/mother-hands-receipts`. Do not capture approval codes or
private content in shared logs. The installed binary is separate from the
desktop; launching another process cannot control this process's broker.

For D-O20 physical acceptance, use two separate fresh, private Home/workspace
pairs and the **installed** binary with `--hold-before-publish 20` (integer
0–30 only, configured at startup). In each session consent, approve, and while
execution is held, first test `stop`, then in the second session test Ctrl-D.
Confirm no file or named temporary file appears. For Stop, confirm actual
kernel/broker halt, worker completion and evidence health, then Quit. Ctrl-D
must exit unclean/nonzero after emergency ingress. Save the observed receipts
and independently inspect the workspace. Record these as **OBSERVED** Owner
results; automated PTY tests are separate evidence. No installed physical
acceptance, H1 acceptance, C-11 closure, cross-session undo, Windows/macOS
terminal support, sandboxing or phase advancement is claimed by this document.

For the session-refactor acceptance addendum, additionally record a normal
create/undo/clean-Quit control and a separate emergency after publication.
For the latter, verify that lift, undo, cleanup, run and approve each refuse
without dialogue and that the Owner file remains unchanged. With an unfinished
preparation, record the retained staging and nonzero Quit. Restart twice and
verify report-only findings and unchanged resource identities/content. Do not
interpret an empty native apply summary as proof of complete resource cleanup.
The frozen contract and oracle amendments are in
[`docs/security/m2d-d-session-spec.md`](../docs/security/m2d-d-session-spec.md)
and [the host registration](../docs/security/m2d-d-host-oracles.md).
