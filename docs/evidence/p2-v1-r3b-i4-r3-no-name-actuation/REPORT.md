# P2-V1-R3B-I4-R3: no scope name actuation — evidence

This directory is the evidence of the repair commit that contains it, on
`review/p2-v1-native-scope-host-qualification`, on top of
`2438f47f1726ae4075a52878bec005b6047691f0` (tree
`d7171834dd00e4dcbd00de1ed4bcae82f0e5f124`, sole parent `c3a12b2b…`: the
accepted P2-V1-R3B-I4-R2 repair). It removes the last name-based actuation
from the settling of a pending scope operation. Nothing here re-declares I4
accepted. G-HOST and G-LIVE stay open; Phase Two remains active;
integration and host provisioning are not authorized.

## 1. Identity

- **Base**: `2438f47f…` (P2-V1-R3B-I4-R2).
- **Candidate**: the commit containing this directory; subject
  "fix(verifier): eliminate scope name cleanup authority".
- **Validated snapshot**: tree `2af04b5055bb90e0198217f1e358d66a0cd0fb6e`, the worktree at the base
  with its 25 changed or new files
  (`scope/candidate-snapshot-tree.txt`,
  `scope/candidate-snapshot-files.SHA256SUMS`). The run outputs
  (`reproduction/before-repair.out`, `primary/`, `validation/`,
  `controls/`), the scope records, this report and `SHA256SUMS` were added
  after it; every other file of the candidate is byte-identical to it.
- **Changed paths** (`scope/changed-paths.txt`), all in the mission's file
  scope (section 19) except `scope.rs` (below):
  - production: `crates/nexus-verifier-sandbox/src/scope/pending.rs` (the
    stop path, the name authority and its state removed from settling;
    documentation), `scope/manager.rs` (the trait's `stop_unit` and the
    production StopUnit request removed; documentation), `fault.rs` (the
    two stop fault points removed), and `scope.rs`, comments only, which is
    outside the mission's section 19 list: its module documentation and the
    settle bound's comments said StopUnit attempts happen, which section 12
    forbids leaving;
  - tests: `crates/nexus-verifier-sandbox/src/scope/tests.rs` (the model
    and the structural guards), `crates/nexus-verifier-sandbox/src/execution/tests.rs`
    (`matrices/test-changes.md`);
  - `docs/security/phase2-governed-verification.md`: the final rule, the
    settling rule, the test tables, the authority inventory, the residuals;
  - this directory.
