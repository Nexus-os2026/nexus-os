# NEXUS OS COMPLETE AUDIT REPORT
Generated: 2026-03-27T12:59:23+00:00
Commit: `5deabe6b14d00ba64c61b95a14dc2028dfc6a6c8`
Repo root: `/home/nexus/NEXUS/nexus-os`
Audit mode: full-system verification only. No source fixes were applied.
Raw evidence: `audit_artifacts/` contains the command outputs, crate logs, and derived analysis used below.

## SUMMARY
```json
{
  "generated": "2026-03-27T12:59:23+00:00",
  "commit": "5deabe6b14d00ba64c61b95a14dc2028dfc6a6c8",
  "repo_root": "/home/nexus/NEXUS/nexus-os",
  "raw_artifacts_dir": "audit_artifacts/",
  "total_crates": 58,
  "crates_compiling": 58,
  "crates_with_clippy_failures": 2,
  "crates_with_test_failures": 1,
  "crates_with_zero_tests": 4,
  "total_tauri_commands": 619,
  "commands_with_todo_or_unimplemented": 0,
  "commands_with_no_frontend_caller": 0,
  "commands_missing_backend_ts_binding": 0,
  "frontend_build": "PASS",
  "frontend_typecheck": "PASS",
  "total_frontend_pages": 84,
  "pages_with_verified_mock_or_demo_logic": 4,
  "pages_with_no_backend_calls": 1,
  "pages_with_no_loading_state": 15,
  "direct_page_invoke_calls": 5,
  "unused_public_functions": 62,
  "backend_wrappers_unused_by_pages": 152,
  "real_orphan_modules": 0
}
```

## CRITICAL FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|CRITICAL|Governance Engine / Evolution Unwired|Both governance crates are declared in workspace/app/tests but have zero runtime references in the desktop backend, so the demo cannot exercise them.|Cargo.toml:50, Cargo.toml:51, app/src-tauri/Cargo.toml:38, app/src-tauri/Cargo.toml:39, tests/integration/Cargo.toml:52, tests/integration/Cargo.toml:53, app/src-tauri/src/main.rs:839, app/src-tauri/src/main.rs:923, app/src-tauri/src/main.rs:933|
|CRITICAL|Governed Control Is Read-Only And Simulated|The AGENT LAB governed-control page never calls `ccExecuteAction`, and the computer-control engine simulates every non-terminal action by returning `Executed: ...` instead of touching the desktop.|app/src/pages/GovernedControl.tsx:3, app/src/pages/GovernedControl.tsx:4, app/src/pages/GovernedControl.tsx:5, app/src/pages/GovernedControl.tsx:6, app/src/pages/GovernedControl.tsx:93, app/src/pages/GovernedControl.tsx:94, app/src/pages/GovernedControl.tsx:95, app/src/pages/GovernedControl.tsx:96, app/src/api/backend.ts:3549, crates/nexus-computer-control/src/engine.rs:98, crates/nexus-computer-control/src/engine.rs:137, crates/nexus-computer-control/src/engine.rs:168, crates/nexus-computer-control/src/engine.rs:213|
|CRITICAL|Governance Oracle Tauri Layer Is Stubbed|`oracle_verify_token` only checks whether the payload string is non-empty, `oracle_get_agent_budget` only mirrors `fuel_remaining`, and the page never invokes `oracleVerifyToken` even while advertising cryptographic guarantees.|app/src-tauri/src/main.rs:22129, app/src-tauri/src/main.rs:22136, app/src-tauri/src/main.rs:22149, app/src-tauri/src/main.rs:22154, app/src/pages/GovernanceOracle.tsx:2, app/src/pages/GovernanceOracle.tsx:37, app/src/pages/GovernanceOracle.tsx:48, app/src/pages/GovernanceOracle.tsx:59, app/src/pages/GovernanceOracle.tsx:92, app/src/pages/GovernanceOracle.tsx:93, app/src/pages/GovernanceOracle.tsx:95, app/src/pages/GovernanceOracle.tsx:96, app/src/pages/GovernanceOracle.tsx:97|
|CRITICAL|World Simulation AGENT LAB Page Cannot Run Simulations|The `world-sim` page only fetches policy and history. The submit/run/result/risk/branch wrappers exist but are unused by any page, so the demo path for the new world-simulation crate is read-only.|app/src/pages/WorldSimulation2.tsx:2, app/src/pages/WorldSimulation2.tsx:52, app/src/pages/WorldSimulation2.tsx:59, app/src/pages/WorldSimulation2.tsx:72, app/src/api/backend.ts:3578, app/src/api/backend.ts:3585, app/src/api/backend.ts:3589, app/src/api/backend.ts:3601, app/src/api/backend.ts:3605|

## MAJOR FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|MAJOR|Measurement Session Is Routed But Unreachable|The measurement-session route exists, but there is no sidebar item and no page-to-page navigation reference to it.|app/src/App.tsx:141, app/src/App.tsx:1712, app/src/App.tsx:1713, app/src/pages/MeasurementSession.tsx:98|
|MAJOR|Computer Control Ships Visible Demo Mode|The primary Computer Control page includes a visible demo-mode toggle and canned `DEMO_ACTIONS` script instead of a backend-driven dry run.|app/src/pages/ComputerControl.tsx:46, app/src/pages/ComputerControl.tsx:202, app/src/pages/ComputerControl.tsx:281, app/src/pages/ComputerControl.tsx:283, app/src/pages/ComputerControl.tsx:287, app/src/pages/ComputerControl.tsx:291, app/src/pages/ComputerControl.tsx:294, app/src/pages/ComputerControl.tsx:305|
|MAJOR|Browser-Mode Demo Fallbacks Still Populate Core UI|App-level demo data still fills agents, chat responses, runtime status, and setup wizard flows whenever the desktop backend is absent.|app/src/App.tsx:374, app/src/App.tsx:385, app/src/App.tsx:468, app/src/App.tsx:606, app/src/App.tsx:807, app/src/App.tsx:1989, app/src/App.tsx:2001, app/src/App.tsx:2013|
|MAJOR|15 Backend-Driven Pages Have No Loading State|These pages perform backend work but expose no explicit loading indicator, increasing the chance of a frozen-looking demo.|app/src/pages/Firewall.tsx:1, app/src/pages/Firewall.tsx:2, app/src/pages/Firewall.tsx:10; app/src/pages/Messaging.tsx:1, app/src/pages/Messaging.tsx:11, app/src/pages/Messaging.tsx:90, app/src/pages/Messaging.tsx:94, app/src/pages/Messaging.tsx:106, app/src/pages/Messaging.tsx:153; app/src/pages/NotesApp.tsx:1, app/src/pages/NotesApp.tsx:3, app/src/pages/NotesApp.tsx:6, app/src/pages/NotesApp.tsx:159, app/src/pages/NotesApp.tsx:166, app/src/pages/NotesApp.tsx:231; app/src/pages/TemporalEngine.tsx:1, app/src/pages/TemporalEngine.tsx:2, app/src/pages/TemporalEngine.tsx:7, app/src/pages/TemporalEngine.tsx:112, app/src/pages/TemporalEngine.tsx:113, app/src/pages/TemporalEngine.tsx:122, ...|
|MAJOR|A/B Validation Falls Back To Placeholder Agent IDs|The A/B validation flow triggered by the page passes an empty agent list and the backend substitutes `prebuilt-*` placeholder IDs when no live agents are found.|app/src/pages/ABValidation.tsx:43, app/src-tauri/src/main.rs:21936, app/src-tauri/src/main.rs:21949, app/src-tauri/src/main.rs:21952, app/src-tauri/src/main.rs:21953, app/src-tauri/src/main.rs:21954|

