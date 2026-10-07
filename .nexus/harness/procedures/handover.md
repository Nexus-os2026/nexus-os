# Nexus handover procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Stopping, approaching compaction, switching provider/model or handing off to a fresh session.

## REQUIRED INPUTS

Explicit MISSION, current Git, PROGRESS, receipts/manifests, open findings and authorized next action.

## PRECONDITIONS

No authority expansion; preserve in-flight or interrupted work safely within current permission.

## ALLOWED ACTIONS

Write authorized progress/evidence/handover; create a checkpoint only if already authorized.

## FORBIDDEN ACTIONS

Inventing final SHA or results; hiding unfinished commands; treating handover text as permission; automatically starting another mission.

## STEPS

1. Inspect actual HEAD/tree/branch/worktree and command/process state. Preserve unexpected changes; do not reset or guess restoration.
2. Reconcile PROGRESS with evidence; distinguish verified, unverified, failed and missing. Hash artifacts and keep external identifiers/locations plus limitations.
3. Fill HANDOVER_TEMPLATE, including exact measured checkpoint, changed files, open findings, forbidden actions and fresh-agent reading order.
4. A commit cannot include its own hash. Record the last measured checkpoint honestly and identify separately sealed post-commit evidence for the delivered SHA/tree. A future reader must resolve Git and verify that seal, never treat an earlier checkpoint as final.
5. Update progress after the last meaningful stage; report actual test commands and results. Keep provider details separate from canonical evidence.
6. Stop at the mission boundary. Lack of context is handled by durable records, not by inventing acceptance or a new task.

## OBSERVABLE OUTPUTS

Concise resumable HANDOVER and reconciled PROGRESS/EVIDENCE tied to actual Git/evidence.

## REQUIRED RECEIPTS

Final status/provenance checks, hashes, optional authorized checkpoint result and any post-commit seal.

## STOP CONDITIONS

Authority/provenance mismatch; insufficient permission to preserve required state; missing evidence blocks the associated claim.

## HANDOFF OUTPUT

HANDOVER plus exact next authorized action and explicit forbidden next actions.
