# NEXUS OS COMPLETE AUDIT REPORT

Generated: 2026-03-27
Commit: latest on main

---

## SUMMARY

| Metric | Value |
|--------|-------|
| Total workspace crates | 58 |
| Crates compiling | **58 (ALL)** |
| Crates with clippy errors | 0 |
| Crates with test failures | 0 |
| New Phase 9.6+ crates | 14 |
| Tests in new crates | 283 (all passing) |
| Total Tauri commands (#[tauri::command]) | 619 |
| Registered in generate_handler![] | 577 |
| Commands with todo!()/unimplemented!() | **0** |
| Commands with no frontend caller | 4 |
| Total frontend pages | 84 |
| Pages with significant mock data (>5) | 17 |
| Pages with no backend calls at all | 1 (commandCenterUi — utility, not a page) |
| Backend.ts export functions | 598 |
| Backend functions never called from pages | **150** |
| Buttons with empty handlers | 0 |
| .env files committed | 0 |
| Hardcoded secrets found | 0 |
| Agent manifest files | 54 |
| Validation run data files | 4 (real data, ~30MB) |

---

## CRITICAL FINDINGS

### C1: `nexus-governance-engine` not wired to desktop app
- **Severity**: CRITICAL
- **File**: `app/src-tauri/Cargo.toml` (missing), `tests/integration/Cargo.toml` (missing)
- **Detail**: Crate exists with 9 tests, compiles, but is NOT a dependency of the Tauri app or integration tests. No tauri commands, no state field in AppState. This is the governance evolution engine — it should be accessible from the UI.
- **Impact**: Governance engine features invisible in demo.

### C2: 42 Tauri commands defined but NOT in generate_handler![]
- **Severity**: CRITICAL
- **Detail**: 619 `#[tauri::command]` functions exist but only 577 are registered in `generate_handler![]`. The 42 unregistered commands exist in the `runtime` module (re-declarations for Tauri window context). These are the _runtime wrapper_ versions that call the same underlying functions. This is **by design** — runtime module wraps non-Tauri plain functions for the Tauri runtime. Not a bug.
- **Revised severity**: INFO — architectural pattern, not a defect.

---

## MAJOR FINDINGS

### M1: 150 backend.ts functions never called from any page
- **Severity**: MAJOR
- **Detail**: 150 out of 598 exported backend functions in `app/src/api/backend.ts` are never imported by any page component. These fall into categories:
  - **SDK-era memory functions** (agentMemoryRemember, agentMemoryRecall, etc.) — superseded by Phase 14 persistent memory
  - **Research/build session functions** (startResearch, buildAppendCode, etc.) — wired to backend but no dedicated page
  - **Flash inference advanced** (flashProfileModel, flashAutoConfigure, etc.) — available but not in UI
  - **Validation run functions** (cmExecuteValidationRun, cmListValidationRuns, etc.) — backend works but page doesn't call them
  - **Computer control actions** (ccExecuteAction) — GovernedControl page uses other functions
  - **World simulation actions** (simSubmit, simRun, simBranch) — WorldSimulation2 page uses different approach
  - **Token economy wallet** (tokenGetWallet, tokenCreateWallet) — no dedicated wallet UI
  - **Collaboration/factory specifics** (memoryListAgents, toolsListAvailable, swfSubmitArtifact) — backend ready, page doesn't use all functions
- **Impact**: Features exist in backend but are inaccessible from UI. Not blocking demo if the pages that DO exist work correctly.

### M2: 17 pages with significant mock data indicators (>5 matches)
- **Severity**: MAJOR
- **Detail**: These pages have "mock", "placeholder", "TODO", "sample", "lorem", or "hardcoded" in their source. Note: many of these are CSS class names like "placeholder" in input fields or variable names — NOT actual mock data. Manual review needed per page:
  - `Civilization` (28) — highest, likely has placeholder content
  - `Integrations` (22) — known to have placeholder integration cards
  - `VoiceAssistant` (15) — has placeholder voice UI
  - `ApiClient` (15) — has sample request data
  - `KnowledgeGraph` (12) — has sample node data
  - `AgentDnaLab` (11) — genome visualization samples
  - `Audit` (11) — uses "sample" in variable names
  - `Settings` (11) — "placeholder" in input fields
  - `Protocols` (10) — protocol display
  - `Telemetry` (9) — telemetry mock metrics
  - `TrustDashboard` (8) — trust score samples
  - `AiChatHub` (7) — chat placeholder text
  - `DeployPipeline` (7) — deployment step samples
  - `ModelHub` (6) — model card samples
  - `LearningCenter` (6) — learning content
  - `ProjectManager` (6) — project templates
  - `Collaboration` (6) — mostly "placeholder" in input fields
- **Impact**: Some pages may show fake data. Needs per-page manual review.

### M3: 4 Tauri commands with no frontend caller
- **Severity**: MAJOR (minor functionality gap)
- **Commands**:
  - `get_rate_limit_status` — rate limiting info, not surfaced in any page
  - `flash_run_benchmark` — flash inference benchmarking, not in FlashInference page
  - `flash_export_benchmark_report` — benchmark export, not in UI
  - `browser_screenshot` — browser screenshot, GovernedControl page uses different approach
- **Impact**: Minor features not accessible from UI.

---

## MINOR FINDINGS

### m1: NotesApp page has no .catch() error handling
- **Severity**: MINOR
- **File**: `app/src/pages/NotesApp.tsx`
- **Detail**: 10+ backend invocations but no `.catch()` calls. Errors would be silently swallowed.
- **Impact**: Notes app may fail silently on backend errors.

### m2: commandCenterUi.tsx appears in page directory but is a utility
- **Severity**: MINOR
- **File**: `app/src/pages/commandCenterUi.ts`
- **Detail**: Not a page, not in router — it's a shared style utility (`commandPageStyle`, `alpha()`, etc.). Should arguably be in `app/src/lib/` or `app/src/utils/`.
- **Impact**: None functionally.

### m3: ModelHub.tsx has a potentially incomplete onClick handler
- **Severity**: MINOR
- **File**: `app/src/pages/ModelHub.tsx:1060`
- **Detail**: `onClick={() =>` appears to span multiple lines, may be fine but worth verifying.
- **Impact**: Likely just a multi-line arrow function, not a bug.

---

## INFO

### I1: All 58 crates compile with zero errors
Every workspace crate compiles cleanly. Only one pre-existing warning in `nexus-desktop-backend` about `cfg(feature = "flash-infer")` — this is a known non-issue.

### I2: All 14 new crates (Phases 9.6-17) have comprehensive tests
| Crate | Tests |
|-------|-------|
| nexus-capability-measurement | 73 |
| nexus-token-economy | 29 |
| nexus-agent-memory | 21 |
| nexus-perception | 19 |
| nexus-world-simulation | 18 |
| nexus-collab-protocol | 18 |
| nexus-software-factory | 18 |
| nexus-external-tools | 17 |
| nexus-computer-control | 16 |
| nexus-predictive-router | 14 |
| nexus-governance-oracle | 12 |
| nexus-browser-agent | 12 |
| nexus-governance-engine | 9 |
| nexus-governance-evolution | 7 |
| **Total** | **283** |

### I3: Zero security issues found
- No hardcoded API keys or secrets
- No committed .env files
- No secrets in recent git history
- All API keys read from environment variables via `std::env::var()`

### I4: 54 prebuilt agent manifests available
Agent ecosystem is populated with real agent definitions.

### I5: 4 real validation run data files (~30MB)
Real benchmark data from actual LLM-judged evaluation runs.

---

## PER-CRATE STATUS (New Phase 9.6-17 Crates)

| Crate | Workspace | App Dep | Integration | Tests | Compiles | Clippy |
|-------|-----------|---------|-------------|-------|----------|--------|
| nexus-capability-measurement | OK | OK | OK | 73 | OK | OK |
| nexus-governance-oracle | OK | OK | OK | 12 | OK | OK |
| nexus-governance-engine | OK | **MISSING** | **MISSING** | 9 | OK | OK |
| nexus-governance-evolution | OK | OK | OK | 7 | OK | OK |
| nexus-predictive-router | OK | OK | OK | 14 | OK | OK |
| nexus-token-economy | OK | OK | OK | 29 | OK | OK |
| nexus-browser-agent | OK | OK | OK | 12 | OK | OK |
| nexus-computer-control | OK | OK | OK | 16 | OK | OK |
| nexus-world-simulation | OK | OK | OK | 18 | OK | OK |
| nexus-perception | OK | OK | OK | 19 | OK | OK |
| nexus-agent-memory | OK | OK | OK | 21 | OK | OK |
| nexus-external-tools | OK | OK | OK | 17 | OK | OK |
| nexus-collab-protocol | OK | OK | OK | 18 | OK | OK |
| nexus-software-factory | OK | OK | OK | 18 | OK | OK |

---

## PER-PAGE STATUS (New Phase 9.6-17 Pages)

| Page | In App.tsx | Backend Calls | Mock Indicators | Status |
|------|-----------|---------------|-----------------|--------|
| MeasurementDashboard | OK | Yes | 0 | CLEAN |
| MeasurementSession | OK | Yes | 0 | CLEAN |
| MeasurementCompare | OK | Yes | 0 | CLEAN |
| MeasurementBatteries | OK | Yes | 0 | CLEAN |
| CapabilityBoundaryMap | OK | Yes | 0 | CLEAN |
| ABValidation | OK | Yes | 0 | CLEAN |
| ModelRouting | OK | Yes | 1 | CLEAN |
| GovernanceOracle | OK | Yes | 0 | CLEAN |
| TokenEconomy | OK | Yes | 1 | CLEAN |
| BrowserAgent | OK | Yes | 2 | OK |
| GovernedControl | OK | Yes | 0 | CLEAN |
| WorldSimulation2 | OK | Yes | 0 | CLEAN |
| Perception | OK | Yes | 4 | OK |
| AgentMemory | OK | Yes | 5 | OK |
| ExternalTools | OK | Yes | 2 | CLEAN |
| Collaboration | OK | Yes | 6 | OK (placeholder in inputs) |
| SoftwareFactory | OK | Yes | 4 | OK |

---

## UNWIRED COMMANDS (in generate_handler but no frontend caller)

1. `get_rate_limit_status`
2. `flash_run_benchmark`
3. `flash_export_benchmark_report`
4. `browser_screenshot`

---

## UNCALLED BACKEND FUNCTIONS (Top categories)

### SDK Memory (superseded by nexus-agent-memory)
agentMemoryRemember, agentMemoryRecall, agentMemoryRecallByType, agentMemoryForget, agentMemoryGetStats, agentMemorySave, getAgentMemories

### Research/Build Sessions (no dedicated page)
startResearch, researchAgentAction, completeResearch, getResearchSession, listResearchSessions, buildAppendCode, buildAddMessage, completeBuild, getBuildSession, getBuildCode, getBuildPreview

### Flash Inference Advanced
flashProfileModel, flashAutoConfigure, flashListSessions, flashGetMetrics, flashEstimatePerformance, flashCatalogRecommend, flashCatalogSearch, flashDownloadModel, flashDownloadMulti, flashDeleteLocalModel, flashAvailableDiskSpace, flashGetModelDir

### Capability Measurement Advanced
cmGetProfile, cmTriggerFeedback, cmEvaluateResponse, cmExecuteValidationRun, cmListValidationRuns, cmGetValidationRun, cmThreeWayComparison

### Token Economy
tokenGetWallet, tokenCreateWallet, tokenCalculateSpawn, tokenCreateDelegation, tokenGetDelegations

### Computer Control / Simulation
ccExecuteAction, simSubmit, simRun, simGetResult, simGetRisk, simBranch

### Autonomous Agent
assignAgentGoal, stopAgentGoal, startAutonomousLoop, stopAutonomousLoop, getAgentCognitiveStatus

### Misc
clearAllAgents, createAgent, getAgentPerformance, detectHardware, checkOllama, pullOllamaModel, getSystemInfo, getAgentIdentity, schedulerHistory, schedulerRunnerStatus, executeTeamWorkflow, transferAgentFuel, runContentPipeline, getLivePreview, publishToMarketplace, installFromMarketplace

---

## MISSING ERROR HANDLING

| Page | Backend Calls | .catch() | Status |
|------|--------------|----------|--------|
| NotesApp | 10+ | 0 | NEEDS FIX |

All other pages with backend calls have at least basic .catch() handling.

---

## SECURITY FINDINGS

**NONE.** Clean security posture:
- Zero hardcoded secrets
- Zero committed .env files
- All sensitive values via environment variables
- URL denylist blocks localhost/metadata endpoints in external tools

---

## RECOMMENDATIONS FOR DEMO

### Must fix (CRITICAL):
1. Wire `nexus-governance-engine` to app (add to Cargo.toml dependencies)

### Should fix (MAJOR):
2. Add .catch() to NotesApp.tsx backend calls
3. Review the 17 pages with mock data indicators — verify which are real mock data vs CSS/variable naming

### Nice to have (MINOR):
4. Surface the 4 unwired Tauri commands in relevant pages
5. Call more backend functions from pages (150 unused)
6. Move commandCenterUi.ts to app/src/lib/

### Already excellent:
- 58/58 crates compile
- 283 tests across 14 new crates, all passing
- Zero security issues
- All new pages wired to router with backend calls
- Hash-chained audit trails across 4 subsystems
- Real validation data (30MB of actual LLM evaluation runs)
