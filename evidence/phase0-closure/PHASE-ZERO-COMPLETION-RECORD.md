# Phase Zero Linux completion evidence record

> **Architect declaration pending.** This record assembles evidence for the
> Architect's final review. It does not itself declare Phase Zero complete.
> The declaration "PHASE ZERO COMPLETE — LINUX SUPPORT PROFILE" is reserved
> for the ChatGPT Architect and has not been made.

Everything here is a claim to verify against Git and GitHub. Nothing in this
record is approval authority. It was assembled after the publication and
metadata missions, from the live GitHub state re-read before commit.

## A. Final source identity

| Item | Value |
|---|---|
| Commit | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Tree | `8106ec232a494a3e9b857c7986f85eee3a7888b9` |
| Parent | `cae7bb5288820589b5cddf255e1bfc9f5e49c88a` (frozen technical candidate, `repair/p0-linux-final-closure-r2`) |
| `main` | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| `rebuild/phase0-trust-boundary` (authoritative) | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| `repair/p0-doc-truth-02` (combined candidate) | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |

`main` and the authoritative branch point to the same commit and therefore
the same source bytes, tree `8106ec23…`.

- **Authoritative:** reached `f727f5c3` by a fast-forward from `71c47acb`
  (247 commits, merge base `71c47acb`).
- **`main`:** reached it by a fast-forward from `80640bba` (401 commits,
  merge base `80640bba`).
- **Neither fast-forward** created a commit.

## B. Platform scope

- Linux is the completed validation target being considered for Phase Zero
  closure.
- Windows and macOS portability validation is deferred.
- No Linux security claim is inherited by Windows or macOS.
- `.github/workflows/ci-portability.yml` (the Windows and macOS jobs) remains
  manual-only and deferred. It was not dispatched for this closure.

## C. Hosted final candidate gate

| Item | Value |
|---|---|
| Run ID | `36624858376` |
| Run number | #111 |
| Attempt | 1 |
| Workflow | `.github/workflows/ci.yml` |
| Event | `workflow_dispatch` (`candidate_sha=f727f5c3…`) |
| Branch | `repair/p0-doc-truth-02` |
| SHA | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Conclusion | success |

- **Jobs:** `test-linux`, `security-audit-linux`, `test-frontend` and
  `test-python` all passed.
- **Revision identity:** every job printed `Tested commit: f727f5c3…` and
  `Workflow definition: …ci.yml@refs/heads/repair/p0-doc-truth-02 at
  f727f5c3…`, after `git checkout --force f727f5c3…`. The workflow rejects a
  run unless the candidate equals `GITHUB_SHA` and `GITHUB_WORKFLOW_SHA`.

Measured from the native logs:

| Evidence | Result |
|---|---|
| Rust workspace (`cargo test --workspace --locked`) | 7,684 passed, 0 failed, 43 ignored, in 293 result summaries |
| Named Phase Zero tests | 231 `p0_fg_*`, 11 `p0_fg1_*`, 8 `p0_r1_*`, 47 `p0_002c5c_*` (297) |
| Frontend (Vitest) | 461 / 461 tests in 103 / 103 files; `tsc --noEmit` clean |
| Python voice | 27 tests OK, 2 skipped; hash-pinned lock, `pip check` clean |
| Builder trusted-entry JS | 21 / 21 |
| Packaged Builder gate | 13 passed |
| Webview, development profile | live harness passed ("all boundary checks passed") |
| Webview, release profile (custom-protocol, embedded dist) | live harness passed |
| fmt / clippy (`-D warnings`) | passed |

## D. Main publication validation

| Item | Value |
|---|---|
| Run ID | `36636434205` |
| Run number | #112 |
| Attempt | 1 |
| Workflow | `.github/workflows/ci.yml` |
| Event | `push` |
| Branch | `main` |
| SHA | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Conclusion | success |

- **Jobs:** the same four jobs passed.
- **Revision identity:** every job printed `Tested commit: f727f5c3…` and
  `Workflow definition: …ci.yml@refs/heads/main at f727f5c3…`. So
  `GITHUB_SHA` = `GITHUB_WORKFLOW_SHA` = tested commit =
  `f727f5c39fab8d5c729a55eb28ad576d3d56ce47`.

Measured Linux totals, the same as run #111:

- **Rust:** 7,684 passed, 0 failed, 43 ignored.
- **Named Phase Zero tests:** 297 (231 / 11 / 8 / 47).
- **Frontend:** 461.
- **Python:** 27, OK, 2 skipped.
- **Builder:** JS 21, packaged gate 13.
- **Webview:** dev and release both passed.

The guards below reported `ok` in the run #112 log:

| Guard | Covers |
|---|---|
| `p0_j2_j3_deployment_docs_withdraw_without_claiming_a_stop` | documentation and withdrawal |
| `p0_fg1_deployment_docs_withdraw_without_claiming_a_stop` | documentation and withdrawal |
| `p0_fg_standalone_readme_names_every_withdrawn_binary` | documentation and withdrawal |
| `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` | Wasmtime |
| `p0_fg_e_vault_key_sources_are_validated_on_what_is_read` | vault |
| `p0_002c5c_final_trust_surface_guard_is_complete` | final trust surface |
| `webview_boundary_live` (dev and release) | webview |

## E. Main security audit

| Item | Value |
|---|---|
| Run ID | `36636434242` |
| Run number | #4 |
| Attempt | 1 |
| Workflow | `.github/workflows/audit.yml` (token permission `contents: read`) |
| Event | `push` |
| Branch | `main` |
| SHA | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Conclusion | success |

- cargo-audit 0.22.1, cargo-deny 0.19.6.
- `advisories ok, bans ok, licenses ok, sources ok`.
- `security-audit: passed`.

**A green gate does not mean zero dependency debt.** It means no advisory
outside the reviewed exception set in `deny.toml`.

## F. Fast-local supporting evidence

| Item | Value |
|---|---|
| Run | `36622280925` (#14, attempt 1, `push` on `repair/p0-doc-truth-02`) |
| SHA | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Conclusion | success |

This run was on the self-hosted runner. It is supporting development
evidence, not the final hosted authority; runs #111 and #112 are the hosted
evidence.

## G. Security and dependency exception

- RUSTSEC-2026-0316 remains a narrowly accepted Wasmtime **non-use**
  exception. The guard
  `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` pins it:
  - no production use of Wasmtime's component module (raw-identifier and
    raw-string lexical forms included);
  - no alias, rename or glob import of the crate;
  - no Cargo dependency rename.
- It does **not** establish that wasmtime 43.0.2 is generally safe.
- The accepted advisory set (11 IDs) is governed by `deny.toml` and applied
  by `scripts/security-audit.sh` with the pinned tools.

## H. Trust-boundary closure summary (Linux)

These are the Phase Zero trust foundations on the final source. The detailed
evidence, limits and non-claims are in
`docs/security/phase0-final-gate-dossier.md` and
`docs/security/phase0-c5-authority-inventory.md` at `f727f5c3`.

- **Workspace authority:**
  - backend-owned `WorkspaceGrant` authority through the workspace authority
    registry, with opaque handles and no frontend minting;
  - canonical workspace and project roots;
  - narrowing-only grants, revocation and expiry.
- **Builder:**
  - retained project identity, revalidated around process launch;
  - a governed workspace and a Nexus-owned, manifest-verified packaged
    toolchain.
- **Processes:** owned and sealed process lifecycle, with cleanup at a
  normal managed exit.
- **Webview:** a privileged boundary on Linux: an app ACL for the main
  window at the app origin, an exact-origin navigation guard, frame
  restrictions and a CSP.
- **Network and credentials:**
  - bounded egress: caller-chosen destinations closed; the Ollama address
    only from operator configuration;
  - credential handling: no credentials on command lines; credentialed
    clients follow no redirects and are bounded in time and size;
  - the vault key is validated, and the six current vault scopes are
    verified before the facade is installed.
- **Autonomy:** L6 (transcendent) agents are refused at every entry point.
- **Secrets:** new plaintext token and credential persistence is closed.
- **Closed routes:** screen observation, OS input, voice capture and other
  ungoverned routes are closed where they could not be governed (154 of 804
  IPC commands closed).
- **Withdrawn surfaces:** unsafe standalone servers, CLIs and deployment
  recipes (18 binaries, plus Docker, Compose, Helm, install and packaging
  recipes) exit with status 69 or fail before doing anything.
- **Dependency gate:** a pinned, single exception set, failing on any new
  advisory.
- **Final trust-surface guard:** 45 rows naming 142 guards, with a
  completeness check over the six Final-Gate guard modules.

Phase Zero claims no universal sandboxing and no universal safety.

## I. Public documentation truth (`main` at `f727f5c3`)

- `README.md`:
  - says "Phase Zero is not complete" (true when the source was published)
    and describes the Linux closure candidate;
  - identifies Linux as the active Phase Zero validation profile, with
    Windows and macOS deferred;
  - says "Local-first is not local-only", and that configured cloud
    providers send request data off the machine;
  - claims no external SOC 2 Type II, no NIST certification, and describes
    the EU AI Act material as an internal self-assessment;
  - says there is no supported standalone server, Docker, Compose, Helm or
    Kubernetes deployment, and names the 18 withdrawn binaries;
  - claims no general operating-system or WebAssembly sandbox, and says
    green dependency CI does not mean zero dependency debt.
