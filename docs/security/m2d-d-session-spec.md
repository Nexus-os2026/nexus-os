Mother will use one broker-owned session boundary for every operation reachable from Hands.
Stop will block new operations; a kernel call already handed off may finish its documented internal work.
After emergency, the Owner can view status and evidence or Quit; undo, cleanup and lift are unavailable.
Restart will report surviving evidence and uncertainty, without deleting files or restoring authority.
Completion will require conserved obligations, independent outcome tests, exact-SHA CI and independent review.

**M2d-D session specification**

Frozen design v1, for independent approval before implementation.

Specification baseline: PR #83, `mother/m2d-d-host`, `03e07b1a69e5c56ba3091c44852eb4cfe1c56204`, based on `d20bc1f6ebe5e61561b85544633355f5db9891ce`.

Decision 12 amendment A, Rulings 1–3, controls where earlier decisions conflict. This specification changes the Hands surface and its transitive implementation. It preserves foundational C-lite and legacy client behavior. It authorizes neither kernel changes nor restart cleanup.

All source locations below refer to `03e07b1a`. This specification was prepared read-only: no repository files changed, tests run or remote CI verified.

**1. Admission and handoff contract**

**1.1 Authoritative session**

The broker issues one `HostSession` identity, one original deny-only cell and one ingress handle before Hands starts accepting input. Bootstrap attaches the broker to that existing identity; it must not replace the cell or copy its value into a newly issued cell.

The session exists before workspace selection, door issuance, coordinator attachment or File selection. A Stop during Home initialization therefore affects the same identity that any later operation would carry.

Every host-owned door, run, mission attachment, preparation, commitment, execution and restorative request carries:

- The immutable broker-issued session identity and original cell.
- Its original owner and, where applicable, door, mission, run and action identity.
- Its request identity and original ingress lifetime.
- Its exact scope, target and prerequisite authorities.
- Its inventory reservation and retention obligations.

There is no optional host binding, late adoption, conversion to legacy authority, or reconstruction of authority from journal strings. Unknown, missing, pruned, mismatched or poisoned authority refuses.

The broker maintains one authoritative admission state. Host presentation flags may mirror it but never grant permission.

**1.2 Operation classes**

| Class | Permitted purpose | After emergency |
|---|---|---|
| Work | Bootstrap work admission, workspace/grant/door/run creation, mission transitions, consent installation, preparation, authorization, dispatch and invocation | No new admission |
| Normal restoration/housekeeping | Normal undo; explicit cleanup; separately admitted private-resource cleanup belonging to an ordinary request or normal close | No new admission |
| Settlement | Cancel, revoke, withdraw, retain ownership, classify an outcome, record it, and acknowledge an exact conclusive result | Permitted within existing obligations |
| Observation | Status, evidence and report-only recovery | Permitted |
| Close | Quiesce producers, settle existing obligations, establish the close proof and record session end | Permitted; cannot authorize prohibited cleanup |

A settlement operation may create a **recording obligation**, but cannot create work authority, issue a restorative grant, invoke a lane, remove filesystem objects or call a cleanup-capable broker wrapper.

Normal completion or denial of a mission is not itself an emergency. Normal undo remains available under its existing conditions when no session Stop/emergency has occurred. An explicit session Stop prevents new work and restoration. Mission cancellation, budget exhaustion, revocation and expiry additionally invalidate the work and request lifetimes to which they apply; none automatically grants a fresh restorative request.

**1.3 Request lifetime**

A command envelope is bound when accepted from the input stream, before queuing or dialogue. It contains the command kind, input sequence, session identity, request identity and original admission snapshot.

A queued command retains that snapshot. Dequeuing, retrying a lock, completing a dialogue, returning from validation or invoking a callback must not refresh it.

Child steps inherit their parent request lifetime. They receive separate, single-use step identities, not a reusable permission for an entire call chain.

For action approval, the envelope also binds the exact pending commitment and its original run and mission generation. For normal undo, it binds the exact eligible undo entry before dialogue; the existing kernel watches are captured before dialogue and remain unchanged through U1.

Mission generations, native Stop watches and the session ingress lifetime remain distinct checks. None substitutes for another.

**1.4 Ordering with lock-free ingress**

A mutex around a load is insufficient.

Session admission must use an atomic comparison ordered against ingress on the **same authoritative atomic state**. That state includes the ingress generation and monotone emergency, closing and failure restrictions, together with the existing immutable first-cause information.

At a sink:

1. Validate ownership, target, scope, capacity and prerequisite authorities.
2. Reserve the complete outcome/resource obligation.
3. Perform slow validation and evidence preparation without holding the ingress boundary.
4. Acquire the resource’s owning hold.
5. Perform the final atomic admission comparison against the request’s original snapshot.
6. Consume exactly one step and perform that sink’s bounded mutation or immediate call handoff.

A successful compare-and-exchange against the unchanged snapshot, including a same-value CAS, can supply the admission ordering. An ordinary load followed by action cannot.

For registry mutations, the successful CAS is the mutation’s linearization point; visibility remains protected by the registry hold. Ingress ordered afterward may overlap physical completion of that already-admitted insertion, but every returned handle is already denied by the original cell.

For kernel or lane calls, the successful final CAS is the call-handoff point. Between it and invocation there may be no validation, evidence write, queue, callback, await, second operation or reusable returned permit. The unavoidable instruction/scheduling interval belongs to the explicitly disclosed single-call handoff limit.

Reservation is not handoff. A request paused after reservation, but before this final CAS, must refuse if ingress wins.

Every subsequent broker-initiated call requires a new step admission. In particular:

```text
restore permission ≠ discard permission
retry_cleanup permission ≠ discard_staging permission
publication permission ≠ old-undo-slot cleanup permission
settlement permission ≠ drain permission
```

Ingress performs its atomic denial before worker reservation, locks, allocation-dependent work, output or I/O. Overflow is permanently fail-closed; generation reuse and wraparound are prohibited.

**1.5 The no-kernel limit**

The guarantee is:

> Once ingress is ordered before a sink’s admission, that sink does not start. An already-handed-off call may finish only its documented internal work. Its return does not authorize another work or restorative step.

This is **not** a guarantee that all filesystem effects cease at the instant of ingress.

The permitted internal work of unchanged kernel calls is explicitly bounded:

| Already-handed-off call | Internal work that may finish |
|---|---|
| `CodingRun::grant` | Its staging allocation, records, authority closure and allocation-failure cleanup, including a call-local lease’s finalization |
| `snapshot`, `edit`, structural/page verification or review | The single entered method’s documented reads, candidate changes, validation, records and failure finalization; no next preparation method |
| `apply_observed` | Existing preflight and final checks, at most its one create publication, synchronization, records, grant closure, compensating unpublish and private-storage finalization |
| `restore` | Existing preflight and U1, the confirmed file’s identity-checked unpublish/restore behavior, compensation, records, grant closure and internal private-storage discard |
| `retry_cleanup` | The bounded set of already-owned temporaries processed internally by that one kernel call, its records and internal store finalization |
| `discard_staging` | Removal/revocation of that run’s staging and its internal finalization |
| An already-entered lane | Completion of that invocation; its returned output grants no permission for an effect or another invocation |

The allowance includes the internal `unpublished()` discard at `kernel/src/coding_run/apply.rs:1443` and successful restore’s private-storage discard at `:1853`. It does **not** include broker `retire → discard → discard_staging` after either call returns.

For publication, the existing observer still checks the original session constraint at the kernel’s final publication check:

- Emergency before that check prevents publication, although the call may perform its internal rollback/finalization.
- Emergency after it may allow the one publication to complete.
- Both outcomes retain interruption, recovery and recording facts.

For normal undo:

- Emergency before restore handoff refuses restore.
- Emergency after restore handoff may arrive before U1 and still fail to prevent removal, because U1 has no session observer.
- After return, only observation, narrowing and settlement may proceed. No broker discard, new undo, cleanup batch or lift follows.

Kernel-local cleanup during an entered call is documented finalization. Destruction initiated by host/broker error handling, retirement, container destruction or shutdown is not.

M2d-K3 remains parked. Closing the remaining kernel-internal cancellation window requires separate kernel authorization.

**2. Restart is report-only**

**2.1 Durable inputs and lost state**

Restart may inspect the existing verified kernel ledger, Mother journal and primary broker evidence. It may perform their existing validation and recovery recording.

It does not recover:

- The previous `HostSession`, cell or command lifetime.
- Workspace registration or grants.
- Native confirmations, consent-cache authority or execution tokens.
- In-memory undo authority.
- Inventory handles, pins or acknowledgements as executable capabilities.

The previous process’s inventory is not a durable cleanup manifest. No new persistent schema is introduced.

**2.2 Required report**

Report findings with their provenance and certainty:

| Evidence | Required reporting |
|---|---|
| Prepared apply/restore without a conclusive outcome | Prepared; publication or temporary-file status unknown |
| Recorded temporary identifiers or cleanup debt | The recorded identifier, controlled cause and removal status; existence is not independently assumed |
| Staging allocation or approval preparation without established discard | Staging recorded as allocated/awaiting approval/not established discarded |
| Recovery-required outcome | Exact controlled recovery reason, preserving any known publication and interruption |
| Unfinished Mother settlement or terminal recording | Exact mission/action association where verifiable; unresolved or recording unavailable otherwise |
| Terminal record before lost in-memory acknowledgement | Durable terminal fact; previous acknowledgement completion unavailable unless established by existing evidence |
| Unclean evidence session or owed/uncertain records | Existing health result and its limitations |
| Prior in-memory-only grant/cleanup failure | Explicitly unavailable after process death unless represented in existing durable evidence |
| Unreadable, invalid or torn source | Typed source-specific failure; never an empty or “all clear” report |

A report distinguishes **recorded state**, **current-process observations**, and **unknown state**. It must not claim to reconstruct every previous obligation when evidence was never durably written.

The report is not proof of absence. Missing ledger information does not mean no staging, no publication or no debt.

**2.3 No cleanup authority**

Restart performs no deletion, sweep, compensating undo, staging discard or cleanup retry. It neither opens a prior workspace for mutation nor converts ledger paths, digests, run identifiers or temporary names into deletion authority.

