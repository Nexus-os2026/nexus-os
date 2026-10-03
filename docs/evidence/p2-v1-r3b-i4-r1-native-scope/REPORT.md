# P2-V1-R3B-I4-R1: native scope ownership closure — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-native-scope-ownership`. A file in a commit cannot name that
commit, so the evidence here is bound to two other things:

- **the baseline**, bound to the base
  `bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9` (tree
  `d92c819da744722701a1db103b70fc17f6d7f4c9`), reproduced in an isolated
  copy and an external crate, never in the review worktree;
- **the candidate's results**, bound to the exact source bytes they
  validated (`validation/candidate/sources.SHA256SUMS`, the evidence scripts
  included), run in a scratch Git checkout of a snapshot of the candidate's
  tree.

The rerun on the committed candidate is kept outside the repository, in
`/home/nexus/NEXUS/p2-v1-r3b-i4-r1-work/post-commit/`; the mission's final
report gives it with its hashes.

Phase Two remains active and not complete. Nothing here contacted a systemd
user manager, created a cgroup, ran the live sandbox, provisioned a host, or
used a runner.

## 1. Identity

- **Base**: `bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9` (P2-V1-R3B-I4,
  "fix(verifier): retain uncertain scope ownership"), tree
  `d92c819da744722701a1db103b70fc17f6d7f4c9`, sole parent
  `3805204f941d9694b2c0c36549d32a5f4e6c157c`.
- **Candidate**: the commit that contains this directory, on
  `review/p2-v1-native-scope-ownership`, whose sole parent is the base;
  subject "fix(verifier): close scope ownership escape paths".
- **The validated snapshot**: tree
  `e7a631270803a566ae0a175e3fc70bd452257774` (scratch commit
  `cca9cb7ad351b50da18774c63fa0fe1ed0df31bc`), made from the worktree at the
  base with its 38 changed or new files
  (`validation/candidate-snapshot.txt`; each file's SHA-256 in
  `validation/candidate-snapshot-files.SHA256SUMS`). This directory's run
  results, this report and `SHA256SUMS` were added after it; every other
  file of the candidate is byte-identical to the snapshot's.
- **Changed paths**, all inside the section 19 envelope
  (`validation/checks/scope.txt`):
  - `crates/nexus-verifier-sandbox/src/scope.rs`
  - `crates/nexus-verifier-sandbox/src/scope/manager.rs`
  - `crates/nexus-verifier-sandbox/src/scope/pending.rs`
  - `crates/nexus-verifier-sandbox/src/scope/tests.rs`
  - `crates/nexus-verifier-sandbox/src/execution.rs`
  - `crates/nexus-verifier-sandbox/src/execution/tests.rs`
  - `crates/nexus-verifier-sandbox/src/fault.rs`
  - `crates/nexus-verifier-sandbox/src/launcher.rs`
  - `docs/security/phase2-governed-verification.md`
  - `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs` (narrowly:
    the two direct-scope callers, and the explicit settling of a failed
    placement)
  - `app/src-tauri/src/phase2_tests.rs` (narrowly: guard additions in
    p2_g_01 and p2_g_06, and p2_g_09's reviewed retry-site pin)
  - `docs/evidence/p2-v1-r3b-i4-r1-native-scope/` (new)
- `scope/native.rs`, `helper.rs`, the protocol, seccomp, Landlock,
  workspace, toolchain, profiles and policies, packaging, workflows, the
  manifests, `Cargo.lock`, `AGENTS.md`, `CLAUDE.md`, `.gitignore`, every
  `.gitattributes`, the custody store's code and tests, the desktop's
  production coding flow, and all earlier evidence (the I4 directory
  included) are byte-identical to the base (`validation/checks/scope.txt`,
  `validation/checks/historical-manifests.txt`). No dependency was added.

## 2. The findings at the base (B-R1-1 to B-R1-7)

All seven were reproduced at the exact base before any repair (`baseline/`;
provenance in `baseline/provenance.txt`, commands in
`baseline/commands.txt`, hashes in `baseline/baseline.SHA256SUMS`):

- **B-R1-1** (A-R1-1). In an isolated copy of the base, probes appended to
  its test-only simulation: an uncertain start with no candidate, and a
  manager panic during establishment. `ScopeManager::start` unwound to its
  caller; the issuing controller's owners were 1 before and 1 after (no
  surviving `PendingScope`); the unit stayed loaded; no StopUnit.
  `b-r1-1-to-4-run.txt`.
