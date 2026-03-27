# NEXUS OS COMPLETE AUDIT REPORT
Generated: 2026-03-27T05:08:51+00:00
Commit: `5476bfad66daed8dea44f2f898e0306850caea4e`
Repo root: `/home/nexus/NEXUS/nexus-os`
Audit mode: rerun verification only. No source fixes were applied during this audit.

## SUMMARY
|Metric|Value|
|---|---|
|Total crates|58|
|Crates compiling|58|
|Crates with clippy failures|1|
|Crates with test failures|1|
|Crates with zero tests|4|
|Total Tauri commands|619|
|Commands with todo!/unimplemented!|0|
|Commands with no frontend caller|0|
|Frontend invoke calls missing handler|0|
|Frontend command calls missing backend.ts binding|23|
|Backend wrappers never called from pages|152|
|Total frontend pages|84|
|Pages with raw mock indicators|49|
|Pages with no direct backend calls|2|
|Pages with no loading state|15|
|Buttons with empty handler|0|
|Alert-only handlers|0|
|Dead/unused public functions|55|
|Swallowed/suppressed error handlers|93|
|Orphan modules|0|
|Undocumented env vars|0|
|Hardcoded secrets|0|

## CRITICAL FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|CRITICAL|A/B Validation|Capability measurement A/B validation still uses synthetic adapters and hardcoded response text, so the visible A/B results are not based on live agent output.|app/src-tauri/src/main.rs:21933; app/src-tauri/src/main.rs:21937; crates/nexus-capability-measurement/src/tauri_commands.rs:290; crates/nexus-capability-measurement/src/tauri_commands.rs:394; crates/nexus-capability-measurement/src/tauri_commands.rs:404|
|CRITICAL|Governance Engine Wiring|The `nexus-governance-engine` and `nexus-governance-evolution` crates are present in workspace, app, and integration manifests, but they still have no runtime references or AppState fields in the desktop backend.|Cargo.toml:50; Cargo.toml:51; app/src-tauri/Cargo.toml:38; app/src-tauri/Cargo.toml:39; tests/integration/Cargo.toml:52; tests/integration/Cargo.toml:53; app/src-tauri/src/main.rs:923; app/src-tauri/src/main.rs:933|

## MAJOR FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|MAJOR|World Simulation|Simulation planning still falls back to synthetic responses and mock persona batches when the planner output is unusable or unavailable.|app/src-tauri/src/main.rs:15748; app/src-tauri/src/main.rs:15755; app/src-tauri/src/main.rs:15769; app/src-tauri/src/main.rs:15789; app/src-tauri/src/main.rs:15830; app/src-tauri/src/main.rs:15866|
|MAJOR|Computer Control|Computer control still opens in demo mode, renders canned `DEMO_ACTIONS`, and states that no real actions are taken in demo mode.|app/src/pages/ComputerControl.tsx:46; app/src/pages/ComputerControl.tsx:202; app/src/pages/ComputerControl.tsx:281; app/src/pages/ComputerControl.tsx:305; crates/nexus-computer-control/src/engine.rs:98; crates/nexus-computer-control/src/engine.rs:139|
|MAJOR|Demo Fallbacks|The shell app still ships broad browser/demo fallback data when the desktop backend is absent or unavailable.|app/src/App.tsx:370; app/src/App.tsx:472; app/src/App.tsx:474; app/src/App.tsx:478; app/src/App.tsx:606; app/src/App.tsx:669|
|MAJOR|Async UX|Fifteen backend-driven pages still have no visible loading state, and 93 async error paths are swallowed or reduced to console-only fallbacks.|See `MISSING ERROR HANDLING` section below.|
|MAJOR|Setup Wizard Mock Path|Setup wizard runtime wiring exists, but `App.tsx` still injects mock hardware, Ollama, and model-pull fallbacks whenever the runtime is not desktop.|app/src/App.tsx:1987; app/src/App.tsx:1989; app/src/App.tsx:2001; app/src/App.tsx:2013; app/src/pages/SetupWizard.tsx:224|

## MINOR FINDINGS
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|MINOR|Integration Tests|`cargo test -p nexus-integration` still fails to compile because `tests/integration` imports `app/src-tauri/src/main.rs` and hits an inner-attribute error at the top of that file.|tests/integration/../../app/src-tauri/src/main.rs:1; /tmp/nexus_audit_rerun/test/nexus-integration.log|
|MINOR|Clippy|`cargo clippy -D warnings` still fails for `nexus-conductor-benchmark`.|benchmarks/conductor-bench/src/nim_cloud_bench.rs:673; benchmarks/conductor-bench/src/cloud_models_bench.rs:226; benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:327|
|MINOR|Synthetic Oracle Verification|`oracle_verify_token` now returns timestamps and token IDs, but validity is still determined only by whether the payload string is non-empty, not by real cryptographic verification.|app/src-tauri/src/main.rs:22130; app/src-tauri/src/main.rs:22137; app/src-tauri/src/main.rs:22141|
|MINOR|Direct Tauri Calls|Twenty-three page-level command invocations still bypass `app/src/api/backend.ts` and call Tauri commands directly.|See `UNWIRED COMMANDS` section below.|
|MINOR|Dead Code|55 public functions are still unused by workspace scan, and 152 backend wrappers are never called from page components.|See `DEAD CODE` section below.|
|MINOR|Zero-Test Crates|Four workspace crates still have zero Rust tests in source scans.|nexus-benchmarks; nexus-conductor-benchmark; nexus-connectors-social; social-poster-agent|

## INFO
|Severity|Area|Finding|Evidence|
|---|---|---|---|
|INFO|Compile Health|All 58 workspace crates now complete `cargo check -p <crate>` successfully.|workspace-wide audit|
|INFO|Tauri Registration|All 619 `#[tauri::command]` functions in `app/src-tauri/src` are registered in `generate_handler![]`.|app/src-tauri/src/main.rs|
|INFO|Command Coverage|There are no registered Tauri commands without a frontend caller, and no frontend-invoked commands missing a registered handler.|app/src-tauri/src/main.rs; app/src|
|INFO|Validation Artifacts|The previously broken `real-battery-llm-judge.json` artifact is no longer present in `data/validation_runs`.|data/validation_runs|
|INFO|Security|No hardcoded secrets, committed `.env` files, or undocumented environment variables were found in this rerun.|repo-wide scan|
|INFO|Resolved Browser Bridge|The browser bridge now reads and parses subprocess stdout instead of returning a fixed mock response.|crates/nexus-browser-agent/src/bridge.rs:117|
|INFO|Measurement Session Recovery|`MeasurementSession` now backfills the most recent session when `sessionId` is empty instead of exiting immediately.|app/src/pages/MeasurementSession.tsx:105; app/src/pages/MeasurementSession.tsx:113|

## WORKSPACE CRATE HEALTH
### Compile Warnings
|Severity|Crate|Observed Warning|
|---|---|---|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/inference_consistency_bench.rs:62:5: warning: fields `output_hash` and `token_count` are never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/inference_consistency_bench.rs:278:5: warning: fields `count`, `mean_ms`, and `errors` are never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/inference_consistency_bench.rs:340:5: warning: field `dominant_output` is never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/inference_consistency_bench.rs:467:5: warning: field `dominant_output` is never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:327:5: warning: fields `mean_ms`, `min_ms`, `max_ms`, and `total` are never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:596:9: warning: field `agentic_valid` is never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/nim_cloud_bench.rs:673:5: warning: fields `p99`, `mean`, and `total` are never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/nim_cloud_bench.rs:717:5: warning: fields `det_runs`, `conc_agents`, `avg_tokens_in`, and `avg_tokens_out` are never read|
|MINOR|nexus-conductor-benchmark|benchmarks/conductor-bench/src/nim_cloud_bench.rs:744:5: warning: field `tokens` is never read|
|MINOR|nexus-conductor-benchmark|warning: `nexus-conductor-benchmark` (bin "inference-consistency-bench") generated 4 warnings|

