# Nexus OS — Strict Frontend Test Audit Report

**Auditor**: Claude Opus 4.6 (acting as independent QA auditor)
**Date**: 2026-03-28
**Scope**: Frontend only (`app/src/`) — static analysis, build verification, test tooling, page-by-page classification
**Methodology**: Evidence-based, skeptical. "Not verified" over optimistic guessing.
**Production code modified**: NONE

---

## A. Frontend Overview

| Property | Value |
|----------|-------|
| **Framework** | React 18.3.1 + TypeScript 5.6.3 |
| **Bundler** | Vite 5.4.8 |
| **Desktop shell** | Tauri 2.0 (IPC bridge) |
| **Styling** | 44 CSS files (4 global design system + 40 page/component-specific) |
| **Router** | Custom state-based router in `App.tsx` (not react-router) |
| **API layer** | `app/src/api/backend.ts` — 646 exported functions, all via `invokeDesktop()` → Tauri `invoke()` |
| **Total page files** | 84 `.tsx` files in `app/src/pages/` |
| **Routed pages** | 83 (all except `commandCenterUi.tsx` which is a utility module) |
| **Lazy-loaded** | 76 pages via `React.lazy()` |
| **Eagerly loaded** | 7 pages: Dashboard, Chat, Agents, Audit, Settings, SetupWizard, Workflows |
| **Total components** | 41 `.tsx` files in `app/src/components/` |
| **Entrypoint** | `app/src/main.tsx` → `App.tsx` |

### Router Architecture

The app uses a **custom state-based router** — no `react-router`, no URL-based routing. Navigation is driven by:
- `useState<Page>` in `App.tsx`
- A `PAGE_ROUTE_OVERRIDES` map for URL-based deep linking
- `pageFromLocation()` for URL → page resolution
- A giant `renderPage()` function (lines 1384–1743) with if/else chains for all 83 pages

All pages render inside a `PageErrorBoundary` + `Suspense` wrapper in the main layout.

### API Layer Architecture

Every backend call goes through `invokeDesktop<T>()` in `backend.ts`:
```typescript
async function invokeDesktop<T>(command: string, args?): Promise<T> {
  if (!hasDesktopRuntime()) {
    throw new Error("desktop runtime unavailable");
  }
  return invoke<T>(command, args);
}
```

`hasDesktopRuntime()` checks for `window.__TAURI__` or `window.__TAURI_INTERNALS__`. Without the Tauri desktop runtime, **every API call throws**. This is a hard dependency, not a graceful degradation.

---

## B. Existing Test Reality

### What EXISTS

| Item | Status | Evidence |
|------|--------|----------|
| `app/tests/smoke.test.js` | EXISTS — 1 test | Checks 6 files exist on disk |
| `app/tests/pages-smoke.test.js` | EXISTS — 17 tests | Filesystem checks: page existence, exports, structure, sizes, infra files |
| `package.json` `"test"` script | EXISTS | `node --test ./tests/*.test.js` |
| Node.js native test runner | USED | `node:test` + `node:assert/strict` |
| All 18 tests | PASSING | Verified: `18 pass, 0 fail` |

### What DOES NOT EXIST

| Item | Status |
|------|--------|
| Vitest | NOT INSTALLED (not in package.json, not in node_modules) |
| Jest | NOT INSTALLED |
| Playwright | NOT INSTALLED |
| Cypress | NOT INSTALLED |
| @testing-library/react | NOT INSTALLED |
| jsdom / happy-dom | NOT INSTALLED |
| Any React render tests | NONE EXIST |
| Any `.test.tsx` or `.spec.tsx` files | NONE EXIST |
| Any `setupTests.*` file | DOES NOT EXIST |
| Any `__tests__/` or `__mocks__/` directories | DO NOT EXIST |
| Any coverage configuration | DOES NOT EXIST |
| ESLint configuration | DOES NOT EXIST |
| `.env` files | DO NOT EXIST |

### CI Pipeline Reality

