# Internal adversarial reviews

Each component is reviewed by another workstream agent, read-only toward the
component worktree. A review is an internal check, never approval. Findings
are dispositioned by the coordinator; blockers go back to the component
owner for repair and are re-verified.

## S5 (items G, C5), reviewed by S4 — range `63eb0d07..3f712633`

Verdict: **NOT READY**.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | blocker (repaired, re-review pending) | Startup `load_prebuilt_agents` registers the 12 prebuilt autonomy-6 manifests (fresh install, or after a row is deleted); `execute_agent_goal` then runs them and `tool_call_autonomy` grants 6. Confirmed in a scratch probe. The "L6 unavailable" claim does not hold. | Repair assigned to S5: no autonomy-6 manifest registered at startup, consistent across restarts; refuse registered autonomy 6 in goal assignment, the autonomous loop and tool-call autonomy; behavioural test of the real startup order; claims corrected. |
| 2 | should-fix | Warden review: with L6 unavailable the Warden never runs, and an inactive Warden means Allow, so `governance.enable_warden_review = true` silently allows. | Coordinator decision: when review is enabled (default off) and no Warden can run, fail closed with a bounded reason. Repair assigned to S5. |
| 3 | should-fix | `fg_approval.rs` guard code sits where the production scanners read it. | Move to `fg_approval/tests.rs` (layout used by S1-S4 and now S6). |
| 4 | should-fix | The self-improvement report counts recorded acceptances as applied improvements. | Repair assigned to S5 (desktop side). |
| 5 | note | Consent approvals over IPC still release HITL-gated Phase Zero steps (bounded by the Phase Zero executor's action set). | Recorded as an explicit non-claim. |
| 6 | note | The label test does not cover the wrappers; domain functions still accept `*_by` strings. | Repair assigned to S5 (remove the parameters). |
| 7 | note | Two guards pin fixed file lists. | Compare with the directory at run time (with 3). |
| 8, 9 | note | Benchmark-only reverse key routing; other open approval-like commands (inert or bounded). | Recorded for the Architect package. |

Verified correct by the reviewer: the C5 closure and the absence of any
cross-provider key fall-through; the three `ApprovalRequired` closures; L6
create/start/approve/restore refusals before state changes; fixed resolver
label and removed kernel forward; E6 (EOF and read errors abort); the early
bounds; no out-of-region edits.

## S4 (items J2-J5), reviewed by S5 — range `63eb0d07..9b19d6c5`

Verdict: **no blocker; acceptable for composition** (composed at `f3c91e0a`).

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | should-fix | Five kept benchmark binaries (`nim_cloud_bench`, `cloud_models_bench`, `inference_consistency_bench`, `local_vs_cloud_battle`, `real_agent_validation`) send the `GROQ_API_KEY` value to the NVIDIA endpoint as a bearer token on curl's command line (pre-existing). | Coordinator decision: withdraw these five on the J1 pattern (repair assigned to S4); they leave the D3 "kept" request. |
| 2 | should-fix | The alias guard does not pin the gateway's public constructors (`http_gateway`, `build_router`). | Add both rows (S4). |
| 3 | should-fix | README still advertises Docker/Helm deployment and says "the standalone command-line binaries are withdrawn" while developer/benchmark binaries remain. | README lines assigned to S4 (Phase Zero wording, no deletion of history). |
| 4 | note | `dump_config` printed the stored NVIDIA key. | Already fixed by S2 (composed). |
| 5 | note | Build scripts, `[[bench]]` targets and dot-directory CI configurations are not inventoried. | Guard extensions assigned to S4. |
| 6 | note | D3 evidence: `nexus-swarm-healthcheck` and `sg5_probe` execute processes or read ambient configuration. | Architect package (D3). |
| 7, 8 | note | Dockerfile base-image pull before the failing step (as J1); commit wording about capture/input code. | Recorded; dossier wording corrected by the coordinator. |

Verified correct by the reviewer: all 12 withdrawn entries are the exact J1
source, read nothing before denying and are pinned per package and
workspace-wide; behavioural tests run only identified absolute binary paths
with cleared environments and bounded waits; J1 identification unchanged; no
reactivation route (sidecar, npm bin, pyproject script, cargo alias, feature
flag); Windows/macOS comparisons normalise line endings and separators.

## S1 (item D), reviewed by S2 — range `63eb0d07..14bde0ed` plus the uncommitted live harness

Verdict: **changes requested**. The application-command ACL is the right
primary control; the reviewer found no path by which non-app content obtains
command authority in the reviewed code.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | blocker (for closing D) | The live native harness is uncommitted and not in CI, runs Linux dev mode only, and two checks pass even with the protection removed (wry 0.54.4 already refuses new windows without a handler; sandboxed frames never receive the key). | S1: commit the harness; every live check must fail with its protection removed (negative controls recorded) or be labelled a regression check only; a Windows cross-origin non-sandboxed subframe proving ACL rejection; production-origin coverage or a stated limitation; CI steps for Linux (Xvfb), Windows and macOS applied by the coordinator. |
| 2 | should-fix | Comments and a commit message over-claim how subframes are refused. | Per-platform mechanisms stated; frontend `sandbox=""` named as load-bearing. |
| 3 | should-fix | The navigation predicate admits more than the app origin (any `tauri://` host, `tauri.localhost` on any port, `data:`, any `about:`, debug loopback on any port). The live harness showed WebKitGTK follows a server redirect without consulting it. | Exact per-platform app origin; exact dev origin only in dev mode; `about:blank`/`about:srcdoc`; `blob:` kept; `data:` denied; variant tests. |
| 4 | should-fix | Guards that formatting or additions can fool (CSP substring checks, ACL sampling, capability files not pinned, comment-blind wiring check, iframe parsing, import spellings). | Strengthen each guard. |
| 5 | should-fix | Settings "Test" reports "Connected" for any value longer than four characters. | Say format only, not verified. |
| 6 | note | The privileged document loads a Google Fonts stylesheet; the CSP allows both Google origins. | Coordinator decision: remove the remote stylesheet and the origins (system fonts), unless vendored. |
| 7, 8 | note | Stale regeneration note; harness markers written under the real HOME; Cargo.lock edge and lib.rs module line outside the listed regions. | Temp dir for markers; lockfile edge (existing package) and module line accepted. |
| 9, 10 | note | Tauri 2.10.3 exempts the IPC channel fetch command from the ACL (no channels used); feature losses documented. | Dossier non-claim. |

The live harness (uncommitted at review time) had already found that on Linux a
server redirect can navigate the privileged document off-origin; S1 is
repairing that.

## S3 (items B, C argv, F, I), reviewed by S4 — range `63eb0d07..c028a625`

Verdict: **NOT READY** (three blockers, each with a small local fix).

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | blocker | `messaging_connect_platform` returned reqwest errors that include the URL carrying the stored Telegram bot token. | Already fixed on the combined branch by S2 (`c9f82ff5`, all branches use `without_url()`), which the reviewer could not see from S3's branch. |
| 2 | blocker | The in-process credentialed POST has no total time bound (client timeout bounds headers and each read separately; a dripping body returned after 5.4 s with a 1 s timeout). | S3: request-level total timeout, slow-drip test, corrected claims. |
| 3 | blocker | Four new source guards split text before normalising CRLF and fail on a Windows checkout. | S3: normalise line endings first; prove with CRLF copies. |
| 4 | should-fix | Model registration after a download posts to a hard-coded `localhost:11434`, not the authorized Ollama address. | S3: use the authorized address; guard it. |
| 5 | note | Other credential-bearing reqwest providers follow redirects (Anthropic's `x-api-key` survives a cross-host 307/308). | S3: no-redirect policy for those clients, or a recorded non-claim. |
| 6 | note | Messaging clients have no timeout and read unbounded bodies. | S3 for send/poll; the connect command (S2's) by the coordinator. |
| 7 | note | I5 cleanup covers a normal exit only; Ollama pull/chat curl children are bounded (900 s) but not registered; a test closes the global registry. | Non-claim scoped to normal exit; local registry in tests. |
| 8 | note | Remaining helpers are bare `curl` from PATH. | Architect request 1 (PATH as operator configuration). |
| 9 | note | `tools_execute` reveals whether tool credential variables are set before refusing; a comment is inaccurate. | S3: refuse first if the level guard is preserved, else non-claim; fix the comment. |
| 10-14 | note | Setup-wizard UX after the Ollama closure; curl-site guard scope; the desktop build's reqwest uses the OS trust store and system proxy (feature unification), not bundled roots; search redirects may downgrade to http; a progress callback lacks a panic guard. | Claims corrected; https-only redirects for search; others recorded. |

Verified correct by the reviewer: the 14 closures; the Ollama address rule
(normalisation and refusal cases); agent WebFetch and SearXNG; the six
refused external tools; MCP credential refusal; unweakened C5B tests; Nexus
Link policy; credentials off argv for the four providers; the credential
curl-site classification; no `ollama serve`/`which`; curl reaping; in-region
edits only.

## S2 (items A, E, H), reviewed by S6 — range `63eb0d07..b087dbca`

Verdict: **no blocker in S2's changes; acceptable as composed** (`26843f7f`).

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | blocker-class, pre-existing | Messaging send/poll returned reqwest errors carrying the Telegram token URL. | Fixed on the combined branch by S3 (`messaging_transport_error`, guarded). |
| 2 | should-fix | Refusal tests would write into the real `~/.nexus` (and contact services) if a fix regressed or during a negative control. | S2: path-injected helpers or a child process with a temporary HOME. |
| 3 | should-fix | A hand-written plaintext `config.toml` is copied verbatim into an unencrypted backup. | S2: copy only an encrypted envelope or into an encrypted archive; otherwise skip and report. |
| 4 | should-fix | Non-interface configuration saves discard the save outcome, so plaintext encryption under the ambient key or re-keying happens without report or audit. | S2: report and audit every outcome other than a plain write. |
| 5, 6 | note | Registry entries and the setup-wizard Ollama path. | Resolved in composition. |
| 7, 9, 12, 13, 15 | note | Unguarded credential-field completeness; operator/ambient key derivation collision; case-sensitive API Client secret detection; corrupt vault rows reported as a wrong key; env-lock hygiene in tests. | Cheap hardening assigned to S2. |
| 8, 10, 11, 14, 16, 17 | note | Userinfo in URLs; first-run and lost-update races; unbounded config/token reads; `key_env` behaviour change; backup exclusion by name; CLI outcome (CLI withdrawn). | Dossier non-claims. |

Verified correct by the reviewer: legacy derivations byte-identical with
pinned vectors; the credential gate (new, changed, moved, whitespace-only,
cleared-then-re-added); files never overwritten when unreadable; owner-only
atomic writes; bounded, descriptor-bound key-file validation with identity
re-check; read-only vault verification before use; OAuth, deploy and Supabase
closures; messaging connect using only the stored token; backups skipping the
six stores with owner-only archives; all hunks within granted regions.

### S5 repairs (`3f712633..bd5ba357`, composed at `0d08a380`)

- F1: no autonomy-6 prebuilt manifest is loaded at startup (first run and
  every restart register the same 42 agents; earlier L6 rows stay untouched);
  goal assignment, the autonomous loop and tool-call autonomy refuse an L6
  agent before any state change; behavioural startup-order test.
- F2: Warden review enabled with no runnable Warden denies with a bounded
  reason; disabled review unchanged.
- F3: guards moved to `fg_approval/tests.rs`. F4: the self-improvement report
  counts only applied changes; `cycles_run` counts runs. F6: the consent
  resolver label is fixed inside the consent module. F7: guard sources are
  read from their directories.
- Negative controls recorded by S5 for every behavioural test.
- Coordinator composition commit `910063d0`: the consent module checks the
  kernel's simulation and arena limits (no desktop copies); `093c5642`
  registers the S5 closures and guards. Re-review assigned to S6.

### Re-review of the S5 repairs and the composition commits, by S6

Verdict: **no blocker**. No remaining route registers, restores, starts,
schedules or gives a goal or tool call to an autonomy-6 agent (each route
checked). The composition commits `0d08a380`, `910063d0` and `093c5642`
weaken no assertion; the composed bounds satisfy both streams' guards.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | should-fix (unreachable today) | The scheduled executor restarts a stopped registered agent (and records a skip audit) before the goal refusal. | Coordinator composition commit: the L6 check first in `ScheduledGoalExecutor::execute`, with a test. |
| 2 | should-fix | Warden fail-closed is satisfied by any caller-created agent named `nexus-warden` (the only agent that can match in production). No authority gain (the review toggle is interface-owned), but the claim and the audit trail are wrong. | S5: in Phase Zero an enabled review denies without any name lookup; the test updated; toggle ownership recorded as a non-claim. |
| 3-8 | note | `resume_agent` without an L6 check; raw id comparison in the stored-record check; `== 6`; a goal-assignment guard scanning two files; the directory walker's use not pinned; `review_consent_batch` resolving transcendent rows. | S5 follow-up. |
| 9, 10 | note | Disabled startup-scheduling code without an L6 filter; a pre-existing cron-interval bug in the moved loop code. | Recorded. |

### S3 repairs (`c028a625..6f54fb8a`, composed at `a629b0cf`)

Ten commits, each with a recorded negative control: total time bound on the
in-process credentialed POST (a dripping body now fails after the timeout);
guards read CRLF and LF forms identically (each guard runs on both); model
registration only at the authorized Ollama address; credential-bearing
reqwest providers follow no redirect; messaging send/poll bounded in time and
size; tests use their own download registry and the exit claim is scoped to a
normal exit; the six refused tools are refused before availability is
reported; search redirects only to https; the desktop build's TLS stack
(native-tls, OS trust store, system proxy) stated; pull progress callbacks
panic-guarded. Re-review assigned to S4.

## S6 (items K, DEP), reviewed by S3 — range `63eb0d07..61dd2321`

Verdict: **ready for composition; no blocker**. Dependency changes verified
independently: lockfile package diff exactly the precise updates plus the rmcp
prune; checksums match; unified feature sets unchanged except an unused
schemars feature; cargo-audit 19 -> 8 vulnerabilities on the same database;
npm changes development-only with sha512 integrity.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | should-fix | A sub-minute agent schedule is refused with only a stderr line while `create_agent`/`start_agent` succeed, so it silently never runs. | S5 (owns those functions): validate before saving and report the refusal. |
| 2 | should-fix | The governed plan path drops a whole cost record when the model-supplied project name exceeds the new bounds or history is full. | S6: sanitise the name to the bound and report refusals. |
| 3 | should-fix | The voice probe fix is not shown on the GPU-mismatch host; voice tests may download models. | S6: offline test environment; local evidence with and without the GPU mask; the workflow mask decision stays with the coordinator. |
| 4-10 | note | Variant guard asserts text only (the composed branch has a zero-model-call test); aws-lc-sys system library auto-detection; frontend error stderr not rate-limited; the cron refusal echoes caller text; latent sub-minute CronTrigger; benchmark NaN on a refused arena run; a FIFO at the budget file blocks. | S6 truncates the echo; the rest recorded. |

### S4 repairs (`9b19d6c5..5dc2229a`, composed at `35b3b26f`)

Six conductor benchmarks that sent a provider key as a bearer token on
curl's command line are withdrawn on the J1 pattern (18 withdrawn binaries in
total: the protocols server and alias, nexus-cli, nx, coding-agent,
social-poster-agent, the nx-* harness binaries and these six); gateway module
and router constructor pinned as withdrawn entry APIs; build scripts of the
packages with a withdrawn entry point pinned; bench targets inventoried;
recipes and CI configurations in dot directories covered; README and
benchmark reports updated (historical results kept); the recipe walk derives
skipped build output from the root `.gitignore` (resolving a composition
conflict with the P0-002C4D2 toolchain guard, which S4's earlier walk had
tripped by naming the toolchain directory). Coordinator commits `2ac27366`
(empty benchmark process-site registry) and `51240bee` (guard rows).
Composed desktop suite: 413 passed, 0 failed. Re-review assigned to S5.

### Re-review of the S3 repairs, by S4

Verdict: **B2, B3 and SF4 fixed and tested** (each fails when reverted, also
verified by the reviewer's own reverts); no weakened assertion; in-region
edits. Follow-up assigned to S3: the desktop Nexus Code diagnostics and
provider auto-detection still run `which` from PATH (the "runs no program to
find Ollama" claim was too broad); a test stand-in that does not read the
request body (Windows reset risk); no-redirect for the desktop swarm's
Anthropic client; a crate-wide no-client guard; a doc correction. Response
size caps for the remaining credentialed providers are optional (else a
non-claim).

### Composition status (2026-09-29 ~01:20Z)

Combined candidate `f31a63bb` (local): S2, S3, S4, S5 and S6 including their
review repairs; coordinator commits for registry entries and guard rows,
the consent-module bound reconciliation (`910063d0`), the L6-first check in
the scheduled goal executor (`636356fd`) and the composed sub-minute
schedule test (`55ae7959`). Desktop library suite on `f31a63bb`: 425 passed,
0 failed, 5 ignored (the five pre-existing `c4b_fixture_*` ignores). S1
(item D) is not yet composed.

Further repairs since the previous entries:
- S6 (review by S3): plan cost records never dropped silently; schedule
  refusals are fixed texts with no caller text (`register_agent`,
  `validate_cron`); voice tests fully offline. Voice evidence on the
  GPU-mismatch host: CTranslate2 reports no CUDA device and writes nothing to
  stderr with and without the GPU mask; the fast-local mask stays (CI's CUDA
  torch untested here).
- S5 (re-review by S6): an enabled Warden review denies without any Warden
  lookup (no caller-created stand-in can allow); L6 checks use a named bound
  (>= 6) and every id spelling; resume and review-each refuse L6; the
  goal-assignment guard scans every desktop source; manifest schedules the
  scheduler rejects are refused at create and start. Re-review of these by S6
  in progress.
- S2 (re-review by S4): no blocker; a decoy-key bypass of the API Client
  secret check (regression from the case-insensitive change), desktop tests
  that construct the real application state against the real home, kernel
  tests reading the OS keyring, a backup wording mismatch and a narrow
  secret-name pattern are being fixed by S2.
- S3 (re-review by S4): no blocker; desktop Nexus Code diagnostics and
  provider auto-detection start no process to find programs; stand-ins read
  requests fully; the desktop swarm's Anthropic client follows no redirect;
  a crate-wide client guard. Residual no-redirect for the remaining desktop
  nx and swarm provider clients and a hermetic setup-wizard test are being
  done by S3.

### Re-review of the S5 follow-up and composition commits, by S6

Verdict: **no blocker, no should-fix.** Every earlier finding (Warden stand-in,
resume, id spellings, `== 6`, guard scope, walker pin, review-each, the
scheduled-tick restart) is fixed and pinned so that a revert fails. The
Warden denial reads no agent, name or model and cannot be bypassed; nothing
depends on the deleted Warden consent path; the adapted P0-001 lock tests keep
their harness and watchdog byte-identical. The schedule refusal composes with
the bounded `validate_cron` (bounded texts; nothing saved, registered,
scheduled or audited). Composition commits `b09c63b4`, `55ae7959`, `08892f21`
and `636356fd` weaken nothing.

Notes: a stored agent whose schedule the bounded scheduler now refuses cannot
be started again (no route edits a stored manifest; dossier consequence);
the lock tests no longer exercise consent denial after a blocked cycle (other
coverage remains); the goal-assignment guard counts one call spelling (queued
to S5); a commit-message over-claim about reviewed actions is corrected in a
later message (the dossier uses the corrected statement: of the actions the
Phase Zero executor permits, only knowledge-graph updates reach the review).
