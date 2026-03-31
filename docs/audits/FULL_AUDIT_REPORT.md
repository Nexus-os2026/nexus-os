# NEXUS OS COMPLETE AUDIT REPORT

Generated: 2026-03-28T02:25:50Z
Commit: 28b0b2c706f10a6900cddcfa35ddbaecfd1049a0

## SUMMARY
```tsv
metric	value
Generated	2026-03-28T02:25:50Z
Commit	28b0b2c706f10a6900cddcfa35ddbaecfd1049a0
Workspace dirty entries	14
Total crates	62
Crates compiling	62
Crates with clippy failures	0
Crates with test failures	1
Crates with zero tests	0
Total Tauri commands	637
Registered commands in generate_handler!	637
Commands with todo!/unimplemented!	0
Commands with no frontend caller	7
Commands with no TypeScript binding	7
Backend exports in app/src/api/backend.ts	631
Backend exports never called from pages	155
Total frontend pages	84
Pages with mock/placeholder indicators (heuristic)	50
Confirmed user-visible fake-data fallbacks	3
Pages with no backend calls	1
Pages missing error handling	0
Pages missing loading state	0
Buttons with no handler / empty handler findings	0
Dead/unused public functions	66
Orphan modules	0
Undocumented env vars	121
Frontend mock grep hits	297
Backend mock grep hits	63
```

## WORKTREE STATE
```text
M .gitlab-ci.yml
 M Cargo.lock
 M Cargo.toml
 M FULL_AUDIT_REPORT.md
 M agents/conductor/Cargo.toml
 M app/src-tauri/Cargo.toml
 M app/src-tauri/src/main.rs
 M app/src/api/backend.ts
 M app/src/pages/Protocols.tsx
 M audit.toml
 M deny.toml
?? A2A_WIRING_REPORT.md
?? crates/nexus-a2a/
?? crates/nexus-memory/
```

## CRITICAL FINDINGS
```tsv
id	severity	location	summary	evidence	impact
AUD-001	CRITICAL	Cargo.toml:64-65; app/src-tauri/Cargo.toml:51; crates/nexus-a2a/; crates/nexus-memory/	Workspace/app dependencies reference untracked crates	git status shows untracked crates/nexus-a2a/ and crates/nexus-memory/; git ls-files returns no tracked files for either path	Fresh clone or CI cannot reproduce the local workspace used for the demo
AUD-002	CRITICAL	tests/integration/Cargo.toml:15-83; app/src-tauri/src/main.rs:52,5833,5839,5849,5858,5866,5871	cargo test -p nexus-integration fails on unresolved nexus_a2a references	tests/integration/Cargo.toml declares no nexus-a2a dependency, while main.rs imports and calls nexus_a2a symbols	Backend integration suite is red and demo-signoff automation is blocked
```

## MAJOR FINDINGS
```tsv
id	severity	location	summary	evidence	impact
AUD-003	MAJOR	app/src-tauri/src/main.rs:23097-23149; app/src-tauri/src/main.rs:28311-28317; app/src/api/backend.ts	7 MCP2 Tauri commands are implemented and registered but have no frontend binding or caller	mcp2_server_status, mcp2_server_handle, mcp2_server_list_tools, mcp2_client_add, mcp2_client_remove, mcp2_client_discover, mcp2_client_call appear in generate_handler! but no matching exports exist in app/src/api/backend.ts and no page references them	Features are unwired and unreachable from the shipped UI
AUD-004	MAJOR	app/src/components/browser/BuildMode.tsx:81-182; app/src/components/browser/BuildMode.tsx:185-233; app/src/components/browser/BuildMode.tsx:299-321; app/src/components/browser/BuildMode.tsx:323-416	Browser build mode can simulate a successful build with generated code and scripted agent conversation	When desktop runtime/build RPCs fail, the component falls back to generateMockCode(), generateConversation(), and a synthetic session	A demo can look functional without exercising the backend
AUD-005	MAJOR	app/src/voice/PushToTalk.ts:70-75; app/src/voice/PushToTalk.ts:97-102	Push-to-talk returns a hardcoded mock transcript outside desktop/runtime speech support	Fallback returns source=mock-whisper and transcript="create an agent to post weekly Rust updates"	Voice UX can appear to work while returning fake content
AUD-006	MAJOR	app/src/App.tsx:1895-1904	Setup wizard injects synthetic hardware data in non-desktop mode	Fallback returns gpu="Mock GPU" with fixed VRAM/RAM and recommended models	Hardware detection UI can present fake device capabilities to users
```

## MINOR FINDINGS
```tsv
id	severity	location	summary	evidence	impact
AUD-007	MINOR	app/src/pages/commandCenterUi.tsx	Shared UI helper is stored under pages/ but is not a routable page	File is not in router/nav and imports are spread across 22 page files	Page inventory is noisier and routing audits can misclassify the file
AUD-008	MINOR	app/src/api/backend.ts (155 exports; see appendix)	Large unused TypeScript backend surface	155 exported helpers are never referenced from app/src/pages	Dead bindings increase maintenance cost and make wiring gaps harder to spot
AUD-009	MINOR	Multiple crate files (66 entries; see appendix)	Unused public functions detected	Heuristic search found 66 public functions with no callers in crates/ or app/	Technical debt and drift risk
AUD-010	MINOR	Multiple files (121 entries; see appendix)	Environment variables referenced in code are not documented	121 env var names are missing from README.md and .env.example	Setup drift and hidden runtime requirements
AUD-011	MINOR	package.json; tsconfig.json; app/package.json; app/tsconfig.json	Root-level frontend config completeness check fails	package.json and tsconfig.json are missing at repo root even though app/package.json and app/tsconfig.json exist	Audit automation at repo root produces false failures or requires special casing
```

## INFO
```tsv
id	severity	location	summary	evidence	impact
AUD-012	INFO	app/src-tauri/src/main.rs:13749-13828	No real todo!/unimplemented! command implementations were found	Only string-based policy checks mention todo!/unimplemented!; command bodies themselves are implemented	Good signal: registered Tauri commands are not stubbed with Rust macros
AUD-013	INFO	app/src/pages/**/*	No empty button handlers or forms without submit handlers were found	Greps for empty onClick handlers, console.log/alert handlers, and forms without onSubmit returned 0 findings	No confirmed dead buttons from the static audit heuristics
AUD-014	INFO	crates/**/*; app/**/*	No hardcoded secrets or committed .env files were found	Secret grep returned no matches; find . -name .env and recent git history checks were empty	No immediate secret-leak finding from this audit pass
AUD-015	INFO	app/src-tauri/src/main.rs (AppState)	Governance Oracle has no dedicated AppState field/init entry	AppState audit found no field_lines/init_lines for governance_oracle while adjacent demo crates do have explicit state wiring	State wiring is inconsistent but not currently blocking
AUD-016	INFO	data/validation_runs/*.json; agents/prebuilt/*.json; crates/nexus-capability-measurement/data/battery_v1.json	Validation/demo data assets are present	3 validation run files exist, 54 agent manifests exist, battery_v1.json contains 20 problems	Data integrity checks passed for the audited files
```

## INTEGRATION TEST FAILURE DETAIL
```tsv
kind	location	detail
crate	tests/integration/Cargo.toml	nexus-integration
missing_dependency	tests/integration/Cargo.toml:15-83	No nexus-a2a dependency declared
failing_import	app/src-tauri/src/main.rs:52	use nexus_a2a::A2aState;
failing_wrapper	app/src-tauri/src/main.rs:5833	nexus_a2a::tauri_commands::a2a_crate_get_agent_card
failing_wrapper	app/src-tauri/src/main.rs:5839	nexus_a2a::tauri_commands::a2a_crate_list_skills
failing_wrapper	app/src-tauri/src/main.rs:5849	nexus_a2a::tauri_commands::a2a_crate_send_task
failing_wrapper	app/src-tauri/src/main.rs:5858	nexus_a2a::tauri_commands::a2a_crate_get_task
failing_wrapper	app/src-tauri/src/main.rs:5866	nexus_a2a::tauri_commands::a2a_crate_discover_agent
failing_wrapper	app/src-tauri/src/main.rs:5871	nexus_a2a::tauri_commands::a2a_crate_get_status
cargo_test	cargo test -p nexus-integration	FAIL rc=101
```

## PER-CRATE STATUS
```tsv
crate	manifest	cargo_check	cargo_clippy_D_warnings	cargo_test	static_test_markers	tests_run	zero_tests	notes
coder-agent	agents/coder/Cargo.toml	PASS	PASS	PASS	45	42	no	
nexus-connectors-llm	connectors/llm/Cargo.toml	PASS	PASS	PASS	345	291	no	
nexus-flash-infer	crates/nexus-flash-infer/Cargo.toml	PASS	PASS	PASS	76	54	no	
nexus-llama-bridge	llama-bridge/Cargo.toml	PASS	PASS	PASS	31	32	no	
nexus-kernel	kernel/Cargo.toml	PASS	PASS	PASS	2023	2017	no	
nexus-persistence	persistence/Cargo.toml	PASS	PASS	PASS	58	58	no	
nexus-sdk	sdk/Cargo.toml	PASS	PASS	PASS	217	218	no	
designer-agent	agents/designer/Cargo.toml	PASS	PASS	PASS	3	3	no	
coding-agent	agents/coding-agent/Cargo.toml	PASS	PASS	PASS	5	5	no	
screen-poster-agent	agents/screen-poster/Cargo.toml	PASS	PASS	PASS	6	6	no	
self-improve-agent	agents/self-improve/Cargo.toml	PASS	PASS	PASS	7	7	no	
social-poster-agent	agents/social-poster/Cargo.toml	PASS	PASS	PASS	1	1	no	
nexus-connectors-web	connectors/web/Cargo.toml	PASS	PASS	PASS	8	8	no	
nexus-connectors-core	connectors/core/Cargo.toml	PASS	PASS	PASS	8	8	no	
nexus-content	content/Cargo.toml	PASS	PASS	PASS	3	3	no	
web-builder-agent	agents/web-builder/Cargo.toml	PASS	PASS	PASS	12	12	no	
workflow-studio-agent	agents/workflow-studio/Cargo.toml	PASS	PASS	PASS	4	4	no	
nexus-connectors-social	connectors/social/Cargo.toml	PASS	PASS	PASS	1	1	no	
nexus-connectors-messaging	connectors/messaging/Cargo.toml	PASS	PASS	PASS	46	46	no	
nexus-workflows	workflows/Cargo.toml	PASS	PASS	PASS	5	5	no	
nexus-research	research/Cargo.toml	PASS	PASS	PASS	12	12	no	
nexus-cli	cli/Cargo.toml	PASS	PASS	PASS	114	106	no	
nexus-conductor	agents/conductor/Cargo.toml	PASS	PASS	PASS	28	28	no	
nexus-collaboration	agents/collaboration/Cargo.toml	PASS	PASS	PASS	22	22	no	
nexus-factory	factory/Cargo.toml	PASS	PASS	PASS	30	30	no	
nexus-marketplace	marketplace/Cargo.toml	PASS	PASS	PASS	84	84	no	
nexus-adaptation	adaptation/Cargo.toml	PASS	PASS	PASS	23	23	no	
nexus-analytics	analytics/Cargo.toml	PASS	PASS	PASS	5	5	no	
nexus-control	control/Cargo.toml	PASS	PASS	PASS	15	15	no	
nexus-self-update	self-update/Cargo.toml	PASS	PASS	PASS	10	10	no	
nexus-integration	tests/integration/Cargo.toml	PASS	PASS	FAIL(101)	19		no	FAIL: unresolved nexus_a2a in included app/src-tauri/src/main.rs during cargo test
nexus-agent-memory	crates/nexus-agent-memory/Cargo.toml	PASS	PASS	PASS	21	21	no	
nexus-airgap	packaging/airgap/Cargo.toml	PASS	PASS	PASS	15	15	no	
nexus-auth	auth/Cargo.toml	PASS	PASS	PASS	32	32	no	
nexus-browser-agent	crates/nexus-browser-agent/Cargo.toml	PASS	PASS	PASS	12	12	no	
nexus-capability-measurement	crates/nexus-capability-measurement/Cargo.toml	PASS	PASS	PASS	77	77	no	
nexus-cloud	cloud/Cargo.toml	PASS	PASS	PASS	22	22	no	
nexus-collab-protocol	crates/nexus-collab-protocol/Cargo.toml	PASS	PASS	PASS	18	18	no	
nexus-computer-control	crates/nexus-computer-control/Cargo.toml	PASS	PASS	PASS	16	16	no	
nexus-distributed	distributed/Cargo.toml	PASS	PASS	PASS	179	179	no	
nexus-enterprise	enterprise/Cargo.toml	PASS	PASS	PASS	21	21	no	
nexus-external-tools	crates/nexus-external-tools/Cargo.toml	PASS	PASS	PASS	17	17	no	
nexus-governance-engine	crates/nexus-governance-engine/Cargo.toml	PASS	PASS	PASS	9	9	no	
nexus-governance-oracle	crates/nexus-governance-oracle/Cargo.toml	PASS	PASS	PASS	12	12	no	
nexus-governance-evolution	crates/nexus-governance-evolution/Cargo.toml	PASS	PASS	PASS	7	7	no	
nexus-integrations	integrations/Cargo.toml	PASS	PASS	PASS	42	42	no	
nexus-mcp	crates/nexus-mcp/Cargo.toml	PASS	PASS	PASS	25	25	no	
nexus-metering	metering/Cargo.toml	PASS	PASS	PASS	18	18	no	
nexus-perception	crates/nexus-perception/Cargo.toml	PASS	PASS	PASS	19	19	no	
nexus-predictive-router	crates/nexus-predictive-router/Cargo.toml	PASS	PASS	PASS	14	14	no	
nexus-protocols	protocols/Cargo.toml	PASS	PASS	PASS	92	92	no	
nexus-software-factory	crates/nexus-software-factory/Cargo.toml	PASS	PASS	PASS	18	18	no	
nexus-telemetry	telemetry/Cargo.toml	PASS	PASS	PASS	21	22	no	
nexus-tenancy	tenancy/Cargo.toml	PASS	PASS	PASS	50	51	no	
nexus-token-economy	crates/nexus-token-economy/Cargo.toml	PASS	PASS	PASS	29	29	no	
nexus-world-simulation	crates/nexus-world-simulation/Cargo.toml	PASS	PASS	PASS	18	18	no	
nexus-desktop-backend	app/src-tauri/Cargo.toml	PASS	PASS	PASS	90	90	no	
nexus-a2a	crates/nexus-a2a/Cargo.toml	PASS	PASS	PASS	32	32	no	
nexus-benchmarks	benchmarks/Cargo.toml	PASS	PASS	PASS	2	1	no	
nexus-conductor-benchmark	benchmarks/conductor-bench/Cargo.toml	PASS	PASS	PASS	1	1	no	
nexus-server	crates/nexus-server/Cargo.toml	PASS	PASS	PASS	2	2	no	
nexus-memory	crates/nexus-memory/Cargo.toml	PASS	PASS	PASS	119	120	no	
```