The `.gitlab-ci.yml` job `frontend-tests:` (line 80) runs:
```yaml
script:
  - cd app
  - npm ci
  - npm run build
```

**It does NOT run `npm test`.** The CI "frontend test" job is actually just a build verification. The existing 18 smoke tests are never executed in CI.

### Verdict

**Frontend tests technically exist** — 18 filesystem-level smoke tests using Node.js native test runner. However:
- They test file existence and string patterns, NOT React rendering
- They are NEVER run in CI
- There are ZERO React component render tests
- There is no test framework capable of rendering React components installed

The prior audit's claim of "0 frontend tests" is **almost correct** — there are 0 *render* tests. The 18 existing tests are static filesystem checks only.

---

## C. Build and Static Verification Results

### TypeScript Typecheck

```
Command: npx tsc --noEmit
Result: PASS (0 errors, 0 warnings)
```

All 84 pages, 41 components, API layer, and types pass strict TypeScript checking.

### Vite Production Build

```
Command: npx vite build
Result: PASS — built in 5.83s
```

All 84 pages successfully compile and bundle into production chunks. No build errors. Key output:
- 76 lazy-loaded page chunks generated
- 2 manual chunks: `admin` (106KB), `enterprise` (121KB)
- Vendor chunk: `vendor-react` (134KB)
- Main bundle: `index` (422KB)
- Largest page: `AiChatHub` (50KB)

### Existing Smoke Tests

```
Command: node --test ./tests/*.test.js
Result: 18/18 PASS
```

### Identified Build Issues

| Issue | Severity |
|-------|----------|
| No ESLint configured | Medium — no lint enforcement |
| `FlashInference.tsx.bak` in source tree | Low — dead file |
| `tsconfig.json` has `"types": ["vite/client"]` only — no test types | Low — but blocks adding test framework |

---

## D. Page-by-Page Testability Matrix

### Testability Classification Key

| Class | Description |
|-------|-------------|
| **A** | Renders standalone with minimal/no providers |
| **B** | Renders with router/providers only |
| **C** | Requires mocked Tauri/backend bridge |
| **D** | Requires real desktop/Tauri runtime |
| **E** | Requires external services/auth/env secrets |
| **F** | Cannot be meaningfully tested without full app boot |

### Classification Methodology

Since NO render-test framework is installed, testability classification is based on static analysis of:
1. What each page imports from `backend.ts`
2. Whether it calls backend functions on mount (in `useEffect`)
3. Whether it has fallback/empty states
4. Whether it accepts props from `App.tsx` vs fetches own data

### Page Matrix

