# P2-V1-R3B-I4-Q1: host / live qualification matrix

Each G-HOST fact (mission section 8), the live case that establishes it on
the supported host, what the case asserts, what it prints as bounded
evidence, and what it owns and ends. The cases run inside the existing
exact-SHA live gate (`.github/workflows/ci-phase2-linux-sandbox.yml`),
after the checked cleanup observation and before it again; the gate's
pinned count is 39 (31 accepted cases and these 8). The result of a case is
the workflow run's, never this document's: nothing here claims a live pass.

| Fact | Case | Asserts on the host | Printed evidence | Owns and ends |
|---|---|---|---|---|
| H1 real manager identity | `p2q_live_h1_the_manager_is_the_real_uid_s_user_bus` | `/run/user/<uid>` is a 0700 directory of the uid on a tmpfs; `/run/user/<uid>/bus` is a socket of the uid; it is the same socket (device, inode) the checked cleanup observation reaches; `ScopeManager::connect()` succeeds with a bogus `DBUS_SESSION_BUS_ADDRESS` in the environment; `connect_at` (harness) of another path is `BusUnavailable` | uid, bus path, owner, mode | nothing created |
| H2 unified cgroup v2 model | `p2q_live_h2_the_helper_s_membership_is_one_unified_cgroup_v2_line` | the real helper placed through `execution::place` has a `/proc/<pid>/cgroup` of exactly one line `0::<path>`, absolute, normal form, last component the generated unit; `/sys/fs/cgroup` is cgroup2fs; that cgroup's `cgroup.procs` lists the helper | pid, the raw membership line, the unit | the placement: `cgroup.kill`, reap, empty, owner confirmed |
| H3 manager/native binding | `p2q_live_h3_h4_the_manager_s_unit_binds_to_the_kernel_s_cgroup` | from the same live helper and the same object `GetUnit(unit)` returned: `Id == unit` and `ControlGroup == kernel membership`, both byte for byte (no basename, suffix or canonical match), `ControlGroup` absolute and normal | unit, object path, `Id`, `ControlGroup`, kernel membership | the placement, as above |
| H4 runtime properties | the same case | `RuntimeMaxUSec` decodes as `t` and equals the policy's backstop in µs; `OOMPolicy` decodes as `s` and is `continue` | both values and types | as above |
| H5 exact NoSuchUnit | `p2q_live_h5_a_fresh_name_is_exactly_no_such_unit` | `GetUnit` of a fresh, unpredictable `nexus-verifier-q1h5-<32 hex>.scope` is refused with exactly `org.freedesktop.systemd1.NoSuchUnit` (a timeout, a transport failure or any other name fails the case) | the name, the error name and message | nothing created; the loaded verifier scopes unchanged |
| H6 exact UnitExists | `p2q_live_h6_an_existing_name_is_exactly_unit_exists` | a scope the harness creates under a fresh unique name, holding a helper it owns, is entered by that helper; a second `StartTransientUnit` of the name is refused with exactly `org.freedesktop.systemd1.UnitExists`; the harness kills and reaps its helper, issues `StopUnit` (reply recorded, never proof) and observes `GetUnit` answer `NoSuchUnit` within 10 s; the loaded verifier scopes are back to before | the unit, pid, job, the error name and message, the StopUnit reply | its helper and its unit; drop is defense only |
| H7 killed-but-unreaped membership | `p2q_live_h7_a_killed_unreaped_helper_keeps_its_membership` | after the helper entered its proven scope: `cgroup.kill`; the helper becomes a zombie child of the harness (unreaped); for 2 s its `/proc/<pid>/cgroup` stays readable and names the entered cgroup (plain or ` (deleted)`); the scope reads not populated; only then is it reaped, the scope observed removed, the owner confirming | pid, the entered path, how many reads were plain and how many deleted-marked, the occupancy after the kill | the placement |
| H7 and H3, the production path | `p2r1_live_panic_after_the_candidate_is_retained_settles_it` (`FaultPoint::ScopeCandidate`), `p2r1_live_panic_during_the_binding_proof_settles_it` (`FaultPoint::ScopeBinding`) | through `execution::run_with_fault`: a panic after the candidate is retained, and one while `Id`/`ControlGroup` are being bound, on the real host; the finalizer kills the helper, keeps it unreaped, settles the pending operation through the retained candidate, reaps; nothing launched; the tree gone, the helper reaped, no scope left (the accepted `p2r1` machinery) | the case's own assertions | the execution's owner |

G-LIVE (section 9): the accepted scoped cases already run the production
path on the host (`p2d_live_execution_runs_in_a_verified_scope` and every
`execution::run` case: StartTransientUnit, placement, the retained
descriptor, GetUnit, `Id`, `ControlGroup`, `RuntimeMaxUSec`, `OOMPolicy`,
Pending to Proven, launch only after the proof, cgroup and helper cleanup,
the observation after). A passing gate therefore means the manager/native
binding succeeded on the host for every one of them; H3 additionally shows
the compared values.

Fail-closed properties of the cases: a hybrid or v1 host yields no unified
line (H2 fails, and production's `Kernel::membership` returns none, so no
scope is ever proven); an unavailable, malformed or uncertain property
fails H3/H4 (and refuses production's proof); any error name but the exact
one fails H5/H6; a zombie whose membership is unreadable or names anything
else fails H7.
