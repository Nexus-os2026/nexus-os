# 2026-05-13 Full Forensic Audit: Phase 1 Wiring

## Scope

- Audit date: `2026-05-13`
- Repo state: `HEAD` in `/home/nexus/NEXUS/nexus-os`
- Frontend routes audited: `89`
- Registered Tauri commands in `generate_handler!`: `804`
- Method: static wiring audit of every routed page plus reverse-pass command reachability checks
- Runtime status: this phase is code-walk only; loading/empty/error-state presence was verified by reading components, not by launching the UI

## Severity Summary

| Severity | Count | Notes |
| --- | ---: | --- |
| `P1` | 3 | User-visible broken save/sync paths |
| `P2` | 56 | Wiring mismatches, swallowed errors, broken argument shapes, and missing frontend coverage |
| `P3` | 0 | Subagent-reported `P3` wiring issues were normalized to `P2` per the audit rubric |

## Coverage Summary

| Batch | Routes | Pass | Findings |
| --- | --- | ---: | ---: |
| `P1-B1` | `dashboard`, `ai-chat-hub`, `agents`, `file-manager`, `model-hub`, `flash-inference`, `documents`, `scheduler`, `approvals`, `terminal` | 9 | 1 |
| `P1-B2` | `settings`, `nexus-builder`, `nexus-code`, `code-editor`, `api-client`, `database`, `developer-portal`, `deploy-pipeline`, `software-factory`, `protocols` | 2 | 8 |
| `P1-B3` | `email-client`, `voice-assistant`, `messaging`, `integrations`, `system-monitor`, `audit`, `swarm-audit`, `audit-timeline`, `trust`, `firewall` | 4 | 6 |
| `P1-B4` | `compliance`, `permissions`, `browser`, `memory-dashboard`, `dna-lab`, `measurement`, `measurement-session`, `measurement-compare`, `measurement-batteries`, `capability-boundaries` | 2 | 8 |
| `P1-B5` | `model-routing`, `ab-validation`, `browser-agent`, `governance-oracle`, `token-economy`, `governed-control`, `world-sim`, `perception`, `agent-memory`, `external-tools` | 0 | 10 |
| `P1-B6` | `collab-protocol`, `self-rewrite`, `self-improvement`, `consciousness`, `design-studio`, `media-studio`, `dreams`, `notes`, `workflows`, `time-machine` | 3 | 7 |
| `P1-B7` | `timeline-viewer`, `temporal`, `simulation`, `civilization`, `computer-control`, `login`, `workspaces`, `admin-console`, `admin-users`, `admin-fleet` | 3 | 7 |
| `P1-B8` | `admin-compliance`, `admin-policies`, `admin-health`, `usage-billing`, `telemetry`, `cluster`, `distributed-audit`, `policy-management`, `learning-center`, `app-store` | 1 | 9 |
| `P1-B9` | `knowledge-graph`, `project-manager`, `chat`, `command-center`, `mission-control`, `marketplace`, `marketplace-browser`, `immune-dashboard`, `identity` | 6 | 3 |

## P1 Findings

| Page | Command | File:Line | Finding |
| --- | --- | --- | --- |
| `audit` | `complete_tracing_span` | `app/src/pages/Audit.tsx:338-345,707-763,835-890`; `app/src-tauri/src/commands/trust_security.rs:1211-1253`; `app/src-tauri/src/lib.rs:8929-8976`; `app/src/pages/__tests__/Audit.test.tsx:11-29` | The tracing tab sends lowercase span statuses (`ok`, `error`, `cancelled`), but the Rust command only accepts `Ok` or `Error`. The default `ok` path therefore returns `unknown span status`, and the test suite never exercises the broken wiring. |
| `admin-policies` | `admin_policy_update` | `app/src/pages/AdminPolicyEditor.tsx:79-105,145-203`; `app/src/api/backend.ts:2888-2898`; `app/src-tauri/src/commands/enterprise.rs:512-524`; `app/src/pages/__tests__/AdminPolicyEditor.test.tsx:16-31` | `Save Policy` is a no-op: the backend command ignores the submitted policy payload and returns `Ok(())`, so the editor cannot persist changes. |
| `learning-center` | `get_learning_session`, `learning_agent_action` | `app/src/pages/LearningCenter.tsx:547-575,620-683`; `app/src/api/backend.ts:2838-2845,3192-3207`; `app/src-tauri/src/commands/browser_research.rs:980-986`; `app/src-tauri/src/lib.rs:2788-2803,11368-11369`; `app/src/pages/__tests__/LearningCenter.test.tsx:14-34` | `getLearningPaths()` calls `get_learning_session` without the required `session_id`, and `completeLearningStep()` sends `path_id` / `step_id` to `learning_agent_action`, which expects `session_id`, `action`, `url`, and `content`. Mount-time sync and step completion are both broken. |