| # | Page File | Route ID | Lines | Backend Funcs Used | Testability | Confidence |
|---|-----------|----------|-------|-------------------|-------------|------------|
| 1 | Dashboard.tsx | dashboard | 245 | 3 (listAgents, getAuditLog, getSystemInfo) | C | High |
| 2 | Chat.tsx | chat | 485 | 0 (receives props) | B | High |
| 3 | Agents.tsx | agents | 1595 | 11+ (listAgents, startAgent, stopAgent, etc.) | C | High |
| 4 | Settings.tsx | settings | 1002 | 6 (getConfig, saveConfig, etc.) | B (receives props) | High |
| 5 | SetupWizard.tsx | (modal) | 395 | 0 (receives callbacks) | A | High |
| 6 | Workflows.tsx | workflows | 467 | 5 (workflowList, workflowSave, etc.) | C | High |
| 7 | Audit.tsx | audit | 1237 | 16+ | B (receives events prop) | High |
| 8 | AuditTimeline.tsx | audit-timeline | 195 | 3 | B (receives events prop) | High |
| 9 | AiChatHub.tsx | ai-chat-hub | 1812 | 21+ (sendChat, listProviderModels, etc.) | C-D | High |
| 10 | FlashInference.tsx | flash-inference | 610 | 30+ flash_* commands | D | High |
| 11 | CommandCenter.tsx | command-center | 163 | 7 | C | High |
| 12 | ComplianceDashboard.tsx | compliance | 881 | 7 | C | High |
| 13 | ClusterStatus.tsx | cluster | 371 | 5 | C | High |
| 14 | TrustDashboard.tsx | trust | 313 | 5+ | C | High |
| 15 | DistributedAudit.tsx | distributed-audit | 226 | 3 | C | High |
| 16 | PermissionDashboard.tsx | permissions | 498 | Props from App.tsx | B | High |
| 17 | Protocols.tsx | protocols | 841 | 8+ (MCP + A2A) | C | High |
| 18 | Identity.tsx | identity | 840 | 8+ | C | High |
| 19 | Firewall.tsx | firewall | 238 | 3 | C | High |
| 20 | DeveloperPortal.tsx | developer-portal | 454 | 3 | C | Medium |
| 21 | AgentBrowser.tsx | browser | 790 | 2+ | C | High |
| 22 | CodeEditor.tsx | code-editor | 1196 | 7 | C-D | High |
| 23 | Terminal.tsx | terminal | 454 | 3 (terminalExecute) | D | High |
| 24 | FileManager.tsx | file-manager | 618 | 5+ | D | High |
| 25 | SystemMonitor.tsx | system-monitor | 705 | 2+ | C | High |
| 26 | Documents.tsx | documents | 1143 | 8+ | C | High |
| 27 | ModelHub.tsx | model-hub | 1798 | 15+ | C-D | High |
| 28 | NotesApp.tsx | notes | 502 | 4 | C | High |
| 29 | ProjectManager.tsx | project-manager | 597 | 4 | C | High |
| 30 | DatabaseManager.tsx | database | 548 | 5 (dbConnect, dbExecuteQuery, etc.) | D-E | High |
| 31 | ApiClient.tsx | api-client | 484 | 4 | C | High |
| 32 | DesignStudio.tsx | design-studio | 397 | 3+ | C | High |
| 33 | EmailClient.tsx | email-client | 581 | 9 | C-E | High |
| 34 | Messaging.tsx | messaging | 514 | 8 | C-E | High |
| 35 | MediaStudio.tsx | media-studio | 377 | 3+ | C | Medium |
| 36 | AppStore.tsx | app-store/marketplace | 665 | 4+ | C | High |
| 37 | DeployPipeline.tsx | deploy-pipeline | 881 | 5+ | C | High |
| 38 | LearningCenter.tsx | learning-center | 1246 | 8 | C | High |
| 39 | ApprovalCenter.tsx | approvals | 449 | 10 | C | High |
| 40 | PolicyManagement.tsx | policy-management | 383 | 4+ | C | High |
| 41 | TimeMachine.tsx | time-machine | 1605 | 10+ | C | High |
| 42 | VoiceAssistant.tsx | voice-assistant | 1016 | 8+ | D-E | High |
| 43 | WorldSimulation.tsx | simulation | 1163 | 10+ | C | High |
| 44 | ComputerControl.tsx | computer-control | 469 | 14 | D | High |
| 45 | MissionControl.tsx | mission-control | 384 | 5+ | C | High |
| 46 | AgentDnaLab.tsx | dna-lab | 1271 | 23+ | C | High |
| 47 | TimelineViewer.tsx | timeline-viewer | 329 | 2 | C | High |
| 48 | KnowledgeGraph.tsx | knowledge-graph | 864 | 5+ | C | High |
| 49 | ImmuneDashboard.tsx | immune-dashboard | 326 | 4+ | C | High |
| 50 | ConsciousnessMonitor.tsx | consciousness | 348 | 4+ | C | High |
| 51 | DreamForge.tsx | dreams | 281 | 3+ | C | High |
| 52 | TemporalEngine.tsx | temporal | 302 | 4+ | C | High |
| 53 | Civilization.tsx | civilization | 1397 | 28+ | C | High |
| 54 | SelfRewriteLab.tsx | self-rewrite | 284 | 3+ | C | High |
| 55 | AdminDashboard.tsx | admin-console | 169 | 2+ | C | High |
| 56 | AdminUsers.tsx | admin-users | 194 | 4 | C | High |
| 57 | AdminFleet.tsx | admin-fleet | 238 | 3 | C | High |
| 58 | AdminPolicyEditor.tsx | admin-policies | 250 | 2 | C | High |
| 59 | AdminCompliance.tsx | admin-compliance | 226 | 2 | C | High |
| 60 | AdminSystemHealth.tsx | admin-health | 308 | 5 | C | High |
| 61 | Integrations.tsx | integrations | 395 | 4 | C | High |
| 62 | Login.tsx | login | 364 | 4+ | C-E | High |
| 63 | Workspaces.tsx | workspaces | 792 | 8+ | C | High |
| 64 | Telemetry.tsx | telemetry | 326 | 4+ | C | High |
| 65 | UsageBilling.tsx | usage-billing | 360 | 4+ | C | High |
| 66 | Scheduler.tsx | scheduler | 392 | 4+ | C | High |
| 67 | TokenEconomy.tsx | token-economy | 398 | 5+ | C | High |
| 68 | MeasurementDashboard.tsx | measurement | 379 | 5+ | C | High |
| 69 | MeasurementSession.tsx | measurement-session | 270 | 3 | C | High |
| 70 | MeasurementCompare.tsx | measurement-compare | 194 | 2 | C | High |
| 71 | MeasurementBatteries.tsx | measurement-batteries | 114 | 1 | C | High |
| 72 | CapabilityBoundaryMap.tsx | capability-boundaries | 222 | 2 | C | High |
| 73 | ModelRouting.tsx | model-routing | 156 | 3+ | C | High |
| 74 | ABValidation.tsx | ab-validation | 199 | 2+ | C | High |
| 75 | BrowserAgent.tsx | browser-agent | 176 | 9 | D | High |
| 76 | GovernanceOracle.tsx | governance-oracle | 145 | 3 | C | High |
| 77 | GovernedControl.tsx | governed-control | 450 | 10+ | D | High |
| 78 | WorldSimulation2.tsx | world-sim | 233 | 7 | C | High |
| 79 | Perception.tsx | perception | 280 | 5+ | D | High |
| 80 | AgentMemory.tsx | agent-memory | 303 | 4+ | C | High |
| 81 | ExternalTools.tsx | external-tools | 273 | 6+ | C | High |
| 82 | Collaboration.tsx | collab-protocol | 428 | 9 | C | High |
| 83 | SoftwareFactory.tsx | software-factory | 426 | 6+ | C | High |
| — | commandCenterUi.tsx | (utility, not routed) | 89 | 0 | N/A | High |