- **An earlier full run** on tree `5f19c8347e6736bbdccb9bb7ff8eb9f841e8ec9d`,
  identical but for one docstring (`scripts/r3_controls.py` cited the
  mission's controls section as 13; it is 16), had every validation step,
  control run and accepted rerun pass with the same counts and the same
  control outcomes, and the same first compatibility run. Everything was
  repeated on the corrected snapshot so that every result here is of the
  committed files; the earlier outputs stay in the work directory.
- **Unchanged** (`scope/production-files.txt`): every other production
  source of the crate, its manifest and the workspace's, `Cargo.lock`,
  AGENTS.md, CLAUDE.md, the live workflow and its count (39), the live
  harness and its probes, the desktop guards and verification module, the
  live step's fixture controls, and every custody source. No dependency
  change.

## 2. Root cause and reproduction

`matrices/root-cause.md`: the base's `reconcile` stopped an accepted
operation's unit by its name whenever settling could not observe it gone;
the recorded start reply binds the unit created then, not one loaded under
the name later. `reproduction/before-repair.out` (`scripts/reproduce_name_reuse.sh`,
`reproduction/name_reuse_on_the_base.rs`, on the base's own model and
production code): after this request's unit was unloaded and a foreign one
loaded under the same name, a retry issued `StopUnit(<name>)` once, and the
foreign unit was unloaded, its cgroup killed and removed: "REPRODUCED: an
accepted operation stopped the foreign unit loaded under its name". The
host's manual offers no manager-native actuation by a non-reusable handle
(`primary/systemd-manual-excerpts.txt`; `matrices/root-cause.md`).

## 3. The repaired state machine and the manager surface

`matrices/state-machine.md` (nothing issued, collision, candidate,
accepted without a candidate, uncertain without a candidate; actuation,
exactly; panic and ownership; the availability cost) and
`matrices/manager-surface.md` (StopUnit completely removed; the pins).

## 4. Non-live validation (mission section 20)

`validation/commands.txt`, on the validated snapshot, in order:

| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0, no warning (`-D warnings`) |
| `lib-tests` | exit 0, 138 passed |
| `store-tests` | exit 0, 142 passed |
| `codec-tests` | exit 0, 21 passed |
| `core-tests` | exit 0, 47 passed |
| `cleanup-observation` | exit 0, 36 passed |
| `package-layout` | exit 0, 1 passed |
| `live-harness-build-only` | exit 0, built, never run |
| `live-step-fixture-controls` | exit 0, 7 passed |
| `desktop-check` | exit 0 |
| `desktop-clippy` | exit 0, no warning (`-D warnings`) |
| `desktop-phase2-guards` | exit 0, 11 passed |
| `lib-test-list` | exit 0, 138 tests |
| `desktop-guard-list` | exit 0, 11 guards |

The live harness was built, never run.

## 5. Controls

`matrices/controls.md` (every control, retarget and supersession, with its
reason). On the validated snapshot, each control applied once, compiled,
failed its one intended test at its marker and was restored byte for byte,
with the checkout's status clean after every runner; every intended test
passed unmutated.

| Suite | Runner | Result |
|---|---|---|
| R3 mutation controls (`controls/r3/`) | `scripts/r3_controls.py` | 10 of 10 as required (3 structural, 7 behavioural) |
| I4 and I4-R1 behavioural controls (`controls/accepted/i4r1-scope-controls.*`) | the accepted I4-R1 runner, unchanged (SHA-256 verified), through `scripts/accepted_scope_controls_r3.py` | 34 run, all as required (18 I4, 16 I4-R1; 1 retargeted to its renamed test, R2's adaptation kept); 4 superseded |
| R2 controls (`controls/r2/`) | the accepted R2 runner, unchanged (SHA-256 verified), through `scripts/accepted_r2_controls_r3.py` | 5 run, all as required (NC-R2-03 re-anchored, NC-R2-08 re-anchored and retargeted, NC-R2-X-ACCEPTED-ELSEWHERE retargeted); 5 superseded |
| Q1-R1 controls (`controls/q1r1/`) | the accepted Q1-R1 runner, unchanged (SHA-256 verified), through `scripts/accepted_r1_controls_r3.py` | 13 run, all as required; 2 superseded |
| Q1 controls (`controls/q1/`) | the accepted Q1 runner, unchanged | 10 of 10 as required |
| Source guards (`controls/accepted/i4r1-source-guards.*`) | the accepted I4-R1 script, unchanged (SHA-256 verified), through `scripts/accepted_source_guards_r3.py` | 15 guards pass (the 14 accepted, SG-I4-DESTINATION adapted; SG-I4R3-NO-NAME-ACTUATION added); 29 self-tests, all detected; every guard tested |
| Harness-build API/type guards | accepted, unchanged | 27 pass, positive control detected, checkout restored |
| Normal-build API probes | accepted, unchanged | 9 pass, positive control detected |
| Custody store hashes | accepted, unchanged | PASS, 19 files unchanged |
| R3 store controls | accepted, unchanged | 179, identical to the accepted results |
| I2-R1 rerun | accepted, unchanged | 32 + 4 informational, identical |
| R3 API probes | accepted, unchanged | 28, identical |

The 11 superseded controls (4 I4/I4-R1, 5 R2, 2 Q1-R1) each restore a use
of the removed StopUnit request or anchor on the removed name authority;
none can be applied without first restoring that request, which NC-R3-01
to NC-R3-03 control. Each is recorded with its reason and the R3 control
or test that now carries its invariant (`matrices/controls.md`, and each
adapter's first output line in its `run.out`).

The first compatibility run (`controls/compat-run1/`, `scripts/compat_run1.sh`):
the accepted runners exactly as P2-V1-R3B-I4-R2 accepted them, before any
adaptation, on a clean checkout of the snapshot. Each anchor check stopped
at its first anchor R3 removed (`NC-I4-STOP-OK-CONFIRMS`,
`NC-R2-01-UNCONDITIONAL-STOP`, `Q1-R1-NC2-STOP-REPLY-PROOF`) and the source
guards at their `stop_unit` self-test. The full runs: the I4/I4-R1 runner
passed its first six controls and stopped at `NC-I4-STOP-OK-CONFIRMS`; the
R2 runner stopped at `NC-R2-01-UNCONDITIONAL-STOP`; the Q1-R1 runner's
`Q1-R1-NC1-FOREIGN-STOPPED` did not compile (the removed `stop_unit`), its
next three controls passed, and it stopped at `Q1-R1-NC2-STOP-REPLY-PROOF`.
Their baselines (`controls/compat-run1/baselines.out`) found no test under
the six names R3 renamed or replaced (`i4_06`, `i4_07`, `i4_09`,
`i4r1_13`, `i4r2_05` and `i4r2_x`); every other baseline passed. No runner
left the checkout changed.

## 6. Live status

NOT RUN — HOST PREREQUISITE STILL BLOCKED. The historical run 37091121444
(3d46b4d) stopped at "Required host layers": `github-runner` (uid 1001) has
no `/run/user/1001`, no user manager bus and no memory, pids or cpu
delegation. Nothing was provisioned and the workflow was not dispatched.

## 7. Limits

- Everything here runs over the deterministic model of the user manager
  and the kernel; the host's systemd behaviour (when it unloads a unit, how
  long a replacement stays) stays a live-gate fact, never relied on for
  safety.
- An accepted operation without a candidate stays `CleanupFailed`, its
  helper an unreaped zombie, while a unit of its name is loaded (its own,
  or a foreign replacement); an uncertain one without a candidate,
  possibly for the life of the backend process (accepted availability
  cost).
- Candidate acquisition is the accepted I4-R1 mechanism, unchanged: a
  cgroup of the unit's name that the kernel reports the bound, unreaped
  helper in. A party able to move the backend's helper into its own cgroup
  of that path (same uid, within the delegated tree) would have that
  cgroup ended through its descriptor; such a party can already signal the
  helper.
- Observation (unchanged, recorded only): the accepted live cases `p2c`
  `escapes_denied` and `p2g` `cargo_test` set process environment variables
  in the multi-threaded live harness (Q1-R1's H1 review).
