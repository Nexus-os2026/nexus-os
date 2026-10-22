# P2-V1-R3B-I4-R1: candidate-time primary-source excerpts, verbatim

Read from the host's installed manual pages (package `systemd`
255.4-1ubuntu8.17; `host-man.packages.txt`), by `zcat`, read-only; the
compressed pages are kept outside the repository at the SHA-256 in
`host-man.SHA256SUMS`. No systemd tool was run and no bus was contacted.
The roff markup is kept as installed in the `.roff` extracts beside this
file (`systemd.unit.5.gc-section.roff`: lines 618-712 of the page;
`systemd.unit.5.collectmode.roff`: lines 1148-1175).

## org.freedesktop.systemd1(5): what a delivered StartTransientUnit reply means

`baseline/primary/org.freedesktop.systemd1.v255-host.txt` (kept outside
the repository), lines 111-115, the method's signature:

>     StartTransientUnit(in  s name,
>                        in  s mode,
>                        in  a(sv) properties,
>                        in  a(sa(sv)) aux,
>                        out o job);

lines 786-788: "[...] mode is the same as in StartUnit() [...]", and of
StartUnit, lines 547 and 555-557:

> StartUnit() enqueues a start job and possibly depending jobs. [...] On
> reply, if successful, this method returns the newly created job object
> which has been enqueued for asynchronous activation.

So a delivered job path means the start job existed when the reply was
sent; GetUnit, sent only after that reply arrived, is processed after it.

## systemd.unit(5), "UNIT GARBAGE COLLECTION" (why a delivered start's later `NoSuchUnit` is evidence)

> The system and service manager loads a unit's configuration automatically
> when a unit is referenced for the first time. It will automatically unload
> the unit configuration and state again when the unit is not needed anymore
> ("garbage collection"). A unit may be referenced through a number of
> different mechanisms:
>
> 2. The unit is currently starting, running, reloading or stopping.
>
> 4. A job for the unit is pending.
>
> 7. The unit has running processes associated with it.

(Items 1, 3, 5 and 6 omitted: another unit's dependency, the failed state,
an IPC client's pin, perpetual units. The production request sets
`CollectMode=inactive-or-failed`, so a failed scope is unloaded too:
`crates/nexus-verifier-sandbox/src/scope/manager.rs`, `start_scope`.)

## systemd.unit(5), `CollectMode=`

> Tweaks the "garbage collection" algorithm for this unit. Takes one of
> inactive or inactive-or-failed. If set to inactive the unit will be
> unloaded if it is in the inactive state and is not referenced by clients,
> jobs or other units — however it is not unloaded if it is in the failed
> state. [...] This behaviour is altered if this option is set to
> inactive-or-failed: in this case the unit is unloaded even if the unit is
> in a failed state [...]

## org.freedesktop.systemd1(5), scope unit objects (the `ControlGroup` the binding reads)

> SCOPE UNIT OBJECTS
> All scope unit objects implement the org.freedesktop.systemd1.Scope
> interface (described here) in addition to the generic
> org.freedesktop.systemd1.Unit interface (see above).
>
> readonly s ControlGroup = '...';
>
> Properties
> All properties correspond directly with the matching properties of
> service units.

(`baseline/primary/org.freedesktop.systemd1.v255-host.txt`, kept outside
the repository: lines 4376-4378, 4403 and 4609.) For service units:

> ControlGroup indicates the control group path the processes of this
> service unit are placed in.

(line 2121.) `Id` (unit interface): "Id contains the primary name of the
unit." (line 1226; `baseline/primary/excerpts.md`.)

## What is not claimed

- No source here states that the user manager processes or answers one
  connection's calls in the order they were sent; I4-R1 relies on no such
  fence (`baseline/primary/excerpts.md`: the D-Bus Specification matches a
  reply to its call by `REPLY_SERIAL` and is designed for asynchronous
  operation; NetworkManager's "Notes on D-Bus": a response may overtake the
  response to an earlier call).
- `ControlGroupId` is listed but not described; it is not used.
- That `ControlGroup` equals `/proc/<pid>/cgroup`'s path for the same cgroup
  holds when the backend and the user manager share a cgroup namespace;
  this was not exercised on a host (G-HOST/G-LIVE).