## MINOR FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|MINOR|Integration Tests Fail|`cargo test -p nexus-integration` does not compile because `tests/integration` includes `app/src-tauri/src/main.rs` and hits an inner-attribute error at line 1.|tests/integration/../../app/src-tauri/src/main.rs:1, audit_artifacts/backend_health/test/nexus-integration.log|
|MINOR|Clippy Fails In Two Crates|`cargo clippy -D warnings` fails for `nexus-conductor-benchmark` and `nexus-desktop-backend`.|benchmarks/conductor-bench/src/cloud_models_bench.rs:220, benchmarks/conductor-bench/src/inference_consistency_bench.rs:433, benchmarks/conductor-bench/src/cloud_models_bench.rs:813, app/src-tauri/src/main.rs:15788, audit_artifacts/backend_health/clippy/nexus-conductor-benchmark.log, audit_artifacts/backend_health/clippy/nexus-desktop-backend.log|
|MINOR|Page-Level Tauri Calls Bypass backend.ts|Five direct `invoke(...)` calls bypass the centralized TypeScript bindings even though equivalent wrappers exist.|app/src/pages/TemporalEngine.tsx:143, app/src/pages/CodeEditor.tsx:276, app/src/pages/CodeEditor.tsx:304, app/src/pages/CodeEditor.tsx:384, app/src/pages/CodeEditor.tsx:411|
|MINOR|Backend Surface Area Outruns Page Usage|152 exported backend wrappers are never referenced from page components, leaving a large amount of dormant UI wiring.|app/src/api/backend.ts:102, app/src/api/backend.ts:174, app/src/api/backend.ts:617, app/src/api/backend.ts:1654, app/src/api/backend.ts:3328, app/src/api/backend.ts:3885|
|MINOR|Dead-Code Heuristic Found 62 Unused Public Functions|The repo still contains a large set of public functions with no Rust/TS callers under the audit heuristic.|crates/nexus-software-factory/src/roles.rs:24, crates/nexus-perception/src/engine.rs:241, crates/nexus-capability-measurement/src/tauri_commands.rs:467, crates/nexus-browser-agent/src/session.rs:128, crates/nexus-computer-control/src/engine.rs:84|
|MINOR|Zero-Test Crates|Four crates currently report zero Rust tests in the per-crate test pass.|audit_artifacts/backend_health/test_summary.tsv|

## INFO
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|INFO|All 58 Crates Compile|`cargo check -p <crate>` succeeded for every workspace package.|audit_artifacts/backend_health/check_summary.tsv|
|INFO|All 619 Tauri Commands Are Registered|Every `#[tauri::command]` found under `app/src-tauri/src` is present in `generate_handler![]`.|app/src-tauri/src/main.rs:27176|
|INFO|All 619 Tauri Commands Have A String Binding In backend.ts|A string-level comparison found every registered command name somewhere in `app/src/api/backend.ts`.|app/src/api/backend.ts|
|INFO|Strict No-Caller Scan Found No Registered Command Without Some Frontend Reference|The user-script-style grep returned no registered Rust command with zero frontend references.|audit_artifacts/static/unused_tauri_commands.txt|
|INFO|Frontend Typecheck And Production Build Pass|`npm run lint` and `npm run build` both succeeded in `app/`.|audit_artifacts/frontend_build/lint.log, audit_artifacts/frontend_build/build.log|
|INFO|Security Scan Found No Hardcoded Secrets Or Committed .env Files|No matching secrets, `.env` files, or recent `.env` additions were found.|audit_artifacts/data_security/security_audit.txt|
|INFO|Validation Data Exists|Validation artifacts, prebuilt agent manifests, and measurement battery data are present and non-empty.|data/validation_runs/real-battery-baseline.json, agents/prebuilt, crates/nexus-capability-measurement/data/battery_v1.json|
|INFO|Orphan-Module Heuristic Produced Only Test False Positives|The only `ORPHAN:` hits were standalone integration-test files under `crates/nexus-flash-infer/tests/`.|crates/nexus-flash-infer/tests/autoconfig_test.rs, crates/nexus-flash-infer/tests/registry_test.rs|

## WORKSPACE CRATE HEALTH
|Crate|Check|Check Warns|Clippy|Tests|Test Count|Notes|
|---|---|---|---|---|---|---|
|coder-agent|PASS|0|PASS|PASS|42|-|
|coding-agent|PASS|0|PASS|PASS|5|-|
|designer-agent|PASS|0|PASS|PASS|3|-|
|nexus-adaptation|PASS|0|PASS|PASS|23|-|
|nexus-agent-memory|PASS|0|PASS|PASS|21|-|
|nexus-airgap|PASS|0|PASS|PASS|15|-|
|nexus-analytics|PASS|0|PASS|PASS|5|-|
|nexus-auth|PASS|0|PASS|PASS|32|-|
|nexus-benchmarks|PASS|0|PASS|PASS|0|zero tests|
|nexus-browser-agent|PASS|0|PASS|PASS|12|-|
|nexus-capability-measurement|PASS|0|PASS|PASS|76|-|
|nexus-cli|PASS|0|PASS|PASS|103|-|
|nexus-cloud|PASS|0|PASS|PASS|22|-|
|nexus-collab-protocol|PASS|0|PASS|PASS|18|-|
|nexus-collaboration|PASS|0|PASS|PASS|22|-|
|nexus-computer-control|PASS|0|PASS|PASS|16|-|
|nexus-conductor|PASS|0|PASS|PASS|28|-|
|nexus-conductor-benchmark|PASS|17|FAIL|PASS|0|17 cargo-check warnings; clippy failure; zero tests|
|nexus-connectors-core|PASS|0|PASS|PASS|8|-|
|nexus-connectors-llm|PASS|0|PASS|PASS|290|-|
|nexus-connectors-messaging|PASS|0|PASS|PASS|45|-|
|nexus-connectors-social|PASS|0|PASS|PASS|0|zero tests|
|nexus-connectors-web|PASS|0|PASS|PASS|8|-|
|nexus-content|PASS|0|PASS|PASS|3|-|
|nexus-control|PASS|0|PASS|PASS|15|-|
|nexus-desktop-backend|PASS|2|FAIL|PASS|90|2 cargo-check warnings; clippy failure|
|nexus-distributed|PASS|0|PASS|PASS|179|-|
|nexus-enterprise|PASS|0|PASS|PASS|21|-|
|nexus-external-tools|PASS|0|PASS|PASS|17|-|
|nexus-factory|PASS|0|PASS|PASS|30|-|
|nexus-flash-infer|PASS|0|PASS|PASS|54|-|
|nexus-governance-engine|PASS|0|PASS|PASS|9|-|
|nexus-governance-evolution|PASS|0|PASS|PASS|7|-|
|nexus-governance-oracle|PASS|0|PASS|PASS|12|-|
|nexus-integration|PASS|0|PASS|FAIL|UNKNOWN|test failure|
|nexus-integrations|PASS|0|PASS|PASS|42|-|
|nexus-kernel|PASS|0|PASS|PASS|2015|-|
|nexus-llama-bridge|PASS|0|PASS|PASS|31|-|
|nexus-marketplace|PASS|0|PASS|PASS|84|-|
|nexus-metering|PASS|0|PASS|PASS|18|-|
|nexus-perception|PASS|0|PASS|PASS|19|-|
|nexus-persistence|PASS|0|PASS|PASS|58|-|
|nexus-predictive-router|PASS|0|PASS|PASS|14|-|
|nexus-protocols|PASS|0|PASS|PASS|92|-|
|nexus-research|PASS|0|PASS|PASS|12|-|
|nexus-sdk|PASS|0|PASS|PASS|217|-|
|nexus-self-update|PASS|0|PASS|PASS|9|-|
|nexus-software-factory|PASS|0|PASS|PASS|18|-|
|nexus-telemetry|PASS|0|PASS|PASS|21|-|
|nexus-tenancy|PASS|0|PASS|PASS|50|-|
|nexus-token-economy|PASS|0|PASS|PASS|29|-|
|nexus-workflows|PASS|0|PASS|PASS|5|-|
|nexus-world-simulation|PASS|0|PASS|PASS|18|-|
|screen-poster-agent|PASS|0|PASS|PASS|6|-|
|self-improve-agent|PASS|0|PASS|PASS|7|-|
|social-poster-agent|PASS|0|PASS|PASS|0|zero tests|
|web-builder-agent|PASS|0|PASS|PASS|12|-|
|workflow-studio-agent|PASS|0|PASS|PASS|4|-|

