# Mission evidence

## Mission

NEXUS-HARNESS-V1

## CLAIM

Harness implementation is ready for the local checkpoint and final evidence stage. Independent acceptance remains pending.

## OBSERVATION

Base main 6e15dee613ea35823d1adbf6d8917ed59eeda583 / tree 7192ba94a1954c2c416572b890f5e0e7af834586. 243 pre-existing refs captured. Three Candidate-10 worktrees clean; 13,123 Candidate-10 evidence files hashed. See preflight receipts and isolation-before.json.

## EXECUTION RECEIPT

`evidence/receipts.jsonl` stores measured executions; `evidence/manifest.json` indexes hashes.

## ARTIFACT

Authorization, inventory, isolation snapshot and logs under evidence/. Git preserves original instructions at the base SHA.

## REVIEW FINDING

Independent review NOT_RUN. Implementer semantic preservation audit below is not independent review.

## ACCEPTANCE DECISION

None. Architect review pending.

## Limitations

Linux local validation only. No hosted CI or live mutation campaigns. Stale tasks/todo.md and tasks/lessons.md are context, not authority. Existing claude-mem contains NEXUS_CONTEXT.md but no SKILL.md; preserved as found.

## Preservation map

Original base AGENTS.md: 28,330 UTF-8 bytes, 1,044 lines. C9 instructions were 29,120 bytes; differences are dated state and the Phase Two profile retained in policy §24. New root: 7,076 UTF-8 bytes, 123 lines, below the 12,000-byte project budget. The detailed normative text in original §§1–19 is retained verbatim in POLICY (stage-e-preservation and structural validator).

| Original section | Final location | Disposition |
|---|---|---|
| 1. Purpose | `.nexus/harness/POLICY.md#original-1` | Original normative text retained verbatim; procedure adds observable receipts. |
| 2. Operating Model | `.nexus/harness/POLICY.md#original-2` | Original normative text retained verbatim; procedure adds observable receipts. |
| 3. The Autonomous Execution Loop | `.nexus/harness/POLICY.md#original-3` | Original normative text retained verbatim; procedure adds observable receipts. |
| 4. Definition of Completion | `.nexus/harness/POLICY.md#original-4` | Original normative text retained verbatim; procedure adds observable receipts. |
| 5. Phase Discipline | `.nexus/harness/POLICY.md#original-5` | Original normative text retained verbatim; procedure adds observable receipts. |
| 6. Trust-Boundary Constitution | `.nexus/harness/POLICY.md#original-6` | Original normative text retained verbatim; procedure adds observable receipts. |
| 7. Git Constitution | `.nexus/harness/POLICY.md#original-7` | Original normative text retained verbatim; procedure adds observable receipts. |
| 8. Worktree Discipline | `.nexus/harness/POLICY.md#original-8` | Original normative text retained verbatim; procedure adds observable receipts. |
| 9. Commit Discipline | `.nexus/harness/POLICY.md#original-9` | Original normative text retained verbatim; procedure adds observable receipts. |
| 10. Remote Verification | `.nexus/harness/POLICY.md#original-10` | Original normative text retained verbatim; procedure adds observable receipts. |
| 11. CI Constitution | `.nexus/harness/POLICY.md#original-11` | Original normative text retained verbatim; procedure adds observable receipts. |
| 12. Anti-Loop Autonomous Failure Recovery Protocol | `.nexus/harness/POLICY.md#original-12` | Original normative text retained verbatim; procedure adds observable receipts. |
| 13. Testing Standards | `.nexus/harness/POLICY.md#original-13` | Original normative text retained verbatim; procedure adds observable receipts. |
| 14. Platform Portability Rules | `.nexus/harness/POLICY.md#original-14` | Original normative text retained verbatim; procedure adds observable receipts. |
| 15. Dependency Discipline | `.nexus/harness/POLICY.md#original-15` | Original normative text retained verbatim; procedure adds observable receipts. |
| 16. Secrets and Credentials | `.nexus/harness/POLICY.md#original-16` | Original normative text retained verbatim; procedure adds observable receipts. |
| 17. Auditability | `.nexus/harness/POLICY.md#original-17` | Original normative text retained verbatim; procedure adds observable receipts. |
| 18. Review Standard for Security-Critical Changes | `.nexus/harness/POLICY.md#original-18` | Original normative text retained verbatim; procedure adds observable receipts. |
| 19. Scope Expansion Is Not Refactoring | `.nexus/harness/POLICY.md#original-19` | Original normative text retained verbatim; procedure adds observable receipts. |
| 20. Final Mission States | `.nexus/harness/POLICY.md#original-20` | Execution states refined per this mission; no self-acceptance or partial completion. |
| 21. Project and Sources of Truth | `.nexus/harness/POLICY.md#original-21` | Remote/ref and worktree identity rules retained. |
| 22. Phase Status | `.nexus/harness/POLICY.md#original-22` | Dated phase assertions remain in base Git history; stale state grants no scope. |
| 23. Protected Refs | `.nexus/harness/POLICY.md#original-23` | All four immutable closure refs retained, accepted/later refs preserved. |
| 24. Supported Validation Profile | `.nexus/harness/POLICY.md#original-24` | Linux-only claim limits retained; supplied Phase Two fail-closed profile also retained. |
| 25. Current CI Gate | `.nexus/harness/POLICY.md#original-25` | Dated workflow list remains in base history; exact-SHA/current-workflow gate retained. |
| 26. Workflow Doctrine | `.nexus/harness/POLICY.md#original-26` | Direct-tool autonomy and independent Architect retained. |
| 27. History | `.nexus/harness/POLICY.md#original-27` | Historical snapshot remains in Git; not live authority. |
| 28. Final Rule | `.nexus/harness/POLICY.md#original-28` | Preserve authority/evidence/history/security/scope and fail closed retained. |

