# P2-V1-R3B-I4-Q1: supported-host and live native scope qualification — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-native-scope-host-qualification`, descended from the accepted
I4-R1 commit `ea475eac8c2a9b527bd1b6e1f6e7b47beede7203` (tree
`270cd428b872e35fe6597914388c01896c00d22b`, sole parent
`bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9`). The mission closes no gap by
itself: it adds, to the existing exact-SHA live gate, the cases that
establish on the supported host the facts the accepted scope mechanism
depends on (G-HOST H1 to H7) and makes the gate's pass depend on them
(G-LIVE). The authoritative live result is a run of that gate on the exact
candidate SHA, after this commit exists and is pushed; nothing here claims
one. Phase Two remains active and not complete; integration is not
authorized.

## 1. Identity

- **Base**: `ea475eac8c2a9b527bd1b6e1f6e7b47beede7203` (P2-V1-R3B-I4-R1,
  accepted), tree `270cd428…`, sole parent `bb0dfec0…`.
- **Candidate**: the commit containing this directory; subject
  "test(verifier): qualify native scope host boundary".
- **Validated snapshot**: tree `c91efc5bfb18cef128d660a93a12d87caa409d74`, the worktree at the base
  with its 20 changed or new files (`scope/candidate-snapshot-tree.txt`,
  `scope/candidate-snapshot-files.SHA256SUMS`). The run outputs under
  `validation/` and `controls/`, this report and `SHA256SUMS` were added
  after it; every other file of the candidate is byte-identical to it.
- **Changed paths** (`scope/changed-paths.txt`), all in the mission's
  envelope (section 16):
  - `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`: the eight
    host-qualification cases (`p2q`, two `p2r1`), the scoped array 21 -> 29;
  - `crates/nexus-verifier-sandbox/tests/support/host_qualification.rs`
    (new): the harness's bounded probe of the real user manager and kernel;
  - `crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs`:
    `UserBus::path` made public for the probe (one visibility word);
  - `crates/nexus-verifier-sandbox/src/scope/tests.rs`: the unit guard
    `i4q1_…` pinning the manager's exact error identities (test code only);
  - `.github/workflows/ci-phase2-linux-sandbox.yml`: the pinned count
    31 -> 39, the unified hierarchy as a required layer, host facts printed;
  - `scripts/ci/test_phase2_cleanup_check.py`: the step's fixture controls'
    count (required by the workflow change);
  - `app/src-tauri/src/phase2_tests.rs`: `p2_g_10` (new), `p2_g_09`'s pins;
  - `docs/security/phase2-governed-verification.md`: sections 15 and 18;
  - `docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/` (new).
- **No production file changed**: `scope.rs`, `scope/manager.rs`,
  `scope/pending.rs`, `scope/native.rs`, `execution.rs`, `launcher.rs`,
  the custody code, the manifests and `Cargo.lock` are byte-identical to the
  base (`validation/sources.SHA256SUMS` against the base's, and the accepted
  I4-R1 source guards rerun). No dependency was added.

## 2. What is qualified, and how

`matrices/host-live-qualification.md` maps H1 to H7 to the cases, their
assertions, their evidence and what they own; `primary/excerpts.md` checks
every asserted host contract against primary sources (systemd v255's
`bus-common-errors.h` and `dbus-manager.c`, Linux v6.17's
`kernel/cgroup/cgroup.c`, the host's `org.freedesktop.systemd1(5)`), kept
distinct from what is observed. In brief: H1 the real uid's user bus, the
same socket the checked observation reaches, the environment ignored; H2 one
unified `0::` membership line of the live helper, absolute and normal, on
cgroup2fs; H3 `GetUnit`'s object with `Id == unit` and `ControlGroup ==`
the kernel's membership, byte for byte, from the same helper; H4
`RuntimeMaxUSec` (`t`) and `OOMPolicy` (`s`) exact; H5 a fresh name refused
with exactly `NoSuchUnit`; H6 a harness-owned uniquely named scope refused
a second start with exactly `UnitExists`, then ended and observed gone; H7
a killed, unreaped helper's membership kept readable, plus the production
finalizer settling a retained candidate on the host. The accepted I4-R1
no-candidate rule is untouched; no ordering guarantee is added.

The probe (`tests/support/host_qualification.rs`) is harness-only by
construction: a `tests/` file included by the live harness alone, in no
library, named by no production source or manifest (`p2_g_10`), connecting
through the checked bus descriptor with EXTERNAL, every call bounded, an
error name returned as data. The normal build's surface is unchanged
(`controls/accepted/i4r1-normal-api-probes.summary.json`).

## 3. Non-live validation (mission section 19)

`validation/commands.txt`, on the validated snapshot, in order:
| Command | Result |
|---|---|
| `fmt-check` | exit 0 |
| `diff-check` | exit 0 |
| `clippy` | exit 0 |
| `lib-tests` | exit 0, 117 passed |
| `store-tests` | exit 0, 142 passed |
| `codec-tests` | exit 0, 21 passed |
| `core-tests` | exit 0, 47 passed |
| `cleanup-observation` | exit 0, 36 passed |
| `package-layout` | exit 0, 1 passed |
| `live-harness-build-only` | exit 0, built, never run |
| `live-step-fixture-controls` | exit 0, 7 passed |
| `desktop-check` | exit 0 |
| `desktop-phase2-guards` | exit 0, 10 passed (p2_g_10 included) |
| `lib-test-list` | exit 0 |

The live harness was built, never run, from the implementation shell.

## 4. Controls (mission section 18)

`matrices/controls.md` maps the twelve items. New Q1 mutation controls
(`controls/q1/summary.json`): 10 counted, every one applied once, compiled, failed its intended test at its marker and restored byte for byte with the checkout's status unchanged (NC-Q1-NO-SUCH-UNIT-NAME, -UNIT-EXISTS-NAME, -ANY-ERROR-IS-ABSENT, -ANY-ERROR-IS-COLLISION on the production manager, caught by the unit guard `i4q1_…`; NC-Q1-LIVE-COUNT-STALE, -LIVE-CASE-DROPPED, -STEP-TEST-STALE, -PROBE-IDENTITY-DRIFT, -CGROUP2-LAYER-OPTIONAL, -CONTINUE-ON-ERROR on the gate, the harness, the step test and the probe, caught by `p2_g_10`). Accepted suites rerun
unchanged (`controls/accepted/`): the 38 I4-R1 behavioural controls (21 I4 + 17 I4-R1), the 27 harness-build and 9 normal-build API guards and the 14 source guards (24 self-tests) all as required with files restored; the custody store's pinned hashes unchanged; R3's 179 store controls, the I2-R1 rerun (32 + 4) and R3's 28 API probes identical to their accepted results.

## 5. The live gate

`matrices/live-gate-contract.md`: how the gate is dispatched and what each
step establishes. The final mission report names the run (ID, attempt,
tested SHA, jobs) that was dispatched for this candidate. A failure of the
"Required host layers" step is a missing host prerequisite, never
provisioned by this mission.

## 6. Limits

- Nothing here was exercised on the supported host by this mission's
  implementation shell; the live cases run only in the exact-SHA gate.
- Item 8 of section 18 (the zombie's membership is observed, not assumed)
  is a live assertion plus the accepted I4-R1 reaping controls; no local
  mutation control can exercise the observation itself.
- The runner user's user manager (`/run/user/<uid>`, the bus, delegation)
  is a host prerequisite the gate checks and this mission never provides.
- G-HOST and G-LIVE close only when the gate passes on the runner; that is
  an Architect reading of the run, not a claim of this evidence.