## PER-PAGE STATUS
```tsv
page	path	route_refs	nav_refs	backend_call_lines	backend_call_count	mock_indicator_lines	interactive_lines	button_handler_findings	has_error_handling	has_loading_state	notes
ABValidation	app/src/pages/ABValidation.tsx	app/src/App.tsx:1634-1635 (ab-validation)	app/src/App.tsx:179 (ab-validation)	2	1	NONE	70,77,156	0	yes	yes	
AdminCompliance	app/src/pages/AdminCompliance.tsx	app/src/App.tsx:1703-1704 (admin-compliance)	app/src/App.tsx:219 (admin-compliance)	1,5,80,86	4	NONE	114,115,120	0	yes	yes	
AdminDashboard	app/src/pages/AdminDashboard.tsx	app/src/App.tsx:1691-1692 (admin-console)	app/src/App.tsx:216 (admin-console)	1,4,56	3	NONE	76	0	yes	yes	
AdminFleet	app/src/pages/AdminFleet.tsx	app/src/App.tsx:1697-1698 (admin-fleet)	app/src/App.tsx:218 (admin-fleet)	1,6,78	3	NONE	172,173,176	0	yes	yes	
AdminPolicyEditor	app/src/pages/AdminPolicyEditor.tsx	app/src/App.tsx:1700-1701 (admin-policies)	app/src/App.tsx:220 (admin-policies)	1,6,92	3	NONE	118,130,201	0	yes	yes	
AdminSystemHealth	app/src/pages/AdminSystemHealth.tsx	app/src/App.tsx:1706-1707 (admin-health)	app/src/App.tsx:221 (admin-health)	1,9,111,122,161	5	NONE	264,267,299,302,306,309	0	yes	yes	
AdminUsers	app/src/pages/AdminUsers.tsx	app/src/App.tsx:1694-1695 (admin-users)	app/src/App.tsx:217 (admin-users)	1,7,48	3	104	96,108,131,182	0	yes	yes	
AgentBrowser	app/src/pages/AgentBrowser.tsx	app/src/App.tsx:1598-1599 (browser)	app/src/App.tsx:171 (browser)	1,6,56,166	4	75	242,244,261,263,273,276	0	yes	yes	
AgentDnaLab	app/src/pages/AgentDnaLab.tsx	app/src/App.tsx:1613-1614 (dna-lab)	app/src/App.tsx:172 (dna-lab)	1,2,22,217,222,232,233,273,298,377	10	851,857,875,912,929,936,954,961,989,1006,1014	549,552,579,580,591,612,613,725,759,812,815,818,821,863,891,918,942,967,994,1031,1053,1068,1088,1100	0	yes	yes	
AgentMemory	app/src/pages/AgentMemory.tsx	app/src/App.tsx:1652-1653 (agent-memory)	app/src/App.tsx:186 (agent-memory)	1,13,86,113	4	200,206,208,230,243	181,182,183,184,220,235,244,312,313	0	yes	yes	
Agents	app/src/pages/Agents.tsx	app/src/App.tsx:1415-1417 (agents)	app/src/App.tsx:148 (agents)	1,9,10,194,281,289,404,408,430	9	557,1159	460,461,488,492,496,576,578,608,610,653,655,684,685,742,744,925,926,959,961,1060,1061,1172,1173,1233,1234,1256,1257,1281,1282,1368,1369,1392,1393,1421,1422	0	yes	yes	
AiChatHub	app/src/pages/AiChatHub.tsx	app/src/App.tsx:1580-1581 (ai-chat-hub)	app/src/App.tsx:147 (ai-chat-hub)	1,11,146,380,393,457,462,476,491,495,500,507,539,644,691,732,742,841,901	19	187,865,1178,1542,1583,1677,1777	285,289,783,1093,1099,1108,1123,1143,1164,1180,1196,1208,1211,1249,1250,1257,1266,1275,1278,1291,1292,1293,1294,1333,1338,1343,1349,1352,1354,1361,1384,1403,1436,1506,1509,1544,1545,1584,1614,1617,1635,1638,1682,1684,1704,1718,1719,1732,1752,1753,1756,1782,1785	0	yes	yes	
ApiClient	app/src/pages/ApiClient.tsx	app/src/App.tsx:1562-1563 (api-client)	app/src/App.tsx:199 (api-client)	1,3,86,112,113,130	6	86,378,412,413,430,431,450,453,464,465,488,495,497,503,505	303,304,333,340,345,350,353,379,392,406,414,425,432,443,459,466,479,508,509,533	0	yes	yes	
ApprovalCenter	app/src/pages/ApprovalCenter.tsx	app/src/App.tsx:1595-1596 (approvals)	app/src/App.tsx:154 (approvals)	1,23,24,65,89,406,429,434,448	9	298	233,235,259,260,276,277,312,313	0	yes	yes	
AppStore	app/src/pages/AppStore.tsx	app/src/App.tsx:1577-1578 (app-store); app/src/App.tsx:1577-1578 (marketplace); app/src/App.tsx:1577-1578 (marketplace-browser)	app/src/App.tsx:229 (app-store); app/src/App.tsx:236 (marketplace); app/src/App.tsx:237 (marketplace-browser)	1,9,98	3	212,377	216,220,286,289,352,355,369,370,381,394	0	yes	yes	
Audit	app/src/pages/Audit.tsx	app/src/App.tsx:1493-1494 (audit)	app/src/App.tsx:164 (audit)	1,20,21,161,709,710,712,716,717,780,781,782,783,784,811,888,896,1010	18	266,272,296,302,308,314,338,353,648,853,982	274,278,316,320,355,359,374,378,417,420,425,428,449,624,628,650,654,667,671,927,930,938,941,945,948,952,955,959,962,1013,1016,1027,1028,1029,1030,1031,1032,1047,1062,1065,1088,1136	0	yes	yes	
AuditTimeline	app/src/pages/AuditTimeline.tsx	app/src/App.tsx:1502-1503 (audit-timeline)	app/src/App.tsx:165 (audit-timeline)	1,4,50,55,70	5	NONE	128,132,159	0	yes	yes	
BrowserAgent	app/src/pages/BrowserAgent.tsx	app/src/App.tsx:1637-1638 (browser-agent)	app/src/App.tsx:180 (browser-agent)	1,12,42	3	115,124	99,112,117,126,127,128	0	yes	yes	
CapabilityBoundaryMap	app/src/pages/CapabilityBoundaryMap.tsx	app/src/App.tsx:1628-1629 (capability-boundaries)	app/src/App.tsx:177 (capability-boundaries)	1,8,70	3	NONE	90	0	yes	yes	
Chat	app/src/pages/Chat.tsx	app/src/App.tsx:1388-1390 (chat)	app/src/App.tsx:233 (chat)	1,6,196,242,251	5	136,489	312,315,323,326,418,421,458,466,468,495	0	yes	yes	
Civilization	app/src/pages/Civilization.tsx	app/src/App.tsx:1685-1686 (civilization)	app/src/App.tsx:211 (civilization)	1,2,33,308,309,310,311,312,328,385,393,406,438,498,634,638,642,654,990,991,1062,1063	22	788,910,987,1059,1132,1133,1142,1149,1159,1160,1189,1190,1192,1193,1195,1204,1209,1218,1219,1270,1271,1277,1286,1287,1296,1305,1307,1308	205,207,791,811,814,852,913,965,990,997,1024,1062,1077,1108,1111,1114,1134,1150,1161,1178,1196,1210,1220,1235,1252,1278,1288,1297,1309,1339,1341	0	yes	yes	
ClusterStatus	app/src/pages/ClusterStatus.tsx	app/src/App.tsx:1511-1512 (cluster)	app/src/App.tsx:224 (cluster)	1,11,59	3	242,247,266,271	183,185,250,251,274,275	0	yes	yes	
CodeEditor	app/src/pages/CodeEditor.tsx	app/src/App.tsx:1526-1527 (code-editor)	app/src/App.tsx:198 (code-editor)	1,11,21,27,31,34,245,272,276,296,304,310,343,384,411,668,687	17	820,854,1030,1081	746,756,796,797,798,799,800,801,802,803,812,822,831,833,850,866,870,887,892,976,977,987,1002,1003,1004,1005,1035,1038,1042,1058,1059,1082,1149,1167,1171	0	yes	yes	
Collaboration	app/src/pages/Collaboration.tsx	app/src/App.tsx:1658-1659 (collab-protocol)	app/src/App.tsx:188 (collab-protocol)	1,15,105	3	201,202,206,272,317,332	207,216,265,267,279,326,327,328,334,335,337	0	yes	yes	
CommandCenter	app/src/pages/CommandCenter.tsx	app/src/App.tsx:1499-1500 (command-center)	app/src/App.tsx:234 (command-center)	1,2,48	3	NONE	132,136,140,144,148,152	0	yes	yes	
commandCenterUi	app/src/pages/commandCenterUi.tsx	NONE	NONE	NONE	0	NONE	208,210	0	no	no	shared helper imported by 22 page files
ComplianceDashboard	app/src/pages/ComplianceDashboard.tsx	app/src/App.tsx:1508-1509 (compliance)	app/src/App.tsx:168 (compliance)	1,10,11,84,103,109,208,214	8	NONE	265,269,479,482,703,707,734,737,770,773,778,781,853	0	yes	yes	
ComputerControl	app/src/pages/ComputerControl.tsx	app/src/App.tsx:1601-1602 (computer-control)	app/src/App.tsx:212 (computer-control)	2,17,75,79,230	5	483,484,507	255,257,262,264,281,282,291,317,393,395,401,403,426,429,445,447,462,464,486,488,510,512	0	yes	yes	
ConsciousnessMonitor	app/src/pages/ConsciousnessMonitor.tsx	app/src/App.tsx:1676-1677 (consciousness)	app/src/App.tsx:191 (consciousness)	1,2,11,167,172,183,190,217	8	NONE	260,330	0	yes	yes	
Dashboard	app/src/pages/Dashboard.tsx	app/src/App.tsx:1385-1386 (dashboard)	app/src/App.tsx:146 (dashboard)	1,2,77	3	NONE	116,118	0	yes	yes	
DatabaseManager	app/src/pages/DatabaseManager.tsx	app/src/App.tsx:1559-1560 (database)	app/src/App.tsx:200 (database)	1,8,94,245	4	323,330,424,512	333,347,351,356,396,412,415,416,417,483,485,495,513,538,560,596,686	0	yes	yes	
DeployPipeline	app/src/pages/DeployPipeline.tsx	app/src/App.tsx:1586-1587 (deploy-pipeline)	app/src/App.tsx:202 (deploy-pipeline)	1,15,167,183,188	5	483,493,765,767,789,812,814	391,396,409,464,469,497,500,512,532,542,642,644,649,651,656,658,674,769,791,816,837	0	yes	yes	
DesignStudio	app/src/pages/DesignStudio.tsx	app/src/App.tsx:1565-1566 (design-studio)	app/src/App.tsx:193 (design-studio)	1,9,53,68	4	NONE	232,234,240,242,248,250,310,313	0	yes	yes	
DeveloperPortal	app/src/pages/DeveloperPortal.tsx	app/src/App.tsx:1505-1506 (developer-portal)	app/src/App.tsx:201 (developer-portal)	1,2,50,67,158	5	NONE	203,229,273,289	0	yes	yes	
DistributedAudit	app/src/pages/DistributedAudit.tsx	app/src/App.tsx:1520-1521 (distributed-audit)	app/src/App.tsx:225 (distributed-audit)	1,2,30,40	4	NONE	NONE	0	yes	yes	
Documents	app/src/pages/Documents.tsx	app/src/App.tsx:1538-1539 (documents)	app/src/App.tsx:152 (documents)	1,10,123,172,176,180,186	7	1074	334,378,379,487,825,868,869,884,885,1091,1092	0	yes	yes	
DreamForge	app/src/pages/DreamForge.tsx	app/src/App.tsx:1679-1680 (dreams)	app/src/App.tsx:195 (dreams)	1,12,127,134	4	NONE	319,337,340	0	yes	yes	
EmailClient	app/src/pages/EmailClient.tsx	app/src/App.tsx:1568-1569 (email-client)	app/src/App.tsx:158 (email-client)	1,2,119,127	4	402,472,476,479	346,354,358,361,371,383,423,425,452,453,461,482,484,486,501,502,503,504	0	yes	yes	
ExternalTools	app/src/pages/ExternalTools.tsx	app/src/App.tsx:1655-1656 (external-tools)	app/src/App.tsx:187 (external-tools)	1,10,87	3	184,191	133,134,135,147,197,198	0	yes	yes	
FileManager	app/src/pages/FileManager.tsx	app/src/App.tsx:1532-1533 (file-manager)	app/src/App.tsx:149 (file-manager)	1,10,165,302	4	373,383,482	333,334,335,336,337,346,353,357,364,365,375,384,385,396,397,398,412,455,456,457,467,468,557,558	0	yes	yes	
Firewall	app/src/pages/Firewall.tsx	app/src/App.tsx:1607-1608 (firewall)	app/src/App.tsx:167 (firewall)	1,2,11	3	NONE	37,39	0	yes	yes	
FlashInference	app/src/pages/FlashInference.tsx	app/src/App.tsx:1544-1545 (flash-inference)	app/src/App.tsx:151 (flash-inference)	1,15,141,150,151,161,200,202,208,383	10	637	449,455,467,497,544,545,560,601,623,630,631,638	0	yes	yes	
GovernanceOracle	app/src/pages/GovernanceOracle.tsx	app/src/App.tsx:1640-1641 (governance-oracle)	app/src/App.tsx:181 (governance-oracle)	1,2,36	3	NONE	114	0	yes	yes	
GovernedControl	app/src/pages/GovernedControl.tsx	app/src/App.tsx:1646-1647 (governed-control)	app/src/App.tsx:183 (governed-control)	1,9,83,94	4	NONE	208,217	0	yes	yes	
Identity	app/src/pages/Identity.tsx	app/src/App.tsx:1523-1524 (identity)	app/src/App.tsx:239 (identity)	1,14,15,191,365,366,382,418,433,439,443,468	12	619,722,728,788	514,558,561,592,622,665,669,672,707,734,779,791,805,827	0	yes	yes	
ImmuneDashboard	app/src/pages/ImmuneDashboard.tsx	app/src/App.tsx:1673-1674 (immune-dashboard)	app/src/App.tsx:238 (immune-dashboard)	1,10,194,200,206	5	NONE	428,484,501,512	0	yes	yes	
Integrations	app/src/pages/Integrations.tsx	app/src/App.tsx:1709-1710 (integrations)	app/src/App.tsx:161 (integrations)	1,18,97,100	4	229,384,392,393,394,398,402,403,404,405,409,410,411,415,416,417,421,422,423,427,428,429	174,176,198,200,206,208,238,241,254,257,267,318,331,333,361,364	0	yes	yes	
KnowledgeGraph	app/src/pages/KnowledgeGraph.tsx	app/src/App.tsx:1670-1671 (knowledge-graph)	app/src/App.tsx:230 (knowledge-graph)	1,2,17,228,370,380,381	7	522,658,680,702,749,756,762,768,799,806,827,844	525,544,547,620,623,626,661,683,705,731,772,809,830,847	0	yes	yes	
LearningCenter	app/src/pages/LearningCenter.tsx	app/src/App.tsx:1592-1593 (learning-center)	app/src/App.tsx:228 (learning-center)	1,14,98,112,283,290,298,332,424,433,440,537,539,547,548,551,558,559,561,578,620,656,675,680,725,1037,1240,1241	28	457,465,473,481,489,1130	705,780,812,843,874,877,903,960,962,1041,1066,1086,1136	0	yes	yes	
Login	app/src/pages/Login.tsx	app/src/App.tsx:1712-1713 (login)	app/src/App.tsx:214 (login)	1,8,135	3	NONE	210,212	0	yes	yes	
MeasurementBatteries	app/src/pages/MeasurementBatteries.tsx	app/src/App.tsx:1625-1626 (measurement-batteries)	app/src/App.tsx:176 (measurement-batteries)	1,2,40	3	NONE	69	0	yes	yes	
MeasurementCompare	app/src/pages/MeasurementCompare.tsx	app/src/App.tsx:1622-1623 (measurement-compare)	app/src/App.tsx:175 (measurement-compare)	1,2,56	3	NONE	99,102	0	yes	yes	
MeasurementDashboard	app/src/pages/MeasurementDashboard.tsx	app/src/App.tsx:1616-1617 (measurement)	app/src/App.tsx:173 (measurement)	1,8,126	3	NONE	163,169,185,268,270,277	0	yes	yes	
MeasurementSession	app/src/pages/MeasurementSession.tsx	app/src/App.tsx:1619-1620 (measurement-session)	app/src/App.tsx:174 (measurement-session)	1,2,105	3	NONE	194	0	yes	yes	
MediaStudio	app/src/pages/MediaStudio.tsx	app/src/App.tsx:1574-1575 (media-studio)	app/src/App.tsx:194 (media-studio)	4,11,70	3	NONE	191,193,205,208,257,259,292,294	0	yes	yes	
Messaging	app/src/pages/Messaging.tsx	app/src/App.tsx:1571-1572 (messaging)	app/src/App.tsx:160 (messaging)	1,11,91,107,156	5	292,358,359	296,298,303,305,310,313,360	0	yes	yes	
MissionControl	app/src/pages/MissionControl.tsx	app/src/App.tsx:1610-1611 (mission-control)	app/src/App.tsx:235 (mission-control)	1,13,168,178,185	5	NONE	286,289,292,379,388,402,407,412,416,425,479,501,518,585	0	yes	yes	
ModelHub	app/src/pages/ModelHub.tsx	app/src/App.tsx:1541-1542 (model-hub)	app/src/App.tsx:150 (model-hub)	1,18,182,227,267,341	6	590,1526,1543,1595,1612,1629	620,622,705,1059,1060,1265,1266,1280,1281,1297,1298,1402,1403,1420,1421,1484,1485,1556,1557,1642,1643	0	yes	yes	
ModelRouting	app/src/pages/ModelRouting.tsx	app/src/App.tsx:1631-1632 (model-routing)	app/src/App.tsx:178 (model-routing)	1,7,37	3	91	96	0	yes	yes	
NotesApp	app/src/pages/NotesApp.tsx	app/src/App.tsx:1553-1554 (notes)	app/src/App.tsx:196 (notes)	1,3,6,159,166,231,253,378	8	430,534,583	399,409,410,419,431,438,444,457,481,499,503,507,508,538,539,540,542,544,547,558,566,613	0	yes	yes	
Perception	app/src/pages/Perception.tsx	app/src/App.tsx:1649-1650 (perception)	app/src/App.tsx:185 (perception)	12	1	205,211,266,275	216,229,283,284	0	yes	yes	
PermissionDashboard	app/src/pages/PermissionDashboard.tsx	NONE	NONE	1,21,146,224	4	NONE	265,267,358,366,375,384,386,394,396,410,412,435,436,440,441,530,532	0	yes	yes	embedded via app/src/App.tsx:1483 (rendered inside permissions page)
PolicyManagement	app/src/pages/PolicyManagement.tsx	app/src/App.tsx:1604-1605 (policy-management)	app/src/App.tsx:226 (policy-management)	1,7,57	3	NONE	109,111,133,134,213,214,303,304	0	yes	yes	
ProjectManager	app/src/pages/ProjectManager.tsx	app/src/App.tsx:1556-1557 (project-manager)	app/src/App.tsx:231 (project-manager)	1,2,180,187	4	13,79,154,328,359,619	316,321,352,353,356,363,390,454,502,521,616,652,683	0	yes	yes	
Protocols	app/src/pages/Protocols.tsx	app/src/App.tsx:1517-1518 (protocols)	app/src/App.tsx:203 (protocols)	1,23,277	3	497,543,549,570,577,654,661,677,780,786	455,501,504,554,557,581,584,588,591,601,607,613,681,684,719,722,727,730,735,738,791,794	0	yes	yes	
Scheduler	app/src/pages/Scheduler.tsx	app/src/App.tsx:1589-1590 (scheduler)	app/src/App.tsx:153 (scheduler)	1,9,109	3	239,243	211,212,221,228,313,362,365,368	0	yes	yes	
SelfRewriteLab	app/src/pages/SelfRewriteLab.tsx	app/src/App.tsx:1688-1689 (self-rewrite)	app/src/App.tsx:190 (self-rewrite)	1,2,11,174,220,258,279,482	8	NONE	391,468,471,477,508,526,568,571	0	yes	yes	
Settings	app/src/pages/Settings.tsx	app/src/App.tsx:1724-1725 (settings)	app/src/App.tsx:156 (settings)	1,5,76,105,118,124,182,188,227,243,282	11	315,318,320,321,324,489,504,547,613,644,923	355,359,505,510,552,555,595,615,618,646,649,702,713,718,719,764,777,841,844,873,877,903,907,926,929,934,937,983,996	0	yes	yes	
SetupWizard	app/src/pages/SetupWizard.tsx	NONE	NONE	1,76,134	3	224	381,384,420,423,448,451,461,464,468,471,500,505,548,551,555,559,601,604,608,612,637,640,649,699,702,708,712,745,748,756,760	0	yes	yes	embedded via app/src/App.tsx:1894 (rendered as modal overlay via showSetupWizard)
SoftwareFactory	app/src/pages/SoftwareFactory.tsx	app/src/App.tsx:1661-1662 (software-factory)	app/src/App.tsx:189 (software-factory)	1,12,84	3	163,164,244,245	165,172,230,250	0	yes	yes	
SystemMonitor	app/src/pages/SystemMonitor.tsx	app/src/App.tsx:1535-1536 (system-monitor)	app/src/App.tsx:163 (system-monitor)	1,7,145,220	4	NONE	341,352,570	0	yes	yes	
Telemetry	app/src/pages/Telemetry.tsx	app/src/App.tsx:1718-1719 (telemetry)	app/src/App.tsx:223 (telemetry)	1,7,105,132	4	32,395,396,447,468,472,482,490,492	196,198,368,422,424,549,551,556,558	0	yes	yes	
TemporalEngine	app/src/pages/TemporalEngine.tsx	app/src/App.tsx:1682-1683 (temporal)	app/src/App.tsx:208 (temporal)	1,2,7,113,114,123,144	7	335,369,375	204,307,308,313,314,351,383,420	0	yes	yes	
Terminal	app/src/pages/Terminal.tsx	app/src/App.tsx:1529-1530 (terminal)	app/src/App.tsx:155 (terminal)	1,3,99,127,204,257,264,271,356,388,510	11	659	568,569,582,587,594,619,620,630,634,671,672,673,685,689	0	yes	yes	
TimelineViewer	app/src/pages/TimelineViewer.tsx	app/src/App.tsx:1667-1668 (timeline-viewer)	app/src/App.tsx:207 (timeline-viewer)	1,2,87	3	NONE	195,196,231	0	yes	yes	
TimeMachine	app/src/pages/TimeMachine.tsx	app/src/App.tsx:1547-1548 (time-machine)	app/src/App.tsx:206 (time-machine)	1,18,228,234,238,249,485,489	8	901,1485	509,618,619,641,642,873,874,913,914,960,997,998,1011,1012,1227,1228,1278,1279,1438,1439,1455,1456,1498,1499,1516,1517,1578,1580	0	yes	yes	
TokenEconomy	app/src/pages/TokenEconomy.tsx	app/src/App.tsx:1643-1644 (token-economy)	app/src/App.tsx:182 (token-economy)	1,9,139,155,161	5	437	196,198,342,344,452,454	0	yes	yes	
TrustDashboard	app/src/pages/TrustDashboard.tsx	app/src/App.tsx:1514-1515 (trust)	app/src/App.tsx:166 (trust)	1,12,99,401,406	5	271,295,296,308,309,326,341,362	235,237,255,275,297,330,350,367,446,448,452,454	0	yes	yes	
UsageBilling	app/src/pages/UsageBilling.tsx	app/src/App.tsx:1721-1722 (usage-billing)	app/src/App.tsx:222 (usage-billing)	1,8,108	3	321	174,177,184,186,325,327	0	yes	yes	
VoiceAssistant	app/src/pages/VoiceAssistant.tsx	app/src/App.tsx:1583-1584 (voice-assistant)	app/src/App.tsx:159 (voice-assistant)	9,18,349,361,439,489,493,499,583,605,792,804,808	13	35,53,291,311,312,313,314,356,552,588,589,596,931,934,938	734,735,832,833,854,856,887,985,986	0	yes	yes	
Workflows	app/src/pages/Workflows.tsx	app/src/App.tsx:1496-1497 (workflows)	app/src/App.tsx:205 (workflows)	1,26,263	3	NONE	357,359,399,408,435,439,463,487,488,497	0	yes	yes	
Workspaces	app/src/pages/Workspaces.tsx	app/src/App.tsx:1715-1716 (workspaces)	app/src/App.tsx:215 (workspaces)	1,10,235,239	4	365,615	343,345,349,372,374,379,381,424,439,526,528,589,591,636,638,643,645,677,679,697,699,759,761,766,768	0	yes	yes	
WorldSimulation	app/src/pages/WorldSimulation.tsx	app/src/App.tsx:1550-1551 (simulation)	app/src/App.tsx:210 (simulation)	1,13,284,311,318,352,359,552	8	624,634,853,858,1137	587,592,613,680,717,721,777,779,784,787,860,862,900,903,1145	0	yes	yes	
WorldSimulation2	app/src/pages/WorldSimulation2.tsx	app/src/App.tsx:1664-1665 (world-sim)	app/src/App.tsx:184 (world-sim)	1,2,62,76	4	110,112	102,114	0	yes	yes	
```

