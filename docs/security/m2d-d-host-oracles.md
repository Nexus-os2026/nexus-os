# M2d-D session host protocol and oracle registration

The frozen [session specification](m2d-d-session-spec.md), including binding
amendment B, governs Hands. [Oracle amendments](m2d-d-session-oracle-amendments.md)
quote superseded host expectations. The historical registrations below explain
provenance; they do not grant a current Hands recovery or resume operation.

## Current protocol

`mother-hands --home ABSOLUTE_HOME --workspace ABSOLUTE_EMPTY_FOLDER
--name NAME.txt --text TEXT [--hold-before-publish SECONDS]` is the deterministic
Linux candidate. It has no model, shell, network, preview or content-file reader.
All four selections are required; the existing broker validates exact content,
name, storage and workspace. The hold is 0–30 seconds and is set before work.
Both input and output must be terminals. Process state is non-dumpable before
Home opens. The entire Home is excluded from the selected workspace.

One broker-issued identity and original deny-only cell exist before input.
Bootstrap attaches to that identity. Kernel ledger, Mother history and primary
evidence/health are inspected before hold configuration, workspace selection,
grants, doors or mission admission. Missing new stores differ from invalid
existing stores. Unresolved obligations prevent readiness; restart is report-only.
No report reconstructs consent, workspace permission, undo or an ack capability.

| Command | Current behavior |
|---|---|
| `run` | One File mission: Files grant, original door and mission consent; deterministic lanes to the exact pending action. |
| `approve` | The pending target is captured at input acceptance. Native approval and execution retain that request; Mother continuation is single-use and separately admitted. |
| `undo` | Normal operation only: capture the exact eligible target before dialogue, then fresh native confirmation. Missing, busy, consumed or changed targets refuse. |
| `cleanup` | Normal operation only: explicitly request a fixed list of returned disposable entries; no implicit drain. Preserve active, borrowed, current-undo and recovery/Owner material. |
| `detail` | Normal operation only: consume bounded Owner-only working detail. |
| `status`, `evidence` | Observe session/native halt, recording and resource obligations. Recent evidence is bounded and is not a complete chain export. |
| `stop` | Original-cell denial first; one event identity, recording off the reader. Repeated Stop remains ingress until seal. |
| `lift-for-undo` | Always unavailable, without dialogue or native resume. |
| `quit` | Quiesce and truthfully settle, then prove close; never invoke undo. |

Emergency permits only status, evidence and Quit. It admits no new work,
restoration or cleanup. An already-entered kernel call may finish its documented
internal work, including compensation and internal cleanup. Every subsequent
broker call requires its own admission. The existing publication observer
remains; no new kernel observer is claimed. Removing emergency undo does not
close the native handoff window.

## Input, output and result facts

Commands retain their original sequence, identity, target, generation and
absolute deadline through queueing, validation, dialogue and callbacks.
Door/grant/undo limits are 120 seconds from original command acceptance;
mission/action limits are 300 seconds, with the action also bounded by both
native commitment clocks. A fresh code never refreshes command authority.

One dialogue owns the terminal. Complete write and flush precede reader
freshness acknowledgement and `INPUT READY`. Buffered complete answers and
partial lines are invalidated; CR, LF and CRLF each submit once. Stop, Quit and
EOF remain active during this boundary. Codes are eight lowercase hex digits,
checked against the fixed escaped display with a bounded collision budget.

Only an actual `no` is a decline. Wrong/invalid answers are protocol failures;
interruption is invalidation, never proof of Owner denial. `ACTION DENIED`
requires the conclusive declined outcome consumed by Mother. Physical restore,
handoff, grant closure, interruption and recording remain independent facts.
Cleanup is requested, never described as confirmed. Inventory output includes
excluded/in-flight/unknown entries and independent finalization/recording debt.

Output escapes untrusted values and preserves complete fenced approval facts.
The isolated legacy walk keeps its original three dialogues; it is not part of
the Hands authority graph. Source guards pin both exact confirmer paths and
method bodies and prohibit importing legacy/raw authority into Hands services.

## Stop and close ownership