### Testability Distribution Summary

| Class | Count | % | Description |
|-------|-------|---|-------------|
| A | 1 | 1% | SetupWizard — renders standalone with no backend deps |
| B | 5 | 6% | Chat, Settings, Audit, AuditTimeline, PermissionDashboard — receive props from App.tsx |
| C | 64 | 77% | Require mocked Tauri `invoke()` bridge to render without crashing |
| C-D | 3 | 4% | AiChatHub, CodeEditor, ModelHub — some features need real runtime |
| C-E | 3 | 4% | EmailClient, Messaging, Login — need external service tokens |
| D | 6 | 7% | FlashInference, Terminal, FileManager, ComputerControl, BrowserAgent, GovernedControl, Perception — need real Tauri runtime |
| D-E | 1 | 1% | DatabaseManager — needs real DB connection |

**Key finding**: 77% of pages (Class C) could be smoke-tested with a mock `window.__TAURI_INTERNALS__` + mock `invoke()` function. Only 12% strictly require a running Tauri desktop runtime.

---

## E. Confirmed Working Frontend Surfaces

**Evidence type**: TypeScript compilation + Vite bundle + static analysis. NOT runtime render verification.

| What | Evidence |
|------|----------|
| All 84 page files compile | `tsc --noEmit` passes with 0 errors |
| All 84 pages bundle | `vite build` produces chunks for every page |
| All 83 routed pages are reachable | Every page ID in `renderPage()` matches a lazy import |
| All pages except 2 import from `backend.ts` | 82/84 pages call real Tauri commands |
| 18 filesystem smoke tests pass | File existence, exports, structure verified |
| API layer is consistent | 646 functions, all route through `invokeDesktop()` |
| Error boundaries exist | `PageErrorBoundary.tsx` wraps all page renders |
| No broken imports | TypeScript strict mode catches any missing exports |