Report-only recovery may open existing stores and write their ordinary session/recovery records. That administrative activity does not authorize workspace or staging cleanup.

The existing no-sweep assertion at `mother/host/tests/faults.rs:387` remains binding.

**2.4 Unresolved history and repeated starts**

An unresolved File mission remains non-runnable. No held-resume, replayed approval, adoption or new command retries its uncertain operation.

A failed or incomplete recovery report prevents work admission. Existing unresolved evidence restrictions remain in force; restart cannot clear them merely by opening another session.

A conclusively completed historical mission does not restore its workspace or undo slot. A fresh mission requires independently valid fresh-session eligibility, a newly selected empty workspace and new consent. Non-empty-workspace refusal remains unchanged.

Repeated starts:

- Report the same unresolved durable facts again.
- Do not progressively remove resources or upgrade uncertainty to success.
- Do not duplicate a mission’s terminal transition.
- May add ordinary new session/recovery evidence under the existing schemas.
- Never make previous cleanup debt disappear merely because its in-memory owner died.

If manual external changes occur, reports must distinguish them from Mother-established cleanup. M2d-D has no acknowledgement workflow that certifies such manual recovery.

Durable restart cleanup, persistent workspace authority and cross-session undo remain separate future work. C-11 remains open.

**3. Emergency, settlement and close state machine**

Lifecycle and obligation state are separate. `Emergency` does not imply either “everything finished” or “clean close impossible.”

**3.1 Lifecycle table**

`Observe` below means `status` and `evidence`. Repeated `stop` remains an ingress event in every live state, rather than an ordinary restorative command.

| State | Event | Next state | Allowed Owner commands | Permitted internal activity |
|---|---|---|---|---|
| Bootstrap | Valid initialization and recovery report; no restriction | Normal | Observe, Quit; work commands only after readiness | Explicit bootstrap steps and reporting |
| Bootstrap | Emergency/EOF/read error | Emergency, or Closing for input failure | Observe, Quit while input remains usable | Complete entered bootstrap calls; Stop recording; no workspace/work admission |
| Bootstrap | Initialization, validation or recording failure | Failed | Observe where available, Quit | Preserve evidence and ownership; bounded shutdown |
| Normal | Valid work/undo/cleanup request | Normal | `run`, `approve`, normal `undo`, `cleanup`, `detail`, Observe, Quit, subject to existing eligibility | Individually admitted steps |
| Normal | Ordinary mission completion/denial | Normal, mission terminal | Normal commands subject to one-mission and undo rules | Finish records/acks; retain or explicitly clean resources |
| Normal | Session Stop/emergency | Emergency | Observe, Quit | Cancellation, revocation, settlement, recording, exact conclusive ack |
| Normal | Fatal host error, poison or exhausted accounting | Failed | Observe where possible, Quit | Deny first; preserve ownership; truthful settlement |
| Emergency | Work, undo, cleanup, lift or `detail` command, including queued commands | Emergency | Refuse; no dialogue or effect | Typed refusal record if recordable |
| Emergency | Repeated Stop | Emergency | Observe, Quit | New ingress receipt; bounded/coalesced Stop recording; no cause replacement |
| Emergency | Entered kernel call returns | Emergency | Observe, Quit | Preserve complete outcome; close authority; retain cleanup debt; no subsequent removal |
| Emergency | Obligations become conclusively settled | Emergency | Observe, Quit | No automatic reopening |
| Normal | Quit | Closing-normal | No further work commands | Cancel dialogue; quiesce; settle; explicitly admitted close cleanup using Quit’s original lifetime |
| Emergency/Failed | Quit, EOF or shutdown request | Closing-restricted | None | Narrowing, recording and observation only; no new cleanup |
| Closing-normal | Emergency before final seal | Closing-restricted | None | Invalidate remaining cleanup steps; complete only handed-off calls |
| Either Closing | Complete close proof, producer quiescence and durable end records | Closed-clean | None | Release only proven safe resources; return success |
| Either Closing | Deadline, failure, uncertainty or any debt remains | Closed-unclean | None | Preserve resources/evidence without cleanup-capable destruction; return nonzero |
| Closed-clean/unclean | Retained handle used | Same | None | Typed ended-session refusal; no new admission |

EOF/read error performs emergency ingress before shutdown. Fatal input/output failures retain the existing nonzero-exit policy.

An idle emergency session can close cleanly once its Stop workers and records finish. A published Owner file is not an obligation to undo it.

**3.2 Complete obligation inventory**

One session inventory replaces independent ownership by `slot`, `kept` and `deferred`. Those names may remain as non-owning indices.

Each entry includes:

- Session/request/mission/run/action identities, including pre-commitment preparation identity.
- Original cell and lifetime.
- Resource state: reserved, preparing, pending, borrowed/in-flight, returned, retained or released.
- Owned kernel object, workspace/grant bindings and observed resource identities.
- Publication class, interruption facts, first terminal cause and recovery state.
- Kernel/broker/Mother recording obligations independently.
- Exact commitment/run pins and acknowledgement state.
- Staging, temporary, private-storage-finalization and authority-closure obligations.
- Worker ownership and any uncertainty caused by panic or lost results.

The conservation rule is:

```text
created obligations
= live + reserved + in-flight + retained + conclusively discharged
```

These sets are disjoint by obligation identity. Taking an effect for execution changes its state; it does not subtract it from accounting.

Before creating a resource, reserve its inventory entry and all necessary retention capacity. Capacity failure occurs before that creation. Never evict unresolved entries to admit another action.

The implementation shall bound action/resource entries at 256 per host session. Each entry reserves its fixed metadata and recording/closure fields before use. Existing stricter broker limits still apply. Stop-worker accounting has a separate four-slot reserve and cannot be exhausted by work admission. Capacity tests cover both limits and their composition.

**3.3 Destruction and unwind ownership**

Session-owned `CodingRun` values stay in inventory-owned, destructor-suppressed storage. Workers borrow them under a per-resource guard; they do not take raw values into ordinary stack locals.

A suitable safe implementation is an inventory-owned `ManuallyDrop<CodingRun>` with explicit release. The exact wrapper may differ, but these behaviors are mandatory:

| Object/path | Required behavior |
|---|---|
| `FilesEffect::drop` | Mark/transfer the existing inventory entry as abandoned or cleanup-pending. No I/O, ledger write, cleanup or new resource-owning allocation |
| Pending-install refusal, authorization failure, cancellation, prune | Release references/reservations; preserve the same resource entry and exact recording debt |
| Borrow/operation guard unwind | Mark outcome unknown where necessary and return responsibility to its existing entry; never drop the kernel object |
| Undo-slot replacement | Change indices; retain the old entry. Cleanup requires a separate admitted step |
| `IssuedGrant` failure/unwind | Preserve exact grant-closure responsibility. Revocation is permitted settlement; failure remains visible |
| Final facade, port, coordinator, worker closure or channel drop | Cannot become the last uncontrolled owner of cleanup-capable objects |
| `StagingLease` nested in a retained run | Its destructor cannot run merely because an effect, inventory or Home is dropped |
| `OwnedTemp` nested in a retained run | Same rule; failed cleanup must not gain a destructor retry |
| Startup failure or spawn rejection | Retain any constructed session/backend payload through the same controlled ownership path |
| Unclean process exit | Retain or intentionally leak the bounded quarantine until OS process teardown; no promise of later cleanup or recording |
| Clean release | Allowed only after cleanup-capable nested resources are proven released, never allocated, or otherwise incapable of removal |

The quarantine must not depend on a poisoned inventory lock being successfully reacquired during `Drop`. Resource storage remains owned even if metadata cannot be updated; that condition itself is an unresolved obligation and prevents clean close.

Kernel-local temporaries created and finalized **inside an already-entered call** remain subject to §1.5. For example, `grant()` can close its own partially allocated lease. Such a panic/error path is part of the enumerated call boundary, is tested explicitly, and cannot justify dropping the containing run after the call unwinds.

An opaque internal cleanup result is not proof of success. In particular, the void private-store discard at `kernel/src/coding_run/apply.rs:2381` cannot alone clear an inventory debt. Verify the expected session-created storage’s absence through read-only observation, or retain `finalization_unverified`. This observation supplies no deletion authority.

Intentionally retained, conclusively recorded apply evidence is distinguished from disposable staging and failed cleanup. Releasing an obsolete undo capability may close its handles while retaining that evidence under the existing no-purge policy. It must not relabel failed or uncertain cleanup as intentional retention.

**3.4 Close proof and bound**

One three-second monotonic deadline covers shutdown, including input, startup, work, Stop workers, settlement, cleanup where allowed, recording, rendering and joins. Helpers do not restart it.

Before any clean-close record:

1. Close work admission and invalidate unconsumed command/dialogue envelopes.
2. Stop and acknowledge the input producer, accounting for ingress already accepted.
3. Quiesce every other ingress/admission producer and join all owned work and Stop workers.
4. Settle every applicable terminal and exact acknowledgement obligation.
5. Complete permitted normal-close cleanup; restricted close performs none.
6. Establish no unresolved publication, in-flight effect, cleanup debt, unclosed authority, owed record, exact ack, or unaccounted resource.
7. Atomically seal the session against new events.
8. Record Mother’s session end and then primary evidence close; require their successful durable results.

A Stop before sealing invalidates the previous proof. No producer may append an unaccounted Stop after sealing. Retained ingress handles then return `SessionEnded`.

A journal close that succeeds before primary evidence close fails is reported as a **partial close**, followed by nonzero unclean exit. Never emit `SESSION CLOSED recorded` for that case.

A timed-out worker remains outstanding. Detaching its handle is not joining it. A bounded exit must not run its cleanup-capable owner destructors.

**4. Sealed route-to-sink catalogue and #83 replacement boundary**

For compact source references:

- `H/` = `mother/host/src/`
- `M/` = `mother/src/`
- `B/` = `crates/nexus-governed-control/src/`
- `K/` = `kernel/src/`

Every reference expands relative to the specification’s repository root.

The broker facade exposes closed operations and opaque handles. It exposes no raw `GovernedControl`, `Authority`, `AgentDoor`, `CodingRun`, generic `Intent`, public checked closure or conversion to a legacy handle.

