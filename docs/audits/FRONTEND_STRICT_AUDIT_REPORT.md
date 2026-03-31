# Strict Frontend Test Audit — Nexus OS

Generated: 2026-03-28T13:17:56.927Z

## A. Frontend Overview

- Framework: React 18 + TypeScript + Vite 5 (`app/package.json`).
- Router model: custom `page` state + `history.pushState` in `app/src/App.tsx` (no `react-router`).
- Entry points: `app/src/main.tsx` -> `app/src/App.tsx`.
- Route definitions: `type Page` + `PAGE_ROUTE_OVERRIDES` + `renderPage()` in `app/src/App.tsx`.
- Total page files in `app/src/pages`: 84.
- Total route IDs in App shell: 84.
- Total unique route paths: 84.
- Route-backed page files: 82.
- Utility-only (not route-bound) files: SetupWizard.tsx, commandCenterUi.tsx.

Route access segmentation (code-derived):
- Public routes: 84 (no route-level auth guards found in frontend router).
- Auth-gated routes: none detected in frontend router wiring.
- Admin routes: 6 (admin-console, admin-users, admin-fleet, admin-policies, admin-compliance, admin-health).
- Experimental routes (AGENT LAB section): 21 (browser, dna-lab, measurement, measurement-session, measurement-compare, measurement-batteries, capability-boundaries, model-routing, ab-validation, browser-agent, governance-oracle, token-economy, governed-control, world-sim, perception, agent-memory, external-tools, collab-protocol, software-factory, self-rewrite, consciousness).
- Hidden routes (in route map but absent from sidebar): 0.
- Utility-only pages/components: 2 (SetupWizard.tsx, commandCenterUi.tsx).

## B. Existing Test Reality

- Existing frontend tests are `node:test` files in `app/tests/*.test.js`.
- Existing tests validate file existence/string structure; they do not mount React components in DOM or browser.
- No Vitest/Jest/Playwright/Cypress/React Testing Library configuration found in `app/package.json` or app config.
- CI runs frontend typecheck/build (`.github/workflows/ci.yml`, `.gitlab-ci.yml`) but no browser/E2E frontend test stage.
- Strict claim check: frontend tests are **not zero**, but current tests are **structural-only**, not UI-behavior tests.

Minimum non-destructive test strategy used for this audit:
- Added temporary audit scripts only (`app/tests/audit_frontend_smoke.mjs`, `app/tests/generate_frontend_audit_report.mjs`).
- Bundled and SSR-rendered every `src/pages/*.tsx` to first paint with stubs (no production code changes).

## C. Build And Static Verification Results

Commands executed:

```bash
cd app && npm test
cd app && npm run lint
cd app && npm run build
cd app && node ./tests/audit_frontend_smoke.mjs
```

Outcomes:
- `npm test`: PASS (18/18).
- `npm run lint` (`tsc --noEmit`): PASS.
- `npm run build`: PASS.
- `audit_frontend_smoke.mjs`: PASS for bundling/import/SSR render of all 84 page files.
- Build warning observed: dynamic + static import overlap for `@tauri-apps/api/event.js` (Vite reporter warning), not a build blocker.

Static routing/import checks:
- Duplicate route paths: none.
- Hidden route IDs (route map minus nav): none.
- Page files not route-bound: `SetupWizard.tsx`, `commandCenterUi.tsx`.
- `SetupWizard.tsx` is mounted as conditional overlay in `App.tsx` (not dead code).
- `commandCenterUi.tsx` is a shared UI utility module imported by multiple pages (not dead code).

## D. Page-By-Page Testability Matrix

Testability classes:
- A: Renders standalone with minimal providers
- B: Renders with router/providers only
- C: Requires mocked Tauri/backend bridge
- D: Requires real desktop/Tauri runtime
- E: Requires external services/auth/env secrets
- F: Cannot be meaningfully tested without full app boot

Class distribution (route IDs): A=0, B=0, C=16, D=50, E=12, F=6.
SSR first-paint smoke renders: 84/84 route IDs map to a page file that rendered in the audit harness.

