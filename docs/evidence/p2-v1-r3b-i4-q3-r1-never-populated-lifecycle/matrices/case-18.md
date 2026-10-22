# Live case 18, `p2d_live_unmovable_process_fails_closed` (new behaviour)

The case (`tests/phase2_live_sandbox.rs`) spawns the real helper and drives
`never_populated::qualify` (`tests/support/never_populated.rs`) over its
world `NeverPopulated`: production's `execution::place` and
`execution::RetainedBoundary::retry`, the helper's exit observed without
reaping it (`never_populated::exit_unreaped`, `unreaped`), the checked scope
observation (`loaded_scopes_by`) and an explicit `release` of a boundary
still held when the case fails. The case itself reaps, sleeps, waits,
observes and retries nothing (`np_18`).

## The procedure (every wait bounded)

| Step | Proof | Failure (each reported with what was observed) |
|---|---|---|
| 1 | the baseline of loaded verifier scopes | an unobservable manager (never "no scopes") |
| 2 | `SIGKILL`; the exit observed by its own report, `waitid(P_PID, WEXITED \| WNOHANG \| WNOWAIT)`, polled within 10 s; killed by `SIGKILL`; still unreaped | no exit within the bound; another exit; reaped |
| 3 | production's placement: no scope proven, a boundary retained | a proven scope; cleanup confirmed at once |
| 4 | exactly one new verifier scope, named in the boundary's own description | none, several, or another's |
| 5 | while a full retry still ends 5 s before the backstop: every retry unconfirmed, the helper unreaped, the scope loaded; at least one | a confirmation before the backstop (less 1 s) is a failure; the scope gone; the helper reaped |
| 6 | the scope collected no earlier than the backstop (less 1 s) and within 30 s after it; the helper still unreaped | collected early; not collected; reaped |
| 7 | a bounded explicit retry (at most 3) confirms the operation | unconfirmed: released explicitly, never as a confirmation |
| 8 | the helper reaped now (by production); the loaded scopes back to the baseline within 10 s | not reaped; scopes left |

A retry begun before the backstop that confirms after it (a slow manager
call) is accepted only after the backstop (less 1 s) and after at least one
unconfirmed retry.

## The case's runtime backstop

`never_populated::limits(limits())`: the harness's limits with
`runtime_backstop_secs = 45` (`BACKSTOP_SECS`), nothing else changed;
production's policy and default (660 s) and the harness's own `limits()`
(120 s) are unchanged. 45 s is the smallest round value that leaves room,
before the backstop, for production's placement (`PLACEMENT_TIMEOUT` 10 s,
then one settling, `SETTLE_TIMEOUT` 10 s), one full explicit retry (10 s)
and a 5 s margin: `Plan::coherent` (45 > 10 + 10 + 10 + 5), checked before
anything is done. Production uses the value only as the start's
`RuntimeMaxUSec` (`scope/manager.rs`) and in the proof of a placed scope
(`scope/pending.rs`), which this case never reaches; nothing couples it to
the wall timeout. 5 s (the mission's preference) is not used: the scope
would be collected during production's own placement, and no retry before
the backstop would remain to qualify.

## The helper's reap order

Killed (step 2) → observed exited, not reaped (`WNOWAIT`: still a waitable
zombie, its process id reserved) → placed (production owns it with the
operation) → unreaped through every retry before the backstop and through
the collection (steps 5, 6) → reaped by production only in the retry that
confirms the operation (step 7), checked at step 8. Nothing in the case or
its support reaps it; a failure releases the boundary to production's drop
backstop, which never reaps a helper of an unresolved operation.

## Diagnostics

Every failure carries: what failed; the unit (from the observation and the
boundary's description); production's placement error; whether a boundary
was retained; the retries made; the elapsed time since the placement began;
whether the cleanup was confirmed; the last loaded scopes (or why they could
not be observed); and the boundary's own description
(`PendingScope { unit, helper_pid, issued, collided, accepted, instance,
candidate, owned, settled }`, production's existing `Debug`; no new
introspection). `np_14` pins the report's fields.

## Expected live timing

About 20 s placement (or 10 s when the identity is not captured), one or
two unconfirmed retries, the collection at about 45 s, the confirmation at
once: about 46-50 s for the case.