Mother receives a session-bound port and coordinator admission interface. Authoritative checks remain at **coordinator transition and lane invocation sinks**, not merely at Hands commands. Cross-crate integration must not create a dependency cycle: Mother implements its private transition sinks using broker-issued session bindings; the broker does not depend on Mother’s coordinator.

Catalogue notation:

- **W**: work step; ingress before admission refuses.
- **R**: restoration/housekeeping step; prohibited after emergency.
- **S**: settlement of an existing obligation.
- **O**: observation.
- **L**: lifecycle operation.
- **K**: admitted kernel call, with only §1.5 finalization permitted afterward.
- **D**: route removed from the sealed surface.

All W/R rows carry the original request lifetime. All S rows carry the exact obligation identity rather than a fresh work lifetime.

| ID / route and current call path | Immutable authority and lifetime | Admission, obligations and post-ingress outcome |
|---|---|---|
| R01 Bootstrap: `H/hands.rs:703 → H/lib.rs:70 → GovernedControl::new` | Pre-issued session seed; Owner-selected absolute Home; startup request | L: attach once before exposure. Account initialization and evidence-session ownership. Entered calls may finish; no work authority after ingress. Failed startup uses controlled destruction |
| R02 Store/journal opening: `H/hands.rs:222 → H/lib.rs:119,193 → M/adapter/mod.rs:1058` | Session-owned Home handles and fixed paths | L: journal/private-store writes are separately tracked. Recovery observation never grants mission permission. Preserve file/parent sync and symlink refusal |
| R03 Startup hold configuration: `H/hands.rs:748 → B/governed.rs:964 → B/files/mod.rs:2594` | Immutable startup value, 0–30 seconds; startup lifetime | W: configure once before work. No late configuration through retained raw owner |
| R04 Workspace registration: `H/hands.rs:752 → B/governed.rs:851 → B/files/mod.rs:2696 → :2712,2727` | Session, selected canonical directory identity, exclusions | W: distinguish kernel registration from broker installation. Recheck/admit each after validation. A refused final installation exposes no workspace or grants |
| R05 Standing Files grant: `H/hands.rs:168 → B/governed.rs:1033 → B/authority/policy.rs` | Session, exact workspace/scope/duration and original confirmation request | W: separate dialogue, approval evidence and final grant insertion. Track issued grant closure. Late refusal appends truthful refusal; no usable grant |
| R06 Door issuance: `H/hands.rs:182 → H/lib.rs:207 → B/governed.rs:647 → B/authority/run.rs:611` | Original session, Mother agent, exact reserved door and dialogue lifetime | W at activation, after evidence/validation. Door born with binding. Earlier issuance evidence is preserved with typed activation refusal if activation loses |
| R07 Port/attachment/File selection: `H/hands.rs:193,228,240 → M/adapter/mod.rs:544,551 → B/authority/file_session.rs:257,329,271` | Original door/port/coordinator attachment and same cell | W at claim/selection. No replacement, recycled eligibility or membership growth. Handles themselves grant no work permission |
| R08 Draft/revise/submit: `H/hands.rs:272,279 → M/coordinator/lite.rs:209,213,221 → M/coordinator.rs:1139,1185,1226` | Session, mission reservation, contract reference and request generation | W at each actual state transition, after private-store/journal preparation. Interrupted reservations remain accounted; stale callbacks cannot install runnable state |
| R09 Mission/held consent: `H/hands.rs:283 → M/coordinator.rs:1270 → B/governed/door.rs:365,609 → B/authority/run.rs:1200` | Exact contract hash, request, original door and lifetime | W at consent-cache installation and subsequent coordinator transition. A late yes is unusable. Held File replay cannot restore eligibility |
| R10 Run creation: `M/adapter/mod.rs:428 → B/governed/door.rs:150,163 → B/authority/run.rs:483` | Original session/door, exact mission and run origin | W under run-table hold. Attach binding before exposure. Pre-commitment preparation retains run against pruning |
| R11 Adoption/join/replay: `M/adapter/mod.rs:893,970 → B/governed/door.rs:270 → B/authority/run.rs:808` | Existing immutable origin | D for takeover. Same-origin identity checks remain; no host adoption of legacy/different-door/different-cell runs. Replay may register reporting debt, never execution authority |
| R12 Lane reservation/invocation: `M/coordinator.rs:1717,1724 → H/hands.rs:111` | Session, mission/phase/generation, original parent request | W separately at reservation and invocation. Admit invocation after blocking records. Already-entered invocation may return; its output cannot start a later effect |
| R13 File binding: `M/coordinator/lite.rs:111 → M/adapter/lite_port.rs:59 → B/governed/door.rs:458 → B/authority/run.rs:1319` | Same run/session, immutable mission consent and workspace identity | W at backend installation after filesystem validation. Adapter insertion is a separate projection. Backend refusal cannot be hidden by an empty adapter map |
| R14 Effect selection: `M/adapter/mod.rs:489,503 → B/governed/door.rs:230 → B/governed.rs:393,418` | Session-bound run and allowed File request | W. Sealed port accepts supported File effects only. Network/process/connector/spend/display routes are structurally unavailable or typed refusal before a sink |
| R15 File validation/preparation: `B/files/mod.rs:2780 → :2825 → :2852` | Retained preparation handle, exact admission/grant/name/content | Reserve resource entry and run pin before slow work. Final staging admission uses that handle, never a fresh optional lookup |
| R16 Kernel run/read grant: `B/files/mod.rs:2874,2879 → K/coding_run.rs:832` | Exact project and backend-issued binding to broker run | Separate W steps for read-grant issue and K construction. Track grant closure even if construction fails. Constructor failure does not authorize staging |
| R17 Staging methods: `B/files/mod.rs:1633 → :1640,1641,1642,1649,1654,1671` | Inventory-owned run and parent request | Separate W/K steps for `grant`, `snapshot`, `edit`, verify, page check and review. Never treat `stage_one` as one admitted operation. On ingress between methods, retain current resources and stop the sequence |
| R18 Preparation error/display refusal: `B/files/mod.rs:2892,2901,2910`; proposal return `B/governed.rs:429,435` | Original preparation entry/request | S ownership transfer. Any normal private cleanup is a separate R step. After ingress no `retire` or implicit drain removes anything |
| R19 Commitment/pending installation: `B/control.rs:255,274 → B/authority/commitment.rs:826 → B/control.rs:126` | Exact session/run/preparation/action; retained pin | Separate W admissions for commitment creation and pending installation. Failed install ends the commitment truthfully and retains staging/record debt. No pruning gap |
| R20 Action dialogue/token/authorization: `H/hands.rs:369 → B/control.rs:299 → B/authority/commitment.rs:1099,1203,1246` | Exact commitment binding, original approve envelope and native confirmation | W at token issuance and authorization after final grant validation. Denial records the actual answer. Stop invalidates stale confirmation without claiming it was affirmative |
| R21 Execution dispatch: `H/hands.rs:394 → B/control.rs:324 → B/authority/commitment.rs:1282 → B/control.rs:369` | Exact pending entry, execution guard, original request | Taking pending ownership is S, not admission. W after revalidation and before invocation. In-flight entry remains counted on every failure |
| R22 Apply preparation/publication: `B/files/mod.rs:1068 → :1115,1126,1141,1154 → K/coding_run/apply.rs` | Original guard/session, review hash, candidate, exact write grant | Separate W/K admissions for approval bridge, page recheck, grant issue and apply. Preserve observer at `B/files/mod.rs:1339`. Kernel-internal finalization allowed; no subsequent broker cleanup after ingress |
| R23 Outcome/undo-slot replacement: `B/files/mod.rs:1155,1207,1216 → :735` | Exact executing entry and prior undo-entry identity | S: classify, record receipt and change non-owning undo index. Old entry remains owned. Its cleanup requires its own R admission using the causative request lifetime |
| R24 Deny/cancel/halt/prune/finish: `H/hands.rs:372 → B/control.rs:410`; `M/adapter/lite_port.rs:139 → B/governed/door.rs:246`; `B/governed.rs:333,361` | Exact owned run/action and settlement obligation | S allowed after ingress. Split `settle` from `files.drain`. Pruning releases neither unresolved resources nor required pins |
| R25 Mother settlement/D3/ack: `M/coordinator/lite.rs:192 → M/coordinator/lite_settle.rs:166,210,335,467,494 → B/governed/door.rs:311 → B/authority/commitment.rs:603` | Exact mission/effect/action, immutable first cause and conclusive record | S. Withdraw before deadline snapshot. Preserve full outcome. Ack only exact conclusively settled action after durable terminal recording; cleanup ownership survives ack |
| R26 Emergency/worker recording: `H/stop.rs:213 → B/governed.rs:551` | Original ingress handle and event/worker identities | Atomic ingress first. Worker performs native narrowing and synchronous recording. Max four workers; coalescing still records ingress receipts. No cleanup drain |
| R27 Normal undo: `H/hands.rs:470 → B/governed.rs:931 → B/files/mod.rs:2326,2381 → K/coding_run/apply.rs:1755,1840` | Exact current undo entry, original request, separate affirmative confirmation and original native watches | R/K at request/approval/grant/restore boundaries. Emergency before restore handoff refuses; afterward the entered restore may finish. Slot return/retirement is S only |
| R28 Inherited undo drain: `B/files/mod.rs:2343 → :2297 → :712` | Exact bounded cleanup-entry list and original undo-command lifetime | Replace with explicit per-entry R calls before dialogue as applicable. No implicit batch permission; no claim these removals were confirmed undo |
| R29 Explicit cleanup: `H/hands.rs:498 → B/governed.rs:980 → B/files/mod.rs:2518 → K/coding_run/apply.rs:1019; K/coding_run.rs:2226` | Snapshot of eligible inventory IDs, original cleanup-command lifetime | One attempt per selected entry. `retry_cleanup` and staging discard are separate R/K steps. Preserve interrupted current entry and unattempted tail. Count all inventory states |
| R30 Implicit drain/retirement: `B/governed.rs:337,435`; `B/files/mod.rs:2862,712,847,2373` | Existing resource entry; never ambient owner authority | Remove implicit removal from session paths. Normal cleanup scheduler requests explicit R steps; emergency settlement only transfers/records |
| R31 Native lift: `H/hands.rs:434 → B/governed.rs:580 → B/authority/run.rs:1159` | None exposed through Hands | D. `lift-for-undo` is refused without dialogue in every Hands state. Legacy native resume remains outside this surface |
| R32 Inference binding/model attempt: retained door `B/governed/door.rs:500,544 → B/authority/run.rs:1377,1435 → B/inference.rs:337,371` | No such capability issued to Hands | D at the sealed surface. Compile/source tests prove no route; backend positive controls still exercise configured legacy inference and attempts. No-model configuration alone is insufficient evidence |
| R33 Status/evidence/detail/recovery: `H/hands.rs:412,518,532,646 → H/lib.rs:225,230 → B/governed.rs:949,955 → B/files/mod.rs:2468` | Read-only session projection; take-once working detail | O; any existing `touch` recording is explicitly S. No drain. After emergency `detail` command is refused; permitted evidence projections remain bounded and truthful |
| R34 Quit/shutdown/session end: `H/hands.rs:565,890 → M/coordinator/lite.rs:286 → B/governed.rs:502 → H/lib.rs:219` | Original Quit request and complete inventory | L/S, with R only during normal close under §3. No count-only shutdown success. Clean result requires the complete proof |
| R35 Destruction/startup failure: `H/hands.rs:794,598,904`; `B/files/mod.rs:1245`; `K/coding_run.rs:667`; `K/coding_run/apply.rs:621` | Inventory/quarantine ownership, including channel and worker payloads | Pure ownership disposition outside entered kernel calls. No hidden cleanup on failure, unwind, final-owner drop or process shutdown |
| R36 Queued commands/retained handles: `H/hands.rs:823,849,873`; retained `Mission` at `:129` | Original envelope and immutable session binding | Every use reaches the same final sink admission. Queuing, cloning, reserving, callback completion and dropping/rebuilding adapters cannot refresh authority |