| Route ID | Route Path | File Path | Nav Section | Dependencies | Class | Render Status | Issues Found | Confidence |
|---|---|---|---|---|---|---|---|---|
| dashboard | /legacy-dashboard | app/src/pages/Dashboard.tsx | CORE | backend: getAuditLog, getLiveSystemMetricsJson, listAgents | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| chat | /chat | app/src/pages/Chat.tsx | OTHER | backend: hasDesktopRuntime, listProviderModels, desktop-guard | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. | Medium |
| agents | /agents | app/src/pages/Agents.tsx | CORE | backend: getPreinstalledAgents, hasDesktopRuntime, listProviderModels +8, direct-tauri, desktop-guard, sessionStorage | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. | Medium |
| audit | /audit | app/src/pages/Audit.tsx | MONITORING | backend: getAuditLog, getAuditChainStatus, hasDesktopRuntime +12, desktop-guard | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. Contains demo/mock/fallback paths. | Medium |
| workflows | /workflows | app/src/pages/Workflows.tsx | AUTOMATION | backend: getAgentTaskHistory, getScheduledAgents, hasDesktopRuntime +2, desktop-guard, localStorage | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| marketplace | /publish | app/src/pages/AppStore.tsx | OTHER | backend: getPreinstalledAgents, hasDesktopRuntime, marketplaceInstall +3, desktop-guard, env | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| settings | /settings | app/src/pages/Settings.tsx | CORE | backend: checkLlmStatus, getLlmRecommendations, testLlmConnection +3, desktop-guard, fetch, localStorage | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. Contains demo/mock/fallback paths. | Medium |
| command-center | /command | app/src/pages/CommandCenter.tsx | OTHER | backend: listAgents, startAgent, stopAgent +4, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| audit-timeline | /timeline | app/src/pages/AuditTimeline.tsx | MONITORING | backend: getAuditLog, getAuditChainStatus, hasDesktopRuntime, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| marketplace-browser | /marketplace-browser | app/src/pages/AppStore.tsx | OTHER | backend: getPreinstalledAgents, hasDesktopRuntime, marketplaceInstall +3, desktop-guard, env | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| developer-portal | /developer-portal | app/src/pages/DeveloperPortal.tsx | DEVELOPER | backend: hasDesktopRuntime, marketplacePublish, marketplaceMyAgents, desktop-guard | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| compliance | /compliance | app/src/pages/ComplianceDashboard.tsx | MONITORING | backend: getComplianceStatus, getComplianceAgents, getAuditLog +4, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| cluster | /cluster | app/src/pages/ClusterStatus.tsx | ENTERPRISE | backend: hasDesktopRuntime, getLiveSystemMetrics, listAgents +5, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| trust | /trust | app/src/pages/TrustDashboard.tsx | MONITORING | backend: getTrustOverview, hasDesktopRuntime, reputationRegister +6, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. Contains demo/mock/fallback paths. | Medium |
| distributed-audit | /chain | app/src/pages/DistributedAudit.tsx | ENTERPRISE | backend: getAuditLog, getAuditChainStatus, hasDesktopRuntime, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| permissions | /permissions | app/src/pages/PermissionDashboard.tsx | MONITORING | backend: bulkUpdatePermissions, getAgentPermissions, getCapabilityRequest +4, desktop-guard | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. | Medium |
| protocols | /protocols | app/src/pages/Protocols.tsx | DEVELOPER | backend: getAgentCards, getMcpTools, getProtocolsRequests +17, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| identity | /identity | app/src/pages/Identity.tsx | OTHER | backend: identityGetAgentPassport, identityExportPassport, identityGenerateProof +8, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| firewall | /firewall | app/src/pages/Firewall.tsx | MONITORING | backend: getFirewallStatus, getFirewallPatterns, hasDesktopRuntime, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| browser | /browser | app/src/pages/AgentBrowser.tsx | AGENT LAB | backend: hasDesktopRuntime, navigateTo, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| computer-control | /computer-control | app/src/pages/ComputerControl.tsx | SIMULATION | backend: captureScreen, computerControlGetHistory, computerControlStatus +10, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| code-editor | /code | app/src/pages/CodeEditor.tsx | DEVELOPER | backend: getGitRepoStatus, hasDesktopRuntime, sendChat +3, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| terminal | /terminal | app/src/pages/Terminal.tsx | CORE | backend: terminalExecute, terminalExecuteApproved, TerminalCommandResult | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| file-manager | /files | app/src/pages/FileManager.tsx | CORE | backend: fileManagerList, fileManagerRead, fileManagerWrite +4, env | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| system-monitor | /monitor | app/src/pages/SystemMonitor.tsx | MONITORING | backend: getLiveSystemMetrics, listAgents | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| notes | /notes | app/src/pages/NotesApp.tsx | CREATIVE | backend: notesGet, notesList, notesSave +1 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| project-manager | /projects | app/src/pages/ProjectManager.tsx | LEARN & DISCOVER | backend: hasDesktopRuntime, projectList, projectSave +1, desktop-guard, env | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| database | /database | app/src/pages/DatabaseManager.tsx | DEVELOPER | backend: dbConnect, dbListTables, dbExecuteQuery +2, env | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| api-client | /api-client | app/src/pages/ApiClient.tsx | DEVELOPER | backend: apiClientRequest, apiClientListCollections, apiClientSaveCollections +1, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| design-studio | /design | app/src/pages/DesignStudio.tsx | CREATIVE | backend: executeAgentGoal, fileManagerCreateDir, fileManagerList +3 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| email-client | /email | app/src/pages/EmailClient.tsx | COMMUNICATION | backend: hasDesktopRuntime, emailList, emailSave +6, desktop-guard, env | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. Contains demo/mock/fallback paths. | Medium |
| messaging | /messaging | app/src/pages/Messaging.tsx | COMMUNICATION | backend: getConfig, getMessagingStatus, listAgents +5, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| media-studio | /media | app/src/pages/MediaStudio.tsx | CREATIVE | backend: analyzeMediaFile, executeAgentGoal, fileManagerCreateDir +2, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| app-store | /agent-store | app/src/pages/AppStore.tsx | LEARN & DISCOVER | backend: getPreinstalledAgents, hasDesktopRuntime, marketplaceInstall +3, desktop-guard, env | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| ai-chat-hub | /ai-chat | app/src/pages/AiChatHub.tsx | CORE | backend: sendChat, chatWithOllama, conductBuild +19, direct-tauri, desktop-guard, localStorage, sessionStorage | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. Contains demo/mock/fallback paths. | Medium |
| deploy-pipeline | /deploy | app/src/pages/DeployPipeline.tsx | DEVELOPER | backend: factoryCreateProject, factoryBuildProject, factoryTestProject +8, desktop-guard | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| learning-center | /learn | app/src/pages/LearningCenter.tsx | LEARN & DISCOVER | backend: getUserProfile, getLearningPaths, startTeachMode +13, env, localStorage | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| policy-management | /policies | app/src/pages/PolicyManagement.tsx | ENTERPRISE | backend: policyList, policyValidate, policyTest +1 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| documents | /documents | app/src/pages/Documents.tsx | CORE | backend: indexDocument, chatWithDocuments, listIndexedDocuments +4 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| model-hub | /models | app/src/pages/ModelHub.tsx | CORE | backend: searchModels, getModelInfo, checkModelCompatibility +12, direct-tauri, env | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| time-machine | /time-machine | app/src/pages/TimeMachine.tsx | AUTOMATION | backend: timeMachineListCheckpoints, timeMachineGetCheckpoint, timeMachineCreateCheckpoint +12 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| voice-assistant | /voice | app/src/pages/VoiceAssistant.tsx | COMMUNICATION | backend: hasDesktopRuntime, sendChat, voiceGetStatus +4, desktop-guard | C | SSR first paint OK (audit harness) | Desktop bridge can be mocked, but backend behavior is unverified in this environment. | Medium |
| approvals | /approvals | app/src/pages/ApprovalCenter.tsx | CORE | backend: approveConsentRequest, batchApproveConsents, batchDenyConsents +6, direct-tauri, desktop-guard | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| simulation | /world-simulation | app/src/pages/WorldSimulation.tsx | SIMULATION | backend: chatWithSimulationPersona, createSimulation, getSimulationReport +7, direct-tauri, desktop-guard | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| mission-control | /dashboard | app/src/pages/MissionControl.tsx | OTHER | backend: trayStatus, listAgents, getImmuneStatus +7 | F | SSR first paint OK (audit harness) | Depends on shell-level props/state for meaningful behavior. | Medium |
| dna-lab | /dna-lab | app/src/pages/AgentDnaLab.tsx | AGENT LAB | backend: breedAgents, getAgentGenome, getAgentLineage +15, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| timeline-viewer | /timeline-viewer | app/src/pages/TimelineViewer.tsx | AUTOMATION | backend: getTemporalHistory, temporalSelectFork | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| knowledge-graph | /knowledge | app/src/pages/KnowledgeGraph.tsx | LEARN & DISCOVER | backend: cogfsGetContext, cogfsGetEntities, cogfsGetGraph +10, direct-tauri, localStorage | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| immune-dashboard | /immune | app/src/pages/ImmuneDashboard.tsx | OTHER | backend: getImmuneStatus, getImmuneMemory, setPrivacyRules +4 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| consciousness | /consciousness | app/src/pages/ConsciousnessMonitor.tsx | AGENT LAB | backend: getAgentConsciousness, getConsciousnessHeatmap, getConsciousnessHistory +3, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| dreams | /dreams | app/src/pages/DreamForge.tsx | CREATIVE | backend: getDreamStatus, getDreamQueue, getDreamHistory +4 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| temporal | /temporal | app/src/pages/TemporalEngine.tsx | AUTOMATION | backend: runDilatedSession, temporalFork, temporalRollback, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| civilization | /civilization | app/src/pages/Civilization.tsx | SIMULATION | backend: civGetEconomyStatus, civGetGovernanceLog, civGetParliamentStatus +25, direct-tauri, desktop-guard | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| self-rewrite | /self-rewrite | app/src/pages/SelfRewriteLab.tsx | AGENT LAB | backend: selfRewriteAnalyze, selfRewriteApplyPatch, selfRewriteGetHistory +4, direct-tauri | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| admin-console | /admin-console | app/src/pages/AdminDashboard.tsx | ENTERPRISE | backend: adminOverview | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| admin-users | /admin-users | app/src/pages/AdminUsers.tsx | ENTERPRISE | backend: adminUsersList, adminUserCreate, adminUserUpdateRole +1 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| admin-fleet | /admin-fleet | app/src/pages/AdminFleet.tsx | ENTERPRISE | backend: adminFleetStatus, adminAgentStopAll, adminAgentBulkUpdate | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| admin-policies | /admin-policies | app/src/pages/AdminPolicyEditor.tsx | ENTERPRISE | backend: adminPolicyGet, adminPolicyUpdate, adminPolicyHistory | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. Contains demo/mock/fallback paths. | Medium |
| admin-compliance | /admin-compliance | app/src/pages/AdminCompliance.tsx | ENTERPRISE | backend: adminComplianceStatus, adminComplianceExport | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| admin-health | /admin-health | app/src/pages/AdminSystemHealth.tsx | ENTERPRISE | backend: adminSystemHealth, backupCreate, backupList +3 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| integrations | /integrations | app/src/pages/Integrations.tsx | COMMUNICATION | backend: integrationsList, integrationTest, integrationConfigure +1 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| login | /login | app/src/pages/Login.tsx | ENTERPRISE | backend: authLogin, authSessionInfo, authLogout +2 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. Contains demo/mock/fallback paths. | Medium |
| workspaces | /workspaces | app/src/pages/Workspaces.tsx | ENTERPRISE | backend: workspaceList, workspaceCreate, workspaceGet +4 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| telemetry | /telemetry | app/src/pages/Telemetry.tsx | ENTERPRISE | backend: telemetryStatus, telemetryHealth, telemetryConfigGet +1 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| usage-billing | /usage-billing | app/src/pages/UsageBilling.tsx | ENTERPRISE | backend: meteringUsageReport, meteringCostBreakdown, meteringExportCsv +2 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| scheduler | /scheduler | app/src/pages/Scheduler.tsx | CORE | backend: schedulerCreate, schedulerDelete, schedulerDisable +3 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| flash-inference | /flash-inference | app/src/pages/FlashInference.tsx | CORE | backend: flashListLocalModels, flashDetectHardware, flashCreateSession +9, direct-tauri, localStorage | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| measurement | /measurement | app/src/pages/MeasurementDashboard.tsx | AGENT LAB | backend: cmListSessions, cmGetBatteries, cmGetScorecard +2 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| measurement-session | /measurement-session | app/src/pages/MeasurementSession.tsx | AGENT LAB | backend: cmGetSession, cmGetGamingFlags, cmListSessions | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| measurement-compare | /measurement-compare | app/src/pages/MeasurementCompare.tsx | AGENT LAB | backend: cmCompareAgents, cmListSessions | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| measurement-batteries | /measurement-batteries | app/src/pages/MeasurementBatteries.tsx | AGENT LAB | backend: cmGetBatteries | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| capability-boundaries | /capability-boundaries | app/src/pages/CapabilityBoundaryMap.tsx | AGENT LAB | backend: cmGetBoundaryMap, cmGetCalibration, cmGetCensus +2 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| model-routing | /model-routing | app/src/pages/ModelRouting.tsx | AGENT LAB | backend: routerGetAccuracy, routerGetModels, routerGetFeedback +1 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| ab-validation | /ab-validation | app/src/pages/ABValidation.tsx | AGENT LAB | backend: cmRunAbValidation, listAgents | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| browser-agent | /browser-agent | app/src/pages/BrowserAgent.tsx | AGENT LAB | backend: browserCreateSession, browserExecuteTask, browserNavigate +6 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| governance-oracle | /governance-oracle | app/src/pages/GovernanceOracle.tsx | AGENT LAB | backend: oracleStatus, oracleGetAgentBudget, listAgents | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| token-economy | /token-economy | app/src/pages/TokenEconomy.tsx | AGENT LAB | backend: tokenGetAllWallets, tokenGetLedger, tokenGetSupply +3 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| governed-control | /governed-control | app/src/pages/GovernedControl.tsx | AGENT LAB | backend: ccGetActionHistory, ccGetCapabilityBudget, ccGetScreenContext +3 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| world-sim | /world-sim | app/src/pages/WorldSimulation2.tsx | AGENT LAB | backend: simGetHistory, simGetPolicy, simSubmit +4 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| perception | /perception | app/src/pages/Perception.tsx | AGENT LAB | backend: perceptionAnalyzeChart, perceptionDescribe, perceptionExtractData +6 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| agent-memory | /agent-memory | app/src/pages/AgentMemory.tsx | AGENT LAB | backend: listAgents, memoryBuildContext, memoryConsolidate +7 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| external-tools | /external-tools | app/src/pages/ExternalTools.tsx | AGENT LAB | backend: toolsExecute, toolsGetAudit, toolsGetPolicy +4 | E | SSR first paint OK (audit harness) | Needs auth/external services and/or secrets; isolated smoke cannot verify real flow. | Medium |
| collab-protocol | /collab-protocol | app/src/pages/Collaboration.tsx | AGENT LAB | backend: collabAddParticipant, collabCastVote, collabCreateSession +9 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |
| software-factory | /software-factory | app/src/pages/SoftwareFactory.tsx | AGENT LAB | backend: swfAssignMember, swfCreateProject, swfEstimateCost +6 | D | SSR first paint OK (audit harness) | Requires real desktop/Tauri runtime for meaningful integration checks. | Medium |

