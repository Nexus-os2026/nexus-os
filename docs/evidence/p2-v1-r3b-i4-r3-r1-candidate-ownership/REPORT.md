# P2-V1-R3B-I4-R3-R1: candidate cleanup ownership — evidence

This directory is the evidence of the repair commit that contains it, on
`review/p2-v1-native-scope-host-qualification`, on top of
`44a4fc6c2f5e81536c07119362c1507f00448cd7` (tree
`ad8bf4f46043c84b4feb961560603d452b2fc4d0`, sole parent `2438f47f…`: the
accepted-in-principle P2-V1-R3B-I4-R3 repair). It separates a retained
candidate's identity from cleanup authority and from scope authority.
Nothing here re-declares I4 accepted. G-HOST and G-LIVE stay open; Phase
Two remains active; integration and host provisioning are not authorized.

## 1. Identity

- **Base**: `44a4fc6c…` (P2-V1-R3B-I4-R3).
- **Candidate**: the commit containing this directory; subject
  "fix(verifier): bind candidate cleanup ownership".
- **Validated snapshot**: tree `c46796f92a334bdcdf4e6c42be61c99b6940b39f`, the worktree at the base
  with its 32 changed or new files
  (`scope/candidate-snapshot-tree.txt`,
  `scope/candidate-snapshot-files.SHA256SUMS`). The run outputs
  (`reproduction/*.out`, `primary/`, `validation/`, `controls/`), the scope
  records, this report and `SHA256SUMS` were added after it; every other
  file of the candidate is byte-identical to it.
- **Changed paths** (`scope/changed-paths.txt`):
  - production: `crates/nexus-verifier-sandbox/src/scope/pending.rs` (the
    observed, owned and proven states; the binding; settling, the proof,
    the drop backstop and the promotion), `scope/manager.rs` (the
    start's identity captured from its own job; `Subscribe`; two property
    reads), `scope.rs` (the proven `Scope` holds its invocation; two
    harness-only read accessors; documentation), `fault.rs` (two fault
    points), and `scope/native.rs` (a retained directory's kernel cgroup
    ID, by file handle), which is outside the mission's section 22 list:
    the binding to the exact directory needs that ID from the descriptor
    itself, and only `native.rs` holds the descriptor (surfaced in
    `REPORT.md` section 8);
  - tests: `crates/nexus-verifier-sandbox/src/scope/tests.rs` (the model,
    structural guards, pure tests), `src/execution/tests.rs`
    (`matrices/test-changes.md`);
  - live harness: `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`
    (H3/H4 extended; built, never run; the count stays 39);
  - `docs/security/phase2-governed-verification.md`: the candidate rule,
    the identity, the binding, settling, the test table, the authority
    inventory, the residuals;
  - this directory.
- **Unchanged** (`scope/production-files.txt`): every other production
  source of the crate, its manifest and the workspace's, `Cargo.lock`,
  AGENTS.md, CLAUDE.md, the live workflow and its count (39), the live
  harness's probe (`tests/support/host_qualification.rs`), the desktop
  guards and verification module, the live step's fixture controls, and
  every custody source. No dependency change.

## 2. Root cause and reproduction

