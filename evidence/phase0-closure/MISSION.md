# Authorized mission (verbatim, as issued by the Architect)

```text
MISSION: P0-FINAL-GATE-CLOSURE
MODE: AUTONOMOUS MULTI-AGENT IMPLEMENTATION, VALIDATION AND EVIDENCE

You are Claude Code, the IMPLEMENTATION EXECUTOR for Nexus AI OS.
ChatGPT is the ARCHITECT and INDEPENDENT SECURITY REVIEWER.

The Owner explicitly requests an uninterrupted, multi-agent implementation
and repair loop, with substantially fewer manual handoffs.

This mission expressly expands the earlier small repair missions into
one named Phase Zero closure programme.

It authorizes:
- integration of ONE already independently reviewed, exact candidate;
- its designated post-integration validation;
- parallel preparation of the remaining Phase Zero technical closures;
- autonomous local testing, diagnosis and repairs within this scope;
- normal commits and pushes on the authorized working branches;
- bounded hosted validation of stable closure candidates;
- a consolidated GitHub evidence package.

It does NOT delegate:
- independent security approval;
- integration of any future, unreviewed SHA;
- acceptance of unresolved security risks;
- checkpoint completion;
- Phase Zero completion;
- Phase One implementation.

Read the entire mission before acting.

======================================================================
1. GOVERNANCE AND THE EXPLICIT SCOPE EXPANSION
======================================================================

Read AGENTS.md, CLAUDE.md and any applicable descendant instructions.

AGENTS.md remains the engineering constitution. Its current-state snapshot
is historical; establish current state from verified Git/GitHub evidence.

The Architect explicitly expands the mission to the workstreams in
section 5. They are authorized implementation work, not merely proposals.

This mission supersedes the earlier:
- one-small-repair scope limits;
- requirement to return for permission after each ordinary repair;
- per-dispatch permission requirement, within section 7's validation budget;
- prohibition on preparing another isolated Phase Zero candidate while
  the current checkpoint awaits its final independent review.

That last exception authorizes UNINTEGRATED CANDIDATE PREPARATION.
It does not declare FG1 complete or authorize another authoritative
integration.

No permanent constitution rewrite is needed to use this larger envelope.
Do not modify permanent governance rules.

Preserve:
- A STRING IS NEVER AUTHORITY.
- Backend-owned authority and explicit grants.
- Narrowing, revocation, generation/context checks and owned cleanup.
- Fail-closed behavior.
- Accurate security claims and non-claims.
- Existing filesystem/process/workspace trust boundaries.

Never:
- modify main;
- force push;
- rebase, amend or squash reviewed/shared history;
- use git reset --hard or git clean;
- delete branches/worktrees containing work;
- overwrite unexpected Owner changes;
- weaken security tests or add continue-on-error;
- reopen a Phase Zero-closed capability;
- start Phase One.

Do not stop for permission before an ordinary edit, test, commit, push,
in-scope repair or authorized validation step.

======================================================================
2. CURRENT FROZEN STATE
======================================================================

Repository:
Nexus-os2026/nexus-os

Authoritative worktree:
/home/nexus/NEXUS/nexus-os

Authoritative branch:
rebuild/phase0-trust-boundary

Expected authoritative starting SHA:
98fbb6a369daa9f3f3998a8555fec7cf67078664

Expected main:
80640bba41e74c17abbf4eb71eafa88bd1ade8db

Original FG1 branch:
implement/p0-fg1-j1-server-withdrawal
1b049e15e27666f573af24bb4ac0a579dbf860c0

CI branch:
implement/ci-fast-local-final-cloud
9bbe4d8abdc7668c8f31069324b22ab315922f03

Composition branch:
implement/p0-fg1-ci-composition
2b47bb09e13cdbee167372c21ad12c51273c5f81

Reviewed repair branch:
repair/p0-fg1-protocols-binary-name

APPROVED CANDIDATE:
71c47acbf3f8ee8210109587b3229f8d89067b6b

APPROVED TREE:
824669d4bea9df5a24e8065081fedb109f0fd66f

Candidate's sole parent:
2b47bb09e13cdbee167372c21ad12c51273c5f81

Reviewed ci.yml blob:
6d1fccbcfe4580e83f562c28ee65d67444f4656d

Reviewed instruction blobs:
AGENTS.md: bc070681ab177b7653d4b3cfeabe3d29c1962e38
CLAUDE.md: e3685becf2fd14232525462274b3efde16a5e877

Accepted evidence:
- Fast-local #6: 36443707802, success, attempt 1, candidate 71c47acb.
- Hosted #107: 36446149987, success, attempt 1, candidate 71c47acb.
- Hosted #106: 36437157020, failure, attempt 1, preserved as evidence.

PR #15 must remain:
- open;
- draft;
- unmerged;
- auto-merge disabled;
- original FG1 head;
- main base.

Verify the GitHub remote by its actual URL.
Do not assume origin is GitHub; origin/gitlab have been mirror remotes.
Use explicit destinations for pushes.

If initial refs or expected source differ, preserve the evidence and
investigate before any write. Do not repair provenance by resetting it.

======================================================================
3. EXACT FG1 INTEGRATION AUTHORIZATION
======================================================================

ARCHITECT APPROVED: INTEGRATE CURRENT CHECKPOINT

Checkpoint:
P0-FG1, including the reviewed CI composition and protocols-name repair.

This integration token authorizes ONLY:

rebuild/phase0-trust-boundary:
98fbb6a369daa9f3f3998a8555fec7cf67078664
    ->
71c47acbf3f8ee8210109587b3229f8d89067b6b

Expected resulting tree:
824669d4bea9df5a24e8065081fedb109f0fd66f

Before integrating:
- verify the repository, remote and authoritative worktree;
- verify all frozen refs and PR #15;
- verify candidate parent/tree and instruction/workflow blobs;
- verify #107 remains successful on the exact candidate;
- verify fast-forward ancestry;
- verify no unexpected tracked/untracked changes;
- recheck the remote authoritative head immediately before the push.

Perform the authoritative update with git merge --ff-only.
Push normally to the GitHub authoritative branch only.
Do not merge a PR or move main, FG1, CI, composition or repair branches.

Then verify local and GitHub authoritative HEAD and tree.

This section also authorizes EXACTLY ONE designated post-integration
hosted dispatch:

gh workflow run ci.yml \
  --repo Nexus-os2026/nexus-os \
  --ref rebuild/phase0-trust-boundary \
  --raw-field candidate_sha=71c47acbf3f8ee8210109587b3229f8d89067b6b

Before dispatch, record existing matching runs and recheck the ref.
Send once. If the response is ambiguous, query GitHub before doing anything
else; do not send a duplicate request.

Verify all five native jobs, checkout/workflow identity, actual test
execution and tested tree == authoritative tree.

Do not rerun #107 or #106.

If this post-integration run fails:
- preserve the failed run;
- report FG1 as awaiting successful post-integration verification;
- diagnose it;
- prepare any in-scope repair only on an unintegrated working branch;
- do not move authoritative again under this token.

Safe work on the other authorized streams may continue.

If resuming after compaction or interruption, use the saved ledger and
GitHub evidence to determine which operations already happened. Never
repeat a push or dispatch merely because the conversation was compacted.

Do not declare P0-FG1 COMPLETE.
ChatGPT will review the post-integration evidence.

======================================================================
4. TEAM, WORKTREES AND EXECUTION OWNERSHIP
======================================================================

Use all available useful agent capacity.

Organize the work into six streams, with a coordinator:
1. Webview, navigation, frames and IPC origin.
2. Configuration encryption, vault startup keys and stored secrets.
3. Egress, credential transport, peers and helper processes.
4. Standalone binaries and deployment/install withdrawal.
5. Approval semantics and residual capability-measurement closure.
6. Reliability, dependency evidence, regression coverage and claim inventory.

Rotate available agents into adversarial review after implementation.
A Claude reviewer is an internal review pass, not Architect approval.

Coordinator responsibilities:
- establish the finding-to-file ownership map before concurrent edits;
- own shared files, composition, pushes, CI and the evidence ledger;
- prevent two agents editing the same file concurrently;
- inspect every component diff before composition;
- preserve one understandable root cause per commit;
- retain component provenance.

Use isolated worktrees/branches rooted at the approved candidate SHA.
The main combined candidate branch is:

implement/p0-final-gate-closure

Use a new worktree beneath:
/home/nexus/NEXUS/

If a proposed branch/worktree already exists, verify its ownership and
provenance. Do not delete or overwrite it.

Ordinary local merges of the authorized component branches into the
combined candidate are authorized. No history rewriting.
Resolve conflicts only where both intended behaviors are understood and
the result remains inside this mission's contracts.

Only the coordinator pushes the combined candidate.
Avoid pushing each worker branch merely to trigger duplicate fast runs.

Use one heavy-build coordinator. Do not launch multiple full workspace
builds or Builder assemblies concurrently on the same modest machine.
Parallelize investigation, implementation and lightweight checks.

No new PR is authorized. Preserve all existing PRs.

======================================================================
5. ARCHITECT-APPROVED TECHNICAL CLOSURE CONTRACTS
======================================================================

Use the current Final-Gate dossier, authority inventory and actual code.
Correct their stale classifications using evidence.

Every changed file must map to one of the following contracts, necessary
tests, minimal supporting build/validation changes, or associated active
documentation.

A. CONFIGURATION KEY

No NEW or CHANGED credential may be written using a key derived only from
ambient/non-secret values such as HOME, USER, USERNAME, HOSTNAME or constants.

Reject missing/empty explicit key material for such writes.

Use established project cryptography and approved startup secret sources.
Do not invent cryptography or introduce an unreviewed key-management system.

Preserve existing credentials and legacy ciphertext.
A legacy read compatibility path must be explicit and tested.
No silent migration, forced re-encryption, guessed replacement key,
credential deletion or fallback to a fabricated empty configuration.

If secure writing cannot be established, refuse the write clearly.
Preserve the rule that interface saves require an existing, loadable
security baseline; missing, empty, malformed or undecryptable configuration
does not authorize a save.

Key-quality claims must match what is actually enforced.

C5. RESIDUAL CAPABILITY-MEASUREMENT ROUTE

Close cm_run_ab_validation using the established Phase Zero bounded denial
pattern, before argument use, credential reads, client construction or
provider contact.

No NVIDIA/OpenRouter credential may fall through to a Groq endpoint.
Do not redesign the evaluator or reopen other cm_* commands.

D. PRIVILEGED WEBVIEW / NAVIGATION / IPC

Treat these as one boundary.

Only the verified application document may receive privileged Nexus
application-command authority.

Remote pages, Builder output, arbitrary loopback services, about:blank,
srcdoc documents and child frames do not gain authority from a URL,
window label or caller assertion.

Prevent untrusted content from replacing the privileged application
document. Enforce application-command origin restrictions using trustworthy
native/backend context, not frontend-supplied origin fields.

Use a restrictive CSP as an additional control.
Do not claim a CSP alone proves the IPC boundary.

For Phase Zero, disabling embedded active previews or external navigation
is authorized wherever safe separation cannot be established.
Preserve the trusted application shell and existing governed backend work.

Test top-level navigation, redirects, new windows, subframes and attempts
to invoke application commands from untrusted content.

Verify relevant Tauri/Wry/platform behavior from the pinned implementation
and primary documentation. Native behavior must be exercised where the
claim depends on it. JavaScript mocks/source scans alone are insufficient.

If a platform boundary cannot be proved, keep that surface unavailable and
report the exact remaining limitation.

E. OPERATOR VAULT KEY SOURCE

Keep security.key_file as an operator-controlled STARTUP source only.
Do not reopen security-section editing or frontend key-file selection.

Validate the opened source, regular-file status, bounded size, applicable
ownership/access permissions, key format and integrity preconditions.
Reject empty, malformed or insecure sources.

Bind validation to the source actually read. Do not validate one pathname
and then silently reopen a different target.

Symlink/redirect compatibility must not bypass these checks.
If a compatibility mode cannot satisfy them, reject that mode explicitly
and document the consequence.

Do not modify real operator keys or migrate the Owner's vault.

B / F. DESTINATIONS AND NEXUS LINK

A syntactically valid URL, DNS answer or host:port is not an egress grant.

Preserve existing approved backend authority and legitimate configured
provider/local-model paths where that authority can be established.
Do not promote frontend values or serialized records into new grants.

Close caller-selected network routes that lack such authority, including
aliases and indirect routes.

Preserve or strengthen existing scheme, effective-port, host/path,
redirect, timeout and response-size controls.

Where address/DNS policy is required, authorize the actual connected
destination and redirects; a preliminary lookup alone is insufficient.

Keep unauthenticated/unapproved Nexus Link transfer unavailable.
An empty peer policy must not authorize all peers.
Do not invent pairing/authentication infrastructure for Phase Zero.

C / H. SECRETS IN ARGV AND AT REST

Remove reachable credential-bearing subprocess arguments.

Use existing approved transport/secret mechanisms, preserving destination
binding, TLS, time/size limits, failure propagation and redaction.
If a safe bounded implementation is unavailable, close the affected route.

Never pass a secret through shell interpolation or log it in diagnostics.
Do not introduce arbitrary curl configuration input as a workaround.

Prevent new plaintext OAuth/token persistence outside an approved secret
store. Preserve existing token files and credentials; do not delete or
silently migrate them.

Disable an affected connection/persistence operation if safe storage is
unavailable. Do not claim that disabling writes encrypts historical data.

G. APPROVAL CHANNELS

A caller's boolean, name, approved_by value or webview message is not
independent human approval.

Keep consequential operations unavailable unless the backend can verify
existing explicit authority bound to the action, content and execution
context.

Preserve EOF -> Abort and all existing Phase Zero denials.
Do not invent a new approval architecture to retain an unsafe operation.

I. HELPER EXECUTABLES AND LIFECYCLE

Remove reachable reliance on untrusted ambient helper selection and
unowned detached launches.

Use existing approved executable authority and retained native lifecycle
ownership where applicable. Otherwise refuse the operation.

In particular, do not leave an unowned ollama serve launch enabled.
Connecting to an independently managed, authorized local service is a
separate operation from starting that service.

No cleanup by caller PID, process name or guessed port.
Preserve bounded termination/reaping and truthful cleanup failures.

J2 / J3 / J4 / J5. STANDALONE SURFACES

Architect disposition for Phase Zero:
withdraw standalone execution/deployment paths that bypass the governed
desktop trust boundary.

Inventory the protocols server, its nexus-os alias, nexus-cli, nx and all
effective entry points and operational consumers.

Where those entry points can expose listeners, execute tools/processes,
or consume ambient project configuration outside the governed boundary,
make the entry point fail closed and withdraw its active install/deploy
instructions and recipes.

Apply the J1 withdrawal principles: no hidden flag, environment variable,
alternate alias or packaging path may reactivate a withdrawn surface.

Preserve embedded libraries and existing governed desktop behavior.
Do not broadly delete modules or claim latent libraries are now governed.
Do not stop existing user deployments, remove data, revoke credentials,
build/publish images or operate external infrastructure.

J1 remains withdrawn. Its accepted defensive binary-identification checks
must not be weakened.

K / RESOURCE BOUNDS. RELIABILITY

Repair demonstrated blockers with evidence-backed, bounded changes.

Test isolation, deterministic synchronization and narrowly necessary
portability/build repairs are authorized.

Preserve test assertions and real platform semantics.
Do not manufacture a pass using sleeps, retries, global single-thread
settings, warning suppression or weakened output contracts.

Review the dossier's unbounded resource surfaces. Bound or close reachable
high-amplification operations; do not use destructive log/data cleanup.

Investigate the GPU/stderr issue without changing host drivers or hiding
test output. If it is an environment-specific product limitation rather
than a safe in-scope code repair, record evidence and proposed disposition.

DEP. DEPENDENCY EVIDENCE

Re-measure security advisories on the closure candidate's actual lockfiles.
Do not reuse an audit count from main as if it described this candidate.

Minimal, directly relevant security dependency updates are authorized.
Inspect lockfile churn and preserve feature/security requirements.

No broad cargo update, npm audit fix --force, unrelated upgrades or
unreviewed major toolchain/framework migration.

When a safe minimal fix is unavailable, provide the exact advisory,
dependency path, affected configuration/reachability and proposed
disposition. Claude may not accept the risk itself.

Use existing approved verification tooling or isolated diagnostic tooling.
Do not weaken scans or introduce global advisory ignores.

GENERAL LIMIT

This is Phase Zero trust-boundary closure, not feature restoration or a
new architecture.

Preserve useful already-governed functions.
Do not disable the entire application to obtain an empty attack surface.

If a genuinely new trust-model choice is required, isolate that item and
continue all other safe work. Collect the decision requests together.

======================================================================
6. AUTONOMOUS REPAIR LOOP
======================================================================

For each root cause:

inspect actual reachability
-> establish evidence
-> implement the smallest complete in-scope repair
-> run focused positive and relevant negative tests
-> review callers, failure paths and platform differences
-> perform an adversarial internal review
-> commit
-> compose
-> run regressions
-> push the stable combined candidate
-> inspect fast-local CI directly
-> diagnose and repair until its exit conditions are met.

Do not return to the Owner after each step.

Preserve AGENTS.md's anti-loop rules:
- every failed iteration must add information;
- after two unsuccessful targeted repairs of the same signature, enter
  Deep Diagnostic Mode;
- do not submit a third speculative patch;
- use --no-fail-fast diagnostics when later failures are hidden;
- distinguish new regressions, pre-existing failures and infrastructure
  failures using evidence.

No empty reruns or meaningless commits merely to obtain another CI attempt.

A blocked workstream does not automatically block safe independent work.
Finish the other authorized work and consolidate the unresolved decisions.

Continue through context compaction. Persist concise working state and
re-establish it from Git/GitHub after resuming.

======================================================================
7. VALIDATION AUTHORITY AND BUDGET
======================================================================

Fast-local:
- retain [self-hosted, Linux, X64, nexus-local];
- retain Linux Rust checks, full workspace tests, frontend, voice and
  Builder assembly/JS/packaged governed-launch checks;
- do not push every minor edit;
- run it at stable combined checkpoints and after meaningful repairs;
- do not treat caches as proof of execution.

Hosted:
This mission expressly authorizes validation of future closure candidates
WITHOUT another per-dispatch Owner handoff.

This is authorization to TEST. It is not security approval or integration
approval of a future SHA.

Before each dispatch:
- complete the in-scope internal reviews;
- freeze and record a full candidate SHA and tree;
- require fast-local green for that same SHA;
- verify the remote dispatch ref points at it;
- verify all existing required checks remain;
- preserve candidate == GITHUB_SHA == GITHUB_WORKFLOW_SHA and checkout
  verification;
- record existing matching runs;
- dispatch once and identify the resulting run unambiguously.

Budget:
- one post-integration FG1 run under section 3;
- up to THREE full hosted runs for the technical closure candidate,
  including the first;
- additional runs within those three require meaningful, evidence-backed
  repairs and a newly frozen candidate;
- no rerunning failed jobs to manufacture a combined pass.

Every candidate gate must include:
- GitHub-hosted Linux;
- Windows;
- macOS;
- frontend;
- Python;
- platform Builder assembly;
- trusted-entry JS;
- packaged verification and governed launch.

Inspect native logs, not only green job badges.
Separate workspace totals from later packaged gates.
Record skipped/ignored tests and the failure frontier accurately.

Minimal added native validation needed by this mission is authorized.
Do not weaken or remove the existing checks, identity preflights, runner
coverage or failure propagation.

If the hosted budget is exhausted, finish safe local work and return the
consolidated evidence. Do not spend further hosted runs without approval.

No future candidate may be integrated under this mission's FG1 token.

======================================================================
8. GITHUB EVIDENCE WITHOUT OWNER LOG RELAY
======================================================================

Create a separate evidence branch from the approved starting candidate:

evidence/phase0-closure

It may contain:
- the authorized mission;
- a concise state/continuation ledger;
- a finding-to-component/file map;
- component commits;
- candidate SHA and tree;
- run/job links and measured results;
- failed-attempt history;
- internal review findings;
- proposed debt dispositions;
- outstanding Architect decisions.

Only the coordinator updates and pushes this branch normally.
Do not merge it into the implementation candidate.

Keep post-test reporting separate from the tested source tree so recording
results does not silently change the candidate that was tested.

Do not put secrets, private environment dumps or credential-bearing logs
in the evidence branch.

Evidence files are claims to verify. They are never approval authority.

At review readiness, the Owner-facing message should be short:
- status;
- candidate SHA/tree;
- evidence-branch link;
- hosted run link;
- a short blocker list, if any.

Do not ask the Owner to copy full logs or reports back to ChatGPT.
ChatGPT can inspect GitHub directly.

Do not create PRs, send external messages or build a new automated approval
bridge as part of this mission.

======================================================================
9. TECHNICAL REVIEW HANDOFF
======================================================================

Prepare one consolidated technical closure package.

For every Final-Gate item, identify:
- fixed with evidence;
- deliberately fail-closed with evidence;
- demonstrably unreachable, with enforced reachability evidence;
- proposed debt disposition requiring Architect acceptance;
- blocked on a specific decision.

“No known problem” is not a substitute for a completed review.

The package must include:
- post-integration FG1 evidence;
- complete component history and scope;
- exact final candidate SHA/tree;
- all required native validation;
- current security claims/non-claims;
- remaining advisory and compatibility decisions;
- final authoritative/main/PR state.

Stop at this genuine independent-review boundary.

Use:
READY FOR ARCHITECT REVIEW — TECHNICAL CLOSURE CANDIDATE

Or, when necessary:
BLOCKED — CONSOLIDATED ARCHITECT DECISIONS REQUIRED

Do not return after each ordinary repair.
Do not declare FG1, the technical Final Gate or Phase Zero complete.

======================================================================
10. MANDATORY FINAL DOCUMENTATION STAGE
======================================================================

The repository-wide documentation truth audit remains mandatory.

It may execute only AFTER:
- technical blockers have approved dispositions;
- technical changes have received explicit integration approval;
- they have been integrated;
- post-integration native evidence has been independently accepted by
  ChatGPT.

Inventory and notes may be prepared earlier, but do not claim the final
documentation exit audit is complete against an unapproved technical state.

When ChatGPT explicitly releases this stage, its scope is already defined:

Audit all materially relevant tracked active documentation:
- root README;
- AGENTS.md current-state sections;
- CLAUDE.md adapter accuracy;
- docs and docs/security;
- architecture, Builder and deployment documentation;
- install instructions and examples;
- roadmap/status/phase references;
- feature lists, diagrams, counts and limitations.

Preserve permanent governance rules.
Preserve historical evidence, labelling it clearly where needed.

Distinguish:
- implemented and verified;
- implemented but disabled in Phase Zero;
- planned;
- withdrawn/deprecated;
- unsupported;
- known debt;
- explicit non-claims.

Re-measure counts before publishing them.

Include the Phase One through Phase Six roadmap using the verified final
codebase and the Owner's established high-level directions.
Do not describe planned capabilities as implemented.

Prepare the Phase Zero completion record for Architect review without
asserting completion prematurely.

Documentation changes require their own exact candidate, validation
evidence and independent approval before authoritative integration.

Only ChatGPT may ultimately declare:
PHASE ZERO COMPLETE.

No Phase One work is authorized.

======================================================================
11. START NOW
======================================================================

Perform preconditions, execute the exact approved FG1 integration,
coordinate the authorized agents, and work through the technical closure
programme.

Use brief progress updates.
Do not ask the Owner to manage agents or transport CI evidence.
Do not stop because a routine repair needs another commit.

Persist until the consolidated technical candidate is ready for independent
review, or safe progress is exhausted and the remaining decisions have
been collected into one evidence-backed handoff.
```
