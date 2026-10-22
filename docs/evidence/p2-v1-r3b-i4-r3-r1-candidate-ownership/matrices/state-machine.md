# P2-V1-R3B-I4-R3-R1: the ownership state machine

One execution's scope is a `ScopeBoundary` (`scope/pending.rs`): `None`,
`Pending(PendingScope)` or `Proven(Scope)`. R3-R1 splits what a pending
operation holds into separate states. A retained descriptor is a cgroup's
native identity, never proof that this request created that cgroup.

## States of a pending operation

| State | Holds | `cgroup.kill` | Launch | Confirms the operation |
|---|---|---|---|---|
| issued, no candidate | the unit name (a locator), the bound helper | nothing to kill | no | an accepted start: `NoSuchUnit`, then the helper outside a cgroup of the name; an uncertain start: never; a collision: the helper outside |
| observed candidate | a descriptor of the cgroup the kernel reported the helper in, and the path it was opened from (`owned: false`) | **never** | no | **never** by its emptiness; an accepted start: the absence rule above |
| owned candidate (cleanup authority) | the same descriptor, bound to the start's own invocation (`owned: true`) | yes, through the descriptor only | no | its emptiness or removal |
| proven scope (scope authority) | the owned descriptor and the invocation it is bound to, as a `Scope` | yes (finalization) | **yes**, only this state | its emptiness or removal (finalization) |

Only an accepted start whose identity was captured ever retains a
candidate (`acquire` returns without one; the proof requires it). An
uncertain start opens nothing.

## Transitions

| From | To | When (and only when) |
|---|---|---|
| prepared | issued | the request may be dispatched (`issued = true` before `start_scope`) |
| issued | settled | `Started::NotIssued`: the start's signals could not be watched, the request was never sent |
| issued | accepted, identity recorded | `Started::Accepted(Ok(id))`: the delivered reply and the identity captured from the start's own job, recorded together after the post-dispatch fault point |
| issued | accepted, no identity | `Started::Accepted(Err(_))`: never owns a candidate; only absence confirms it |
| issued | collided | exactly `UnitExists`, delivered: nothing opened, read or killed; confirmed once the helper is outside |
| issued | uncertain | anything else (no reply, an unexpected error, a reply that does not decode, a panic before recording): never opens, owns, ends, proves or confirms |
| no candidate | observed | the kernel reports the bound helper in an absolute, normal-form path named for the unit, and that path opens as a cgroup v2 directory (the proof, or settling's `acquire`) |
| observed | owned | `bind`: the helper reported at exactly the descriptor's path after the open, the directory not removed; GetUnit's unit has `InvocationID` == captured, `Id` == name, `ControlGroup` == the path, `ControlGroupId` == the descriptor's own kernel cgroup ID, `InvocationID` == captured again. In the proof, after `populated` and `cgroup.procs`; in settling, once per attempt |
| owned | proven | `establish`, after the proof's policy (`limits`, `RuntimeMaxUSec`, `OOMPolicy`, `InvocationID` again) and the promotion point: the owned candidate moves into the `Scope`, in one non-panicking step |
| owned, policy failed | settled | settling ends it through its descriptor and confirms it empty or removed; nothing launched |
| observed | (stays observed) | any binding failure, uncertainty included: never ended; a later attempt may bind it with fresh reads, never from a recorded path |

## What ends what

| Path | Ends |
|---|---|
| `reconcile` (finalizer, retry) | `end_owned()`: the owned candidate only; then `observe`: a newly bound candidate only |
| `end_now` (drop backstop) | `end_owned()` only, while unresolved |
| the proof, `acquire`, `bind`, `gone` | nothing |
| a proven scope (finalization, its own drop backstop) | its descriptor |

`cgroup.kill` occurs twice in `pending.rs`: in `end_owned` (through
`owned_dir()`, which filters `candidate.owned`) and for a `Proven` scope.
Nothing acts on a unit by its name or object path (P2-V1-R3B-I4-R3,
unchanged).

## Panic points (`fault.rs`, and the model's `Op`)

| Point | State at the panic | Outcome (`r3r1_09`) |
|---|---|---|
| inside the start, after its effect, before a reply (`Op::Start`) | issued, nothing recorded | uncertain: retained, nothing opened or ended, the helper unreaped |
| inside the start, after the success reply, while the identity is captured (`Op::Capture`) | issued, nothing recorded | the same (the reply and the identity are one return: nothing is recorded until both are in hand) |
| `AfterScopeStart` (the start returned) | issued, nothing recorded | the same |
| `ScopeProof` (placement seen, nothing opened) | accepted, identity recorded | settling observes, binds, ends, confirms |
| `ScopeCandidate` (descriptor retained, not bound) | observed | settling binds with fresh reads, then ends and confirms; nothing ended before |
| inside the binding (`ScopeBinding`; `Op::GetUnit`, `UnitInstance`, `UnitId`, `ControlGroup`, `ControlGroupId`) | observed | the same |
| `ScopeOwned` (bound, the policy not proven) | owned | settling ends it through its descriptor and confirms; no launch |
| `ScopeProperties`, `Op::RuntimeMax` (during the policy) | owned | the same |
| `ScopePromotion` (every proof passed, not yet Proven) | owned | the same: no launch |

No panic crosses the owner (`execution::run`, `execution::place`), and the
helper of an unresolved operation is never reaped.
