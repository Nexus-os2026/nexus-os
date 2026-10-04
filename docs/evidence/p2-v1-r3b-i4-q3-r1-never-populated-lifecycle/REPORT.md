# P2-V1-R3B-I4-Q3-R1: never-populated scope lifecycle — evidence

This directory is the evidence for the qualification-repair commit that
contains it. The commit is on `review/p2-v1-native-scope-host-qualification`,
on top of `d16d7ca30e9f6431c9399e6facd4bda26b4a1740`: the R3-R1 candidate
that failed live run 37163435032 at case 18.

The commit changes no production file. It corrects two things to match the
real systemd-v255 lifecycle of a scope whose cgroup was never populated:
- the deterministic model;
- live case 18.

It does not re-declare I4 accepted. G-LIVE stays open, and Phase Two remains
active. Integration and any live run require the Architect's authorization.

## 1. Identity

- **Base**: `d16d7ca3…` (P2-V1-R3B-I4-R3-R1, whose live run failed).
- **Candidate**: the commit that contains this directory. Its subject is
  "test(verifier): model never-populated scope lifecycle".
- **Validated snapshot**: tree `992fdeb2c36287ed78fa6ee32be7e8b6064f0284`.
  This is the worktree at the base, with every code, test, guard and
  documentation change of this candidate and its scripts
  (`scope/candidate-snapshot-tree.txt`,
  `scope/candidate-snapshot-files.SHA256SUMS`).
  - Added after the snapshot:
    - `matrices/`;
    - `scripts/production_files.sh` and `scripts/assemble.sh` (both evidence
      tooling: one reads Git, one copies run outputs);
    - the run outputs (`baseline/`, `primary/`, `validation/`, `controls/`);
    - the scope records, this report and `SHA256SUMS`.
  - Every other file of the candidate is byte-identical to the snapshot. No
    code, test, guard or documentation file changed after the snapshot.
- **Changed paths** (`scope/changed-paths.txt`):
  - model: `crates/nexus-verifier-sandbox/src/scope/tests.rs` and
    `src/execution/tests.rs` (`matrices/lifecycle-model.md`,
    `matrices/test-changes.md`);
  - live harness: `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`.
    Only case 18 changed. It was built and never run. The count stays 39.
  - new support: `tests/support/never_populated.rs`;
  - new fixture target: `tests/phase2_never_populated.rs`;
  - `app/src-tauri/src/phase2_tests.rs`: case 18's pins in `p2_g_09` only;
  - `docs/security/phase2-governed-verification.md`;
  - this directory.
- **Unchanged** (`scope/production-files.txt`): all 42 production sources and
  manifests are the same as the base, including:
  - the mission's six named files (`scope/manager.rs`, `scope/pending.rs`,
    `scope/native.rs`, `scope.rs`, `execution.rs`, `launcher.rs`);
  - `Cargo.lock`, AGENTS.md and CLAUDE.md;
  - the live workflow and its count (39), and the live step's fixture
    controls;
  - the probe (`tests/support/host_qualification.rs`) and the checked
    observation (`tests/support/cleanup_observation.rs`).

  No dependency changed.

## 2. The failed live baseline (preserved)

`matrices/baseline-failure.md`, `baseline/`.

| Item | Result |
|---|---|
| Run 37163435032 | exact candidate `d16d7ca3…` on the G-HOST runner |
| Cases 01–17 | passed |
| Case 18 | failed: "the refused scope is unconfirmed after 3 explicit attempts" |
| Cases 19–39 | not executed |