### Clippy Failures
|Severity|Crate|Observed Error|
|---|---|---|
|MINOR|nexus-conductor-benchmark|error: fields `p99`, `mean`, and `total` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/nim_cloud_bench.rs:673:5|
|MINOR|nexus-conductor-benchmark|error: fields `det_runs`, `conc_agents`, `avg_tokens_in`, and `avg_tokens_out` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/nim_cloud_bench.rs:717:5|
|MINOR|nexus-conductor-benchmark|error: field `tokens` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/nim_cloud_bench.rs:744:5|
|MINOR|nexus-conductor-benchmark|error: fields `output_hash` and `token_count` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/inference_consistency_bench.rs:62:5|
|MINOR|nexus-conductor-benchmark|error: fields `count`, `mean_ms`, and `errors` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/inference_consistency_bench.rs:278:5|
|MINOR|nexus-conductor-benchmark|error: field `dominant_output` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/inference_consistency_bench.rs:340:5|
|MINOR|nexus-conductor-benchmark|error: field `dominant_output` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/inference_consistency_bench.rs:467:5|
|MINOR|nexus-conductor-benchmark|error: field `cost_per_token` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/cloud_models_bench.rs:220:5|
|MINOR|nexus-conductor-benchmark|error: fields `output_hash` and `token_count` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/cloud_models_bench.rs:226:5|
|MINOR|nexus-conductor-benchmark|error: fields `count`, `min_ms`, `max_ms`, `mean_ms`, and `errors` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/cloud_models_bench.rs:526:5|
|MINOR|nexus-conductor-benchmark|error: field `dominant_output` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/cloud_models_bench.rs:588:5|
|MINOR|nexus-conductor-benchmark|error: doc list item overindented|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/cloud_models_bench.rs:9:5|
|MINOR|nexus-conductor-benchmark|error: fields `mean_ms`, `min_ms`, `max_ms`, and `total` are never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:327:5|
|MINOR|nexus-conductor-benchmark|error: field `agentic_valid` is never read|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:596:9|
|MINOR|nexus-conductor-benchmark|error: manual implementation of `.is_multiple_of()`|
|MINOR|nexus-conductor-benchmark|--> benchmarks/conductor-bench/src/inference_consistency_bench.rs:433:20|

### Test Failures
|Severity|Crate|Observed Failure|
|---|---|---|
|MINOR|nexus-integration|error: an inner attribute is not permitted in this context|
|MINOR|nexus-integration|error: could not compile `nexus-integration` (lib test) due to 1 previous error; 2 warnings emitted|

### Zero-Test Crates
|Severity|Crate|Source Test Count|
|---|---|---|
|MINOR|nexus-benchmarks|0|
|MINOR|nexus-conductor-benchmark|0|
|MINOR|nexus-connectors-social|0|
|MINOR|social-poster-agent|0|

## TAURI COMMAND AUDIT
|Check|Result|
|---|---|
|`#[tauri::command]` definitions found|619|
|`generate_handler![]` registrations found|619|
|Commands missing from `generate_handler![]`|0|
|Registered commands with no frontend caller|0|
|Frontend-invoked commands missing handler|0|
|Frontend command calls missing backend.ts binding|23|

### Flagged Commands
|Severity|Command|Finding|Evidence|
|---|---|---|---|
|CRITICAL|cm_run_ab_validation|Registered command still produces synthetic A/B results via hardcoded adapters.|app/src-tauri/src/main.rs:21933; crates/nexus-capability-measurement/src/tauri_commands.rs:394; crates/nexus-capability-measurement/src/tauri_commands.rs:404|
|MINOR|oracle_verify_token|Treats any non-empty token payload as valid; no real signature verification is performed.|app/src-tauri/src/main.rs:22130; app/src-tauri/src/main.rs:22137|

## PER-CRATE STATUS
|Crate|cargo check|Warnings|cargo clippy -D warnings|cargo test|Source Tests|Notes|
|---|---|---|---|---|---|---|
|coder-agent|PASS|no|PASS|PASS|45|clean|
|coding-agent|PASS|no|PASS|PASS|5|clean|
|designer-agent|PASS|no|PASS|PASS|3|clean|
|nexus-adaptation|PASS|no|PASS|PASS|23|clean|
|nexus-agent-memory|PASS|no|PASS|PASS|21|clean|
|nexus-airgap|PASS|no|PASS|PASS|15|clean|
|nexus-analytics|PASS|no|PASS|PASS|5|clean|
|nexus-auth|PASS|no|PASS|PASS|32|clean|
|nexus-benchmarks|PASS|no|PASS|PASS|0|zero tests|
|nexus-browser-agent|PASS|no|PASS|PASS|12|clean|
|nexus-capability-measurement|PASS|no|PASS|PASS|76|clean|
|nexus-cli|PASS|no|PASS|PASS|114|clean|
|nexus-cloud|PASS|no|PASS|PASS|22|clean|
|nexus-collab-protocol|PASS|no|PASS|PASS|18|clean|
|nexus-collaboration|PASS|no|PASS|PASS|22|clean|
|nexus-computer-control|PASS|no|PASS|PASS|16|clean|
|nexus-conductor|PASS|no|PASS|PASS|28|clean|
|nexus-conductor-benchmark|PASS|yes|FAIL|PASS|0|check warnings, clippy failed, zero tests|
|nexus-connectors-core|PASS|no|PASS|PASS|8|clean|
|nexus-connectors-llm|PASS|no|PASS|PASS|345|clean|
|nexus-connectors-messaging|PASS|no|PASS|PASS|46|clean|
|nexus-connectors-social|PASS|no|PASS|PASS|0|zero tests|
|nexus-connectors-web|PASS|no|PASS|PASS|8|clean|
|nexus-content|PASS|no|PASS|PASS|3|clean|
|nexus-control|PASS|no|PASS|PASS|15|clean|
|nexus-desktop-backend|PASS|no|PASS|PASS|90|clean|
|nexus-distributed|PASS|no|PASS|PASS|179|clean|
|nexus-enterprise|PASS|no|PASS|PASS|21|clean|
|nexus-external-tools|PASS|no|PASS|PASS|17|clean|
|nexus-factory|PASS|no|PASS|PASS|30|clean|
|nexus-flash-infer|PASS|no|PASS|PASS|76|clean|
|nexus-governance-engine|PASS|no|PASS|PASS|9|clean|
|nexus-governance-evolution|PASS|no|PASS|PASS|7|clean|
|nexus-governance-oracle|PASS|no|PASS|PASS|12|clean|
|nexus-integration|PASS|no|PASS|FAIL|19|tests failed|
|nexus-integrations|PASS|no|PASS|PASS|42|clean|
|nexus-kernel|PASS|no|PASS|PASS|2021|clean|
|nexus-llama-bridge|PASS|no|PASS|PASS|31|clean|
|nexus-marketplace|PASS|no|PASS|PASS|84|clean|
|nexus-metering|PASS|no|PASS|PASS|18|clean|
|nexus-perception|PASS|no|PASS|PASS|19|clean|
|nexus-persistence|PASS|no|PASS|PASS|58|clean|
|nexus-predictive-router|PASS|no|PASS|PASS|14|clean|
|nexus-protocols|PASS|no|PASS|PASS|92|clean|
|nexus-research|PASS|no|PASS|PASS|12|clean|
|nexus-sdk|PASS|no|PASS|PASS|217|clean|
|nexus-self-update|PASS|no|PASS|PASS|10|clean|
|nexus-software-factory|PASS|no|PASS|PASS|18|clean|
|nexus-telemetry|PASS|no|PASS|PASS|21|clean|
|nexus-tenancy|PASS|no|PASS|PASS|50|clean|
|nexus-token-economy|PASS|no|PASS|PASS|29|clean|
|nexus-workflows|PASS|no|PASS|PASS|5|clean|
|nexus-world-simulation|PASS|no|PASS|PASS|18|clean|
|screen-poster-agent|PASS|no|PASS|PASS|6|clean|
|self-improve-agent|PASS|no|PASS|PASS|7|clean|
|social-poster-agent|PASS|no|PASS|PASS|0|zero tests|
|web-builder-agent|PASS|no|PASS|PASS|12|clean|
|workflow-studio-agent|PASS|no|PASS|PASS|4|clean|

## NEW CRATE WIRING
|Crate|Workspace Line|App Dep Line|Integration Dep Line|Tauri Cmds|Tests|Compiles|main.rs Refs|Note|
|---|---|---|---|---|---|---|---|---|
|nexus-capability-measurement|48|36|50|1|76|PASS|103, 21760, 21771, 21772, 21782, 21783, 21792, 21793|runtime refs present|
|nexus-governance-oracle|49|37|51|0|12|PASS|106, 22132, 22138|runtime refs present, but AppState has no dedicated governance-oracle field|
|nexus-governance-engine|50|38|52|0|9|PASS|none|no runtime refs in app/src-tauri/src/main.rs|
|nexus-governance-evolution|51|39|53|0|7|PASS|none|no runtime refs in app/src-tauri/src/main.rs|
|nexus-predictive-router|52|40|54|0|14|PASS|109, 21976, 21977, 21992, 21995, 22007, 22008, 22014|runtime refs present|
|nexus-token-economy|54|42|56|0|29|PASS|115|runtime refs present|
|nexus-browser-agent|53|41|55|0|12|PASS|112, 22044, 22058, 22059, 22073, 22074, 22082, 22083|runtime refs present|
|nexus-computer-control|55|43|57|0|16|PASS|118, 22302, 22303, 22334, 22342|runtime refs present|
|nexus-world-simulation|56|44|58|0|18|PASS|121, 22355, 22364, 22372, 22393, 22405, 22407|runtime refs present|
|nexus-perception|57|45|59|0|19|PASS|124, 22435, 22444, 22454, 22462, 22472, 22480, 22489|runtime refs present|
|nexus-agent-memory|58|46|60|0|21|PASS|127, 22529, 22545, 22564, 22605|runtime refs present|
|nexus-external-tools|59|47|61|0|17|PASS|130, 22614, 22625, 22638, 22645, 22653, 22665|runtime refs present|
|nexus-collab-protocol|60|48|62|0|18|PASS|133, 22788, 22796, 22803, 22810|runtime refs present|
|nexus-software-factory|61|49|63|0|18|PASS|136, 22861, 22869, 22876, 22889|runtime refs present|

