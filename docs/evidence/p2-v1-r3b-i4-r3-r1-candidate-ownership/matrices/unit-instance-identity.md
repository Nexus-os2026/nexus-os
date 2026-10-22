# P2-V1-R3B-I4-R3-R1: the unit-instance identity (primary sources)

Sources: Ubuntu's exact systemd package source, `import/255.4-1ubuntu8.17`
with its 84 patches applied in order (the host runs systemd
255.4-1ubuntu8.17), and upstream systemd-stable `v255.4`; the D-Bus
specification and `bus/dispatch.c` at `dbus-1.14.10` (the host's
dbus-daemon). Every cited systemd file is identical to upstream v255.4
except `dbus-unit.c`, `dbus-manager.c`, `cgroup.c` and `manager.c`, whose
Ubuntu changes concern PID reads, request limits, a control-group path
check and log levels, none of the paths below. Line numbers are the
applied tree's; `primary/systemd-255.4-excerpts.txt` reproduces every cited
range with its file's SHA-256 (`scripts/primary_excerpts.sh`).

## Section 10: `org.freedesktop.systemd1.Unit.InvocationID`

| Question | Answer (v255.4) | Source |
|---|---|---|
| What generates it | `unit_acquire_invocation_id()`: `sd_id128_randomize()` (`random_bytes()`, made a v4 UUID), then `unit_set_invocation_id()` | `unit.c` 5377–5391; `sd-id128.c` 326–339; `unit.c` 3465–3500 |
| When a scope acquires it | on every start, in `scope_enter_running()`, before the PIDs are attached; a start that attaches no PID fails afterwards, keeping the new ID | `scope.c` 415–444, 460–487 |
| A new random identity per invocation | yes: each start draws a fresh random 128-bit value; nothing reuses one | as above |
| A same-name replacement | another `Unit` object, started separately: its own random ID. A transient start requires a pristine unit (no job, no fragment), so it never inherits one | `dbus-manager.c` 997–1055; `unit.c` 5153–5170 |
| Zero or absent | null until the unit's first start; the property then encodes as an empty `ay` (else exactly 16 bytes) | `bus-get-properties.c` 57–72; `dbus-unit.c` 940 |
| Can it change during one invocation | no: it is set only by a start path (`*_start`/`*_enter_running` of each unit type) or restored unchanged by deserialization after a daemon-reload or reexec | the `unit_acquire_invocation_id` callers; `unit-serialize.c` 478–490 |
| Can a caller choose or forge it | no: it is not a settable property; StartTransientUnit and SetUnitProperties refuse an unknown property ("Cannot set property %s, or unknown property") | `dbus-unit.c` 2490–2497 |
| Transient scopes specifically | as above: `scope_start()` (a transient scope only) realizes the cgroup, then `scope_enter_running()` acquires the ID, attaches the requested PIDs and enters RUNNING | `scope.c` 415–487 |
| Is it written on the cgroup | not by a user manager: `cgroup_xattr_apply()` returns before `cgroup_invocation_id_xattr_apply()` unless the manager is the system manager. The kernel's directory therefore carries no instance identity here | `cgroup.c` 1043–1057 |
| The directory instance | `ControlGroupId` (`t`): the kernel cgroup ID of the unit's directory, read by `name_to_handle_at()` at each realization (the same file-handle ID a retained descriptor yields) | `dbus-unit.c` 1558; `cgroup.c` 2495–2510; `cgroup-util.c` 1441–1455 |

A later `GetUnit(name)` → `InvocationID` read (what `systemd-run` does
after its job, `run.c` 1242–1283) is not bound to the request: by then the
name may hold another unit. It is never used to capture an identity.

## Section 11: binding the identity to the exact Start

StartTransientUnit's success reply carries the job path `J` of the start
job this request queued. `J` binds the request to the unit object, and the
identity is captured while `J` is still installed:

1. A delivered success reply means the unit was pristine (no job, no
   fragment) and was made transient by this very request
   (`dbus-manager.c` 1022–1050), and `J` was installed on it
   (`dbus-unit.c` 1825–1841).
2. While `J` is installed the unit cannot be collected (`unit_may_gc()`
   returns false for `u->job`, `unit.c` 446–447), so no other unit can
   hold the name, and a unit has one installed job at a time.
3. The reply is sent before the method handler returns
   (`dbus-unit.c` 1915–1919); jobs run later, from a defer event source at
   idle priority (`manager.c` 736–742, 2421–2441). `J` running is the
   unit's only start while `J` exists: it acquires the ID (null before:
   the unit was pristine and never started).
