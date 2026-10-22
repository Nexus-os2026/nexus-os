# P2-V1-R3B-I4: native scope ownership and uncertain remote operations — evidence

This directory is the evidence of the candidate commit that contains it, on
`review/p2-v1-native-scope-ownership`. A file in a commit cannot name that
commit, so the evidence here is bound to two other things:

- **the baseline**, bound to the base
  `3805204f941d9694b2c0c36549d32a5f4e6c157c`, read from its Git objects;
- **the candidate's results**, bound to the exact source bytes they
  validated (`validation/candidate/sources.SHA256SUMS`), run in a scratch
  Git checkout of a snapshot of the candidate's tree.

The rerun on the committed candidate is kept outside the repository, in
`/home/nexus/NEXUS/p2-v1-r3b-i4-work/post-commit/`; the mission's final
report gives it with its hashes.

Phase Two remains active and not complete. Nothing here contacted a
systemd user manager, created a cgroup, ran the live sandbox, provisioned a
host or used a runner (section 7, `validation/no-live/`).

## 1. Identity

| Item | Value |
|---|---|
| Base | `3805204f941d9694b2c0c36549d32a5f4e6c157c` (tree `43d4446e1a5b4f5d333798218c4538b3025679b4`, sole parent `66a245c183b04ff1e2ae30fbbd065a57381df09c`), the accepted custody-store candidate |
| Candidate | The commit containing this file. Sole parent: the base. Subject: `fix(verifier): retain uncertain scope ownership` |
| Pre-commit snapshot | Tree `d6539ad45d3eea96cf00a19a0b99ec3065a83054`: the worktree's tracked and new files when the candidate runs began, committed into a scratch Git checkout whose `HEAD^{tree}` equals it (`validation/candidate-runs.txt`, `validation/candidate-snapshot.txt`). `validation/candidate-snapshot-files.SHA256SUMS` hashes its 35 changed and new files (`validation/candidate-snapshot-worktree-status.txt`). The candidate differs from it only by files added to this directory afterwards: the run outputs, the generated coverage matrix and restoration summary, `validation/normalization.tsv`, `REPORT.md` and `SHA256SUMS`. No source, document, script or hand-written matrix changed after the snapshot. |

Changed paths, relative to the base (`validation/checks/scope.txt`):

- `crates/nexus-verifier-sandbox/src/scope.rs` (modified; still the module
  root) and new private submodules `src/scope/manager.rs`,
  `src/scope/native.rs`, `src/scope/pending.rs`, and the test-only
  `src/scope/tests.rs`;
- `crates/nexus-verifier-sandbox/src/execution.rs` and
  `src/execution/tests.rs` (modified);
- `crates/nexus-verifier-sandbox/src/fault.rs` (seven fault points);
- `crates/nexus-verifier-sandbox/src/launcher.rs` (the narrow allowance: a
  private, never reused helper serial; `scope/launcher-diff.txt`);
- `docs/security/phase2-governed-verification.md` (sections 5, 9, 15, 17,
  18);
- `docs/evidence/p2-v1-r3b-i4-native-scope/` (new).

Byte-identical to the base (`validation/checks/scope.txt`): `lib.rs`; the
helper, protocol, seccomp, Landlock, workspace, toolchain, profile and
policy sources; every test target outside `src/` (the custody store, core
and codec and their support, the cleanup observer, the live harness, the
package layout); the manifests, `Cargo.lock`, the toolchain pin,
`AGENTS.md`, `CLAUDE.md`, `.gitignore`; workflows, packaging and the
desktop; the custody design and boundary documents; every earlier evidence
directory. The custody sources hash to the values recorded when R3 was
accepted (`validation/checks/custody-hashes.txt`). No new dependency, no
manifest edit, no `.gitattributes` file.

## 2. The findings at the base

`baseline/` binds the base's control flow to its Git objects
(`baseline/extract_findings.py`, `baseline/source-findings.txt`: each
excerpt with its line range in the base, its SHA-256 and the needles it
must hold; RESULT: every excerpt holds its needles):

