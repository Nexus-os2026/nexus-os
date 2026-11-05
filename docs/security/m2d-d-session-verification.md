# M2d-D session verification and relocation map

This is implementation evidence for the frozen session specification, amendment B
and Ruling R-1. It does not change their authority or the kernel's contract.
The independently specified model and baseline receipts live under
`mother/host/tests/session/`. A planned schedule key is not an executed test.
The builder's PR update records actual executions, unsuccessful experiments,
restored controls, and the exact candidate used by each gate.

## Route relocation

`B` is `crates/nexus-governed-control/src`, `M` is `mother/src`, and `H` is
`mother/host/src`. The complete original sink vocabulary remains in
`sinks.tsv`; `exclusions.tsv` identifies removed routes and their legacy controls.

| Route | Final implementation boundary |
|---|---|
| R01 | `B/host_session.rs`: `SessionSeed`, original `Core`, one `HostSession` attachment. `H/session_home.rs`: session services opened only with that seed. Input and the Stop reporter start before Home initialization. |
| R02 | `H/session_home.rs`, `B/host_session/recovery.rs`, `M/adapter/private_file.rs`: fixed Home stores, report-only Mother history, verified native records and primary health. Initialization is lifecycle accounting, not operational content permission. |
| R03 | `B/files/mod.rs`: `set_hold_with_halt`, startup scope, final session admission before storing the bounded hold. |
| R04 | `Kernel::register` separately admits project selection; `Files::register_with_halt` admits workspace installation under its owning hold after identity validation. |
| R05 | `B/authority/policy.rs`: `GrantStore::request_with_halt` separates dialogue and issue record; `Room::fill` separately admits grant installation. |
| R06 | `B/governed.rs`: `issue_agent_door_with_halt`; `B/authority/run.rs`: `activate_door_with_halt`. The door carries its original session at issuance. |
| R07 | `B/authority/file_session.rs`: `FileSessions::port`, `FilePort::attach`, `FileSessions::claim`; original port/attachment identity is retained. |
| R08 | `M/coordinator.rs`: `Inner::record`, consent and phase reservation, paused-rest storage, and exact transition projection. Every work append is admitted before append/sync under the coordinator hold; its projection belongs to that one transition. |
| R09 | `AgentDoor::confirm_mission`, `RunRegistry::consent_slot`, `ConsentSlot::fill`. Held confirmation and resume remain unavailable through `SessionPort`. |
| R10 | `B/authority/run.rs`: final insertion under the run table hold. Original origin/cell precedes exposure; File provenance and preparation ownership prevent pruning gaps. |
| R11 | `B/host_session.rs` and `M/adapter/session_port.rs`: closed File operations; no adoption, membership growth or executable replay capability. |
| R12 | `M/coordinator.rs`: phase reservation, `LaneInvoked` append and the separate `LaneInvocation` admission immediately before the actual lane callback. |
| R13 | `RunRegistry::admit_files`, `SessionDoor::admit_files`, and the session port's bounded projection. Exact consent/workspace/backend run remain the authority. |
| R14 | `SessionDoor::create_file_effect` accepts the closed `FileCreate` request. The port does not expose generic `Intent`, network, process, connector, spend or display operations. |
| R15 | `Files::prepare_session` and `Shared::reserve`: reserve bounded action, resource and run-retention ownership before native grants/staging. Validation never converts a missing run to a legacy path. |
| R16 | `RunResource::grant` and the constructor in Files preparation: independent read-grant and native constructor handoffs. |
| R17 | `CodingRun::mutate`: each staging grant, snapshot, edit, structural verification, page verification and review has its own final admission and native borrow. |
| R18 | `CodingRun`, `NativeBorrow` and `FilesEffect` dispositions retain the same inventory entry on error, refused display, unwind and failed proposal. No removal is implicit in those transfers. |
| R19 | `CommitmentRegistry::prepare_reserved`, final run-retaining check and `Control::Place::fill`: commitment creation and pending insertion are separate operations. Failed insertion leaves exact outcome/pin/resource debt. |
| R20 | Native commitment approval/token issuance, authorization and `complete` validation retain the separately accepted approval request. The terminal's per-question answer history does not select stronger outcomes. |
| R21 | `Control::execute`: borrowing pending ownership is settlement. Native revalidation review, commitment start and invocation are separately checked. `ExecutionGuard` unwind retains unrecorded outcome debt. |
| R22 | `CodingRun` approval bridge, inert-page recheck, write-grant issuance and observed apply. The existing `Observer` remains the publication observation point. |
| R23 | `Shared::applied` replaces a non-owning undo index. The prior `RunResource` remains owned; replacement itself does not clean it. |
| R24 | Session deny/cancel/halt/finish/prune/revoke/withdraw perform narrowing and settlement. Their File dispositions retain resources; the legacy drain cannot consume host inventory. |
| R25 | `M/coordinator/lite_settle.rs` and `B/host_session/receipt.rs`: exact physical outcome, first terminal intent, D3 withdrawal before snapshot, durable Mother terminal reference, bounded retained acknowledgement receipt, then pin release. |
| R26 | `H/stop.rs`, `B/host_session/stop.rs`: original memory-only ingress, exact event identity, at most four owned workers, bounded pending debt and independently acknowledged event/outcome delivery. |
| R27 | `HostSession::undo_confirmed`, versioned `UndoClaim`, `Files::undo_one`: original target before dialogue, native watches retained, distinct request/undo-grant/restore handoffs, explicit grant closure. |
| R28 | No inherited session undo drain. Explicit cleanup or the original normal-close capability selects eligible entries independently. |
| R29 | `Files::cleanup_host` snapshots eligible inventory entries. Each native retry/discard is distinct; `RunResource::prove_finalization` separately verifies absence, native records, authority closure and admitted parent sync. |
| R30 | Host `retire`, `keep`, proposal/settlement return and drop transfer or mark ownership. They perform no implicit filesystem removal. |
| R31 | `CommandKind::Unavailable` and closed session APIs: lift is always unavailable without dialogue. |
| R32 | No session inference binding or model-attempt capability; configured legacy inference tests remain outside this surface. |
| R33 | `HostSession` read-only projections, `operation_observation`, `file_working_detail`, Mother `terminal_facts`. Detail is normal-state, exact-action, take-once Owner data. Status/evidence remain available under denial. |
| R34 | `B/host_session/close.rs`, `M/coordinator/session_close.rs`, `H/hands.rs`: original Quit capability, producer joins, complete debt proof, atomic seal, Mother end, primary end, final delivery/modes. |
| R35 | Inventory-owned `ManuallyDrop<NativeRun>`, borrowing guards, retained thread/channel payloads and controlled quarantine. Unverified bounded exit never exposes cleanup-capable destructors. |
| R36 | `H/terminal.rs`: original immutable request/target/deadline at acceptance, one combined 64-entry input budget, exact dequeue target, no later rebinding or deadline refresh. |
| R37 | `M/adapter/private_file.rs`: reserved private obligation, directory/temp creation, write, file sync, rename, parent sync and independently admitted error removal. Intended retained blobs and failed temporary cleanup remain different facts. |

