# Architect declaration: Phase Two

| Item | Value |
|---|---|
| Date | 2026-10-04 |
| Declared by | the ChatGPT Architect (the project's independent Architect) |
| Declaration | **PHASE TWO COMPLETE — LINUX SUPPORT PROFILE** |
| Checkpoint commit | `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` |
| Checkpoint tree | `bb4eb1c7d1137c907f0ace14269b2e1a747881cb` |
| Frozen branch | `rebuild/phase2-governed-verification` = `dc52fadb…` |
| Evidence branch | `evidence/phase2-closure` |
| Public `main` at declaration | `4b36f60694148b60029733bc7b3d26e5a215d460` (unchanged; it does not hold Phase Two) |

This file records the declaration as it was issued. It is an Architect
project declaration, not an external certification, attestation or audit,
and it carries no signature. The evidence file carries no authority by
itself: it is a record to verify against Git and GitHub. The supporting
evidence is in `PHASE-TWO-COMPLETION-RECORD.md`, `RUNS.md`, the copied deep
local audit (`DEEP-LOCAL-AUDIT-*`) and `OWNER-WORKSPACE-OBSERVATION.txt`.

## Scope

- **Linux support profile only:** Linux x86_64 hosts where every mandatory
  sandbox layer is available.
- **Windows and macOS are not claimed.** On every other platform,
  verification is unavailable and fails closed.
- It is not a general production-readiness certification of Nexus OS.

## What Phase Two delivered

Governed sandboxed verification of a verified coding candidate:

> backend-bound request → exact candidate → private workspace → packaged
> verifier toolchain → mandatory namespaces, Landlock and seccomp →
> resource policy → native scope candidate → exact successful-start
> `InvocationID` → exact manager unit → exact `ControlGroup` →
> retained-descriptor `ControlGroupId` → `InvocationID` recheck → cleanup
> ownership → complete policy proof → Proven → launch → bounded, truthful
> cleanup and finalization.

**A STRING IS NEVER AUTHORITY.** No run id, profile name, unit name, process
id, path text, model output or frontend value grants anything.

## Validation the declaration relied on

| Run | Workflow | Event | Attempt | SHA | Result |
|---|---|---|---|---|---|
| `37198034561` | NEXUS OS CI (hosted closure CI) | `workflow_dispatch` | 1 | `dc52fadb…` | success, all four jobs |
| `37201338898` | NEXUS OS Phase Two Linux Sandbox (live qualification) | `workflow_dispatch` | 1 | `dc52fadb…` | success: 39/39 live cases, H1–H7, pre- and post-cleanup observations |
| `37203318840` | NEXUS OS Fast Local CI (automatic post-integration) | `push` | 1 | `dc52fadb…` | success, all three jobs |

The mandatory deep local reality audit (P2-CLOSE-A1) and its single blocker's
resolution are recorded in `PHASE-TWO-COMPLETION-RECORD.md` §D. The full run
ledger, failures included, is in `RUNS.md`.

## Not created

No tag, version or release was created for this declaration. `main` was not
moved. Phase Three has not started.