## P2 Findings

| Page | Command | File:Line | Finding |
| --- | --- | --- | --- |
| `terminal` | `nx_*` via `NexusCodePage` | `app/src/App.tsx:1643`; `app/src/pages/NexusCodePage.tsx`; `app/src/api/backend.ts`; `app/src-tauri/src/lib.rs` | The routed terminal page resolves to real backend commands, but no dedicated frontend test exercises the routed `NexusCodePage` wiring. |
| `settings` | provider settings load/save | `app/src/pages/Settings.tsx:281-302,1004-1013` | Provider settings load and save failures are swallowed; save only logs in development builds. |
| `nexus-builder` | builder page wiring | `app/src/pages/NexusBuilder.tsx:257-275,499-861` | Wiring reads as consistent, but no page-specific frontend test exercises the builder command surface. |
| `nexus-code` | `nxConsentRespond` | `app/src/pages/NexusCode.tsx:232-239,433-443` | Consent-response failures are swallowed, so a rejected backend update is invisible in the page UI. |
| `api-client` | collection bootstrap/save | `app/src/pages/ApiClient.tsx:117-158,161-238` | Collection bootstrap and save failures are swallowed or development-only logged, so the page falls back silently. |
| `database` | `dbDisconnect` | `app/src/pages/DatabaseManager.tsx:100-133,152-201,241-283` | Disconnect failures are only development-console logged; the page has no explicit error surface for that action. |
| `developer-portal` | `loadMyAgents()` | `app/src/pages/DeveloperPortal.tsx:57-65,135-156,285-310` | Backend failures during agent loading are silently ignored, leaving stale or empty UI without an error state. |
| `deploy-pipeline` | bootstrap state | `app/src/pages/DeployPipeline.tsx:155-194,196-351`; `app/src/pages/__tests__/DeployPipeline.test.tsx:11-31` | Bootstrap failures collapse into default/empty state with no user-visible error banner. |
| `software-factory` | mount-time loads | `app/src/pages/SoftwareFactory.tsx:90-115` | Initial backend loads fall back to empty/null on error without surfacing the failure. |
| `email-client` | list/OAuth/send/delete | `app/src/pages/EmailClient.tsx:67-75,128-137,232-327,411-420,563-570`; `app/src/pages/__tests__/EmailClient.test.tsx:18-32`; `app/src-tauri/src/lib.rs:9257-9327` | Failures are logged or silently converted into local-draft fallbacks; the page does not render a user-visible error for core mail actions. |
| `messaging` | messaging actions | `app/src/pages/Messaging.tsx:148-229,241-380`; `app/src/pages/__tests__/Messaging.test.tsx:19-34` | The component sets `msgError` but never renders it, so connect/send failures are hidden. |
| `integrations` | `handleConfigure()` | `app/src/pages/Integrations.tsx:93-143,263-273,314-340` | Configure-path backend failures are caught and ignored completely. |
| `audit-timeline` | timeline loads | `app/src/pages/AuditTimeline.tsx:54-76,92-104,106-150` | Backend failures are console-logged only; the page has no rendered error state. |
| `firewall` | firewall loads/actions | `app/src/pages/Firewall.tsx:11-17,19-63` | Backend failures are only warned in the console; no user-facing error surface exists. |
| `compliance` | compliance bootstrap | `app/src/pages/ComplianceDashboard.tsx:103,126`; `app/src/pages/__tests__/ComplianceDashboard.test.tsx:19` | `Promise.all(...catch(() => null/[]))` swallows backend failures, and `overallStatus` falls back to `compliant`, so failed calls render as healthy. |
| `permissions` | `setAgentLlmProviderApi(...)` | `app/src/pages/PermissionDashboard.tsx:497,515`; `app/src/pages/__tests__/PermissionDashboard.test.tsx:13` | LLM-provider updates are fired with `void`, so save failures vanish. |
| `memory-dashboard` | initial load/refresh | `app/src/pages/Memory.tsx:54,73,142`; `app/src/pages/__tests__/Memory.test.tsx:16` | Initial load and refresh failures are swallowed; bootstrap errors never reach the banner. |
| `dna-lab` | `evolve_population` | `app/src/pages/AgentDnaLab.tsx:270`; `app/src-tauri/src/lib.rs:3415`; `app/src/pages/__tests__/AgentDnaLab.test.tsx:15` | `handleEvolve()` sends only `{ generations: 1 }`, but the Rust command requires `agent_ids`, `task`, and `generations`. |
| `measurement-session` | session default selection | `app/src/App.tsx:1736`; `app/src/pages/MeasurementSession.tsx:115`; `crates/nexus-capability-measurement/src/tauri_commands.rs:135`; `app/src/pages/__tests__/MeasurementSession.test.tsx:12` | When no `sessionId` is provided, the page selects `sessions[sessions.length - 1]`, but the backend returns newest-first, so the UI opens the oldest session instead of the latest. |
| `measurement-compare` | session bootstrap | `app/src/pages/MeasurementCompare.tsx:56,74`; `app/src/pages/__tests__/MeasurementCompare.test.tsx:11` | Session-list load failures are only logged. |
| `measurement-batteries` | battery bootstrap | `app/src/pages/MeasurementBatteries.tsx:40,59`; `app/src/pages/__tests__/MeasurementBatteries.test.tsx:10` | Battery-list load failures are only logged. |
| `capability-boundaries` | boundary bootstrap | `app/src/pages/CapabilityBoundaryMap.tsx:57,79`; `app/src/pages/__tests__/CapabilityBoundaryMap.test.tsx:15` | Boundary-map bootstrap only logs failures instead of rendering them. |
| `model-routing` | mount and estimate loads | `app/src/pages/ModelRouting.tsx:37,48,55` | Mount-time and estimate failures are only logged; no visible error state exists. |
| `ab-validation` | run action coverage | `app/src/pages/ABValidation.tsx:42,61`; `app/src/pages/__tests__/ABValidation.test.tsx:12` | The page’s backend action lives behind the Run button, but the test suite only renders the page and never exercises the invocation. |
| `browser-agent` | policy/session/agent loads | `app/src/pages/BrowserAgent.tsx:42,48,56,65,74,81` | Load, content, and close failures are only logged; the page has no explicit error state for those actions. |
| `governance-oracle` | bootstrap and budget lookups | `app/src/pages/GovernanceOracle.tsx:36,46,53` | Bootstrap and budget failures are only logged; the UI provides no error banner. |
| `token-economy` | initial fetch / recalculation | `app/src/pages/TokenEconomy.tsx:139,155,167` | Initial fetch falls back to empty/null, and reward/burn recalculations are only warned in development, so failures are not surfaced. |
| `governed-control` | `cc_execute_action` | `app/src/pages/GovernedControl.tsx:80,217`; `crates/nexus-computer-control/src/actions.rs:22`; `app/src-tauri/src/commands/crate_bridges.rs:622` | The default action JSON uses a `type` field, but `ComputerAction` is an externally tagged enum. The starter payload cannot deserialize, so the execute path rejects immediately. |
| `world-sim` | `get_simulation_status`, `get_simulation_report` | `app/src/pages/WorldSimulation.tsx:246-263,313-343`; `app/src/pages/__tests__/WorldSimulation.test.tsx:19-29`; `app/src/api/backend.ts:1690-1701` | `refreshSimulation()` lacks `try/catch` and is launched with `void`, so simulation-status/report rejections escape instead of surfacing in UI. |
| `perception` | action coverage | `app/src/pages/Perception.tsx:97,144,191`; `app/src/pages/__tests__/Perception.test.tsx:12` | Tests only render the page and never exercise Initialize or Perceive backend actions. |
| `agent-memory` | load and action flows | `app/src/pages/AgentMemory.tsx:96,115,125,131,137,143,150` | Most actions are fire-and-forget, and initial load only warns and clears state, so failures are swallowed. |
| `external-tools` | refresh and rate-limit actions | `app/src/pages/ExternalTools.tsx:76,89,94,111,132` | Refresh falls back to empty state on error, and the rate-limit path only logs in development. |
| `collab-protocol` | protocol bootstrap | `app/src/pages/Collaboration.tsx:105-125,171-227`; `app/src/pages/__tests__/Collaboration.test.tsx:19-23`; `app/src-tauri/src/commands/crate_bridges.rs:1000-1145`; `app/src-tauri/src/lib.rs:12031-12042` | Mount and refresh flows swallow failures with `.catch(() => [])` / `.catch(() => null)` and never surface a protocol-load error. |
| `self-rewrite` | typed wrapper path | `app/src/api/backend.ts:104-109,2729-2742`; `app/src/pages/SelfRewriteLab.tsx:167-179,253-280`; `app/src-tauri/src/lib.rs:10081-10129,11765-11771` | Helper wrappers JSON-parse values that Tauri already deserializes, so the typed path is wrong and only the raw-invoke fallback survives; history load also silently empties on error. |
| `consciousness` | bootstrap and refresh flows | `app/src/pages/ConsciousnessMonitor.tsx:165-180,201-215,223-243`; `app/src/pages/__tests__/ConsciousnessMonitor.test.tsx:19-23` | Bootstrap catches and ignores failures, and `Promise.allSettled` refresh leaves partial failures invisible. |
| `dreams` | refresh, auto-start, return typing | `app/src/pages/DreamForge.tsx:104-182,186-307`; `app/src/api/backend.ts:3228-3230`; `app/src-tauri/src/lib.rs:3519-3558,11721-11726`; `app/src/pages/__tests__/DreamForge.test.tsx:25-29` | Refresh silently drops partial failures, auto-start swallows errors, and `triggerDreamNow` is typed as `void` even though Rust returns a success string. |
| `notes` | note content/save/delete | `app/src/pages/NotesApp.tsx:165-199,231-249,274-327,390-401,471-501`; `app/src/api/backend.ts:4171-4180`; `app/src-tauri/src/lib.rs:9411-9434,11646-11649`; `app/src/pages/__tests__/NotesApp.test.tsx:17-20` | `fetchNoteContent()` suppresses backend errors, and save/delete wrappers are typed `void` even though the Rust commands return success strings. |
| `workflows` | workflow backend coverage | `app/src/pages/Workflows.tsx:248-260,313-369,526-527`; `app/src/pages/__tests__/Workflows.test.tsx:1-10`; `app/src-tauri/src/lib.rs:2102,2166-2169,9459-9477,9870-9875` | The page has backend wiring for task history and hivemind actions, but no frontend test exercises the integration paths. |
| `time-machine` | checkpoint/detail fetches | `app/src/pages/TimeMachine.tsx:154-173,204-225,371-381,388-399,416-442,457-477,495-502,671-686,930-953,1295-1304,1368-1380,1500-1512`; `app/src/pages/__tests__/TimeMachine.test.tsx:19-29`; `app/src-tauri/src/lib.rs:3182-3235,3519-3558,3600-3614,8448-8485,11549-11553` | Initial checkpoint and detail fetches clear state inside catches without surfacing the error. |
| `timeline-viewer` | `temporal_select_fork` | `app/src/pages/TimelineViewer.tsx:89-93`; `app/src/api/backend.ts:3170-3171`; `app/src-tauri/src/lib.rs:3573-3578`; `app/src/pages/__tests__/TimelineViewer.test.tsx:17-27` | `handleCommit()` sends only `forkId`, but the Rust command requires both `decision_id` and `fork_id`. |
| `temporal` | `get_temporal_history`, `temporal_select_fork` | `app/src/pages/TemporalEngine.tsx:110-145`; `app/src/api/backend.ts:2303-2304`; `app/src-tauri/src/lib.rs:3573-3604`; `app/src/pages/__tests__/TemporalEngine.test.tsx:18-40` | `get_temporal_history` is called with `{ count: 10 }` even though the backend expects `limit`, and the page stores the returned JSON string as structured history; the commit path also sends camelCase ids instead of snake_case. |
| `simulation` | `get_simulation_status`, `get_simulation_report` | `app/src/pages/WorldSimulation.tsx:246-263,313-343`; `app/src/pages/__tests__/WorldSimulation.test.tsx:19-29`; `app/src/api/backend.ts:1690-1701` | This route reuses `WorldSimulation.tsx`, so the same unhandled-refresh rejection issue as `world-sim` applies here. |
| `civilization` | `civ_propose_rule`, `civ_resolve_dispute` | `app/src/pages/Civilization.tsx:433-447,493-510`; `app/src-tauri/src/commands/advanced.rs:157-164,220-228`; `app/src/pages/__tests__/Civilization.test.tsx:26-36` | Raw `invoke()` payloads use camelCase keys (`proposerId`, `ruleText`, `agentA`, `agentB`), but the Rust commands require snake_case parameters, so both actions reject before backend execution. |
| `computer-control` | `computer_control_toggle`, `stop_computer_action` | `app/src/pages/ComputerControl.tsx:315,399`; `app/src/api/backend.ts:458-489`; `app/src/pages/__tests__/ComputerControl.test.tsx:18-27` | Enable and stop actions are launched without error handling, so backend failures are swallowed. |
| `admin-users` | `admin_user_update_role`, `admin_user_deactivate` | `app/src/pages/AdminUsers.tsx:66-82,153-185`; `app/src/api/backend.ts:2865-2870`; `app/src/pages/__tests__/AdminUsers.test.tsx:16-25` | Role-change and deactivation failures are caught and ignored; only list load is covered by tests. |
| `admin-fleet` | `admin_agent_stop_all`, `admin_agent_bulk_update` | `app/src/pages/AdminFleet.tsx:99-116,169-176`; `app/src/api/backend.ts:2878-2884`; `app/src/pages/__tests__/AdminFleet.test.tsx:17-26` | Stop-all and bulk actions swallow backend failures and never update user-visible state. |
| `admin-compliance` | `admin_compliance_status`, `admin_compliance_export` | `app/src/pages/AdminCompliance.tsx:75-116`; `app/src/pages/__tests__/AdminCompliance.test.tsx:17-32`; `app/src/api/backend.ts:2902-2907` | Mount/export failures collapse into an empty dashboard; there is no rendered success or error banner. |
| `admin-health` | `admin_system_health`, `telemetry_health`, `backup_*` | `app/src/pages/AdminSystemHealth.tsx:91-159,223-313`; `app/src/pages/__tests__/AdminSystemHealth.test.tsx:21-36`; `app/src/api/backend.ts:2911-2912,3120-3136`; `app/src-tauri/src/commands/enterprise.rs:97-180,612-646` | Provider rows expect fields the backend never emits, and backup actions send the wrong argument shapes (`backup_create` booleans missing; `backup_verify` / `backup_restore` send `id` instead of `archive_path`). |
| `telemetry` | `telemetry_config_update` | `app/src/pages/Telemetry.tsx:111-169,283-317,509-517`; `app/src/pages/__tests__/Telemetry.test.tsx:18-33`; `app/src/api/backend.ts:3074-3089`; `telemetry/src/config.rs:5-31`; `app/src-tauri/src/commands/enterprise.rs:1092-1102` | The log-format select offers `json`, `text`, and `compact`, but Rust deserializes only `Json` and `Pretty`, so user-driven saves can emit invalid enum values. |
| `cluster` | cluster/mesh status | `app/src/pages/ClusterStatus.tsx:45-78,148-231`; `app/src/pages/__tests__/ClusterStatus.test.tsx:26-40`; `app/src/api/backend.ts:1079-1084,2485-2507`; `app/src-tauri/src/commands/model_hub.rs:479-600`; `app/src-tauri/src/commands/advanced.rs:294-360` | The page expects `cpu_usage_percent`, `memory_usage_percent`, and sync counters that Rust does not return, so metrics render as zero/undefined while failures are hidden. |
| `distributed-audit` | `get_audit_log`, `get_audit_chain_status` | `app/src/pages/DistributedAudit.tsx:40-57,151-156,200-223`; `app/src/pages/__tests__/DistributedAudit.test.tsx:11-26`; `app/src/api/backend.ts:128-133,743-744`; `app/src-tauri/src/lib.rs:2228-2235,9042-9046` | Backend failures are coerced to `[]` / `null`, and `chainValid` defaults to `true`, so chain-fetch failure renders as a clean empty audit. |
| `policy-management` | policy refresh | `app/src/pages/PolicyManagement.tsx:44-55,147-154`; `app/src/pages/__tests__/PolicyManagement.test.tsx:11-26`; `app/src/api/backend.ts:907-931`; `app/src-tauri/src/commands/governance.rs:236-298` | Refresh errors are swallowed into empty lists, making outage and empty policy store indistinguishable. |
| `app-store` | GitLab marketplace search | `app/src/pages/AppStore.tsx:70-95,139-150,227-230,370-380`; `app/src/pages/__tests__/AppStore.test.tsx:12-29`; `app/src/api/backend.ts:749-760,1217-1219`; `app/src-tauri/src/commands/apps.rs:1879-1894` | GitLab-search failures are swallowed to an empty list with no error banner for that path; tests only cover the initial preinstalled-agent load. |
| `command-center` | agent-grid refresh/actions | `app/src/pages/CommandCenter.tsx:26-65,79-100`; `app/src/pages/__tests__/CommandCenter.test.tsx:11-26` | `loadData()` and `handleAction()` swallow backend failures, so action failures collapse into the same loading/empty UI. |
| `mission-control` | refresh bundle | `app/src/pages/MissionControl.tsx:145-176,404-405,417-423,449-520,566-568`; `app/src/pages/__tests__/MissionControl.test.tsx:21-38` | The dashboard refresh bundle uses `Promise.allSettled` plus ignore-only fallbacks, masking backend failures into blank/loading cards instead of surfacing an error. |
| `project-manager` | `project_list`, `project_save` | `app/src/pages/ProjectManager.tsx:119-136,186-196,343-346`; `app/src/pages/__tests__/ProjectManager.test.tsx:10-27` | `loadProject()` returns `null` on any failure, and `saveProject()` only logs in development, so persistence failures disappear into defaults. |