**Confidence: HIGH for compile-time correctness. UNVERIFIED for runtime rendering.**

---

## F. Confirmed Broken Frontend Surfaces

| What | Evidence | Severity |
|------|----------|----------|
| CI `frontend-tests` job does not run tests | `.gitlab-ci.yml:83-86` runs `npm run build` only, not `npm test` | **Critical** |
| 0 React render tests | No `@testing-library/react`, no vitest, no jsdom installed | **Critical** |
| Existing 18 smoke tests not in CI | `npm test` is never called in the pipeline | **High** |
| `FlashInference.tsx.bak` dead file | Exists in `app/src/pages/` | Low |

---

## G. Mock/Demo/Fallback-Only Surfaces

### App-Level Mock Mode

`App.tsx:426`: `useState<RuntimeMode>("mock")` — the app **defaults to mock mode** and upgrades to "desktop" only when `window.__TAURI__` is detected (line 535). In mock mode:
- A persistent amber warning banner is shown (line 1772)
- All handler functions that call backend return early or no-op (lines 773, 808, 829, 846, 863, 891, 907)
- Agents list stays empty
- Chat sends return mock responses
- Config saves are skipped

**This means**: Opening the app in a browser (without Tauri) gives a functional-looking shell with zero real data.

### Pages with Demo/Preview Modes

| Page | Pattern | Details |
|------|---------|---------|
| **ComputerControl.tsx** | `mode: 'demo' \| 'live'` | Has `DEMO_ACTIONS` array (8 hardcoded steps). Defaults to `'live'` but has explicit "Preview Mode" toggle. Both modes exist — demo is supplementary. |
| **DeveloperPortal.tsx** | `INITIAL_STEPS` | Hardcoded verification checklist. UI scaffolding, not fake data. |

### Pages with Hardcoded UI Content (Not Mock Data)

| Page | What | Assessment |
|------|------|------------|
| LearningCenter.tsx | `COURSES`, `CHALLENGES`, `KNOWLEDGE` arrays | Static curriculum content — intentional, not a mock. Also imports 8 backend functions for tracking progress. |
| EmailClient.tsx | `FOLDERS`, `TEMPLATES` arrays | UI scaffolding (folder names, email templates). Real email ops go through backend. |
| ProjectManager.tsx | `COLUMNS`, `TAGS`, `DEFAULT_SPRINTS` | Default Kanban config. Real project data from backend. |
| Workflows.tsx | `NODE_PALETTE`, `TEMPLATES` | Workflow node type definitions and templates. Backend-wired for save/load. |
| Messaging.tsx | `PLATFORMS` array | Platform configuration cards mapping to real config fields. Not fake data. |
| NotesApp.tsx | `TEMPLATES` | Note templates (blank, meeting, todo). Real note CRUD from backend. |
| AdminPolicyEditor.tsx | `TEMPLATES` | Policy preset templates (strict, balanced, permissive). Real policy save to backend. |
| SetupWizard.tsx | `AGENTS` array | 6 default agent setup choices. Callbacks to App.tsx for real creation. |

### Component-Level Mock Fallbacks (Not in Pages)

Three browser-mode components silently fall back to mock implementations when Tauri is unavailable:

| Component | File | Mock Behavior |
|-----------|------|---------------|
| BuildMode | `components/browser/BuildMode.tsx` | `generateMockCode()` creates fake HTML/CSS/JS chunks. `catch { /* fall through to mock */ }` |
| LearnMode | `components/browser/LearnMode.tsx` | `runMockLearning()` — explicitly named mock function. Conditional: if desktop → real; else → mock |
| ResearchMode | `components/browser/ResearchMode.tsx` | `simulateAgent()` — named "simulate". Try Tauri, catch → "continue in mock mode" |

