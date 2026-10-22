# The failed live baseline: run 37163435032 (preserved, never rewritten)

`baseline/` holds the run's bounded extraction (`result.txt`, `run.json`,
`jobs.json`, `expected-cases.txt`), the complete job log's hash
(`run-log.sha256.txt`: 103,679 bytes,
`14ddc0e7aaf365be57e08f14d34654b0c1208aa8b9950aca618abc018885a39a`), its
host-layer and live-step lines (`live-step-excerpt.txt`), the user manager's
journal for the case-18 scope (`journal-excerpt.txt`) and the host's verifier
residue when this candidate was prepared (`host-residue.txt`), made by
`scripts/baseline.sh` (reads only).

| Fact | Value |
|---|---|
| Run | 37163435032, attempt 1, `workflow_dispatch` by `Nexus-os2026` (the host's `gh` identity), 2026-10-03T23:58:10Z |
| Candidate | `d16d7ca30e9f6431c9399e6facd4bda26b4a1740` on `review/p2-v1-native-scope-host-qualification` (this candidate's base) |
| Runner | id 21 `nexus-local-asus`, placed in `/user.slice/user-1001.slice/user@1001.service/app.slice/nexus-github-runner.service` (G-HOST, accepted) |
| Steps 1-11 | success: identity, exact checkout, toolchain, host layers, unit tests (153), cleanup observation controls (36 + 7) |
| Step 12 (live suite) | failure |
| Step 13 (kernel and desktop controls) | skipped |
| Cases 01-17 | passed |
| Case 18 `p2d_live_unmovable_process_fails_closed` | failed at 00:01:49.773Z: "the refused scope is unconfirmed after 3 explicit attempts" (`phase2_live_sandbox.rs:952`), 50.0 s after it began: production's 10 s placement wait, its 10 s settling, then 3 explicit retries of 10 s each, every one unconfirmed |
| Cases 19-39 (H1-H7 included) | not executed |
| After the suite | "a verifier scope was left behind: nexus-verifier-deb565108b334be56fbff5e9df044ead.scope" |
| The scope | started 00:00:59.754Z (its start job `done`: not refused); "Scope reached runtime time limit. Stopping." and "Failed with result 'timeout'" at 00:02:59.766Z, +120.012 s, the case's `runtime_backstop_secs: 120`; then collected (`CollectMode=inactive-or-failed`) |
| The helper | the test process exited by 00:01:53Z (the job's end), so its zombie helper was reaped; the scope outlived that by 66 s |
| Residue now | 0 `nexus-verifier*` cgroups; no journal line for the scope after its timeout; only runs 37163435032 and 37091121444 of the workflow exist |

No rerun and no second dispatch was made. The next exact-SHA live run of
this candidate requires the Architect's authorization.
