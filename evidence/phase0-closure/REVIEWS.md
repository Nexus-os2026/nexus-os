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