CLAUDE-specific audit: authority/startup/mission/Git/test/reporting requirements route through root, POLICY and procedures; unique trust overclaim, lock lifetime, audit redaction, Drop and explicit test-gate rules moved to POLICY §§29–30. Long-session recovery remains in CLAUDE and recover/handover. Stale phase-specific remote sentence removed in favor of explicit mission remote.

Inventory: `.agents` and `.codex` absent at base; `.claude` has commands/plans/roadmap/settings and three existing skill directories. Full path inventory in evidence/inventory.txt. Existing evidence conventions use docs/evidence, REPORT.md, validation logs and SHA256SUMS; audits use docs/audits. No Markdown/config linter found in scripts/workflows/package scripts; hooksPath holds samples only. Native runtime fmt/Clippy/test/build scripts do not validate this documentation/Python-only harness and are not executed. .gitignore currently hides .nexus and new Claude skills; narrow exceptions are required.

Root mandatory concepts will be retained in AGENTS authority, startup, trust, Git and evidence sections. This map is a semantic implementer assessment; automated anchor/byte checks cannot prove policy equivalence.

## Implementer semantic self-review (not independent acceptance)

- Authority/ownership: both providers load the same explicit mission; no CURRENT
  authority pointer, runtime permission code, governance bypass or implicit
  external action. The validator only reads documents/logs/Git; it never executes
  receipt argv. Skill adapters have one canonical procedure link and no copied
  policy sections. Independent REVIEW remains INCONCLUSIVE.
- Lifetime/recovery: progress records measured checkpoints, receipts identify
  command/candidate and preserve raw output; recover reconciles interrupted
  processes before rerun. A commit cannot embed its own SHA; final clean Git
  identity and validation will be sealed externally at
  `/tmp/nexus-harness-v1-final/final.json`. The final report supplies its hash.
- Fail-closed/error semantics: missing fields/logs, malformed/tampered hashes,
  mismatched identities, duplicate/unknown controls, absent baseline and mixed
  campaign IDs reject. NOT_RUN stays unexecuted; source restoration alone never
  validates a semantic outcome. The parser validates recorded relationships,
  not real-world truth or physical build freshness.
- Privilege/cleanup: no production/runtime/network/CI configuration edits, no
  installed dependencies, no protected-ref mutations. Tests use disposable
  synthetic Git repositories and fixture files only. No Candidate-10 campaign
  or build commands were invoked.
- Race/staleness: before/after snapshots show point-in-time stability, not a
  filesystem lock or concurrency proof. External actors changing protected
  state between snapshots would require investigation. Git optional locks are
  disabled for isolation reads; observed app-owned refs/codex are explicitly
  excluded, all 243 pre-existing non-Codex refs are compared.
- Portability: normal UTF-8 files, relative links, Python stdlib, Git and no
  symlinks/executable-bit dependency. Execution evidence is Linux only;
  Windows/macOS and live Claude loading remain unrun. Provider frontmatter and
  canonical resolution are locally validated.