## Entered native calls

Every row below is a separate broker handoff. Arguments are backend-owned values,
never reconstructed from displayed strings. The per-request call ledger retains
entered, returned and failed counts independently. A missing return is unknown.
The unchanged native method may complete its documented internal allocation,
recording, compensation and cleanup; its return permits no successor call.

| Call | Exact inputs and ownership | Returned evidence and remaining obligation |
|---|---|---|
| Project selection | Canonical selected root, fixed exclusions, native project registry | Native project identity; later broker installation is separately admitted. |
| Read grant | Selected project ID and exact agent/run binding | Reserved grant ID/registry/binding remains owned if construction fails. |
| Native constructor | Verified ledger, selected project, original scopes and Stop observation | Native run stored directly in destructor-suppressed inventory; no ordinary local owner. |
| Staging grant | Original native run and read grant | Staging allocation and native grant records; staging/grant closure remain separate. |
| Snapshot | Original native run and selected paths | Native snapshot record; no permission for edit. |
| Edit | Exact create name and reviewed bytes | Native candidate mutation/record; later verification is separate. |
| Structural verification | Same candidate and native run | Verification result/records; failure retains ownership. |
| Page verification | Same candidate and consent-bound page facts | Native inert-page result and recording; no new effect type. |
| Review | Same native candidate | Verified review hash and native review record. |
| Revalidation review | Exact pending effect's same native candidate | May mutate verification and append review records; it is not a getter. |
| Approval bridge | Native project name, exact reviewed hash and broker approval bridge | Includes its internal native review and approval record; page recheck is separate. |
| Page recheck | Exact previously verified page and current consent facts | Native recheck result; write grant/apply are separate. |
| Write grant | Selected project and exact original binding | Reserved authority closure obligation survives errors. |
| Observed apply | Exact native approval, write grant, staging store and existing observer | Physical publication, interruption/recovery and ledger outcome remain independent; an error does not prove absence. |
| Restore request | Exact retained undo entry, native project and Owner bridge | Original watches and actual Owner answer; no refreshed request. |
| Undo grant | Original target, original request and exact workspace binding | Its own pre-reserved grant obligation; closure is explicit. |
| Restore | Exact native restore approval and separately issued write grant | Completed/refused/uncertain physical outcome, independently reported grant closure and records. |
| Retry cleanup | Native-owned temporary identities from the same retained entry | Internal item removals and final native record; failure retains debt/tail. A following discard requires new admission. |
| Staging discard | Exact inventory-owned native staging object | Observed absence alone does not prove durable removal or required records. |
| Cancel | Exact owned native run | Narrowing/settlement only; cannot authorize cleanup or new work. |
| Parent finalization sync | Already-owned, identity-checked parent descriptor | Separately admitted normal housekeeping establishes parent durability; unavailable in restricted close. |

