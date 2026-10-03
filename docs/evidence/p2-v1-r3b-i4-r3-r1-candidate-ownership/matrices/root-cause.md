# P2-V1-R3B-I4-R3-R1: root cause and reproduction

## The defect (base `44a4fc6c`, P2-V1-R3B-I4-R3)

`PendingScope::acquire` (and the proof's own open) treated the kernel's
report of the helper's cgroup as cleanup authority:

```
membership(bound helper) → a path whose last component is the unit name
→ open(path) → candidate.kill() → candidate retained
```

`reconcile` then killed the retained candidate again, the drop backstop
(`end_now`) killed it, and its emptiness confirmed the operation. The
helper's current membership proves only that this retained child is in that
cgroup now; it does not prove that this request created the cgroup or owns
what else is in it. Two ways a foreign unit's cgroup holds the helper with
the unit's own name:

- A. a foreign unit already holds the generated name; the request is
  refused as exactly `UnitExists` (no effect), the refusal is lost
  (`Started::Uncertain`), and the foreign unit's cgroup holds the helper
  beside a process of its own;
- B. the start is accepted, this request's unit is unloaded, and another
  client loads a unit of its own under the same name, at the same path,
  holding the helper and a process of its own.

R3 also proved an uncertain start like an accepted one, and every proof
compared only `Id` and `ControlGroup`, both of which a same-name unit has:
a foreign cgroup with the requested limits and properties could be proven
and launched into.

## Reproduction (before repair)

`scripts/reproduce_candidate_cleanup.sh` appends
`reproduction/candidate_cleanup_on_the_base.rs` to a scratch checkout of
the base (its own model and production code, nothing else changed), runs
its two tests once and restores the file byte for byte
(`reproduction/before-repair.out`, cargo exit 0, restore identical, status
clean). The foreign units' processes are model process ids: the model never
signals a real process.

| Case | What the base does |
|---|---|
| A1: lost `UnitExists`, the foreign cgroup holds the helper and process 4000101, matching limits and properties, through `execution::run` | the foreign cgroup opened once, the manager's proof read 4 times, **proven and launched into**; `cgroup.kill` through its descriptor once; process 4000101 **ended** |
| A2: the same world, through the harness owner (`execution::place`) | **proven** (a `ScopedHelper` owning the foreign cgroup); its end kills it |
| A3: the same world, the foreign unit's `OOMPolicy` not as expected | refused (`Mismatch("out-of-memory policy")`), then settling **ends** process 4000101 |
| B1: accepted, the helper never placed; this request's unit unloaded; a foreign unit loaded under the name holds the helper and process 4000102; retry | the retry opens the foreign cgroup, `cgroup.kill`s it and confirms: process 4000102 **ended** |
| B2: accepted; while the proof waits, the unit is replaced and the helper taken (another thread, as another client would) | the replacement **proven, launched into and ended** |

The output prints "REPRODUCED A…" and "REPRODUCED B…".

## After repair

`scripts/verify_after_repair.sh` runs the same module, unchanged, against
the candidate snapshot (`reproduction/after-repair.out`): both tests fail
at their own "not reproduced" assertions. A1–A3: the foreign cgroup is
never opened (an uncertain start has no identity), never ended, nothing is
proven or launched, process 4000101 untouched. B1–B2: the replacement's
cgroup is retained only as an observed candidate (opened by descriptor),
its identity is not the captured one, so it is never bound, ended, proven
or launched into; process 4000102 untouched.

## Primary-source answer

systemd v255 gives every start of a unit a fresh random `InvocationID`,
which no caller can set, and the start's own job lets Nexus capture the
identity of exactly the invocation its request began
(`matrices/unit-instance-identity.md`). So a safe unit-instance identity
exists: the mission's "BLOCKED — NO SAFE UNIT-INSTANCE IDENTITY" stop does
not apply, and the fail-closed fallback is used only where no identity was
captured (an uncertain start, or an accepted start whose capture failed).