## PER-PAGE STATUS
|Page|File|In App.tsx|Direct Backend Hits|Raw Mock Hits|Interactive Elements|Loading|Error Handling|Console Lines|
|---|---|---|---|---|---|---|---|---|
|ABValidation|app/src/pages/ABValidation.tsx|yes|2|0|2|yes|yes|45|
|AdminCompliance|app/src/pages/AdminCompliance.tsx|yes|1|0|3|yes|yes|-|
|AdminDashboard|app/src/pages/AdminDashboard.tsx|yes|1|0|1|yes|yes|-|
|AdminFleet|app/src/pages/AdminFleet.tsx|yes|1|0|3|yes|yes|-|
|AdminPolicyEditor|app/src/pages/AdminPolicyEditor.tsx|yes|1|0|3|yes|yes|-|
|AdminSystemHealth|app/src/pages/AdminSystemHealth.tsx|yes|1|0|6|yes|yes|-|
|AdminUsers|app/src/pages/AdminUsers.tsx|yes|1|1|4|yes|yes|-|
|AgentBrowser|app/src/pages/AgentBrowser.tsx|yes|1|1|6|yes|yes|-|
|AgentDnaLab|app/src/pages/AgentDnaLab.tsx|yes|2|11|24|yes|yes|-|
|AgentMemory|app/src/pages/AgentMemory.tsx|yes|20|5|9|yes|yes|-|
|Agents|app/src/pages/Agents.tsx|yes|2|2|33|yes|yes|227,241|
|AiChatHub|app/src/pages/AiChatHub.tsx|yes|1|7|53|yes|yes|-|
|ApiClient|app/src/pages/ApiClient.tsx|yes|1|15|20|yes|yes|-|
|AppStore|app/src/pages/AppStore.tsx|yes|1|2|10|yes|yes|91,163,177|
|ApprovalCenter|app/src/pages/ApprovalCenter.tsx|yes|2|1|8|yes|yes|-|
|Audit|app/src/pages/Audit.tsx|yes|2|11|42|yes|yes|-|
|AuditTimeline|app/src/pages/AuditTimeline.tsx|yes|1|0|3|no|yes|-|
|BrowserAgent|app/src/pages/BrowserAgent.tsx|yes|18|2|6|yes|yes|43,44,45,78,83|
|CapabilityBoundaryMap|app/src/pages/CapabilityBoundaryMap.tsx|yes|8|0|1|yes|yes|66|
|Chat|app/src/pages/Chat.tsx|yes|1|2|10|yes|yes|-|
|Civilization|app/src/pages/Civilization.tsx|yes|3|28|31|yes|yes|-|
|ClusterStatus|app/src/pages/ClusterStatus.tsx|yes|1|4|6|yes|yes|-|
|CodeEditor|app/src/pages/CodeEditor.tsx|yes|8|4|35|yes|yes|-|
|Collaboration|app/src/pages/Collaboration.tsx|yes|27|6|10|no|yes|-|
|CommandCenter|app/src/pages/CommandCenter.tsx|yes|1|0|6|yes|yes|-|
|ComplianceDashboard|app/src/pages/ComplianceDashboard.tsx|yes|2|0|13|yes|yes|-|
|ComputerControl|app/src/pages/ComputerControl.tsx|yes|1|3|22|yes|yes|-|
|ConsciousnessMonitor|app/src/pages/ConsciousnessMonitor.tsx|yes|2|0|2|yes|yes|-|
|Dashboard|app/src/pages/Dashboard.tsx|yes|1|0|2|yes|yes|-|
|DatabaseManager|app/src/pages/DatabaseManager.tsx|yes|1|4|17|no|yes|278|
|DeployPipeline|app/src/pages/DeployPipeline.tsx|yes|1|7|21|yes|yes|-|
|DesignStudio|app/src/pages/DesignStudio.tsx|yes|1|0|8|yes|yes|-|
|DeveloperPortal|app/src/pages/DeveloperPortal.tsx|yes|4|0|4|yes|yes|-|
|DistributedAudit|app/src/pages/DistributedAudit.tsx|yes|1|0|0|yes|yes|-|
|Documents|app/src/pages/Documents.tsx|yes|1|1|11|yes|yes|-|
|DreamForge|app/src/pages/DreamForge.tsx|yes|1|0|3|no|yes|-|
|EmailClient|app/src/pages/EmailClient.tsx|yes|1|4|18|yes|yes|82,91|
|ExternalTools|app/src/pages/ExternalTools.tsx|yes|14|2|6|yes|yes|-|
|FileManager|app/src/pages/FileManager.tsx|yes|1|3|24|yes|yes|155,176|
|Firewall|app/src/pages/Firewall.tsx|yes|1|0|2|no|yes|-|
|FlashInference|app/src/pages/FlashInference.tsx|yes|3|1|12|yes|yes|630,631|
|GovernanceOracle|app/src/pages/GovernanceOracle.tsx|yes|3|0|1|yes|yes|42,50|
|GovernedControl|app/src/pages/GovernedControl.tsx|yes|9|0|0|yes|yes|-|
|Identity|app/src/pages/Identity.tsx|yes|2|4|14|yes|yes|-|
|ImmuneDashboard|app/src/pages/ImmuneDashboard.tsx|yes|4|0|4|yes|yes|-|
|Integrations|app/src/pages/Integrations.tsx|yes|1|22|16|yes|yes|-|
|KnowledgeGraph|app/src/pages/KnowledgeGraph.tsx|yes|7|12|14|yes|yes|-|
|LearningCenter|app/src/pages/LearningCenter.tsx|yes|7|6|13|yes|yes|675|
|Login|app/src/pages/Login.tsx|yes|1|0|2|yes|yes|-|
|MeasurementBatteries|app/src/pages/MeasurementBatteries.tsx|yes|2|0|1|yes|yes|43|
|MeasurementCompare|app/src/pages/MeasurementCompare.tsx|yes|3|0|2|yes|yes|62|
|MeasurementDashboard|app/src/pages/MeasurementDashboard.tsx|yes|8|0|6|yes|yes|134|
|MeasurementSession|app/src/pages/MeasurementSession.tsx|yes|3|0|1|yes|yes|-|
|MediaStudio|app/src/pages/MediaStudio.tsx|yes|1|0|8|no|yes|-|
|Messaging|app/src/pages/Messaging.tsx|yes|7|3|7|no|yes|-|
|MissionControl|app/src/pages/MissionControl.tsx|yes|1|0|18|yes|yes|-|
|ModelHub|app/src/pages/ModelHub.tsx|yes|1|6|21|yes|yes|212|
|ModelRouting|app/src/pages/ModelRouting.tsx|yes|7|1|1|yes|yes|44,52|
|NotesApp|app/src/pages/NotesApp.tsx|yes|8|3|22|no|yes|-|
|Perception|app/src/pages/Perception.tsx|yes|19|4|4|yes|yes|-|
|PermissionDashboard|app/src/pages/PermissionDashboard.tsx|yes|4|0|17|yes|yes|-|
|PolicyManagement|app/src/pages/PolicyManagement.tsx|yes|1|0|8|yes|yes|-|
|ProjectManager|app/src/pages/ProjectManager.tsx|yes|1|6|13|yes|yes|135|
|Protocols|app/src/pages/Protocols.tsx|yes|3|10|19|yes|yes|-|
|Scheduler|app/src/pages/Scheduler.tsx|yes|1|2|8|yes|yes|-|
|SelfRewriteLab|app/src/pages/SelfRewriteLab.tsx|yes|5|0|8|no|yes|-|
|Settings|app/src/pages/Settings.tsx|yes|3|11|29|yes|yes|-|
|SetupWizard|app/src/pages/SetupWizard.tsx|yes|0|1|31|yes|yes|-|
|SoftwareFactory|app/src/pages/SoftwareFactory.tsx|yes|22|4|4|no|yes|-|
|SystemMonitor|app/src/pages/SystemMonitor.tsx|yes|1|0|3|no|yes|-|
|Telemetry|app/src/pages/Telemetry.tsx|yes|1|9|9|yes|yes|-|
|TemporalEngine|app/src/pages/TemporalEngine.tsx|yes|4|3|8|no|yes|-|
|Terminal|app/src/pages/Terminal.tsx|yes|1|1|14|no|yes|-|
|TimeMachine|app/src/pages/TimeMachine.tsx|yes|1|2|28|yes|yes|-|
|TimelineViewer|app/src/pages/TimelineViewer.tsx|yes|1|0|3|no|yes|-|
|TokenEconomy|app/src/pages/TokenEconomy.tsx|yes|13|1|6|yes|yes|-|
|TrustDashboard|app/src/pages/TrustDashboard.tsx|yes|1|8|12|yes|yes|-|
|UsageBilling|app/src/pages/UsageBilling.tsx|yes|1|1|6|yes|yes|-|
|VoiceAssistant|app/src/pages/VoiceAssistant.tsx|yes|1|15|9|yes|yes|-|
|Workflows|app/src/pages/Workflows.tsx|yes|1|0|10|yes|yes|-|
|Workspaces|app/src/pages/Workspaces.tsx|yes|1|2|25|yes|yes|-|
|WorldSimulation|app/src/pages/WorldSimulation.tsx|yes|1|5|15|no|yes|-|
|WorldSimulation2|app/src/pages/WorldSimulation2.tsx|yes|3|0|0|yes|yes|-|
|commandCenterUi|app/src/pages/commandCenterUi.tsx|no|0|0|4|no|no|-|

