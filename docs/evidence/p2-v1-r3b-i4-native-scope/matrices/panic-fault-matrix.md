# P2-V1-R3B-I4 panic / fault matrix

Every deterministic fault point (`crates/nexus-verifier-sandbox/src/fault.rs`),
where it fires, what is owned at that instant and by whom, and what an
injected panic leaves. Production never injects a fault: `execution::run`
passes none (`execution.rs:324-334`), `fault::at(None, _)` never panics,
and `run_with_fault` exists only under the harness gate
(`execution.rs:336`, guarded by `p2_g_06` and SG-I4-DESKTOP-REPLICA).
`ScopeManager::start` (public) never injects one either.

A panic inside `attempt` unwinds to `execute`'s `catch_unwind`
(`execution.rs:423`); everything lives in `Owned`, which `attempt` only
borrows, and the one finalizer (`execution.rs:558`) then runs over whatever
exists. A panic inside the finalizer moves what remains into a
`RetainedBoundary` (`execution.rs:560`). The result is never
`NotRun::Scope` + `Cleanup::Confirmed` unless absence or cleanup was
observed: every panic before the launch is `NotRun::Interrupted`.

| Fault point | Fires at | Owned at that instant | An injected panic leaves | Tests |
|---|---|---|---|---|
| `AfterSpawn` | `execution.rs:491` | the helper, its output threads; no scope operation | the helper killed and reaped; confirmed; `NotRun::Interrupted` | live `p2r1_live_panic_after_helper_spawn_is_finalized` (unchanged) |
| `BeforeScopeStart` (new) | `scope/pending.rs:246` | the helper; a prepared, unissued operation (in `Owned`) | nothing was requested (no call reaches the manager); confirmed | I4-28 |
| `AfterScopeStart` (new) | `scope/pending.rs:254` | the issued operation (in `Owned`); its reply in hand | settled by the finalizer (the candidate found through the helper's membership, R2), or retained | I4-25 |
| `ScopeReconcile` (new) | `scope/pending.rs:269` (an uncertain start), `:162`, `:171`, `:186` (settling) | the issued operation | retained (`CleanupFailed`); a retry settles it | I4-27 |
| `ScopeProof` | `scope/pending.rs:297` | the issued operation, the helper placed, no candidate yet | the finalizer acquires the candidate from the helper's membership and settles (R2); confirmed | I4-26; live `p2r1_live_panic_while_proving_the_scope_stops_it` (unchanged) |
| `ScopeCandidate` (new) | `scope/pending.rs:301` | the issued operation with its candidate descriptor | settled through that very descriptor (no reopen), or retained with it | I4-26 |
| `ScopeProperties` (new) | `scope/pending.rs:324` | the issued operation with its candidate | settled through the candidate, or retained | I4-26 |
| `BeforeScopeStop` (new) | `scope/pending.rs:176` | the unresolved operation, not yet stopped | retained; a retry stops and settles it | I4-27 |
| `AfterScopeStop` (new) | `scope/pending.rs:182` | the unresolved operation, StopUnit returned | retained (the stop's effect is observed only by a later settle) | I4-27 |
| `AfterScope` | `execution.rs:504` | the proven scope, the helper; no launch | the proven scope ended as before; confirmed; nothing launched | I4-28; live `p2r1_live_panic_after_the_scope_is_proven_is_finalized` (unchanged) |
| `AfterLaunch`, `Running`, `Draining`, `DrainThread`, `BeforeFinalize` | `execution.rs:518`, `:537`, `:540`, `:115`, `:547` | unchanged from the base | unchanged from the base | live p2r1 suite (unchanged), `p2r1_*` unit tests |
| `Finalizing` (panic or failed step) | `execution.rs:567` | unchanged from the base | the boundary retained | `p2r1_a_failed_or_panicking_finalization_retains_the_live_boundary`, live p2r1 suite |

## Panics outside the fault points

| Where | Owned | What a panic leaves |
|---|---|---|
| `ScopeManager::start` (public), while establishing | its local `ScopeBoundary` | settled in place (`scope.rs:192-200`), then the panic resumes; an operation still unconfirmed is dropped as defense in depth only (as before: the base stopped the unit best-effort and resumed) |
| `RetainedBoundary::retry` | the boundary | `Err(self)`: still retained (`execution.rs:177-186`) |
| a drop of an unresolved operation | — | `PendingScope::drop` ends what its candidate holds; `end_now` kills, never reaps, the helper; nothing is confirmed (I4-24) |
| `ZbusManager` dropped on any thread (the last owner of an operation or manager) | the runtime | `shutdown_background()`: never blocks, never panics inside an asynchronous runtime (`scope/manager.rs:276-283`) |