### Specific Failures / Warnings
- `nexus-conductor-benchmark`: cargo check warning hotspots at `benchmarks/conductor-bench/src/inference_consistency_bench.rs:62`, `benchmarks/conductor-bench/src/inference_consistency_bench.rs:278`, `benchmarks/conductor-bench/src/nim_cloud_bench.rs:673`, `benchmarks/conductor-bench/src/cloud_models_bench.rs:220`, `benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:327`; clippy failures logged in `audit_artifacts/backend_health/clippy/nexus-conductor-benchmark.log`.
- `nexus-desktop-backend`: cargo check warnings plus a clippy failure because `simulation_mock_response` is dead code at `app/src-tauri/src/main.rs:15788`.
- `nexus-integration`: test compilation fails because `tests/integration` includes `app/src-tauri/src/main.rs` and hits the inner-attribute error at `tests/integration/../../app/src-tauri/src/main.rs:1`.
- Zero-test crates: `nexus-benchmarks`, `nexus-conductor-benchmark`, `nexus-connectors-social`, `social-poster-agent`.

## NEW CRATE WIRING
|Crate|Workspace|App Dep|Integration Dep|AppState/main.rs Ref|Rust Tests|Compiles|
|---|---|---|---|---|---|---|
|nexus-capability-measurement|yes|yes|yes|yes|76|yes|
|nexus-governance-oracle|yes|yes|yes|partial|12|yes|
|nexus-governance-engine|yes|yes|yes|no|9|yes|
|nexus-governance-evolution|yes|yes|yes|no|7|yes|
|nexus-predictive-router|yes|yes|yes|yes|14|yes|
|nexus-token-economy|yes|yes|yes|yes|29|yes|
|nexus-browser-agent|yes|yes|yes|yes|12|yes|
|nexus-computer-control|yes|yes|yes|yes|16|yes|
|nexus-world-simulation|yes|yes|yes|yes|18|yes|
|nexus-perception|yes|yes|yes|yes|19|yes|
|nexus-agent-memory|yes|yes|yes|yes|21|yes|
|nexus-external-tools|yes|yes|yes|yes|17|yes|
|nexus-collab-protocol|yes|yes|yes|yes|18|yes|
|nexus-software-factory|yes|yes|yes|yes|18|yes|

Notes:
- `nexus-governance-engine` and `nexus-governance-evolution` are the only new crates with `AppState/main.rs Ref = no`; `rg -n "nexus_governance_engine|nexus_governance_evolution" app/src-tauri/src/main.rs` returned no matches.
- `nexus-governance-oracle` is only partially wired: the page exists and wrapper functions exist, but the Tauri command layer is stubbed and there is no long-lived oracle state in `AppState`.

