# Nexus implement procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Bounded implementation already authorized by a mission.

## REQUIRED INPUTS

Verified MISSION, current Git and PROGRESS, relevant source/tests, applicable POLICY sections, acceptance criteria.

## PRECONDITIONS

Mission preflight verified; correct isolated branch/worktree; source writes and tests explicitly within scope.

## ALLOWED ACTIONS

The smallest evidence-backed scoped change, required tests, separate self-review, and commits only if authorized.

## FORBIDDEN ACTIONS

Unrelated refactors; production edits in a planning mission; deleting user work; weakening tests/containment; assumed push/integration.

## STEPS

1. Establish the root cause/invariant before repair. Map planned files to permitted scope.
2. Implement one bounded change; keep a failure ledger with SHA, signature, hypothesis, evidence, attempt and outcome.
3. Run relevant mission checks and record receipts, including failed commands. Inspect the full diff and `git diff --check`; review locks, identity, cleanup and privilege boundaries separately where applicable.
4. Update PROGRESS with measured outcomes, omitted checks and next action. Follow POLICY §12: two failed targeted attempts require deep diagnosis before a third.
5. Before any authorized commit check status/stat/full diff, scope, dependencies, generated files and secrets. One purpose per commit; preserve shared history.
6. Use verify before delivery. Push/remote verification/integration are separate explicitly authorized actions, not implied by a local commit.

## OBSERVABLE OUTPUTS

Scoped diff, test results, self-review and optional authorized local checkpoint.

## REQUIRED RECEIPTS

Required tests, diff/scope checks, actual Git identity and commit result when permitted; no independent acceptance claim.

## STOP CONDITIONS

Scope expansion, new trust model, destructive action needed, repeated failure budget exhausted, secrets or unexpected changes.

## HANDOFF OUTPUT

Updated PROGRESS/EVIDENCE and handover of exact candidate to verification or Architect if blocked.
