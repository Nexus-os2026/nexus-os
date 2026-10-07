# Nexus mission procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Beginning or resuming an explicitly Architect-approved mission.

## REQUIRED INPUTS

Explicit mission path and original authorization; AGENTS, POLICY, repository identity, Git refs; PROGRESS/HANDOVER if present.

## PRECONDITIONS

Actual authorization is available. No writes begin from a filename, branch, memory or convenience pointer alone.

## ALLOWED ACTIONS

Read-only provenance inspection; create mission records or worktree only if the authorization permits those writes.

## FORBIDDEN ACTIONS

Selecting a mission by recency; self-expanding scope; silently fixing a base/worktree mismatch; acting on stale progress.

## STEPS

1. Read AGENTS → POLICY → explicit MISSION → this procedure. Missing authority blocks writes.
2. Run `git status -sb`, `git rev-parse HEAD`, `git rev-parse HEAD^{tree}`, `git branch --show-current`, `git worktree list --porcelain`, and `git remote -v` with credential-safe output.
3. Verify repository/remote, expected base or last authorized checkpoint, branch, tree and worktree. Inventory protected refs and unrelated active worktrees before changes. Unexpected state stops work; preserve it.
4. If resuming, read PROGRESS and HANDOVER; match receipt hashes/candidate identity and verify pending processes without rerunning them automatically.
5. Record authority, explicit permission denials, required checks and acceptance criteria using MISSION_TEMPLATE. Record actual observations and unresolved inputs separately.
6. Select only the next authorized procedure. Update progress after every stage.

## OBSERVABLE OUTPUTS

Verified base/branch/tree/worktree and explicit scope; missing inputs or conflicts recorded.

## REQUIRED RECEIPTS

Git/provenance commands, hashes of mission inputs, protected-state snapshot and evidence inventory; no test outcome inferred from inspection.

## STOP CONDITIONS

Unknown authority; ambiguous baseline; unexpected dirty work; ref/mission mismatch; unavailable required evidence.

## HANDOFF OUTPUT

PROGRESS with next authorized action, exact measured identity and receipt references; blocked issues for Architect.