## PER-PAGE STATUS
|Page|File|Imported|Route|Nav|Backend Calls|Mock/Demo Hits|Interactive|Loading|Route Ref|Notes|
|---|---|---|---|---|---|---|---|---|---|---|
|ABValidation|app/src/pages/ABValidation.tsx|yes|ab-validation|yes|1|0|2|yes|app/src/App.tsx:1727|-|
|AdminCompliance|app/src/pages/AdminCompliance.tsx|yes|admin-compliance|yes|4|0|3|yes|app/src/App.tsx:1796|-|
|AdminDashboard|app/src/pages/AdminDashboard.tsx|yes|admin-console|yes|3|0|1|yes|app/src/App.tsx:1784|-|
|AdminFleet|app/src/pages/AdminFleet.tsx|yes|admin-fleet|yes|3|0|3|yes|app/src/App.tsx:1790|-|
|AdminPolicyEditor|app/src/pages/AdminPolicyEditor.tsx|yes|admin-policies|yes|3|0|3|yes|app/src/App.tsx:1793|-|
|AdminSystemHealth|app/src/pages/AdminSystemHealth.tsx|yes|admin-health|yes|5|0|6|yes|app/src/App.tsx:1799|-|
|AdminUsers|app/src/pages/AdminUsers.tsx|yes|admin-users|yes|3|0|4|yes|app/src/App.tsx:1787|-|
|AgentBrowser|app/src/pages/AgentBrowser.tsx|yes|browser|yes|4|1|6|yes|app/src/App.tsx:1691|-|
|AgentDnaLab|app/src/pages/AgentDnaLab.tsx|yes|dna-lab|yes|10|0|24|yes|app/src/App.tsx:1706|-|
|AgentMemory|app/src/pages/AgentMemory.tsx|yes|agent-memory|yes|4|0|9|yes|app/src/App.tsx:1745|-|
|Agents|app/src/pages/Agents.tsx|yes|agents|yes|9|0|33|yes|app/src/App.tsx:1508|-|
|AiChatHub|app/src/pages/AiChatHub.tsx|yes|ai-chat-hub|yes|38|1|53|yes|app/src/App.tsx:1673|-|
|ApiClient|app/src/pages/ApiClient.tsx|yes|api-client|yes|14|1|20|yes|app/src/App.tsx:1655|-|
|ApprovalCenter|app/src/pages/ApprovalCenter.tsx|yes|approvals|yes|11|0|8|yes|app/src/App.tsx:1688|-|
|AppStore|app/src/pages/AppStore.tsx|yes|marketplace/marketplace-browser/app-store|yes|3|0|10|yes|app/src/App.tsx:1670|-|
|Audit|app/src/pages/Audit.tsx|yes|audit|yes|12|1|42|yes|app/src/App.tsx:1586|-|
|AuditTimeline|app/src/pages/AuditTimeline.tsx|yes|audit-timeline|yes|5|0|3|no|app/src/App.tsx:1595|-|
|BrowserAgent|app/src/pages/BrowserAgent.tsx|yes|browser-agent|yes|3|0|6|yes|app/src/App.tsx:1730|-|
|CapabilityBoundaryMap|app/src/pages/CapabilityBoundaryMap.tsx|yes|capability-boundaries|yes|3|0|1|yes|app/src/App.tsx:1721|-|
|Chat|app/src/pages/Chat.tsx|yes|chat|yes|5|1|10|yes|app/src/App.tsx:1481|-|
|Civilization|app/src/pages/Civilization.tsx|yes|civilization|yes|28|0|31|yes|app/src/App.tsx:1778|-|
|ClusterStatus|app/src/pages/ClusterStatus.tsx|yes|cluster|yes|3|0|6|yes|app/src/App.tsx:1604|-|
|CodeEditor|app/src/pages/CodeEditor.tsx|yes|code-editor|yes|17|0|35|yes|app/src/App.tsx:1619|-|
|Collaboration|app/src/pages/Collaboration.tsx|yes|collab-protocol|yes|3|0|10|no|app/src/App.tsx:1751|-|
|CommandCenter|app/src/pages/CommandCenter.tsx|yes|command-center|yes|3|0|6|yes|app/src/App.tsx:1592|-|
|commandCenterUi|app/src/pages/commandCenterUi.tsx|no|none|no|0|0|2|no|-|utility module in pages/; not a route page|
|ComplianceDashboard|app/src/pages/ComplianceDashboard.tsx|yes|compliance|yes|8|0|13|yes|app/src/App.tsx:1601|-|
|ComputerControl|app/src/pages/ComputerControl.tsx|yes|computer-control|yes|6|8|22|yes|app/src/App.tsx:1694|-|
|ConsciousnessMonitor|app/src/pages/ConsciousnessMonitor.tsx|yes|consciousness|yes|8|0|2|yes|app/src/App.tsx:1769|-|
|Dashboard|app/src/pages/Dashboard.tsx|yes|dashboard|yes|3|0|2|yes|app/src/App.tsx:1478|-|
|DatabaseManager|app/src/pages/DatabaseManager.tsx|yes|database|yes|2|0|17|no|app/src/App.tsx:1652|-|
|DeployPipeline|app/src/pages/DeployPipeline.tsx|yes|deploy-pipeline|yes|5|0|21|yes|app/src/App.tsx:1679|-|
|DesignStudio|app/src/pages/DesignStudio.tsx|yes|design-studio|yes|4|0|8|yes|app/src/App.tsx:1658|-|
|DeveloperPortal|app/src/pages/DeveloperPortal.tsx|yes|developer-portal|yes|5|0|4|yes|app/src/App.tsx:1598|-|
|DistributedAudit|app/src/pages/DistributedAudit.tsx|yes|distributed-audit|yes|4|0|0|yes|app/src/App.tsx:1613|-|
|Documents|app/src/pages/Documents.tsx|yes|documents|yes|7|0|11|yes|app/src/App.tsx:1631|-|
|DreamForge|app/src/pages/DreamForge.tsx|yes|dreams|yes|4|0|3|no|app/src/App.tsx:1772|-|
|EmailClient|app/src/pages/EmailClient.tsx|yes|email-client|yes|4|0|18|yes|app/src/App.tsx:1661|-|
|ExternalTools|app/src/pages/ExternalTools.tsx|yes|external-tools|yes|3|0|6|yes|app/src/App.tsx:1748|-|
|FileManager|app/src/pages/FileManager.tsx|yes|file-manager|yes|4|0|24|yes|app/src/App.tsx:1625|-|
|Firewall|app/src/pages/Firewall.tsx|yes|firewall|yes|3|0|2|no|app/src/App.tsx:1700|-|
|FlashInference|app/src/pages/FlashInference.tsx|yes|flash-inference|yes|11|0|12|yes|app/src/App.tsx:1637|-|
|GovernanceOracle|app/src/pages/GovernanceOracle.tsx|yes|governance-oracle|yes|3|0|1|yes|app/src/App.tsx:1733|status/budget only; no token verification UI|
|GovernedControl|app/src/pages/GovernedControl.tsx|yes|governed-control|yes|4|0|0|yes|app/src/App.tsx:1739|read-only control dashboard|
|Identity|app/src/pages/Identity.tsx|yes|identity|yes|12|0|14|yes|app/src/App.tsx:1616|-|
|ImmuneDashboard|app/src/pages/ImmuneDashboard.tsx|yes|immune-dashboard|yes|6|0|4|yes|app/src/App.tsx:1766|-|
|Integrations|app/src/pages/Integrations.tsx|yes|integrations|yes|5|0|16|yes|app/src/App.tsx:1802|-|
|KnowledgeGraph|app/src/pages/KnowledgeGraph.tsx|yes|knowledge-graph|yes|8|0|14|yes|app/src/App.tsx:1763|-|
|LearningCenter|app/src/pages/LearningCenter.tsx|yes|learning-center|yes|24|0|13|yes|app/src/App.tsx:1685|-|
|Login|app/src/pages/Login.tsx|yes|login|yes|3|0|2|yes|app/src/App.tsx:1805|-|
|MeasurementBatteries|app/src/pages/MeasurementBatteries.tsx|yes|measurement-batteries|yes|3|0|1|yes|app/src/App.tsx:1718|-|
|MeasurementCompare|app/src/pages/MeasurementCompare.tsx|yes|measurement-compare|yes|3|0|2|yes|app/src/App.tsx:1715|-|
|MeasurementDashboard|app/src/pages/MeasurementDashboard.tsx|yes|measurement|yes|3|0|6|yes|app/src/App.tsx:1709|-|
|MeasurementSession|app/src/pages/MeasurementSession.tsx|yes|measurement-session|no|4|0|1|yes|app/src/App.tsx:1712|routed but no App.tsx nav entry or page-to-page link found|
|MediaStudio|app/src/pages/MediaStudio.tsx|yes|media-studio|yes|4|0|8|no|app/src/App.tsx:1667|-|
|Messaging|app/src/pages/Messaging.tsx|yes|messaging|yes|6|0|7|no|app/src/App.tsx:1664|-|
|MissionControl|app/src/pages/MissionControl.tsx|yes|mission-control|yes|5|0|14|yes|app/src/App.tsx:1703|-|
|ModelHub|app/src/pages/ModelHub.tsx|yes|model-hub|yes|7|0|21|yes|app/src/App.tsx:1634|-|
|ModelRouting|app/src/pages/ModelRouting.tsx|yes|model-routing|yes|3|0|1|yes|app/src/App.tsx:1724|-|
|NotesApp|app/src/pages/NotesApp.tsx|yes|notes|yes|8|0|22|no|app/src/App.tsx:1646|-|
|Perception|app/src/pages/Perception.tsx|yes|perception|yes|1|0|4|yes|app/src/App.tsx:1742|-|
|PermissionDashboard|app/src/pages/PermissionDashboard.tsx|yes|permissions|yes|4|0|17|yes|app/src/App.tsx:1536|-|
|PolicyManagement|app/src/pages/PolicyManagement.tsx|yes|policy-management|yes|3|0|8|yes|app/src/App.tsx:1697|-|
|ProjectManager|app/src/pages/ProjectManager.tsx|yes|project-manager|yes|4|0|13|yes|app/src/App.tsx:1649|-|
|Protocols|app/src/pages/Protocols.tsx|yes|protocols|yes|3|0|19|yes|app/src/App.tsx:1610|-|
|Scheduler|app/src/pages/Scheduler.tsx|yes|scheduler|yes|3|0|8|yes|app/src/App.tsx:1682|-|
|SelfRewriteLab|app/src/pages/SelfRewriteLab.tsx|yes|self-rewrite|yes|8|0|8|no|app/src/App.tsx:1781|-|
|Settings|app/src/pages/Settings.tsx|yes|settings (default fallback)|yes|36|2|29|yes|app/src/App.tsx:1817|-|
|SetupWizard|app/src/pages/SetupWizard.tsx|yes|modal overlay|no|4|1|31|yes|app/src/App.tsx:1987|-|
|SoftwareFactory|app/src/pages/SoftwareFactory.tsx|yes|software-factory|yes|3|0|4|no|app/src/App.tsx:1754|-|
|SystemMonitor|app/src/pages/SystemMonitor.tsx|yes|system-monitor|yes|4|0|3|no|app/src/App.tsx:1628|-|
|Telemetry|app/src/pages/Telemetry.tsx|yes|telemetry|yes|5|3|9|yes|app/src/App.tsx:1811|-|
|TemporalEngine|app/src/pages/TemporalEngine.tsx|yes|temporal|yes|7|0|8|no|app/src/App.tsx:1775|-|
|Terminal|app/src/pages/Terminal.tsx|yes|terminal|yes|10|0|14|no|app/src/App.tsx:1622|-|
|TimelineViewer|app/src/pages/TimelineViewer.tsx|yes|timeline-viewer|yes|3|0|3|no|app/src/App.tsx:1760|-|
|TimeMachine|app/src/pages/TimeMachine.tsx|yes|time-machine|yes|8|0|28|yes|app/src/App.tsx:1640|-|
|TokenEconomy|app/src/pages/TokenEconomy.tsx|yes|token-economy|yes|5|0|6|yes|app/src/App.tsx:1736|-|
|TrustDashboard|app/src/pages/TrustDashboard.tsx|yes|trust|yes|5|1|12|yes|app/src/App.tsx:1607|-|
|UsageBilling|app/src/pages/UsageBilling.tsx|yes|usage-billing|yes|3|0|6|yes|app/src/App.tsx:1814|-|
|VoiceAssistant|app/src/pages/VoiceAssistant.tsx|yes|voice-assistant|yes|10|1|9|yes|app/src/App.tsx:1676|-|
|Workflows|app/src/pages/Workflows.tsx|yes|workflows|yes|3|0|10|yes|app/src/App.tsx:1589|-|
|Workspaces|app/src/pages/Workspaces.tsx|yes|workspaces|yes|4|0|25|yes|app/src/App.tsx:1808|-|
|WorldSimulation|app/src/pages/WorldSimulation.tsx|yes|simulation|yes|9|0|15|no|app/src/App.tsx:1643|-|
|WorldSimulation2|app/src/pages/WorldSimulation2.tsx|yes|world-sim|yes|4|0|0|yes|app/src/App.tsx:1757|read-only AGENT LAB world-sim page|

