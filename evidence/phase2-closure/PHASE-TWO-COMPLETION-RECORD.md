# Phase Two Linux completion record

This record documents the Phase Two engineering checkpoint, its validation,
the mandatory deep local reality audit and the closure. It is a record of
facts to verify against Git and GitHub, not approval authority. Only the
Architect declared Phase Two complete (`PHASE-TWO-ARCHITECT-DECLARATION.md`).
The claims below are bounded by §G, and the known warnings in §F stand.

## A. Source identity

| Item | Value |
|---|---|
| Checkpoint commit | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` ("docs(evidence): add the R1 run logs the ignore rules excluded") |
| Checkpoint tree | `bb4eb1c7d1137c907f0ace14269b2e1a747881cb` |
| Parent | `45b53ac749981793ee91a4dbc566e2add7e46ef6` ("fix(security): move Wasmtime to patched 36.x"), whose parent is `a7001311221023d37d73e6355ec0c0f5020a32a8` |
| Frozen branch | `rebuild/phase2-governed-verification` = `dc52fadb…`, created by P2-CLOSE-F1 without a new commit |
| Authoritative implementation branch | `implement/phase2-governed-verification` = `dc52fadb…`, reached by the P2-CLOSE-I1 `git merge --ff-only` from `4d6763a8afe998bb5a54f0b9ecfd420cd4abfa8f` (30 commits, no merge commit) |
| Security-repair branch | `repair/p2-security-wasmtime-36` = `dc52fadb…` |
| Phase Two history | 43 commits from `e47bf65788247946eb8401138f57435e7c1680fc` (the `main` the Phase Two mission started from) to `dc52fadb…`, none a merge commit |
| `main` | `4b36f60694148b60029733bc7b3d26e5a215d460`, unchanged by this closure; it does not hold Phase Two (merge-base with the checkpoint: `e47bf657…`) |

Unchanged Phase Zero and Phase One frozen and evidence refs:

| Ref | SHA |
|---|---|
| `rebuild/phase0-trust-boundary` | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| `evidence/phase0-closure` | `e33cf1ff1b8de0d0c6c8751e24d85ed98b3cf9b1` |
| `rebuild/phase1-governed-coding` | `14270a9a38770ac84456c1f812042d2967edec42` |
| `evidence/phase1-closure` | `c237937189b5977eaad01acad6a4c52bcce796cc` |

Unchanged Phase Two review and evidence refs: `repair/p2-validation-workflow-bootstrap` (`4b36f606…`), `review/p2-v1-cleanup-observers` (`45898e05…`), `evidence/p2-v1-r3b-i2-r1` (`8e2c46c0…`), `review/p2-v1-custody-recorder-design` (`7689e599…`), `review/p2-v1-custody-store-implementation` (`3805204f…`), `review/p2-v1-native-scope-ownership` (`ea475eac…`), `review/p2-v1-native-scope-host-qualification` (`a7001311…`).

## B. Delivered trust boundary

Phase Two adds governed, sandboxed verification of a verified coding
candidate on Linux x86_64. The design record is
`docs/security/phase2-governed-verification.md` at the checkpoint. The
accepted chain:

1. **Backend-bound request.** The owner supplies only an opaque run id and a
   profile name. The name selects a compiled-in profile and grants nothing.
   Approval is the backend's native dialog, bound by the kernel to the
   approved inputs and execution generation.
2. **Exact candidate.** The verified candidate of that run is materialized
   into the workspace and re-checked after the run.
3. **Private workspace.** It is derived from the real uid
   (`/run/user/<uid>/nexus-verifier/ws-<128-bit random>`), never `$HOME` or
   `$XDG_RUNTIME_DIR`, created exclusively and owner-only on the uid's tmpfs,
   and retained by descriptor. Its removal is descriptor-based and
   identity-checked.
4. **Packaged verifier toolchain.** A compiled-in manifest is checked
   exactly against the root-owned installed tree and host runtime, and
   re-verified before launch. The entry is executed by descriptor.
5. **Mandatory layers.** All six namespaces are created in one `unshare`.
   Landlock runs in strict mode at ABI 6 or later, and must be fully
   enforced. The seccomp allow-list is pinned to x86_64, kills x32 calls and
   allows no `socket`. There is no degraded mode: an unavailable layer fails
   closed.
6. **Resource policy.** Memory, swap, pids, CPU, wall deadline, runtime
   backstop and output ceilings are requested and then proven.
7. **Native scope candidate.** The cgroup the kernel reports the backend's
   own unreaped helper in is retained by descriptor, as an observation only.
8. **Exact successful-start `InvocationID`.** It is captured from the
   start's own job signals: sender-bound and serial-ordered.
9. **Exact manager unit:** `GetUnit` for the generated name, and its `Id`.
10. **Exact `ControlGroup`:** exactly the path the kernel reported.
11. **Retained-descriptor `ControlGroupId`:** the descriptor's own kernel
    cgroup ID.
12. **`InvocationID` recheck.**
13. **Cleanup ownership.** Only now may `cgroup.kill` reach the candidate.
14. **Complete policy proof:** populated, helper listed, exact limit files,
    `RuntimeMaxUSec`, `OOMPolicy=continue`, and the `InvocationID` re-checked
    again.
15. **Proven.** Only an owned candidate is promoted.
16. **Launch.** Nothing reaches the helper before the scope is proven.
17. **Bounded, truthful cleanup and finalization.** There is one finalizer
    for normal ends and panics alike, with bounded waits. An unresolved
    scope operation keeps its helper unreaped. Anything unconfirmed is
    retained for a retry and keeps Apply refused, and no result is reported
    as passed without confirmed cleanup.

**A string is never authority.** A run id, profile name, unit name, process
id, cgroup path text, model output or frontend value is a locator or data,
never authority. No unit is ever acted upon by its name, a foreign unit is
never mutated, `NoSuchUnit` and `UnitExists` are matched exactly, and an
uncertain start fails closed.

## C. Validation

Exact-SHA runs on `dc52fadb…` (`RUNS.md` has the full ledger):

| Run | Role | Result |
|---|---|---|
| `37198034561` | NEXUS OS CI, `workflow_dispatch`, attempt 1: hosted closure CI | success: `test-linux`, `test-frontend`, `test-python`, `security-audit-linux` |
| `37201338898` | NEXUS OS Phase Two Linux Sandbox, `workflow_dispatch`, attempt 1: dedicated live qualification on the supported host | success |
| `37203318840` | NEXUS OS Fast Local CI, `push` to `implement/phase2-governed-verification`, attempt 1: automatic post-integration | success: `fast-linux`, `fast-python`, `fast-frontend` |

Counts established in the run logs and the deep local audit:

| Item | Count |
|---|---|
| Live sandbox (run `37201338898`) | 39 expected, 39 observed, none missing or unexpected, all passed; H1–H7 passed; pre- and post-live cleanup observations clean |
| Verifier sandbox unit suite (run `37201338898`) | 156 passed |
| Cleanup-observation controls (run `37201338898`) | 36 passed; 7 fixture controls OK |
| Kernel Phase Two controls (run `37201338898`) | 26 passed |
| Desktop Phase Two/One and verification controls (run `37201338898`) | 32 passed |
| Hosted `test-linux` (run `37198034561`) | 8,398 passed, 0 failed across 304 test-result lines (workspace tests, including the 39 live cases on the hosted runner, plus the packaged Builder tests); `fmt` and Clippy clean |
| Packaged Builder toolchain and governed launch | 13 passed (runs `37198034561` and `37203318840`) |
| Trusted Builder entry tests | 21 passed |
| Frontend | 105 test files, 483 tests passed (runs `37198034561` and `37203318840`) |
| Python voice | 27 tests OK (run `37198034561`, 2 skipped: no whisper backend; run `37203318840`, OK) |
| Deep-local non-live verifier suite (P2-CLOSE-A1) | 421 passed |
| Fresh security scan | PASS: hosted `security-audit-linux` (run `37198034561`), Fast Local's security step (run `37203318840`) and a fresh local scan (P2-CLOSE-A1) |

## D. Deep local audit (P2-CLOSE-A1)

The audit ran on the Nexus PC itself. Its report, command transcript and
original manifest are copied byte for byte in this directory
(`DEEP-LOCAL-AUDIT-*`). It established:

- **Git state:**
  - the authoritative worktree is clean at `dc52fadb…` (tree `bb4eb1c7…`);
  - `git fsck --full --strict` is clean except for 93 dangling objects (none
    pruned);
  - 105 worktrees were audited: five historical Phase Zero worktrees are
    dirty and unrelated to Phase Two (§F W1);
  - one unrelated Phase Zero stash (§F W2).
- **Runner and host:**
  - one runner registration (21 `nexus-local-asus`);
  - its listener runs in `github-runner`'s user manager under
    `user@1001.service/app.slice/nexus-github-runner.service`;
  - the old system runner service is disabled and inactive;
  - zero verifier cgroups and zero verifier processes.
- **Security:**
  - a fresh local security scan (`scripts/security-audit.sh --install`,
    isolated `cargo-audit` 0.22.1 and `cargo-deny` 0.19.6, fresh RustSec
    `ef6173cb`) passed;
  - Wasmtime is `36.0.17`, and its production reachability is core Wasm only.
- **Source review:** all seven trust-boundary areas passed, and a second
  adversarial pass found no closure blocker.

At audit time the verdict was **AUDIT BLOCKED**, solely for
`A1-BLOCKER-WORKSPACE-OBSERVATION`: from uid 1000 the runner's private
`/run/user/1001/nexus-verifier` could not be observed after Fast Local run
`37203318840`. The Owner then ran the Architect-specified read-only
observation as uid 1001 (`OWNER-WORKSPACE-OBSERVATION.txt`, 2026-10-04T16:07:28Z):
the directory, `1001:1001` mode 700, had **0 entries**. The Architect
accepted that observation and resolved the blocker.

## E. Security repair (P2 security closure R1)

- **The failure:** hosted closure CI run `37191730167` on `a7001311` failed
  only `security-audit-linux` on RUSTSEC-2026-0327 (wasmtime 43.0.2,
  component async-lifted callbacks, critical).
- **The repair, in `45b53ac7`:**
  - Wasmtime moved from **43.0.2 to 36.0.17**, Wasmtime's maintained 36.x
    security line (MSRV 1.86; Rust stays 1.94.0). On 36.0.17,
    RUSTSEC-2026-0327 does not apply, and RUSTSEC-2026-0114, -0222, -0269
    and -0316 are patched.
  - **RUSTSEC-2026-0327 was never ignored.** No Wasmtime advisory exception
    remains: six stale exceptions (RUSTSEC-2026-0269, -0222, -0316, -0247,
    -0250, -0251) were removed and none was added.
  - The Phase Zero Wasmtime guard now refuses any Wasmtime exception, and
    rejects Wasmtime async and component-model APIs in production sources.
- **Reachability:** production Wasmtime use is core Wasm only. This is
  backed by a build with `component-model` and `async` compiled out.
- **Evidence:** `docs/evidence/p2-security-wasmtime-36-r1/` at the checkpoint.

Historical process notes, not claims:
- **Effort below MAX:** the executor's visible effort was below the required
  MAX for:
  - the R1 repair implementation;
  - the R1 hosted-CI pre-checks and dispatch;
  - the live requalification's monitoring turns and its final evidence
    collection and report (that report understated this, saying only the
    acknowledgements ran lower);
  - P2-CLOSE-I1, which did not require MAX.
- **Effort at MAX:** the R1 hosted-CI evidence collection, the live
  requalification's pre-checks and dispatch, P2-CLOSE-A1 and P2-CLOSE-F1.
- **What this does not affect:** the runs' results are GitHub facts,
  independent of executor effort. P2-CLOSE-A1 independently re-verified the
  checkpoint at MAX.
- **Cancelled run:** the automatic Fast Local run `37196418437`, triggered by
  the R1 push, was cancelled before its Linux job started (`RUNS.md`).
- **Contact address:** one crates.io metadata request in R1 sent the Owner's
  contact address in its User-Agent. This is disclosed in the R1 evidence;
  the address is not repeated here.

## F. Known warnings and deferred hardening

The deep audit's warnings stand as recorded in `DEEP-LOCAL-AUDIT-REPORT.md` §O:

| ID | Warning |
|---|---|
| A1-W1 | Five historical Phase Zero worktrees hold uncommitted local work (none touches a Phase Two path; each HEAD is an ancestor of the checkpoint). |
| A1-W2 | One historical Phase Zero stash remains (`stash@{0}`, R5 pre-correction draft, superseded by `41955580`). |
| A1-W3 | Six older custody evidence manifests (I3-I1, I3-P-R1 … R5) checksum files outside their own directories. They verify at their own recording commits, not on the current tree. This is evidence-manifest design debt. |
| A1-W4 | The closure runs were previously recorded only locally. This directory now records their identities (`RUNS.md`); their logs are not committed. |
| A1-W5 | Stale current-state documentation: `AGENTS.md` Part II (2026-09-30 snapshot), the standalone `audit.yml` gate wording (ruled non-blocking) and the README's "not integrated". |
| A1-W6 | NEXUS OS Fast Local CI runs the live sandbox suite on the self-hosted host without the Phase Two workflow's cleanup observations. This was the cause of the audit blocker. |
| A1-W7 | Host posture: the operator account is in the root-equivalent `docker` group, and a root `gitlab-runner` daemon and `dockerd` run on the qualification host. |

Deferred hardening and recorded boundaries:

- **Wasmtime features:** default-feature minimization (`default-features =
  false` with an explicit core set) remains future hardening.
  `component-model` and `async` are compiled in but unreachable.
- **Same-UID processes:** malicious same-UID host processes are outside the
  Phase Two claim (§G).
- **Child reaping:** third-party crates were not exhaustively audited for
  generic child reaping. Nexus production code has none outside the sandbox's
  own PID-1 init.
- **Availability:** fail-closed availability limits stand. An unconfirmed
  cleanup keeps Apply refused until a retry confirms it, and an unresolved
  scope operation can hold its helper unreaped for the life of the backend
  process.

## G. Non-claims

Phase Two does not claim:

- anything beyond the **Linux x86_64 support profile** where every mandatory
  layer is available;
- any Windows or macOS qualification;
- protection from kernel vulnerabilities or root compromise;
- protection from malicious same-UID host processes modifying user-owned
  state outside Nexus's retained identities;
- a private mount view, private `/proc`, or metadata confidentiality for host
  path names;
- any degraded sandbox mode (there is none: unavailable means unavailable);
- that Nexus OS as a whole is production-ready;
- any external audit, attestation or certification.

Phase Zero's and Phase One's recorded scope, limitations and non-claims
continue to apply.

## H. Closure

- **Frozen checkpoint:** the engineering checkpoint is frozen as
  `rebuild/phase2-governed-verification` = `dc52fadb…` (tree `bb4eb1c7…`).
- **Evidence:** the closure evidence is this separate branch,
  `evidence/phase2-closure`, which starts at `dc52fadb…`, adds only
  `evidence/phase2-closure/`, and is never merged into the frozen checkpoint
  or `main`.
- **`main`:** stays at `4b36f606…` and does not hold Phase Two. Public status
  and `main` integration are separate and need their own Architect mission.
- **Not created:** no tag, version or release was created, and Phase Three
  has not started.

## I. Validation of this directory (P2-CLOSE-F1 Amendment A1)

`git diff --check` has one documented finding in the byte-for-byte preserved
deep-audit command transcript at line 47
(`evidence/phase2-closure/DEEP-LOCAL-AUDIT-COMMANDS.txt:47: trailing
whitespace.`). The source artifact already contains that trailing space, and
its accepted SHA-256 (`1ecef01e8a6a3ba073817b15ffc85217865d237d323cb42f493041419e09492a`)
is preserved exactly. All other closure evidence paths pass `git diff
--check`. The exception applies to that one line of that one copied file
only.
