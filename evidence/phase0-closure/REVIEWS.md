# Internal adversarial reviews

Each component is reviewed by another workstream agent, read-only toward the
component worktree. A review is an internal check, never approval. Findings
are dispositioned by the coordinator; blockers go back to the component
owner for repair and are re-verified.

## S5 (items G, C5), reviewed by S4 — range `63eb0d07..3f712633`

Verdict: **NOT READY**.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| 1 | blocker | Startup `load_prebuilt_agents` registers the 12 prebuilt autonomy-6 manifests (fresh install, or after a row is deleted); `execute_agent_goal` then runs them and `tool_call_autonomy` grants 6. Confirmed in a scratch probe. The "L6 unavailable" claim does not hold. | Repair assigned to S5: no autonomy-6 manifest registered at startup, consistent across restarts; refuse registered autonomy 6 in goal assignment, the autonomous loop and tool-call autonomy; behavioural test of the real startup order; claims corrected. |
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