The apply guarantee ends at the existing observer's authoritative session read.
A later emergency may fall inside the remaining native-check/publication window.
No new kernel observer is added and removing emergency undo does not close that
window. Restore has its original native watches and the same entered-call limit.

## Ownership and close evidence

| Owner or transfer | Required retained fact |
|---|---|
| `RunResource` / `CodingRun` references | One inventory owner; stack/slot/effect handles only borrow or reference it. |
| `NativeBorrow` | In-flight native object remains retained if its owning lock is poisoned. Unwind marks uncertainty before release. |
| `ProjectReadLease`, write grant, `IssuedGrant` | Exact registry/ID/binding and explicit closure; failure is not successful revocation. |
| Native `CodingRun`, `StagingLease`, `OwnedTemp` | Nested cleanup-capable destructors remain suppressed outside entered calls, including final-owner and rejected-thread payload destruction. |
| `Place`, preparation slot, `Reserved`, `ConsentSlot` | Reserved bounded capacity is returned only by its owned transfer; poison/lost transfer remains observable. |
| `ExecutionGuard`, `RecordBatch` | Actual outcome/end record or owed/unknown debt; destructor cannot claim successful recording. |
| Private obligation | Exact expected temporary and observed identity, intentional retained blob, required sync/recording status. |
| Pending action and acknowledgement | Session/mission/effect/door/run/commitment, conclusive physical result, verified durable Mother reference and idempotent successful release receipt. |
| Undo index | Versioned non-owning target; replacement cannot destroy the previous native owner, and a queued command cannot adopt a new version. |
| Worker, channel and finalizer | Owned producer identity, actual join/body result, pending event/delivery debt, or retained quarantine. |

Clean close requires every ordinary producer quiescent and joined, all exact
settlement/acknowledgement and record obligations resolved, all resource facts
established, and only the three reserved final obligations outstanding at seal.
Mother's durable end precedes primary close. Delivery and terminal finalization
remain separate success requirements. A producer/append subdeadline leaves time
inside the original three-second budget to report a truthful unclean result;
it never renews the deadline or declares an unjoined worker finished.