## UNWIRED COMMANDS
```tsv
command	impl_fn	generate_handler_ref	typescript_binding	frontend_caller
mcp2_server_status	app/src-tauri/src/main.rs:23097	app/src-tauri/src/main.rs:28311	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_server_handle	app/src-tauri/src/main.rs:23104	app/src-tauri/src/main.rs:28312	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_server_list_tools	app/src-tauri/src/main.rs:23112	app/src-tauri/src/main.rs:28313	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_client_add	app/src-tauri/src/main.rs:23119	app/src-tauri/src/main.rs:28314	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_client_remove	app/src-tauri/src/main.rs:23130	app/src-tauri/src/main.rs:28315	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_client_discover	app/src-tauri/src/main.rs:23135	app/src-tauri/src/main.rs:28316	missing in app/src/api/backend.ts	no app/src frontend caller
mcp2_client_call	app/src-tauri/src/main.rs:23143	app/src-tauri/src/main.rs:28317	missing in app/src/api/backend.ts	no app/src frontend caller
```

## CROSS-CRATE WIRING
```tsv
crate	workspace_member	app_dependency	integration_dependency	source_test_markers	cargo_check	cargo_test
nexus-capability-measurement	Cargo.toml:48	app/src-tauri/Cargo.toml:36	tests/integration/Cargo.toml:50	77	PASS	PASS
nexus-governance-oracle	Cargo.toml:49	app/src-tauri/Cargo.toml:37	tests/integration/Cargo.toml:51	12	PASS	PASS
nexus-governance-engine	Cargo.toml:50	app/src-tauri/Cargo.toml:38	tests/integration/Cargo.toml:52	9	PASS	PASS
nexus-governance-evolution	Cargo.toml:51	app/src-tauri/Cargo.toml:39	tests/integration/Cargo.toml:53	7	PASS	PASS
nexus-predictive-router	Cargo.toml:52	app/src-tauri/Cargo.toml:40	tests/integration/Cargo.toml:54	14	PASS	PASS
nexus-token-economy	Cargo.toml:54	app/src-tauri/Cargo.toml:42	tests/integration/Cargo.toml:56	29	PASS	PASS
nexus-browser-agent	Cargo.toml:53	app/src-tauri/Cargo.toml:41	tests/integration/Cargo.toml:55	12	PASS	PASS
nexus-computer-control	Cargo.toml:55	app/src-tauri/Cargo.toml:43	tests/integration/Cargo.toml:57	16	PASS	PASS
nexus-world-simulation	Cargo.toml:56	app/src-tauri/Cargo.toml:44	tests/integration/Cargo.toml:58	18	PASS	PASS
nexus-perception	Cargo.toml:57	app/src-tauri/Cargo.toml:45	tests/integration/Cargo.toml:59	19	PASS	PASS
nexus-agent-memory	Cargo.toml:58	app/src-tauri/Cargo.toml:46	tests/integration/Cargo.toml:60	21	PASS	PASS
nexus-external-tools	Cargo.toml:59	app/src-tauri/Cargo.toml:47	tests/integration/Cargo.toml:61	17	PASS	PASS
nexus-collab-protocol	Cargo.toml:60	app/src-tauri/Cargo.toml:48	tests/integration/Cargo.toml:62	18	PASS	PASS
nexus-software-factory	Cargo.toml:61	app/src-tauri/Cargo.toml:49	tests/integration/Cargo.toml:63	18	PASS	PASS
```

## APPSTATE FIELD AUDIT
```tsv
alias	field_refs	init_refs	status
capability_measurement	app/src-tauri/src/main.rs:927	app/src-tauri/src/main.rs:1217,app/src-tauri/src/main.rs:1459	wired
governance_oracle	NONE	NONE	no dedicated state field/init
governance_engine	app/src-tauri/src/main.rs:939,app/src-tauri/src/main.rs:940	app/src-tauri/src/main.rs:1229,app/src-tauri/src/main.rs:1258,app/src-tauri/src/main.rs:1471,app/src-tauri/src/main.rs:1474	wired
governance_evolution	app/src-tauri/src/main.rs:941	app/src-tauri/src/main.rs:1261,app/src-tauri/src/main.rs:1477	wired
predictive_router	app/src-tauri/src/main.rs:928	app/src-tauri/src/main.rs:1218,app/src-tauri/src/main.rs:1460	wired
token_economy	app/src-tauri/src/main.rs:930	app/src-tauri/src/main.rs:1220,app/src-tauri/src/main.rs:1462	wired
browser_agent	app/src-tauri/src/main.rs:929	app/src-tauri/src/main.rs:1219,app/src-tauri/src/main.rs:1461	wired
computer_control	app/src-tauri/src/main.rs:862,app/src-tauri/src/main.rs:931	app/src-tauri/src/main.rs:1030,app/src-tauri/src/main.rs:1221,app/src-tauri/src/main.rs:1329,app/src-tauri/src/main.rs:1463	wired
world_simulation	app/src-tauri/src/main.rs:932	app/src-tauri/src/main.rs:1222,app/src-tauri/src/main.rs:1464	wired
perception	app/src-tauri/src/main.rs:933	app/src-tauri/src/main.rs:1223,app/src-tauri/src/main.rs:1465	wired
agent_memory	app/src-tauri/src/main.rs:865,app/src-tauri/src/main.rs:934	app/src-tauri/src/main.rs:1033,app/src-tauri/src/main.rs:1224,app/src-tauri/src/main.rs:1332,app/src-tauri/src/main.rs:1466	wired
external_tools	app/src-tauri/src/main.rs:935	app/src-tauri/src/main.rs:1225,app/src-tauri/src/main.rs:1467	wired
collab_protocol	app/src-tauri/src/main.rs:936	app/src-tauri/src/main.rs:1226,app/src-tauri/src/main.rs:1468	wired
software_factory	app/src-tauri/src/main.rs:937	app/src-tauri/src/main.rs:1227,app/src-tauri/src/main.rs:1469	wired
```