- Secrets: no environment dumps or credential reads. Targeted new-file pattern
  scan found no private-key/token-shaped values; this is not exhaustive proof.
  Mission-provided paths and public SSH remotes remain inspectable provenance.

## Root preservation details

| Required root concept | Retained root section |
|---|---|
| Governed/local-first; Owner/Architect/executor chain; autonomous bounded work | Authority and purpose |
| No self-expansion; strings not authority; explicit external effects | Authority and purpose |
| Fail closed; current explicit scope vs stale state; mismatch stops writes | Authority and purpose |
| Explicit mission, canonical policy/procedure, Git then verified progress | Mandatory mission startup and continuation |
| Backend authority, paths, grants, process identity, bounded cleanup, observable errors | Trust boundaries (full detailed requirements in POLICY §6 and §30) |
| Exact provenance; explicit remote/ref; protected/frozen refs; destructive Git authority | Git and protected history |
| Claims vs evidence, missing proof, no manufactured green, no partial-run aggregation | Evidence, completion and independent review |
| Local vs hosted green, checkpoint vs implementation, independent review | Evidence, completion and independent review |
| No automatic phase/next mission/main integration | Git and protected history; Evidence, completion and independent review |

## Measured validation ledger

- `stage-c-skills` and `stage-d-skills`: all 14 frontmatter files valid (existing
  bundled quick_validate.py; no packages installed).
- `stage-g-structure-v2`: seven canonical procedures, all templates and mission
  records, original policy preservation and existing Claude skill bytes valid.
- `stage-h-regressions`: 33 tests passed. Includes 117→183 prerequisite mapping
  without mutation execution, semantic status separation, bad/missing evidence,
  fake kills, partial runs, and synthetic protected-ref/source/evidence changes.
- `stage-h-isolation`: 243 existing refs, 3 Candidate-10 worktrees and all 13,123
  Candidate-10 evidence files unchanged. GitHub main was verified read-only at
  startup; no remote writes are authorized or performed.
- `stage-h-scope`: Python syntax valid, only scoped paths, no symlinks, narrow
  ignore exceptions, no targeted secret-pattern matches. This is an implementer
  check; independent semantic/security review remains required.
- All earlier unstaged stage diff checks exited 0. New untracked files are also checked
  after staging before commit. No native Markdown/config linter exists in the
  inspected base scripts/workflows/package configuration.

## Unresolved boundaries

Independent Architect review is pending. Hosted CI, Windows/macOS execution and
live Claude session testing were not run and are not claimed. Existing
claude-mem has no SKILL.md at this base; its tracked content is preserved exactly
rather than repaired outside scope. Historical tasks/lessons remain unchanged
and may contradict current policy; startup explicitly treats them as context.
No implementation defect is currently known. Final clean-commit validation is
pending until the external seal exists; do not infer it from this checkpoint.

## Resolved validation finding

`stage-i-staged-whitespace` exited 2: Git's verbatim worktree porcelain output
ends with a blank record separator. The raw preflight log was not edited.
An evidence-local `.gitattributes` entry exempts only blank-at-EOF checking for
`preflight-4.stdout.txt`; all other whitespace checks and all source/docs remain
unchanged. Its original receipt SHA-256 still validates. This is log
preservation, not a passing claim for the initial failed check.

## Local checkpoint and final delivery binding

Implementation checkpoint: `db0f603be16588b431583a7cacd14072e86b3819` / tree `35d89e8d91fcf71f839a437f5dfcce7f02f07eec`.
Its parent is the verified GitHub main base. `stage-i-checkpoint` is the measured
local commit receipt. The next local commit is limited to this mission's
handover/progress/review/evidence records. No further implementation is planned.

The repository records intentionally identify this actual checkpoint rather
than inventing a final self-referential SHA. Final clean delivery identity,
full local verification and final READY_FOR_REVIEW status are sealed separately
at `/tmp/nexus-harness-v1-final/final.json`, with full-field final HANDOVER.md,
receipts.jsonl and logs in that directory. The final response supplies the seal
hash. This artifact proves local Git identity/validation at its timestamps; it
does not prove independent acceptance, hosted CI, portability or authorization.
Missing external evidence must be treated as missing, not inferred from prose.

GitHub main remained `6e15dee613ea35823d1adbf6d8917ed59eeda583` at the final
read-only comparison (`stage-i-remote`, exit 0). No push was performed.