## UNWIRED COMMANDS
### Registered Rust Commands With No Frontend Caller
None. The user-script-style `generate_handler![]` vs `app/src/` grep returned no registered command with zero frontend references.

### Registered Rust Commands Missing A backend.ts String Binding
None. Every registered command name appears somewhere in `app/src/api/backend.ts`.

### Page-Level Tauri Calls That Bypass backend.ts
|Page|Command|Direct Invoke|Existing Wrapper|
|---|---|---|---|
|TemporalEngine|temporal_select_fork|app/src/pages/TemporalEngine.tsx:143|app/src/api/backend.ts:2940|
|CodeEditor|file_manager_list|app/src/pages/CodeEditor.tsx:276|app/src/api/backend.ts:1060|
|CodeEditor|file_manager_home|app/src/pages/CodeEditor.tsx:304|app/src/api/backend.ts:2925|
|CodeEditor|file_manager_read|app/src/pages/CodeEditor.tsx:384|app/src/api/backend.ts:1068|
|CodeEditor|file_manager_write|app/src/pages/CodeEditor.tsx:411|app/src/api/backend.ts:1064|

### Backend Wrappers Never Called From Pages (152)
|Wrapper|Definition|
|---|---|
|emailSearchMessages|app/src/api/backend.ts:1006|
|clearAllAgents|app/src/api/backend.ts:102|
|getAgentOutputs|app/src/api/backend.ts:1042|
|projectGet|app/src/api/backend.ts:1052|
|createAgent|app/src/api/backend.ts:106|
|assignAgentGoal|app/src/api/backend.ts:1128|
|stopAgentGoal|app/src/api/backend.ts:1159|
|startAutonomousLoop|app/src/api/backend.ts:1168|
|stopAutonomousLoop|app/src/api/backend.ts:1184|
|getAgentCognitiveStatus|app/src/api/backend.ts:1191|
|getAgentMemories|app/src/api/backend.ts:1211|
|agentMemoryRemember|app/src/api/backend.ts:1227|
|agentMemoryRecall|app/src/api/backend.ts:1245|
|agentMemoryRecallByType|app/src/api/backend.ts:1259|
|agentMemoryForget|app/src/api/backend.ts:1274|
|agentMemoryGetStats|app/src/api/backend.ts:1283|
|agentMemorySave|app/src/api/backend.ts:1287|
|agentMemoryClear|app/src/api/backend.ts:1291|
|getSelfEvolutionMetrics|app/src/api/backend.ts:1312|
|getSelfEvolutionStrategies|app/src/api/backend.ts:1321|
|triggerCrossAgentLearning|app/src/api/backend.ts:1330|
|getHivemindStatus|app/src/api/backend.ts:1347|
|cancelHivemind|app/src/api/backend.ts:1356|
|getOsFitness|app/src/api/backend.ts:1552|
|getFitnessHistory|app/src/api/backend.ts:1556|
|getRoutingStats|app/src/api/backend.ts:1560|
|getUiAdaptations|app/src/api/backend.ts:1564|
|recordPageVisit|app/src/api/backend.ts:1572|
|recordFeatureUse|app/src/api/backend.ts:1576|
|overrideSecurityBlock|app/src/api/backend.ts:1580|
|getOsImprovementLog|app/src/api/backend.ts:1592|
|getMorningOsBriefing|app/src/api/backend.ts:1596|
|recordRoutingOutcome|app/src/api/backend.ts:1600|
|recordOperationTiming|app/src/api/backend.ts:1613|
|getPerformanceReport|app/src/api/backend.ts:1624|
|getSecurityEvolutionReport|app/src/api/backend.ts:1628|
|recordKnowledgeInteraction|app/src/api/backend.ts:1632|
|getOsDreamStatus|app/src/api/backend.ts:1644|
|setSelfImproveEnabled|app/src/api/backend.ts:1648|
|screenshotAnalyze|app/src/api/backend.ts:1654|
|screenshotGenerateSpec|app/src/api/backend.ts:1658|
|voiceProjectStart|app/src/api/backend.ts:1670|
|voiceProjectStop|app/src/api/backend.ts:1674|
|voiceProjectAddChunk|app/src/api/backend.ts:1678|
|voiceProjectGetStatus|app/src/api/backend.ts:1685|
|voiceProjectGetPrompt|app/src/api/backend.ts:1689|
|voiceProjectUpdateIntent|app/src/api/backend.ts:1693|
|stressGeneratePersonas|app/src/api/backend.ts:1705|
|stressGenerateActions|app/src/api/backend.ts:1709|
|stressEvaluateReport|app/src/api/backend.ts:1713|
|deployGenerateDockerfile|app/src/api/backend.ts:1719|
|deployValidateConfig|app/src/api/backend.ts:1723|
|deployGetCommands|app/src/api/backend.ts:1727|
|evolverRegisterApp|app/src/api/backend.ts:1733|
|evolverUnregisterApp|app/src/api/backend.ts:1737|
|getAgentPerformance|app/src/api/backend.ts:174|
|evolverListApps|app/src/api/backend.ts:1741|
|evolverDetectIssues|app/src/api/backend.ts:1745|
|freelanceGetStatus|app/src/api/backend.ts:1751|
|freelanceStartScanning|app/src/api/backend.ts:1755|
|freelanceStopScanning|app/src/api/backend.ts:1759|
|freelanceEvaluateJob|app/src/api/backend.ts:1763|
|freelanceGetRevenue|app/src/api/backend.ts:1767|
|getAutoEvolutionLog|app/src/api/backend.ts:178|
|getLivePreview|app/src/api/backend.ts:1781|
|publishToMarketplace|app/src/api/backend.ts:1793|
|installFromMarketplace|app/src/api/backend.ts:1797|
|setAutoEvolutionConfig|app/src/api/backend.ts:182|
|forceEvolveAgent|app/src/api/backend.ts:196|
|transcribePushToTalk|app/src/api/backend.ts:208|
|startJarvisMode|app/src/api/backend.ts:212|
|stopJarvisMode|app/src/api/backend.ts:216|
|jarvisStatus|app/src/api/backend.ts:220|
|detectHardware|app/src/api/backend.ts:224|
|checkOllama|app/src/api/backend.ts:228|
|pullOllamaModel|app/src/api/backend.ts:232|
|runSetupWizard|app/src/api/backend.ts:241|
|pullModel|app/src/api/backend.ts:248|
|ensureOllama|app/src/api/backend.ts:257|
|isOllamaInstalled|app/src/api/backend.ts:264|
|deleteModel|app/src/api/backend.ts:268|
|isSetupComplete|app/src/api/backend.ts:277|
|listAvailableModels|app/src/api/backend.ts:281|
|schedulerHistory|app/src/api/backend.ts:3048|
|schedulerRunnerStatus|app/src/api/backend.ts:3059|
|executeTeamWorkflow|app/src/api/backend.ts:3067|
|transferAgentFuel|app/src/api/backend.ts:3082|
|runContentPipeline|app/src/api/backend.ts:3100|
|analyzeScreen|app/src/api/backend.ts:311|
|flashProfileModel|app/src/api/backend.ts:3115|
|flashAutoConfigure|app/src/api/backend.ts:3120|
|flashListSessions|app/src/api/backend.ts:3146|
|flashGetMetrics|app/src/api/backend.ts:3161|
|flashEstimatePerformance|app/src/api/backend.ts:3173|
|flashCatalogRecommend|app/src/api/backend.ts:3180|
|flashCatalogSearch|app/src/api/backend.ts:3185|
|flashDownloadModel|app/src/api/backend.ts:3221|
|flashDownloadMulti|app/src/api/backend.ts:3229|
|flashDeleteLocalModel|app/src/api/backend.ts:3236|
|flashAvailableDiskSpace|app/src/api/backend.ts:3240|
|flashGetModelDir|app/src/api/backend.ts:3244|
|cmGetProfile|app/src/api/backend.ts:3295|
|cmTriggerFeedback|app/src/api/backend.ts:3321|
|cmEvaluateResponse|app/src/api/backend.ts:3328|
|cmExecuteValidationRun|app/src/api/backend.ts:3365|
|cmListValidationRuns|app/src/api/backend.ts:3373|
|cmGetValidationRun|app/src/api/backend.ts:3378|
|computerControlCaptureScreen|app/src/api/backend.ts:338|
|cmThreeWayComparison|app/src/api/backend.ts:3383|
|routerRouteTask|app/src/api/backend.ts:3400|
|routerRecordOutcome|app/src/api/backend.ts:3404|
|computerControlExecuteAction|app/src/api/backend.ts:342|
|oracleVerifyToken|app/src/api/backend.ts:3464|
|tokenGetWallet|app/src/api/backend.ts:3479|
|tokenCreateWallet|app/src/api/backend.ts:3487|
|tokenCalculateSpawn|app/src/api/backend.ts:3521|
|tokenCreateDelegation|app/src/api/backend.ts:3528|
|tokenGetDelegations|app/src/api/backend.ts:3539|
|ccExecuteAction|app/src/api/backend.ts:3549|
|simSubmit|app/src/api/backend.ts:3578|
|simRun|app/src/api/backend.ts:3585|
|simGetResult|app/src/api/backend.ts:3589|
|simGetRisk|app/src/api/backend.ts:3601|
|simBranch|app/src/api/backend.ts:3605|
|memoryGetEntry|app/src/api/backend.ts:3693|
|memoryListAgents|app/src/api/backend.ts:3729|
|toolsListAvailable|app/src/api/backend.ts:3739|
|setAgentModel|app/src/api/backend.ts:386|
|swfSubmitArtifact|app/src/api/backend.ts:3885|
|getSystemInfo|app/src/api/backend.ts:411|
|getAgentIdentity|app/src/api/backend.ts:532|
|listIdentities|app/src/api/backend.ts:536|
|marketplaceInfo|app/src/api/backend.ts:583|
|getBrowserHistory|app/src/api/backend.ts:607|
|getAgentActivity|app/src/api/backend.ts:611|
|startResearch|app/src/api/backend.ts:617|
|researchAgentAction|app/src/api/backend.ts:624|
|completeResearch|app/src/api/backend.ts:640|
|getResearchSession|app/src/api/backend.ts:646|
|listResearchSessions|app/src/api/backend.ts:652|
|startBuild|app/src/api/backend.ts:658|
|buildAppendCode|app/src/api/backend.ts:662|
|buildAddMessage|app/src/api/backend.ts:674|
|completeBuild|app/src/api/backend.ts:688|
|getBuildSession|app/src/api/backend.ts:694|
|getBuildCode|app/src/api/backend.ts:700|
|getBuildPreview|app/src/api/backend.ts:704|
|startLearning|app/src/api/backend.ts:710|
|getKnowledgeBase|app/src/api/backend.ts:714|
|getLearningSession|app/src/api/backend.ts:718|
|learningAgentAction|app/src/api/backend.ts:753|
|getProviderUsageStats|app/src/api/backend.ts:794|