`matrices/root-cause.md`. On the base, `acquire` (and the proof's open)
killed a candidate as soon as the kernel reported the helper in a cgroup
of the unit's name. `reproduction/before-repair.out`
(`scripts/reproduce_candidate_cleanup.sh`,
`reproduction/candidate_cleanup_on_the_base.rs`, on the base's own model
and production code): a lost `UnitExists` whose foreign cgroup held the
helper and a process of its own was opened, proven, launched into and its
process ended (A1, A2, A3); a same-name replacement holding the helper was
ended by a retry (B1), and proven, launched into and ended while the proof
waited (B2). `reproduction/after-repair.out`
(`scripts/verify_after_repair.sh`, the same module against the candidate):
both tests fail at their own "not reproduced" assertions:

```
cargo test exit: 101 (expected 101: both tests fail)
A fails at its own assertion ("A1 not reproduced"): yes
B fails at its own assertion ("B1 not reproduced"): yes
test result line: test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 153 filtered out; finished in 1.07s
execution/tests.rs sha256 after restore: a6a972b42e3c39f82e4be6851e9ae1b25c02646aff9fe6d8a918df2ae68553bb (identical: yes)
checkout status entries after restore: 0
```

## 3. The unit-instance identity (primary sources)

`matrices/unit-instance-identity.md`, `primary/systemd-255.4-excerpts.txt`
(`scripts/primary_excerpts.sh`; Ubuntu's exact systemd 255.4-1ubuntu8.17
source with its patches applied, upstream v255.4, the D-Bus specification
and dbus-daemon at 1.14.10). `InvocationID` is a fresh random 128-bit
value per start, never caller-settable, null until the first start; a
same-name replacement is another unit with its own. The start's own job
binds the request to the unit while the identity is captured: a safe
binding exists, so the "BLOCKED — NO SAFE UNIT-INSTANCE IDENTITY" stop does
not apply.

## 4. The state machine and the manager surface

`matrices/state-machine.md` (observed, owned and proven candidates; the
transitions; what ends what; the panic points) and
`matrices/manager-surface.md` (the nine requests, `Subscribe` added; the
pins).

## 5. Non-live validation (mission section 23)

`validation/commands.txt`, on the validated snapshot, in order:

| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0, no warning (`-D warnings`) |
| `lib-tests` | exit 0, 153 passed |
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
| `lib-test-list` | exit 0, 153 tests |
| `desktop-guard-list` | exit 0, 11 guards |

The live harness was built, never run.

## 6. Controls

`matrices/controls.md`.
- R3-R1 mutation controls (`controls/r3r1/summary.json`): 11 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted R3 controls (adapted) (`controls/r3/summary.json`): 10 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted R2 controls (adapted) (`controls/r2/summary.json`): 5 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted Q1-R1 controls (adapted) (`controls/q1r1/summary.json`): 13 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted Q1 controls (`controls/q1/summary.json`): 10 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted I4 and I4-R1 controls (adapted) (`controls/accepted/i4r1-scope-controls.summary.json`): 34 counted, all as required: True, every intended test passing unmutated: True, files identical after: True, failures: none.
- Accepted I4-R1 API/type guards (adapted, `controls/accepted/i4r1-api-guards.summary.json`): 27 guards, all as required: True, positive control: True, sources identical after: True.
- Accepted I4-R1 source guards (adapted, `controls/accepted/i4r1-source-guards.json`): 16 guards, all pass: True, 41 self-tests, all detected: True.
- Accepted I4-R1 normal-build API guards (unchanged, `controls/accepted/i4r1-normal-api-probes.summary.json`): 9 guards, all as required: True, positive control: True.
- Custody hashes, accepted runners unchanged (`controls/accepted/`): RESULT: PASS.
- R3 store controls (179), accepted runners unchanged (`controls/accepted/`): RESULT: IDENTICAL.
- I2-R1 controls (32 + 4), accepted runners unchanged (`controls/accepted/`): RESULT: IDENTICAL.
- R3 API probes (28), accepted runners unchanged (`controls/accepted/`): RESULT: IDENTICAL.

The first, unadapted compatibility run (`controls/compat-run1/runs.txt`):

| Runner (as R3 accepted it) | Unadapted result on the candidate |
|---|---|
| R3 controls | stops at its first missing anchor: NC-R3-01-STOPUNIT-IN-RECONCILE |
| R2 controls (R3's adapter) | stops at its first missing anchor: NC-R2-03-LOST-COLLISION-CONVERTED |
| Q1-R1 controls (R3's adapter) | 12 of 13 as required; not detected: Q1-R1-NC1-FOREIGN-KILLED |
| Q1 controls | 10 of 10 as required; not detected: none |
| I4 and I4-R1 controls (R3's adapter) | stops at its first missing anchor: NC-I4-START-TIMEOUT-ABSENT |
| I4-R1 source guards (R3's adapter) | stops at a self-test anchor R3-R1 moved (`group == path`) |
| I4-R1 API/type guards | stops at P-I4-PENDING-CANDIDATE's reopen anchor (`Option<Box<dyn CgroupDir>>`) |
| I4-R1 normal-build API guards | 9 guards, all as required: True |

## 7. Live status

NOT RUN — HOST PREREQUISITE STILL BLOCKED. The historical run 37091121444
stopped at "Required host layers": `github-runner` (uid 1001) has no
`/run/user/1001`, no user manager bus and no memory, pids or cpu
delegation. Nothing was provisioned and the workflow was not dispatched.
The extended H3/H4 case (the identity's type, length and equality with what
production recorded; `ControlGroupId`; the binding across the reads) awaits
that gate.

## 8. Limits and decisions surfaced

- Everything here runs over the deterministic model of the user manager
  and the kernel; the real manager's signal delivery and identity values
  are a live-gate fact (H3/H4), never relied on for safety: when they are
  absent the operation fails closed.
- `Subscribe`: a manager request that acts on no unit (the manager sends
  this connection its signals), added so that a start can capture its own
  identity; the request allowlist pins it.
- `scope/native.rs` changed (outside the section 22 list): a retained
  directory's kernel cgroup ID, read by file handle from the descriptor, so
  the binding is to that exact directory, not to a path.
- An uncertain start, even one that created its scope, is never ended,
  proven or confirmed (accepted availability cost, mission section 13); an
  accepted start without a captured identity is confirmed only by the
  manager's absence.
- The binding follows systemd's own record of the unit's directory
  (`ControlGroupId`, refreshed at each realization): a same-uid party able
  to replace the directory and make the manager realize it again could
  make that record name its own directory; such a party can already signal
  the helper.
- Under a congested manager bus (more than 1024 queued messages) the new
  unit is not announced before its job runs, no identity is captured, and
  the accepted start fails closed.
