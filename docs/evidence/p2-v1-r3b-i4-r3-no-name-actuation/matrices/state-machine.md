# P2-V1-R3B-I4-R3: the pending scope operation after the repair

One execution's scope is `None` → `Pending` (owned before its
StartTransientUnit is issued) → `Proven` (every proof passed) or gone
(settling confirmed it). Unchanged. What changed is what settling may do
while the operation is `Pending`: it never acts on a unit by its name.

## Settling (`PendingScope::reconcile`, then `observe`)

| Start outcome (state) | Cleanup mechanism | Confirmed only when | Otherwise | Tests |
|---|---|---|---|---|
| A. **Nothing issued** | none needed | at once: nothing can exist | — | `i4_28`, `i4r1_x_a_panic_at_any_ownership_point_…` |
| B. **Collision**: exactly `UnitExists`, delivered (`collided`) | none: the unit is foreign; nothing of it is stopped, killed, opened, read or claimed | the kernel reports the helper outside any cgroup of the name | retained with its unreaped helper | `i4r2_06`, `i4r1_15`, `i4r1_16`, `i4_17`, `i4q1r1_nc1` |
| C. **Candidate** (accepted or uncertain): the kernel reported the bound helper in a cgroup of the unit's name, retained by descriptor | `cgroup.kill` through the retained descriptor | the candidate is observed empty or removed | `CleanupFailed`, retained with its descriptor and its unreaped helper | `i4r3_04` (accepted), `i4r2_03`, `i4r2_04` (uncertain), `i4_07`, `i4_08` |
| D. **Accepted, no candidate**: the success reply delivered and recorded (`accepted`) | none: only observed (GetUnit on the issuing connection; the helper's membership) | GetUnit answers exactly `NoSuchUnit` and the kernel then reports the helper outside any cgroup of the name | `CleanupFailed`, retained with its unreaped helper, while a unit of the name is loaded (this request's or a foreign replacement); nothing acts on that unit | `i4r3_01`, `i4r3_02`, `i4r3_03`, `i4_05`, `i4_06`, `i4_09` |
| E. **Uncertain, no candidate** (a timeout, a broken transport, an unexpected error, a reply that does not decode, a panic before the reply was recorded) | none: only observed, in case the kernel reports the helper in a cgroup of the unit (then C) | only through a candidate (C); never by GetUnit's presence or absence, never by the helper's position | `CleanupFailed`, retained with its unreaped helper, possibly for the life of the backend process | `i4r2_01`, `i4r2_02`, `i4r2_07`, `i4r1_11`–`i4r1_13`, `i4q1r1_nc2` |

Every attempt is bounded (`Timing::settle`, then it returns `false`) and
can be retried on the issuing connection without the `ScopeManager`
(`i4r3_09`, `i4_23`, `i4r1_19`, `i4r2_08`).

## Actuation, exactly

- The only manager request that acts is StartTransientUnit, issued once
  per operation for a fresh backend-random name (`issue_and_prove`).
- Settling (`reconcile`, `observe`, `acquire`, `gone`) and the drop
  backstop (`end_now`) ask the manager nothing but GetUnit, through
  `absent`, and only for an accepted operation without a candidate.
- The only thing ever ended is what a retained candidate holds:
  `cgroup.kill` through its descriptor (`reconcile`, `acquire`,
  `end_now`).
- `accepted` changes only what the manager's absence can confirm (row D
  versus E), never what may be acted upon. It is set only in the delivered
  reply's arm, after the post-dispatch fault point, so a panic before the
  reply is recorded leaves the operation uncertain (`i4r2_07`).
- Pinned by behaviour (`i4r3_01`–`i4r3_04`, `i4r3_09` and the preserved R2
  tests) and by structure (`i4r3_10_*`; the adapted source guard
  `SG-I4-DESTINATION` and the added `SG-I4R3-NO-NAME-ACTUATION`).

## Panic and ownership (unchanged, re-proven)

- A panic after the request is dispatched but before its reply is recorded
  leaves `accepted == false`: the operation is uncertain, so even the
  manager's later absence of the unit confirms nothing (`i4r2_07`).
- No panic crosses the last owner; the operation and its helper stay owned
  together and the helper is never reaped while the operation is
  unresolved (`i4r1_01`, `i4r1_x_a_panic_at_any_ownership_point_…`,
  `i4_27`, `i4q1r1_nc3`).
- Dropping an unresolved operation ends what its candidate holds, without
  the manager, and never reaps its helper (`i4_24`, `i4r1_03`).

## Availability cost (explicit, accepted)

- D: an accepted operation without a candidate stays `CleanupFailed`, its
  helper an unreaped zombie, until the manager has independently unloaded
  every unit of its name. A foreign unit loaded under the name delays that
  for as long as it stays loaded; it is never harmed.
- E: an uncertain operation without a candidate stays `CleanupFailed`
  indefinitely unless the kernel later reports the helper in a cgroup of
  the unit (then C).