## DEAD CODE
### Unused Public Function Candidates (62)
|Function|Definition|
|---|---|
|suggested_capabilities|crates/nexus-software-factory/src/roles.rs:24|
|get_artifact|crates/nexus-software-factory/src/project.rs:103|
|complete_project|crates/nexus-software-factory/src/factory.rs:232|
|stage_cost|crates/nexus-software-factory/src/economy.rs:10|
|refund_escrow|crates/nexus-token-economy/src/wallet.rs:163|
|record_snapshot|crates/nexus-token-economy/src/supply.rs:35|
|clear_cache|crates/nexus-perception/src/engine.rs:241|
|provider_model_id|crates/nexus-perception/src/engine.rs:245|
|find_clickable_elements|crates/nexus-perception/src/screen.rs:29|
|ask_about_screen|crates/nexus-perception/src/screen.rs:53|
|extract_form_data|crates/nexus-perception/src/extraction.rs:16|
|extract_table_data|crates/nexus-perception/src/extraction.rs:30|
|read_page|crates/nexus-perception/src/document.rs:8|
|extract_table|crates/nexus-perception/src/document.rs:26|
|tools_by_category|crates/nexus-external-tools/src/registry.rs:76|
|filter_by_constraints|crates/nexus-predictive-router/src/cost_optimizer.rs:28|
|max_difficulty|crates/nexus-predictive-router/src/difficulty_estimator.rs:28|
|check_staging|crates/nexus-predictive-router/src/staging.rs:18|
|can_propose|crates/nexus-collab-protocol/src/roles.rs:14|
|complete_session|crates/nexus-collab-protocol/src/protocol.rs:51|
|completed_sessions|crates/nexus-collab-protocol/src/protocol.rs:65|
|active_count|crates/nexus-collab-protocol/src/protocol.rs:69|
|session_cost|crates/nexus-collab-protocol/src/economy.rs:6|
|with_data|crates/nexus-collab-protocol/src/message.rs:68|
|with_reasoning|crates/nexus-collab-protocol/src/message.rs:73|
|with_references|crates/nexus-collab-protocol/src/message.rs:78|
|is_broadcast|crates/nexus-collab-protocol/src/message.rs:83|
|has_changed|crates/nexus-governance-engine/src/versioning.rs:6|
|is_valid_successor|crates/nexus-governance-engine/src/versioning.rs:11|
|default_capabilities|crates/nexus-governance-engine/src/capability_model.rs:23|
|active_count|crates/nexus-world-simulation/src/engine.rs:213|
|restore_fs|crates/nexus-world-simulation/src/sandbox.rs:100|
|execute_validation_run_real|crates/nexus-capability-measurement/src/evaluation/validation_run.rs:237|
|run_batch_evaluation|crates/nexus-capability-measurement/src/tauri_commands.rs:292|
|get_ab_comparison|crates/nexus-capability-measurement/src/tauri_commands.rs:467|
|compute_articulation|crates/nexus-capability-measurement/src/scoring/articulation.rs:72|
|difficulty_description|crates/nexus-capability-measurement/src/battery/difficulty.rs:6|
|level_description|crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:9|
|references_tool_output|crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:22|
|acknowledges_limitations|crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:35|
|level_description|crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:9|
|has_causal_language|crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:24|
|avoids_correlation_trap|crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:40|
|level_description|crates/nexus-capability-measurement/src/vectors/adaptation.rs:9|
|shows_epistemic_honesty|crates/nexus-capability-measurement/src/vectors/adaptation.rs:26|
|distinguishes_source_reliability|crates/nexus-capability-measurement/src/vectors/adaptation.rs:41|
|level_description|crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:9|
|has_explicit_dependencies|crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:24|
|has_rollback_handling|crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:39|
|append_to_chain|crates/nexus-capability-measurement/src/reporting/audit_trail.rs:18|
|profile_summary|crates/nexus-capability-measurement/src/reporting/cross_vector.rs:6|
|detect_anomalies|crates/nexus-capability-measurement/src/reporting/cross_vector.rs:21|
|keyword_count|crates/nexus-agent-memory/src/index.rs:80|
|tag_count|crates/nexus-agent-memory/src/index.rs:88|
|update_importance|crates/nexus-agent-memory/src/store.rs:76|
|request_channel|crates/nexus-governance-oracle/src/submission.rs:7|
|check_balance|crates/nexus-browser-agent/src/economy.rs:13|
|score_browser_task|crates/nexus-browser-agent/src/measurement.rs:16|
|is_running|crates/nexus-browser-agent/src/bridge.rs:150|
|close_agent_sessions|crates/nexus-browser-agent/src/session.rs:128|
|generate_speculative|crates/nexus-flash-infer/src/speculative.rs:116|
|set_subprocess_timeout_ms|crates/nexus-computer-control/src/engine.rs:84|