## Pass Pages

Verified with no wiring finding in this phase:

- `dashboard`
- `ai-chat-hub`
- `agents`
- `file-manager`
- `model-hub`
- `flash-inference`
- `documents`
- `scheduler`
- `approvals`
- `code-editor`
- `protocols`
- `voice-assistant`
- `system-monitor`
- `swarm-audit`
- `trust`
- `browser`
- `measurement`
- `self-improvement`
- `design-studio`
- `media-studio`
- `login`
- `workspaces`
- `admin-console`
- `usage-billing`
- `knowledge-graph`
- `chat`
- `marketplace`
- `marketplace-browser`
- `immune-dashboard`
- `identity`

## Reverse Pass

### Verified Tauri Commands With No Routed Page or Component Caller

Method:

- Searched `app/src` for wrapper usage outside `app/src/api/backend.ts`
- Excluded test files from the reachability check
- Only commands with zero routed-page/component call sites are listed below

| Command | File:Line | Finding |
| --- | --- | --- |
| `get_agent_cognitive_status` | `app/src/api/backend.ts:1378`; `app/src-tauri/src/lib.rs:9563-9567,11675`; `app/src-tauri/src/commands/cognitive.rs:2121-2135` | Wrapper exists, but no routed page or non-test component in `app/src` calls it. |
| `get_agent_memories` | `app/src/api/backend.ts:1400`; `app/src-tauri/src/lib.rs:9580-9586,11677`; `app/src-tauri/src/commands/cognitive.rs:2154-2168` | Wrapper exists, but no routed page or non-test component in `app/src` calls it. |
| `get_hivemind_status` | `app/src/api/backend.ts:1534`; `app/src-tauri/src/lib.rs:9879-9883,11700`; `app/src-tauri/src/commands/cognitive.rs:2300-2318` | Wrapper exists, but no routed page or non-test component in `app/src` calls it. |
| `self_improve_get_report` | `app/src-tauri/src/commands/self_improvement.rs:530-537`; `app/src-tauri/src/lib.rs:10251-10255,11814` | The command is registered, but no frontend wrapper caller or direct routed-page usage was found under `app/src`. |
| `test_emit_event` | `app/src-tauri/src/lib.rs:9484-9496,11671` | Test-only command registration with no frontend caller under `app/src`. |

### UI States With No Backend

No routed page was proven to be entirely backend-free in this pass. Some routes rely on shared stores or event-driven state instead of direct page-local `invoke()` calls, so an exact “UI with no backend” orphan list would require a second reachability pass across shared stores and non-page components.

## Residual Gaps

- This phase did not launch the UI, so loading/empty/error-state behavior is code-walk verified only.
- Shared-component alias routes can inherit the same wiring defect more than once. Counts above are route-based, not deduplicated by component file.
- Reverse-pass command reachability is conservative: only commands with zero routed-page/component callers outside `backend.ts` are listed as orphaned.