| | The base | Excerpt |
|---|---|---|
| B-I4-1 | StartTransientUnit's error or timeout propagates by `?` from `start_with_fault` before any retained scope object exists; `attempt` maps it to `NotRun::Scope` with nothing owned | base `scope.rs:123-148`, `:163-196`; `execution.rs:478-482` |
| B-I4-2 | A failed proof calls StopUnit best-effort and discards its result | base `scope.rs:197-213` |
| B-I4-3 | A panic while proving takes the same best-effort StopUnit path, then resumes unwinding | base `scope.rs:202-213`, `:216-225` |
| B-I4-4 | `Owned` and `RetainedBoundary` hold `Option<Scope>` and `Option<Helper>`: no pending state | base `execution.rs:151-158`, `:334-358` |
| B-I4-5 | Finalization and retry know only `Option<Scope>`, so a unit that may exist with no proven `Scope` cannot be reconciled | base `execution.rs:531-582`, `:160-176` |

`baseline/scope_ownership_model.rs` is a standard-library-only model of
the base's exact branching (labelled a source-derived model, not the D-Bus
implementation; `baseline/model-output.txt`). Of its 8 scenarios, 6 are
counterexamples: 2 where a remote effect survives while Nexus owns nothing
and reports `Cleanup::Confirmed` (start took effect but the helper never
entered, reply lost; stop had no effect, reply lost), and 4 where a
possible effect is left with no owner and confirmed unobserved (start took
effect and placed, reply lost; stop took effect, reply lost; and the
property proof uncertain after placement, with the stop's effect either
way). No real StartTransientUnit, StopUnit or timeout was exercised: the
base has no injectable transport, and no base source was modified to
produce this evidence (`baseline/commands.txt`).

## 3. The ownership model

The full description is in `docs/security/phase2-governed-verification.md`
(section 9, "Uncertain remote operations"), `scope/pending.rs`'s module
documentation, and `matrices/ownership-transition-matrix.md`. In brief:

- **One execution's scope is a `ScopeBoundary`**: `None`, `Pending`
  (a boxed `PendingScope`) or `Proven` (a `Scope`). It lives in the
  execution's owner (`Owned`) and, when cleanup is unconfirmed, in the
  `RetainedBoundary`.
- **A `PendingScope` is the owner of everything one StartTransientUnit
  request may have created.** It is created by `ScopeManager::prepare`
  before the request and stored by the execution before it is issued. It
  binds:
  - the retained helper: its pid (a locator) and a backend-local serial
    that is never reused (`launcher.rs`, the narrow edit);
  - one backend-generated unit nonce (a locator, never authority);
  - the expected `ResourcePolicy`;
  - the controller of the issuing connection, by `Arc` (so it outlives the
    `ScopeManager`, and settling never changes connection);
  - its state (`issued`, `collided`, `settled`, StopUnit diagnostics);
  - the candidate cgroup, by descriptor, as soon as the kernel reports the
    helper in it (cleanup ownership only).
- **Transitions.** None → Pending before the request; Pending → Proven
  only by `ScopeBoundary::establish`, in place, after every proof passed
  (the only constructor of a `Scope`); Pending → gone only when settling
  confirms it; Pending → retained when it cannot. No transition takes a
  unit name, a pid or a path.
- **Confirmation (settling).** The operation is gone only when:
  - nothing was issued; or
  - the manager refused the name (`UnitExists`) and the kernel reports
    the helper in a cgroup not of that name (the other unit is never
    stopped, opened or killed); or
  - **R2**: the retained candidate reads Empty or Removed (the helper was
    seen in it, so the start job has run), as for a proven scope; or
  - **R1**: with no candidate, GetUnit on the issuing connection answers
    `NoSuchUnit` and then the kernel reports the unreaped helper in a
    cgroup not of the unit's name.
