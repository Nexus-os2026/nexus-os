# Phase One Linux completion record

The Architect declared **PHASE ONE COMPLETE — LINUX SUPPORT PROFILE** for
the checkpoint below; see `PHASE-ONE-ARCHITECT-DECLARATION.md`. This record
keeps the following apart:

- engineering facts (A–C);
- the declaration (D);
- validation evidence (E–F);
- trust-boundary guarantees (G);
- non-claims (H);
- deferred hardening and known issues (I).

Everything here is a claim to verify against Git and GitHub. Nothing in this
record is approval authority.

## A. Source identity

| Item | Value |
|---|---|
| Checkpoint commit | `14270a9a38770ac84456c1f812042d2967edec42` |
| Checkpoint tree | `66dfec0f757fa4b72f237a709de733f22c170358` |
| Parent | `882a67c437a3af10d067f83a133109f4c5cacf9d` |
| Frozen Phase One branch | `rebuild/phase1-governed-coding` = `14270a9a…` |
| Public `main` | `14270a9a…` |
| Candidate branch | `implement/phase1-autonomous-candidate` = `14270a9a…` |
| Previous `main` (Phase Zero status) | `28835843f065ce9efee31b145ea18543b8eea432` |
| Frozen Phase Zero checkpoint (unchanged) | `rebuild/phase0-trust-boundary` = `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Phase Zero evidence (unchanged) | `evidence/phase0-closure` = `e33cf1ff1b8de0d0c6c8751e24d85ed98b3cf9b1` |

**Integration:**

- `main` moved `28835843` → `14270a9a` by `git merge --ff-only` and a normal,
  non-force push. There were 15 commits, no merge commits and no integration
  commit. `main` and the frozen branch hold the same bytes.
- `rebuild/phase1-governed-coding` was created at `14270a9a…` only after the
  post-integration CI and Security Audit passed on that exact SHA.
- **Validation PR #16** (head `implement/phase1-autonomous-candidate`, base
  `main`) was a draft opened only to obtain hosted validation. It was never
  used as a merge mechanism. When `main` was fast-forwarded, GitHub marked it
  merged automatically (2026-09-30T09:52:57Z) because its head became
  reachable from `main`. Its recorded merge SHA is `14270a9a…` itself. This is
  not an additional merge commit.
- No tag, release or version was created or changed.

## B. Commit history (`28835843..14270a9a`, oldest first)

| Commit | Subject |
|---|---|
| `6cd2147d` | feat(trust): add governed coding run primitive |
| `299b6730` | fix(trust): close coding run staging grant lifecycle |
| `a2711817` | fix(trust): bind coding run staging cleanup to identity |
| `80dc0e97` | fix(trust): reconcile coding run ledger with terminal state |
| `7d0f056f` | feat(coding-run): governed local-model worker (P1-03) |
| `8f48a49b` | feat(coding-run): backend-owned project registration (P1-04) |
| `7fecdd95` | feat(coding-run): backend-computed owner review (P1-05) |
| `e9fd4eb7` | feat(coding-run): native owner approval and stale-safe apply (P1-06) |
| `d7033604` | feat(coding-run): owner-triggered single-run restore (P1-07) |
| `ecd28c95` | fix(trust): close time_machine_what_if state mutation |
| `e60a23d7` | feat(desktop): governed coding flow end to end (P1-08) |
| `6ecc5385` | fix(desktop): print approval text literally and expose hidden characters |
| `62193b85` | fix(coding-run): harden apply for shared directories and hidden versions |
| `882a67c4` | fix(coding-run): bound review headers and model answers; claim discard |
| `14270a9a` | fix(trust): close Phase One outcome finalization |

The history is linear from Phase Zero status commit `28835843`. Each Phase One
repair was a separate forward commit, with no rebase, amend, squash or force
push.

## C. Engineering facts: the delivered workflow

One governed coding workflow on Linux, reached through the desktop's
Governed Coding page.

- **Project selection.**
  - The backend opens the native folder picker itself (the official Tauri
    dialog plugin, `tauri-plugin-dialog` `=2.7.0`).
  - The pick must be its own canonical path. The filesystem root and the
    Nexus state directory are refused.
  - The folder's directory identity is retained.
  - The frontend holds only an opaque project id and a display name.
  - Registrations last for the process lifetime only.
- **Run authority.**
  - Each run receives a fresh, run-bound, read-only `UserSelected` grant, and
    re-checks the retained folder identity when it opens the project.
  - Apply and restore each use a separate short-lived write grant. It is
    closed after every attempt, and clean success requires that closure to be
    confirmed.
- **Worker.**
  - The worker holds no authority of its own. It can list staged files, read
    a staged file inside the frozen read scope, submit a typed create/replace
    proposal, or finish.
  - Model output is parsed into a closed JSON structure. Anything else is
    rejected and recorded.
  - Turns, reads, candidate size, proposals, response size and the deadline
    are fixed backend constants.
  - The model is pinned into the run. The client is in-process and
    loopback-only, with no proxy and no redirects.
- **Staging.** Edits reach only a private staging snapshot. Protected inputs
  and `.git` cannot be edited.
- **Verification and review.**
  - In-process structural verification is bound to the run, base, exact
    candidate and profile.
  - The backend computes a bounded review diff and binds it to the same
    values.
- **Approval and apply.**
  - Approval is a backend-invoked native confirmation of that exact binding.
    It can't be constructed, deserialized or cloned by callers.
  - Apply preflights every target for stale base bytes, absent creations,
    redirects and scope.
  - Pre-images are stored before the first write.
  - Replacements use atomic exchange, and the file they displace is verified.
    Creations use `link`, which never overwrites.
  - Rollback and recovery states are reported truthfully.
- **Restore.** A single-run restore needs its own native confirmation. It
  refuses owner divergence and tampered pre-images.
- **Ledger.** A durable, fail-closed coding-run ledger records every step.
  Clean success requires durable finalization.
- **Residual closed.** `time_machine_what_if` production mutation (caller
  strings setting agent fuel, forcing agent state or toggling Warden review)
  is closed fail-closed.

## D. Architect declaration

**PHASE ONE COMPLETE — LINUX SUPPORT PROFILE**, declared by the ChatGPT
Architect on 2026-09-30 for `14270a9a38770ac84456c1f812042d2967edec42`. See
`PHASE-ONE-ARCHITECT-DECLARATION.md`. That file records the declaration and
carries no signature or external certification.

## E. Pre-integration validation (exact candidate)

| Run | Run ID | Attempt | Event | SHA | Result |
|---|---|---|---|---|---|
| Fast Local #26 | `36694877599` | 1 | `push` | `14270a9a…` | success (`fast-linux`, `fast-frontend`, `fast-python`) |
| NEXUS OS CI #115 | `36694889009` | 1 | `workflow_dispatch` (`candidate_sha=14270a9a…`) | `14270a9a…` | success (`test-linux`, `security-audit-linux`, `test-frontend`, `test-python`) |
| Security Audit #7 | `36694892587` | 1 | `workflow_dispatch` | `14270a9a…` | success |

## F. Post-integration validation (`main`)

| Run | Run ID | Attempt | Event | SHA | Result |
|---|---|---|---|---|---|
| NEXUS OS CI #116 | `36698933160` | 1 | `push` on `main` | `14270a9a…` | success (all four jobs) |
| Security Audit #8 | `36698933259` | 1 | `push` on `main` | `14270a9a…` | success |

CI #116 printed `Tested commit: 14270a9a…` and
`Workflow definition: …ci.yml@refs/heads/main at 14270a9a…`.

Measured from the run logs (Fast Local #26 and CI #115 reported the same Rust
and frontend totals):

| Evidence | Result |
|---|---|
| Rust workspace (`cargo test --workspace --locked`) | 7,849 passed, 0 failed, 43 ignored, in 293 result summaries |
| Phase Zero Final-Gate guards | 244 / 244; `p0_002c5c` 47 / 47; all Phase Zero-named tests 709 / 709 |
| Phase Zero regression selection (local, per milestone) | 297 / 0 (231 `p0_fg_`, 11 `p0_fg1_`, 8 `p0_r1_`, 47 `p0_002c5c_`) |
| Phase One named tests (CI #116) | 158 `p1a_*` / `p1_*` tests passed |
| Frontend (Vitest) | 479 / 479 in 105 / 105 files; `tsc --noEmit` clean |
| Python voice | 27 tests OK, 2 skipped (hosted runner) |
| Builder trusted-entry JS | 21 / 21 |
| Packaged Builder gate | 13 passed |
| Webview boundary, dev and release profiles | live harness passed |
| fmt / clippy (`-D warnings`) | passed |

Security Audit passes; no new Phase One warning-class regression versus the
Phase Zero baseline. Existing allowed warnings remain. Security Audit #7 and
#8 report the same advisory set as Phase Zero baseline Security Audit #5
(`36644319340` on `28835843`). All three have the same 28 advisory IDs, and
19 unmaintained/unsound warnings across the same 15 crates.

Supporting assurance during development:

- Every milestone ran negative controls and bounded mutation controls. Every
  mutant failed a test assertion, and each was restored byte-identically.
- An independent adversarial review found one high, one medium and five low
  defects. All were repaired, with tests, before the final candidate.
- The Architect's final review found one outcome-finalization defect family.
  It was repaired in `14270a9a`.

## G. Trust-boundary guarantees (Linux support profile)

These hold only for the governed coding path described in C, on Linux.

- **A string is never authority.** Frontend JSON, raw paths, run and project
  ids, model output, project files (including `AGENTS.md`, `CLAUDE.md` and
  READMEs), test output and environment values are data.
- **Project and scope.** No governed coding command takes a path, grant or
  approval flag. Project authority comes only from the backend's native
  selection and the backend registry.
- **No side doors.** The governed coding path has no shell, git, arbitrary
  process, package-manager or cloud-provider route. Its only HTTP client is
  the loopback Ollama client.
- **Staged mutation only.** Model-proposed changes reach only private
  staging. The owner's project is written only by apply, after native
  approval of the exact run, base and candidate.
- **Stale-safe apply.** Concurrent owner changes, a replaced root, redirects
  or a changed candidate reject apply with zero writes.
- **Truthful outcomes.** Partial failures are rolled back where safe, or
  reported as requiring recovery. No clean success is reported without
  durable finalization and confirmed closure of the temporary write grant.
- **Guarded restore.** Restore undoes exactly one applied run. It refuses if
  the owner changed any applied file, or if a pre-image was altered.
- **Closed residual.** `time_machine_what_if` performs no state or
  configuration mutation.

## H. Non-claims

Phase One does not claim:

- arbitrary project test execution;
- shell coding authority;
- arbitrary process execution;
- git mutation;
- cloud coding models;
- web-enabled coding research;
- package installation;
- deployment;
- multi-agent coding;
- computer-use coding;
- Windows or macOS governed coding;
- crash-persistent coding-run authority;
- atomic multi-file filesystem transactions;
- semantic correctness of code from structural verification;
- general Time Machine rollback authority.

Also:

- Phase One is not a production-readiness declaration.
- It is not an external certification or audit.
- It does not make Nexus a complete, general-purpose autonomous coding
  system.
- Phase Zero's scope, limitations and non-claims (its completion record,
  section M) continue to apply.
- Phase Two has not started.

## I. Deferred hardening and known issues

- **Per-run read-only project grant (future authority hygiene, not a Phase
  One failure).** The grant uses a bounded four-hour expiry instead of eager
  revocation at run completion. It is run-bound, read-only and unusable by a
  different binding.
- **Pre-existing test race outside Phase One.** Kernel test
  `omniscience::executor::tests::queue_and_execute_no_approval` shares a
  process-global kill-switch flag with sibling tests. It failed once, during
  Fast Local #20, and passed on rerun. The code is unchanged since v9.0.0.
  It is recorded for a future bounded mission and was not changed by
  Phase One.
- **Crash recovery is manual.** An interrupted apply or restore leaves its
  pre-images and temporary names for manual recovery, and a later snapshot
  reports a leftover temporary file by name.
