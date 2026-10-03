# P2-V1-R3B-I4-Q1: primary-source excerpts, verbatim

What the host-qualification cases assert is checked here against primary
sources, kept distinct from what is observed on the host. The retrieved
files are kept outside the repository at the SHA-256 in `retrieval.txt`
(upstream systemd v255 and Linux v6.17 sources, the versions installed on
the supported host: systemd 255.4-1ubuntu8.17, kernel 6.17.0-35-generic),
read-only; no systemd tool was run and no bus was contacted to collect
them.

## Generic D-Bus semantics (unchanged from I4-R1)

The D-Bus Specification matches a reply to its call by `REPLY_SERIAL` and
is designed for asynchronous operation; messages between two peers keep
their order, but a response to a method call may overtake the response to
an earlier call (`docs/evidence/p2-v1-r3b-i4-r1-native-scope/baseline/primary/excerpts.md`).
Nothing in I4-Q1 adds an ordering guarantee: the no-candidate rule for an
uncertain start stands.

## Documented systemd semantics (upstream source, tag v255)

`src/libsystemd/sd-bus/bus-common-errors.h`, lines 6 and 9: the exact error
identities the production manager reads as definite answers, and that H5
and H6 assert against the real manager:

```c
#define BUS_ERROR_NO_SUCH_UNIT                 "org.freedesktop.systemd1.NoSuchUnit"
#define BUS_ERROR_UNIT_EXISTS                  "org.freedesktop.systemd1.UnitExists"
```

`src/core/dbus-manager.c`, `bus_get_unit_by_name` (lines 482-484): GetUnit
of a name the manager has not loaded:

```c
                u = manager_get_unit(m, name);
                if (!u)
                        return sd_bus_error_setf(error, BUS_ERROR_NO_SUCH_UNIT, "Unit %s not loaded.", name);
```

`src/core/dbus-manager.c`, `transient_unit_from_message` (lines 1016-1018):
StartTransientUnit of a name already loaded:

```c
        if (!unit_is_pristine(u))
                return sd_bus_error_setf(error, BUS_ERROR_UNIT_EXISTS,
                                         "Unit %s was already loaded or has a fragment file.", name);
```

`org.freedesktop.systemd1(5)` (the host's manual, systemd 255.4): scope unit
objects implement `org.freedesktop.systemd1.Scope` beside the generic
`org.freedesktop.systemd1.Unit`; `Id` "contains the primary name of the
unit"; a scope's properties "correspond directly with the matching
properties of service units", and `ControlGroup` "indicates the control
group path the processes of this ... unit are placed in" (I4-R1's
`baseline/primary/excerpts.md`, lines 1226, 2121, 4376-4378, 4609 of the
rendered page). `RuntimeMaxUSec` is `t`, `OOMPolicy` is `s`.

## Documented kernel semantics (upstream source, tag v6.17)

`kernel/cgroup/cgroup.c`, `proc_cgroup_show` (lines 6460-6485): what H7
qualifies on the host, as the kernel documents it:

```c
		/*
		 * On traditional hierarchies, all zombie tasks show up as
		 * belonging to the root cgroup.  On the default hierarchy,
		 * while a zombie doesn't show up in "cgroup.procs" and
		 * thus can't be migrated, its /proc/PID/cgroup keeps
		 * reporting the cgroup it belonged to before exiting.  If
		 * the cgroup is removed before the zombie is reaped,
		 * " (deleted)" is appended to the cgroup path.
		 */
		if (cgroup_on_dfl(cgrp) || !(tsk->flags & PF_EXITING)) {
			retval = cgroup_path_ns_locked(cgrp, buf, PATH_MAX,
						current->nsproxy->cgroup_ns);
			...
			seq_puts(m, buf);
		} else {
			seq_puts(m, "/");
		}

		if (cgroup_on_dfl(cgrp) && cgroup_is_dead(cgrp))
			seq_puts(m, " (deleted)\n");
```

So, on the unified hierarchy: a killed, unreaped helper's
`/proc/<pid>/cgroup` keeps naming the cgroup it was in (H7's expectation),
with ` (deleted)` once the manager removed it (which I4's `names_unit`
reads as "outside", as modelled); a zombie no longer counts in
`cgroup.procs`, so the cgroup reads empty (which settling relies on). The
path is rendered in the reader's cgroup namespace
(`current->nsproxy->cgroup_ns`): the manager's `ControlGroup` and the
backend's `/proc/<pid>/cgroup` agree byte for byte only when both see the
same namespace, which H3 establishes on the host rather than assumes.

## What is observed on the host, not documented here

The actual values of `Id`, `ControlGroup`, the kernel membership, the
error names and messages, and the zombie's membership are printed by the
live cases as evidence in the exact-SHA workflow's log; they are never
authority. Nothing here was exercised on the supported host by this
mission's implementation shell.
