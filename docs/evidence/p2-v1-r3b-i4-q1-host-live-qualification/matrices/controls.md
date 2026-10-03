# P2-V1-R3B-I4-Q1: negative-control mapping (mission section 18)

| # | Mechanism | Control(s) | Kind | Where it runs |
|---|---|---|---|---|
| 1 | exact `Id` equality matters | `NC-I4R1-ID-MISMATCH` (accepted I4-R1, rerun unchanged); live H3 asserts `id == unit` byte for byte | mutation; live assertion | `accepted_reruns.sh`; the gate |
| 2 | exact `ControlGroup` equality matters | `NC-I4R1-CONTROLGROUP-MISMATCH`, `NC-I4R1-NO-CONTROLGROUP` (accepted); live H3 asserts `control_group == kernel` | mutation; live assertion | as above |
| 3 | same basename under another slice is insufficient | `NC-I4R1-CONTROLGROUP-BASENAME` (accepted); live H3 compares whole paths | mutation; live assertion | as above |
| 4 | wrong/relative/parent-traversing control-group paths are insufficient | `NC-I4R1-X-MEMBERSHIP-NOT-NORMAL`, `NC-I4R1-CONTROLGROUP-MISMATCH` (accepted); live H2/H3 assert absolute normal form | mutation; live assertion | as above |
| 5 | an unavailable or malformed property does not prove | `NC-I4R1-X-CONTROLGROUP-UNCERTAIN`, `NC-I4-PROPERTY-ERROR-DROPS` (accepted); live H3/H4 fail on any non-`s`/`t` value | mutation; live assertion | as above |
| 6 | `NoSuchUnit` is recognized only by the exact identity | `NC-Q1-NO-SUCH-UNIT-NAME`, `NC-Q1-ANY-ERROR-IS-ABSENT` (the production manager), `NC-Q1-PROBE-IDENTITY-DRIFT` (the live case's literal); live H5 | mutation; live assertion | `q1_controls.py`; the gate |
| 7 | `UnitExists` is recognized only by the exact identity | `NC-Q1-UNIT-EXISTS-NAME`, `NC-Q1-ANY-ERROR-IS-COLLISION`; live H6 | mutation; live assertion | as above |
| 8 | killed-but-unreaped membership is observed, not assumed | `NC-I4R1-REAP-UNRESOLVED`, `NC-I4-X-DROP-REAPS`, `NC-I4-PID-CLEANUP` (accepted: production never reaps an unresolved operation's helper and never takes its death as cleanup); live H7 reads `/proc/<pid>/cgroup` of the zombie repeatedly and fails if unreadable or other | mutation; live assertion (live only for the observation itself) | as above |
| 9 | no launch before the real binding is proven | `NC-I4-LAUNCH-PENDING`, `NC-I4R1-NO-CONTROLGROUP` (accepted); live `p2r1_live_panic_during_the_binding_proof_settles_it` and the stand-in launch tests (`i4r1_17`) | mutation; live assertion | as above |
| 10 | harness-only host probes cannot compile in a normal build | the probe is a `tests/` file included by the harness alone (`p2_g_10`: no production source or manifest names it); `NC-Q1-CGROUP2-LAYER-OPTIONAL` guards the host layer; the accepted normal-build probes (9) rerun show no new normal surface | static guard; mutation; compile probes | `q1_controls.py`; `accepted_reruns.sh` |
| 11 | a cleanup-observation failure cannot become "nothing left" | the accepted `phase2_cleanup_observation` tests (36) and the live step's fixture controls (`scripts/ci/test_phase2_cleanup_check.py`, 7), rerun with the new count; `p2_g_09` unchanged in substance | fixture controls | `validate.sh` |
| 12 | the live pass-count pin cannot silently omit a qualification case | `NC-Q1-LIVE-COUNT-STALE`, `NC-Q1-LIVE-CASE-DROPPED`, `NC-Q1-STEP-TEST-STALE`, `NC-Q1-CONTINUE-ON-ERROR` (`p2_g_10`: pinned count = 10 unscoped + the scoped array, every case by name, the fixture controls' count, no `continue-on-error`) | mutation | `q1_controls.py` |

Every mutation control hashes its files before, mutates exactly its
target, runs its one intended test alone, fails at its marker, restores
byte for byte and checks the checkout's Git status (`controls/`). The live
assertions are exercised only by the exact-SHA workflow; this evidence
records them as the gate's contract, not as a result.