R14 and R32 are deliberately sealed exclusions, not new features. Adding any excluded route requires a new specification and approval.

The implementation must provide a relocation map from every catalogue row to its final sink functions. Each kernel call within R17, R22, R27 and R29 is a distinct generated-test sink.

**4.1 Exact disposition of #83 code**

| Current code | Required disposition |
|---|---|
| `H/stop.rs::Gate`, route counters and `M/adapter/mod.rs::AdmissionGate` | Delete as authority mechanisms. Retain presentation/receipt projections derived from the broker session |
| `H/hands.rs::GatedLane` | Replace with a session-bound invocation wrapper checked at the actual invocation boundary; remove independent host permission decisions |
| `Runtime`’s raw `Home`/owner access and `FileSession<GovernedPort>` construction | Replace with opaque session services and a session-bound Mother port/coordinator. Hands cannot obtain raw owner or door handles |
| `Home::door_with_halt` and public host `_with_halt(... Option<HaltCell>)` routes | Remove from the host surface. Replace with mandatory session methods. Preserve ordinary legacy APIs and behavior |
| Mutable/optional late host-cell association | Replace for host objects with immutable issuance-time binding. Keep no-adoption checks and fail-closed unknown-run handling |
| `RecoveryCheck` as permission to start fresh post-emergency restoration | Delete that host behavior. Normal requests use the original session request lifetime; no emergency exception |
| `lift-for-undo` implementation and stale-lift re-engagement machinery | Delete from Hands. Remove #83-only host lift plumbing; preserve native legacy resume |
| `Shared::{slot,kept,deferred,unclosed}` as independent owners; `FilesEffect::run: Mutex<Option<CodingRun>>` | Replace with one inventory and non-owning indices/entry references |
| `Shared::retire`, `applied`, `not_published`, `Files::drain`, `drain_recovery` | Separate ownership/settlement from explicit cleanup. No unconditional cleanup in session paths |
| `Runtime::close` and startup-close destructor worker | Replace with inventory close proof and controlled safe release/quarantine. Destruction is not a cleanup mechanism |
| C-lite publication extraction, exact effect association, D3, first-cause handling, terminal records, pins/acks | Keep and integrate; do not collapse or generically rewrite them |
| Original-cell/no-adoption repair, Decision 10 pruning refusal | Keep their invariants and regression schedules |
| Terminal CR/LF/CRLF, CRLF rendering, stdout-lock repair, acknowledged fresh input, bounded queues, fresh codes and walk renderer | Keep |
| Bounded Stop workers, distinct panic/failure receipts and startup-independent reporter | Keep; bind to session ingress/event accounting |
| FileJournal file/parent sync repair, privacy guards and narrow scanner exceptions | Keep |
| Kernel, desktop behavior, workflows and dependencies | No new changes. Preserve `#![deny(unsafe_code)]`; retain approved host `uuid`/Linux `nix` additions only |

Existing regression tests are retained except for the explicit expectation replacements in §5. Their security schedules remain covered.

**5. Oracle, H2′, documentation and Owner-notice amendments**

Amendments must be committed before the implementation they judge. Each `test(oracle-amend)` records the old assertion, replacement assertion, affected test and Decision 12 amendment-A justification. Historical evidence is preserved.

| Existing expectation/location | Frozen replacement assertion |
|---|---|
| Host oracle protocol: commands and recovery semantics in `docs/security/m2d-d-host-oracles.md` | After emergency only status/evidence/Quit commands; repeated Stop still ingresses. No lift/undo/cleanup dialogue or sink |
| “Recovery B” at `host-oracles.md:116`; `faults.rs:90` | Preserve actual post-final-check publication and interruption evidence. Then refuse lift, undo, cleanup, run and approve; zero new sink admissions. File remains unless an independently pre-emergency restore was already handed off |
| Composed counters after successful emergency undo, `faults.rs:280` | Remove emergency undo success. Keep queued/reserved work and repeated Stop schedules; observe real backend transitions and resource changes, not `Gate` counters |
| `stop_tests.rs:7,68` synthetic gate coverage | Replace authority claims with session-sink schedules and successful real-route controls. Presentation-counter tests may remain presentation tests only |
| Pre-selection recovery, `pty.rs:198` | Emergency before selection prevents door/run/File selection and all work; lift is refused without dialogue |
| Repeated Stop during lift/undo, `pty.rs:381` | First emergency during a **normal** undo at each boundary; repeated emergency thereafter. Before handoff: no restore. After handoff: exact allowed finalization. No fresh undo or lift afterward |
| H1 worker-cap test, `faults.rs:136` | Preserve four held workers, fifth ingress/coalescing and receipts. Lift refusal is unconditional host unavailability, not merely “workers outstanding” |
| Cleanup stall after emergency, `faults.rs:333` | Start normal cleanup before the first emergency, stall its real kernel boundary, then ingress twice. Reader remains responsive; entered call may finish; next cleanup call does not start |
| Cleanup success after emergency, `faults.rs:364` | Split: normal cleanup positive control succeeds; emergency pending-effect drop retains staging, cleanup refuses, inventory reports debt and Quit exits unclean |
| Decision 11 host outcome tests, `hands_tests.rs:537–549` | Normal undo/cleanup overtaken by first emergency report actual result; declined requests never say “confirmed”; remove successful host-lift scenario and replace with unconditional refusal |
| Host-bound portions of `files_host_decision10_tests.rs` | Use mandatory session routes. Preserve pre-handoff refusal, post-handoff outcome, pruning and tail-retention schedules. Remove fresh post-emergency recovery success expectations |
| Legacy recovery controls in those files and frozen kernel/broker halt/restore suites | Keep unchanged outside the sealed surface; their success does not expose those routes to Hands |
| Restart test, `faults.rs:387`; D-O11 | Keep no-sweep assertion; add repeated starts, lost in-memory accounting and explicit unknown reporting |
| D-O19 normal undo | Keep full facts, affirmative/negative answers, edited-file refusal, once-only semantics and unavailable restart undo. Add first-emergency boundary schedules |
| Close S9 and successful create/deny Quit tests | Preserve successful normal clean-close controls, now requiring explicit resource accounting before close. Add idle-emergency clean control and debt-bearing emergency unclean control |
| Broad “zero effects after ingress” assertions | Replace with: zero **new admissions** after ingress; only enumerated pre-admitted call completion may change resources. Count finalization separately, never ignore it |
| Original D-O8/H2′ host emergency-resume wording | Apply the host amendment below; foundational kernel/broker ordering-B behavior remains unchanged |
| Original MD-14 restore-watch plant | Keep as a legacy kernel/broker control. It is no longer evidence for a supported host emergency-resume route |
| Original no-kernel-observer plant proposal | No new K3 observer is expected. Replace with plants admitting a second broker call or weakening the existing publication observer |

**Host amendment to H2′**

> Hands offers undo only as a normal, Owner-confirmed, same-session operation before emergency. Its original request lifetime and existing native watches are captured before dialogue and never refreshed. Emergency before restore handoff refuses restore. Emergency after handoff may be followed by completion of that entered restore, including its internal finalization. Hands offers no lift, fresh undo or cleanup after emergency. The original apply run is never rearmed.

The original kernel and legacy broker H2′ contract remains unchanged.

Update these documents together:

- `docs/security/m2d-d-host-oracles.md`: protocol, ingress ordering, catalogue, close proof, generated matrix and amendments.
- `mother/README.md`: command table, handoff limit, restart report, cleanup restrictions, receipts and installed acceptance.
- Root `README.md`: accurate candidate scope and limitations.
- `p0-m2d-design.md`: explicit host-only addendum to §6.3, §6.5, §6.6, D-O8/D-O19 and H2′.
- `m2d-b2-h1-decision.md`: host addendum preserving H1 while removing the supported host-resume expectation.
- Decision 12: retain its original history; amendment A and this specification supersede the cleanup-on-restart promise.
- PR description and physical acceptance procedure: identify every changed host expectation.

No documentation may say that removing emergency undo closes the kernel handoff window.

**Exact emergency notice**

> Emergency Stop received. Mother will admit no new work, undo or cleanup in this session. An operation already handed to the kernel may still finish, including its internal cleanup. You can view status and evidence, or quit. Restart only reports surviving evidence and uncertainty; it does not clean up or restore undo.