### Orphan Modules
- No real orphan modules were verified.
- The heuristic scan only flagged standalone integration tests under `crates/nexus-flash-infer/tests/` (`autoconfig_test.rs`, `registry_test.rs`, `profiler_test.rs`, `downloader_test.rs`, `catalog_test.rs`, `budget_test.rs`), which are not library modules and should not be counted as routeable/orphan Rust modules.

## MOCK DATA LOCATIONS
### Verified Mock / Demo Logic
|Severity|Finding|Evidence|
|---|---|---|
|MAJOR|Browser demo fallback data and simulated chat replies|app/src/App.tsx:374, app/src/App.tsx:385, app/src/App.tsx:468, app/src/App.tsx:606, app/src/App.tsx:1320|
|MAJOR|Setup Wizard injects mock hardware/Ollama/model responses outside desktop runtime|app/src/App.tsx:1989, app/src/App.tsx:2001, app/src/App.tsx:2005, app/src/App.tsx:2009, app/src/App.tsx:2013, app/src/App.tsx:2017, app/src/App.tsx:2021, app/src/pages/SetupWizard.tsx:224|
|MAJOR|Computer Control page ships a full canned demo action sequence|app/src/pages/ComputerControl.tsx:46, app/src/pages/ComputerControl.tsx:202, app/src/pages/ComputerControl.tsx:281, app/src/pages/ComputerControl.tsx:283, app/src/pages/ComputerControl.tsx:287, app/src/pages/ComputerControl.tsx:291, app/src/pages/ComputerControl.tsx:294, app/src/pages/ComputerControl.tsx:305|
|MAJOR|A/B validation falls back to placeholder agent IDs when no live agents are discovered|app/src-tauri/src/main.rs:21936, app/src-tauri/src/main.rs:21949, app/src-tauri/src/main.rs:21950, app/src-tauri/src/main.rs:21952, app/src-tauri/src/main.rs:21953, app/src-tauri/src/main.rs:21954|

### Raw Indicator Matches Reviewed As Non-Issues
|Severity|Review Outcome|Evidence|
|---|---|---|
|INFO|Comment explicitly says data is not hardcoded|app/src/pages/AgentBrowser.tsx:75, app/src/pages/ApiClient.tsx:86|
|INFO|"mock" appears in error/runtime labels, not fake payloads|app/src/pages/AiChatHub.tsx:187, app/src/pages/Chat.tsx:136, app/src/pages/Settings.tsx:489, app/src/pages/Settings.tsx:504|
|INFO|String match on "DEMOTE" is a badge label, not demo data|app/src/pages/TrustDashboard.tsx:482|

## MISSING ERROR HANDLING
Strict grep found no page with backend usage and zero `catch/error` tokens. The larger UX problem is missing loading state plus widespread silent/no-op catches.