The reader does no broker, coordinator, output or filesystem work for Stop.
At most four owned Stop workers record exact events. A fifth remains pending;
coalescing does not establish recording. Every event/outcome is delivered or
retained as an undelivered obligation. Spawn, panic, poison, overflow and lost
results remain failures. Join success does not prove body or record success.

`STOP REQUEST RECEIVED`, corroborated native halt, synchronous recording-phase
completion and durable/owed/uncertain recording are separate receipts.

The first shutdown trigger captures one three-second monotonic deadline.
Input, startup, work and Stop producers must quiesce and join; settlement,
resource, grant, recording and exact-ack debts must be resolved. Normal close
may retire undo authority without removing the published Owner file. Restricted
close cannot authorize new cleanup. Destructor-suppressed inventory retains
resources on error, panic, poison, rejected spawn and failed transfer.

Reserve Mother end, primary end and final delivery before the seal CAS. After
sealing only those reserved operations may finish; ingress returns
`SessionEnded`. Mother end precedes primary end. Terminal modes are restored
before the successful receipt; delivery and renderer join must finish within
the original deadline for zero exit. Part of that same budget is reserved for
partial-close reporting and finalization. A timed-out append remains Unknown.
Both end records may be durable even when final delivery/finalization fails.

The exact emergency, uncertainty, restart and amended unclean notices are in
the specification and executable protocol tests. Partial close reports each
end as Recorded, Failed or Unknown (including a not-attempted distinction).
No promise survives process exit.

## Verification and scope

The route catalogue is `mother/host/tests/session/sinks.tsv`; structural
exclusions name sealed boundaries, legacy positive controls and bypass plants.
Literal expected tables are independently fingerprinted. They do not consume
production classifications as primitive observations. Native ledgers, actual
resource identities/content, journal prefixes and process-death receipts check
implementation behavior. Planned matrix keys and model-only products are
reported separately from executed schedules in the PR evidence.

Mandatory coverage includes successful controls and unscheduled-worker denial
at every applicable sink; before/after admission and ownership transfers;
capacity, poison, exact acknowledgement, all thirteen mission states, native
recovery-required publication, normal/emergency close, repeated restart and
real PTY/storage stalls. Every plant is red, restored and never committed.
No meaningful failing test or uncertainty can be waived into a green gate.