## E. Confirmed Working Frontend Surfaces

Evidence-backed only:
- Frontend compiles and builds successfully (`npm run lint`, `npm run build`).
- Existing structural tests pass (`npm test`, 18/18).
- All 84 `src/pages/*.tsx` files bundled, imported, and SSR-rendered to first paint in the audit harness.
- App shell route map resolves 84 route IDs with no duplicate paths.

Important constraint:
- This confirms compile/import/initial-render viability, **not** full interactive correctness against real backend runtime.

## F. Confirmed Broken Frontend Surfaces

- No compile-time or first-render route crash was confirmed in this environment.
- No broken lazy import or dead route mapping was confirmed.
- `commandCenterUi.tsx` appears under `src/pages` but is a utility module, not a route page; this is structure debt, not a runtime break.

## G. Mock/Demo/Fallback-Only Surfaces

Confirmed fallback/mock patterns (examples with code evidence):
- Global app mock mode and demo warning behavior (`app/src/App.tsx:504-519`, `app/src/App.tsx:800-805`, `app/src/App.tsx:1772-1775`).
- Chat fallback response when desktop runtime is missing (`app/src/App.tsx:1227-1233`).
- Setup wizard mock fallback for model download state (`app/src/pages/SetupWizard.tsx:224-228`) and mock hardware/ollama handlers in shell (`app/src/App.tsx:1893-1926`).
- Computer control preview/demo flow with scripted `DEMO_ACTIONS` (`app/src/pages/ComputerControl.tsx:202-211`, `app/src/pages/ComputerControl.tsx:286-305`).
- Audit chain verification client fallback in mock mode (`app/src/pages/Audit.tsx:849-867`).
- Learning center offline localStorage fallback (`app/src/pages/LearningCenter.tsx:146-151`, `app/src/pages/LearningCenter.tsx:620-635`).
- Login page explicit fallback session/config objects (`app/src/pages/Login.tsx:42-60`, `app/src/pages/Login.tsx:124-129`).
- Admin policy editor fail-open fallback (retains template state on backend failure) (`app/src/pages/AdminPolicyEditor.tsx:85-87`).