- `docs/SOC2_TYPE_II_CONTROLS.md`, `docs/NIST_800_53_MAPPING.md`,
  `docs/EU_AI_ACT_CONFORMITY.md` and `docs/SINGAPORE_AI_GOVERNANCE.md` each
  begin with a "not an audit, not a certification" status banner marking
  them as internal, legacy mappings.
- `docs/DEPLOYMENT.md` and `docs/ENTERPRISE_DEPLOYMENT.md` state that
  standalone server and deployment paths are withdrawn and that the old
  enterprise guide is legacy.
- `SECURITY.md` makes no universal sandbox, blanket no-ambient-authority or
  supported-version claim.

## J. Public GitHub metadata truth

- **Repository description** (exact):
  "Governed, local-first agentic AI operating environment — Phase Zero trust-boundary rebuild, Linux validation profile"
- **Topics** (exact set): `agentic-ai`, `agents`, `ai`, `cybersecurity`,
  `governance`, `llm`, `local-first`, `operating-system`, `rust`, `tauri`.
- **Removed topics:** `owasp`, `post-quantum-cryptography`, `a2a-protocol`
  and `mcp` were removed deliberately as misleading or unvalidated public
  feature labels.
- The default branch is still `main`, the visibility is public and the
  license is MIT, all unchanged.

## K. Historical releases

- The published releases `v10.3.0`, `v10.5.0` and `v10.6.0` are titled
  "[HISTORICAL] …".
- Each begins with the "HISTORICAL PRE-PHASE-ZERO SNAPSHOT" banner, with the
  original notes preserved byte for byte below it.
- Tags were not moved, releases were not deleted, and assets are unchanged
  (0 before and after).
- No current Phase Zero production release was created.
- **Remaining UI limitation:** GitHub still labels the historical `v10.6.0`
  as "Latest". Clearing that flag would only make GitHub promote another
  historical release. The label stays, and the "[HISTORICAL]" title and the
  banner give the truthful context.

## L. Pull request and publication state

- PR #1 (`rebuild/phase0-trust-boundary` → `main`) was automatically marked
  merged and closed at 2026-09-29T21:55:32Z, when `main` fast-forwarded to
  the same commit as its head.
- Its `merge_commit_sha` is `f727f5c39fab8d5c729a55eb28ad576d3d56ce47`, the
  PR head itself. No merge commit was created.
- No one used the PR merge action. GitHub attributes the detected merge to
  the account that pushed.

## M. Accepted limitations and non-claims

- Windows and macOS are deferred. No Phase Zero claim covers them.
- There is no external certification, attestation or independent audit.
- There is no general OS or WebAssembly sandbox for agents.
- Green dependency CI does not mean zero dependency debt.
- Accepted security advisories remain documented exceptions in `deny.toml`.
  RUSTSEC-2026-0316 is a Wasmtime non-use exception only.
- Historical releases remain historical; they are not supported releases.
- Existing deployments, containers, images and credentials from older
  instructions are not stopped, removed or revoked by the source
  withdrawal.
- Phase Zero closure implies no Phase One capability.
- Developer and benchmark binaries permitted by Decision M remain
  development-only: not shipped, pinned by the inventory guard, and not
  documented as runtime entry points.
- Standalone server deployment remains withdrawn.
- The remaining dispositions and limits of the Final-Gate dossier apply as
  written there. These include the ambient derivation as legacy
  compatibility only, IPC HITL approval as not authority, and owned-download
  cleanup at normal exit only.

## N. Final closure checklist

| Step | Evidence | State |
|---|---|---|
| Technical candidate frozen | `repair/p0-linux-final-closure-r2` at `cae7bb52` (R2C) | done |
| R2C accepted | Architect review; used as the base of the docs repair | done |
| Documentation truth repaired | `f727f5c3` (P0-DOC-TRUTH-02R1), parent `cae7bb52`, with Decision-M guard alignment | done |
| Fast Local green | run `36622280925` (#14) on `f727f5c3` | done |
| Hosted Final Gate #111 green | run `36624858376`, attempt 1, 4/4 jobs, `f727f5c3` | done |
| Authoritative fast-forward complete | `rebuild/phase0-trust-boundary` `71c47acb` → `f727f5c3` | done |
| Public `main` fast-forward complete | `main` `80640bba` → `f727f5c3` | done |
| Main-push CI #112 green | run `36636434205`, attempt 1, 4/4 jobs, `f727f5c3` | done |
| Main audit #4 green | run `36636434242`, attempt 1, `f727f5c3` | done |
| Public description aligned | exact text in section J | done |
| Topics aligned | exact set in section J | done |
| Historical releases bannered | v10.3.0, v10.5.0 and v10.6.0 (section K) | done |
| `main` == authoritative | both `f727f5c3`, tree `8106ec23` | done |
| Completion evidence record created | this file | done |
| **Architect final declaration** | reserved for the ChatGPT Architect | **PENDING** |