These are sub-components of `AgentBrowser.tsx` and are the **only cases** where mock behavior is silent (no user-facing "demo mode" message).

### Voice Mock Fallback

`app/src/voice/PushToTalk.ts` has a 3-tier fallback:
1. Tauri desktop STT (if `hasDesktopRuntime()`)
2. Web Speech API (browser native)
3. Hardcoded mock transcript: `"create an agent to post weekly Rust updates"` with source `"mock-whisper"`

### Setup Wizard Mock Data

When `runtimeMode !== "desktop"`, the setup wizard (App.tsx lines 1896–1925) returns:
- Mock hardware: `gpu: "Mock GPU", vram_mb: 8192, ram_mb: 16384`
- Ollama: `{ connected: false }`
- Models: `[]`

### Explicit "No Demo Data" Policy

App.tsx contains explicit comments (lines 386–387):
> "Demo agent/chat functions removed — no fake data is served when desktop runtime is absent."

Chat without backend returns a clear message (line 1227):
> "Chat requires the desktop runtime. No simulated responses are provided."

### Pages with Explicit "Desktop Required" Messages

These pages detect browser mode and show clear user-facing messages:

| Page | Message |
|------|---------|
| AppStore.tsx | "Agent Store requires the desktop runtime." |
| DeployPipeline.tsx | "Deploy Pipeline requires the Tauri desktop runtime to execute real builds." |
| TrustDashboard.tsx | "Reputation system requires desktop runtime." |
| WorldSimulation.tsx | "The desktop backend is required to create persistent, auditable world simulations." |
| VoiceAssistant.tsx | "Voice capture requires the desktop runtime." |
| CodeEditor.tsx | Agent assist shows "(desktop required)" label |

### Connection Status Indicator

App.tsx topbar always shows a visible chip: `"live"` (desktop connected) or `"mock"` (browser mode).

### Verdict on Mocks

**No page serves fake data as if it were real.** The patterns found are:
1. App-level mock mode with visible warning banner + topbar indicator
2. One page (ComputerControl) with an explicit Preview/Live toggle
3. Three browser sub-components (BuildMode, LearnMode, ResearchMode) silently fall back to mock — the **only silent mocks**
4. Voice has a hardcoded fallback transcript — silent but low-impact
5. Static UI content (templates, palettes, platform lists) — configuration, not fake state
6. Explicit "no demo data" policy documented in code comments

---

## H. Not Verifiable from Current Environment

The following **cannot be verified** without a running Tauri desktop backend:

| Category | Pages Affected | Why |
|----------|---------------|-----|
| Any page rendering with real data | ALL 83 | All data comes through `invokeDesktop()` which requires `window.__TAURI__` |
| Runtime render behavior | ALL 83 | No jsdom/React test framework installed |
| Flash Inference (local LLM) | FlashInference.tsx | Requires llama.cpp runtime |
| Terminal execution | Terminal.tsx | Requires `terminalExecute` IPC |
| File system operations | FileManager.tsx | Requires `fileManagerList` IPC |
| Computer control | ComputerControl.tsx, GovernedControl.tsx | Requires screen capture + input control |
| Browser automation | BrowserAgent.tsx | Requires browser agent runtime |
| Perception/vision | Perception.tsx | Requires camera/screen capture |
| Email send/receive | EmailClient.tsx | Requires OAuth tokens + IMAP/SMTP |
| Messaging platforms | Messaging.tsx | Requires platform bot tokens |
| Database connections | DatabaseManager.tsx | Requires real SQLite/DB connection |
| Voice processing | VoiceAssistant.tsx | Requires Speech API + potentially local model |
| OAuth/auth flows | Login.tsx | Requires auth provider |
| Cluster/mesh | ClusterStatus.tsx | Requires peer discovery |

**Bottom line**: Without a Tauri runtime, we can verify compile-time correctness but NOT runtime behavior. Every page will crash on first `useEffect` that calls a backend function without a mocked invoke bridge.