**Exact uncertainty notice, unchanged**

> A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.

**Exact restart limitation notice**

> Recovery report only. Mother has not removed files or staging, restored workspace permission, or restored undo. Some unfinished work may be known only from incomplete evidence.

**Exact unclean-exit notice**

> UNCLEAN EXIT: unfinished or unverified obligations remain. Mother has not recorded a clean session close. No cleanup or recording is promised after this process exits.

Use outcome-specific messages rather than an unconditional “confirmed” phrase:

- `Undo was not confirmed. No restore was handed off.`
- `Stop received before undo handoff. The restore did not start.`
- `Stop received after undo handoff. The restore completed; remaining obligations are reported separately.`
- `Stop received after undo handoff. The restore was refused.`
- `Stop received after undo handoff. The restore outcome is uncertain; recovery is required.`

Only the actual confirmation and handoff facts select these messages. Cleanup is requested, not described as natively confirmed.

Keep `STOP REQUEST RECEIVED`, corroborated session/native halt and `STOP RECORDING COMPLETED/FAILED` distinct. Recording completion means the worker’s synchronous phase returned; durability and debt remain separately reported.

`SESSION CLOSED recorded` is emitted only after the complete clean-close proof and durable end results.

**6. Independent expected outcomes, generated verification and exit criterion**

**6.1 Independent reference model**

Before production changes, implement a small test-only reference model from the rules below. It must not call production admission predicates, publication classifiers, transition functions or cleanup counters.

The model consumes scheduled events and primitive fixture observations. Production results are compared with its expected state and obligations.

Its state consists of:

```text
Session:
  identity, lifecycle, ingress generation, emergency history,
  first terminal cause, producer/worker set

Request:
  identity, original generation, exact scope,
  confirmation {unasked, declined, affirmed},
  step states {reserved, refused, handed_off, returned}

Action:
  exact mission/run/commitment identity,
  publication {not_dispatched, pending, confirmed_absent, published, uncertain},
  interruption set, recovery_required, terminal disposition,
  recording status per store, exact acknowledgement state

Resources:
  unique resource IDs, owning inventory entry,
  state {reserved, live, borrowed, retained, released},
  cleanup/closure evidence and unresolved facts
```

Core transition rules:

| Event | Independent expected result |
|---|---|
| Request accepted | Snapshot current lifetime; acceptance alone creates no effect permission |
| Ingress before final admission | Step refused; no sink-side mutation/call; earlier obligations conserved |
| Ingress after final admission | Only that admitted mutation/call may complete; no child/successor admission |
| Publication interruption before kernel final check | No publication; internal finalization may occur; cleanup/record failures remain obligations |
| Interruption after kernel final check | One publication may occur; retain all actual interruption/recovery/record facts; no continuation |
| Restore handed off before first emergency | Existing kernel outcome is allowed; no separate post-return cleanup |
| Drop or failed transfer | Same resource remains owned; no removal |
| Capacity refusal | No unreserved resource created and no unresolved entry evicted |
| Record failure | Operational denial remains engaged; recording debt remains; no durable-success claim |
| Exact conclusive acknowledgement | Release only its pin/ack obligation; resource-cleanup debt is unaffected |
| Unknown/mismatched/duplicate acknowledgement | Wrong identity refuses; exact duplicate is idempotent; no additional release |
| Close with any debt/unknown/in-flight item | Unclean, nonzero; no clean-close receipt |
| Idle emergency, all records complete and inventory clear | Clean close permitted |
| Restart | Evidence-derived report; no authority reconstruction or resource removal |

Publication and terminal disposition are independent:

- `Executing`, generic failure, missing lookup and unreadable state do not prove nonpublication.
- Only conclusive nonpublication permits `confirmed_absent`.
- Pending/unknown/unreadable results at terminal settlement become uncertain.
- Published × interrupted/not-interrupted × recovery-required/not-required must all remain representable.
- Record status is an additional independent dimension.
- Stop, budget, denial, lane failure and verification failure preserve the first established disposition. A later generic interruption cannot replace it.
- A terminal uncertain mission does not become runnable when a late result arrives. Later evidence may refine the reported physical outcome without rewriting its terminal history or acknowledging an unresolved terminal record.
- Ordinary approval success after invalidation is forbidden; truthful reporting of an already-handed-off publication is required. Oracles inspect both ordinary `ActionApproved` and settlement publication records.

D3 is specified independently: when Mother’s deadline wins, withdraw/exclude further dispatch first, then obtain the exact action’s publication snapshot. A still-pending result cannot become a plain lapse proving nothing happened.

The reference model’s resource set is compared with actual workspace/staging state, backend entries and verified durable records. An implementation-reported inventory total cannot establish conservation by itself.

**6.2 Generation scheme**

Generate the Cartesian product of:

```text
catalogue sink
× applicable pause point
× ingress variant
× relevant primitive outcome/failure
```

Required pause points:

1. Input acceptance and queue insertion/dequeue.
2. Identity/token/lifetime capture.
3. Before/after dialogue, including decline.
4. Before/after each slow validation and evidence write.
5. Before acquiring the owning hold.
6. After reservation, before final admission.
7. Immediately before/after final admission.
8. Kernel entry/preflight and existing final check.
9. After physical effect, before outcome recording.
10. Before each subsequent cleanup call.
11. Ownership transfer, pin/ack release and pruning.
12. Close assessment, input quiescence, final seal and each end-record boundary.
13. Error, unwind, spawn rejection and final-owner destruction.

Ingress variants:

- Real terminal CR, LF and CRLF Stop.
- Direct original-cell emergency with the worker unscheduled.
- Repeated emergency.
- EOF, partial-line Ctrl-D, master close and read failure.
- Applicable mission Stop, Stop-all, door halt/fence and budget exhaustion.
- Applicable grant revocation, grant expiry, commitment expiry on both clocks, kernel Stop and kernel write-grant expiry.
- Quit/closing and fatal host failure where they restrict admission.

Every work/restoration sink has an unscheduled-worker emergency case. Native Stop propagation must not conceal a missing session boundary.

Each excluded combination has a checked-in reason: unsupported route, no such authority, no filesystem sink, or unreachable lifecycle. “No configured model” is not a substitute for proving route sealing and retaining backend positive controls.

For internal cleanup batches, generate interruption at every existing internal item boundary. The expected result permits the already-entered batch to finish; it forbids the next broker call. Tests must not incorrectly demand a new session check inside unchanged kernel code.

**6.3 Bounded multi-event exploration**

Exhaustively explore up to three concurrent actors and three interfering events over the small reference machine, including:

- Queue → emergency → dequeue.
- Reserve → emergency → invoke.
- Prepare → cancel → prune.
- Legacy execution token → attempted host adoption → emergency.
- Pending installation → failed ownership transfer → drop.
- Normal undo handoff → emergency → attempted cleanup.
- Cleanup item A → emergency → item B.
- Publish B → replace undo slot A → emergency.
- Emergency → repeated emergency → Quit.
- Close assessment → emergency → end-record attempt.
- Poison → emergency → healthy settlement touch.
- Terminal append → failed ack → capacity pressure.

Include deterministic longer traces for File A → non-File action → File B, all four Stop-worker slots plus fifth coalesced ingress, repeated starts, and capacity exhaustion/release.

This bounded claim must be reported as bounded coverage, not proof of all possible schedules.

**6.4 Capacity, poison and process death**

Exercise zero/one/boundary/full capacity in each registry, consent table, pending table, inventory, worker reserve and input queue. Include simultaneous capacity pressure on run retention and commitment pins. Successful controls must demonstrate admission when capacity is available.

Inject poison or panic before and after each ownership-changing hold: session history, run/commitment/pending tables, Files resource access, coordinator, terminal intent, Stop worker state and close accounting. Engage denial first; preserve resources and report typed failure.

Real child-process death is mandatory at:

- Staging preparation/allocation.
- Durable settlement start.
- Kernel apply preparation and final publication boundaries.
- Restore preparation and restore completion.
- Cleanup handoff and interrupted cleanup.
- Terminal append before/after sync and a torn terminal line.
- Fence/poison settlement.
- Terminal record before exact ack.
- Mother close before primary evidence close.
- A pending Stop record.

Each child must emit an exact boundary receipt before the parent kills and reaps it. Restart uses genuinely new broker/process state.

Require the exact expected successful kernel recovery report where applicable, including the torn-Mother-journal case. Separately require Mother to reject the invalid journal. An `err:` string or generic recovery output never satisfies success.

Repeat the restart and compare resource identities/content and findings. No deletion or invented authority may appear.

Retain real PTY tests, cursor-column rendering checks, safe-output/privacy scans, acknowledged partial-input tests and real storage-stall fixtures.

**6.5 Required plants**

Run each plant independently, require the specified oracle to fail, then restore the source. Plants are not committed.

| Plant family | Required detection |
|---|---|
| One bypass per W/R sink | Corresponding generated schedule observes forbidden admission/effect |
| Raw owner/door escape, legacy conversion or generic checked closure | Compile/privacy/source-boundary guard fails |
| Refresh lifetime after queue/dialogue/validation | Original-request schedule fails |
| Move final check before slow work | Held-validation schedule fails |
| Replace CAS ordering with load/check/action | Forced ingress in the gap fails |
| Attach/change session cell after issuance | Retained token/preparation schedule fails |
| Missing/pruned run becomes legacy/no-cell | Pre-stage schedule fails before allocation |
| Remove preparation retention or commitment pin | Capacity/prune schedule fails |
| Reuse restore/retry permission for discard | Post-return emergency schedule sees a second call |
| Restore implicit drain in settlement/proposal/staging | Transitive-route schedule fails |
| Drop old undo slot directly | Slot-replacement conservation/destructor schedule fails |
| Omit reserved, deferred, borrowed, kept, unclosed or recording entries | Resource-set/close comparison fails |
| Drop `CodingRun`, `StagingLease` or `OwnedTemp` on error/shutdown | Actual-resource destructor schedule fails |
| Ignore private-storage finalization uncertainty | Failed-discard close test fails |
| Ack wrong/uncertain/undurable action or release twice | Exact-association and capacity tests fail |
| Collapse publication/interruption/recovery or overwrite first cause | Outcome-product model comparison fails |
| Treat worker panic/spawn failure as recording completion | Receipt and close tests fail |
| Delay engagement to worker or put broker/output/I/O on reader | Unscheduled-worker and H1 stall tests fail |
| Reintroduce emergency undo/cleanup/lift | Command and sealed-route tests fail |
| Sweep on restart, restore consent, adopt or retry uncertainty | Real-death/repeat-start tests fail |
| Accept LF only, hold stdout while waiting, accept stale partial answer | Real PTY tests fail |
| Accept recovery error, skip torn assertion, fake boundary receipt | Harness self-tests fail |
| Weaken existing publication observer | Before-final-check physical publication test fails |
| Close before worker/debt drain or admit ingress after seal | Close-race model and durable-record checks fail |