## FRONTEND PAGE -> BACKEND -> CRATE TRACE
```text
=== MeasurementDashboard ===
FILE app/src/pages/MeasurementDashboard.tsx
BINDING cmListSessions app/src/api/backend.ts:3317 COMMANDS ['cm_list_sessions'] RUST ['app/src-tauri/src/main.rs:21916']
BINDING cmGetBatteries app/src/api/backend.ts:3343 COMMANDS ['cm_get_batteries'] RUST ['app/src-tauri/src/main.rs:21958']
BINDING cmGetScorecard app/src/api/backend.ts:3310 COMMANDS ['cm_get_scorecard'] RUST ['app/src-tauri/src/main.rs:21905']
BINDING cmStartSession app/src/api/backend.ts:3292 COMMANDS ['cm_start_session'] RUST ['app/src-tauri/src/main.rs:21881']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== MeasurementSession ===
FILE app/src/pages/MeasurementSession.tsx
BINDING cmGetSession app/src/api/backend.ts:3303 COMMANDS ['cm_get_session'] RUST ['app/src-tauri/src/main.rs:21894']
BINDING cmGetGamingFlags app/src/api/backend.ts:3329 COMMANDS ['cm_get_gaming_flags'] RUST ['app/src-tauri/src/main.rs:21936']
BINDING cmListSessions app/src/api/backend.ts:3317 COMMANDS ['cm_list_sessions'] RUST ['app/src-tauri/src/main.rs:21916']
=== MeasurementCompare ===
FILE app/src/pages/MeasurementCompare.tsx
BINDING cmCompareAgents app/src/api/backend.ts:3336 COMMANDS ['cm_compare_agents'] RUST ['app/src-tauri/src/main.rs:21947']
BINDING cmListSessions app/src/api/backend.ts:3317 COMMANDS ['cm_list_sessions'] RUST ['app/src-tauri/src/main.rs:21916']
=== MeasurementBatteries ===
FILE app/src/pages/MeasurementBatteries.tsx
BINDING cmGetBatteries app/src/api/backend.ts:3343 COMMANDS ['cm_get_batteries'] RUST ['app/src-tauri/src/main.rs:21958']
=== CapabilityBoundaryMap ===
FILE app/src/pages/CapabilityBoundaryMap.tsx
BINDING cmGetBoundaryMap app/src/api/backend.ts:3365 COMMANDS ['cm_get_boundary_map'] RUST ['app/src-tauri/src/main.rs:21980']
BINDING cmGetCalibration app/src/api/backend.ts:3370 COMMANDS ['cm_get_calibration'] RUST ['app/src-tauri/src/main.rs:21987']
BINDING cmGetCensus app/src/api/backend.ts:3375 COMMANDS ['cm_get_census'] RUST ['app/src-tauri/src/main.rs:21996']
BINDING cmGetGamingReportBatch app/src/api/backend.ts:3380 COMMANDS ['cm_get_gaming_report_batch'] RUST ['app/src-tauri/src/main.rs:22005']
BINDING cmUploadDarwin app/src/api/backend.ts:3385 COMMANDS ['cm_upload_darwin'] RUST ['app/src-tauri/src/main.rs:22014']
=== ABValidation ===
FILE app/src/pages/ABValidation.tsx
BINDING cmRunAbValidation app/src/api/backend.ts:3420 COMMANDS ['cm_run_ab_validation'] RUST ['app/src-tauri/src/main.rs:22059']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== ModelRouting ===
FILE app/src/pages/ModelRouting.tsx
BINDING routerGetAccuracy app/src/api/backend.ts:3440 COMMANDS ['router_get_accuracy'] RUST ['app/src-tauri/src/main.rs:22127']
BINDING routerGetModels app/src/api/backend.ts:3443 COMMANDS ['router_get_models'] RUST ['app/src-tauri/src/main.rs:22134']
BINDING routerGetFeedback app/src/api/backend.ts:3451 COMMANDS ['router_get_feedback'] RUST ['app/src-tauri/src/main.rs:22152']
BINDING routerEstimateDifficulty app/src/api/backend.ts:3446 COMMANDS ['router_estimate_difficulty'] RUST ['app/src-tauri/src/main.rs:22141']
=== GovernanceOracle ===
FILE app/src/pages/GovernanceOracle.tsx
BINDING oracleStatus app/src/api/backend.ts:3486 COMMANDS ['oracle_status'] RUST ['app/src-tauri/src/main.rs:22239']
BINDING oracleGetAgentBudget app/src/api/backend.ts:3498 COMMANDS ['oracle_get_agent_budget'] RUST ['app/src-tauri/src/main.rs:22313']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== TokenEconomy ===
FILE app/src/pages/TokenEconomy.tsx
BINDING tokenGetAllWallets app/src/api/backend.ts:3510 COMMANDS ['token_get_all_wallets'] RUST ['app/src-tauri/src/main.rs:22380']
BINDING tokenGetLedger app/src/api/backend.ts:3522 COMMANDS ['token_get_ledger'] RUST ['app/src-tauri/src/main.rs:22402']
BINDING tokenGetSupply app/src/api/backend.ts:3529 COMMANDS ['token_get_supply'] RUST ['app/src-tauri/src/main.rs:22415']
BINDING tokenGetPricing app/src/api/backend.ts:3570 COMMANDS ['token_get_pricing'] RUST ['app/src-tauri/src/main.rs:22480']
BINDING tokenCalculateReward app/src/api/backend.ts:3541 COMMANDS ['token_calculate_reward'] RUST ['app/src-tauri/src/main.rs:22432']
BINDING tokenCalculateBurn app/src/api/backend.ts:3533 COMMANDS ['token_calculate_burn'] RUST ['app/src-tauri/src/main.rs:22422']
=== BrowserAgent ===
FILE app/src/pages/BrowserAgent.tsx
BINDING browserCreateSession app/src/api/backend.ts:3455 COMMANDS ['browser_create_session'] RUST ['app/src-tauri/src/main.rs:22161']
BINDING browserExecuteTask app/src/api/backend.ts:3460 COMMANDS ['browser_execute_task'] RUST ['app/src-tauri/src/main.rs:22174']
BINDING browserNavigate app/src/api/backend.ts:3465 COMMANDS ['browser_navigate'] RUST ['app/src-tauri/src/main.rs:22191']
BINDING browserGetContent app/src/api/backend.ts:3470 COMMANDS ['browser_get_content'] RUST ['app/src-tauri/src/main.rs:22209']
BINDING browserCloseSession app/src/api/backend.ts:3474 COMMANDS ['browser_close_session'] RUST ['app/src-tauri/src/main.rs:22217']
BINDING browserGetPolicy app/src/api/backend.ts:3479 COMMANDS ['browser_get_policy'] RUST ['app/src-tauri/src/main.rs:22225']
BINDING browserSessionCount app/src/api/backend.ts:3481 COMMANDS ['browser_session_count'] RUST ['app/src-tauri/src/main.rs:22232']
BINDING browserScreenshot app/src/api/backend.ts:3955 COMMANDS ['browser_screenshot'] RUST ['app/src-tauri/src/main.rs:22200']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== GovernedControl ===
FILE app/src/pages/GovernedControl.tsx
BINDING ccGetActionHistory app/src/api/backend.ts:3587 COMMANDS ['cc_get_action_history'] RUST ['app/src-tauri/src/main.rs:22506']
BINDING ccGetCapabilityBudget app/src/api/backend.ts:3591 COMMANDS ['cc_get_capability_budget'] RUST ['app/src-tauri/src/main.rs:22514']
BINDING ccGetScreenContext app/src/api/backend.ts:3599 COMMANDS ['cc_get_screen_context'] RUST ['app/src-tauri/src/main.rs:22530']
BINDING ccVerifyActionSequence app/src/api/backend.ts:3595 COMMANDS ['cc_verify_action_sequence'] RUST ['app/src-tauri/src/main.rs:22522']
BINDING ccExecuteAction app/src/api/backend.ts:3576 COMMANDS ['cc_execute_action'] RUST ['app/src-tauri/src/main.rs:22487']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== WorldSimulation2 ===
FILE app/src/pages/WorldSimulation2.tsx
BINDING simGetHistory app/src/api/backend.ts:3620 COMMANDS ['sim_get_history'] RUST ['app/src-tauri/src/main.rs:22568']
BINDING simGetPolicy app/src/api/backend.ts:3624 COMMANDS ['sim_get_policy'] RUST ['app/src-tauri/src/main.rs:22576']
BINDING simSubmit app/src/api/backend.ts:3605 COMMANDS ['sim_submit'] RUST ['app/src-tauri/src/main.rs:22540']
BINDING simRun app/src/api/backend.ts:3612 COMMANDS ['sim_run'] RUST ['app/src-tauri/src/main.rs:22552']
BINDING simGetResult app/src/api/backend.ts:3616 COMMANDS ['sim_get_result'] RUST ['app/src-tauri/src/main.rs:22560']
BINDING simGetRisk app/src/api/backend.ts:3628 COMMANDS ['sim_get_risk'] RUST ['app/src-tauri/src/main.rs:22581']
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
=== Perception ===
FILE app/src/pages/Perception.tsx
BINDING perceptionAnalyzeChart app/src/api/backend.ts:3689 COMMANDS ['perception_analyze_chart'] RUST ['app/src-tauri/src/main.rs:22676']
BINDING perceptionDescribe app/src/api/backend.ts:3653 COMMANDS ['perception_describe'] RUST ['app/src-tauri/src/main.rs:22622']
BINDING perceptionExtractData app/src/api/backend.ts:3677 COMMANDS ['perception_extract_data'] RUST ['app/src-tauri/src/main.rs:22658']
BINDING perceptionExtractText app/src/api/backend.ts:3659 COMMANDS ['perception_extract_text'] RUST ['app/src-tauri/src/main.rs:22631']
BINDING perceptionFindUiElements app/src/api/backend.ts:3671 COMMANDS ['perception_find_ui_elements'] RUST ['app/src-tauri/src/main.rs:22650']
BINDING perceptionGetPolicy app/src/api/backend.ts:3695 COMMANDS ['perception_get_policy'] RUST ['app/src-tauri/src/main.rs:22685']
BINDING perceptionInit app/src/api/backend.ts:3645 COMMANDS ['perception_init'] RUST ['app/src-tauri/src/main.rs:22612']
BINDING perceptionQuestion app/src/api/backend.ts:3665 COMMANDS ['perception_question'] RUST ['app/src-tauri/src/main.rs:22640']
BINDING perceptionReadError app/src/api/backend.ts:3683 COMMANDS ['perception_read_error'] RUST ['app/src-tauri/src/main.rs:22668']
=== AgentMemory ===
FILE app/src/pages/AgentMemory.tsx
BINDING listAgents app/src/api/backend.ts:98 COMMANDS ['list_agents'] RUST ['app/src-tauri/src/main.rs:2148']
BINDING memoryBuildContext app/src/api/backend.ts:3732 COMMANDS ['memory_build_context'] RUST ['app/src-tauri/src/main.rs:22750']
BINDING memoryConsolidate app/src/api/backend.ts:3744 COMMANDS ['memory_consolidate'] RUST ['app/src-tauri/src/main.rs:22773']
BINDING memoryDeleteEntry app/src/api/backend.ts:3726 COMMANDS ['memory_delete_entry'] RUST ['app/src-tauri/src/main.rs:22741']
BINDING memoryGetPolicy app/src/api/backend.ts:3760 COMMANDS ['memory_get_policy'] RUST ['app/src-tauri/src/main.rs:22796']
BINDING memoryGetStats app/src/api/backend.ts:3740 COMMANDS ['memory_get_stats'] RUST ['app/src-tauri/src/main.rs:22765']
BINDING memoryLoad app/src/api/backend.ts:3752 COMMANDS ['memory_load'] RUST ['app/src-tauri/src/main.rs:22786']
BINDING memoryQueryEntries app/src/api/backend.ts:3710 COMMANDS ['memory_query_entries'] RUST ['app/src-tauri/src/main.rs:22713']
BINDING memorySave app/src/api/backend.ts:3748 COMMANDS ['memory_save'] RUST ['app/src-tauri/src/main.rs:22781']
BINDING memoryStoreEntry app/src/api/backend.ts:3701 COMMANDS ['memory_store_entry'] RUST ['app/src-tauri/src/main.rs:22692']
=== ExternalTools ===
FILE app/src/pages/ExternalTools.tsx
BINDING toolsExecute app/src/api/backend.ts:3770 COMMANDS ['tools_execute'] RUST ['app/src-tauri/src/main.rs:22810']
BINDING toolsGetAudit app/src/api/backend.ts:3789 COMMANDS ['tools_get_audit'] RUST ['app/src-tauri/src/main.rs:22841']
BINDING toolsGetPolicy app/src/api/backend.ts:3797 COMMANDS ['tools_get_policy'] RUST ['app/src-tauri/src/main.rs:22854']
BINDING toolsGetRegistry app/src/api/backend.ts:3781 COMMANDS ['tools_get_registry'] RUST ['app/src-tauri/src/main.rs:22827']
BINDING toolsRefreshAvailability app/src/api/backend.ts:3785 COMMANDS ['tools_refresh_availability'] RUST ['app/src-tauri/src/main.rs:22834']
BINDING toolsVerifyAudit app/src/api/backend.ts:3793 COMMANDS ['tools_verify_audit'] RUST ['app/src-tauri/src/main.rs:22849']
BINDING getRateLimitStatus app/src/api/backend.ts:3962 COMMANDS ['get_rate_limit_status'] RUST ['app/src-tauri/src/main.rs:27504']
=== Collaboration ===
FILE app/src/pages/Collaboration.tsx
BINDING collabAddParticipant app/src/api/backend.ts:3813 COMMANDS ['collab_add_participant'] RUST ['app/src-tauri/src/main.rs:22882']
BINDING collabCastVote app/src/api/backend.ts:3849 COMMANDS ['collab_cast_vote'] RUST ['app/src-tauri/src/main.rs:22942']
BINDING collabCreateSession app/src/api/backend.ts:3803 COMMANDS ['collab_create_session'] RUST ['app/src-tauri/src/main.rs:22863']
BINDING collabDeclareConsensus app/src/api/backend.ts:3858 COMMANDS ['collab_declare_consensus'] RUST ['app/src-tauri/src/main.rs:22959']
BINDING collabDetectConsensus app/src/api/backend.ts:3868 COMMANDS ['collab_detect_consensus'] RUST ['app/src-tauri/src/main.rs:22976']
BINDING collabGetPatterns app/src/api/backend.ts:3884 COMMANDS ['collab_get_patterns'] RUST ['app/src-tauri/src/main.rs:23006']
BINDING collabGetPolicy app/src/api/backend.ts:3880 COMMANDS ['collab_get_policy'] RUST ['app/src-tauri/src/main.rs:22999']
BINDING collabGetSession app/src/api/backend.ts:3872 COMMANDS ['collab_get_session'] RUST ['app/src-tauri/src/main.rs:22984']
BINDING collabListActive app/src/api/backend.ts:3876 COMMANDS ['collab_list_active'] RUST ['app/src-tauri/src/main.rs:22992']
BINDING collabSendMessage app/src/api/backend.ts:3826 COMMANDS ['collab_send_message'] RUST ['app/src-tauri/src/main.rs:22904']
BINDING collabStart app/src/api/backend.ts:3822 COMMANDS ['collab_start'] RUST ['app/src-tauri/src/main.rs:22899']
BINDING collabCallVote app/src/api/backend.ts:3839 COMMANDS ['collab_call_vote'] RUST ['app/src-tauri/src/main.rs:22925']
=== SoftwareFactory ===
FILE app/src/pages/SoftwareFactory.tsx
BINDING swfAssignMember app/src/api/backend.ts:3896 COMMANDS ['swf_assign_member'] RUST ['app/src-tauri/src/main.rs:23022']
BINDING swfCreateProject app/src/api/backend.ts:3890 COMMANDS ['swf_create_project'] RUST ['app/src-tauri/src/main.rs:23013']
BINDING swfEstimateCost app/src/api/backend.ts:3939 COMMANDS ['swf_estimate_cost'] RUST ['app/src-tauri/src/main.rs:23090']
BINDING swfGetCost app/src/api/backend.ts:3927 COMMANDS ['swf_get_cost'] RUST ['app/src-tauri/src/main.rs:23072']
BINDING swfGetPipelineStages app/src/api/backend.ts:3935 COMMANDS ['swf_get_pipeline_stages'] RUST ['app/src-tauri/src/main.rs:23085']
BINDING swfGetPolicy app/src/api/backend.ts:3931 COMMANDS ['swf_get_policy'] RUST ['app/src-tauri/src/main.rs:23080']
BINDING swfGetProject app/src/api/backend.ts:3919 COMMANDS ['swf_get_project'] RUST ['app/src-tauri/src/main.rs:23057']
BINDING swfListProjects app/src/api/backend.ts:3923 COMMANDS ['swf_list_projects'] RUST ['app/src-tauri/src/main.rs:23065']
BINDING swfStartPipeline app/src/api/backend.ts:3908 COMMANDS ['swf_start_pipeline'] RUST ['app/src-tauri/src/main.rs:23043']
```

## DEAD CODE
### Unused Public Functions
```tsv
function	path	line
suggested_capabilities	crates/nexus-software-factory/src/roles.rs	24
get_artifact	crates/nexus-software-factory/src/project.rs	103
complete_project	crates/nexus-software-factory/src/factory.rs	232
stage_cost	crates/nexus-software-factory/src/economy.rs	10
check_delegation	crates/nexus-token-economy/src/gating.rs	52
refund_escrow	crates/nexus-token-economy/src/wallet.rs	163
record_snapshot	crates/nexus-token-economy/src/supply.rs	35
clear_cache	crates/nexus-perception/src/engine.rs	241
provider_model_id	crates/nexus-perception/src/engine.rs	245
find_clickable_elements	crates/nexus-perception/src/screen.rs	29
read_error	crates/nexus-perception/src/screen.rs	46
ask_about_screen	crates/nexus-perception/src/screen.rs	53
extract_form_data	crates/nexus-perception/src/extraction.rs	16
extract_table_data	crates/nexus-perception/src/extraction.rs	30
read_page	crates/nexus-perception/src/document.rs	8
extract_table	crates/nexus-perception/src/document.rs	26
analyze_chart	crates/nexus-perception/src/document.rs	50
tools_by_category	crates/nexus-external-tools/src/registry.rs	76
filter_by_constraints	crates/nexus-predictive-router/src/cost_optimizer.rs	28
max_difficulty	crates/nexus-predictive-router/src/difficulty_estimator.rs	28
with_llm	crates/nexus-predictive-router/src/difficulty_estimator.rs	81
check_staging	crates/nexus-predictive-router/src/staging.rs	18
can_propose	crates/nexus-collab-protocol/src/roles.rs	14
complete_session	crates/nexus-collab-protocol/src/protocol.rs	51
completed_sessions	crates/nexus-collab-protocol/src/protocol.rs	65
session_cost	crates/nexus-collab-protocol/src/economy.rs	6
with_data	crates/nexus-collab-protocol/src/message.rs	68
with_reasoning	crates/nexus-collab-protocol/src/message.rs	73
with_references	crates/nexus-collab-protocol/src/message.rs	78
is_broadcast	crates/nexus-collab-protocol/src/message.rs	83
register_manifest	crates/nexus-a2a/src/server.rs	47
has_changed	crates/nexus-governance-engine/src/versioning.rs	6
is_valid_successor	crates/nexus-governance-engine/src/versioning.rs	11
restore_fs	crates/nexus-world-simulation/src/sandbox.rs	100
run_batch_evaluation	crates/nexus-capability-measurement/src/tauri_commands.rs	292
get_ab_comparison	crates/nexus-capability-measurement/src/tauri_commands.rs	469
compute_articulation	crates/nexus-capability-measurement/src/scoring/articulation.rs	72
difficulty_description	crates/nexus-capability-measurement/src/battery/difficulty.rs	6
level_description	crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs	9
references_tool_output	crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs	22
acknowledges_limitations	crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs	35
level_description	crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs	9
has_causal_language	crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs	24
avoids_correlation_trap	crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs	40
level_description	crates/nexus-capability-measurement/src/vectors/adaptation.rs	9
shows_epistemic_honesty	crates/nexus-capability-measurement/src/vectors/adaptation.rs	26
distinguishes_source_reliability	crates/nexus-capability-measurement/src/vectors/adaptation.rs	41
level_description	crates/nexus-capability-measurement/src/vectors/planning_coherence.rs	9
has_explicit_dependencies	crates/nexus-capability-measurement/src/vectors/planning_coherence.rs	24
has_rollback_handling	crates/nexus-capability-measurement/src/vectors/planning_coherence.rs	39
append_to_chain	crates/nexus-capability-measurement/src/reporting/audit_trail.rs	18
profile_summary	crates/nexus-capability-measurement/src/reporting/cross_vector.rs	6
detect_anomalies	crates/nexus-capability-measurement/src/reporting/cross_vector.rs	21
keyword_count	crates/nexus-agent-memory/src/index.rs	80
tag_count	crates/nexus-agent-memory/src/index.rs	88
update_importance	crates/nexus-agent-memory/src/store.rs	76
run_stdio	crates/nexus-mcp/src/transport.rs	7
request_channel	crates/nexus-governance-oracle/src/submission.rs	7
episode_type	crates/nexus-memory/src/types.rs	256
is_temporally_valid	crates/nexus-memory/src/types.rs	403
check_balance	crates/nexus-browser-agent/src/economy.rs	13
score_browser_task	crates/nexus-browser-agent/src/measurement.rs	16
is_running	crates/nexus-browser-agent/src/bridge.rs	150
close_agent_sessions	crates/nexus-browser-agent/src/session.rs	128
generate_speculative	crates/nexus-flash-infer/src/speculative.rs	116
set_subprocess_timeout_ms	crates/nexus-computer-control/src/engine.rs	84
```