- **The helper is never reaped while its operation is unresolved.** It is
  killed, so its cgroup empties, but its pid stays reserved (a queued start
  job cannot attach a reused pid) and its membership observable. It is
  reaped only after the operation is confirmed gone.
- **Drop is defense only.** A dropped unresolved operation ends what its
  candidate holds, without waiting or the manager; `end_now` kills but
  never reaps its helper; nothing is confirmed.

## 4. Start uncertainty

- **Request.** `issue_and_prove` (`scope/pending.rs:241`) marks the
  operation issued, then sends StartTransientUnit with the base's exact
  properties (`scope/manager.rs:182`).
- **Outcomes.** A delivered job path is `Accepted`; `UnitExists` is
  `Collision` (no effect); every other error name, a timeout, a broken
  transport and a reply that does not decode are `Uncertain` and never
  read as success or absence.
- **Reconciliation of an uncertain start.** The request had no surviving
  effect only if GetUnit on the issuing connection, after the request,
  answers `NoSuchUnit` and the kernel then reports the helper outside
  (R1); the error is returned and settling confirms it without a StopUnit
  (I4-02). Otherwise the unit is discovered through the helper's
  membership and proven exactly like an accepted one (I4-03), or settled.
- **No-effect proof.** Only R1 (manager absence and the kernel's view of
  the bound helper together) or a collision with the helper outside; the
  manager's absence alone is never enough (I4-11,
  NC-I4-MANAGER-ABSENCE-ONLY).
- **Establishment of a Proven scope.** The helper's membership names the
  unit; the cgroup is opened by that path once and retained at once; it is
  a cgroup v2 directory, populated, listing the helper, with exactly the
  limits; GetUnit has the unit; RuntimeMaxUSec and OOMPolicy are exact.
  Then, and only then, the candidate becomes the `Scope`, in place, and
  only then can the handshake and launch run (I4-01, I4-18).

## 5. Stop uncertainty

- **Attempt.** Settling ends everything in the candidate (`cgroup.kill`),
  observes once, and only if nothing is confirmed calls StopUnit
  (`scope/pending.rs:176-182`). A collided operation is never stopped.
- **Ambiguity.** The reply (delivered, an error, a timeout) is recorded
  for diagnostics and never read by a decision: a delivered reply is not
  cleanup (I4-07), and a failed or timed-out one is not proof that nothing
  was stopped (I4-06, I4-08, I4-09).
- **Reconciliation.** After every attempt, settling observes again within
  `SETTLE_TIMEOUT`, by R2 or R1, acquiring the candidate from the helper's
  membership if it now names the unit.
- **Confirmation requirements.** A retained candidate Empty or Removed, or
  the manager's absence together with the helper outside a cgroup of the
  name; and the helper resolved (reaped only then). Otherwise the
  operation is retained and the execution reports `CleanupFailed`.

## 6. Execution integration

- **`Owned`** holds `helper` and `scope: ScopeBoundary`. `attempt` stores
  the helper before its output threads, prepares and stores the pending
  operation before its request, establishes it in place, and launches only
  after `is_proven()` (`execution.rs:497-513`).
- **Finalization** (`end`, `execution.rs:595`) handles a proven scope as
  before (`cgroup.kill`, helper kill and bounded reap, bounded wait for
  Empty/Removed) and a pending one by killing the helper, settling, and
  reaping only once settled. Whatever cannot be confirmed is moved by
  `retain` into the `RetainedBoundary`: `CleanupFailed`, whatever the
  earlier `NotRun::Scope` (I4-20, I4-21).
- **`RetainedBoundary`** holds a `ScopeBoundary` and the helper, borrows
  nothing, and is `Send + 'static`. `retry` runs the same steps;
  `holds_scope` is true for a proven scope or an unresolved operation.
- **Retry after the `ScopeManager` is dropped.** The operation holds its
  issuing controller by `Arc` (I4-23, NC-I4-RETRY-NEEDS-MANAGER-BORROW).
  This is exactly the desktop's use: it drops the `ScopeManager` right
  after `run` and retries later from another thread.
