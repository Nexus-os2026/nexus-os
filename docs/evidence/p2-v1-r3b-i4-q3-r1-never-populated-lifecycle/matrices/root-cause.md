# Root cause (P2-V1-R3B-I4-Q3, accepted) and how this candidate encodes it

Classification (Architect): primary, a HARNESS ASSUMPTION DEFECT; contributing,
real systemd-v255 lifecycle behaviour missing from the deterministic model.
The R3-R1 production cleanup and authority architecture is accepted and
unchanged.

## The real lifecycle (primary sources: `primary/never-populated-excerpts.txt`)

1. Case 18 kills its helper and does not reap it before the placement (since
   P2-V1-R3B-I4-R1: the helper of an unresolved operation is never reaped).
2. StartTransientUnit resolves the helper's process id to a pidref when the
   request is handled (`dbus-scope.c`, PIDs); `pidfd_get_pid` fails with
   `-ESRCH` only once the process is reaped (`process-util.c`), so an exited,
   unreaped helper verifies (`pidref_verify`, `pidref.c`).
3. As the scope enters `running`, its processes are attached once
   (`scope_enter_running`, `scope.c`); it is refused ("No PIDs left to
   attach") only if none was attached. `unit_attach_pids_to_cgroup`
   (`cgroup.c`; Ubuntu's patch changes only paths and its bus fallback) counts
   a successful `cgroup.procs` write as attached.
4. The kernel finds the zombie (`cgroup_procs_write_start`: `-ESRCH` only for
   a missing task) and its migration skips an exiting task silently
   (`cgroup_migrate_add_task`, `PF_EXITING`); a migration of no task succeeds
   (`cgroup_migrate_execute`). The start succeeds; the helper stays where it
   was (`proc_cgroup_show` keeps naming its original cgroup).
5. An exited task never counts as populated (`cgroup_exit`,
   `css_set_move_task`), so the scope's cgroup is never populated.
6. On cgroup v2 the manager ends a scope for emptiness only on an `IN_MODIFY`
   of `cgroup.events` (`unit_watch_cgroup`, `unit_check_cgroup_events`; no
   synthesized event on the unified hierarchy:
   `unit_synthesize_cgroup_empty_event`), which the kernel sends only when
   `populated` changes (`cgroup_update_populated`). A never-populated scope
   is never ended that way.
7. It stays loaded and running until its `RuntimeMaxUSec` expires
   (`scope_dispatch_timer`: stopping, failed 'timeout'), then is collected.
8. Production never acts on a unit by its name (P2-V1-R3B-I4-R3) and never
   owns a cgroup the helper is not in (P2-V1-R3B-I4-R3-R1): its accepted
   operation without a candidate is confirmed only by `NoSuchUnit` with the
   helper outside, after that collection, and its helper is reaped only then.

## What was wrong, and what this candidate changes

| Where | The assumption | Now |
|---|---|---|
| Live case 18 | the refused scope is gone within its explicit retries and within 10 s more | the case qualifies the real lifecycle: unconfirmed before its own 45 s backstop (expected), confirmed after the collection, the helper reaped only then, the scopes back to the baseline (`case-18.md`) |
| The model (`src/scope/tests.rs`) | "systemd collects a unit once nothing is left in it": `State::unload` at any time | a unit ends only by the cgroup-empty notification (only a cgroup that was populated and ran empty produces it), its runtime backstop or another client's stop, and only an ended unit is collected (`lifecycle-model.md`) |
| Production | none found | unchanged (`scope/production-files.txt`: 42 files, every one the same as the base) |
