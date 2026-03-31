# Frontend Strict Test Audit (Evidence-Only)

Generated: 2026-03-30T14:07:44.815Z
Audit scope: frontend only (`app/`), no production code modifications.

## A. Frontend overview
- Framework: React 18 + TypeScript + Vite 5 (`app/package.json`).
- Router model: custom history/page-state router in `app/src/App.tsx` (no React Router).
- Entrypoints: `app/src/main.tsx` -> `app/src/App.tsx`.
- Route/page totals (freshly recomputed):
  - Route IDs in App shell: **85**
  - Unique route paths: **85**
  - Page modules in `app/src/pages`: **85**
  - Route-backed page files: **83**
  - Utility-only files in `src/pages`: **SetupWizard.tsx, commandCenterUi.tsx**
- Route segmentation from code:
  - Public routes: **85** (no route-level auth guard found)
  - Auth-gated routes: **0 confirmed**
  - Admin routes: **6** (admin-console, admin-users, admin-fleet, admin-policies, admin-compliance, admin-health)
  - Experimental (AGENT LAB) routes: **22**
  - Hidden routes (in route map but not nav): **0**

## B. Existing test reality
- The claim "0 frontend tests" is **false** as of this audit.
- Confirmed frontend test files: 9
  - app/tests/smoke.test.js
  - app/tests/pages-smoke.test.js
  - app/tests/page-modules.test.js
  - app/src/__tests__/pages.test.ts
  - app/src/pages/__tests__/Dashboard.test.tsx
  - app/src/pages/__tests__/Firewall.test.tsx
  - app/src/pages/__tests__/Settings.test.tsx
  - app/src/pages/__tests__/SetupWizard.test.tsx
  - app/src/pages/__tests__/Workflows.test.tsx
- Confirmed tooling in repo:
  - Vitest (`app/vitest.config.ts`)
  - React Testing Library + jest-dom (`app/package.json`)
  - Node built-in test runner (`node --test` scripts in `app/tests/*.test.js`)
- What is still missing:
  - No Playwright/Cypress E2E suite in `app/`
  - No coverage threshold/config for frontend tests
  - GitHub Actions frontend job does not run `npm test` (only typecheck/build)
- Easiest non-destructive test strategy today (confirmed workable):
  1. `npm run test` for current structural + Vitest suite
  2. `node tests/audit_frontend_smoke.mjs` for page-module SSR smoke
  3. Route-shell smoke (`tests/audit_app_route_smoke_results.json` generated in this audit)

## C. Build and static verification results
Commands executed (from `app/`):

```bash
npm run test
npm run lint
npm run build
node tests/audit_frontend_smoke.mjs
# additional strict route-shell smoke (audit-only):
node <inline script> -> tests/audit_app_route_smoke_results.json
```

Observed outcomes:
- `npm run test`: **PASS**
  - Node tests: 21/21 pass
  - Vitest: 91/91 pass
- `npm run lint` (`tsc --noEmit`): **FAIL**
- `npm run build` (`npx tsc && npx vite build`): **FAIL**
- `node tests/audit_frontend_smoke.mjs`: **PASS**
  - 85/85 page modules bundled/imported/SSR-rendered
- App route-shell first-paint smoke: **PASS**
  - 85/85 routes rendered, 0 failures

Exact failing TypeScript errors (build/lint blocker):
```text
src/pages/__tests__/SetupWizard.test.tsx(17,9): error TS2322 ... Property 'onRunSetup' does not exist on type 'SetupWizardProps'.
src/pages/__tests__/SetupWizard.test.tsx(33,9): error TS2322 ... Property 'onRunSetup' does not exist on type 'SetupWizardProps'.
src/test/setup.ts(4,1): error TS2304: Cannot find name 'global'.
```

Static checks (Phase 1):
- Duplicate route paths: **none**
- Broken lazy imports in App: **none** (77/77 target files exist)
- Route IDs without nav item: **none**
- Page files unreachable by router: **2** (`SetupWizard.tsx` modal-only, `commandCenterUi.tsx` utility-only)
- TODO/FIXME markers in page code: **none found**
- Unsafe any patterns in pages: **128 matches** (`rg -n "\bany\b|as any" app/src/pages`)