## FRONTEND PAGE -> BACKEND -> CRATE TRACE
|Page|File|In App.tsx|Backend Imports|Interactive Elements|Mock Lines|Error Handling|Loading|
|---|---|---|---|---|---|---|---|
|MeasurementDashboard|app/src/pages/MeasurementDashboard.tsx|yes|cmListSessions, cmGetBatteries, cmGetScorecard, cmStartSession, listAgents|6|-|yes|yes|
|MeasurementSession|app/src/pages/MeasurementSession.tsx|yes|cmGetSession, cmGetGamingFlags, cmListSessions|1|-|yes|yes|
|MeasurementCompare|app/src/pages/MeasurementCompare.tsx|yes|cmCompareAgents, cmListSessions|2|-|yes|yes|
|MeasurementBatteries|app/src/pages/MeasurementBatteries.tsx|yes|cmGetBatteries|1|-|yes|yes|
|CapabilityBoundaryMap|app/src/pages/CapabilityBoundaryMap.tsx|yes|cmGetBoundaryMap, cmGetCalibration, cmGetCensus, cmGetGamingReportBatch, cmUploadDarwin|1|-|yes|yes|
|ABValidation|app/src/pages/ABValidation.tsx|yes|cmRunAbValidation|2|-|yes|yes|
|ModelRouting|app/src/pages/ModelRouting.tsx|yes|routerGetAccuracy, routerGetModels, routerGetFeedback, routerEstimateDifficulty|1|91|yes|yes|
|GovernanceOracle|app/src/pages/GovernanceOracle.tsx|yes|oracleStatus, oracleGetAgentBudget, listAgents|1|-|yes|yes|
|TokenEconomy|app/src/pages/TokenEconomy.tsx|yes|tokenGetAllWallets, tokenGetLedger, tokenGetSupply, tokenGetPricing, tokenCalculateReward, tokenCalculateBurn|6|437|yes|yes|
|BrowserAgent|app/src/pages/BrowserAgent.tsx|yes|browserCreateSession, browserExecuteTask, browserNavigate, browserGetContent, browserCloseSession, browserGetPolicy, browserSessionCount, browserScreenshot, listAgents|6|115, 124|yes|yes|
|GovernedControl|app/src/pages/GovernedControl.tsx|yes|ccGetActionHistory, ccGetCapabilityBudget, ccGetScreenContext, ccVerifyActionSequence, listAgents|0|-|yes|yes|
|WorldSimulation|app/src/pages/WorldSimulation.tsx|yes|chatWithSimulationPersona, createSimulation, getSimulationReport, getSimulationStatus, hasDesktopRuntime, injectSimulationVariable, listSimulations, pauseSimulation, runParallelSimulations, startSimulation|15|613, 623, 842, 847, 1126|yes|no|
|Perception|app/src/pages/Perception.tsx|yes|perceptionAnalyzeChart, perceptionDescribe, perceptionExtractData, perceptionExtractText, perceptionFindUiElements, perceptionGetPolicy, perceptionInit, perceptionQuestion, perceptionReadError|4|205, 211, 266, 275|yes|yes|
|AgentMemory|app/src/pages/AgentMemory.tsx|yes|listAgents, memoryBuildContext, memoryConsolidate, memoryDeleteEntry, memoryGetPolicy, memoryGetStats, memoryLoad, memoryQueryEntries, memorySave, memoryStoreEntry|9|200, 206, 208, 230, 243|yes|yes|
|ExternalTools|app/src/pages/ExternalTools.tsx|yes|toolsExecute, toolsGetAudit, toolsGetPolicy, toolsGetRegistry, toolsRefreshAvailability, toolsVerifyAudit, getRateLimitStatus|6|184, 191|yes|yes|
|Collaboration|app/src/pages/Collaboration.tsx|yes|collabAddParticipant, collabCastVote, collabCreateSession, collabDeclareConsensus, collabDetectConsensus, collabGetPatterns, collabGetPolicy, collabGetSession, collabListActive, collabSendMessage, collabStart, collabCallVote|10|193, 194, 198, 264, 309, 324|yes|no|
|SoftwareFactory|app/src/pages/SoftwareFactory.tsx|yes|swfAssignMember, swfCreateProject, swfEstimateCost, swfGetCost, swfGetPipelineStages, swfGetPolicy, swfGetProject, swfListProjects, swfStartPipeline|4|156, 157, 237, 238|yes|no|

## UNWIRED COMMANDS
### Registered Commands With No Frontend Caller
None.

### Frontend Command Calls Missing `app/src/api/backend.ts` Binding
|Severity|Command|Page Call Site|Backend Registration|
|---|---|---|---|
|MINOR|reset_agent_consciousness|app/src/pages/ConsciousnessMonitor.tsx:225|app/src-tauri/src/main.rs:27504|
|MINOR|cogfs_query|app/src/pages/KnowledgeGraph.tsx:403|app/src-tauri/src/main.rs:27525|
|MINOR|cogfs_search|app/src/pages/KnowledgeGraph.tsx:411|app/src-tauri/src/main.rs:27529|
|MINOR|cogfs_watch_directory|app/src/pages/KnowledgeGraph.tsx:450|app/src-tauri/src/main.rs:27527|
|MINOR|cogfs_index_file|app/src/pages/KnowledgeGraph.tsx:453|app/src-tauri/src/main.rs:27524|
|MINOR|cogfs_watch_directory|app/src/pages/KnowledgeGraph.tsx:475|app/src-tauri/src/main.rs:27527|
|MINOR|cogfs_index_file|app/src/pages/KnowledgeGraph.tsx:477|app/src-tauri/src/main.rs:27524|
|MINOR|mesh_add_peer|app/src/pages/Identity.tsx:396|app/src-tauri/src/main.rs:27544|
|MINOR|notes_list|app/src/pages/NotesApp.tsx:181|app/src-tauri/src/main.rs:27431|
|MINOR|notes_save|app/src/pages/NotesApp.tsx:207|app/src-tauri/src/main.rs:27433|
|MINOR|notes_save|app/src/pages/NotesApp.tsx:314|app/src-tauri/src/main.rs:27433|
|MINOR|notes_delete|app/src/pages/NotesApp.tsx:334|app/src-tauri/src/main.rs:27434|
|MINOR|notes_save|app/src/pages/NotesApp.tsx:347|app/src-tauri/src/main.rs:27433|
|MINOR|temporal_fork|app/src/pages/TemporalEngine.tsx:125|app/src-tauri/src/main.rs:27511|
|MINOR|temporal_rollback|app/src/pages/TemporalEngine.tsx:150|app/src-tauri/src/main.rs:27513|
|MINOR|mutate_agent|app/src/pages/AgentDnaLab.tsx:259|app/src-tauri/src/main.rs:27490|
|MINOR|civ_vote|app/src/pages/Civilization.tsx:458|app/src-tauri/src/main.rs:27532|
|MINOR|civ_run_election|app/src/pages/Civilization.tsx:485|app/src-tauri/src/main.rs:27536|
|MINOR|get_git_repo_status|app/src/pages/CodeEditor.tsx:329|app/src-tauri/src/main.rs:27403|
|MINOR|self_rewrite_suggest_patches|app/src/pages/SelfRewriteLab.tsx:179|app/src-tauri/src/main.rs:27550|
|MINOR|self_rewrite_analyze|app/src/pages/SelfRewriteLab.tsx:223|app/src-tauri/src/main.rs:27549|
|MINOR|self_rewrite_apply_patch|app/src/pages/SelfRewriteLab.tsx:301|app/src-tauri/src/main.rs:27553|
|MINOR|self_rewrite_rollback|app/src/pages/SelfRewriteLab.tsx:321|app/src-tauri/src/main.rs:27554|

