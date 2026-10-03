# P2-V1-R3B-I4-R1: complete module self-review (mission section 21)

Reread after the focused tests passed: `scope.rs`, `scope/manager.rs`,
`scope/native.rs`, `scope/pending.rs`, `execution.rs`, `launcher.rs`,
`fault.rs`, the live harness's callers, the desktop's guards and the
security document. The searches and their complete output are in
`validation/review-searches.txt` (`scripts/review_searches.sh`, read-only);
each is interpreted here. Citations are to
`crates/nexus-verifier-sandbox/src/`.

## The section 21 hazards

| # | Hazard | Search | Finding |
|---|---|---|---|
| R1 | public pending ownership without the helper | declarations and re-exports of the operation, failure types, settling | `PendingScope` is `pub(crate)` (`scope/pending.rs:90`) and re-exported only to the crate (`scope.rs:65`); `reconcile` is `pub(crate)` (`scope/pending.rs:164`); `StartFailed`, `settle`, `into_pending` are gone; `ScopedHelper`, `PlacementFailed` and `place` are harness-gated and own the helper with the operation. None found. |
| R2 | public alternate manager socket | manager and connection constructors | `connect` derives the bus from the real uid (`scope.rs:141-145`); `connect_at` is harness-gated (`scope.rs:150-152`); `connect_to` is private (`scope.rs:156-164`); `ZbusManager::connect_at` is crate-private, in a private module; one `Builder::address`. None found in a normal build (N-I4R1-CONNECT-AT). |
| R3 | public direct start weaker than `run` | every start of an operation | starts happen only in `attempt` (`execution.rs:633-637`) and in the harness owner (`execution.rs:290-291`); both store the operation beside the helper first. None found. |
| R4 | `resume_unwind` while pending | every `catch_unwind` and `resume_unwind` | no `resume_unwind`; `catch_unwind` in `retry` (`execution.rs:186-196`), `place` (`execution.rs:266`), `ScopedHelper::end` (`execution.rs:320-330`), `execute` (`execution.rs:559`), `finalize` (`execution.rs:694-697`); each keeps or retains its owner. None found. |
| R5 | unit-name-only authority | every use of the unit name | the name is the request's locator, GetUnit's and StopUnit's argument, the `names_unit` filter and the `Id` comparison; it never proves, settles or opens anything by itself (the candidate comes from the kernel's report of the bound helper; the proof needs the binding). None found. |
| R6 | basename-only binding | every comparison of a path, id or name | `id == self.unit` and `group == path`, byte for byte (`scope/pending.rs:340-349`); `names_unit` compares the last component only to locate a candidate, and requires a normal absolute path (`scope/pending.rs:420-425`); no canonicalization. None found. |
| R7 | GetUnit absence after an uncertain start | absence, acceptance, uncertainty | `accepted` is set only in the delivered arm (`scope/pending.rs:270-273`); `gone` consults `absent` only when accepted (`scope/pending.rs:239-240`); the uncertain arm consults nothing (`scope/pending.rs:284`). None found. |
| R8 | StopUnit reply as proof | every StopUnit outcome | stored in `last_stop` (`scope/pending.rs:192`), read only by `Debug`; never by a decision. None found. |
| R9 | helper reaped before its operation is confirmed | every reap | `end` reaps only after `reconcile` succeeded or for a proven or absent scope (`execution.rs:736-753`); `end_now` reaps only when nothing is unresolved (`execution.rs:770-772`); `reap_helper` only while proven (`execution.rs:308-316`). None found. |
| R10 | serial wrap; spawn before identity | the allocation and the spawn | `checked_add` with `fetch_update`, refusing at exhaustion (`launcher.rs:192-197`); allocated before the sockets, pipes and `spawn()` (`launcher.rs:220`, `launcher.rs:230`); no `fetch_add`. None found. |
| R11 | candidate dropped on a later proof error | every assignment and take of the candidate | inserted at once (`scope/pending.rs:311`) or acquired (`scope/pending.rs:212-227`); taken only on success (`scope/pending.rs:495`); never cleared. None found. |
| R12 | launch before the ControlGroup proof | proof steps; the launch guard | `get_unit`, `unit_id`, `control_group`, then the properties (`scope/pending.rs:327-363`); the handshake only after `is_proven` (`execution.rs:642-648`). None found. |
| R13 | test manager in production | every test seam | the simulation module, `with_controller`, `pending()` and `SPAWNED` are `cfg(test)`; `connect_at`, `place`, `run_with_fault`, `holds_*` are harness-gated. None found. |
| R14 | unit-name sweeps | enumeration, patterns, lookups | none. |
| R15 | PID-only cleanup | signals, waits, opens by id | the pid is the request's `PIDs` and `/proc/<pid>/cgroup` of the retained unreaped child (`scope/native.rs:50-54`), and half of the binding (with the identity); nothing is signalled or waited for by pid in the scope module. None found. |
| R16 | manager reconnect changing authority | every controller | one controller per `ScopeManager`, built once (`scope.rs:156-164`), shared by `Arc` with each operation it issued; no reconnect. None found. |
| R17 | the live harness's callers | the direct API in `tests/phase2_live_sandbox.rs` | the scope-hold test and the unmovable-process test use `execution::place`; a failed placement's cleanup is settled explicitly through the shared `settle` (never dropped, never reaped beside its operation); the scope-hold test now also requires its owner to confirm the end. `connect_at` is used only by the missing-bus case. |
| R18 | the desktop's route | `verification.rs`, `coding_flow.rs`, `phase2_tests.rs` | `ScopeManager::connect()` then `execution::run(` once each (unchanged); the guards pin both and forbid `connect_at`, `execution::place`, `ScopedHelper`, `PlacementFailed`, `reap_helper` in the desktop, and pin the harness gating; p2_g_09's retry pin counts the harness's two new `settle` sites. |

