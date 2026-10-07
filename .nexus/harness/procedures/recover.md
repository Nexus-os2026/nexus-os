# Nexus recover procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Interruption, failed orchestration, context loss or a fresh agent resuming existing work.

## REQUIRED INPUTS

Original authorization, explicit MISSION, AGENTS/POLICY, PROGRESS/HANDOVER, current Git and raw receipt inventory.

## PRECONDITIONS

No continuation based solely on memory; inspect before any writes or reruns.

## ALLOWED ACTIONS

Read-only reconstruction; resume only actions independently verified as still authorized and safe.

## FORBIDDEN ACTIONS

Re-running potentially active commands blindly; deleting/restoring unexpected work; trusting a stale summary; treating missing output as success.

## STEPS

1. Read AGENTS → POLICY → explicit MISSION → this procedure → PROGRESS → HANDOVER. Locate authorization and verify repository/remote identity.
2. Record actual status/HEAD/tree/branch/worktrees. Compare with the last measured checkpoint and evidence; stop on unexplained drift rather than repairing it silently.
3. Check for running/interrupted executions through available host tools. Inspect logs/exit status and artifacts; mark incomplete execution HARNESS_ERROR or NOT_RUN as appropriate, never infer success from absence of failures.
4. Reconcile completed actions against hashed receipts and candidate identities. Distinguish source restoration from build/execution restoration before any reused binary runs.
5. Read the failure ledger: preserve old runs, investigate repeated signatures, and obey the diagnostic/attempt budget. No empty reruns.
6. Resume only the next already authorized action; otherwise hand over a blocked report with the missing evidence/authority and exact reason.

## OBSERVABLE OUTPUTS

Reconstructed, verified continuation state with discrepancies and incomplete executions explicit.

## REQUIRED RECEIPTS

Fresh provenance, inspected log hashes, process observations and any new authorized check. Historical claims remain attributed.

## STOP CONDITIONS

Unknown process ownership, missing authority, candidate mismatch, unexplained source/build state, required evidence absent.

## HANDOFF OUTPUT

Corrected PROGRESS/HANDOVER with the next authorized action or BLOCKED and evidence.