## D. Page-by-page testability matrix
Testability class definitions:
- A: Renders standalone with minimal providers
- B: Renders with router/providers only
- C: Requires mocked Tauri/backend bridge
- D: Requires real desktop/Tauri runtime
- E: Requires external services/auth/env secrets
- F: Cannot be meaningfully tested without full app boot

Class distribution across 85 routes:
- A: 0
- B: 0
- C: 50
- D: 12
- E: 18
- F: 5

| Route ID | Route Path | File Path | Section | Dependencies | Class | App Route Smoke | Page Module Smoke | Notes |
|---|---|---|---|---|---|---|---|---|
| dashboard | /legacy-dashboard | app/src/pages/Dashboard.tsx | CORE | backend | C | pass | pass | backend call without explicit runtime guard |
| chat | /chat | app/src/pages/Chat.tsx | OTHER | backend,desktop-guard | F | pass | pass | shell-prop coupled |
| agents | /agents | app/src/pages/Agents.tsx | CORE | backend,direct-tauri,desktop-guard,env | F | pass | pass | shell-prop coupled; imports @tauri-apps/api directly |
| audit | /audit | app/src/pages/Audit.tsx | MONITORING | backend,desktop-guard,env | F | pass | pass | shell-prop coupled; contains demo/mock/fallback logic |
| workflows | /workflows | app/src/pages/Workflows.tsx | AUTOMATION | backend,desktop-guard | C | pass | pass |  |
| marketplace | /publish | app/src/pages/AppStore.tsx | OTHER | backend,desktop-guard,env | E | pass | pass | three routes share this file |
| settings | /settings | app/src/pages/Settings.tsx | CORE | backend,desktop-guard,fetch,env | F | pass | pass | shell-prop coupled; contains demo/mock/fallback logic |
| command-center | /command | app/src/pages/CommandCenter.tsx | OTHER | backend,desktop-guard | C | pass | pass |  |
| audit-timeline | /timeline | app/src/pages/AuditTimeline.tsx | MONITORING | backend,desktop-guard,env | C | pass | pass |  |
| marketplace-browser | /marketplace-browser | app/src/pages/AppStore.tsx | OTHER | backend,desktop-guard,env | E | pass | pass | three routes share this file |
| developer-portal | /developer-portal | app/src/pages/DeveloperPortal.tsx | DEVELOPER | backend,desktop-guard | E | pass | pass |  |
| compliance | /compliance | app/src/pages/ComplianceDashboard.tsx | MONITORING | backend,desktop-guard | C | pass | pass |  |
| cluster | /cluster | app/src/pages/ClusterStatus.tsx | ENTERPRISE | backend,desktop-guard | C | pass | pass |  |
| trust | /trust | app/src/pages/TrustDashboard.tsx | MONITORING | backend,desktop-guard | C | pass | pass | contains demo/mock/fallback logic |
| distributed-audit | /chain | app/src/pages/DistributedAudit.tsx | ENTERPRISE | backend,desktop-guard | C | pass | pass |  |
| permissions | /permissions | app/src/pages/PermissionDashboard.tsx | MONITORING | backend,desktop-guard | F | pass | pass | shell-prop coupled |
| protocols | /protocols | app/src/pages/Protocols.tsx | DEVELOPER | backend,desktop-guard | C | pass | pass |  |
| identity | /identity | app/src/pages/Identity.tsx | OTHER | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| firewall | /firewall | app/src/pages/Firewall.tsx | MONITORING | backend,desktop-guard,env | C | pass | pass |  |
| browser | /browser | app/src/pages/AgentBrowser.tsx | AGENT LAB | backend,desktop-guard | C | pass | pass |  |
| computer-control | /computer-control | app/src/pages/ComputerControl.tsx | SIMULATION | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly; contains demo/mock/fallback logic |
| code-editor | /code | app/src/pages/CodeEditor.tsx | DEVELOPER | backend,desktop-guard | C | pass | pass |  |
| terminal | /terminal | app/src/pages/Terminal.tsx | CORE | backend | C | pass | pass | backend call without explicit runtime guard |
| file-manager | /files | app/src/pages/FileManager.tsx | CORE | backend,env | C | pass | pass | backend call without explicit runtime guard |
| system-monitor | /monitor | app/src/pages/SystemMonitor.tsx | MONITORING | backend | C | pass | pass | backend call without explicit runtime guard |
| notes | /notes | app/src/pages/NotesApp.tsx | CREATIVE | backend | C | pass | pass | backend call without explicit runtime guard |
| project-manager | /projects | app/src/pages/ProjectManager.tsx | LEARN & DISCOVER | backend,desktop-guard,env | C | pass | pass |  |
| database | /database | app/src/pages/DatabaseManager.tsx | DEVELOPER | backend,env | C | pass | pass | backend call without explicit runtime guard; contains demo/mock/fallback logic |
| api-client | /api-client | app/src/pages/ApiClient.tsx | DEVELOPER | backend,desktop-guard,env | E | pass | pass |  |
| design-studio | /design | app/src/pages/DesignStudio.tsx | CREATIVE | backend | C | pass | pass | backend call without explicit runtime guard |
| email-client | /email | app/src/pages/EmailClient.tsx | COMMUNICATION | backend,desktop-guard,env | E | pass | pass | contains demo/mock/fallback logic |
| messaging | /messaging | app/src/pages/Messaging.tsx | COMMUNICATION | backend,direct-tauri | E | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| media-studio | /media | app/src/pages/MediaStudio.tsx | CREATIVE | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| app-store | /agent-store | app/src/pages/AppStore.tsx | LEARN & DISCOVER | backend,desktop-guard,env | E | pass | pass | three routes share this file |
| ai-chat-hub | /ai-chat | app/src/pages/AiChatHub.tsx | CORE | backend,direct-tauri,desktop-guard,env | E | pass | pass | imports @tauri-apps/api directly; contains demo/mock/fallback logic |
| deploy-pipeline | /deploy | app/src/pages/DeployPipeline.tsx | DEVELOPER | backend,desktop-guard,env | E | pass | pass |  |
| learning-center | /learn | app/src/pages/LearningCenter.tsx | LEARN & DISCOVER | backend,env | C | pass | pass | backend call without explicit runtime guard; contains demo/mock/fallback logic |
| policy-management | /policies | app/src/pages/PolicyManagement.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| documents | /documents | app/src/pages/Documents.tsx | CORE | backend | C | pass | pass | backend call without explicit runtime guard |
| model-hub | /models | app/src/pages/ModelHub.tsx | CORE | backend,direct-tauri,env | E | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| time-machine | /time-machine | app/src/pages/TimeMachine.tsx | AUTOMATION | backend | C | pass | pass | backend call without explicit runtime guard |
| voice-assistant | /voice | app/src/pages/VoiceAssistant.tsx | COMMUNICATION | backend,desktop-guard,env | E | pass | pass |  |
| approvals | /approvals | app/src/pages/ApprovalCenter.tsx | CORE | backend,direct-tauri,desktop-guard,env | D | pass | pass | imports @tauri-apps/api directly; contains demo/mock/fallback logic |
| simulation | /world-simulation | app/src/pages/WorldSimulation.tsx | SIMULATION | backend,direct-tauri,desktop-guard | D | pass | pass | imports @tauri-apps/api directly |
| mission-control | /dashboard | app/src/pages/MissionControl.tsx | OTHER | backend | C | pass | pass | backend call without explicit runtime guard |
| dna-lab | /dna-lab | app/src/pages/AgentDnaLab.tsx | AGENT LAB | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| timeline-viewer | /timeline-viewer | app/src/pages/TimelineViewer.tsx | AUTOMATION | backend | C | pass | pass | backend call without explicit runtime guard |
| knowledge-graph | /knowledge | app/src/pages/KnowledgeGraph.tsx | LEARN & DISCOVER | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly; contains demo/mock/fallback logic |
| immune-dashboard | /immune | app/src/pages/ImmuneDashboard.tsx | OTHER | backend | C | pass | pass | backend call without explicit runtime guard; contains demo/mock/fallback logic |
| consciousness | /consciousness | app/src/pages/ConsciousnessMonitor.tsx | AGENT LAB | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| dreams | /dreams | app/src/pages/DreamForge.tsx | CREATIVE | backend | C | pass | pass | backend call without explicit runtime guard |
| temporal | /temporal | app/src/pages/TemporalEngine.tsx | AUTOMATION | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| civilization | /civilization | app/src/pages/Civilization.tsx | SIMULATION | backend,direct-tauri,desktop-guard | D | pass | pass | imports @tauri-apps/api directly |
| self-rewrite | /self-rewrite | app/src/pages/SelfRewriteLab.tsx | AGENT LAB | backend,direct-tauri | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| admin-console | /admin-console | app/src/pages/AdminDashboard.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| admin-users | /admin-users | app/src/pages/AdminUsers.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| admin-fleet | /admin-fleet | app/src/pages/AdminFleet.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| admin-policies | /admin-policies | app/src/pages/AdminPolicyEditor.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard; contains demo/mock/fallback logic |
| admin-compliance | /admin-compliance | app/src/pages/AdminCompliance.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| admin-health | /admin-health | app/src/pages/AdminSystemHealth.tsx | ENTERPRISE | backend | C | pass | pass | backend call without explicit runtime guard |
| integrations | /integrations | app/src/pages/Integrations.tsx | COMMUNICATION | backend | E | pass | pass | backend call without explicit runtime guard |
| login | /login | app/src/pages/Login.tsx | ENTERPRISE | backend | E | pass | pass | backend call without explicit runtime guard; contains demo/mock/fallback logic |
| workspaces | /workspaces | app/src/pages/Workspaces.tsx | ENTERPRISE | backend | E | pass | pass | backend call without explicit runtime guard |
| telemetry | /telemetry | app/src/pages/Telemetry.tsx | ENTERPRISE | backend | E | pass | pass | backend call without explicit runtime guard |
| usage-billing | /usage-billing | app/src/pages/UsageBilling.tsx | ENTERPRISE | backend | E | pass | pass | backend call without explicit runtime guard |
| scheduler | /scheduler | app/src/pages/Scheduler.tsx | CORE | backend | C | pass | pass | backend call without explicit runtime guard |
| flash-inference | /flash-inference | app/src/pages/FlashInference.tsx | CORE | backend,direct-tauri,env | D | pass | pass | backend call without explicit runtime guard; imports @tauri-apps/api directly |
| measurement | /measurement | app/src/pages/MeasurementDashboard.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| measurement-session | /measurement-session | app/src/pages/MeasurementSession.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| measurement-compare | /measurement-compare | app/src/pages/MeasurementCompare.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| measurement-batteries | /measurement-batteries | app/src/pages/MeasurementBatteries.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| capability-boundaries | /capability-boundaries | app/src/pages/CapabilityBoundaryMap.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| model-routing | /model-routing | app/src/pages/ModelRouting.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| ab-validation | /ab-validation | app/src/pages/ABValidation.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| browser-agent | /browser-agent | app/src/pages/BrowserAgent.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| governance-oracle | /governance-oracle | app/src/pages/GovernanceOracle.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| token-economy | /token-economy | app/src/pages/TokenEconomy.tsx | AGENT LAB | backend,env | C | pass | pass | backend call without explicit runtime guard |
| governed-control | /governed-control | app/src/pages/GovernedControl.tsx | AGENT LAB | backend,env | C | pass | pass | backend call without explicit runtime guard |
| world-sim | /world-sim | app/src/pages/WorldSimulation2.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| perception | /perception | app/src/pages/Perception.tsx | AGENT LAB | backend | E | pass | pass | backend call without explicit runtime guard |
| agent-memory | /agent-memory | app/src/pages/AgentMemory.tsx | AGENT LAB | backend,env | C | pass | pass | backend call without explicit runtime guard |
| external-tools | /external-tools | app/src/pages/ExternalTools.tsx | AGENT LAB | backend,env | E | pass | pass | backend call without explicit runtime guard |
| collab-protocol | /collab-protocol | app/src/pages/Collaboration.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| software-factory | /software-factory | app/src/pages/SoftwareFactory.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |
| memory-dashboard | /memory-dashboard | app/src/pages/Memory.tsx | AGENT LAB | backend | C | pass | pass | backend call without explicit runtime guard |