- **Panic behaviour.** Seven new fault points (before and after the
  request, during reconciliation, after the candidate, during the property
  proof, before and after StopUnit). A panic anywhere leaves the operation
  in its owner; the finalizer settles it or retains it; a panicked
  execution is `NotRun::Interrupted`, never `NotRun::Scope` with a
  confirmation it did not observe (`matrices/panic-fault-matrix.md`,
  I4-25..I4-28).

## 7. Tests

`validation/candidate/` holds each section 23 command's complete output,
the toolchain (`toolchain.txt`) and `sources.SHA256SUMS`
(`commands.txt`: every command exit 0):

| Command | Result |
|---|---|
| `cargo test --locked -p nexus-verifier-sandbox --lib` | 90 passed (35 new: 28 execution-level, 7 scope-level) |
| `--test phase2_custody_store` | 142 passed |
| `--test phase2_custody_codec` | 21 passed |
| `--test phase2_custody_core` | 47 passed |
| `--test phase2_cleanup_observation` | 36 passed |
| `--test phase2_package_layout` | 1 passed |
| `--test phase2_live_sandbox --no-run` | built, never run |
| `cargo clippy --locked -p nexus-verifier-sandbox --lib --test phase2_custody_store --test phase2_live_sandbox -- -D warnings` | clean |
| `cargo fmt --all -- --check`, `git diff --check` | clean |
| beyond section 23: `cargo clippy ... --lib --tests -- -D warnings` | clean |

- **35 new unit tests** (`src/execution/tests.rs`: 28; `src/scope/tests.rs`:
  7) cover I4-01 to I4-30 over a deterministic simulation of the user
  manager and the kernel (`src/scope/tests.rs`, compiled only for unit
  tests). Its manager answers each call from a script: a reply delivered,
  an effect whose reply is lost, no effect with the reply lost, a timeout, a
  disconnection, a malformed reply, `UnitExists`; StopUnit with and without
  effect under each reply. Its kernel keeps the helper's membership and
  the cgroups (populated, killed, removed, a process that survives a kill
  or a stop). Execution-level tests run a real stand-in helper that greets
  like the real one and copies any launch message to its stdout, so a
  launch reaching it is visible (I4-18).
- **Coverage**: `matrices/test-control-coverage.md` maps every requirement
  to its tests and controls: all 30 covered, and all 16 items of the
  completion standard met.
- **Stability** (`validation/stability/`): the unit-test binary run 25
  times with the default test threads and 5 times with one thread: 30 of 30
  runs passed.
- **No live systemd** (`validation/no-live/`): the unit-test binary run
  under `strace -f`: no `connect()` at all (no bus, no user manager); no
  systemd tool executed (the programs executed were the test binary,
  `/bin/cat`, `/bin/dd` and the stand-in helper scripts); each of the 14
  accesses beneath `/sys/fs/cgroup` is a read-only open of `cpu.max`, and
  both `/proc` cgroup reads are `/proc/self/cgroup` (the standard library's
  `available_parallelism` in the test harness). RESULT: PASS.
- **The production caller** (`validation/desktop-callers.txt`): the desktop
  backend type-checks against the candidate (library and tests) and its
  nine Phase Two guard tests pass.

## 8. Controls and guards

Reported separately and never added into one figure.

**Behavioural controls** (`controls/scope/`, `scripts/scope_controls.py`):
21 counted (16 required, 5 additional), all as required. Each restores one
wrong behaviour in `scope.rs`, `scope/pending.rs` or `execution.rs`,
compiles, runs its one intended test alone and fails it with its marker in
the failing assertion; every intended test passes unmutated; the files are
restored byte for byte (`controls/restoration-summary.md`). The sixteen
required controls are the mission's (NC-I4-START-TIMEOUT-ABSENT,
-START-LOST-OWNER, -START-NAME-AUTH, -LAUNCH-PENDING, -NO-EARLY-DIR,
-PROPERTY-ERROR-DROPS, -STOP-OK-CONFIRMS, -STOP-ERR-NO-EFFECT,
-STOP-TIMEOUT-DROPS, -MANAGER-ABSENCE-ONLY, -MEMBERSHIP-NAME-ONLY,
-PENDING-NOT-RETAINED, -RETRY-NEEDS-MANAGER-BORROW, -PANIC-OWNER-LOSS,
-PID-CLEANUP, -STRING-SWEEP); the five additional ones are a collision
claimed by name, a dropped boundary reaping the helper, another helper's
membership as evidence, the manager's absence ignored by the proof, and
settling without ending what the candidate holds.