### Backend Exports Never Called From Pages
```tsv
function	path	line
clearAllAgents	app/src/api/backend.ts	102
createAgent	app/src/api/backend.ts	106
getAgentPerformance	app/src/api/backend.ts	174
getAutoEvolutionLog	app/src/api/backend.ts	178
setAutoEvolutionConfig	app/src/api/backend.ts	182
forceEvolveAgent	app/src/api/backend.ts	196
transcribePushToTalk	app/src/api/backend.ts	208
startJarvisMode	app/src/api/backend.ts	212
stopJarvisMode	app/src/api/backend.ts	216
jarvisStatus	app/src/api/backend.ts	220
detectHardware	app/src/api/backend.ts	224
checkOllama	app/src/api/backend.ts	228
pullOllamaModel	app/src/api/backend.ts	232
runSetupWizard	app/src/api/backend.ts	241
pullModel	app/src/api/backend.ts	248
ensureOllama	app/src/api/backend.ts	257
isOllamaInstalled	app/src/api/backend.ts	264
deleteModel	app/src/api/backend.ts	268
isSetupComplete	app/src/api/backend.ts	277
listAvailableModels	app/src/api/backend.ts	281
analyzeScreen	app/src/api/backend.ts	311
computerControlCaptureScreen	app/src/api/backend.ts	338
computerControlExecuteAction	app/src/api/backend.ts	342
setAgentModel	app/src/api/backend.ts	386
getSystemInfo	app/src/api/backend.ts	411
a2aCrateSendTask	app/src/api/backend.ts	541
a2aCrateGetTask	app/src/api/backend.ts	545
a2aCrateDiscoverAgent	app/src/api/backend.ts	549
getAgentIdentity	app/src/api/backend.ts	559
listIdentities	app/src/api/backend.ts	563
marketplaceInfo	app/src/api/backend.ts	610
getBrowserHistory	app/src/api/backend.ts	634
getAgentActivity	app/src/api/backend.ts	638
startResearch	app/src/api/backend.ts	644
researchAgentAction	app/src/api/backend.ts	651
completeResearch	app/src/api/backend.ts	667
getResearchSession	app/src/api/backend.ts	673
listResearchSessions	app/src/api/backend.ts	679
startBuild	app/src/api/backend.ts	685
buildAppendCode	app/src/api/backend.ts	689
buildAddMessage	app/src/api/backend.ts	701
completeBuild	app/src/api/backend.ts	715
getBuildSession	app/src/api/backend.ts	721
getBuildCode	app/src/api/backend.ts	727
getBuildPreview	app/src/api/backend.ts	731
startLearning	app/src/api/backend.ts	737
getKnowledgeBase	app/src/api/backend.ts	741
getLearningSession	app/src/api/backend.ts	745
learningAgentAction	app/src/api/backend.ts	780
getProviderUsageStats	app/src/api/backend.ts	821
emailSearchMessages	app/src/api/backend.ts	1033
getAgentOutputs	app/src/api/backend.ts	1069
projectGet	app/src/api/backend.ts	1079
assignAgentGoal	app/src/api/backend.ts	1155
stopAgentGoal	app/src/api/backend.ts	1186
startAutonomousLoop	app/src/api/backend.ts	1195
stopAutonomousLoop	app/src/api/backend.ts	1211
getAgentCognitiveStatus	app/src/api/backend.ts	1218
getAgentMemories	app/src/api/backend.ts	1238
agentMemoryRemember	app/src/api/backend.ts	1254
agentMemoryRecall	app/src/api/backend.ts	1272
agentMemoryRecallByType	app/src/api/backend.ts	1286
agentMemoryForget	app/src/api/backend.ts	1301
agentMemoryGetStats	app/src/api/backend.ts	1310
agentMemorySave	app/src/api/backend.ts	1314
agentMemoryClear	app/src/api/backend.ts	1318
getSelfEvolutionMetrics	app/src/api/backend.ts	1339
getSelfEvolutionStrategies	app/src/api/backend.ts	1348
triggerCrossAgentLearning	app/src/api/backend.ts	1357
getHivemindStatus	app/src/api/backend.ts	1374
cancelHivemind	app/src/api/backend.ts	1383
getOsFitness	app/src/api/backend.ts	1579
getFitnessHistory	app/src/api/backend.ts	1583
getRoutingStats	app/src/api/backend.ts	1587
getUiAdaptations	app/src/api/backend.ts	1591
recordPageVisit	app/src/api/backend.ts	1599
recordFeatureUse	app/src/api/backend.ts	1603
overrideSecurityBlock	app/src/api/backend.ts	1607
getOsImprovementLog	app/src/api/backend.ts	1619
getMorningOsBriefing	app/src/api/backend.ts	1623
recordRoutingOutcome	app/src/api/backend.ts	1627
recordOperationTiming	app/src/api/backend.ts	1640
getPerformanceReport	app/src/api/backend.ts	1651
getSecurityEvolutionReport	app/src/api/backend.ts	1655
recordKnowledgeInteraction	app/src/api/backend.ts	1659
getOsDreamStatus	app/src/api/backend.ts	1671
setSelfImproveEnabled	app/src/api/backend.ts	1675
screenshotAnalyze	app/src/api/backend.ts	1681
screenshotGenerateSpec	app/src/api/backend.ts	1685
voiceProjectStart	app/src/api/backend.ts	1697
voiceProjectStop	app/src/api/backend.ts	1701
voiceProjectAddChunk	app/src/api/backend.ts	1705
voiceProjectGetStatus	app/src/api/backend.ts	1712
voiceProjectGetPrompt	app/src/api/backend.ts	1716
voiceProjectUpdateIntent	app/src/api/backend.ts	1720
stressGeneratePersonas	app/src/api/backend.ts	1732
stressGenerateActions	app/src/api/backend.ts	1736
stressEvaluateReport	app/src/api/backend.ts	1740
deployGenerateDockerfile	app/src/api/backend.ts	1746
deployValidateConfig	app/src/api/backend.ts	1750
deployGetCommands	app/src/api/backend.ts	1754
evolverRegisterApp	app/src/api/backend.ts	1760
evolverUnregisterApp	app/src/api/backend.ts	1764
evolverListApps	app/src/api/backend.ts	1768
evolverDetectIssues	app/src/api/backend.ts	1772
freelanceGetStatus	app/src/api/backend.ts	1778
freelanceStartScanning	app/src/api/backend.ts	1782
freelanceStopScanning	app/src/api/backend.ts	1786
freelanceEvaluateJob	app/src/api/backend.ts	1790
freelanceGetRevenue	app/src/api/backend.ts	1794
getLivePreview	app/src/api/backend.ts	1808
publishToMarketplace	app/src/api/backend.ts	1820
installFromMarketplace	app/src/api/backend.ts	1824
schedulerHistory	app/src/api/backend.ts	3075
schedulerRunnerStatus	app/src/api/backend.ts	3086
executeTeamWorkflow	app/src/api/backend.ts	3094
transferAgentFuel	app/src/api/backend.ts	3109
runContentPipeline	app/src/api/backend.ts	3127
flashProfileModel	app/src/api/backend.ts	3142
flashAutoConfigure	app/src/api/backend.ts	3147
flashListSessions	app/src/api/backend.ts	3173
flashGetMetrics	app/src/api/backend.ts	3188
flashEstimatePerformance	app/src/api/backend.ts	3200
flashCatalogRecommend	app/src/api/backend.ts	3207
flashCatalogSearch	app/src/api/backend.ts	3212
flashDownloadModel	app/src/api/backend.ts	3248
flashDownloadMulti	app/src/api/backend.ts	3256
flashDeleteLocalModel	app/src/api/backend.ts	3263
flashAvailableDiskSpace	app/src/api/backend.ts	3267
flashGetModelDir	app/src/api/backend.ts	3271
cmGetProfile	app/src/api/backend.ts	3322
cmTriggerFeedback	app/src/api/backend.ts	3348
cmEvaluateResponse	app/src/api/backend.ts	3355
cmExecuteValidationRun	app/src/api/backend.ts	3392
cmListValidationRuns	app/src/api/backend.ts	3400
cmGetValidationRun	app/src/api/backend.ts	3405
cmThreeWayComparison	app/src/api/backend.ts	3410
routerRouteTask	app/src/api/backend.ts	3427
routerRecordOutcome	app/src/api/backend.ts	3431
oracleVerifyToken	app/src/api/backend.ts	3491
tokenGetWallet	app/src/api/backend.ts	3506
tokenCreateWallet	app/src/api/backend.ts	3514
tokenCalculateSpawn	app/src/api/backend.ts	3548
tokenCreateDelegation	app/src/api/backend.ts	3555
tokenGetDelegations	app/src/api/backend.ts	3566
simBranch	app/src/api/backend.ts	3632
memoryGetEntry	app/src/api/backend.ts	3720
memoryListAgents	app/src/api/backend.ts	3756
toolsListAvailable	app/src/api/backend.ts	3766
swfSubmitArtifact	app/src/api/backend.ts	3912
governanceEngineGetRules	app/src/api/backend.ts	4046
governanceEngineEvaluate	app/src/api/backend.ts	4050
governanceEngineGetAuditLog	app/src/api/backend.ts	4062
governanceEvolutionGetThreatModel	app/src/api/backend.ts	4068
governanceEvolutionRunAttackCycle	app/src/api/backend.ts	4072
```

### Orphan Modules
```text
NONE (0 orphan modules detected outside tests/)
```

## MOCK DATA LOCATIONS
### Confirmed User-Visible Fake/Fallback Behavior
```tsv
severity	location	detail
MAJOR	app/src/components/browser/BuildMode.tsx:81-182; 299-416	Mock build session/code/conversation fallback
MAJOR	app/src/voice/PushToTalk.ts:70-75; 97-102	mock-whisper source and hardcoded fallback transcript
MAJOR	app/src/App.tsx:1895-1904	Setup wizard returns synthetic hardware profile
```