## H. Not Verifiable From Current Environment

The following are not fully verifiable with isolated frontend testing in this environment:
- Real Tauri IPC/invoke behavior and desktop plugin side effects.
- Filesystem-backed flows (e.g., code/file editors, local model assets) under true desktop runtime.
- OAuth/provider integrations and authenticated external APIs (OpenAI/Anthropic/email/integrations).
- Live LLM/provider behavior, token metering, streaming reliability, and runtime event bridge correctness.
- End-to-end governance flows requiring backend state transitions and real-time events.

Conservative classification counts indicating runtime dependence:
- C (mockable bridge needed): 16
- D (real desktop/Tauri runtime likely required): 50
- E (external auth/services/secrets): 12
- F (full app boot dependent): 6

## I. Highest-Risk Frontend Gaps

Critical:
- No true frontend behavioral test stack (no DOM component tests, no browser/E2E tests), despite 84 route IDs.

High:
- Large runtime-coupled surface to Tauri/backend APIs; isolated frontend can pass while runtime integrations still fail.
- Frequent catch-and-continue patterns can mask backend failures and present partially-functional UI states.

Medium:
- Multiple fallback/demo/offline paths may be mistaken for working live functionality.
- `src/pages` contains non-route utility module (`commandCenterUi.tsx`), increasing route/page counting ambiguity.

Low:
- Build warning on mixed static/dynamic import of `@tauri-apps/api/event.js` should be tracked for chunking clarity.

## J. Recommended Next Steps

1. Add a real frontend test harness (Vitest + React Testing Library + jsdom) for page mount smoke and critical interactions.
2. Add route-level smoke tests that mount `App` with each route path and assert first-paint plus error-boundary behavior.
3. Introduce a formal Tauri bridge mock layer for component tests (`@tauri-apps/api/core` invoke + event listeners).
4. Prioritize coverage for classes D/E/F pages first (runtime/external/full-boot dependent).
5. Add Playwright E2E in desktop-integrated CI (or nightly) for real backend + IPC validation.
6. Enforce CI gates for frontend tests (not just typecheck/build) and route coverage drift checks.
7. Separate route pages vs utility modules (move `commandCenterUi.tsx` out of `src/pages`) to reduce audit ambiguity.

---

### Audit Artifacts

- `app/tests/audit_frontend_smoke.mjs` (temporary audit-only smoke harness)
- `app/tests/audit_frontend_smoke_results.json` (machine-readable findings)
- `app/tests/generate_frontend_audit_report.mjs` (temporary audit-only report generator)