# Controls

## Q3-R1 negative controls (mission section 8): `controls/q3r1/summary.json`

`scripts/q3r1_controls.py`, run in an isolated clean snapshot of the
candidate (`controls/runs.txt`). Each control restores one wrong behaviour;
its intended test passes unmutated, compiles mutated, fails alone on the
control's own marker, and every file is restored byte for byte (hashes and
Git status checked). The live case is never run: its controls are detected
by the fixture target's structural check of its source.

| Mission item | Control | Mutation | Intended test | Marker |
|---|---|---|---|---|
| 1 | NC-Q3R1-01-MODEL-EMPTY-ENDS-NEVER-POPULATED | the model ends a scope on an empty notification merely because it is empty | `q3r1_01` (lib) | a never-populated scope ended on a cgroup-empty notification |
| 1 | NC-Q3R1-01B-MODEL-COLLECTS-RUNNING | the model's manager collects a unit that has not ended (immediate unload) | `q3r1_01` (lib) | a running scope was collected |
| 2 | NC-Q3R1-02-EXIT-OBSERVATION-REAPS | the exit observation drops `WNOWAIT` (it reaps) | `np_16` | the exit observation reaped the helper |
| 2 | NC-Q3R1-02B-CASE-REAPS-HELPER | the live case reaps the helper before its placement | `np_18` | the case's world does try_reap |
| 3 | NC-Q3R1-03-PRE-BACKSTOP-CONFIRMATION-ACCEPTED | a confirmation before the backstop is taken as the lifecycle | `np_03` | qualified a world that deviates from the lifecycle |
| 4 | NC-Q3R1-04-PLACEMENT-SUCCESS-ACCEPTED | a proven placement is accepted | `np_05` | qualified a world that deviates from the lifecycle |
| 5 | NC-Q3R1-05-CASE-BACKSTOP-REMOVED | the case's limits keep the production backstop | `np_15` | the case does not set its own runtime backstop |
| 5 | NC-Q3R1-05B-CASE-USES-HARNESS-LIMITS | the live case places with the harness's limits | `np_18` | never_populated::limits(limits()) |
| 6 | NC-Q3R1-06-UNBOUNDED-EXIT-WAIT | no bound on the exit wait | `np_08` | an unbounded wait |
| 6 | NC-Q3R1-06B-UNBOUNDED-COLLECTION-WAIT | no bound on the collection wait | `np_10` | an unbounded wait |
| 7 | NC-Q3R1-07-SLEEP-AS-EXIT-PROOF | a sleep for the whole bound stands in for the exit report | `np_09` | the exit was not waited for by its report |
| 8 | NC-Q3R1-08-NO-BASELINE-CHECK | the scopes' return to the baseline is not checked | `np_11` | qualified a world that deviates from the lifecycle |

The fixture model stops any wait past 600 s of virtual time ("an unbounded
wait"), so an unbounded mutation fails rather than hangs; it places a helper
whose exit was never reported (as a running one would be), so a sleep is
never proof of an exit.

## Accepted controls (mission section 12)

The accepted runners and adapters were each run unchanged. Their SHA-256 was
verified before import where the runner itself does so. Every suite ran on
the candidate as accepted, with no unadapted or compatibility run needed:
- this candidate changes no production file;
- the Q1 and Q1-R1 anchors in the live harness are intact.

| Suite (accepted) | Runner | Result on the candidate |
|---|---|---|
| R3-R1 controls (11) | `r3r1_controls.py` | 11 of 11 as required |
| R3 controls (10) | `accepted_r3_controls_r3r1.py` | 10 of 10 as required |
| R2 controls (5 counted) | `accepted_r2_controls_r3r1.py` | 5 of 5 as required |
| Q1-R1 controls (13 counted) | `accepted_r1_controls_r3r1.py` | 13 of 13 as required |
| Q1 controls (10) | `q1_controls.py` | 10 of 10 as required |
| I4 and I4-R1 controls (34 counted) | `accepted_scope_controls_r3r1.py` | 34 of 34 as required |
| I4-R1 API/type guards (27) | `accepted_api_guards_r3r1.py` | 27, positive control true |
| I4-R1 normal-build API guards (9) | `normal_api_probes.py` | 9, positive control true |
| I4-R1 source guards (16, 41 self-tests) | `accepted_source_guards_r3r1.py` | all pass, all self-tests detected |
| Custody hashes (19 files) | I4's script | PASS |
| R3 store controls (179) | R3's runner | IDENTICAL to R3's record |
| I2-R1 controls (32 + 4) | I3-I1's runner | IDENTICAL to the record |
| R3 API probes (28) | R3's runner | IDENTICAL to R3's record |