## DEAD CODE
### Unused Public Functions
|Severity|Function|Definition|
|---|---|---|
|MINOR|keyword_count|crates/nexus-agent-memory/src/index.rs:80|
|MINOR|tag_count|crates/nexus-agent-memory/src/index.rs:88|
|MINOR|update_importance|crates/nexus-agent-memory/src/store.rs:76|
|MINOR|is_running|crates/nexus-browser-agent/src/bridge.rs:150|
|MINOR|check_balance|crates/nexus-browser-agent/src/economy.rs:13|
|MINOR|score_browser_task|crates/nexus-browser-agent/src/measurement.rs:16|
|MINOR|close_agent_sessions|crates/nexus-browser-agent/src/session.rs:128|
|MINOR|difficulty_description|crates/nexus-capability-measurement/src/battery/difficulty.rs:6|
|MINOR|append_to_chain|crates/nexus-capability-measurement/src/reporting/audit_trail.rs:18|
|MINOR|profile_summary|crates/nexus-capability-measurement/src/reporting/cross_vector.rs:6|
|MINOR|detect_anomalies|crates/nexus-capability-measurement/src/reporting/cross_vector.rs:21|
|MINOR|compute_articulation|crates/nexus-capability-measurement/src/scoring/articulation.rs:72|
|MINOR|run_batch_evaluation|crates/nexus-capability-measurement/src/tauri_commands.rs:293|
|MINOR|get_ab_comparison|crates/nexus-capability-measurement/src/tauri_commands.rs:433|
|MINOR|shows_epistemic_honesty|crates/nexus-capability-measurement/src/vectors/adaptation.rs:26|
|MINOR|distinguishes_source_reliability|crates/nexus-capability-measurement/src/vectors/adaptation.rs:41|
|MINOR|has_explicit_dependencies|crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:24|
|MINOR|has_rollback_handling|crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:39|
|MINOR|has_causal_language|crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:24|
|MINOR|avoids_correlation_trap|crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:40|
|MINOR|references_tool_output|crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:22|
|MINOR|acknowledges_limitations|crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:35|
|MINOR|session_cost|crates/nexus-collab-protocol/src/economy.rs:6|
|MINOR|with_data|crates/nexus-collab-protocol/src/message.rs:68|
|MINOR|with_reasoning|crates/nexus-collab-protocol/src/message.rs:73|
|MINOR|with_references|crates/nexus-collab-protocol/src/message.rs:78|
|MINOR|is_broadcast|crates/nexus-collab-protocol/src/message.rs:83|
|MINOR|complete_session|crates/nexus-collab-protocol/src/protocol.rs:51|
|MINOR|completed_sessions|crates/nexus-collab-protocol/src/protocol.rs:65|
|MINOR|can_propose|crates/nexus-collab-protocol/src/roles.rs:14|
|MINOR|set_subprocess_timeout_ms|crates/nexus-computer-control/src/engine.rs:84|
|MINOR|tools_by_category|crates/nexus-external-tools/src/registry.rs:76|
|MINOR|generate_speculative|crates/nexus-flash-infer/src/speculative.rs:116|
|MINOR|has_changed|crates/nexus-governance-engine/src/versioning.rs:6|
|MINOR|is_valid_successor|crates/nexus-governance-engine/src/versioning.rs:11|
|MINOR|request_channel|crates/nexus-governance-oracle/src/submission.rs:7|
|MINOR|read_page|crates/nexus-perception/src/document.rs:8|
|MINOR|extract_table|crates/nexus-perception/src/document.rs:26|
|MINOR|clear_cache|crates/nexus-perception/src/engine.rs:241|
|MINOR|provider_model_id|crates/nexus-perception/src/engine.rs:245|
|MINOR|extract_form_data|crates/nexus-perception/src/extraction.rs:16|
|MINOR|extract_table_data|crates/nexus-perception/src/extraction.rs:30|
|MINOR|find_clickable_elements|crates/nexus-perception/src/screen.rs:29|
|MINOR|ask_about_screen|crates/nexus-perception/src/screen.rs:53|
|MINOR|max_difficulty|crates/nexus-predictive-router/src/difficulty_estimator.rs:28|
|MINOR|with_llm|crates/nexus-predictive-router/src/difficulty_estimator.rs:81|
|MINOR|check_staging|crates/nexus-predictive-router/src/staging.rs:18|
|MINOR|stage_cost|crates/nexus-software-factory/src/economy.rs:10|
|MINOR|complete_project|crates/nexus-software-factory/src/factory.rs:232|
|MINOR|get_artifact|crates/nexus-software-factory/src/project.rs:103|
|MINOR|suggested_capabilities|crates/nexus-software-factory/src/roles.rs:24|
|MINOR|check_delegation|crates/nexus-token-economy/src/gating.rs:52|
|MINOR|record_snapshot|crates/nexus-token-economy/src/supply.rs:35|
|MINOR|refund_escrow|crates/nexus-token-economy/src/wallet.rs:163|
|MINOR|restore_fs|crates/nexus-world-simulation/src/sandbox.rs:100|

