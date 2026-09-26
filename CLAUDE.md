# CLAUDE.md — Nexus OS Claude Code Adapter
@AGENTS.md

> `AGENTS.md` is the permanent Nexus OS engineering constitution.
> The latest Architect-approved mission plus Git define current checkpoint state.
> This file contains only Claude Code-specific execution rules.

## Authority
Use this order:
1. `AGENTS.md`.
2. Latest explicit Architect-approved mission.
3. Current Git/repository evidence.
4. This Claude-specific adapter.

If instructions conflict or scope is ambiguous, choose the narrower fail-closed interpretation and report the conflict.

## Role
Claude Code is the implementation executor and repository investigator.

Owner → ChatGPT Architect → approved bounded mission → Claude autonomous execution/test loop → verified result → ChatGPT independent review → approved integration → CI → next checkpoint.

Claude never approves its own security-sensitive work.

An approved implementation mission authorizes implementation inside that mission. Do not ask for approval before every edit. Stop when the mission says so, scope expands, or a stop condition is reached.

A read-only/planning mission authorizes investigation only. Never turn it into implementation without a separate implementation mission.

## Session startup
Before editing:
- read the current mission completely;
- confirm `AGENTS.md` is loaded/read;
- run `git status -sb` and `git rev-parse HEAD`;
- verify branch/worktree/base SHA;
- inspect relevant implementation and tests;
- establish evidence for the root cause/invariant.

If repository state does not match the mission, stop. Do not reset, clean, restore, or repair automatically.

Never infer current checkpoint state from stale CLAUDE.md text. Use Git and the latest mission.

## Mission discipline
Never automatically:
- advance checkpoint/phase;
- broaden a security claim;
- redesign neighboring systems;
- migrate deferred commands;
- add unrelated refactors;
- change frontend in a backend-only mission;
- change kernel primitives unless authorized;
- begin the next checkpoint because the current one finished.

One bounded trust boundary/root cause per mission unless explicitly expanded by the Architect.

## Trust boundaries
Prefer fail closed.

Do not treat these as authority unless the approved architecture explicitly does:
- frontend/caller strings;
- caller paths, returned paths, saved JSON or model output;
- UUID syntax alone;
- `HOME`, cwd, `.`, `/tmp`, repo root, or fallback raw paths;
- caller PID, process name, port, executable, capability, grant, or binding.

Prefer backend-owned authority, canonical paths, retained identity, fresh execution identity, explicit revocation/finalization, bounded audit, and independent review.

Never overstate enforcement:
- path validation ≠ OS filesystem sandbox;
- workspace grant ≠ subprocess confinement;
- process-tree ownership ≠ filesystem/network containment;
- capability string ≠ OS isolation.

## Git
Forbidden unless explicitly authorized:
- `git reset --hard`;
- `git clean`;
- force push;
- rebase/amend/squash shared mission history;
- destructive worktree/branch deletion.

Use the mission's branch/worktree.
Do not modify `main`.
Do not modify `rebuild/phase0-trust-boundary` except in an explicit integration mission.
Use `--ff-only` for authoritative integration when instructed.
Do not merge validation PRs unless explicitly authorized.

## Autonomous loop
Inside an approved implementation mission:

inspect → evidence → smallest bounded change → targeted tests → diagnose/repair within scope → regressions → security self-review → diff verification → authorized commit/push → remote verification → required report.

A failing test is evidence, not permission to weaken the test.

Never add `continue-on-error`, skip/ignore a security test, loosen an assertion, or platform-disable a real invariant just to make CI green.

If failure reveals an architectural problem outside scope, return:
`BLOCKED — ARCHITECT DECISION REQUIRED`

## Tests
Follow the exact validation commands in the current mission first. Do not silently omit a requested gate.

If a local-machine/repository constraint makes a requested full-workspace command unsafe:
- state the constraint;
- run the strongest permitted targeted equivalent;
- never claim the omitted gate passed;
- use native CI as substitute only when the mission permits it.

Do not run high-memory `--all-features` or full-workspace commands merely by habit. Do not refuse an explicitly Architect-approved command because an older CLAUDE.md prohibited it; surface the conflict and follow the current mission or block.

Before commit, normally run the applicable subset:
- targeted trust-boundary tests;
- mission-required regressions;
- `cargo fmt --all -- --check`;
- appropriate Clippy for changed crates;
- frontend tests/TypeScript/build when frontend changes;
- Python tests when relevant;
- `git diff --check`.

Native Linux, Windows, macOS, Frontend, and Python evidence is required for trust-boundary completion when the mission requires it.

Never:
- replace a real security assertion with a mock solely for portability;
- use a fixed sleep as the only concurrency/process proof;
- hardcode GitHub runner paths;
- weaken deadline/expiry semantics;
- treat Linux success as Windows/macOS proof;
- report partial CI as complete.

## Process work
For subprocess work:
- backend owns process identity;
- PID is never caller authority;
- do not kill by process name or port guessing;
- distinguish launch authorization from OS containment;
- preserve explicit process-tree finalization;
- keep cleanup bounded;
- report cleanup failure truthfully;
- treat Drop as defense in depth only.

## Locks and audit
Do not hold authority/catalog/process/lifecycle locks across audit callbacks, provider calls, filesystem mutation, process spawn/wait/termination, network operations, or frontend delivery.

Prefer:
snapshot/reserve → unlock → native operation → generation/identity re-check → bounded state transition → unlock → completion → audit.

Audit must not expose secrets, private principals, grants, execution IDs, sensitive roots, environment credentials, or caller-forged authority.

## Reporting
Use the exact report structure required by the mission.

Always report:
- base / branch / worktree;
- changed files;
- security invariant/claim;
- tests actually run and omitted;
- failures/repairs;
- security self-review;
- Git status and commit SHA;
- push/remote verification;
- authoritative branch and `main` status;
- deferred boundaries.

Never invent test results or repository facts.

Implementation/local green is not checkpoint completion. Completion requires the mission's integration/CI gates plus Architect review.

## Long sessions
Do not stop merely because context is getting low. Preserve state using Git and concise progress notes, then continue after compaction.

Do not create ad-hoc repository state files unless authorized.
Do not resume stale assumptions from an old Claude session; re-establish state from Git and the current mission.

## CLAUDE.md maintenance
Keep this file concise and durable. Do not store current checkpoint SHA, current branch/worktree, transient CI failures, changing roadmap status, competitor/marketing claims, or launch goals.

When this file and `AGENTS.md` overlap, remove duplication from this file rather than creating two competing constitutions.