---

## I. Highest-Risk Frontend Gaps

### Critical

| # | Risk | Evidence | Impact |
|---|------|----------|--------|
| 1 | **0 React render tests** | No test framework installed (no vitest, jest, jsdom, @testing-library) | Any page refactor can silently break rendering. No way to catch JSX/hook errors pre-deploy. |
| 2 | **CI does not run frontend tests** | `.gitlab-ci.yml:83-86` runs `npm run build` only | Existing 18 smoke tests provide no CI protection. Any new test added won't run automatically. |
| 3 | **19 pages silently swallow backend errors** | `.catch(() => {})` pattern in AgentMemory, Agents, AiChatHub, ApiClient, ApprovalCenter, AuditTimeline, Audit, DeployPipeline, EmailClient, ExternalTools, Firewall, FlashInference, GovernedControl, LearningCenter, ModelHub, Settings, SetupWizard, TokenEconomy, VoiceAssistant | Backend failures are invisible to users. Pages appear "working" with empty state when backend is actually failing. |

### High

| # | Risk | Evidence | Impact |
|---|------|----------|--------|
| 4 | **Mock mode indistinguishable from broken** | App defaults to `RuntimeMode("mock")`, many handler functions silently no-op | A user opening the app in a browser sees a beautiful UI that does nothing. The amber banner is the only clue. |
| 5 | **No ESLint** | No eslint config in app/ | No code quality enforcement beyond TypeScript types |
| 6 | **6 truly unused components** | ConsentApprovalModal, SpeculativePreview, FuelBar, MetricCard, DataStream, CyberButton, TimelineStream, RadialGauge — 0 importers | Dead code adding maintenance burden and confusion |

### Medium

| # | Risk | Evidence | Impact |
|---|------|----------|--------|
| 7 | **MeasurementSession receives empty sessionId** | `App.tsx:1620` passes `sessionId=""` | Page renders but cannot load any session data — effectively a stub when navigated directly |
| 8 | **ComputerControl has demo mode** | `DEMO_ACTIONS` array, "Preview Mode" toggle | Users may confuse demo walkthrough with real computer control |
| 9 | **Custom router, no URL-based routing for most pages** | State-based `setPage()`, no browser back/forward | Users lose navigation context on refresh |
| 10 | **No 404 / unknown page handling** | `renderPage()` falls through to `<Settings>` for unknown pages | Any invalid page ID silently shows Settings |

### Low

| # | Risk | Evidence | Impact |
|---|------|----------|--------|
| 11 | **Hardcoded learning content** | `LearningCenter.tsx` — courses, challenges as const arrays | Content updates require code changes |
| 12 | **`.bak` file in source** | `FlashInference.tsx.bak` | Noise |
| 13 | **commandCenterUi.tsx not a page** | Not routed, utility module | Minor — file naming suggests page but isn't one |

---

## J. Recommended Next Steps

### Immediate (Unblocks All Testing)

1. **Install Vitest + jsdom + @testing-library/react**
   ```bash
   cd app && npm install -D vitest jsdom @testing-library/react @testing-library/jest-dom
   ```
   Add `vitest.config.ts` with jsdom environment.

2. **Fix CI to run `npm test`**
   Change `.gitlab-ci.yml` line 86:
   ```yaml
   script:
     - cd app
     - npm ci
     - npm test       # ← ADD THIS
     - npm run build
   ```

3. **Create a Tauri invoke mock** for testing
   A single mock module that intercepts `window.__TAURI_INTERNALS__` and returns empty/default responses would unblock testing for 77% of pages (Class C).

### Short-Term (First Render Coverage)

4. **Add page import tests** — verify every lazy-loaded page can be dynamically imported without crashing (no render needed, just import validation)

5. **Add smoke render tests for Class A-B pages** (6 pages) — these can render with just a minimal provider wrapper:
   - SetupWizard (standalone)
   - Chat (pass empty props)
   - Settings (pass empty props)
   - Audit (pass empty events)
   - AuditTimeline (pass empty events)
   - PermissionDashboard (pass agent props)