The case-18 scope ran until its 120 s `RuntimeMaxUSec` ("Scope reached
runtime time limit", failed 'timeout') and was then collected. No verifier
residue exists now.

There was no rerun and no second dispatch.

## 3. Root cause and the real lifecycle (primary sources)

`matrices/root-cause.md`, `primary/never-populated-excerpts.txt`
(`scripts/primary_excerpts.sh`). Sources:
- Ubuntu's exact systemd 255.4-1ubuntu8.17 source, with its patches applied;
- Linux v6.17 `kernel/cgroup/cgroup.c` (`primary/linux-v6.17.identity.txt`).

The diagnosis was accepted as P2-V1-R3B-I4-Q3. The chain:
1. A killed, unreaped helper still verifies as a pidref.
2. Its attach "succeeds".
3. The kernel skips the exiting task, so the scope starts and its cgroup is
   never populated.
4. No cgroup-empty notification is ever produced for that cgroup. The scope
   runs until its runtime backstop and is then collected.
5. Production's accepted operation without a candidate is confirmed only after
   that collection, and its helper is reaped only then.

The case and the model assumed the opposite. Production was right.

## 4. The model correction

`matrices/lifecycle-model.md`. A unit now ends only by one of three events:
- the cgroup-empty notification (only for a cgroup that was populated and
  then ran empty);
- its runtime backstop;
- another client's stop.

Only an ended unit is collected. `State::unload` is private. Every former
caller now goes through the lifecycle, and every accepted test keeps its
assertions. The mission's tests A–F are `q3r1_01` (A, B, E), `q3r1_02` (C, D,
E) and `q3r1_03` (F).

## 5. Live case 18

`matrices/case-18.md`. The case drives `never_populated::qualify` through
production's `place` and `RetainedBoundary::retry`:
- The helper is killed by `SIGKILL`. Its exit is established by its own
  report: `waitid(WEXITED | WNOHANG | WNOWAIT)`, polled within 10 s. No sleep
  is used as proof.
- Placement never succeeds.
- Retries before the backstop stay unconfirmed, the helper stays unreaped and
  the scope stays loaded. This is expected.
- The scope is collected at its own backstop (45 s) and no earlier.
- A bounded retry then confirms.
- Production reaps the helper only then.
- The scopes return to the baseline.

Every wait is bounded. Failures carry the diagnostics the mission asked for,
from production's existing `Debug` only; there is no new introspection.

**Backstop value.** The case uses its own backstop of 45 s, not about 5 s, and
`Plan::coherent` checks it. 45 s is the smallest round value that leaves room,
before the backstop, for all of:
- production's placement and settling (10 s + 10 s);
- one full explicit retry (10 s);
- a 5 s margin.

A 5 s backstop would collect the scope during production's own placement, so
no retry before the backstop would be left to qualify. The global default
(660 s) and the harness's `limits()` (120 s) are unchanged.

## 6. Non-live validation (mission section 12)

`validation/commands.txt`, run on the validated snapshot in order:

| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0, no warning (`-D warnings`) |
| `lib-tests` | exit 0, 156 passed (153 accepted + 3) |
| `store-tests` | exit 0, 142 passed |
| `codec-tests` | exit 0, 21 passed |
| `core-tests` | exit 0, 47 passed |
| `cleanup-observation` | exit 0, 36 passed |
| `package-layout` | exit 0, 1 passed |
| `live-harness-build-only` | exit 0, built, never run |
| `never-populated-fixtures` | exit 0, 18 passed |
| `live-step-fixture-controls` | exit 0, 7 passed |
| `desktop-check` | exit 0 |
| `desktop-clippy` | exit 0, no warning (`-D warnings`) |
| `desktop-phase2-guards` | exit 0, 11 passed |
| `lib-test-list` | exit 0, 156 tests |
| `desktop-guard-list` | exit 0, 11 guards |
| `never-populated-fixture-list` | exit 0, 18 tests |

The live harness was built and never run.

## 7. Controls

`matrices/controls.md`.

Each suite ran in an isolated clean snapshot of the validated tree
(`controls/runs.txt`, `controls/accepted/runs.txt`), with Git status 0 before
and after every runner:

- Q3-R1 negative controls, mission section 8 (`controls/q3r1/summary.json`):
  - 12 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted R3-R1 controls, R3-R1's runner unchanged (`controls/r3r1/summary.json`):
  - 11 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted R3 controls, R3-R1's adapter unchanged (`controls/r3/summary.json`):
  - 10 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted R2 controls, R3-R1's adapter unchanged (`controls/r2/summary.json`):
  - 5 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted Q1-R1 controls, R3-R1's adapter unchanged (`controls/q1r1/summary.json`):
  - 13 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted Q1 controls, their runner unchanged (`controls/q1/summary.json`):
  - 10 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted I4 and I4-R1 controls, R3-R1's adapter unchanged
  (`controls/accepted/i4r1-scope-controls.summary.json`):
  - 34 counted; all as required: True;
  - every intended test passing unmutated: True;
  - files identical after: True; failures: none.
- Accepted I4-R1 API/type guards, R3-R1's adapter unchanged
  (`controls/accepted/i4r1-api-guards.summary.json`):
  - 27 guards; all as required: True;
  - positive control: True; sources identical after: True.
- Accepted I4-R1 normal-build API guards
  (`controls/accepted/i4r1-normal-api-probes.summary.json`):
  - 9 guards; all as required: True;
  - positive control: True.
- Accepted I4-R1 source guards, R3-R1's adapter unchanged
  (`controls/accepted/i4r1-source-guards.json`):
  - 16 guards; all pass: True;
  - 41 self-tests; all detected: True.
- Custody hashes (`controls/accepted/custody-hashes.txt`): 19 files;
  RESULT: PASS.
- R3 store controls (`controls/accepted/r3-store-comparison.txt`): 179;
  RESULT: IDENTICAL.
- I2-R1 controls (`controls/accepted/i2r1-comparison.txt`): 32 + 4;
  RESULT: IDENTICAL.
- R3 API probes (`controls/accepted/r3-api-comparison.txt`): 28;
  RESULT: IDENTICAL.

No accepted runner or adapter needed a further adaptation, and none was
changed. This candidate changes no production file. The Q1 and Q1-R1
controls that mutate the live harness still find every anchor they need,
because case 18 kept them.

## 8. Live status

NOT RERUN — ARCHITECT AUTHORIZATION REQUIRED.

No workflow was dispatched by this mission. The host was not modified:
- the G-HOST runner (id 21, `nexus-github-runner.service` in the user manager)
  stayed online and idle;
- the old system unit stayed disabled and inactive;
- no sudo, service, lingering, user-manager or cgroup change was made;
- the preserved rev-4 migration WIP was left exactly as it was.

Disclosure: a read-only host check before the commit included
`sudo -n true`. It authenticated nothing, ran nothing privileged and changed
nothing. The journal records it as `sudo[784584]: nexus : a password is
required ; COMMAND=/usr/bin/true` at 2026-10-04T02:04:01Z. The mission
forbids sudo, so this is reported as a deviation. No other sudo invocation
was made.

## 9. Limits and decisions surfaced

- **Availability cost.** In production (660 s backstop), a refused placement
  of an already-exited helper keeps its operation unconfirmed, and its zombie
  helper unreaped, for up to about 11 minutes. This availability cost is
  already accepted (R3-R1: no name actuation; uncertain or candidate-less
  operations confirmed only by absence). It is now documented as a residual
  in `docs/security/phase2-governed-verification.md`. A shorter production
  backstop would need its own mission.
- **Backstop value.** The case uses its own 45 s backstop instead of the
  mission's preferred value of about 5 s (section 5).
- **Model fidelity.** The model is not a systemd simulator. It is only as
  faithful as the invariant needs. `r3r1_02` lets a foreign cgroup take a
  killed helper, which the real kernel cannot do (`cgroup_migrate_add_task`);
  that world is stricter than the real one and was kept unchanged.
- **Kernel source.** The kernel excerpts are upstream v6.17, not Ubuntu's
  6.17.0-35 package source. The cited functions are the upstream ones; the
  live journal of run 37163435032 agrees with them.
- **Lifecycle timing.** The real lifecycle timing (the collection at about the
  backstop) is proven only live. The next exact-SHA live run needs the
  Architect's authorization.