### Backend Wrappers Never Called From Pages
|Severity|Wrapper|Definition|
|---|---|---|
|MINOR|clearAllAgents|app/src/api/backend.ts:102|
|MINOR|createAgent|app/src/api/backend.ts:106|
|MINOR|getAgentPerformance|app/src/api/backend.ts:174|
|MINOR|getAutoEvolutionLog|app/src/api/backend.ts:178|
|MINOR|setAutoEvolutionConfig|app/src/api/backend.ts:182|
|MINOR|forceEvolveAgent|app/src/api/backend.ts:196|
|MINOR|transcribePushToTalk|app/src/api/backend.ts:208|
|MINOR|startJarvisMode|app/src/api/backend.ts:212|
|MINOR|stopJarvisMode|app/src/api/backend.ts:216|
|MINOR|jarvisStatus|app/src/api/backend.ts:220|
|MINOR|detectHardware|app/src/api/backend.ts:224|
|MINOR|checkOllama|app/src/api/backend.ts:228|
|MINOR|pullOllamaModel|app/src/api/backend.ts:232|
|MINOR|runSetupWizard|app/src/api/backend.ts:241|
|MINOR|pullModel|app/src/api/backend.ts:248|
|MINOR|ensureOllama|app/src/api/backend.ts:257|
|MINOR|isOllamaInstalled|app/src/api/backend.ts:264|
|MINOR|deleteModel|app/src/api/backend.ts:268|
|MINOR|isSetupComplete|app/src/api/backend.ts:277|
|MINOR|listAvailableModels|app/src/api/backend.ts:281|
|MINOR|analyzeScreen|app/src/api/backend.ts:311|
|MINOR|computerControlCaptureScreen|app/src/api/backend.ts:338|
|MINOR|computerControlExecuteAction|app/src/api/backend.ts:342|
|MINOR|setAgentModel|app/src/api/backend.ts:386|
|MINOR|getSystemInfo|app/src/api/backend.ts:411|
|MINOR|getAgentIdentity|app/src/api/backend.ts:532|
|MINOR|listIdentities|app/src/api/backend.ts:536|
|MINOR|marketplaceInfo|app/src/api/backend.ts:583|
|MINOR|getBrowserHistory|app/src/api/backend.ts:607|
|MINOR|getAgentActivity|app/src/api/backend.ts:611|
|MINOR|startResearch|app/src/api/backend.ts:617|
|MINOR|researchAgentAction|app/src/api/backend.ts:624|
|MINOR|completeResearch|app/src/api/backend.ts:640|
|MINOR|getResearchSession|app/src/api/backend.ts:646|
|MINOR|listResearchSessions|app/src/api/backend.ts:652|
|MINOR|startBuild|app/src/api/backend.ts:658|
|MINOR|buildAppendCode|app/src/api/backend.ts:662|
|MINOR|buildAddMessage|app/src/api/backend.ts:674|
|MINOR|completeBuild|app/src/api/backend.ts:688|
|MINOR|getBuildSession|app/src/api/backend.ts:694|
|MINOR|getBuildCode|app/src/api/backend.ts:700|
|MINOR|getBuildPreview|app/src/api/backend.ts:704|
|MINOR|startLearning|app/src/api/backend.ts:710|
|MINOR|getKnowledgeBase|app/src/api/backend.ts:714|
|MINOR|getLearningSession|app/src/api/backend.ts:718|
|MINOR|learningAgentAction|app/src/api/backend.ts:753|
|MINOR|getProviderUsageStats|app/src/api/backend.ts:794|
|MINOR|emailSearchMessages|app/src/api/backend.ts:1006|
|MINOR|getAgentOutputs|app/src/api/backend.ts:1042|
|MINOR|projectGet|app/src/api/backend.ts:1052|
|MINOR|assignAgentGoal|app/src/api/backend.ts:1128|
|MINOR|stopAgentGoal|app/src/api/backend.ts:1159|
|MINOR|startAutonomousLoop|app/src/api/backend.ts:1168|
|MINOR|stopAutonomousLoop|app/src/api/backend.ts:1184|
|MINOR|getAgentCognitiveStatus|app/src/api/backend.ts:1191|
|MINOR|getAgentMemories|app/src/api/backend.ts:1211|
|MINOR|agentMemoryRemember|app/src/api/backend.ts:1227|
|MINOR|agentMemoryRecall|app/src/api/backend.ts:1245|
|MINOR|agentMemoryRecallByType|app/src/api/backend.ts:1259|
|MINOR|agentMemoryForget|app/src/api/backend.ts:1274|
|MINOR|agentMemoryGetStats|app/src/api/backend.ts:1283|
|MINOR|agentMemorySave|app/src/api/backend.ts:1287|
|MINOR|agentMemoryClear|app/src/api/backend.ts:1291|
|MINOR|getSelfEvolutionMetrics|app/src/api/backend.ts:1312|
|MINOR|getSelfEvolutionStrategies|app/src/api/backend.ts:1321|
|MINOR|triggerCrossAgentLearning|app/src/api/backend.ts:1330|
|MINOR|getHivemindStatus|app/src/api/backend.ts:1347|
|MINOR|cancelHivemind|app/src/api/backend.ts:1356|
|MINOR|getOsFitness|app/src/api/backend.ts:1552|
|MINOR|getFitnessHistory|app/src/api/backend.ts:1556|
|MINOR|getRoutingStats|app/src/api/backend.ts:1560|
|MINOR|getUiAdaptations|app/src/api/backend.ts:1564|
|MINOR|recordPageVisit|app/src/api/backend.ts:1572|
|MINOR|recordFeatureUse|app/src/api/backend.ts:1576|
|MINOR|overrideSecurityBlock|app/src/api/backend.ts:1580|
|MINOR|getOsImprovementLog|app/src/api/backend.ts:1592|
|MINOR|getMorningOsBriefing|app/src/api/backend.ts:1596|
|MINOR|recordRoutingOutcome|app/src/api/backend.ts:1600|
|MINOR|recordOperationTiming|app/src/api/backend.ts:1613|
|MINOR|getPerformanceReport|app/src/api/backend.ts:1624|
|MINOR|getSecurityEvolutionReport|app/src/api/backend.ts:1628|
|MINOR|recordKnowledgeInteraction|app/src/api/backend.ts:1632|
|MINOR|getOsDreamStatus|app/src/api/backend.ts:1644|
|MINOR|setSelfImproveEnabled|app/src/api/backend.ts:1648|
|MINOR|screenshotAnalyze|app/src/api/backend.ts:1654|
|MINOR|screenshotGenerateSpec|app/src/api/backend.ts:1658|
|MINOR|voiceProjectStart|app/src/api/backend.ts:1670|
|MINOR|voiceProjectStop|app/src/api/backend.ts:1674|
|MINOR|voiceProjectAddChunk|app/src/api/backend.ts:1678|
|MINOR|voiceProjectGetStatus|app/src/api/backend.ts:1685|
|MINOR|voiceProjectGetPrompt|app/src/api/backend.ts:1689|
|MINOR|voiceProjectUpdateIntent|app/src/api/backend.ts:1693|
|MINOR|stressGeneratePersonas|app/src/api/backend.ts:1705|
|MINOR|stressGenerateActions|app/src/api/backend.ts:1709|
|MINOR|stressEvaluateReport|app/src/api/backend.ts:1713|
|MINOR|deployGenerateDockerfile|app/src/api/backend.ts:1719|
|MINOR|deployValidateConfig|app/src/api/backend.ts:1723|
|MINOR|deployGetCommands|app/src/api/backend.ts:1727|
|MINOR|evolverRegisterApp|app/src/api/backend.ts:1733|
|MINOR|evolverUnregisterApp|app/src/api/backend.ts:1737|
|MINOR|evolverListApps|app/src/api/backend.ts:1741|
|MINOR|evolverDetectIssues|app/src/api/backend.ts:1745|
|MINOR|freelanceGetStatus|app/src/api/backend.ts:1751|
|MINOR|freelanceStartScanning|app/src/api/backend.ts:1755|
|MINOR|freelanceStopScanning|app/src/api/backend.ts:1759|
|MINOR|freelanceEvaluateJob|app/src/api/backend.ts:1763|
|MINOR|freelanceGetRevenue|app/src/api/backend.ts:1767|
|MINOR|getLivePreview|app/src/api/backend.ts:1781|
|MINOR|publishToMarketplace|app/src/api/backend.ts:1793|
|MINOR|installFromMarketplace|app/src/api/backend.ts:1797|
|MINOR|schedulerHistory|app/src/api/backend.ts:3048|
|MINOR|schedulerRunnerStatus|app/src/api/backend.ts:3059|
|MINOR|executeTeamWorkflow|app/src/api/backend.ts:3067|
|MINOR|transferAgentFuel|app/src/api/backend.ts:3082|
|MINOR|runContentPipeline|app/src/api/backend.ts:3100|
|MINOR|flashProfileModel|app/src/api/backend.ts:3115|
|MINOR|flashAutoConfigure|app/src/api/backend.ts:3120|
|MINOR|flashListSessions|app/src/api/backend.ts:3146|
|MINOR|flashGetMetrics|app/src/api/backend.ts:3161|
|MINOR|flashEstimatePerformance|app/src/api/backend.ts:3173|
|MINOR|flashCatalogRecommend|app/src/api/backend.ts:3180|
|MINOR|flashCatalogSearch|app/src/api/backend.ts:3185|
|MINOR|flashDownloadModel|app/src/api/backend.ts:3221|
|MINOR|flashDownloadMulti|app/src/api/backend.ts:3229|
|MINOR|flashDeleteLocalModel|app/src/api/backend.ts:3236|
|MINOR|flashAvailableDiskSpace|app/src/api/backend.ts:3240|
|MINOR|flashGetModelDir|app/src/api/backend.ts:3244|
|MINOR|cmGetProfile|app/src/api/backend.ts:3295|
|MINOR|cmTriggerFeedback|app/src/api/backend.ts:3321|
|MINOR|cmEvaluateResponse|app/src/api/backend.ts:3328|
|MINOR|cmExecuteValidationRun|app/src/api/backend.ts:3365|
|MINOR|cmListValidationRuns|app/src/api/backend.ts:3373|
|MINOR|cmGetValidationRun|app/src/api/backend.ts:3378|
|MINOR|cmThreeWayComparison|app/src/api/backend.ts:3383|
|MINOR|routerRouteTask|app/src/api/backend.ts:3400|
|MINOR|routerRecordOutcome|app/src/api/backend.ts:3404|
|MINOR|oracleVerifyToken|app/src/api/backend.ts:3464|
|MINOR|tokenGetWallet|app/src/api/backend.ts:3479|
|MINOR|tokenCreateWallet|app/src/api/backend.ts:3487|
|MINOR|tokenCalculateSpawn|app/src/api/backend.ts:3521|
|MINOR|tokenCreateDelegation|app/src/api/backend.ts:3528|
|MINOR|tokenGetDelegations|app/src/api/backend.ts:3539|
|MINOR|ccExecuteAction|app/src/api/backend.ts:3549|
|MINOR|simSubmit|app/src/api/backend.ts:3578|
|MINOR|simRun|app/src/api/backend.ts:3585|
|MINOR|simGetResult|app/src/api/backend.ts:3589|
|MINOR|simGetRisk|app/src/api/backend.ts:3601|
|MINOR|simBranch|app/src/api/backend.ts:3605|
|MINOR|memoryGetEntry|app/src/api/backend.ts:3693|
|MINOR|memoryListAgents|app/src/api/backend.ts:3729|
|MINOR|toolsListAvailable|app/src/api/backend.ts:3739|
|MINOR|swfSubmitArtifact|app/src/api/backend.ts:3885|