The final builder report separates model products, executed real-route cases,
process deaths, structural plants and behavioral plants. Every unsuccessful
mutation experiment remains disclosed and contributes zero to detection counts.
Independent review, exact-SHA CI, and Owner physical acceptance follow the local
builder result; none is inferred from it.

## Concrete verification index

`mother/host/tests/session/sink-verification.tsv` accounts for every one of the
75 registered W/R entries by final implementation boundary, real-route oracle
and plant receipt. The two historical implicit-cleanup entries are retained
with explicit removal/conservation evidence. `exclusions.tsv` contains 17
closed or removed routes and their applicable legacy positive controls.
`executed-final-boundary-baseline.json` contains the actual late audit rows:
one door-confirmation CAS gap, twelve adapter projection/dispatch violations,
and six reservation violations were observed before their repairs. Overlapping
setup runs are not added together as distinct coverage.

`executed-plants.json` indexes 189 detected independent mutation executions.
It excludes failed compile, filter and fixture attempts; corrected attempts and
all original diagnostic logs remain in the builder scratch archive. Some
mechanisms were deliberately re-exercised by stronger final-boundary oracles.
The index does not claim 189 distinct implementation mechanisms. Every planted
source was restored before checkpointing; no mutation is part of the candidate.

| Ownership boundary | Behavioral conservation or exact-release evidence | Detected plant family |
|---|---|---|
| Native inventory / resource reservation / preparation retention | Actual staged identities and contents survive refusal, unwind, pruning and final-owner drop; the 257th native allocation is refused before allocation | `omit_native_inventory_transfer`, `omit_resource_reservation_transfer`, `omit_preparation_run_retention`, `prune_precommitment_origin` |
| Native borrow and nested `CodingRun` / `StagingLease` / `OwnedTemp` | Poison/unwind retains the inventory owner; failed native restore leaves an actual temporary whose inode/content survives last-owner drop | native owner destructor, old undo owner drop, `ownedtemp_destructor` |
| Read/write/undo grant obligations and `IssuedGrant` | Native grant handoff, explicit closure or uncertainty, retained resource facts and no clean discharge without closure | undo-grant bypass; erased grant-closure fact; absence-only finalization |
| `Place` and `PreparationSlot` | Two simultaneous compound reservations; transferring or dropping A preserves B, failure returns only its own capacity, and released capacity is usable | both transfer-twice and both omitted-return plants |
| `ConsentSlot` and transition `Reserved` | Real consent permits native binding; original admission still precedes cached consent and approval/start completion | consent-cache omission; transition-reservation omission; final registry bypasses |
| Commitment pin and exact acknowledgement | Pending/uncertain/undurable/unknown acknowledgements refuse. A's terminal cannot acknowledge B. Distinct durable references survive duplicate exact acks; both native preparation pins still discharge during close | commitment-pin omission; unknown/undurable/wrong-terminal ack; double native release |
| `ExecutionGuard` and `RecordBatch` | Unwind cannot write a fictitious successful end; native pending/lost/uncertain records independently block clean close | execution-guard recording-on-drop; native record-debt omission |
| Private-store obligation | Actual per-API calls and observed temporary identity; uncertain finalization stays owed independently of intentional retention | seven private sink plants; resource omission; absence-only durability |
| Worker/result/channel/rejected closure | Owned joins, bounded worker slots, exact event and result delivery, retained spawn/panic/lost-result debt and finalizer quarantine | Stop, continuation, delivery and close plant families |
| Undo index / queued target / continuation | Non-owning versioned target; busy borrow retires it, return gets a new version, old queued approval cannot act on B | old undo drop; borrowed/returned target reuse; duplicate/old-context continuation |

The 153,000-key schedule enumeration is a planning cross-product, not an
execution count. Reported execution counts distinguish physical fixtures,
process deaths, parent/child controls, literal model products and structural
guards. Final-HEAD workspace/host/crash/desktop gate receipts are recorded by
the builder outside this document so their commit identity remains exact.