**API/type guards** (`controls/api-guards/`, `scripts/api_guards.py`): 19
guards (13 visibility, 6 trait-property), all as required, with the positive
control (the public API's ordinary use builds) and the harness check (a
deliberate E0308). Each probe is an external caller that fails to build with
exactly its expected error lines; a visibility guard's self-check reopens
the guarded item in a scratch checkout and the same probe then builds; a
trait-property guard's self-check asks the same bound of a type that has it.
They show a pending operation cannot be built, edited, settled with injected
faults or rebound from outside; a `Scope` cannot be built from a name; the
boundary type, the manager and kernel interfaces, the test constructor,
`prepare` and the helper serial are unreachable; and no owner is `Clone`,
`Serialize` or `Deserialize`.

**Source guards** (`controls/source-guards.json`,
`scripts/source_guards.py`): 9 guards, all passing, each with self-tests (13
injected violations, all detected): the bus address derived from the real
uid only and owner-checked; the fixed destination, object path and
interfaces; no unit enumeration, pattern or lookup by process; no process
signalled, waited for or opened by id in the scope and execution sources;
the simulation compiled only for unit tests, with one manager and one kernel
view in a normal build; no `Serialize` or `Deserialize`; no background
cleanup; one-way construction of pending and proven scopes; and a replica of
the desktop's sandbox-source guards (the desktop's own nine guards also
pass, section 7).

**The accepted suites, rerun unchanged** (the custody store is not part of
this mission): R3's 179 store controls with R3's runner
(`controls/r3-store/`), the I2-R1 suite (32 counted controls and 4
informational runs) with I3-I1's runner (`controls/i2r1-rerun/`) and R3's
28 API probe results with R3's runner
(`controls/r3-api-probes/`): each identical, entry by entry, to its as-run
result when R3 was accepted (`validation/checks/r3-store-comparison.txt`,
`i2r1-comparison.txt`, `r3-api-comparison.txt`). Every historical evidence
manifest, R3's included, still verifies against its own snapshot
(`validation/checks/historical-manifests.txt`).

**Review** (`matrices/module-self-review.md`): the section 20 searches,
run on the candidate (`validation/review-searches.txt`), the value
classification, the public API movement and the callers. Every source
citation in the matrices resolves (`validation/citations.txt`).

**Rehearsal.** Before the snapshot, the whole run was rehearsed on a
development snapshot with the same sources (every step exit 0), to
exercise every post-processing script; its outputs are not evidence here
and no result here comes from it.

## 9. Evidence

