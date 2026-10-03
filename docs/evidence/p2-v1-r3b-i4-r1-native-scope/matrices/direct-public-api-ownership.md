# P2-V1-R3B-I4-R1: direct and public API ownership

Every way into a scope, by build. "Normal build": the crate without the
`live-sandbox-harness` feature (the desktop, a release); the evidence is
the external-crate probes (`controls/normal-api-probes/`). "Harness build":
the crate's own integration tests and live harness, which its
dev-dependency builds with the feature (`controls/api-guards/`).
Citations are to `crates/nexus-verifier-sandbox/src/` of the candidate.

## Constructing a manager

| Item | Normal build | Harness build | Notes |
|---|---|---|---|
| `ScopeManager::connect` | public | public | the bus `/run/user/<real uid>/bus`, derived from `getuid()` (`scope.rs:141-145`) |
| `ScopeManager::connect_at(path)` | absent: E0599 (N-I4R1-CONNECT-AT) | public, gated (`scope.rs:149-152`) | the live harness's fail-closed checks of the bus checks only; the same checks as `connect` (`scope.rs:156`) |
| `connect_to(path)` | private | private | the one constructor of a production controller (`scope.rs:156-164`) |
| `ScopeManager::with_controller` | absent | absent: E0599 (P-I4-WITH-CONTROLLER) | unit tests only (`scope.rs:167-172`) |
| `ZbusManager`, `Kernel`, the `Manager` and `Native` traits | private module | private module (P-I4-MANAGER-MODULE, P-I4-NATIVE-MODULE) | destination, object path and interfaces are constants (`scope/manager.rs:24-29`) |

## Starting a scope

| Item | Normal build | Harness build | Who owns the helper | Who owns the scope operation | Failure, panic |
|---|---|---|---|---|---|
| `execution::run` | public (`execution.rs:454`) | public | the execution's `Owned` (`execution.rs:479`), from before the scope is prepared | `Owned.scope`, prepared and stored before its request (`execution.rs:633-636`) | never unwinds (`execution.rs:559`); the finalizer confirms or retains both in one `RetainedBoundary` (`execution.rs:694`, `execution.rs:503-509`) |
| `execution::place` | absent: E0425 (N-I4R1-PLACE) | public, gated (`execution.rs:257`) | consumed: `ScopedHelper.helper` (`execution.rs:262-265`) | `ScopedHelper.scope`, prepared in place (`execution.rs:290`) | never unwinds: the attempt runs behind `catch_unwind` (`execution.rs:266`); an error or a panic ends both with the finalizer's steps (`execution.rs:273`, `execution.rs:320-330`), confirmed or one `RetainedBoundary` holding both |
| `ScopeManager::start` (I4) | removed: E0599 (N-I4R1-DIRECT-START) | removed: E0599 (P-I4R1-START-REMOVED) | — | — | — |
| `ScopeManager::prepare` | crate-private | crate-private: E0624 (P-I4-PREPARE) | its callers own it (`execution.rs:633`, `execution.rs:290`) | — | — |
| `ScopeBoundary::establish` | crate-private | crate-private: E0603 on the type (P-I4-BOUNDARY-TYPE) | — | in place: Pending to Proven only after every proof (`scope/pending.rs:485-501`) | a panic leaves the operation in its owner |

## Owners

| Item | Normal build | Harness build | Holds | Drop |
|---|---|---|---|---|
| `RetainedBoundary` | public, fields private (P-I4-RETAINED-SCOPE) | + `holds_scope`, `holds_helper` (absent in a normal build: N-I4R1-HOLDS) | the scope boundary and the unreaped helper (`execution.rs:176-179`) | defense only (`execution.rs:221-228`) |
| `RetainedBoundary::retry` | public | public | runs `end` behind `catch_unwind`; `Err(self)` keeps both (`execution.rs:186-195`) | — |
| `ScopedHelper` | absent: E0425 (N-I4R1-SCOPED-HELPER) | public, gated; fields private (P-I4R1-SCOPED-FORGE, -TAKE-HELPER, -TAKE-SCOPE); not Clone, not Deserialize | the helper and its proven scope (`execution.rs:235-238`) | defense only: `end_now` (`execution.rs:334-340`) |
| `ScopedHelper::helper`, `scope` | absent | `&Helper` and `&Scope` only (`execution.rs:295-302`): no reap, kill or move of the helper through them | — | — |
| `ScopedHelper::reap_helper` | absent | only while the scope is proven (`execution.rs:308-316`) | — | — |
| `ScopedHelper::end` | absent | confirmed, or one `RetainedBoundary` (`execution.rs:320-330`) | — | — |
| `PlacementFailed` | absent: E0425 (N-I4R1-PLACEMENT-FAILED) | public, gated: `error` and `cleanup` only; not Clone (P-I4R1-PLACEMENT-CLONE) | the cleanup of everything the placement owned (`execution.rs:245-249`) | its boundary's drop |

## Pending operations

| Item | Normal build | Harness build | Notes |
|---|---|---|---|
| `PendingScope` | crate-private: E0603 (N-I4R1-PENDING-SCOPE) | crate-private: E0603 (P-I4-PENDING-NEW, -STATE, -CANDIDATE, -UNIT, -RECONCILE, -CLONE, -SERIALIZE, -DESERIALIZE) | `pub(crate) struct PendingScope` (`scope/pending.rs:90`), re-exported only to the crate (`scope.rs:65`) |
| `StartFailed` (I4) | removed: E0425 (N-I4R1-START-FAILED) | removed: E0425 (P-I4R1-START-FAILED-REMOVED) | no failure carries an operation apart from its helper |
| `PendingScope::settle(&Helper)` (I4) | removed | removed | settling is `reconcile`, crate-private (`scope/pending.rs:164`), called only by the finalizer's `end` after it killed the helper it keeps unreaped (`execution.rs:736-745`) |
| `PendingScope::unit`, `ScopeBoundary::into_pending` (I4) | removed | removed | used only by the removed public start |
