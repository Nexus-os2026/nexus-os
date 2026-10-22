# P2-V1-R3B-I4-R3: root cause

## The defect (base 2438f47f, the accepted R2 candidate)

`PendingScope::reconcile` (`crates/nexus-verifier-sandbox/src/scope/pending.rs`
at the base) ended an accepted operation that it could not observe gone by
asking the manager to stop its unit by name:

```rust
if !self.stop_authorized() {          // fn stop_authorized(&self) -> bool { self.accepted }
    return self.observe(&controller, helper, fault);
}
fault::at(fault, FaultPoint::BeforeScopeStop);
self.stops = self.stops.saturating_add(1);
self.last_stop = Some(controller.manager.stop_unit(&self.unit));   // StopUnit(name, "replace")
fault::at(fault, FaultPoint::AfterScopeStop);
self.observe(&controller, helper, fault)
```

`accepted` records that StartTransientUnit's success reply (its job path)
was delivered and recorded: the manager created a unit of that name for
this request at that moment. It identifies nothing later. A transient unit
"will be released as soon as it is not running or referenced anymore"
(org.freedesktop.systemd1(5), systemd 255 on the host), and the request asks
`CollectMode=inactive-or-failed`; once released, the name is free, and any
client of the same user manager (the same uid) can load a unit of its own
under it. Settling can run long after the start (a retained boundary is
retried whenever its owner asks), so a later settling attempt stopped
whatever unit then held the name.

The root cause is one of authority, not of timing: R2 treated the recorded
reply as a lasting authority over a reusable name. The name, and the object
path GetUnit returns for it (derived from the name), are locators; no
answer about them at one moment binds the unit that holds them at another.

## Reproduction (before the repair)

`reproduction/name_reuse_on_the_base.rs`, appended in a scratch snapshot of
2438f47f to `execution/tests.rs` and run alone, on the base's own model and
production code (`scripts/reproduce_name_reuse.sh`; output
`reproduction/before-repair.out`):

1. The start is delivered and recorded (accepted); the helper never enters
   the unit's cgroup (no candidate); the first settling's StopUnit has no
   effect, so the operation is retained (`CleanupFailed`).
2. This request's unit is unloaded; a foreign unit is loaded under the same
   name, in a new cgroup of the same path, holding a process of its own.
3. A retry of the retained boundary settles: `StopUnit(<name>)` is issued
   once; the foreign unit is unloaded, its cgroup killed and removed, its
   process's membership `... (deleted)`; the retry reports the operation
   confirmed.

Result: `REPRODUCED: an accepted operation stopped the foreign unit loaded
under its name`; the test file was restored byte for byte and the scratch
checkout's status was clean.

## Why no replacement actuation

The mission excludes every replacement (Unit.Stop on an object path,
KillUnit by name, another name-only call, a GetUnit object path as
identity, a property comparison before acting, verify-then-act by name):
each still acts through a locator that a later unit can hold.

The host's manual was read for a manager-native capability that is not
reusable (the mission's stop condition). None exists in this manager's
interface:

- every Manager request that acts on a unit takes its name (`StopUnit`,
  `KillUnit`, `QueueSignalUnit`, `RestartUnit`, `ResetFailedUnit`,
  `AbandonScope`, ...), and every Unit method acts on the object whose path
  is derived from the name;
- `GetUnitByPIDFD` and `GetUnitByInvocationID` are lookups: they return the
  same object path (and, for the former, the unit's invocation id); any
  action after them is by that path, so verify-then-act;
- unit references (`RefUnit`/`UnrefUnit`, `Unit.Ref`/`Unref`) keep a unit
  from being released while the referencing client holds them. They are a
  lifetime pin, not an actuation handle: acting would still be by name or
  object path, a reference is lost with the connection that holds it, and
  for an uncertain start it is not known to exist. Not used; recorded for
  the Architect.

The non-reusable capability this backend does hold is kernel-native: the
candidate cgroup's directory, retained by descriptor (`cgroup.kill`, then
`cgroup.events`, through that descriptor). R3 keeps it as the only means of
ending what an operation created, and removes name actuation.