## E. Confirmed working frontend surfaces
Only evidence-backed items:
- Frontend tests exist and execute (`npm run test` passes).
- All 85 page modules can be bundled/imported/SSR-rendered in isolation (`tests/audit_frontend_smoke_results.json`).
- All 85 declared routes first-paint render through App shell without synchronous crash in SSR smoke (`tests/audit_app_route_smoke_results.json`).
- Router map integrity checks pass (no duplicate route paths, no broken lazy import targets).

## F. Confirmed broken frontend surfaces
Evidence-backed failures only:
- Frontend typecheck/build currently broken by test code typing errors:
  - `app/src/pages/__tests__/SetupWizard.test.tsx` (invalid props for current `SetupWizardProps`)
  - `app/src/test/setup.ts` (`global` type unresolved)
- Result: `npm run lint` and `npm run build` both fail in current state.

## G. Mock/demo/fallback-only surfaces
Confirmed explicit fallback/demo behavior in source (not inferred):
- Global demo/mock mode and desktop-runtime warning banner in `app/src/App.tsx` (runtime mode switches, demo toast, non-desktop message paths).
- Login fallback objects used on backend failure (`app/src/pages/Login.tsx`: `FALLBACK_SESSION` / `FALLBACK_CONFIG` and catch path).
- Admin policy editor fail-open fallback (`app/src/pages/AdminPolicyEditor.tsx` keeps local template when backend fetch fails).
- ComputerControl contains explicit `DEMO_ACTIONS` simulation sequence (`app/src/pages/ComputerControl.tsx`).
- Audit chain client-side fallback verification path in non-desktop mode (`app/src/pages/Audit.tsx`).
- LearningCenter persists offline/local fallback when backend sync fails (`app/src/pages/LearningCenter.tsx`).

