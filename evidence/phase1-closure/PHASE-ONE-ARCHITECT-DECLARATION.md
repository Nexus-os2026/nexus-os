# Architect declaration: Phase One

| Item | Value |
|---|---|
| Date | 2026-09-30 |
| Declared by | the ChatGPT Architect (the project's independent Architect) |
| Declaration | **PHASE ONE COMPLETE — LINUX SUPPORT PROFILE** |
| Checkpoint commit | `14270a9a38770ac84456c1f812042d2967edec42` |
| Checkpoint tree | `66dfec0f757fa4b72f237a709de733f22c170358` |
| Frozen branch | `rebuild/phase1-governed-coding` = `14270a9a…` |
| Public `main` at declaration | `14270a9a38770ac84456c1f812042d2967edec42` |

This file records the declaration as it was issued. It is a record, not
approval authority in itself. It carries no signature and is not an
external certification, attestation or audit. The supporting evidence is in
`PHASE-ONE-COMPLETION-RECORD.md`.

## What Phase One delivered

One governed coding workflow on Linux:

> Owner selects project → backend-owned project registration → fresh run
> authority → local Ollama-only worker → bounded staging edits → structural
> verification → backend-generated review → native owner approval →
> stale-safe project apply → pre-images → guarded single-run restore.

The Architect recorded these trust properties:

- **A STRING IS NEVER AUTHORITY.**
- **Project selection and references:**
  - Project selection is backend/native.
  - Raw frontend paths are not authority.
  - Run and project IDs are opaque references, not authority.
  - Project and model content is task data.
- **Coding worker authority:**
  - It has no shell authority, no git authority and no arbitrary process
    authority.
  - It has no cloud-model fallback, and the Ollama endpoint is loopback-only.
  - Model output becomes only typed, bounded edit proposals.
  - Worker mutations occur only in private staging.
  - Protected inputs cannot be edited.
- **Verification and review:**
  - Structural verification is bound to the exact candidate.
  - Owner review is backend-computed.
- **Apply and restore:**
  - Apply requires native approval bound to the exact run, base and
    candidate.
  - Concurrent owner changes reject apply.
  - Apply uses pre-images and truthful rollback and recovery states.
  - Restore refuses owner divergence.
- **Clean-success conditions:**
  - Temporary write authority must close before clean success.
  - Durable finalization is required before clean success.
- **Time Machine:** `time_machine_what_if` production mutation is closed.

## Validation the declaration relied on

| Run | Run ID | Attempt | Event | SHA | Result |
|---|---|---|---|---|---|
| Fast Local #26 | `36694877599` | 1 | `push` (candidate branch) | `14270a9a…` | success |
| NEXUS OS CI #115 | `36694889009` | 1 | `workflow_dispatch` (`candidate_sha`) | `14270a9a…` | success, all four jobs |
| Security Audit #7 | `36694892587` | 1 | `workflow_dispatch` | `14270a9a…` | success |
| NEXUS OS CI #116 (post-integration) | `36698933160` | 1 | `push` (`main`) | `14270a9a…` | success, all four jobs |
| Security Audit #8 (post-integration) | `36698933259` | 1 | `push` (`main`) | `14270a9a…` | success |

Security Audit passes; no new Phase One warning-class regression versus the
Phase Zero baseline. Existing allowed warnings remain.

## Integration provenance

- `main` reached `14270a9a…` by an authorized `git merge --ff-only` from
  `28835843f065ce9efee31b145ea18543b8eea432` (15 commits, no merge commit).
- Validation PR #16 was never used as a merge mechanism. GitHub marked it
  merged automatically when its head became reachable from `main`. Its
  recorded merge SHA is `14270a9a…` itself, not an additional merge commit.

## Non-claims

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

Phase Zero's recorded scope, limitations and non-claims continue to apply.
No version, tag or release was created for this declaration. Phase Two has
not started.

## Deferred hardening (not a Phase One failure)

The per-run read-only project grant uses a bounded four-hour expiry instead
of eager revocation at run completion. It is run-bound, read-only and
unusable by a different binding. It is tracked as future authority hygiene
and is not a Phase One blocker.
