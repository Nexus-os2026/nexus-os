# P2-V1-R3B-I4-R2: uncertain start name-authority closure — evidence

This directory is the evidence of the repair commit that contains it, on
`review/p2-v1-native-scope-host-qualification`, on top of
`c3a12b2b54b50b5384c4a1d0cbb624615a4f4d27` (tree
`37289aef658249db3c36777ec235e2a72293ed99`, sole parent `3d46b4d…`: the
accepted Q1-R1 qualification repair). It repairs a production trust-boundary
defect in the I4-R1 settling of a pending scope operation. The I4-R1
acceptance is superseded for this invariant; nothing here re-declares I4
accepted. G-HOST and G-LIVE stay open; Phase Two remains active;
integration and host provisioning are not authorized.

## 1. Identity

- **Base**: `c3a12b2b…` (P2-V1-R3B-I4-Q1-R1). Its production code is the
  I4-R1 code (`ea475eac`), unchanged.
- **Candidate**: the commit containing this directory; subject
  "fix(verifier): close uncertain scope name authority".
- **Validated snapshot**: tree `ef085a1723f7add9b1ae34f5fe5a482897739409`, the worktree at the base
  with its 17 changed or new files
  (`scope/candidate-snapshot-tree.txt`, `scope/candidate-snapshot-files.SHA256SUMS`).
  The run outputs (`reproduction/`, `validation/`, `controls/`), the scope
  records, this report and `SHA256SUMS` were added after it; every other
  file of the candidate is byte-identical to it.
- **Changed paths** (`scope/changed-paths.txt`), all in the mission's
  envelope (section 20):
  - `crates/nexus-verifier-sandbox/src/scope/pending.rs` (production): the
    name authority in `reconcile` (`stop_authorized`, `observe`) and its
    documentation;
  - `crates/nexus-verifier-sandbox/src/execution/tests.rs`: `i4r2_01` to
    `i4r2_08`; `i4r1_01` (second part) and `i4r1_12` tightened;
  - `crates/nexus-verifier-sandbox/src/scope/tests.rs`: the model's lost
    collision reply and foreign process; `i4r2_x` (structural);
    `i4q1r1_nc2` tightened;
  - `docs/security/phase2-governed-verification.md`: the settling rule,
    the name authority, the authority inventory, the R2 test table, the
    residuals;
  - this directory.
- **Unchanged** (`scope/production-files.txt`): every other production
  source (`scope.rs`, `scope/manager.rs`, `scope/native.rs`,
  `execution.rs`, `launcher.rs`, `helper.rs`, `fault.rs`), the manifests,
  `Cargo.lock`, AGENTS.md, CLAUDE.md, the live workflow and its count
  (39), the live harness and its probe (the Q1-R1 H6 owner stays the
  production one), the desktop guards. No dependency change.

## 2. Root cause and reproduction

`matrices/root-cause.md`: the unrepaired `reconcile` stopped the unit by
its name for every operation not yet confirmed gone, an uncertain one
included; a collision whose `UnitExists` reply was lost is such an
operation, and its name is a foreign unit's. `reproduction/before-repair.out`:
the candidate's tests against the unrepaired production code (the base,
only the two test files replaced): 11 of the 12 fail, each on a stop by name without a recorded start reply (`i4r2_01`: "Timeout: the foreign unit was stopped by its name"), or, for the structural guard, on the absent authority check; `i4r2_06` (a delivered `UnitExists`) passes, as it did.

## 3. The repaired state machine

`matrices/state-machine.md`: accepted, collision, uncertain without a
candidate, uncertain with a candidate; exactly when a stop by name may and
may not occur; panic and ownership; the availability cost.

## 4. Non-live validation (mission section 21)

`validation/commands.txt`, on the validated snapshot, in order:
| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0, no warning (`-D warnings`) |
| `lib-tests` | exit 0, 131 passed (122 + the eight `i4r2` tests + the structural guard) |
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
| `lib-test-list` | exit 0, 131 tests |
| `desktop-guard-list` | exit 0, 11 guards |

The live harness was built, never run.

## 5. Controls

`matrices/controls.md`. R2 mutation controls (`controls/r2/summary.json`):
10 counted (8 required, 2 additional), every one applied once, compiled, failed its one intended test at its marker and restored byte for byte with the checkout's status clean, every intended test passing unmutated. Accepted Q1-R1 controls (`controls/q1r1/summary.json`):
15 counted, all as required, through the unchanged runner (SHA-256 verified), one retargeted (`Q1-R1-NC2-STOP-REPLY-PROOF` to `i4r2_05`, reason recorded). Accepted Q1 controls (`controls/q1/summary.json`):
10 counted, all as required, unchanged. Accepted suites (`controls/accepted/`): the 38 I4 and I4-R1 behavioural controls all as required through the unchanged runner (SHA-256 verified), two retargeted with the reason recorded (`NC-I4R1-STOP-REPLY-ABSENCE` to `i4r2_05`; `NC-I4-X-COLLISION-CLAIMED` to `i4r1_16`, after the first full rerun found it failing `i4_17` at its error classification instead of its marker: `controls/accepted/first-run-*`); the 27 harness-build API guards and the 9 normal-build API probes, each with its positive control; the 14 source guards with all 24 self-tests detected; the custody store's pinned hashes; R3's 179 store controls, the I2-R1 rerun (32 + 4 informational) and R3's 28 API probes identical to their accepted results; the checkout's status clean at all 12 checkpoints.

## 6. Live status

NOT RUN — HOST PREREQUISITE STILL BLOCKED. The historical run 37091121444
(3d46b4d) stopped at "Required host layers": `github-runner` (uid 1001) has
no `/run/user/1001`, no user manager bus and no memory, pids or cpu
delegation. Nothing was provisioned and the workflow was not dispatched.

## 7. Limits

- Everything here runs over the deterministic model of the user manager
  and the kernel; the host's systemd behaviour stays a live-gate fact.
- An uncertain start without a candidate is never confirmed: its execution
  is `CleanupFailed` and its helper an unreaped zombie for the life of the
  backend process (accepted availability cost).
- An accepted operation without a candidate may still be stopped by name:
  its recorded reply proves the manager created the unit for this request,
  not that a unit loaded under the name later is the same one (a party of
  the same uid that learned the name could load another after this
  backend's was unloaded). Binding the stop to the unit's own identity is
  not attempted.
- Candidate acquisition is the accepted I4-R1 mechanism, unchanged: a
  cgroup of the unit's name that the kernel reports the bound, unreaped
  helper in.
- Observation (unchanged, recorded only): the accepted live cases `p2c`
  `escapes_denied` and `p2g` `cargo_test` set process environment variables
  in the multi-threaded live harness (Q1-R1's H1 review).
