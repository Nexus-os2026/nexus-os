# P2-V1-R3B-I4 ownership-transition matrix

Every transition of the execution's native boundary: the retained helper
and the scope (`ScopeBoundary`: None, Pending, Proven). Line references are
to the candidate's sources (`crates/nexus-verifier-sandbox/src/`). The
invariant every row preserves: from the moment a request may have had an
effect, something owns it until it is proven (Proven) or confirmed gone
(settled); nothing in between can be dropped silently.

## Owners

| Owner | Holds | Where |
|---|---|---|
| `execution::Owned` (in `execute`'s own frame) | `helper: Option<Helper>`, `scope: ScopeBoundary`, the output threads | `execution.rs:349` |
| `RetainedBoundary` (returned in `Cleanup::Failed`) | `scope: ScopeBoundary` (Pending or Proven), `helper: Option<Helper>`; no borrow, `Send + 'static` | `execution.rs:167` |
| `PendingScope` (boxed in `ScopeBoundary::Pending`, or `StartFailed.unresolved`) | `Arc<Controller>` (the issuing connection), unit locator, helper binding (pid, serial), limits, `issued`/`collided`/`settled`, `candidate: Option<Box<dyn CgroupDir>>`, `stops`/`last_stop` | `scope/pending.rs:63` |
| `Scope` (in `ScopeBoundary::Proven`) | unit locator, the proven cgroup by descriptor | `scope.rs:297` |
| `ScopeManager` | `Arc<Controller>` (shared with every operation it prepared) | `scope.rs:129` |

## Transitions

| # | From | To | Where | Condition / guarantee | Tests |
|---|---|---|---|---|---|
| T1 | — | helper owned | `execution.rs:463-490` (`attempt`: `owned.helper.insert`) | stored before its output threads, before any scope request | I4-28, p2r1 suite |
| T2 | helper owned | Pending (not issued) | `execution.rs:497-500` (`prepare`, stored in `owned.scope`) | a fresh backend nonce; stored before the request can have any effect; binds the helper (pid, serial), the limits and the issuing controller | I4-28 |
| T3 | Pending (not issued) | Pending (issued) | `scope/pending.rs:248` (`self.issued = true`) | set immediately before `start_scope` (`:249`); a panic before it leaves nothing issued | I4-25, I4-28 |
| T4 | Pending (issued) | Pending (collided) | `scope/pending.rs:257-262` | `UnitExists`: no effect; the loaded unit is never stopped or claimed | I4-17 |
| T5 | Pending (issued) | Pending (issued, candidate) | `scope/pending.rs:300` (`prove`), `:201-217` (`acquire` while settling) | the cgroup the kernel reports the bound helper in, opened once and retained before any later fallible check; cleanup ownership only | I4-04, I4-10, I4-26 |
| T6 | Pending (candidate, every proof passed) | Proven | `scope/pending.rs:448-466` (`ScopeBoundary::establish`) | in place: the candidate is moved into `Scope`, the pending marked settled, `*self = Proven`; the only constructor of a `Scope` | I4-01, I4-16 |
| T7 | Proven | launch permitted | `execution.rs:506-513` | `is_proven()` guard before the handshake; `launched` set only after it | I4-18, I4-28 |
| T8 | Pending (issued) | gone (settled) | `scope/pending.rs:153-196` → `execution.rs:600-607` | R1, R2 or collided-outside observed; then `*scope = None`, and only then the helper reaped | I4-02, I4-05, I4-08, I4-20 |
| T9 | Pending (unresolved) | retained | `execution.rs:366-372` (`Owned::retain`), from `finalize` (`:558-571`) | settling failed or panicked: the operation and the unreaped helper move into the `RetainedBoundary` | I4-06, I4-07, I4-09, I4-20, I4-21, I4-27 |
| T10 | retained | gone | `execution.rs:177-186` (`retry` → `end`) | the same settling, on the issuing connection, without the `ScopeManager` | I4-22, I4-23 |
| T11 | retained | retained | `execution.rs:177-186` | `retry` fails or panics: `Err(self)` | I4-22 |
| T12 | Proven | gone | `execution.rs:595-624` (`end`) | as before: `cgroup.kill`, helper kill and bounded reap, bounded wait for Empty/Removed | I4-19, I4-28, p2r1 suite |
| T13 | Proven | retained | `execution.rs:366-372` | as before | p2r1 suite |
| T14 | Pending (never issued) | gone | `scope/pending.rs:154-156`, `execution.rs:366-372` | nothing was requested: nothing can exist (`holds()` is false) | I4-28 |
| T15 | any | dropped (defense only) | `scope/pending.rs:367-373`, `execution.rs:203-209`, `:375-380`, `:630-639` | ends what the candidate holds and kills the helper, without waiting or the manager; never reaps the helper of an unresolved operation; never a confirmation | I4-24 |
| T16 | — | `ScopeManager::start` (public) | `scope.rs:186-218` | its own local boundary: Proven returned, or `StartFailed` after settling (a panic: settled, then resumed), with the operation owned when unconfirmed | I4-01..03, I4-12, scope tests |

## What can never happen

| Forbidden transition | Why it cannot | Evidence |
|---|---|---|
| Pending → Proven from a unit name, a path or a pid | `Scope` has private fields and one construction site (T6), which needs the candidate opened from the helper's own membership | P-I4-SCOPE-FORGE, P-I4-PENDING-NEW, SG-I4-ONE-WAY, I4-16, NC-I4-START-NAME-AUTH |
| Pending → gone on a StopUnit reply | settling never reads `last_stop` | I4-07, NC-I4-STOP-OK-CONFIRMS |
| Pending → gone on an uncertain reply | uncertainty enters settling's observations only | I4-03, I4-06, I4-09, NC-I4-START-TIMEOUT-ABSENT, NC-I4-STOP-ERR-NO-EFFECT, NC-I4-STOP-TIMEOUT-DROPS |
| Pending lost by a panic | the operation lives in `Owned` before T3, and in place through T5 and T6 | I4-25..27, NC-I4-PANIC-OWNER-LOSS |
| Pending lost on return | `retain` hands every unresolved operation on | I4-20, I4-21, NC-I4-PENDING-NOT-RETAINED, NC-I4-START-LOST-OWNER |
| Pending settled by the helper's death | settling needs R1 or R2; a dead helper is neither | I4-30, NC-I4-PID-CLEANUP |
| helper reaped while its operation is unresolved | `end` reaps only after settling (T8); `end_now` never | I4-24, NC-I4-X-DROP-REAPS |
| retry needing the ScopeManager | the operation holds the controller by `Arc` | I4-23, NC-I4-RETRY-NEEDS-MANAGER-BORROW |
| another helper's membership as evidence | settling binds the helper by serial and pid | `i4_a_pending_operation_is_bound_to_its_own_helper`, NC-I4-X-ANY-HELPER |