### Pages With Backend Calls But No Loading State (15)
|Page|File|Backend Call Evidence|Finding|
|---|---|---|---|
|Firewall|app/src/pages/Firewall.tsx|app/src/pages/Firewall.tsx:1, app/src/pages/Firewall.tsx:2, app/src/pages/Firewall.tsx:10|no explicit loading token found|
|Messaging|app/src/pages/Messaging.tsx|app/src/pages/Messaging.tsx:1, app/src/pages/Messaging.tsx:11, app/src/pages/Messaging.tsx:90, app/src/pages/Messaging.tsx:94, app/src/pages/Messaging.tsx:106, app/src/pages/Messaging.tsx:153|no explicit loading token found|
|NotesApp|app/src/pages/NotesApp.tsx|app/src/pages/NotesApp.tsx:1, app/src/pages/NotesApp.tsx:3, app/src/pages/NotesApp.tsx:6, app/src/pages/NotesApp.tsx:159, app/src/pages/NotesApp.tsx:166, app/src/pages/NotesApp.tsx:231|no explicit loading token found|
|TemporalEngine|app/src/pages/TemporalEngine.tsx|app/src/pages/TemporalEngine.tsx:1, app/src/pages/TemporalEngine.tsx:2, app/src/pages/TemporalEngine.tsx:7, app/src/pages/TemporalEngine.tsx:112, app/src/pages/TemporalEngine.tsx:113, app/src/pages/TemporalEngine.tsx:122|no explicit loading token found|
|DreamForge|app/src/pages/DreamForge.tsx|app/src/pages/DreamForge.tsx:1, app/src/pages/DreamForge.tsx:12, app/src/pages/DreamForge.tsx:124, app/src/pages/DreamForge.tsx:131|no explicit loading token found|
|SystemMonitor|app/src/pages/SystemMonitor.tsx|app/src/pages/SystemMonitor.tsx:1, app/src/pages/SystemMonitor.tsx:7, app/src/pages/SystemMonitor.tsx:144, app/src/pages/SystemMonitor.tsx:217|no explicit loading token found|
|SoftwareFactory|app/src/pages/SoftwareFactory.tsx|app/src/pages/SoftwareFactory.tsx:1, app/src/pages/SoftwareFactory.tsx:12, app/src/pages/SoftwareFactory.tsx:83|no explicit loading token found|
|Collaboration|app/src/pages/Collaboration.tsx|app/src/pages/Collaboration.tsx:1, app/src/pages/Collaboration.tsx:15, app/src/pages/Collaboration.tsx:103|no explicit loading token found|
|TimelineViewer|app/src/pages/TimelineViewer.tsx|app/src/pages/TimelineViewer.tsx:1, app/src/pages/TimelineViewer.tsx:2, app/src/pages/TimelineViewer.tsx:86|no explicit loading token found|
|WorldSimulation|app/src/pages/WorldSimulation.tsx|app/src/pages/WorldSimulation.tsx:1, app/src/pages/WorldSimulation.tsx:13, app/src/pages/WorldSimulation.tsx:283, app/src/pages/WorldSimulation.tsx:306, app/src/pages/WorldSimulation.tsx:313, app/src/pages/WorldSimulation.tsx:319|no explicit loading token found|
|DatabaseManager|app/src/pages/DatabaseManager.tsx|app/src/pages/DatabaseManager.tsx:8, app/src/pages/DatabaseManager.tsx:241|no explicit loading token found|
|MediaStudio|app/src/pages/MediaStudio.tsx|app/src/pages/MediaStudio.tsx:1, app/src/pages/MediaStudio.tsx:4, app/src/pages/MediaStudio.tsx:11, app/src/pages/MediaStudio.tsx:69|no explicit loading token found|
|AuditTimeline|app/src/pages/AuditTimeline.tsx|app/src/pages/AuditTimeline.tsx:1, app/src/pages/AuditTimeline.tsx:4, app/src/pages/AuditTimeline.tsx:49, app/src/pages/AuditTimeline.tsx:54, app/src/pages/AuditTimeline.tsx:67|no explicit loading token found|
|SelfRewriteLab|app/src/pages/SelfRewriteLab.tsx|app/src/pages/SelfRewriteLab.tsx:1, app/src/pages/SelfRewriteLab.tsx:2, app/src/pages/SelfRewriteLab.tsx:11, app/src/pages/SelfRewriteLab.tsx:173, app/src/pages/SelfRewriteLab.tsx:219, app/src/pages/SelfRewriteLab.tsx:257|no explicit loading token found|
|Terminal|app/src/pages/Terminal.tsx|app/src/pages/Terminal.tsx:1, app/src/pages/Terminal.tsx:3, app/src/pages/Terminal.tsx:99, app/src/pages/Terminal.tsx:127, app/src/pages/Terminal.tsx:254, app/src/pages/Terminal.tsx:261|no explicit loading token found|

### Silent / Suppressed Catch Examples
|Page|Evidence|Observation|
|---|---|---|
|GovernedControl|app/src/pages/GovernedControl.tsx:78, app/src/pages/GovernedControl.tsx:93, app/src/pages/GovernedControl.tsx:94, app/src/pages/GovernedControl.tsx:95, app/src/pages/GovernedControl.tsx:96|empty catches or `catch(() => null/[])` on every backend read|
|WorldSimulation2|app/src/pages/WorldSimulation2.tsx:58, app/src/pages/WorldSimulation2.tsx:59, app/src/pages/WorldSimulation2.tsx:74|policy/history loads silently degrade to null/[]|
|BrowserAgent|app/src/pages/BrowserAgent.tsx:43, app/src/pages/BrowserAgent.tsx:44, app/src/pages/BrowserAgent.tsx:45, app/src/pages/BrowserAgent.tsx:78, app/src/pages/BrowserAgent.tsx:83|errors are logged to console but not surfaced to the user|
|GovernanceOracle|app/src/pages/GovernanceOracle.tsx:41, app/src/pages/GovernanceOracle.tsx:49|errors only go to `console.error`|
|Collaboration|app/src/pages/Collaboration.tsx:105, app/src/pages/Collaboration.tsx:106, app/src/pages/Collaboration.tsx:107, app/src/pages/Collaboration.tsx:116, app/src/pages/Collaboration.tsx:119, app/src/pages/Collaboration.tsx:122|backend failures are normalized to empty/null state without UI error|
|SoftwareFactory|app/src/pages/SoftwareFactory.tsx:85, app/src/pages/SoftwareFactory.tsx:86, app/src/pages/SoftwareFactory.tsx:87, app/src/pages/SoftwareFactory.tsx:88, app/src/pages/SoftwareFactory.tsx:98, app/src/pages/SoftwareFactory.tsx:101, app/src/pages/SoftwareFactory.tsx:104|project/pipeline fetches silently degrade|

## DATA / CONFIG INTEGRITY
- Validation runs present: `data/validation_runs/real-battery-baseline.json` (54 sessions), `data/validation_runs/run1-pre-bugfix-baseline.json` (54 sessions), `data/validation_runs/run2-post-bugfix.json` (54 sessions).
- Prebuilt agent manifests present: 54 JSON manifests under `agents/prebuilt/`.
- Measurement battery present: `crates/nexus-capability-measurement/data/battery_v1.json` with 20 problems.
- Root-level config audit: `.gitlab-ci.yml` and `Cargo.toml` exist; root `package.json` and `tsconfig.json` do not. Frontend build tooling lives under `app/`, where `app/package.json` and `app/tsconfig.json` both exist and build successfully.
- Environment-variable grep only surfaced standard environment keys (`CARGO_MANIFEST_DIR`, `XDG_DATA_HOME`) not documented in root docs; no clearly project-specific undocumented env contract was found.

## SECURITY FINDINGS
- No hardcoded secrets matched the scan pattern in `crates/` or `app/`.
- No committed `.env` files were found in the repository tree.
- `git log --oneline -5 --diff-filter=A -- "*.env"` returned no recent `.env` additions.

## AUDIT NOTES
- `cargo +nightly udeps` was unavailable, so dead-code reporting uses the requested grep heuristic instead of `cargo udeps`.
- The `pub fn` unused scan is heuristic and can over-report when callers are generated dynamically or live outside Rust/TS text search, but manual spot checks confirmed multiple genuine dead functions.
- The page inventory heuristic initially over-reported a few route misses; the `PER-PAGE STATUS` table above normalizes known false positives (`Chat`, `Agents`, `PermissionDashboard`, `AppStore`, `Settings`, `SetupWizard`).
- The single non-route file under `app/src/pages/` is `app/src/pages/commandCenterUi.tsx`, which is a shared UI helper module imported by many pages.
