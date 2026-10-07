# Nexus verify procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Validating an exact candidate against its mission acceptance contract.

## REQUIRED INPUTS

MISSION required gates, candidate SHA/tree, scope diff, toolchain identity, raw logs and receipt schema.

## PRECONDITIONS

Candidate identity verified; execution environment and side effects permitted; required tools available without unauthorized installs.

## ALLOWED ACTIONS

Run required checks, inspect complete results, classify failures and preserve logs within mission writes.

## FORBIDDEN ACTIONS

Fabricated results; unrun gates marked passing; local green called hosted green; changing requirements to conceal failure.

## STEPS

1. Freeze the candidate identity and scope; record tool versions and relevant non-secret environment facts.
2. Enumerate every required criterion as run/not run. Run exact authorized commands; preserve stdout, stderr, exit status, timestamps and hashes as RECEIPTS specifies.
3. Inspect failures beyond the first visible frontier. Isolated diagnostic no-fail-fast runs remain separate from acceptance evidence.
4. Validate command-to-control mappings rather than requiring duplicate stdout markers. Prerequisite PASS never means mutations executed.
5. Verify restored source AND execution freshness if earlier mutations/builds are relevant. Stop on uncontrolled mismatch; use only approved scoped cleanup.
6. Recheck Git and protected/unrelated state. Compare exact candidate, required criteria and evidence. Mark READY_FOR_REVIEW only for fulfilled execution exit conditions; missing evidence blocks the corresponding claim.
7. If CI is a mission gate, verify all required jobs on that exact SHA; no dispatch or push without explicit permission.

## OBSERVABLE OUTPUTS

Acceptance-criterion matrix, intact logs, measured candidate identity, truthful mission status.

## REQUIRED RECEIPTS

Every executed gate, omissions with reasons, scope/provenance and restoration checks, CI run/job IDs where actually run.

## STOP CONDITIONS

Unexpected candidate drift, uncontrolled infrastructure error, restoration failure, missing authority, unexplained artifacts.

## HANDOFF OUTPUT

Exact candidate and evidence inventory for independent review; unmet gates remain visible.