Final local gates: fmt; five named crates with all-target/all-feature locked
Clippy and warnings denied; workspace tests with no fail-fast; host/PTY/crash,
privacy/source guards; 23 frozen named desktop live receipts; diff check;
lockfile unchanged from `03e07b1a`; kernel/workflows unchanged from `d20bc1f6`.
The Architect owns push, exact-SHA CI and PR updates. Independent security
review and Owner installed-binary physical acceptance remain separate.
See [Run Hands](../../mother/README.md#run-hands-m2d-d-candidate-linux-only).
No H1/C-11/capability acceptance, portable support, merge or phase advancement
is implied.

## Historical registrations

The following describes earlier checkpoints. Where it offers host emergency
resume, cleanup or fresh undo, the current specification above supersedes it.
The corresponding legacy controls remain valid outside Hands.

## Review 2 — Decision 6 (registered on 6ba3d000)

The worker-unscheduled ingress schedules use the real terminal Stop route
before draft and between draft/submit. Both must refuse, with zero backend
runs or model calls. Broker schedules engage the original memory-only File
cell while workspace validation is held, between validation and acquiring the
binding hold, and immediately before File/inference binding, run insertion,
and model-attempt admission inside the hold. No recording worker runs. The
broker's own table counts must be unchanged; no Files preparation or
commitment is allowed. Positive controls increment every backend count.

Startup is held at Home opening and recovery reporting. Two ingress events
and two worker outcomes each get exactly one receipt, including two recording
panics before startup finishes. Receipts must appear before releasing startup.
The renderer oracle interprets wire CR/LF cursor movement and requires column
zero for every row, with an LF-only negative control.

Plants (one at a time, uncommitted): backend engagement delayed to the worker;
after-validation re-check removed; host-only gate; startup-dependent receipt
reporting/event absorption; LF-only renderer. Broker checks already present
before this repair remain covered by the validation schedules and legacy tests.

### Decisions 7–8: session cell on every host admission (fa8fa1f5)

`files_host_decision8_tests.rs` freezes the saved baseline-sweep schedules.
No Stop worker runs: each test observes the engaged backend cell with both
native Stop snapshots still unset. Counts come from real door/run/grant and
commitment registries and the actual Files staging/execution hooks.

- Door: ingress at `ISSUE_RECORDED`, before dialogue, and ordinary run insertion
  before File selection. Every host door and its runs carry the original cell.
- Workspace: ingress at entry and after validation before installation.
- Files grant: ingress at entry, in the native answer and in `GrantIssued`
  evidence before insertion.
- Files preparation: ingress after membership and after filesystem validation
  before staging; after `Prepared` before commitment insertion.
- Approval/authorization/dispatch: final grant validation after the initial run
  check; retain the existing `AUTHORIZE_RECORDED` control.
- Mission/held consent: `CONSENT_BEFORE_FILL`; File topology/selected routes;
  Decision 6 run, File/inference binding and model-attempt counters unchanged.
- Controls: live host-cell door → File selection → create; legacy no-cell
  registration/grant and legacy new door after native resume. Frozen halt tests
  are not amended.

Decision 8 supersedes the uncommitted probe's retroactive evidence expectation:
`AgentDoorOpened` already written before `ISSUE_RECORDED` stays; refusal appends
`AgentDoorDeclined` with `door_activation_refused_halted`. Grant insertion,
authorization and execution refusal also require route-specific closed codes.
No refusal record claims the effect succeeded.

Baseline API adapters are temporary test-only forwarding shims to the unchanged
legacy APIs, dropping the new optional argument solely to run these probes on
fa8fa1f5; they are never committed. Red: 14 failures, 10 passing controls,
`decision8/red-fa8fa1f5.log`. The production changes follow this oracle commit.
Plants remove each route's final re-check, drop the door cell, and retain only
the pre-dialogue door check. All plants must be red and never committed.

Fresh sweep supplement (registered before the corresponding repairs): native
approval token issuance is separately counted by its returned token at final
grant validation. Consent/held-consent cache and prepared-commitment refusals
must append their closed route code after earlier evidence. Five additional
oracles are red with fa8fa1f5 enforcement plus only the registered test hooks
(`decision8/red-fresh-fa8fa1f5.log`); implementation files were temporarily
replaced from the oracle checkpoint for that run and restored byte-for-byte.

The complete sweep additionally covers replay adoption before File selection
and pending-effect installation after commitment preparation. Adoption uses
independent live/refusal controls, ingress after the commitment view and at
final membership insertion, and a retained token after a successful adoption.
Pending installation uses `PLACE_BEFORE_FILL` and the post-validation hook in
the pending-table hold, with real `pending_held`/`pending_places` counters.
Baseline enforcement gives 3 adoption failures and 2 pending-install failures
(`red-adoption-fa8fa1f5.log`, `red-pending-fa8fa1f5.log`). These are the same
optional-cell propagation/final-check rule; legacy no-cell routes stay intact.

## Review 3 / Decision 9 (registered on 01643b19)

- Hold legacy File execution at `FILE_EXECUTE_START`, after its guard captured
  `None`; attempt host adoption; engage only the session cell, no worker.
- Start B on a legacy run with commitment A; at `PREPARE_BEFORE_INSERT` adopt
  through A; at `PLACE_BEFORE_FILL` engage only the session cell, no worker.
- Both schedules require typed `NotMember`, unchanged membership/cell, and no
  continuation of the refused takeover. The fixture's original legacy owner
  cancels its own work after refusal to drain the held operation. This cleanup
  is explicit: the host cell is NOT claimed to stop an unrelated legacy run.
  On the vulnerable baseline adoption succeeds, so this cleanup is not taken:
  the actual publication and late pending installation remain observable.
- Legacy adoption and publication remain live; original door/same cell join is
  idempotent; original door with a newly attached cell refuses an old run;
  other doors refuse even with the same cell, and with a different cell.
- Plants: allow host adoption of a cell-less run; allow a different-cell join.
- Real walk PTY wire: before and after its first dialogue, every progress row
  starts in column zero under the renderer's CR/LF cursor model. LF-only walk
  output is the negative plant.