6. **Add smoke render tests for Class C pages** (64 pages) — with mocked `invoke()` returning empty arrays/objects

### Medium-Term

7. **Add ESLint** with react-hooks plugin to catch hook ordering issues
8. **Fix silent error swallowing** — replace `.catch(() => {})` with `.catch((e) => setError(e))` in at least the 19 affected pages
9. **Add fallback/error state to renderPage()** — currently falls through to Settings for unknown page IDs
10. **Fix MeasurementSession empty sessionId** — either pass a real ID or don't render the page without one

### Long-Term

11. **Playwright E2E tests** against a real Tauri build for critical flows (agent creation, chat, audit)
12. **Visual regression tests** for key pages
13. **Consider migrating to react-router** for proper URL-based navigation, back/forward support, and deep linking

---

## Appendix: Unused Component Audit

| Component | File | Imported By | Verdict |
|-----------|------|-------------|---------|
| ConsentApprovalModal | components/agents/ | NOBODY | Dead code — delete |
| SpeculativePreview | components/agents/ | NOBODY | Dead code — delete |
| FuelBar | components/ui/ | NOBODY | Dead code — delete |
| MetricCard | components/ui/ | GlassPanel (also unused) | Dead code — delete |
| GlassPanel | components/ui/ | MetricCard (also unused) | Dead code — delete |
| DataStream | components/ui/ | NOBODY | Dead code — delete |
| CyberButton | components/ui/ | NOBODY | Dead code — delete |
| TimelineStream | components/viz/ | NOBODY | Dead code — delete |
| RadialGauge | components/viz/ | NOBODY | Dead code — delete |
| ActivityFeed | components/agents/ | Agents.tsx | USED — keep |
| Avatar | components/agents/ | AgentCard.tsx | USED (transitively) — keep |
| ActivityStream | components/browser/ | ResearchMode.tsx | USED (transitively) — keep |
| KnowledgeCard | components/browser/ | LearnMode.tsx | USED (transitively) — keep |
| VoiceOrb | components/fx/ | VoiceOverlay.tsx | USED (transitively) — keep |

**9 components** are truly dead code. **5 components** are used transitively.

---

## Appendix: Silent Error Swallowing Pages

These 19 pages use `.catch(() => {})` to silently discard backend errors:

1. AgentMemory.tsx
2. Agents.tsx
3. AiChatHub.tsx
4. ApiClient.tsx
5. ApprovalCenter.tsx
6. AuditTimeline.tsx
7. Audit.tsx
8. DeployPipeline.tsx
9. EmailClient.tsx
10. ExternalTools.tsx
11. Firewall.tsx
12. FlashInference.tsx
13. GovernedControl.tsx
14. LearningCenter.tsx
15. ModelHub.tsx
16. Settings.tsx
17. SetupWizard.tsx
18. TokenEconomy.tsx
19. VoiceAssistant.tsx

These pages will show empty/loading states when the backend is unavailable, with no error indication to the user.

---

## Final Verdict

| Claim from Prior Audit | This Audit's Finding |
|------------------------|---------------------|
| "84 frontend pages" | **Confirmed** — 84 .tsx files, 83 routed |
| "0 frontend tests" | **Partially wrong** — 18 filesystem smoke tests exist, but 0 React render tests |
| "98% of pages fully wired" | **Confirmed structurally** — 82/84 pages import from backend.ts. Cannot confirm runtime wiring without Tauri. |
| "No fake data served as real" | **Confirmed** — mock mode has visible banner, ComputerControl demo mode is labeled |
| "All 84 pages verified" | **Overstated** — compilation verified, runtime rendering NOT verified |

**The frontend compiles and bundles cleanly. Every page exists, exports correctly, and imports real backend functions. But without a React test framework or running Tauri runtime, we cannot confirm that any page actually renders correctly at runtime. The claim of "98% wired" is structurally true based on import analysis, but functionally unverified.**