Each ownership transfer in the implementation receives at least one omission/drop plant. A source scan alone cannot substitute for a behavioral plant.

**6.6 Full exit criterion**

The implementation mission is complete only when all of the following hold:

1. **Specification approval:** independent Codex review approves this frozen specification before production implementation.
2. **Oracle-first evidence:** the independent model, amendments, route manifest and tests are committed first. Baseline red evidence distinguishes unavailable APIs from observed behavioral failures.
3. **Sealing:** the final call graph covers every catalogue row and transitive sink. Compile/privacy/source guards prevent Hands and retained handles from reaching raw routes.
4. **Admission and accounting:** generated outcomes agree with the independent model, actual resources, backend state and durable records. No unexplained exclusions or unowned resources remain.
5. **Plants:** every required route, boundary and ownership-transfer plant is detected; all restored controls pass.
6. **Local gates:** formatting; all-target warning-denied Clippy for the five affected/foundation crates; `cargo test --workspace --locked --no-fail-fast`; mandatory PTY/crash suites; all 23 named desktop live receipts; privacy/source guards; `git diff --check`; dependency and scope inspection.
7. **Dependency boundary:** relative to `d20bc1f6`, `Cargo.lock` changes only the `nexus-mother-host` dependency list by adding `nix` and `uuid`. No package/version churn, kernel change or workflow change.
8. **Exact-SHA CI:** local candidate, remote candidate and tested workflow revisions agree. At this snapshot the current `ci.yml` jobs are `test-linux` and `security-audit-linux`; require both green, plus `audit.yml`’s `security-audit` on the same candidate SHA. Verify all steps, not only workflow conclusions. Fast-local CI is supplementary.
9. **No hidden failures:** no disabled meaningful test, authoritative `continue-on-error`, weakened assertion or accepted unexplained red gate. The known `hm_m3_n6_a_p4` flake remains an explicit blocker if it fails a required gate; this specification does not authorize its repair or waiver.
10. **Independent implementation review:** fresh-context review receives the diff, frozen specification, model, invariants and evidence. Findings are resolved and required checks rerun on the final SHA. Preserve MOTHER.md’s separate security-core review and Owner approval requirements.
11. **Owner reporting:** provide the starting/final SHAs, bounded repair branch and commits, per-repair files, tests, plant receipts, CI run/job IDs, amendments, limitations and remaining issues.
12. **Acceptance boundary:** installed-binary physical acceptance is recorded separately with matching binary/commit receipts. Automated success does not claim H1 acceptance, C-11 closure, portable support, capability acceptance, merge approval or phase advancement.

The implementation should refactor #83 through reviewable commits and preserve its history, rather than restart from an unrelated base.

Planning estimate: **12–18 engineering days**, followed by **3–5 working days for independent review, repairs and final CI**. This includes approximately 1–2 days for the executable reference model and amendments, 3–5 for the facade and sink migration, 3–5 for resource ownership/close handling, and 4–6 for generated schedules, process-death fixtures and plants, with some overlap. Removing K3 reduces kernel work; it does not make ownership, unwind and close verification a small patch.
---
## Binding amendments after the Codex check (`m2d-d-spec-codex-review-1.md`: APPROVE WITH CHANGES)
Both MEDIUM amendments in that review are part of this frozen spec, exactly as written there. The builder implements them as written and does not reinterpret them.

- **MEDIUM — §§1.3–1.4, 3.1 and 3.4: normal-close cleanup has no consistent admission rule.** Requests retain their original snapshot; the authoritative atomic includes `closing`, and admission compares against the unchanged snapshot. Entering `Closing-normal` therefore invalidates Quit’s snapshot, although close cleanup must use Quit’s original lifetime. See [spec:67](/home/nexus/NEXUS/m2d-d-session-spec.md:67), [spec:220](/home/nexus/NEXUS/m2d-d-session-spec.md:220) and [spec:294](/home/nexus/NEXUS/m2d-d-session-spec.md:294). Freeze an explicit, cleanup-only close transition that preserves the original ingress generation, invalidates ordinary requests, and refuses remaining cleanup when emergency/failure intervenes. Add its independent model rule, successful normal-close control and emergency-race plant.

- **MEDIUM — §4, R08/R21: the catalogue still omits reachable work sinks.** R21 places admission **after revalidation**, but [control.rs:340](/home/nexus/NEXUS/mother-m2d-d/crates/nexus-governed-control/src/control.rs:340) calls `FilesEffect::revalidate`, which invokes kernel `review()` at [files/mod.rs:1054](/home/nexus/NEXUS/mother-m2d-d/crates/nexus-governed-control/src/files/mod.rs:1054). That call records `ReviewComputed` and can withdraw verification; it is not pure validation. R08 also omits the actual `advance`, `phase_done` and `resolve_wait` transition paths at [coordinator.rs:1402](/home/nexus/NEXUS/mother-m2d-d/mother/src/coordinator.rs:1402), [:1495](/home/nexus/NEXUS/mother-m2d-d/mother/src/coordinator.rs:1495) and [:1521](/home/nexus/NEXUS/mother-m2d-d/mother/src/coordinator.rs:1521). Expand [spec:358](/home/nexus/NEXUS/m2d-d-session-spec.md:358) and the sink-generation requirement at [spec:377](/home/nexus/NEXUS/m2d-d-session-spec.md:377): give these sinks explicit authority, admission, obligation and post-ingress rules, with generated schedules and bypass plants.

No HIGH or separate LOW findings.

The six-item assessment is:

1. **Contract:** substantively resolved, including kernel-internal finalization and emergency during normal undo; close admission needs the correction above.
2. **Restart:** resolved—report-only, no reconstructed deletion authority, honest uncertainty and repeated-start behavior.
3. **Lifecycle/destruction:** inventory and quarantine rules are concrete; close-request semantics remain inconsistent.
4. **Sealed catalogue:** requires the listed additions.
5. **Amendments/notices:** resolved, including H2′, explicit host replacements and non-overclaiming Owner wording.
6. **Independent verification:** model, generation, plants and full exit gates are specified; extend them for the two corrections.

**APPROVE WITH CHANGES — exactly the two amendments above, frozen before production implementation.**

The no-kernel boundary and preservation of foundational C-lite/legacy behavior are explicit. The baseline kernel diff against `d20bc1f6` is empty. Read-only review at `03e07b1a69e5c56ba3091c44852eb4cfe1c56204`; no files changed, tests run or CI verified.
---
## Binding amendment B (from the pre-mortem `m2d-d-spec-premortem.md`; adopted by the Architect 2026-11-04)
The pre-mortem's full findings table and builder-choice list (lines 1–165 of `m2d-d-spec-premortem.md`) are binding context. Amendment B below is binding text, and overrides v1 wherever they differ.