## Value classification

| Value | Where | Class | Becomes authority? |
|---|---|---|---|
| backend-generated unit name | `scope.rs:177-189` | LOCATOR | never: equality only locates a candidate (cleanup) or matches `Id` as one part of the binding |
| helper process id | the retained child | LOCATOR | never; with the identity it binds observations to the operation |
| helper identity (serial) | `launcher.rs:192-197` | RETAINED HELPER IDENTITY | binds the operation to its helper; no authority by itself |
| the `Helper` value | owned by `Owned`, `ScopedHelper`, `RetainedBoundary` | RETAINED HELPER IDENTITY (the owned child) | — |
| `PendingScope` | `scope/pending.rs:90` | PENDING REMOTE OPERATION | owner of everything its request may have created |
| `issued`, `accepted`, `collided`, `settled`, `stops`, `last_stop` | `scope/pending.rs:90-124` | DATA (the operation's state; `last_stop` diagnostics only) | — |
| the issuing controller (`Arc<Controller>`) | `scope/pending.rs:95` | DATA (the connection, fixed at construction) | — |
| kernel membership path | `scope/native.rs:50-54` | LOCATOR | never; opens the candidate once |
| candidate descriptor | `scope/pending.rs:311` | RETAINED NATIVE CGROUP | cleanup ownership only, until promoted |
| GetUnit object path | `scope/pending.rs:327-334` | LOCATOR (of the manager's object) | never |
| `Id` | `scope/manager.rs:282-293` | MANAGER-BOUND UNIT IDENTITY | only together with the `ControlGroup` binding and every native proof |
| `ControlGroup` | `scope/manager.rs:295-306` | MANAGER-BOUND UNIT IDENTITY (the binding) | only together with every other proof |
| limits, `RuntimeMaxUSec`, `OOMPolicy` | `scope/pending.rs:301-363` | DATA | — |
| `Scope` | `scope/pending.rs:500` | PROVEN SCOPE AUTHORITY | the only value a launch follows |
| `establish`, `reconcile`, `retain`, `retry`, `place`, `end` | `scope/pending.rs:485-502`, `scope/pending.rs:164-207`, `execution.rs:503-509`, `execution.rs:186-196`, `execution.rs:257-275`, `execution.rs:731-761` | TRANSITION | each moves ownership in place, or keeps it |

## Residuals (not defects of this module; carried to the report's limitations)

- The binding and the absence rule are validated against the deterministic
  simulation only; systemd's real `Id`, `ControlGroup`, `UnitExists`,
  `NoSuchUnit` and the cgroup namespace's paths remain for G-HOST/G-LIVE.
- An uncertain start whose helper is never seen in a cgroup of its unit
  stays `CleanupFailed`, its helper a zombie, for the backend process's
  lifetime.
- A candidate whose binding failed is still cleanup ownership and is
  confirmed through its own descriptor (Empty/Removed), as accepted in I4;
  the unit of the name is not stopped by name in that case (no sweep).
- Nothing is reconstructed across a backend restart.