### Raw Mock / Placeholder Grep Hits (heuristic)
```tsv
scope	path	line	snippet
frontend	app/src/voice/PushToTalk.ts	5	source: "tauri-stt" | "web-speech" | "mock-whisper";
frontend	app/src/voice/PushToTalk.ts	74	source: "mock-whisper"
frontend	app/src/voice/PushToTalk.ts	101	source: "mock-whisper"
frontend	app/src/App.tsx	142	type RuntimeMode = "desktop" | "mock";
frontend	app/src/App.tsx	386	// Demo agent/chat functions removed — no fake data is served when desktop runtime
frontend	app/src/App.tsx	426	const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("mock");
frontend	app/src/App.tsx	433	const [selectedModel, setSelectedModel] = useState("mock");
frontend	app/src/App.tsx	505	setRuntimeMode("mock");
frontend	app/src/App.tsx	560	`Connected to desktop backend. Default model: ${normalizedConfig.llm.default_model || "mock-1"}.`
frontend	app/src/App.tsx	570	setRuntimeMode("mock");
frontend	app/src/App.tsx	714	const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock";
frontend	app/src/App.tsx	967	const model = selectedModel === "mock" ? getModelForAgent(selectedAgent) : selectedModel;
frontend	app/src/App.tsx	968	const isOllamaModel = model.startsWith("ollama/") || (!model.includes("/") && model !== "mock");
frontend	app/src/App.tsx	1227	// No desktop runtime — show clear message instead of fake responses
frontend	app/src/App.tsx	1806	{connectionStatus === "connected" ? "live" : "mock"}
frontend	app/src/types.ts	229	export type ConnectionStatus = "connected" | "mock";
frontend	app/src/pages/VoiceAssistant.tsx	35	sampleRate: number;
frontend	app/src/pages/VoiceAssistant.tsx	53	sampleRate: number;
frontend	app/src/pages/VoiceAssistant.tsx	291	function resampleTo16Khz(input: Float32Array, inputRate: number): Float32Array {
frontend	app/src/pages/VoiceAssistant.tsx	311	function encodePcm16Base64(samples: Float32Array): string {
frontend	app/src/pages/VoiceAssistant.tsx	312	const bytes = new Uint8Array(samples.length * 2);
frontend	app/src/pages/VoiceAssistant.tsx	313	for (let index = 0; index < samples.length; index += 1) {
frontend	app/src/pages/VoiceAssistant.tsx	314	const clamped = Math.max(-1, Math.min(1, samples[index]));
frontend	app/src/pages/VoiceAssistant.tsx	356	sampleRate: 16000,
frontend	app/src/pages/VoiceAssistant.tsx	552	sampleRate: audioContext.sampleRate,
frontend	app/src/pages/VoiceAssistant.tsx	588	const resampled = resampleTo16Khz(merged, session.sampleRate);
frontend	app/src/pages/VoiceAssistant.tsx	589	if (resampled.length === 0) {
frontend	app/src/pages/VoiceAssistant.tsx	596	const transcriptionRaw = await voiceTranscribe(encodePcm16Base64(resampled));
frontend	app/src/pages/VoiceAssistant.tsx	934	value={settings.sampleRate}
frontend	app/src/pages/VoiceAssistant.tsx	938	sampleRate: parseInt(e.target.value, 10),
frontend	app/src/pages/EmailClient.tsx	402	<input className="ec-search" placeholder="Search emails..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
frontend	app/src/pages/EmailClient.tsx	472	<input value={composeTo} onChange={e => setComposeTo(e.target.value)} placeholder="recipient@example.com" />
frontend	app/src/pages/EmailClient.tsx	476	<input value={composeSubject} onChange={e => setComposeSubject(e.target.value)} placeholder="Subject" />
frontend	app/src/pages/EmailClient.tsx	479	<textarea className="ec-compose-body" value={composeBody} onChange={e => setComposeBody(e.target.value)} placeholder="Write your email..." />
frontend	app/src/pages/ComputerControl.tsx	483	placeholder="App name (e.g. Firefox)"
frontend	app/src/pages/ComputerControl.tsx	484	className="flex-1 rounded-xl border border-violet-500/20 bg-slate-950/70 px-3 py-2 text-sm text-cyan-50 placeholder:text-cyan-100/30"
frontend	app/src/pages/ComputerControl.tsx	507	placeholder='{"type":"click","x":100,"y":200}'
frontend	app/src/pages/Messaging.tsx	292	placeholder={`${platform.label} token`}
frontend	app/src/pages/Messaging.tsx	358	<input value={replyChannel} onChange={e => setReplyChannel(e.target.value)} placeholder="Channel / Chat ID" style={{ flex: "0 0 140px", background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} />
frontend	app/src/pages/Messaging.tsx	359	<input value={replyText} onChange={e => setReplyText(e.target.value)} placeholder="Type a message..." style={{ flex: 1, background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} onKeyDown={e => e.key === 'Enter' && handleSendReply()} />
frontend	app/src/pages/Terminal.tsx	659	placeholder="Type a command..."
frontend	app/src/pages/AppStore.tsx	212	placeholder="Search by name, description, or capability..."
frontend	app/src/pages/AppStore.tsx	377	placeholder="Search nexus-agent repos on GitLab..."
frontend	app/src/pages/ApiClient.tsx	86	/* Collections are loaded from backend on mount, not hardcoded */
frontend	app/src/pages/ApiClient.tsx	378	<input className="ac-url-input" value={activeReq.url} onChange={e => updateReq({ url: e.target.value })} placeholder="Enter request URL..." onKeyDown={e => e.key === "Enter" && sendRequest()} />
frontend	app/src/pages/ApiClient.tsx	412	<input className="ac-kv-key" value={kv.key} onChange={e => updateKV("params", i, { key: e.target.value })} placeholder="Key" />
frontend	app/src/pages/ApiClient.tsx	413	<input className="ac-kv-value" value={kv.value} onChange={e => updateKV("params", i, { value: e.target.value })} placeholder="Value" />
frontend	app/src/pages/ApiClient.tsx	430	<input className="ac-kv-key" value={kv.key} onChange={e => updateKV("headers", i, { key: e.target.value })} placeholder="Header name" />
frontend	app/src/pages/ApiClient.tsx	431	<input className="ac-kv-value" value={kv.value} onChange={e => updateKV("headers", i, { value: e.target.value })} placeholder="Value" />
frontend	app/src/pages/ApiClient.tsx	450	<textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder='{"key": "value"}' spellCheck={false} />
frontend	app/src/pages/ApiClient.tsx	453	<textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder="Raw text body..." spellCheck={false} />
frontend	app/src/pages/ApiClient.tsx	464	<input className="ac-kv-key" value={kv.key} onChange={e => updateKV("bodyForm", i, { key: e.target.value })} placeholder="Key" />
frontend	app/src/pages/ApiClient.tsx	465	<input className="ac-kv-value" value={kv.value} onChange={e => updateKV("bodyForm", i, { value: e.target.value })} placeholder="Value" />
frontend	app/src/pages/ApiClient.tsx	488	<input className="ac-auth-input" value={activeReq.authToken} onChange={e => updateReq({ authToken: e.target.value })} placeholder="Bearer token..." type="password" />
frontend	app/src/pages/ApiClient.tsx	495	<input className="ac-auth-input" value={activeReq.authUser} onChange={e => updateReq({ authUser: e.target.value })} placeholder="Username" />
frontend	app/src/pages/ApiClient.tsx	497	<input className="ac-auth-input" value={activeReq.authPass} onChange={e => updateReq({ authPass: e.target.value })} placeholder="Password" type="password" />
frontend	app/src/pages/ApiClient.tsx	503	<input className="ac-auth-input" value={activeReq.authKeyName} onChange={e => updateReq({ authKeyName: e.target.value })} placeholder="e.g. x-api-key" />
frontend	app/src/pages/ApiClient.tsx	505	<input className="ac-auth-input" value={activeReq.authKeyValue} onChange={e => updateReq({ authKeyValue: e.target.value })} placeholder="API key value" type="password" />
frontend	app/src/pages/AgentBrowser.tsx	75	// Track governance stats — fuel is estimated per-action, not hardcoded
frontend	app/src/pages/DeployPipeline.tsx	483	<input value={newName} onChange={e => setNewName(e.target.value)} placeholder="my-app" />
frontend	app/src/pages/DeployPipeline.tsx	493	<input value={newSourceDir} onChange={e => setNewSourceDir(e.target.value)} placeholder="." />
frontend	app/src/pages/DeployPipeline.tsx	765	<input value={bundleOutputPath} onChange={e => setBundleOutputPath(e.target.value)} placeholder="Output path"
frontend	app/src/pages/DeployPipeline.tsx	767	<input value={bundleComponents} onChange={e => setBundleComponents(e.target.value)} placeholder="Components (optional, comma sep)"
frontend	app/src/pages/DeployPipeline.tsx	789	<input value={validatePath} onChange={e => setValidatePath(e.target.value)} placeholder="Bundle path"
frontend	app/src/pages/DeployPipeline.tsx	812	<input value={installBundlePath} onChange={e => setInstallBundlePath(e.target.value)} placeholder="Bundle path"
frontend	app/src/pages/DeployPipeline.tsx	814	<input value={installDir} onChange={e => setInstallDir(e.target.value)} placeholder="Install directory"
frontend	app/src/pages/LearningCenter.tsx	1130	placeholder="Type your response or describe what you built..."
frontend	app/src/pages/ClusterStatus.tsx	242	placeholder="Task description"
frontend	app/src/pages/ClusterStatus.tsx	247	placeholder="Agent IDs (comma sep)"
frontend	app/src/pages/ClusterStatus.tsx	266	placeholder="Agent ID"
frontend	app/src/pages/ClusterStatus.tsx	271	placeholder="Target peer ID"
frontend	app/src/pages/ModelRouting.tsx	91	placeholder="Enter task text to estimate difficulty..."
frontend	app/src/pages/Scheduler.tsx	239	<input value={name} onChange={(e) => setName(e.target.value)} style={inputStyle} placeholder="my-scheduled-task" />
frontend	app/src/pages/Scheduler.tsx	243	<input value={agentDid} onChange={(e) => setAgentDid(e.target.value)} style={inputStyle} placeholder="agent-uuid" />
frontend	app/src/pages/Collaboration.tsx	201	<input placeholder="Title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} />
frontend	app/src/pages/Collaboration.tsx	202	<input placeholder="Goal" value={goal} onChange={(e) => setGoal(e.target.value)} style={inputStyle} />
frontend	app/src/pages/Collaboration.tsx	206	<input placeholder="Lead Agent ID" value={leadAgent} onChange={(e) => setLeadAgent(e.target.value)} style={inputStyle} />
frontend	app/src/pages/Collaboration.tsx	272	<input placeholder="Agent ID" value={newAgentId} onChange={(e) => setNewAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/Collaboration.tsx	317	<input placeholder="As agent..." value={msgFrom} onChange={(e) => setMsgFrom(e.target.value)} style={{ ...inputStyle, width: 140 }} />
frontend	app/src/pages/Collaboration.tsx	332	<textarea placeholder="Message..." value={msgText} onChange={(e) => setMsgText(e.target.value)} rows={2} style={{ ...inputStyle, marginTop: 6, resize: "vertical" }} />
frontend	app/src/pages/ProjectManager.tsx	328	<input className="pm-search" placeholder="Search tasks..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
frontend	app/src/pages/ProjectManager.tsx	359	<input className="pm-modal-input" placeholder="Task title..." value={newTaskTitle} onChange={e => setNewTaskTitle(e.target.value)} autoFocus onKeyDown={e => e.key === "Enter" && createTask()} />
frontend	app/src/pages/ProjectManager.tsx	619	<textarea className="pm-detail-desc" value={selectedTask.description} onChange={e => { updateTask(selectedTask.id, { description: e.target.value }); setSelectedTask({ ...selectedTask, description: e.target.value }); }} placeholder="Add description..." />
frontend	app/src/components/browser/ResearchMode.tsx	96	// continue in mock mode
frontend	app/src/components/browser/ResearchMode.tsx	292	<div className="browser-iframe-shell browser-iframe-shell--placeholder">
frontend	app/src/components/browser/ResearchMode.tsx	293	<div className="browser-placeholder">
frontend	app/src/components/browser/ResearchMode.tsx	294	<span className="browser-placeholder-icon">⌁</span>
frontend	app/src/components/browser/ResearchMode.tsx	295	<span className="browser-placeholder-text">Research Mode</span>
frontend	app/src/components/browser/ResearchMode.tsx	296	<span className="browser-placeholder-hint">
frontend	app/src/components/browser/ResearchMode.tsx	326	placeholder="Enter research topic..."
frontend	app/src/pages/ApprovalCenter.tsx	298	placeholder="Reason (optional)"
frontend	app/src/components/chat/History.tsx	52	placeholder="Search transmissions..."
frontend	app/src/pages/SoftwareFactory.tsx	163	<input placeholder="Project title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} />
frontend	app/src/pages/SoftwareFactory.tsx	164	<textarea placeholder="Describe what to build..." value={userRequest} onChange={(e) => setUserRequest(e.target.value)} rows={3} style={{ ...inputStyle, resize: "vertical" }} />
frontend	app/src/pages/SoftwareFactory.tsx	244	<input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/SoftwareFactory.tsx	245	<input placeholder="Name" value={agentName} onChange={(e) => setAgentName(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/FileManager.tsx	373	<input className="fm-search-input" placeholder="Filter files by name..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} />
frontend	app/src/pages/FileManager.tsx	383	<input className="fm-new-item-input" placeholder={newItemType === "dir" ? "folder-name" : "filename.ext"} value={newItemName} onChange={(e) => setNewItemName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateItem(); if (e.key === "Escape") setNewItemType(null); }} autoFocus />
frontend	app/src/pages/FileManager.tsx	482	<div className="fm-preview-image-placeholder">
frontend	app/src/pages/ExternalTools.tsx	184	<input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, width: 140 }} />
frontend	app/src/pages/ExternalTools.tsx	191	placeholder='{"action": "list_repos", "user": "octocat"}'
frontend	app/src/pages/AiChatHub.tsx	187	if (lower.includes("no llm provider") || lower.includes("mock")) {
frontend	app/src/pages/AiChatHub.tsx	865	// Create placeholder assistant message for streaming
frontend	app/src/pages/AiChatHub.tsx	1178	<input className="ch-search" placeholder="Search..." value={view === "history" ? historySearch : searchQuery} onChange={e => view === "history" ? setHistorySearch(e.target.value) : setSearchQuery(e.target.value)} />
frontend	app/src/pages/AiChatHub.tsx	1542	<textarea ref={inputRef} className="ch-input" value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} placeholder={`Message ${activeModel?.name ?? "AI"}...`} rows={1} />
frontend	app/src/pages/AiChatHub.tsx	1583	<textarea value={comparePrompt} onChange={e => setComparePrompt(e.target.value)} placeholder="Enter a prompt to compare responses..." rows={3} />
frontend	app/src/pages/AiChatHub.tsx	1677	placeholder={builderStarted ? "Describe what you want to change..." : "Describe what you want to build..."}
frontend	app/src/pages/AiChatHub.tsx	1777	placeholder={`Enter ${meta?.label ?? provider} API key...`}
frontend	app/src/pages/FlashInference.tsx	637	<input ref={inputRef} value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} disabled={!hasAnyModel || generating} placeholder={!hasAnyModel ? "Load a model first..." : generating ? "Generating..." : mode === "auto" ? "Type a message (auto-routed)..." : `Type a message (${MODE_LABELS[mode]})...`} style={{ flex:1, background:"#0d1117", border:"1px solid #1e3a5f", borderRadius:"8px", color:"#e5e7eb", padding:"10px 14px", fontSize:"13px", outline:"none", opacity:!hasAnyModel?0.5:1 }}/>
frontend	app/src/components/browser/BuildMode.tsx	81	/** Generate mock code for a build description, split into typeable chunks. */
frontend	app/src/components/browser/BuildMode.tsx	304	// fall through to mock
frontend	app/src/components/browser/BuildMode.tsx	373	// continue mock
frontend	app/src/components/browser/BuildMode.tsx	415	// mock complete
frontend	app/src/components/browser/BuildMode.tsx	437	placeholder="Describe what to build... (e.g. landing page with hero section and feature cards)"
frontend	app/src/components/browser/BuildMode.tsx	487	: '<span class="build-code-placeholder">Code will appear here as agents write...</span>',
frontend	app/src/components/browser/BuildMode.tsx	510	<div className="build-preview-placeholder">
frontend	app/src/components/browser/BuildMode.tsx	511	<span className="build-preview-placeholder-icon">◇</span>
frontend	app/src/pages/AdminUsers.tsx	104	placeholder="Filter users..."
frontend	app/src/components/agents/CreateAgent.tsx	66	{ value: "mock", label: "mock (Testing)" },
frontend	app/src/components/agents/CreateAgent.tsx	180	placeholder="Agent name"
frontend	app/src/components/agents/CreateAgent.tsx	186	placeholder="Describe mission objectives, constraints, and expected outputs..."
frontend	app/src/components/agents/CreateAgent.tsx	293	placeholder="*/10 * * * *"
frontend	app/src/components/agents/CreateAgent.tsx	300	placeholder="What should this agent do on each scheduled run?"
frontend	app/src/pages/TrustDashboard.tsx	271	placeholder="Agent DID"
frontend	app/src/pages/TrustDashboard.tsx	295	<input className="td-rep-input" placeholder="DID" value={regDid} onChange={(e) => setRegDid(e.target.value)} />
frontend	app/src/pages/TrustDashboard.tsx	296	<input className="td-rep-input" placeholder="Name" value={regName} onChange={(e) => setRegName(e.target.value)} />
frontend	app/src/pages/TrustDashboard.tsx	308	<input className="td-rep-input" placeholder="Target DID" value={rateDid} onChange={(e) => setRateDid(e.target.value)} />
frontend	app/src/pages/TrustDashboard.tsx	309	<input className="td-rep-input" placeholder="Rater DID" value={raterDid} onChange={(e) => setRaterDid(e.target.value)} />
frontend	app/src/pages/TrustDashboard.tsx	326	placeholder="Comment (optional)"
frontend	app/src/pages/TrustDashboard.tsx	341	<input className="td-rep-input" placeholder="Agent DID" value={taskDid} onChange={(e) => setTaskDid(e.target.value)} />
frontend	app/src/pages/TrustDashboard.tsx	362	placeholder="Paste exported JSON here..."
frontend	app/src/pages/AgentMemory.tsx	200	placeholder="Memory summary..."
frontend	app/src/pages/AgentMemory.tsx	206	<input placeholder="Tags (comma-separated)" value={newTags} onChange={(e) => setNewTags(e.target.value)} style={inputStyle} />
frontend	app/src/pages/AgentMemory.tsx	208	<input placeholder="Domain" value={newDomain} onChange={(e) => setNewDomain(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/AgentMemory.tsx	230	<input placeholder="Search query..." value={queryText} onChange={(e) => setQueryText(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/AgentMemory.tsx	243	<input placeholder="Task description..." value={contextTask} onChange={(e) => setContextTask(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
frontend	app/src/pages/UsageBilling.tsx	321	placeholder="Threshold in USD (e.g. 50.00)"
frontend	app/src/components/browser/BrowserToolbar.tsx	152	placeholder="Enter URL... (Ctrl+L to focus)"
frontend	app/src/pages/NotesApp.tsx	430	<input id="na-search" className="na-search-input" placeholder="Search notes..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
frontend	app/src/pages/NotesApp.tsx	534	<input className="na-title-input" value={selectedNote.title} onChange={e => updateNote(selectedNote.id, { title: e.target.value })} placeholder="Note title..." />
frontend	app/src/pages/NotesApp.tsx	583	placeholder="Start writing... (Markdown supported)"
frontend	app/src/pages/Agents.tsx	557	placeholder="Search agents by name, capability, or description…"
frontend	app/src/pages/Agents.tsx	1159	placeholder="Describe a goal for this agent..."
frontend	app/src/pages/Identity.tsx	619	placeholder='Paste proof JSON to verify...'
frontend	app/src/pages/Identity.tsx	722	placeholder="Peer address (e.g. 192.168.1.50:9090)"
frontend	app/src/pages/Identity.tsx	728	placeholder="Peer name (optional)"
frontend	app/src/pages/Identity.tsx	788	placeholder="address:port"
frontend	app/src/pages/TemporalEngine.tsx	335	placeholder="e.g. Design database schema" style={inputStyle} />
frontend	app/src/pages/TemporalEngine.tsx	369	placeholder="e.g. Build a web scraper for news articles" style={inputStyle} />
frontend	app/src/pages/TemporalEngine.tsx	375	placeholder="agent-1, agent-2" style={inputStyle} />
frontend	app/src/pages/BrowserAgent.tsx	115	<input style={{ ...inputStyle, flex: 1 }} placeholder="Enter browser task..." value={taskInput}
frontend	app/src/pages/BrowserAgent.tsx	124	<input style={{ ...inputStyle, flex: 1 }} placeholder="URL..." value={urlInput}
frontend	app/src/pages/Settings.tsx	315	const samples = new Uint8Array(analyser.fftSize);
frontend	app/src/pages/Settings.tsx	318	analyser.getByteTimeDomainData(samples);
frontend	app/src/pages/Settings.tsx	320	for (let index = 0; index < samples.length; index += 1) {
frontend	app/src/pages/Settings.tsx	321	const normalized = (samples[index] - 128) / 128;
frontend	app/src/pages/Settings.tsx	324	const rms = Math.sqrt(sum / samples.length);
frontend	app/src/pages/Settings.tsx	489	{!p.available && p.name !== "mock" && <span style={{ color: "#ff5252", marginLeft: 6, fontSize: "0.75rem" }}>{p.error_hint || "Unavailable"}</span>}
frontend	app/src/pages/Settings.tsx	504	{!p.is_paid && p.name !== "mock" && <span className="st-badge st-badge-green" style={{ fontSize: "0.7rem", padding: "2px 6px" }}>Free</span>}
frontend	app/src/pages/Settings.tsx	547	placeholder={`Enter ${entry.label} API key`}
frontend	app/src/pages/Settings.tsx	613	placeholder={`Enter ${key.label} key`}
frontend	app/src/pages/Settings.tsx	644	placeholder={`Enter ${key.label}`}
frontend	app/src/pages/Settings.tsx	923	placeholder={'{\n  "tool": "tool_name",\n  "args": {}\n}'}
frontend	app/src/pages/SetupWizard.tsx	224	// Fallback for mock mode:
frontend	app/src/pages/WorldSimulation2.tsx	110	<input placeholder="Scenario description..." value={scenarioDesc} onChange={(e) => setScenarioDesc(e.target.value)} style={{ flex: 1, padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }} />
frontend	app/src/pages/WorldSimulation2.tsx	112	<textarea placeholder='Actions JSON array' value={actionsJson} onChange={(e) => setActionsJson(e.target.value)} rows={3} style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", fontFamily: "monospace", fontSize: 12, boxSizing: "border-box", resize: "vertical", marginBottom: 8 }} />
frontend	app/src/pages/TokenEconomy.tsx	437	placeholder="Filter by agent ID..."
frontend	app/src/pages/Chat.tsx	136	label: selectedModel === "mock" ? "Browser runtime selection" : selectedModel,
frontend	app/src/pages/Chat.tsx	489	placeholder="Transmit directive to NexusOS..."
frontend	app/src/pages/AgentDnaLab.tsx	851	placeholder="Strategy name"
frontend	app/src/pages/AgentDnaLab.tsx	857	placeholder='Parameters JSON, e.g. {"mutation_rate": 0.1, "crossover": "uniform"}'
frontend	app/src/pages/AgentDnaLab.tsx	875	placeholder="Task description"
frontend	app/src/pages/AgentDnaLab.tsx	912	placeholder="e.g. I need an agent that can manage Kubernetes clusters and auto-scale pods"
frontend	app/src/pages/AgentDnaLab.tsx	929	placeholder="User request"
frontend	app/src/pages/AgentDnaLab.tsx	936	placeholder="LLM response (optional)"
frontend	app/src/pages/AgentDnaLab.tsx	954	placeholder='Spec JSON, e.g. {"name": "k8s-agent", "capabilities": ["kubernetes", "scaling"]}'
frontend	app/src/pages/AgentDnaLab.tsx	961	placeholder="System prompt"
frontend	app/src/pages/AgentDnaLab.tsx	989	placeholder="Agent name"
frontend	app/src/pages/AgentDnaLab.tsx	1006	placeholder="Spec JSON"
frontend	app/src/pages/AgentDnaLab.tsx	1014	placeholder="Missing capabilities (comma-separated)"
frontend	app/src/pages/KnowledgeGraph.tsx	522	placeholder="Ask anything about your files..."
frontend	app/src/pages/KnowledgeGraph.tsx	658	placeholder="Enter a topic..."
frontend	app/src/pages/KnowledgeGraph.tsx	680	placeholder="File path (e.g. /home/user/doc.md)"
frontend	app/src/pages/KnowledgeGraph.tsx	702	placeholder="File path (e.g. /home/user/doc.md)"
frontend	app/src/pages/KnowledgeGraph.tsx	749	placeholder="Search query..."
frontend	app/src/pages/KnowledgeGraph.tsx	756	placeholder="Time range (start,end)"
frontend	app/src/pages/KnowledgeGraph.tsx	762	placeholder="Source filter (csv)"
frontend	app/src/pages/KnowledgeGraph.tsx	768	placeholder="Max results"
frontend	app/src/pages/KnowledgeGraph.tsx	799	placeholder="Content to ingest..."
frontend	app/src/pages/KnowledgeGraph.tsx	806	placeholder='Metadata JSON e.g. {"tag":"notes"}'
frontend	app/src/pages/KnowledgeGraph.tsx	827	placeholder="Entry ID to delete"
frontend	app/src/pages/KnowledgeGraph.tsx	844	placeholder="Older than N days"
frontend	app/src/pages/WorldSimulation.tsx	624	placeholder="Name this simulation"
frontend	app/src/pages/WorldSimulation.tsx	634	placeholder="Paste the raw seed material here..."
frontend	app/src/pages/WorldSimulation.tsx	853	placeholder="Inject variable"
frontend	app/src/pages/WorldSimulation.tsx	858	placeholder="Value"
frontend	app/src/pages/WorldSimulation.tsx	1137	placeholder="Why did you make your last decision?"
frontend	app/src/pages/Civilization.tsx	788	placeholder='Propose a rule, for example: "Max 10% token budget per agent"'
frontend	app/src/pages/Civilization.tsx	910	placeholder="Describe the dispute..."
frontend	app/src/pages/Civilization.tsx	987	placeholder="Rule text..."
frontend	app/src/pages/Civilization.tsx	1059	placeholder="Issue description..."
frontend	app/src/pages/Civilization.tsx	1132	<input type="number" value={earnAmount} onChange={(e) => setEarnAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1133	<input value={earnDesc} onChange={(e) => setEarnDesc(e.target.value)} placeholder="Description" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1142	<input type="number" value={spendAmount} onChange={(e) => setSpendAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1149	<input value={spendDesc} onChange={(e) => setSpendDesc(e.target.value)} placeholder="Description" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1159	<input type="number" value={transferAmount} onChange={(e) => setTransferAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1160	<input value={transferDesc} onChange={(e) => setTransferDesc(e.target.value)} placeholder="Description" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1189	<input value={contractDesc} onChange={(e) => setContractDesc(e.target.value)} placeholder="Description" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1190	<input value={contractCriteria} onChange={(e) => setContractCriteria(e.target.value)} placeholder='Criteria JSON (e.g. {"quality":"high"})' style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1192	<input type="number" value={contractReward} onChange={(e) => setContractReward(e.target.value)} placeholder="Reward" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1193	<input type="number" value={contractPenalty} onChange={(e) => setContractPenalty(e.target.value)} placeholder="Penalty" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1195	<input type="number" value={contractDeadline} onChange={(e) => setContractDeadline(e.target.value)} placeholder="Deadline (epoch, optional)" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1204	<input value={completeContractId} onChange={(e) => setCompleteContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1209	<input value={completeEvidence} onChange={(e) => setCompleteEvidence(e.target.value)} placeholder="Evidence (optional)" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1218	<input value={disputeContractId} onChange={(e) => setDisputeContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1219	<textarea value={disputeContractReason} onChange={(e) => setDisputeContractReason(e.target.value)} placeholder="Reason for dispute..." style={textareaStyle} />
frontend	app/src/pages/Civilization.tsx	1270	<input value={planName} onChange={(e) => setPlanName(e.target.value)} placeholder="Plan name" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1271	<input type="number" value={planPrice} onChange={(e) => setPlanPrice(e.target.value)} placeholder="Price (cents)" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1277	<input value={planFeatures} onChange={(e) => setPlanFeatures(e.target.value)} placeholder="Features (comma-separated)" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1286	<input value={invoicePlanId} onChange={(e) => setInvoicePlanId(e.target.value)} placeholder="Plan ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1287	<input value={invoiceBuyerId} onChange={(e) => setInvoiceBuyerId(e.target.value)} placeholder="Buyer ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1296	<input value={payInvoiceId} onChange={(e) => setPayInvoiceId(e.target.value)} placeholder="Invoice ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1305	<input value={payoutDevId} onChange={(e) => setPayoutDevId(e.target.value)} placeholder="Developer ID" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1307	<input type="number" value={payoutAmount} onChange={(e) => setPayoutAmount(e.target.value)} placeholder="Amount (cents)" style={inputStyle} />
frontend	app/src/pages/Civilization.tsx	1308	<input value={payoutPeriod} onChange={(e) => setPayoutPeriod(e.target.value)} placeholder="Period (e.g. 2026-03)" style={inputStyle} />
frontend	app/src/pages/Workspaces.tsx	365	placeholder="e.g. production, staging, team-alpha"
frontend	app/src/pages/Workspaces.tsx	615	placeholder="e.g. oidc:alice or local:nexus"
frontend	app/src/pages/ModelHub.tsx	590	placeholder="Search GGUF models on HuggingFace..."
frontend	app/src/pages/ModelHub.tsx	1526	placeholder="Peer address (e.g. 192.168.1.100:9090)"
frontend	app/src/pages/ModelHub.tsx	1543	placeholder="Peer name (e.g. Office Desktop)"
frontend	app/src/pages/ModelHub.tsx	1595	placeholder="Peer address"
frontend	app/src/pages/ModelHub.tsx	1612	placeholder="Model ID (e.g. TheBloke/Llama-2-7B-GGUF)"
frontend	app/src/pages/ModelHub.tsx	1629	placeholder="Filename (e.g. llama-2-7b.Q4_K_M.gguf)"
frontend	app/src/pages/Audit.tsx	266	placeholder="Operation name (required)"
frontend	app/src/pages/Audit.tsx	272	placeholder="Agent ID (optional)"
frontend	app/src/pages/Audit.tsx	296	placeholder="Trace ID (required)"
frontend	app/src/pages/Audit.tsx	302	placeholder="Parent Span ID"
frontend	app/src/pages/Audit.tsx	308	placeholder="Operation name (required)"
frontend	app/src/pages/Audit.tsx	314	placeholder="Agent ID (optional)"
frontend	app/src/pages/Audit.tsx	338	placeholder="Span ID (required)"
frontend	app/src/pages/Audit.tsx	353	placeholder="Error message (optional)"
frontend	app/src/pages/Audit.tsx	648	placeholder="Invariant name (e.g., capability_checks, fuel_budget, audit_integrity, pii_redaction, hitl_approval, no_unsafe_code)"
frontend	app/src/pages/Audit.tsx	853	// Client-side fallback for mock mode
frontend	app/src/pages/Audit.tsx	982	placeholder="Search events, payloads, hashes..."
frontend	app/src/pages/Protocols.tsx	497	placeholder="Agent base URL (e.g. http://localhost:9000)"
frontend	app/src/pages/Protocols.tsx	543	placeholder="Agent URL"
frontend	app/src/pages/Protocols.tsx	549	placeholder="Task message..."
frontend	app/src/pages/Protocols.tsx	570	placeholder="Agent URL"
frontend	app/src/pages/Protocols.tsx	577	placeholder="Task ID"
frontend	app/src/pages/Protocols.tsx	654	placeholder="Server name"
frontend	app/src/pages/Protocols.tsx	661	placeholder="URL (e.g. http://localhost:8080)"
frontend	app/src/pages/Protocols.tsx	677	placeholder="Auth token (optional)"
frontend	app/src/pages/Protocols.tsx	780	placeholder="Tool name"
frontend	app/src/pages/Protocols.tsx	786	placeholder='Arguments JSON, e.g. {"key": "value"}'
frontend	app/src/pages/Documents.tsx	1074	placeholder={
frontend	app/src/pages/DatabaseManager.tsx	323	placeholder="Connection name..."
frontend	app/src/pages/DatabaseManager.tsx	330	placeholder="SQLite path (e.g. ~/.nexus/data.db)"
frontend	app/src/pages/DatabaseManager.tsx	424	placeholder="Enter SQL query... (Ctrl+Enter to run)"
frontend	app/src/pages/DatabaseManager.tsx	512	<input className="db-builder-filter-val" value={filter.value} onChange={e => updateBuilderFilter(idx, { value: e.target.value })} placeholder="value" />
frontend	app/src/pages/Perception.tsx	205	placeholder="API Key"
frontend	app/src/pages/Perception.tsx	211	placeholder="Model ID"
frontend	app/src/pages/Perception.tsx	266	placeholder="Ask a question about the image..."
frontend	app/src/pages/Perception.tsx	275	placeholder='Optional JSON schema, e.g. {"type": "object"}'
frontend	app/src/pages/Telemetry.tsx	32	sample_rate: number;
frontend	app/src/pages/Telemetry.tsx	396	value={String(config.sample_rate)}
frontend	app/src/pages/Telemetry.tsx	447	placeholder="http://localhost:4317"
frontend	app/src/pages/Telemetry.tsx	468	placeholder="nexus-os"
frontend	app/src/pages/Telemetry.tsx	490	value={editConfig.sample_rate}
frontend	app/src/pages/Telemetry.tsx	492	patchEdit("sample_rate", parseFloat(e.target.value) || 0)
frontend	app/src/pages/Integrations.tsx	229	placeholder={field.placeholder}
frontend	app/src/pages/Integrations.tsx	384	placeholder: string;
frontend	app/src/pages/Integrations.tsx	392	{ key: "webhook_url", label: "Webhook URL", placeholder: "https://hooks.slack.com/services/...", secret: true },
frontend	app/src/pages/Integrations.tsx	393	{ key: "bot_token", label: "Bot Token (optional)", placeholder: "xoxb-...", secret: true },
frontend	app/src/pages/Integrations.tsx	394	{ key: "default_channel", label: "Default Channel", placeholder: "#nexus-alerts" },
frontend	app/src/pages/Integrations.tsx	398	{ key: "webhook_url", label: "Webhook URL", placeholder: "https://outlook.office.com/webhook/...", secret: true },
frontend	app/src/pages/Integrations.tsx	402	{ key: "base_url", label: "Base URL", placeholder: "https://your-org.atlassian.net" },
frontend	app/src/pages/Integrations.tsx	403	{ key: "email", label: "Email", placeholder: "admin@company.com" },
frontend	app/src/pages/Integrations.tsx	404	{ key: "api_token", label: "API Token", placeholder: "ATT...", secret: true },
frontend	app/src/pages/Integrations.tsx	405	{ key: "default_project", label: "Default Project", placeholder: "NEXUS" },
frontend	app/src/pages/Integrations.tsx	409	{ key: "instance_url", label: "Instance URL", placeholder: "https://your-instance.service-now.com" },
frontend	app/src/pages/Integrations.tsx	410	{ key: "username", label: "Username", placeholder: "admin" },
frontend	app/src/pages/Integrations.tsx	411	{ key: "password", label: "Password", placeholder: "********", secret: true },
frontend	app/src/pages/Integrations.tsx	415	{ key: "token", label: "Personal Access Token", placeholder: "ghp_...", secret: true },
frontend	app/src/pages/Integrations.tsx	416	{ key: "default_owner", label: "Default Owner", placeholder: "nexaiceo" },
frontend	app/src/pages/Integrations.tsx	417	{ key: "default_repo", label: "Default Repo", placeholder: "nexus-os" },
frontend	app/src/pages/Integrations.tsx	421	{ key: "base_url", label: "Base URL", placeholder: "https://gitlab.com" },
frontend	app/src/pages/Integrations.tsx	422	{ key: "token", label: "Access Token", placeholder: "glpat-...", secret: true },
frontend	app/src/pages/Integrations.tsx	423	{ key: "default_project_id", label: "Default Project", placeholder: "nexaiceo/nexus-os" },
frontend	app/src/pages/Integrations.tsx	427	{ key: "url", label: "Webhook URL", placeholder: "https://your-api.com/webhook" },
frontend	app/src/pages/Integrations.tsx	428	{ key: "method", label: "HTTP Method", placeholder: "POST" },
frontend	app/src/pages/Integrations.tsx	429	{ key: "secret", label: "HMAC Secret (optional)", placeholder: "your-secret", secret: true },
frontend	app/src/pages/CodeEditor.tsx	820	<input className="ce-search-input" placeholder="Search across all files..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} />
frontend	app/src/pages/CodeEditor.tsx	854	<input className="ce-new-file-input" placeholder="filename.ext" value={newFileName} onChange={(e) => setNewFileName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateFile(); if (e.key === "Escape") { setShowNewFile(false); setNewFileName(""); } }} autoFocus />
frontend	app/src/pages/CodeEditor.tsx	1030	placeholder="Type a command..."
frontend	app/src/pages/CodeEditor.tsx	1081	<input className="ce-git-commit-input" placeholder="Commit message..." value={commitMsg} onChange={(e) => setCommitMsg(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleGitCommit(); }} />
frontend	app/src/pages/TimeMachine.tsx	901	placeholder="Filter by agent ID..."
frontend	app/src/pages/TimeMachine.tsx	1485	placeholder="Checkpoint label..."
backend	crates/nexus-computer-control/src/engine.rs	213	&ComputerAction::ReadClipboard, // placeholder action for rollback
backend	crates/nexus-flash-infer/src/session.rs	35	let dummy_profile = ModelProfile {
backend	crates/nexus-flash-infer/src/session.rs	54	let total_budget = MemoryBudget::calculate(&hw, &dummy_profile, 0);
backend	crates/nexus-flash-infer/src/downloader.rs	824	// Create 4 fake split shard files (33 GB each → 132 GB total)
backend	crates/nexus-flash-infer/src/downloader.rs	825	let shard_size = 33 * 1024; // small fake size in bytes
backend	crates/nexus-flash-infer/src/downloader.rs	892	// Create a fake .gguf file
backend	crates/nexus-flash-infer/src/downloader.rs	893	std::fs::write(storage.model_path("test-Q4_K_M.gguf"), b"fake").unwrap();
backend	app/src-tauri/src/main.rs	483	"mock",
backend	app/src-tauri/src/main.rs	1852	"Triple-audited self-modification and hardcoded cooldown protections".to_string(),
backend	app/src-tauri/src/main.rs	2602	/// Return the default chat/completion model from config (or `"mock-1"`).
backend	app/src-tauri/src/main.rs	2608	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	2613	.unwrap_or_else(|_| "mock-1".to_string())
backend	app/src-tauri/src/main.rs	2719	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	2775	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	5082	name: "mock".to_string(),
backend	app/src-tauri/src/main.rs	5092	let has_real = providers.iter().any(|p| p.available && p.name != "mock");
backend	app/src-tauri/src/main.rs	7918	let (status, message) = if provider_name == "mock" {
backend	app/src-tauri/src/main.rs	9082	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	9118	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	9236	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	13835	"Challenge failed: code is too short or contains placeholder markers".to_string()
backend	app/src-tauri/src/main.rs	15739	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	15877	Ok(simulation_mock_response(prompt))
backend	app/src-tauri/src/main.rs	15898	fn simulation_mock_response(prompt: &str) -> String {
backend	app/src-tauri/src/main.rs	15939	"id": format!("mock-persona-{index}"),
backend	app/src-tauri/src/main.rs	15975	"id": format!("mock-persona-{index}"),
backend	app/src-tauri/src/main.rs	15977	"target": if index % 4 == 1 { Some("mock-persona-0") } else { None::<&str> },
backend	app/src-tauri/src/main.rs	15997	json!({"action":"whisper","target":"mock-persona-0","content":"Coordinate lobbying before the vote","reasoning":"Private coordination can amplify influence."})
backend	app/src-tauri/src/main.rs	18656	"mock-1".to_string()
backend	app/src-tauri/src/main.rs	29225	// In CI / test environments, mock is the expected fallback.
backend	app/src-tauri/src/main.rs	29315	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	29366	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	29415	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	29466	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	29734	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	29772	std::env::set_var("LLM_PROVIDER", "mock");
backend	app/src-tauri/src/main.rs	30695	Some("mock".into()),
backend	crates/nexus-memory/src/embedding.rs	59	/// Creates a new mock embedder.
backend	crates/nexus-memory/src/embedding.rs	71	const MOCK_DIM: usize = 64;
backend	crates/nexus-memory/src/embedding.rs	79	let mut vec = Vec::with_capacity(MOCK_DIM);
backend	crates/nexus-memory/src/embedding.rs	80	for &byte in hash.iter().chain(hash2.iter()).take(MOCK_DIM) {
backend	crates/nexus-memory/src/embedding.rs	96	MOCK_DIM
backend	crates/nexus-memory/src/embedding.rs	100	"mock-sha256-64d"
backend	crates/nexus-memory/src/embedding.rs	225	fn mock_returns_consistent_vectors() {
backend	crates/nexus-memory/src/embedding.rs	233	fn mock_returns_different_vectors_for_different_input() {
backend	crates/nexus-memory/src/embedding.rs	241	fn mock_correct_dimension() {
backend	crates/nexus-memory/src/embedding.rs	249	fn mock_produces_unit_vector() {
backend	crates/nexus-memory/src/embedding.rs	257	fn mock_model_name() {
backend	crates/nexus-memory/src/embedding.rs	259	assert_eq!(embedder.model_name(), "mock-sha256-64d");
backend	crates/nexus-memory/src/embedding.rs	263	fn mock_batch_embed() {
backend	crates/nexus-memory/src/space.rs	199	total_size_bytes: 0, // placeholder — accurate sizing requires serialization
backend	crates/nexus-flash-infer/tests/downloader_test.rs	56	// Create fake .gguf files
backend	crates/nexus-flash-infer/tests/downloader_test.rs	57	std::fs::write(storage.model_path("test-Q4_K_M.gguf"), b"fake-gguf").unwrap();
backend	crates/nexus-perception/src/vision.rs	295	fn test_mock_vision_provider() {
backend	crates/nexus-perception/src/vision.rs	308	"mock-vision-v1"
backend	crates/nexus-perception/src/vision.rs	317	assert_eq!(provider.model_id(), "mock-vision-v1");
backend	crates/nexus-perception/src/engine.rs	287	"mock-v1"
backend	crates/nexus-capability-measurement/src/battery/test_problem.rs	32	/// For tool use problems: available mock tools.
backend	crates/nexus-capability-measurement/src/battery/test_problem.rs	58	pub mock_responses: Vec<MockResponse>,
backend	crates/nexus-perception/src/screen.rs	86	"mock-v1"
backend	crates/nexus-perception/src/screen.rs	97	assert_eq!(result.model_used, "mock-v1");
backend	crates/nexus-governance-evolution/src/synthetic_attacks.rs	8	/// A synthetic attack — a fake request designed to test governance.
backend	crates/nexus-capability-measurement/src/evaluation/comparator.rs	714	fn test_comparator_with_mock_judge() {
```