> **Binding amendment B — complete implementation semantics**
>
> **A0. Scope and precedence**
>
> This amendment supplies the frozen answers requested by the final pre-implementation review. It applies only to the Hands session and its transitive implementation. It does not authorize kernel changes, workflow changes, new persistent schemas, legacy behavior changes, new effect types, restart cleanup, merge, capability acceptance or phase advancement.
>
> All baseline validation limits, privacy rules, supported File classes, native-watch behavior and legacy positive controls remain unchanged except where this specification explicitly replaces a host expectation. Private implementation names may vary; the observable semantics below may not.
>
> **A1. Normal-close capability**
>
> Quit acceptance atomically closes ordinary work admission and captures the original session identity, ingress generation, request identity and a cleanup-only close identity. The original command envelope is immutable.
>
> Normal-close cleanup uses a distinct opaque close capability. It authorizes only cleanup of session-owned resource obligations already reserved when Quit was accepted, including resources subsequently returned by calls already in flight at that time. It cannot authorize workspace registration, grants for work or undo, doors, runs, preparation, publication, restoration of an Owner file, lane invocation, held resume or native lift.
>
> Its final admission predicate is: the same session and close identity; the original ingress generation; Closing-normal still active; no emergency, fatal failure or seal; the exact selected resource still owned and eligible; and the relevant resource prerequisites still valid.
>
> The CAS may compare the current authoritative word only after validating that predicate against the original close capability. This is the sole closing-state exception to comparison against an ordinary request’s original word. It does not refresh the ingress generation, target set, consent or native watches.
>
> Ordinary terminal settlement during close does not mint a new session ingress or refresh close permission. Mission disposition and session ingress remain distinct. Any emergency, input failure or fatal host failure changes the restricting state before blocking work and invalidates every remaining cleanup step.
>
> Each cleanup call requires its own admission. Retrying a CAS, lock or record cannot enlarge the capability. Repeated Quit returns the existing close operation and never creates another capability or deadline.
>
> **A2. Catalogue closure and call boundaries**
>
> Add R37 for operational private-store work. Contract writes and paused-phase writes are Work, not bootstrap administration or settlement. Inventory reservation precedes creation. Directory creation, temporary-file creation, content writes and rename/publication each reach a closed session sink. A filesystem API call already admitted may finish that call; it grants no permission for the next API call.
>
> The error-path temporary removal in `FilePrivate::put` is a separate restoration/housekeeping sink. Session paths must not call the legacy `put` implementation in a way that leaves its unconditional removal reachable. If removal is no longer admissible, retain the exact temporary’s ownership and cleanup debt. Content blobs intentionally retained by the existing private-store policy are recorded as intentional retention, separately from failed temporary cleanup.
>
> Expand R08 to include consent-request reservation, consent installation, Start, phase completion, verification completion, approval recognition, dispatch reservation, dispatch reconciliation, paused-rest storage, ApprovalPending installation and AwaitingApproval installation. Each work transition has the A3 boundary. Terminal settlement and exact recording use settlement boundaries and cannot install runnable state.
>
> Expand R21 to identify separately: pending borrowing, kernel `review` during revalidation, commitment start installation, effect dispatch and each downstream R22 call.
>
> The entered-call ledger must name the kernel project-selection call, project read-grant issuance, `create_for_project_observing`, each R17 method, `request_approval` including its internal review, `recheck_inert_page`, write-grant issuance, `apply_observed`, `request_restore`, `restore`, `retry_cleanup` and `discard_staging`.
>
> For each entered call, the manifest records its exact arguments and owned resources, internal allocation/record/closure/finalization behavior, returned proof, and remaining uncertainty. Nested work inside one unchanged kernel method belongs to that method’s documented boundary. Returning from it never admits another broker call. A getter is excluded only if it performs no mutation, recording, authority acquisition or cleanup.
>
> Administrative Home/store creation and journal session/recovery records are L/S operations with their own accounting. They do not authorize operational content writes.
>
> R09 exposes no held-mission confirmation or resume through Hands, before or after emergency. R11 exposes no adoption or membership-growth operation. Required trait methods on the session port return typed refusal without dialogue or authority changes.
>
> Explicitly include `deny`, grant revocation, finish, shutdown, prune and error wrappers in the transitive manifest. Session settlement calls their narrowing implementations; it never reaches a wrapper that also drains Files.
>
> The destructor manifest includes `ExecutionGuard`, `ProjectReadLease`, `IssuedGrant`, `Place`, `Reserved`, `ConsentSlot`, `RecordBatch`, private-store temporaries, channel payloads and rejected thread closures, as well as the previously named kernel-resource owners.
>
> **A3. Coordinator journal and transition ordering**
>
> A work transition is an exact, single-step record-before-projection operation. Semantic validation, exact generation/reservation validation and record serialization precede its final admission.
>
> Under the coordinator’s owning hold, perform the session admission comparison immediately before handing that one transition to its append operation. Its append/sync and projection of that same transition are completion of the already-admitted transition. This explicit transition boundary contains no port call, lane call, kernel call, operational private-store write or subsequent transition.
>
> No authoritative work-transition record is appended before this admission. Admission failure permits a truthful refusal record, not the proposed transition.
>
> The live runnable projection is not exposed before the required durable append succeeds. An append failure engages denial, retains recording uncertainty and installs no runnable projection. If ingress follows admission while append is in flight, the admitted record may finish, but the original session denies the resulting projection and every successor step.
>
> Settlement append paths validate the exact mission, effect, action, generation, reservation and existing settlement state before writing. An overtaken callback cannot append `EffectSettled`, `ActionApproved` or another event whose prerequisite was already cleared by terminal settlement. Late physical results may refine evidence through permitted settlement; they do not rewrite a terminal mission or make it runnable.
>
> Replay validity is checked for every generated durable prefix, including prefixes ending at each append/sync boundary.
>
> **A4. Final authority predicate and the publication observation point**
>
> A successful session CAS is necessary but not sufficient. Immediately before it, under the relevant owning hold, validate every prerequisite that applies to that sink: immutable owner/origin/target; mission generation and terminal intent; run and door liveness/cancellation; exact consent; commitment state and binding; policy generation and live grants; both applicable deadlines; and unchanged native watches.
>
> Facts copied before slow work are not final validation. Missing, poisoned or changed prerequisites refuse. A prerequisite that does not apply to a sink must be explicitly marked in the manifest; private cleanup does not acquire unrelated work authority.
>
> Registry cancellation/revocation and registry mutation retain their existing common-hold ordering. Outside those holds, the documented native/kernel handoff limits remain; the session CAS must not be described as atomically validating independent native authorities.
>
> The publication guarantee refers to the authoritative session observation performed by the existing publication observer, not to the later return of the whole kernel `final_check` function. Emergency ordered before that observation must be detected. Emergency ordered after that observation may fall within the documented one-publication window even if later native/grant checks have not returned.
>
> Record and test that interval explicitly. Keep native-watch and write-grant checks, and preserve all resulting interruption facts. No new kernel observer or K3 change is authorized.
>
> **A5. Command acceptance, targets and deadlines**
>
> Input acceptance captures an immutable context identifier in addition to the existing envelope fields. This is a bounded, memory-only observation of a previously published session context; the reader does not acquire broker/coordinator/resource locks.
>
> Approval is accepted only against an already published exact pending-action context. Undo is accepted only against an already published eligible undo-entry context. The context identifies the original mission generation, run, commitment or undo entry and its version. If no such context exists, or the target is unavailable because an operation owns it, the command has no target and must refuse. Dequeue may not bind it to a target created later.
>
> At execution, resolve that original context and revalidate it. Missing or changed context refuses without dialogue. Two queued approvals or undos cannot transfer the second request to a replacement target.
>
> Child steps inherit their command envelope. A separately accepted approval or cleanup command is a new root request with its own exact scope; a callback or continuation is not.
>
> Keep the existing dialogue-duration limits. Their absolute deadline begins at the original applicable request acceptance/reservation and is never restarted by queueing, lock wait, dialogue construction, rendering, input-boundary acknowledgement or retry. The File action additionally remains bounded by its original backend commitment deadlines.
>
> The existing acknowledged fresh-answer protocol remains mandatory. A fresh code does not refresh command authority.
>
> **A6. Total lifecycle and shutdown**
>
> Represent lifecycle separately from monotone emergency and fatal-failure restrictions. Emergency history and first terminal intent cannot be cleared by a change of displayed lifecycle.
>
> Every event has a defined result in Bootstrap, Normal, Emergency, Failed, Closing-normal, Closing-restricted, Sealed-finalizing, Closed-clean and Closed-unclean. Unlisted events refuse without changing authority; they are not implicit permission or an unreachable assertion.
>
> Bootstrap Quit enters closing immediately. A late initialization result is retained for controlled settlement/quarantine and cannot install readiness, a workspace or an operational handle. Stop during Bootstrap engages the original session; initialization failure preserves it.
>
> Normal Quit quiesces work and settles a nonterminal mission using an Owner-requested termination, preserving any earlier terminal intent. It withdraws the exact pending action before determining nonpublication. An executing or unreadable action remains pending/uncertain until conclusive evidence exists. Quit never invokes normal undo.
>
> Emergency during Closing-normal changes it to Closing-restricted. Fatal failure in any live or closing state engages denial and makes the exit nonzero. EOF and read error perform emergency ingress and retain their fatal-input/nonzero policy. Repeated Stop remains ingress until sealing.
>
> Capture the one three-second monotonic deadline at the first accepted shutdown trigger. All startup, input, work, Stop, settlement, cleanup, recording, rendering and join helpers use it.
>
> Before sealing, quiesce and acknowledge input and every event producer; join all work and Stop workers; drain the existing settlement obligations; and verify the inventory. The close executor and final output executor are explicitly accounted lifecycle operations, not forgotten work workers.
>
> Reserve the two end-record obligations and final delivery obligation before sealing. The pre-seal proof may exclude only those named final obligations. Seal atomically against ingress. After sealing, ingress handles return `SessionEnded` without changing generations or adding records.
>
> After sealing, only the reserved Mother end record, primary evidence end record and final delivery/finalization may complete. Record Mother’s end before primary evidence close. Any failure, timeout or uncertain result yields Closed-unclean and nonzero. A timed-out append may have completed; its status is unknown unless positively established.
>
> Closed-clean and zero exit require the complete proof, both durable end results, successful required output delivery and terminal finalization within the original deadline. Durable end records can exist even when final output/finalization fails; report those facts separately.
>
> All retained work/restoration handles refuse after sealing. No failed or partial close reopens the session.
>
> **A7. Exact acknowledgement and pruning**
>
> Before releasing a pin, retain a bounded session acknowledgement receipt containing the exact session, mission, effect, original door, run and commitment identity, the conclusive outcome and the verified durable Mother terminal-record reference.
>
> Unknown backend state is not first-ack success. A first acknowledgement requires the exact retained commitment and all specified conclusive/durable prerequisites. A duplicate may succeed without another release only when the session already holds the exact successful acknowledgement receipt. An unknown or mismatched request without that proof refuses and retains debt.
>
> Receipt installation, pin release and pruning must have no gap in which the last proof disappears. An interrupted transfer remains an explicit unresolved acknowledgement obligation.
>
> Receipts consume the pre-reserved bounded metadata budget and remain until sealing. Capacity pressure refuses a new action before creation; it does not evict acknowledgement proof or allocate an unbounded tombstone map.
>
> Restart does not reconstruct these receipts as executable acknowledgement capabilities.
>
> **A8. Cleanup eligibility, ownership and discharge**
>
> Session kernel objects use inventory-owned destructor-suppressed storage, with operation guards borrowing them. No session path takes a cleanup-capable raw object into an ordinary local owner. Final container, closure or channel destruction cannot expose its nested cleanup destructors.
>
> Normal explicit cleanup selects a fixed list of disposable, returned session entries. It excludes active preparations, pending approvals, borrowed/in-flight entries, the current offered undo entry, uncertain recovery material and retained Owner versions. Normal close may additionally retire an obsolete/current undo capability after work is quiescent, while preserving intentionally retained apply evidence. It does not restore or delete the published Owner file.
>
> Each retry and each staging discard is a distinct admitted call. Failure retains the current entry and unattempted tail. Owner-version recovery material is never classified as disposable cleanup.
>
> Presence, removal, durability, grant closure and recording are separate facts. `CleanupStatus::Discarded`, a void discard, an empty summary or a failed authority lookup is not a universal discharge proof.
>
> Read-only absence can establish current observed absence only through the expected identity-checked session-owned parent. It does not prove that the parent-directory synchronization succeeded or that removal is durable. Where the entered kernel call does not return that proof, retain `finalization_unverified`. A separately admitted normal housekeeping synchronization may establish durability; restricted close does not manufacture permission for it.
>
> Likewise, verify required cleanup records independently where the kernel operation records best-effort. A missing/uncertain required record remains recording debt even when removal succeeded. Grant closure requires an explicit conclusive revoked/expired/closed result; generic unreadability is not closure.
>
> Destructor paths perform no filesystem cleanup. Session-specific execution/grant guards transfer or mark their existing obligations rather than initiating hidden recording or cleanup. `RecordBatch` lost/uncertain accounting and failed reservation release remain visible in the inventory and prevent a false clean close.
>
> If ownership metadata is poisoned, storage remains retained without requiring successful lock reacquisition during Drop. Uncertainty prevents clean close.
>
> **A9. Stop event, worker and recording accounting**
>
> Give every accepted ingress one monotonic event identity. Its authoritative denial update occurs first. The subsequent worker’s native narrowing is correlated with that event and is not another session ingress or lifetime refresh.
>
> At most four Stop workers are active. Coalescing defers recording work; it does not declare the event recorded. Maintain bounded pending event/range accounting independent of work capacity. As worker slots become available, drain pending recording work within the original shutdown deadline.
>
> Each worker reports the exact event or pending range it covered, its native narrowing result, synchronous recording-phase result, and separately established durable/owed/uncertain status. A worker cannot cover an ingress accepted after its declared coverage boundary.
>
> If existing evidence schemas cannot represent a proposed aggregate conclusively, process individual pending events or retain uncovered recording debt. Do not extend the schemas or infer durable coverage from a memory receipt.
>
> Spawn failure, panic, poison, counter exhaustion and lost worker results retain explicit failed/uncertain obligations. A worker’s successful join is not proof that its body succeeded. A void/count-only legacy API is not sufficient evidence of recording success.
>
> The reporter starts before blocking initialization and owns its cursor from zero. Every event and outcome is either delivered once or retained as an explicitly undelivered obligation. Receipt-capacity exhaustion fails closed; it cannot silently advance the cursor.
>
> **A10. Independent model and non-vacuous verification**
>
> The model’s expected answers come from this specification and a literal, independently written table committed before production changes. It does not import or generate expectations from production transition tables, `SettleCause::every`, publication extraction, result strings, counters, reported inventory totals, or reported admission/handoff success.
>
> Primitive inputs are controlled fixture events, independently scheduled ordering, actual filesystem observations, verified native ledger records and independently verified journal/evidence records. Production return values and classifiers are outputs being checked, not facts that select the expected answer.
>
> Model all thirteen mission states: Draft, AwaitingConsent, Approved, Investigating, Building, AwaitingApproval, Verifying, Explaining, Done, Stopped, Denied, Failed and Blocked. Cross them with applicable causes, dispatch/wait state, conclusive/uncertain publication, pending terminal records, exact ack debt and retained resources. Include terminal states with outstanding obligations.
>
> Preserve ordinary denial, lane failure and verifier failure/missing-verdict dispositions. A generic fence cannot establish a different first cause. Publish exact terminal intent before exposing its generic halt representation. Later events preserve that intent and all independent physical/recording facts.
>
> Model confirmation as not asked, awaiting answer, explicit decline, affirmative, invalidated, or protocol failed. Model handoff and physical result independently. A callback returning false does not prove an explicit Owner decline.
>
> Every applicable W/R sink has a successful no-interference control through the same real route and available capacity, plus its denial/race controls. Blanket refusal must fail. Every exclusion names the sealed boundary and has a bypass plant; an unsupported host route also retains its applicable legacy positive control.
>
> Native recovery-required publication is exercised through the production extraction and journal path, both alone and together with interruption. Constructing the final publication enum is insufficient.
>
> Distinguish transient publication history, final target presence, rollback/compensation, recovery requirement and record durability. Final absence does not prove that no publication ever occurred. Permit only the documented internal compensation of an already-entered call.
>
> Check both ordinary `ActionApproved` and published settlement representations. Check every durable prefix for replay validity and every ownership transfer against independently observed resources.
>
> Add explicit plants for: blanket denial; copying production classification into the expected result; feeding a production classifier back as a primitive observation; omitting a terminal state/cause row; skipping the private-store error removal; accepting unknown ack as a duplicate; absence-only durable-finalization proof; uncovered coalesced ingress; and a transition append before admission. Each must fail its designated oracle.
>
> Existing bounded exploration remains bounded coverage, not a proof of every schedule. Expected answers are not edited after observing implementation results without a separately reviewed oracle amendment.
>
> **A11. Recovery readiness**
>
> Before R03/R04 or any grant, door, attachment, mission or work admission, produce the required report from all applicable sources: verified kernel ledger, Mother history and primary broker evidence/health. Newly absent stores are distinguished from unreadable, torn, poisoned or otherwise invalid existing stores.
>
> Mother-history inspection uses a report-only path. It does not issue a door, claim File eligibility, adopt a run, install consent, acknowledge a lost capability or run ordinary operational coordinator recovery merely to obtain the report.
>
> Supplement summary APIs with verified existing records where necessary to report known allocations, unfinished records and cleanup facts. An empty `analyze_apply` result is not proof that no staging or private finalization remains.
>
> Source failure or incomplete required reporting prevents readiness. Verifiable unresolved File/authority/recording obligations keep this Home report-only; neither another start nor a fresh command clears them. Conclusive historical missions do not restore authority. Fresh work still requires the existing independent fresh-session checks, a newly selected empty workspace and new consent.
>
> Report facts absent from durable evidence as unavailable, without inventing their completion or precise previous cause. Manual external changes are observations, not Mother-certified cleanup.
>
> **A12. Result envelopes and exact notices**
>
> Every operation result retains independently: actual answer state; request/target identity; admission and handoff facts; physical outcome and certainty; interruption set and first terminal intent; grant closure; recording status per store; cleanup/finalization debt; and acknowledgement status.
>
> Rendering selects text from those facts. Generic success/failure, a callback boolean or an ingress epoch alone cannot select a stronger statement.
>
> Replace the unconditional unclean-exit notice with:
>
> “UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.”
>
> For partial close, additionally state which end record is established Recorded, Failed or Unknown. Never say no clean-close record exists when its write was in flight or a store already recorded it.
>
> `ACTION DENIED` is emitted only for an established explicit decline with the corresponding conclusive outcome consumed by Mother. Otherwise emit `ACTION REFUSED` or `ACTION OUTCOME UNCERTAIN`, with the controlled reason and recording status. Do not describe automatic cancellation or authorization failure as an Owner decline.
>
> “Undo was not confirmed” covers no affirmative answer; it must not imply an explicit decline. “Restore completed” requires conclusive physical completion, even if grant closure or recording separately failed. Unknown physical result uses the uncertainty message. Cleanup is requested, never “confirmed,” and reported totals cover the complete inventory with explicit excluded/in-flight/unknown categories.
>
> Keep ingress receipt, corroborated session/native halt, synchronous recording-phase completion and durable recording separate. `SESSION CLOSED recorded` requires the A6 proof and both durable end results. Failure to deliver/finalize after those results yields nonzero without denying that the records exist.
>
> Before production changes, commit this amendment, the expanded route manifest, literal expected tables, positive controls and plants. Add the new routes and boundaries to every generation, process-death and ownership-transfer matrix.
>
> Local warning-denied Clippy covers `nexus-kernel`, `nexus-governed-control`, `nexus-mother`, `nexus-mother-host` and `nexus-mother-desktop`, with all targets/features and the lockfile fixed. Preserve the other §6.6 gates. Freeze the existing 23 desktop live-test names before implementation; discovery/count alone cannot replace their individual receipts. Required CI remains the exact-SHA jobs named in §6.6, with no workflow change.

