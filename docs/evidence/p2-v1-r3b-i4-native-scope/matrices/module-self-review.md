# P2-V1-R3B-I4 complete-module self-review

Scope: `src/scope.rs`, `src/scope/{manager,native,pending}.rs`,
`src/execution.rs`, `src/launcher.rs`, `src/fault.rs`, the unit tests
(`src/execution/tests.rs`, `src/scope/tests.rs`), their callers (the live
harness `tests/phase2_live_sandbox.rs` and the desktop's
`app/src-tauri/src/coding_flow/verification.rs`, both unchanged) and
`docs/security/phase2-governed-verification.md`. Paths below are relative
to `crates/nexus-verifier-sandbox/src/` unless stated. The searches are
`scripts/review_searches.sh`; their complete output, run on the candidate,
is `validation/review-searches.txt`.

## 1. The section 20 searches

| # | Search | Finding | Verdict |
|---|---|---|---|
| 1 | remote side effects followed by `?` with no owner | The remote calls are `start_scope` (`scope/pending.rs:249`), `stop_unit` (`:181`), `get_unit` (`:316`, `:403`), the two property reads (`:325`, `:331`); none is followed by `?`: each outcome is matched. Every `?` in the module (search 1b) is either before any request (`connect_at`, `random_hex`, `prepare`) or an observation inside the owning `PendingScope`'s own method after `issued` is set (`:296`, `:300`, `:308`, `:315`), so the error returns into an owner that still holds the operation (`ScopeBoundary::establish`, `:456`, leaves it Pending). | none |
| 2 | discarded StopUnit results | The one call records its outcome in `last_stop` (`scope/pending.rs:181`), shown by `Debug` (`:361`) for diagnostics, and never read by any decision; settling observes again after every attempt (`:183-195`). The base's `let _: Result<..> = self.call(.."StopUnit"..)` with nothing after it is gone. | none (repaired during the review: an earlier draft had `let _ =` here, already followed by reconciliation) |
| 3 | timeout interpreted as absence | Every timeout, transport failure, unexpected error name and malformed reply is `Remote::Uncertain` / `Started::Uncertain` (`scope/manager.rs:160-177`, `:214-218`, `:227-231`, `:240-245`, `:254-258`, `:267-271`). Absence is only `Answered(Presence::Absent)` from systemd's `NoSuchUnit` (`:243`), used only together with the helper's membership (`scope/pending.rs:401-406`). `UnitExists` is the only definite "no effect" (`scope/manager.rs:216`). | none |
| 4 | string / PID / path authority | The unit name is a locator: it names the request and the expected last cgroup component (`names_unit`, `scope/pending.rs:388`); no constructor takes it. The pid is the locator of the retained, unreaped `Helper` (`/proc/<pid>/cgroup`, `scope/native.rs:52`; `cgroup.procs`, `scope/pending.rs:311`), bound with the helper's never-reused serial (`:134-136`). A cgroup path is used once, to open the directory the kernel reported (`scope/native.rs:57`); everything after goes through the descriptor. The launcher's `/proc/<pid>` maps write (`launcher.rs:337`) is unchanged. | none |
| 5 | cleanup confirmation from an RPC reply | `settled = true` only after `gone()` (R2: the candidate Empty/Removed; R1: GetUnit `NoSuchUnit` + membership outside), `outside()` for a collided request, or promotion (`scope/pending.rs:163`, `:173`, `:188`, `:461`). `Cleanup::Confirmed` only when nothing is held (`execution.rs:369`) or after `end` returned `true` (`:577`). No reply is read. | none |
| 6 | a native descriptor dropped before later fallible proof | The candidate is stored at once by `self.candidate.insert(open(..)?)` (`scope/pending.rs:300`) before the populated, process-list, limit and manager checks; it leaves the operation only by promotion (`:458`). Settling also acquires one from the helper's membership when none is held (`:201-217`). | none |
| 7 | scope ownership hidden inside a panicking closure | `execute`'s `catch_unwind` (`execution.rs:423`) borrows `owned`, which holds the operation from before its request; `finalize` (`:559`) and `retry` (`:178`) likewise; `ScopeManager::start`'s (`scope.rs:192`) borrows its local boundary, settles it after a panic, then resumes. | none |
| 8 | a Pending state that Drop can erase | `Drop` for `PendingScope` (`scope/pending.rs:367`), `RetainedBoundary` (`execution.rs:203`), `Owned` (`:375`): defense only, never a confirmation; `end_now` never reaps the helper of an unresolved operation (`:630-639`). A boundary is reset to `None` only after settling succeeded (`:607`) or the proven scope was confirmed empty (`:623`); `retain` moves (`mem::take`, `:367`) into the retained boundary. | none |
| 9 | launch while Pending | `handshake` and `launch` (`execution.rs:509`, `:513`) follow `is_proven()` (`:506`); `establish` returns `Ok` only after the in-place promotion. | none |
| 10 | retries dependent on dead stack borrows | No owner has a lifetime parameter (only `ScopeRequest<'a>`, a call's argument); `PendingScope` holds the controller by `Arc` (`scope/pending.rs:68`); `RetainedBoundary` and `PendingScope` are `Send + 'static` (`i4_owners_are_self_contained_values`). | none |
| 11 | background cleanup | No thread or task in the scope module; the execution's only thread is the output drain, as before (`execution.rs:85`). | none |
| 12 | unit-name sweeps | No enumeration, pattern or lookup by process or cgroup. The one unit-name string is the nonce format (`scope.rs:169`); the others found are the output thread's name and the installed helper's name (unchanged). | none |
| 13 | manager reconnection that silently changes authority | A connection is built only in `ScopeManager::connect_at` (`scope.rs:143-151` → `scope/manager.rs:106-136`); an operation keeps the `Arc<Controller>` that issued it for life (no assignment after `new`); a new `ScopeManager` is a new controller for new executions only. A broken connection is never replaced for a pending operation. | none |

Extra searches: 14, production construction of the test seams: the
simulation module and `with_controller` are `#[cfg(test)]`
(`scope.rs:53-54`, `:154`), and the only `Manager`/`Native`/`CgroupDir`
implementations outside tests are `ZbusManager`, `Kernel`, `KernelDir`
(SG-I4-TEST-SEAMS). 15, callers: section 4.

## 2. Value classification

A DATA or LOCATOR value never becomes authority by matching.

| Value | Class | Notes |
|---|---|---|
| unit name (`PendingScope.unit`, `Scope.unit`, `ScopeRequest.unit`) | LOCATOR | the request's name on the bus; the expected last cgroup component. Not authority: no `Scope` or `PendingScope` is built from it (P-I4-PENDING-NEW, P-I4-SCOPE-FORGE, P-I4-PENDING-UNIT) |
| helper pid (`Helper::pid`, `helper_pid`) | LOCATOR | of the retained, unreaped `Helper` only; the pid stays reserved while an operation is unresolved |
| helper serial (`Helper::serial`) | DATA | a binding within the backend process (never reused), crate-private (P-I4-HELPER-SERIAL) |
| `/proc/<pid>/cgroup` path | DATA, then LOCATOR | the kernel's view of the bound helper; used once to open the candidate |
| candidate (`Box<dyn CgroupDir>`) | RETAINED NATIVE IDENTITY | cleanup ownership only; never launch authority |
| `Helper` (child + control socket) | RETAINED NATIVE IDENTITY | the authoritative execution child |
| `PendingScope` (issued, unsettled) | PENDING REMOTE OPERATION | with its `Arc<Controller>` (the issuing connection), binding, limits and candidate |
| `StartFailed.unresolved` | PENDING REMOTE OPERATION | handed to `start`'s caller, owned |
| `Scope` | PROVEN SCOPE AUTHORITY | only by promotion of a pending operation whose every proof passed |
| GetUnit object path | LOCATOR | for the two property reads only |
| `Started`, `Remote<T>` outcomes; limit file text; `cgroup.events`; `cgroup.procs`; RuntimeMaxUSec; OOMPolicy | DATA | compared with expectations; uncertainty is never absence |
| `last_stop`, `stops` | DATA | diagnostics, never evidence |
| `ScopeBoundary::establish`; settling (`reconcile`/`settle`); `Owned::retain`; `RetainedBoundary::retry`; `end` | TRANSITION | see `ownership-transition-matrix.md` |
| `issued`, `collided`, `settled` | TRANSITION state | of the pending remote operation |

## 3. Public API movement

`scope.rs` stays the module root; three private submodules and a test-only
one are new under `src/scope/`. Every movement is deliberate:

| Item | Base | Candidate |
|---|---|---|
| `ScopeManager::connect()` | `scope.rs` | unchanged signature and behaviour |
| `ScopeManager::connect_at(&str) -> Result<Self, ScopeError>` | `scope.rs` | unchanged signature; the socket and owner checks, runtime and bounded connect moved verbatim to `scope/manager.rs` (`ZbusManager::connect_at`) |
| `ScopeManager::start(&Helper, &ResourcePolicy)` | `-> Result<Scope, ScopeError>` | `-> Result<Scope, StartFailed>`: the failure carries `error` and the unresolved operation (`None` once confirmed gone). The live harness's uses (`.unwrap()`, `is_err()` with `{:?}`) compile unchanged |
| `ScopeManager::start_with_fault` | `pub(crate)` | removed; the execution uses `prepare` + `ScopeBoundary::establish` |
| `scope::BUS_CALL_TIMEOUT` | `scope.rs` | same value, defined in `scope/manager.rs`, re-exported at the same path |
| `scope::PLACEMENT_TIMEOUT`, `ScopeError`, `expected_limit_files`, `Occupancy`, `ScopeEvents` | `scope.rs` | unchanged |
| `scope::Scope` | `{ unit, dir: OwnedFd }` | `{ unit, dir: Box<dyn CgroupDir> }` (private); the same public methods `unit`, `kill`, `occupancy`, `wait_empty`, `events` |
| `scope::PendingScope`, `scope::StartFailed`, `scope::SETTLE_TIMEOUT` | — | new |
| D-Bus constants, `call`, the StartTransientUnit property list | `scope.rs` | `scope/manager.rs`, property values unchanged; `call` now distinguishes an error reply's name from uncertainty |
| `/proc/<pid>/cgroup` read; cgroup open, `fstatfs`, `openat` reads, `cgroup.kill`, emptiness | `scope.rs` | `scope/native.rs`, same calls and flags |
| `wait_for_placement`, `Scope::open`'s checks, the property proof | `scope.rs` | `scope/pending.rs` (`prove`), same checks and messages, except: GetUnit `NoSuchUnit` during the proof is `Mismatch("unit not loaded")` (was `Bus`); the candidate is retained before the checks |
| `execution::RetainedBoundary` | `{ scope: Option<Scope>, helper }` | `{ scope: ScopeBoundary, helper }` (private); `retry`, `holds_scope`, `holds_helper` same signatures (`holds_scope`: a proven scope or an unresolved operation) |
| `execution::run`, `run_with_fault`, `ExecutionReport`, `ExitClass`, `classify` | | unchanged |
| `launcher::Helper` | | a private `serial` and `pub(crate) fn serial`; `reap` destructures with `..`; no public change |
| `fault::FaultPoint` | | seven new variants (test and harness only) |

## 4. Callers (unchanged files)

| Caller | Use | Checked by |
|---|---|---|
| `tests/phase2_live_sandbox.rs` | `ScopeManager::connect`/`connect_at` (errors matched as `ScopeError::BusUnavailable`), `start(..).unwrap()`, `start(..).is_err()`, `run_with_fault` with the base's fault points, `RetainedBoundary::{retry, holds_scope, holds_helper}` | built by the section 23 commands (`--no-run`, and clippy `-D warnings`); never run (G-LIVE) |
| `app/src-tauri/src/coding_flow/verification.rs` | `ScopeManager::connect()` then `execution::run` on a blocking thread; the `ScopeManager` dropped at once; the `RetainedBoundary` kept in a `Mutex` shared across threads and retried later from another blocking thread | the desktop backend type-checked against the candidate and its nine Phase Two guard tests run (`validation/desktop-callers.txt`); `i4_23` and `i4_owners_are_self_contained_values` model exactly this use |

Behaviour a live host will see differently (none of it validated live
here; G-HOST, G-LIVE):

- After an uncertain StartTransientUnit the backend asks GetUnit and, unless
  absence is shown, waits for placement (up to `PLACEMENT_TIMEOUT`) instead
  of failing at once; an error reply other than `UnitExists` is treated so.
- A scope that is not proven is settled (killed through its candidate,
  stopped when needed, observed) before the failure is reported, within
  `SETTLE_TIMEOUT` per attempt; an unconfirmed one is `CleanupFailed`, never
  `SandboxUnavailable`.
- A panic while the scope is pending is settled by the finalizer (base: a
  best-effort StopUnit inside `start_with_fault`).

## 5. Residuals and assumptions (recorded, not hidden)

- The confirmation rules rely on systemd semantics that only a supported
  host can confirm: one connection's calls answered in order (a GetUnit
  answered after StartTransientUnit sees its effect); StartTransientUnit
  handled synchronously by the user manager (no deferred authorization);
  the `UnitExists` and `NoSuchUnit` error names; and the kernel's
  `/proc/<pid>/cgroup` for an unreaped, killed helper still naming its
  cgroup, with ` (deleted)` once removed.
- The proof does not consult the unit's `ControlGroup` property (the
  manager's own record of the unit's cgroup path). The kernel side is
  proven (the helper's membership names the unit, the retained directory is
  a populated cgroup v2 directory listing the helper, with exactly the
  limits), and the manager side is proven by name (GetUnit present, exact
  RuntimeMaxUSec and OOMPolicy). Comparing `ControlGroup` with the
  membership would bind the two views of the path, but adds a live-host
  dependency that cannot be validated here; it is a hardening candidate for
  the live module.
- A pending operation whose connection broke cannot be confirmed through
  that connection unless its retained candidate empties; it stays
  `CleanupFailed` for the life of the backend process.
- A helper bound to an unresolved operation whose retained boundary is
  dropped (defense in depth) stays a zombie until the backend exits.
- On a host whose `/proc/<pid>/cgroup` is not the single unified line
  (unsupported), an unprovable absence is `CleanupFailed`.
- Nothing is reconstructed across a backend restart (section 19): the
  scope's runtime backstop and the helper's parent-death chain remain the
  backstops, not Nexus authority.