Important caution:
- Many pages render polished empty/placeholder states even when backend calls fail, so "renders" != "feature works".

## H. Not verifiable from current environment
Marked unverified due missing real runtime/service integration:
- Real Tauri desktop IPC behavior (`@tauri-apps/api` bridge, native plugins, event bus).
- Filesystem/system control side effects (computer control, terminal execution, file operations).
- OAuth and third-party integrations (GitHub/GitLab/Slack/Jira/email providers).
- Live LLM/provider behavior and credentials-dependent paths.
- Real-time backend events, sockets, and long-running orchestration semantics.

Route classes with runtime/external constraints:
- D routes (desktop runtime heavy): identity, computer-control, media-studio, approvals, simulation, dna-lab, knowledge-graph, consciousness, temporal, civilization, self-rewrite, flash-inference
- E routes (external/auth/env/secrets): marketplace, marketplace-browser, developer-portal, api-client, email-client, messaging, app-store, ai-chat-hub, deploy-pipeline, model-hub, voice-assistant, integrations, login, workspaces, telemetry, usage-billing, perception, external-tools
- F routes (full-shell coupled): chat, agents, audit, settings, permissions

## I. Highest-risk frontend gaps
Critical:
- Frontend build/typecheck is currently failing (`npm run lint` / `npm run build`), masking release confidence.
- 85-route UI has no true browser/E2E runtime validation with real Tauri/backend.

