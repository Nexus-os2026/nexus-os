# P2-V1-R3B-I4-Q1-R1: host qualification collision ownership closure — evidence

This directory is the evidence of the repair commit that contains it, on
`review/p2-v1-native-scope-host-qualification`, on top of the Q1
qualification candidate `3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823` (tree
`d2bafa9b81edca6d5b07c3a23d4f2546404abede`, sole parent
`ea475eac8c2a9b527bd1b6e1f6e7b47beede7203`, the accepted I4-R1). It repairs
the Architect's finding on Q1's collision qualification (H6) and reviews
H1's process-wide environment change. It is a test and harness repair: no
production file changes. Q1's own evidence
(`docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/`) is unchanged and
stays the record of 3d46b4d, whose live workflow run 37091121444 stopped at
its host prerequisite (the runner's user manager is absent) and ran no live
case. This mission dispatched no live run. Phase Two remains active;
integration and host provisioning are not authorized.

## 1. Identity

- **Base**: `3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823` (P2-V1-R3B-I4-Q1),
  tree `d2bafa9b…`, sole parent `ea475eac…`.
- **Candidate**: the commit containing this directory; subject
  "fix(verifier): retain qualification collision ownership".
- **Validated snapshot**: tree `7008c9dd074160fbb8118b3469413e25280e14c7`, the worktree at the base
  with its 17 changed or new files
  (`scope/candidate-snapshot-tree.txt`, `scope/candidate-snapshot-files.SHA256SUMS`).
  The reproduction, the run outputs under `validation/` and `controls/`, the
  scope records, this report and `SHA256SUMS` were added after it; every
  other file of the candidate is byte-identical to it.
- **Changed paths** (`scope/changed-paths.txt`), all in the mission's
  envelope (section 12):
  - `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`: H6 on the
    production owner; old H6's `Existing` owner removed; H1 without the
    environment change;
  - `crates/nexus-verifier-sandbox/tests/support/host_qualification.rs`:
    the start without processes; `StopUnit`, the process-carrying start and
    `is_no_such_unit` removed;
  - `crates/nexus-verifier-sandbox/src/scope/tests.rs` (test code only):
    `i4q1r1_nc1` to `nc4` and the H1 pin `i4q1r1_connect_…`;
  - `app/src-tauri/src/phase2_tests.rs`: the guard `p2_g_11`;
  - `docs/security/phase2-governed-verification.md`: the qualification's
    section 15 rows for H1 and H6, the R1 paragraph, one residual in
    section 18;
  - this directory.
- **Unchanged** (`scope/production-files-unchanged.txt`): every production
  source of the sandbox crate (`scope.rs`, `scope/*.rs` but the tests,
  `execution.rs`, `launcher.rs`, `helper.rs`, `fault.rs`), the manifests,
  `Cargo.lock`, AGENTS.md, CLAUDE.md, the live workflow (the pinned count
  stays 39), its step test and the observation support: byte-identical to
  3d46b4d, and the production sources also to ea475eac. No dependency
  change.

## 2. The finding, reproduced

`matrices/finding-reproduction.md`: old H6's code, how a first start
answered `UnitExists` made its owner stop the foreign unit by name (case A),
and how an uncertain or panicking first start made it reap the helper while
the request could still attach it (case B); both reproduced on the
deterministic model against 3d46b4d (`scripts/reproduce_old_h6.sh`,
`reproduction/`), with the repaired first start's behavior in the same
worlds for contrast.

## 3. The repair

`matrices/ownership-model.md`: H6's first start is the production owner's
(`execution::place`), with the accepted I4-R1 semantics for every outcome
(proven, collision, uncertain, panic, cleanup failure); the harness owns
nothing by a name and has no stop path; its one request carries no process
and no property and is sent only for the proven unit, which `primary/`
shows harmless whenever it is handled. Section 9's questions are answered
there one by one. `matrices/h1-environment-review.md`: why H1's environment
change was removed and what establishes the same fact.

## 4. Non-live validation (mission section 13)

`validation/commands.txt`, on the validated snapshot, in order:
| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0, no warning (`-D warnings`) |
| `lib-tests` | exit 0, 122 passed (117 + the five `i4q1r1` tests) |
| `store-tests` | exit 0, 142 passed |
| `codec-tests` | exit 0, 21 passed |
| `core-tests` | exit 0, 47 passed |
| `cleanup-observation` | exit 0, 36 passed |
| `package-layout` | exit 0, 1 passed |
| `live-harness-build-only` | exit 0, built, never run |
| `live-step-fixture-controls` | exit 0, 7 passed |
| `desktop-check` | exit 0 |
| `desktop-clippy` | exit 0, no warning (`-D warnings`) |
| `desktop-phase2-guards` | exit 0, 11 passed (`p2_g_11` included) |
| `lib-test-list` | exit 0, 122 tests |
| `desktop-guard-list` | exit 0, 11 guards |

The live harness was built, never run.

## 5. Controls (mission section 10)

`matrices/controls.md` maps NC1 to NC5 and H1 to their tests and controls.
R1 mutation controls (`controls/r1/summary.json`): 15 counted, every one applied once, compiled, failed its one intended test at its marker and restored byte for byte with the checkout's status clean, every intended test passing unmutated: 8 in the first start's owner and the production source, over the model (NC1-FOREIGN-STOPPED, NC1-FOREIGN-KILLED, NC2-UNRESOLVED-REAPED, NC2-STOP-REPLY-PROOF, NC3-PANIC-ESCAPES, NC3-PANIC-SPLITS, NC4-CLEANUP-UNOBSERVED, H1-CONNECT-ENV) and 7 in the live harness and its probe, through `p2_g_11` (NC1-H6-UNPROVEN-UNIT, NC2-HARNESS-RAW-REAP, NC3-HARNESS-OWN-OWNER, NC4-HARNESS-REAP-BEFORE-KILL, NC5-PROBE-STOPS-BY-NAME, NC5-PROBE-START-CARRIES-A-PROCESS, H1-ENV-MUTATION). Q1's
controls, rerun unchanged (`controls/q1/summary.json`): 10 counted, all as required, as for 3d46b4d.
Accepted suites, rerun unchanged (`controls/accepted/`): the 38 I4-R1 behavioural controls, the 27 harness-build API guards and the 9 normal-build API probes (each with its positive control), the 14 source guards (24 self-tests detected) and the custody store's pinned hashes (19 files) all as required; R3's 179 store controls, the I2-R1 rerun (32 + 4 informational) and R3's 28 API probes identical to their accepted results; the checkout's status clean at all 12 checkpoints.

## 6. Live status

NOT RERUN: the known host prerequisite remains unresolved. The historical
run 37091121444 (3d46b4d) stopped at "Required host layers": `github-runner`
(uid 1001) has no `/run/user/1001`, no user manager bus and no memory, pids
or cpu delegation. Dispatching the same workflow before that is provisioned
would repeat that failure; the mission prohibits it. The live H1 to H7
cases, H6 included, run only in that exact-SHA gate.

## 7. Limits

- Nothing here ran on the supported host; the exact live answers (H6's
  `UnitExists` included) remain live-gate facts.
- The systemd behavior relied on for the request without processes is
  upstream v255's (byte-identical in v255.4); Ubuntu's patches to
  255.4-1ubuntu8.17 were not inspected.
- The harness layer (H6, the probe) is pinned statically (`p2_g_11`): it
  runs only on the supported host. Its executable ownership is the
  production owner's, exercised over the model.
- Residual of the accepted production semantics, inherited, not introduced:
  a collision whose `UnitExists` reply is lost is settled as an uncertain
  start, whose settling attempts `StopUnit` for its own generated name
  (`matrices/ownership-model.md`, reproduction E). An Architect decision.
- An unresolved operation's helper stays unreaped for the life of the
  harness process only (as for the backend process in production).
- Observation: two accepted cases (`p2c` `escapes_denied`, `p2g`
  `cargo_test`) also change the live harness's environment, for another
  invariant; unchanged (`matrices/h1-environment-review.md`).