4. When the scope becomes active, `J` finishes `done`
   (`unit.c` 2652–2656). `job_uninstall()` sends `JobRemoved(J)` before it
   detaches `J` from the unit (`job.c` 160–183), and
   `bus_job_send_removed_signal()` first flushes the unit's pending change
   signal (`dbus-job.c` 299–311): a `PropertiesChanged` for the unit's
   object path on `org.freedesktop.systemd1.Unit` carrying every
   `EMITS_CHANGE` property by value (`dbus-unit.c` 1621–1669;
   `bus-objects.c` 2161–2189), `InvocationID` included (`dbus-unit.c` 940).
   That flushed signal is a `PropertiesChanged` only for a unit already
   announced; a unit never announced gets `UnitNew`, which carries no
   property (`dbus-unit.c` 1593–1620, 1664). The new unit is announced as
   soon as the manager's loop dispatches its D-Bus queue, which it does
   before it runs the next event, the job's run queue included
   (`manager.c` 3268–3290), unless more than 1024 messages are queued on its
   bus (`manager.c` 113, 2453–2475): then no identity is captured and the
   operation fails closed (below).
5. Ordering is established by the sender's own serials, not by delivery
   order: sd-bus assigns each sent message the next cookie in send order
   (`sd-bus.c` 1901–1940); the bus fills in SENDER authoritatively and
   relays the serial unchanged (D-Bus specification 1.14.10, SENDER;
   `dispatch.c` 350–366). So, from systemd's connection (the reply's
   SENDER), every `InvocationID` on the unit's path with a serial between
   the reply's and `JobRemoved(J)`'s was emitted while `J` was installed
   on this request's unit, and is the ID `J`'s start assigned.
6. Delivery to Nexus: `JobRemoved` reaches the requester through the job's
   tracking of it (`dbus-job.c` 299–311; `dbus.c` 1118–1149); the unit's
   change signal reaches the API bus only while some client is subscribed
   or holds a reference (`dbus.c` 1139–1146), so Nexus calls `Subscribe()`
   (`dbus-manager.c` 1367–1398), and adds match rules for exactly those
   two signals before issuing the request.

Delivery inside Nexus (zbus 4.4.0, the locked version): a match rule is
registered with the bus (`AddMatch`, awaited) before the request is sent;
the connection's socket reader hands each message, in the order read, to
every matching stream before it reads the next, waiting while a stream is
full (`connection/socket_reader.rs`), and Nexus reads both streams at once
while it waits for the removal, then drains the changes already queued. A
match on a well-known sender is not checked by zbus itself
(`match_rule/mod.rs`), so Nexus keeps only messages whose bus-set sender is
the reply's. Dropping a matched stream queues the removal of its match on
the connection's runtime, so the watch is dropped only inside that
runtime's context, unwinding included.

The capture rule (`scope::manager::captured_instance`): the start job's
`JobRemoved` result is `done`; its serial is above the reply's; among the
`PropertiesChanged` from the same sender on the unit's path with serials
strictly between the two, exactly one distinct, non-null, 16-byte
`InvocationID` appears. Anything else (no signal within the bound, another
result, no or several values, a serial outside the window, another sender,
a cookie wrap inside the window) captures nothing, and the operation fails
closed. A lost signal can only lose the identity, never substitute one.

## The resulting binding of a candidate

Cleanup ownership of the observed candidate (a cgroup v2 directory the
kernel reported the bound, unreaped helper in, retained by descriptor)
requires, for an accepted start whose identity was captured: GetUnit's
unit for the name has `InvocationID` exactly the captured 16 bytes, its
`Id` exactly the generated name, its `ControlGroup` exactly the path the
descriptor was opened from, its `ControlGroupId` exactly the descriptor's
own kernel cgroup ID, and `InvocationID` exactly the captured value again
after those reads. IDs are never reused, so the two identical reads show
the reads between them were answered for that one invocation. An uncertain
start has no reply, hence no job and no identity: it never owns a
candidate.

## What this relies on, and what qualifies it

- systemd behaviour above: primary source (v255.4, Ubuntu 255.4-1ubuntu8.17).
- That the new unit is announced before its start job runs: the manager's
  loop order (above); under a congested manager bus (more than 1024
  queued messages) it is not, no identity is captured, and the accepted
  start is never owned or proven (fail closed: the execution fails at its
  scope, and the operation is confirmed only once the manager has unloaded
  the unit).
- That the host's manager reports a 16-byte `InvocationID`, that the
  identity production captured equals it, and that `ControlGroupId` equals
  the descriptor's file-handle cgroup ID: the live host qualification's
  H3/H4 case (built here, never run).
- That a same-name replacement has another identity: primary source alone
  (a random value per start); no state-mutating live case is added for it.