- **B-R1-2** (A-R1-2). A `StartFailed` carried an unresolved operation while
  the helper was a separate caller-owned value; dropping the failure left
  the unit loaded and owned by nothing; the helper could then be reaped
  independently, after which settling with any helper confirmed nothing.
- **B-R1-3** (A-R1-3). `PendingScope::settle` returned `true` while the
  helper was still running and unreaped: it neither kills nor owns the
  helper its contract described.
- **B-R1-4** (A-R1-4). The kernel reported the helper in
  `/foreign.slice/<unit>`; the base opened it and proved it (`Ok(Scope)`)
  with only Start, GetUnit, RuntimeMaxUSec and OOMPolicy asked of the
  manager: the basename was the bridge.
- **B-R1-5** (A-R1-6, and B-R1-2's API half). An external crate outside the
  workspace, with no feature of the sandbox crate enabled (`cargo tree`:
  `default` only), compiled `ScopeManager::connect_at`, `scopes.start`,
  `StartFailed` and `PendingScope` (`b-r1-5-normal-caller.*`).
- **B-R1-6** (A-R1-7). A source-derived model of the base's allocation
  (`fetch_add`) issued `[MAX-2, MAX-1, MAX, 0, 1]`: zero issued, and 1
  issued again; the base also spawned the child before taking its serial
  (`b-r1-6-*`; no 2^64 spawns were attempted).
- **B-R1-7** (A-R1-5). The base's absence rule, verbatim, and its three
  "answered in order" statements (`b-r1-7-absence-rule.txt`); the primary
  sources (`baseline/primary/`): the D-Bus Specification matches a reply to
  its call by `REPLY_SERIAL`, is designed for asynchronous operation and
  states no in-order processing; NetworkManager's "Notes on D-Bus": order
  between two peers is kept except that a response may overtake an earlier
  call's; systemd's manual: StartTransientUnit, GetUnit, `Id`,
  `ControlGroup`. No real systemd ordering test was run.

## 3. Public API closure

- **Normal build** (the desktop, a release): a manager is constructed only
  by `ScopeManager::connect` (the bus `/run/user/<real uid>/bus`, owner and
  socket checked); a scope is started only within `execution::run`. There is
  no public direct start, pending operation, settling or failure that
  carries an operation without its helper. Proven from outside by an
  external crate with no feature enabled: 9 normal-build guards
  (`controls/normal-api-probes/`), each failing with exactly its expected
  error and self-checked (the gated ones build with the harness feature, the
  removed ones against the base).
- **Live-harness build**: `connect_at` (the missing-bus checks) and the
  direct owner `execution::place` -> `ScopedHelper`, failing as
  `PlacementFailed { error, cleanup }`. `place` consumes the helper, owns it
  with the scope operation before the request, and never unwinds: an error
  or a panic ends both with the finalizer's own steps, and what cannot be
  confirmed is one `RetainedBoundary` holding both. The owner exposes
  `&Helper` and `&Scope` only; `reap_helper` only while proven; `end`
  confirms or retains; drop is defense only.
- **Helper/Pending ownership**: a pending operation is crate-private and
  lives only beside its helper: in the execution's `Owned`, in a
  `ScopedHelper`, or in a `RetainedBoundary`. Settling (`reconcile`) is
  crate-private and called only by the finalizer's `end`, which kills the
  helper first and keeps it unreaped while the operation is unresolved.
- **Panic behaviour**: no production source resumes a panic. `run`, `place`,
  `ScopedHelper::end` and `RetainedBoundary::retry` contain every panic; the
  result is confirmed cleanup or one retained owner of everything
  unconfirmed (`matrices/panic-fault-matrix.md`).
- The live harness's scope-hold test now places through `place`, keeps the
  same assertions (the helper holds the scope and its counters after its
  final report; kill, reap, empty, removed, no counters after) and adds that
  its owner confirms the end; a failed placement's cleanup is settled
  explicitly. The unmovable-process test no longer reaps a helper beside an
  operation that may still need it. Built, never run.
- The desktop is unchanged; its guards pin `ScopeManager::connect()` and
  `execution::run(` once, forbid the harness surface, and pin its gating.

## 4. Manager / native binding

In proof order (`scope/pending.rs`, `prove`): the kernel's membership of the
retained, unreaped helper (an absolute path in normal form whose last
component is the unit's name: a locator); the directory opened from that
path, retained at once (cleanup ownership); cgroup v2, populated,
`cgroup.procs` listing the helper, exactly the limits; GetUnit for the
generated name; at the object it returned, `Id` (unit interface) byte-equal
to the generated name and `ControlGroup` (scope interface) byte-equal to the
kernel path the candidate was opened from; `RuntimeMaxUSec`; `OOMPolicy`.
Only then does the candidate become the `Scope`, in place, and only then can
the handshake and launch run. A cgroup of the same name in another slice, a
path that merely ends with the name, an empty, relative or parent-traversing
value, a child cgroup, a value that is not a string, an unavailable or
uncertain property, or any disagreement: no launch, and the candidate stays
cleanup ownership (`matrices/manager-native-identity.md`).

## 5. Uncertain start

- **The revised rule.** Without a candidate, the manager's `NoSuchUnit` with
  the helper outside confirms an operation only after its StartTransientUnit
  reply (a job path) was delivered and recorded. An uncertain start (a
  timeout, an unexpected error, a reply that does not decode, a broken
  transport, or a panic before the reply was recorded) is never confirmed
  without a candidate: not by `NoSuchUnit`, not by any StopUnit reply. It
  stays Pending, its helper killed but unreaped, its execution
  `CleanupFailed`, possibly for the backend's lifetime.
- **Why GetUnit absence no longer settles it.** D-Bus keeps message order
  between two peers but does not oblige a recipient to process or answer
  concurrently issued calls in that order, so a later `NoSuchUnit` does not
  show that an unanswered request has had, or will have, no effect. No
  systemd-specific fence is relied on. After a delivered reply the argument
  is causal, not ordering: the reply carried the enqueued job, GetUnit was
  sent after it arrived, and systemd unloads no unit with a pending job or
  running processes.
- **Collisions.** `UnitExists`: the request created nothing; the foreign
  unit is never stopped, opened, killed, claimed or proven; the operation is
  confirmed once the helper is outside any cgroup of the name.
- **Retained behaviour.** Retries stay explicit and keep the operation with
  its helper; a candidate found later (the kernel reports the helper in the
  unit's cgroup) is acquired, ended and confirmed through its descriptor.
  (`matrices/uncertain-start-states.md`.)

## 6. Helper identity

The identity is allocated from a checked counter before the control socket,
the pipes or the child exist (`launcher.rs`, `spawn_with`): nonzero (1 to
`u64::MAX - 1`), increasing, never wrapping; once the next would not fit,
the allocator refuses forever and `Helper::spawn` fails with
`LaunchError::IdentitiesExhausted` before any child is spawned. Not derived
from the pid. Tested with a test-local counter near exhaustion (I4R1-20) and
a test-only spawn counter proving no child is spawned when exhausted
(I4R1-21); the production counter is never touched by these tests.

## 7. Tests

`validation/candidate/` (the section 22 commands on the candidate snapshot,
each with its exit status and complete output):

| Command | Result |
|---|---|
| `cargo test --locked -p nexus-verifier-sandbox --lib` | 116 passed, 0 failed |
| `--test phase2_custody_store` | 142 passed, 0 failed |
| `--test phase2_custody_codec` | 21 passed, 0 failed |
| `--test phase2_custody_core` | 47 passed, 0 failed |
| `--test phase2_cleanup_observation` | 36 passed, 0 failed |
| `--test phase2_package_layout` | 1 passed, 0 failed |
| `--test phase2_live_sandbox --no-run` | built, never run |
| `cargo clippy --locked -p nexus-verifier-sandbox --lib --tests -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |
| `git diff --check` | clean |

The unit tests are I4's 90 (three renamed, four with expectations reversed
by the Architect's A-R1-5 decision, see below) and 26 new: I4R1-01 to
I4R1-21, I4R1-23 and I4R1-24 (I4R1-22 is the desktop's own guard), the
section 14 panic matrix, the normal-form membership test and the
normal-build surface tripwire. Their coverage is
`matrices/test-control-coverage.md`, generated from these runs.

The four reversed I4 expectations, each now asserting the A-R1-5 rule (an
uncertain start without a candidate stays retained): `i4_18` (its eighth
case moved out of the confirmed loop), `i4_21`, `i4_25` (its second part:
the reply was delivered but the panic preceded its recording) and `i4_29`
(its uncertain case). The renamed: `i4_02` (now "... is never proven
absent"), the second `i4_03` (now "... stays owned with its helper"), and
`i4_start_failed_...` (now `i4_a_failed_placement_...`, through the harness
owner since the public start is gone).

Stability (`validation/stability/`): the unit-test binary run 25 times with
the default test threads and 5 times with one thread: 30 of 30 runs passed
every test. No live contact (`validation/no-live/`): the unit-test binary
under `strace -f`: 116 passed; no `connect()` at all, no systemd tool
executed, no file beneath `/sys/fs/cgroup` but the standard library's
read-only `cpu.max` (14 opens), and no `/proc` cgroup file but
`/proc/self/cgroup` (2 reads): PASS.

## 8. Controls and guards

- **Behavioural controls** (`controls/scope/`, `scripts/scope_controls.py`):
  38 counted (I4 required 16, I4 additional 5, I4-R1 required 14, I4-R1
  additional 3), every one applied once, compiled, failed its intended test
  at its marker, and restored byte for byte with the checkout's status
  unchanged; every intended test passes unmutated:
  - the 21 accepted I4 controls (16 required, 5 additional): 19 run as I4
    defined them (imported from the I4 evidence's own runner, verified by
    SHA-256), 2 adapted where the repaired shape moved the same defect
    point, with the same mutation, test and marker
    (`controls/i4-mapping.md`);
  - the 14 I4-R1 controls the mission names, and 3 additional
    (NC-I4R1-X-CONTROLGROUP-UNCERTAIN, -X-ACCEPTED-EARLY,
    -X-MEMBERSHIP-NOT-NORMAL).
- **API/type guards, harness build** (`controls/api-guards/`): 27 guards (18
  visibility, 9 trait-property; 19 I4 and 8 I4-R1), each failing with
  exactly its expected error lines and self-checked; the positive control
  (the public use, and the harness owner's) builds and the harness check
  sees its deliberate error. The 19 I4 guards keep their ids and intentions;
  a pending operation is now crate-private, so the 11 that reached one fail
  first at the type's privacy and are reopened by making it public again
  (the mapping is in each record and in `controls/i4-mapping.md`); 8 new
  guards close the harness owner (no forging, no taking its helper or scope,
  no copy, no restore) and the removed start and split failure.
- **API guards, normal build** (`controls/normal-api-probes/`): 9 guards,
  each failing in a normal build with exactly its expected error: 6 gated
  (`ScopeManager::connect_at`, `execution::place`, `ScopedHelper`,
  `PlacementFailed`, `run_with_fault`, `holds_scope`/`holds_helper`), each
  building once the scratch crate enables the harness feature; 3 removed
  (`ScopeManager::start`, `PendingScope`, `StartFailed`), each building
  against the base; the positive control (`connect`, `run`, `retry`) builds,
  and the normal build enables no feature of the sandbox crate but
  `default`.
- **Source guards** (`controls/source-guards.json`): 14 guards (the 9 I4
  guards, adapted where I4-R1 changed what they pin, and 5 I4-R1: the normal
  surface, no `resume_unwind`, the binding, the uncertain start, the
  identity) pass, and each of the 24 injected violations of their self-tests
  is reported.
- **Restoration** (`controls/restoration-summary.md`): every mutating suite
  restored its files byte for byte and kept the checkout's status (0 entries
  around every step).
- API/type guards and source guards are reported apart from the behavioural
  controls and never added into one figure with them.

## 9. Accepted suites

- The accepted R3 store controls, R3's runner unchanged
  (`controls/r3-store/`): 179 counted controls, all as required, sources
  identical before and after; compared entry by entry with R3's as-run
  results: identical (`validation/checks/r3-store-comparison.txt`).
- The I2-R1 rerun, I3-I1's runner unchanged (`controls/i2r1-rerun/`): 32
  counted controls, all as required, and 4 informational; identical to R3's
  (`validation/checks/i2r1-comparison.txt`).
- R3's API probes, R3's runner unchanged (`controls/r3-api-probes/`): 28
  probes, all as expected; identical
  (`validation/checks/r3-api-comparison.txt`).
- The accepted custody store's pinned files hash exactly as accepted
  (`validation/checks/custody-hashes.txt`).
- Every historical evidence manifest verifies against its own snapshot, the
  I4 evidence included and unchanged
  (`validation/checks/historical-manifests.txt`).
- The desktop (`validation/desktop-callers.txt`): the desktop backend
  type-checks, library and tests, against the candidate, and its nine Phase
  Two guards pass (p2_g_01, p2_g_06 and p2_g_09 strengthened for I4-R1).

## 10. Evidence

- `baseline/`: B-R1-1 to B-R1-7 and their provenance; `baseline/primary/`
  the retrieval record and verbatim excerpts of the primary sources
  (`baseline/baseline.SHA256SUMS` is the manifest of the baseline as
  recorded in the work directory, which also keeps the retrieved documents
  themselves; only their records and excerpts are copied here);
  `baseline/primary-candidate/` the host manual pages read at the candidate
  (extracts, hashes, excerpts).
- `validation/`: the section 22 commands (`candidate/`), stability, no-live,
  the desktop callers, the candidate run's log and console, the snapshot's
  identity and files, the repository checks (`checks/`), the review
  searches, the citations, the coverage generator's result, and
  `normalization.tsv` (every copied file's raw and normalized SHA-256).
- `controls/`: the behavioural controls, both API guard suites, the source
  guards, the accepted suites' reruns, the restoration summary and the I4
  mapping.
- `matrices/`: direct/public API ownership, Pending/Helper transitions,
  manager/native identity, uncertain-start states, panic/fault, test and
  control coverage (generated), and the module self-review.
- `scope/`: protected refs and worktrees at the start and before the commit,
  the pre-commit preflight (refs, workflow triggers, hooks, identity) and
  the changed paths.
- `scripts/`: every runner; `SHA256SUMS`: every file of this directory.

## 11. Publication preflight

Before the commit (`scope/preflight-before-commit.txt`, the `start` phase of
`scripts/preflight.py`): the eleven protected refs and the target branch
match, locally and on `github` (the target still at the base); no workflow's
push trigger can match `review/p2-v1-native-scope-ownership` (`audit.yml`,
`ci.yml` and `pages.yml` push only `main`; `ci-fast-local.yml` only
`implement/**` and `repair/**`; `local-runner-smoke.yml` only
`rebuild/phase0-trust-boundary`; `release.yml` only `v*` tags;
`ci-phase2-linux-sandbox.yml` and `ci-portability.yml` have no push
trigger); no active hook; the commit identity is the repository's. The
`pre-push` and `post-push` readbacks follow the commit; they are kept
outside the repository with the post-commit run, and the mission's final
report gives them.

## 12. Limits

- G-HOST and G-LIVE remain: nothing here ran on a supported host; the
  binding (`Id`, `ControlGroup`, the shared cgroup namespace), `UnitExists`,
  `NoSuchUnit` and `/proc/<pid>/cgroup` of a killed, unreaped helper are
  validated against the deterministic simulation only.
- An uncertain StartTransientUnit whose helper is never seen in a cgroup of
  its unit stays `CleanupFailed` for the backend process's lifetime, its
  helper an unreaped zombie; no systemd fence is relied on to release it.
- A candidate whose binding failed is confirmed through its own descriptor
  (Empty/Removed), as accepted in I4; the unit of the name is not stopped by
  name then (no sweep).
- No cross-restart reconstruction.
- No real systemd supported-host proof; no runner provisioning; no lingering
  change; no live acceptance; no integration; no Phase Two completion.
- The live harness was built, never run.