High:
- 83/85 page modules depend on backend wrappers; 17 import `@tauri-apps/api` directly.
- 56 page files call backend wrappers without explicit `hasDesktopRuntime()` guard at page level (rely on catches/fallbacks).
- Multiple silent catch patterns can fail-open UI states (example: `AdminPolicyEditor`, `EmailClient`, `FlashInference`).

Medium:
- 128 explicit `any` usages in page layer reduce type safety around critical data flows.
- GitHub CI frontend job does not execute frontend tests; GitLab does. This split can hide regressions per pipeline.

Low:
- `src/pages` includes non-route utility module (`commandCenterUi.tsx`), which inflates page counts and can mislead audits.
- Hardcoded localhost references exist in UI placeholders/examples (`Telemetry.tsx`, `SetupWizard.tsx`, `Protocols.tsx`).

## J. Recommended next steps
1. Fix the three TypeScript errors blocking `npm run lint` and `npm run build`.
2. Keep current structural tests, but add route-level component smoke tests under Vitest that mount App per route and assert first paint + no boundary crash.
3. Add a dedicated Tauri bridge mock layer for deterministic unit tests (`invoke` + event listeners).
4. Prioritize D/E/F routes for behavioral tests first (desktop/native/external/full-boot risk).
5. Add Playwright (or equivalent) desktop-integrated E2E for a small critical path set (login/session, chat send, agent start/stop, audit refresh).
6. Align CI: run frontend tests in GitHub Actions as well, not just typecheck/build.
7. Separate utility UI modules from `src/pages` to keep route inventory and coverage metrics accurate.

---

## Audit artifacts generated in this run
- `app/tests/audit_frontend_smoke_results.json` (fresh run: 2026-03-30T14:01:30.009Z)
- `app/tests/audit_app_route_smoke_results.json` (route-shell SSR smoke)
