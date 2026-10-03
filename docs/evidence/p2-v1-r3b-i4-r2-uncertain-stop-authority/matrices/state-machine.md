# P2-V1-R3B-I4-R2: the pending scope operation after the repair

One execution's scope is `None` → `Pending` (owned before its
StartTransientUnit is issued) → `Proven` (every proof passed) or gone
(settling confirmed it). Unchanged. What changed is what settling may do
while the operation is `Pending`, by how its start ended.

## Settling (`PendingScope::reconcile`, then `observe`)

| Start outcome (state) | Name-based `StopUnit` | Cleanup mechanism | Confirmed only when | Otherwise |
|---|---|---|---|---|
| **Accepted**: success reply delivered and recorded (`accepted`) | allowed, bounded, after the candidate (if any) is ended and the state is observed not gone; its reply is never confirmation | candidate ended through its descriptor; without a candidate, the manager's own unit (created for this request) asked to stop | the candidate is empty or removed; without one, GetUnit answers `NoSuchUnit` on the issuing connection and the kernel reports the helper outside any cgroup of the name | `CleanupFailed`, retained with its unreaped helper; retryable |
| **Collision**: exactly `UnitExists`, delivered (`collided`) | never | none: the unit is foreign; nothing of it is opened, killed, read or claimed | the kernel reports the helper outside any cgroup of the name | retained with its unreaped helper |
| **Uncertain, no candidate** (no recorded reply: a timeout, a broken transport, an unexpected error, a reply that does not decode, a panic before the reply was recorded) | **never** (P2-V1-R3B-I4-R2) | none by name; observed within the settling bound, in case the kernel reports the helper in a cgroup of the unit | only once a candidate is found and confirmed (next row); never by GetUnit's presence or absence, never by the helper's position | `CleanupFailed`, retained with its unreaped helper, possibly for the life of the backend process (the accepted availability cost) |
| **Uncertain, with a candidate** (the kernel reported the bound helper in a cgroup of the unit's name, retained by descriptor) | **never** (P2-V1-R3B-I4-R2) | `cgroup.kill` through the retained descriptor, then observation | the candidate is empty or removed | `CleanupFailed`, retained with its descriptor and its unreaped helper |

The helper is killed but never reaped while the operation is unresolved
(its process id stays reserved, so a still-queued start job cannot attach a
reused id); it is reaped only once the operation is confirmed gone
(`execution::end`, unchanged).

## `StopUnit` authority, exactly

- **May occur** only in `reconcile`, past `stop_authorized()`, which is
  exactly `accepted`: StartTransientUnit's success reply (its job path) was
  delivered, decoded and recorded in the pending owner, in the
  `Started::Accepted` arm of `issue_and_prove`, after the post-dispatch
  fault point. It is attempted once per settling attempt, only when the
  operation is not already observed gone, and its reply (delivered, failed
  or timed out) is diagnostics only: what remains is observed again.
- **Never occurs** for a collided operation, an uncertain one (with or
  without a candidate), a reply that arrived but was not recorded before a
  panic, a drop (`end_now` kills the candidate only), or anything a unit
  name, GetUnit's presence or absence, or the helper's position could
  suggest. No other production code calls `stop_unit`.
- Pinned by behaviour (`i4r2_01` to `i4r2_08`, the tightened `i4r1_01`,
  `i4r1_12`, `i4q1r1_nc2`) and by structure
  (`i4r2_x_only_a_recorded_start_reply_authorizes_a_stop_by_name`, the
  accepted source guard `SG-I4R1-UNCERTAIN`).

## Panic and ownership (unchanged, re-proven)

- A panic after the request is dispatched but before its reply is recorded
  (`FaultPoint::AfterScopeStart`, or a panic inside StartTransientUnit)
  leaves `accepted == false`: no name authority afterwards (`i4r2_07`).
- No panic crosses the last owner: `execution::run` and `execution::place`
  catch it, finalize, and confirm or retain the operation with its helper
  together (`i4r1_01`, `i4r1_x`, `i4r2_07`); a retained boundary is
  retried on the issuing connection without its `ScopeManager`, still
  without name authority (`i4r2_08`).
- Dropping an unresolved operation ends what its candidate holds, without
  the manager, and never reaps its helper (unchanged).

## Availability cost (explicit)

An uncertain start without a candidate is never confirmed: its execution is
`CleanupFailed`, never `SandboxUnavailable`, and its helper stays an
unreaped zombie for the life of the backend process unless the kernel later
reports the helper in a cgroup of the unit (then that candidate is ended and
confirmed through its descriptor). This is the price of never stopping a
unit the backend did not prove it created.