These are the remaining material builder choices and the frozen answers supplied above:

| Choice | Frozen answer |
|---|---|
| Session creation | One original broker-issued identity/cell before input; no copied or replacement cell |
| Authority source | Broker session state; presentation flags and counters grant nothing |
| Close permission | Separate cleanup-only capability bound to Quit’s original generation and reserved resources |
| CAS retry | Revalidate the original predicate; never refresh authority |
| Coordinator append | One admitted record-before-projection transition; no downstream operation bundled inside |
| Private-store work | Explicit operational mutation sinks; error removal requires separate permission |
| Kernel-call granularity | The exact entered-call ledger in A2; each subsequent broker call separately admitted |
| Publication boundary | Exact session observation inside the existing observer, with the remaining native-check interval disclosed |
| Prerequisite freshness | Final applicable authority predicate under the owning hold; session CAS alone is insufficient |
| Queued approve/undo target | Original already-published exact context; absent/busy/changed target refuses |
| Dialogue deadline | Original absolute deadline; queueing and rendering consume it |
| Held resume/adoption | Unavailable through Hands in every state |
| Normal Quit during work | Quiesce, withdraw and truthfully settle; no automatic undo |
| EOF/read/fatal error | Denial first, restricted bounded shutdown, nonzero |
| Post-seal activity | Reserved final obligations only; ingress returns `SessionEnded` |
| Resource storage | Inventory-owned destructor-suppressed kernel objects; workers borrow |
| Cleanup selection | Fixed disposable returned-entry list; preserve active, current undo and recovery/Owner material |
| Cleanup discharge | Separate presence, durability, authority and recording proofs |
| Ack after pruning | Exact retained successful receipt or refusal; unknown is not success |
| Coalesced Stop | Pending recording obligation, never implicit completion |
| Failure propagation | Typed operational/recording uncertainty; no count-only or successful-join substitution |
| Restart readiness | All applicable sources checked before work; no operational replay/adoption to obtain a report |
| Expected outcomes | Independent literal tables plus real-route successful controls and primitive observations |
| Terminal causes | Exact first intent preserved; no generic Fence substitution |
| Owner messages | Selected from independent facts; partial/unknown recording stated explicitly |
| Capacities and legacy limits | Existing limits plus the specified 256-entry/four-worker bounds; no unresolved eviction |
| Test exclusions | Checked-in structural reason and bypass plant; legacy positive control where applicable |
| Verification | Five named Clippy crates, workspace/no-fail-fast, mandatory PTY/crash/plants, 23 frozen live receipts, exact-SHA CI |
| Acceptance | Separate Owner physical acceptance and security-core approvals; no merge or phase advancement implied |
