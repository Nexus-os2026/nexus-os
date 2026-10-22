# P2-V1-R3B-I4-Q1: the exact-SHA live gate's contract

The authoritative live result is a run of
`.github/workflows/ci-phase2-linux-sandbox.yml`, dispatched on the ref
whose head is the candidate, with `candidate_sha` equal to that head; the
workflow itself refuses any other event, ref or SHA. It runs on the
self-hosted runner `[self-hosted, Linux, X64, nexus-local]` as the
runner's own user.

## Dispatch

    gh workflow run ci-phase2-linux-sandbox.yml --ref review/p2-v1-native-scope-host-qualification -f candidate_sha=<40-character head>

## What a green run establishes, step by step

1. the candidate SHA is well formed, the event is a dispatch, the workflow
   identity is this file, and `GITHUB_SHA` and `GITHUB_WORKFLOW_SHA` are
   the candidate;
2. the exact candidate is checked out, clean;
3. the runner's Rust 1.94.0 is used (nothing installed);
4. the required host layers exist, else the job fails there (x86_64;
   Landlock ABI >= 6; a private tmpfs `/run/user/<uid>` of the runner's
   uid with a user manager bus; memory, pids and cpu delegated to the
   user manager; the unified cgroup v2 hierarchy); host and version facts
   are printed as evidence;
5. the packaged verifier toolchain is assembled from pinned archives;
6. the sandbox unit tests pass (117, the error-identity guard included);
7. the cleanup-observation fixture controls pass (36 + 7);
8. the checked cleanup observation finds no verifier scope or workspace
   before the suite (else the suite is not run);
9. the live suite runs with every layer required and reports exactly
   `test result: ok. 39 live sandbox cases passed`: the 31 accepted cases
   and the 8 host-qualification cases (`host-live-qualification.md`);
   each command's status is kept, nothing continues on error;
10. the checked cleanup observation finds nothing left after the suite;
11. the Phase Two kernel and desktop controls pass (`p2_g_10` included).

## What a failure means

- a failure at step 4 is a missing host prerequisite: the mission's
  "BLOCKED — G-HOST prerequisite missing"; it is never provisioned by the
  gate or by the executor (no lingering change, no runner change, no sudo);
- a failure at step 8 or 10 is residue or an observation that cannot
  answer: nothing is swept or cleaned by name; it blocks;
- a failing case in step 9 is evidence: a host fact that does not hold
  (section 26) or a defect, to be diagnosed, never weakened.