### Orphan Modules
None verified.

## MOCK DATA LOCATIONS
|Severity|Location|Finding|
|---|---|---|
|MAJOR|app/src/App.tsx:472|Demo chat status reply uses synthetic agent counts.|
|MAJOR|app/src/App.tsx:474|Demo chat explicitly says the user is seeing placeholder data.|
|MAJOR|app/src/App.tsx:478|Demo chat explicitly says actions are simulated.|
|MAJOR|app/src/App.tsx:606|Browser mode boots with a `[DEMO MODE]` message and simulated agent state.|
|MAJOR|app/src/App.tsx:669|Backend failure falls back to demo data.|
|MAJOR|app/src/pages/ComputerControl.tsx:46|Computer control defaults to `demo` mode.|
|MAJOR|app/src/pages/ComputerControl.tsx:202|Computer control renders canned `DEMO_ACTIONS`.|
|MAJOR|app/src/pages/ComputerControl.tsx:305|UI explicitly states the page shows what agents would do without taking real actions.|
|MAJOR|app/src/components/browser/ResearchMode.tsx:96|Research mode silently continues in mock mode when the desktop action fails.|
|MAJOR|app/src/components/browser/BuildMode.tsx:81|Build mode contains local `generateMockCode` generation.|
|MAJOR|app/src/components/browser/BuildMode.tsx:304|Build mode falls through to a mock session when the desktop path fails.|
|MAJOR|app/src/components/browser/BuildMode.tsx:415|Build mode marks completion through a mock-complete path.|
|MAJOR|app/src/pages/Audit.tsx:853|Audit chain verification has a client-side mock-mode fallback.|
|MAJOR|app/src/voice/PushToTalk.ts:74|Push-to-talk returns `mock-whisper` when no recording exists.|
|MAJOR|app/src/voice/PushToTalk.ts:101|Push-to-talk returns a canned transcript with `mock-whisper`.|
|CRITICAL|crates/nexus-capability-measurement/src/tauri_commands.rs:290|Batch evaluation uses mock adapters instead of real agent output.|
|CRITICAL|crates/nexus-capability-measurement/src/tauri_commands.rs:394|A/B baseline path uses hardcoded response text.|
|CRITICAL|crates/nexus-capability-measurement/src/tauri_commands.rs:404|A/B routed path uses hardcoded response text.|
|MAJOR|app/src-tauri/src/main.rs:15750|Simulation planner fallback returns synthetic mock text.|
|MAJOR|app/src-tauri/src/main.rs:15755|Simulation planner uses synthetic fallback on planner error.|
|MAJOR|app/src-tauri/src/main.rs:15769|Test simulation planner always returns mock simulation output.|
|MAJOR|app/src-tauri/src/main.rs:15830|Simulation persona generation emits `mock-persona-*` identities.|
|MAJOR|app/src-tauri/src/main.rs:15866|Simulation decision batches emit `mock-persona-*` targets/actions.|

## MISSING ERROR HANDLING
### Pages With Backend Calls But No Visible Loading State
|Severity|Page|Evidence|
|---|---|---|
|MAJOR|app/src/pages/AuditTimeline.tsx|first backend line 4|
|MAJOR|app/src/pages/Collaboration.tsx|first backend line 3|
|MAJOR|app/src/pages/DatabaseManager.tsx|first backend line 8|
|MAJOR|app/src/pages/DreamForge.tsx|first backend line 12|
|MAJOR|app/src/pages/Firewall.tsx|first backend line 2|
|MAJOR|app/src/pages/MediaStudio.tsx|first backend line 11|
|MAJOR|app/src/pages/Messaging.tsx|first backend line 11|
|MAJOR|app/src/pages/NotesApp.tsx|first backend line 3|
|MAJOR|app/src/pages/SelfRewriteLab.tsx|first backend line 7|
|MAJOR|app/src/pages/SoftwareFactory.tsx|first backend line 3|
|MAJOR|app/src/pages/SystemMonitor.tsx|first backend line 7|
|MAJOR|app/src/pages/TemporalEngine.tsx|first backend line 3|
|MAJOR|app/src/pages/Terminal.tsx|first backend line 3|
|MAJOR|app/src/pages/TimelineViewer.tsx|first backend line 2|
|MAJOR|app/src/pages/WorldSimulation.tsx|first backend line 13|

