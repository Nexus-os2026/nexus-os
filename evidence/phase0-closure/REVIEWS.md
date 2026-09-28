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