| Path | Content |
|---|---|
| `REPORT.md` | This report |
| `baseline/` | B-I4-1 to B-I4-5 from the base's Git objects, the source-derived model, its output, commands and toolchain |
| `validation/candidate/` | The section 23 commands on the snapshot: each command's complete output, the toolchain, `sources.SHA256SUMS`, the unit-test list |
| `validation/stability/`, `validation/no-live/`, `validation/desktop-callers.txt` | Stability, the traced no-live run, the desktop callers |
| `validation/candidate-runs.txt`, `validation/candidate-runs.console.txt`, `validation/candidate-snapshot*` | The run log, its console, and the snapshot's identity, status and files |
| `validation/checks/` | The repository checks: manifests, scope, custody hashes, the reruns' comparisons |
| `validation/review-searches.txt`, `validation/citations.txt` | The section 20 searches; the resolved citations |
| `validation/normalization.tsv` | Every copied output's raw and normalized SHA-256 |
| `controls/scope/` | Each behavioural control's complete log, the baseline runs of the intended tests, `summary.json`, the run's output |
| `controls/api-guards/` | Each guard's build logs (as probed, reopened or self-checked), `summary.json`, the run's output |
| `controls/source-guards.json` | The source guards and their self-tests |
| `controls/r3-store/`, `controls/i2r1-rerun/`, `controls/r3-api-probes/` | The accepted suites, rerun unchanged |
| `controls/restoration-summary.md` | Restoration of every mutating suite, by SHA-256 and status |
| `matrices/` | The remote-operation state matrix, the ownership-transition matrix, the panic/fault matrix, the test/control coverage (generated), the complete-module self-review |
| `scope/` | The protected refs and worktrees at the start, the preflight before the commit (refs, workflow triggers, hooks, identity), the changed paths and the launcher diff |
| `scripts/` | Every runner, guard, check and generator, and the normalization |
| `SHA256SUMS` | Every other file of this directory |

Outputs were copied in through `scripts/normalize.py`: trailing whitespace
and trailing blank lines are removed, so `git diff --check` accepts them
without a `.gitattributes` exemption; nothing else changes.
`scripts/assemble.sh` made 413 of the 414 copies;
`validation/candidate-snapshot-worktree-status.txt` was copied by one more
`normalize.py` call, the same way (the frozen script does not name it). The
`*.log` files are ignored by the repository's `.gitignore` and were
force-added here only, as intentional evidence; `.gitignore` is unchanged.

## 10. Publication preflight

`scope/preflight-before-commit.txt`, before the commit:

- the eleven protected refs at their expected SHAs, locally and on
  `github`; the candidate branch at the base locally and absent on `github`;
- no workflow whose push trigger could match
  `review/p2-v1-native-scope-ownership` (`audit`, `ci`, `pages`: `main`;
  `ci-fast-local`: `implement/**`, `repair/**`; `local-runner-smoke`:
  `rebuild/phase0-trust-boundary`; `release`: tags;
  `ci-phase2-linux-sandbox` and `ci-portability`: dispatch only; no workflow
  has a `pull_request` trigger);
- no active local hook;
- the commit identity: Suresh Karicheti <nexaiceo@gmail.com>.

RESULT: PASS. `scope/protected-refs-start.txt` and
`scope/worktrees-start.txt` are the readbacks at the start of the mission;
`scope/worktrees-before-commit.txt` the worktrees before the commit.

## 11. Limits

- **No supported-host validation.** Everything runs over a deterministic
  simulation of the user manager and the kernel. The confirmation rules
  rely on systemd semantics a supported host must confirm (G-HOST,
  G-LIVE): one connection's calls answered in order, StartTransientUnit
  handled synchronously by the user manager, the `UnitExists` and
  `NoSuchUnit` names, and the kernel's `/proc/<pid>/cgroup` for a killed,
  unreaped helper (with ` (deleted)` once its cgroup is removed). The live
  harness was built, never run.
- **The unit's `ControlGroup` property is not consulted.** The proof binds
  the kernel's view (membership, the retained directory, its process list
  and limits) and the manager's (the unit loaded, its properties) by the
  backend nonce; comparing the manager's cgroup path with the kernel's is a
  hardening candidate for the live module.
- **A broken connection is never replaced** for a pending operation: one
  that cannot be confirmed through its retained candidate stays
  `CleanupFailed` for the life of the backend process; a dropped boundary's
  helper stays a zombie until the backend exits; on an unsupported (v1 or
  hybrid) host an unprovable absence is `CleanupFailed`.
- **No restart reconstruction.** Nothing is rebuilt from a unit name, pid,
  path or record after a backend restart; the scope's runtime backstop and
  the helper's parent-death chain remain the backstops.
- **No runner provisioning, no live acceptance, no authoritative
  integration, no Phase Two completion.**