### Swallowed / Suppressed Error Paths
|Severity|Location|Observed Code|
|---|---|---|
|MAJOR|app/src/App.tsx:641|}).catch(() => {});|
|MAJOR|app/src/App.tsx:731|void Notification.requestPermission().catch(() => {});|
|MAJOR|app/src/App.tsx:826|.catch(() => {});|
|MAJOR|app/src/pages/DistributedAudit.tsx:46|getAuditLog(undefined, 200).catch(() => []),|
|MAJOR|app/src/pages/DistributedAudit.tsx:47|getAuditChainStatus().catch(() => null),|
|MAJOR|app/src/pages/ApprovalCenter.tsx:417|hitlStats().catch(() => null),|
|MAJOR|app/src/pages/ApprovalCenter.tsx:439|.catch(() => {});|
|MAJOR|app/src/pages/ApprovalCenter.tsx:442|.catch(() => {});|
|MAJOR|app/src/pages/ApprovalCenter.tsx:481|.catch(() => {});|
|MAJOR|app/src/pages/Firewall.tsx:12|getFirewallStatus().then(setStatus).catch(() => {});|
|MAJOR|app/src/pages/Firewall.tsx:13|getFirewallPatterns().then(setPatterns).catch(() => {});|
|MAJOR|app/src/pages/DeployPipeline.tsx:192|}).catch(() => {});|
|MAJOR|app/src/pages/ModelRouting.tsx:44|.catch(console.error)|
|MAJOR|app/src/pages/ModelRouting.tsx:52|.catch(console.error);|
|MAJOR|app/src/pages/LearningCenter.tsx:99|learningSaveProgress(JSON.stringify(progress)).catch(() => {});|
|MAJOR|app/src/pages/MeasurementBatteries.tsx:43|.catch(console.error)|
|MAJOR|app/src/pages/ApiClient.tsx:134|apiClientSaveCollections(JSON.stringify(cols)).catch(() => {});|
|MAJOR|app/src/pages/ApiClient.tsx:154|apiClientSaveCollections(JSON.stringify(next)).catch(() => {});|
|MAJOR|app/src/pages/EmailClient.tsx:136|}).catch(() => {});|
|MAJOR|app/src/pages/VoiceAssistant.tsx:414|}).catch(() => {});|
|MAJOR|app/src/pages/MeasurementDashboard.tsx:134|.catch(console.error);|
|MAJOR|app/src/pages/WorldSimulation2.tsx:58|listAgents().catch(() => []),|
|MAJOR|app/src/pages/WorldSimulation2.tsx:59|simGetPolicy().catch(() => null),|
|MAJOR|app/src/pages/GovernedControl.tsx:86|.catch(() => {})|
|MAJOR|app/src/pages/GovernedControl.tsx:93|ccGetActionHistory(selectedAgent).catch(() => []),|
|MAJOR|app/src/pages/GovernedControl.tsx:94|ccGetCapabilityBudget(selectedAgent).catch(() => null),|
|MAJOR|app/src/pages/GovernedControl.tsx:95|ccGetScreenContext(selectedAgent).catch(() => null),|
|MAJOR|app/src/pages/GovernedControl.tsx:96|ccVerifyActionSequence(selectedAgent).catch(() => null),|
|MAJOR|app/src/pages/SetupWizard.tsx:112|}).catch(() => {});|
|MAJOR|app/src/pages/FlashInference.tsx:149|}).catch(() => {});|
|MAJOR|app/src/pages/FlashInference.tsx:165|flashSystemMetrics().then((m: any) => { if (active) setSysMetrics(m); }).catch(() => {});|
|MAJOR|app/src/pages/FlashInference.tsx:172|}).catch(() => {});|
|MAJOR|app/src/pages/ABValidation.tsx:45|.catch(console.error)|
|MAJOR|app/src/pages/CapabilityBoundaryMap.tsx:66|.catch(console.error)|
|MAJOR|app/src/pages/AgentMemory.tsx:92|.catch(() => {});|
|MAJOR|app/src/pages/AgentMemory.tsx:93|memoryGetPolicy().then(setPolicy).catch(() => {});|
|MAJOR|app/src/pages/AgentMemory.tsx:102|memoryGetStats(selectedAgent).catch(() => null),|
|MAJOR|app/src/pages/MeasurementCompare.tsx:62|.catch(console.error);|
|MAJOR|app/src/pages/SoftwareFactory.tsx:85|swfListProjects().catch(() => []),|
|MAJOR|app/src/pages/SoftwareFactory.tsx:86|swfGetPipelineStages().catch(() => []),|
|MAJOR|app/src/pages/SoftwareFactory.tsx:87|swfGetPolicy().catch(() => null),|
|MAJOR|app/src/pages/SoftwareFactory.tsx:88|swfEstimateCost().catch(() => 0),|
|MAJOR|app/src/pages/SoftwareFactory.tsx:98|const p = await swfListProjects().catch(() => []);|
|MAJOR|app/src/pages/SoftwareFactory.tsx:101|const updated = await swfGetProject(selectedProject.id).catch(() => null);|
|MAJOR|app/src/pages/SoftwareFactory.tsx:104|const c = await swfGetCost(updated.id).catch(() => null);|
|MAJOR|app/src/pages/SoftwareFactory.tsx:136|const c = await swfGetCost(p.id).catch(() => null);|
|MAJOR|app/src/pages/Collaboration.tsx:105|collabListActive().catch(() => []),|
|MAJOR|app/src/pages/Collaboration.tsx:106|collabGetPatterns().catch(() => []),|
|MAJOR|app/src/pages/Collaboration.tsx:107|collabGetPolicy().catch(() => null),|
|MAJOR|app/src/pages/Collaboration.tsx:116|const s = await collabListActive().catch(() => []);|
|MAJOR|app/src/pages/Collaboration.tsx:119|const updated = await collabGetSession(selectedSession.id).catch(() => null);|
|MAJOR|app/src/pages/Collaboration.tsx:122|const c = await collabDetectConsensus(updated.id).catch(() => null);|
|MAJOR|app/src/pages/Collaboration.tsx:173|const c = await collabDetectConsensus(s.id).catch(() => null);|
|MAJOR|app/src/pages/BrowserAgent.tsx:43|browserGetPolicy().then((p) => setPolicy(p as Policy)).catch(console.error);|
|MAJOR|app/src/pages/BrowserAgent.tsx:44|browserSessionCount().then(setSessionCount).catch(console.error);|
|MAJOR|app/src/pages/BrowserAgent.tsx:45|listAgents().then((a) => setAgents(normalizeArray<{ id: string; name: string }>(a).filter((x) => x.name))).catch(console.error);|
|MAJOR|app/src/pages/BrowserAgent.tsx:78|.catch(console.error);|
|MAJOR|app/src/pages/BrowserAgent.tsx:83|browserCloseSession(sessionId).then(() => { setSessionId(null); browserSessionCount().then(setSessionCount); }).catch(console.error);|
|MAJOR|app/src/pages/Settings.tsx:234|}).catch(() => {});|
|MAJOR|app/src/pages/Settings.tsx:239|}).catch(() => {});|
|MAJOR|app/src/pages/Audit.tsx:899|getAuditLog(undefined, 500).then(setLiveEvents).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:253|}).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:266|getPreinstalledAgents().then(setPreinstalledAgents).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:267|listProviderModels().then(m => setModelCount(m.length)).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:268|getAvailableProviders().then(setAvailableProviders).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:274|getAvailableProviders().then(setAvailableProviders).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:276|getAvailableProviders().then(setAvailableProviders).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:869|setAgentLlmProvider(selectedAgentId, "auto", true, 0, 0).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:909|setAgentLlmProvider(selectedAgentId, `${p.id}${modelPart}`, p.id === "flash" \|\| p.id === "ollama", 0, 0).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:972|setAgentLlmProvider(selectedAgentId, `${selectedProvider}/${model}`, selectedProvider === "flash" \|\| selectedProvider === "ollama", 0, 0).catch(() => {});|
|MAJOR|app/src/pages/Agents.tsx:1324|setAgentReviewMode(selectedAgentId, true).catch(() => {});|
|MAJOR|app/src/pages/ExternalTools.tsx:78|toolsGetRegistry().catch(() => []),|
|MAJOR|app/src/pages/ExternalTools.tsx:79|toolsGetAudit(20).catch(() => []),|
|MAJOR|app/src/pages/ExternalTools.tsx:80|toolsGetPolicy().catch(() => null),|
|MAJOR|app/src/pages/ExternalTools.tsx:90|const reg = await toolsRefreshAvailability().catch(() => []);|
|MAJOR|app/src/pages/ExternalTools.tsx:102|const a = await toolsGetAudit(20).catch(() => []);|
|MAJOR|app/src/pages/ExternalTools.tsx:135|<button onClick={() => getRateLimitStatus().then(setRateLimits).catch(() => {})} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Rate Limits</button>|
|MAJOR|app/src/pages/GovernanceOracle.tsx:42|.catch(console.error)|
|MAJOR|app/src/pages/GovernanceOracle.tsx:50|.catch(console.error);|
|MAJOR|app/src/pages/ModelHub.tsx:450|.catch(() => {});|
|MAJOR|app/src/pages/TokenEconomy.tsx:141|tokenGetAllWallets().catch(() => []),|
|MAJOR|app/src/pages/TokenEconomy.tsx:142|tokenGetLedger(undefined, 100).catch(() => []),|
|MAJOR|app/src/pages/TokenEconomy.tsx:143|tokenGetSupply().catch(() => null),|
|MAJOR|app/src/pages/TokenEconomy.tsx:144|tokenGetPricing().catch(() => []),|
|MAJOR|app/src/pages/TokenEconomy.tsx:158|.catch(() => {});|
|MAJOR|app/src/pages/TokenEconomy.tsx:164|.catch(() => {});|
|MAJOR|app/src/pages/AuditTimeline.tsx:63|.catch(() => {});|
|MAJOR|app/src/pages/AuditTimeline.tsx:70|getAuditLog(undefined, 500).then(setLiveEvents).catch(() => {});|
|MAJOR|app/src/pages/Perception.tsx:102|const p = await perceptionGetPolicy().catch(() => null);|
|MAJOR|app/src/pages/ComplianceDashboard.tsx:114|getComplianceStatus().catch(() => null),|
|MAJOR|app/src/pages/ComplianceDashboard.tsx:115|getComplianceAgents().catch(() => []),|
|MAJOR|app/src/pages/ComplianceDashboard.tsx:116|getAuditLog(undefined, 50).catch(() => []),|
|MAJOR|app/src/components/agents/AgentDetail.tsx:199|.catch(() => undefined);|

## BUTTON AUDIT
|Check|Result|
|---|---|
|Buttons with empty handler (`onClick={() => {}}`)|0|
|Forms without submit handler|0|
|Alert-only handlers|0|

## PAGES WITH NO DIRECT BACKEND CALLS
|Severity|Page|Note|
|---|---|---|
|INFO|app/src/pages/SetupWizard.tsx|Direct-scan false positive: backend callbacks are injected from `app/src/App.tsx:1987-2028`, with mock fallbacks when not on desktop.|
|INFO|app/src/pages/commandCenterUi.tsx|Shared page/helper file; not routed from `app/src/App.tsx`.|

## DATA INTEGRITY
|Severity|File|Size (bytes)|Baseline Sessions|
|---|---|---|---|
|INFO|data/validation_runs/real-battery-baseline.json|7952461|54|
|INFO|data/validation_runs/run1-pre-bugfix-baseline.json|11439130|54|
|INFO|data/validation_runs/run2-post-bugfix.json|11439114|54|

|Check|Result|
|---|---|
|Prebuilt agent manifests (`agents/prebuilt/*.json`)|54|
|Capability measurement battery problems (`crates/nexus-capability-measurement/data/battery_v1.json`)|20|

## CONFIGURATION COMPLETENESS
|Severity|File|Status|Note|
|---|---|---|---|
|INFO|.gitlab-ci.yml|present||
|INFO|Cargo.toml|present||
|INFO|package.json|missing at repo root|Frontend manifest exists at `app/package.json`.|
|INFO|tsconfig.json|missing at repo root|Frontend TypeScript config exists at `app/tsconfig.json`.|
|INFO|app/package.json|present||
|INFO|app/tsconfig.json|present||

No undocumented environment variables found.

## SECURITY FINDINGS
|Severity|Check|Result|
|---|---|---|
|INFO|Hardcoded secret scan|No verified hardcoded secrets matched the audit scan.|
|INFO|Committed `.env` files|No `.env` files found in the repository tree.|
|INFO|Recent git additions of `.env`|No `.env` additions found in the last five matching commits.|