## MISSING ERROR HANDLING
```text
NONE (0 pages with backend calls were missing error handling or loading-state signals in the static audit)
```

## DATA INTEGRITY
### Validation Runs
```tsv
path	size_bytes	baseline_sessions
data/validation_runs/real-battery-baseline.json	7952461	54
data/validation_runs/run1-pre-bugfix-baseline.json	11439130	54
data/validation_runs/run2-post-bugfix.json	11439114	54
```

### Agent / Battery Assets
```tsv
asset	value
agents/prebuilt/*.json count	54
crates/nexus-capability-measurement/data/battery_v1.json problems	20
```

## CONFIGURATION COMPLETENESS
```tsv
path	status
.gitlab-ci.yml	present
Cargo.toml	present
package.json	missing
tsconfig.json	missing
app/package.json	present
app/tsconfig.json	present
```

### Undocumented Environment Variables
```tsv
env_var	path	line
CARGO_MANIFEST_DIR	protocols/build.rs	5
NEXUS_LLAMA_CPP_PATH	llama-bridge/build.rs	17
HOME	cli/src/lib.rs	377
NEXUS_SELF_IMPROVE_DIR	cli/src/lib.rs	1065
TMPDIR	cli/src/lib.rs	1069
OLLAMA_MODEL	benchmarks/conductor-bench/src/cloud_models_bench.rs	1220
OLLAMA_MODEL	benchmarks/conductor-bench/src/inference_consistency_bench.rs	1123
NVIDIA_MODEL	benchmarks/conductor-bench/src/inference_consistency_bench.rs	1125
NEXUS_LONG_SESSION	benchmarks/conductor-bench/src/inference_consistency_bench.rs	1257
NEXUS_SESSION_DURATION	benchmarks/conductor-bench/src/inference_consistency_bench.rs	1261
NIM_RATE_LIMIT	benchmarks/conductor-bench/src/nim_cloud_bench.rs	42
NIM_DETERMINISM_RUNS	benchmarks/conductor-bench/src/nim_cloud_bench.rs	49
NIM_CONCURRENCY	benchmarks/conductor-bench/src/nim_cloud_bench.rs	56
NIM_MODELS	benchmarks/conductor-bench/src/nim_cloud_bench.rs	63
USER	auth/src/lib.rs	37
USERNAME	auth/src/lib.rs	38
NEXUS_ANTHROPIC_VALIDATE_URL	connectors/core/src/validation.rs	10
NEXUS_BRAVE_VALIDATE_URL	connectors/core/src/validation.rs	26
NEXUS_TELEGRAM_VALIDATE_BASE_URL	connectors/core/src/validation.rs	40
WHATSAPP_ACCESS_TOKEN	connectors/messaging/src/whatsapp.rs	61
WHATSAPP_PHONE_NUMBER_ID	connectors/messaging/src/whatsapp.rs	62
WHATSAPP_VERIFY_TOKEN	connectors/messaging/src/whatsapp.rs	63
DISCORD_BOT_TOKEN	connectors/messaging/src/discord.rs	34
SLACK_BOT_TOKEN	connectors/messaging/src/slack.rs	36
SLACK_APP_TOKEN	connectors/messaging/src/slack.rs	37
TELEGRAM_BOT_TOKEN	connectors/messaging/src/telegram.rs	54
BRAVE_API_KEY	connectors/web/src/search.rs	265
HOME	connectors/llm/src/model_registry.rs	131
COHERE_URL	connectors/llm/src/providers/cohere.rs	28
FIREWORKS_URL	connectors/llm/src/providers/fireworks.rs	23
DEEPSEEK_URL	connectors/llm/src/providers/deepseek.rs	22
MISTRAL_URL	connectors/llm/src/providers/mistral.rs	23
ANTHROPIC_URL	connectors/llm/src/providers/claude.rs	29
GEMINI_URL	connectors/llm/src/providers/gemini.rs	24
TOGETHER_URL	connectors/llm/src/providers/together.rs	23
OPENAI_URL	connectors/llm/src/providers/openai.rs	26
OPENROUTER_URL	connectors/llm/src/providers/openrouter.rs	68
OPENROUTER_HTTP_REFERER	connectors/llm/src/providers/openrouter.rs	96
OPENROUTER_X_TITLE	connectors/llm/src/providers/openrouter.rs	98
NVIDIA_NIM_URL	connectors/llm/src/providers/nvidia.rs	471
GROQ_URL	connectors/llm/src/providers/groq.rs	44
PERPLEXITY_URL	connectors/llm/src/providers/perplexity.rs	23
HOSTNAME	protocols/src/http_gateway.rs	402
NEXUS_CORS_ORIGINS	protocols/src/http_gateway.rs	620
NEXUS_MODE	protocols/src/server_runtime.rs	35
JWT_SECRET	protocols/src/server_runtime.rs	50
NEXUS_HTTP_ADDR	protocols/src/server_runtime.rs	66
NEXUS_SHUTDOWN_TIMEOUT_SECS	protocols/src/server_runtime.rs	78
NEXUS_CONFIG_KEY	kernel/src/config.rs	380
NEXUS_ENCRYPTION_KEY	kernel/src/crypto.rs	92
HOME	kernel/src/backup.rs	209
NEXUS_DATA_DIR	kernel/src/backup.rs	219
HOME	kernel/src/backup.rs	222
HOME	kernel/src/protocols/mcp.rs	612
STABLE_DIFFUSION_WEBUI_URL	kernel/src/actuators/image_gen.rs	38
REPLICATE_API_TOKEN	kernel/src/actuators/image_gen.rs	44
STABLE_DIFFUSION_WEBUI_URL	kernel/src/actuators/image_gen.rs	141
REPLICATE_API_TOKEN	kernel/src/actuators/image_gen.rs	210
REPLICATE_MODEL_VERSION	kernel/src/actuators/image_gen.rs	212
PIPER_MODEL	kernel/src/actuators/tts.rs	65
PIPER_MODEL	kernel/src/actuators/tts.rs	90
NEXUS_PLAYWRIGHT_MODULE	kernel/src/actuators/browser.rs	106
HOME	crates/nexus-flash-infer/src/downloader.rs	104
USERPROFILE	crates/nexus-flash-infer/src/downloader.rs	104
HOME	crates/nexus-flash-infer/src/downloader.rs	270
USERPROFILE	crates/nexus-flash-infer/src/downloader.rs	271
HOME	crates/nexus-flash-infer/src/downloader.rs	660
APPDATA	crates/nexus-flash-infer/src/downloader.rs	664
XDG_DATA_HOME	crates/nexus-flash-infer/src/downloader.rs	667
HOME	crates/nexus-flash-infer/src/downloader.rs	671
HOME	tests/integration/src/full_agent_flow.rs	62
HOME	persistence/src/lib.rs	985
HOME	sdk/src/memory.rs	73
HOME	marketplace/src/sqlite_registry.rs	99
NEXUS_DISCORD_BOT_TOKEN	integrations/src/providers/discord.rs	71
NEXUS_DISCORD_CHANNEL_ID	integrations/src/providers/discord.rs	76
NEXUS_GITHUB_TOKEN	integrations/src/providers/github.rs	43
NEXUS_GITHUB_OWNER	integrations/src/providers/github.rs	48
NEXUS_GITHUB_REPO	integrations/src/providers/github.rs	49
NEXUS_JIRA_BASE_URL	integrations/src/providers/jira.rs	46
NEXUS_JIRA_EMAIL	integrations/src/providers/jira.rs	52
NEXUS_JIRA_TOKEN	integrations/src/providers/jira.rs	56
NEXUS_JIRA_PROJECT	integrations/src/providers/jira.rs	59
NEXUS_SLACK_WEBHOOK_URL	integrations/src/providers/slack.rs	44
NEXUS_SLACK_BOT_TOKEN	integrations/src/providers/slack.rs	49
NEXUS_SLACK_CHANNEL	integrations/src/providers/slack.rs	51
NEXUS_GITLAB_BASE_URL	integrations/src/providers/gitlab.rs	44
NEXUS_GITLAB_TOKEN	integrations/src/providers/gitlab.rs	45
NEXUS_GITLAB_PROJECT_ID	integrations/src/providers/gitlab.rs	51
NEXUS_TEAMS_WEBHOOK_URL	integrations/src/providers/teams.rs	32
NEXUS_TEAMS_ACCESS_TOKEN	integrations/src/providers/teams.rs	157
NEXUS_SNOW_INSTANCE_URL	integrations/src/providers/servicenow.rs	43
NEXUS_SNOW_USERNAME	integrations/src/providers/servicenow.rs	48
NEXUS_SNOW_PASSWORD	integrations/src/providers/servicenow.rs	53
NEXUS_TELEGRAM_BOT_TOKEN	integrations/src/providers/telegram.rs	46
NEXUS_TELEGRAM_CHAT_ID	integrations/src/providers/telegram.rs	51
HOSTNAME	app/src-tauri/src/main.rs	1018
COMPUTERNAME	app/src-tauri/src/main.rs	1019
HOME	app/src-tauri/src/main.rs	1021
HOME	app/src-tauri/src/main.rs	1155
HOSTNAME	app/src-tauri/src/main.rs	1317
COMPUTERNAME	app/src-tauri/src/main.rs	1318
HOME	app/src-tauri/src/main.rs	1320
HOME	app/src-tauri/src/main.rs	8081
CARGO_MANIFEST_DIR	app/src-tauri/src/main.rs	9386
HOME	app/src-tauri/src/main.rs	10145
HOME	app/src-tauri/src/main.rs	10983
HOME	app/src-tauri/src/main.rs	13063
USERPROFILE	app/src-tauri/src/main.rs	13064
HOME	app/src-tauri/src/main.rs	13220
USERPROFILE	app/src-tauri/src/main.rs	13221
HOME	app/src-tauri/src/main.rs	13283
USERPROFILE	app/src-tauri/src/main.rs	13284
HOME	app/src-tauri/src/main.rs	16450
HOSTNAME	app/src-tauri/src/main.rs	20088
USER	app/src-tauri/src/main.rs	20385
USERNAME	app/src-tauri/src/main.rs	20386
HOME	app/src-tauri/src/main.rs	20907
HOME	app/src-tauri/src/main.rs	25045
TEST_MODEL_PATH	llama-bridge/tests/api_test.rs	318
ORIGIN	llama-bridge/llama-cpp/tools/server/webui/tests/stories/fixtures/ai-tutorial.ts	54
```

## SECURITY FINDINGS
```tsv
check	count	detail
hardcoded_secrets	0	grep returned no confirmed hardcoded secret assignments
committed_env_files	0	NONE
recent_env_additions_in_git_history	0	NONE
```
