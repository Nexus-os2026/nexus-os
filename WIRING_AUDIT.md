# NEXUS OS WIRING AUDIT
## Date: 2026-03-16
## Total Features Audited: 96

### SUMMARY
- Fully Wired: 53 features
- Backend Only (no UI): 8 features
- Frontend Only (mock data): 3 features
- Dead Code: 2 features
- Broken: 30 features
- Tauri command cross-check: 270 registered, 178 referenced by frontend `app/src/**/*.ts(x)`, 92 unreferenced, 0 frontend invoke targets missing in backend.

Audit method:
- Static code audit of the current working tree.
- Targeted backend tests passed:
  - `test_index_document_end_to_end`
  - `test_browser_navigate_logs_audit`
  - `test_approve_consent_request_wakes_blocked_wait`
  - `test_time_machine_create_and_list_checkpoints`
  - `test_voice_transcribe_fallback_stub`
  - `test_load_prebuilt_agents_registers_every_manifest`
- Targeted backend tests that did not complete within 15s:
  - `test_create_simulation_command`
  - `test_start_simulation_produces_report`
- External providers/connectors were verified by code path unless a local test existed. No live third-party API credentials were supplied during this audit.

### CRITICAL (must fix for demo)
- Hivemind is not demo-safe: `start_hivemind` exists, but no frontend uses it, `AppState` wires `StubHivemindLlm`, and the coordinator currently simulates subtask completion instead of dispatching real agent work.
- Simulation UI and commands are wired, but targeted simulation tests hung past 15 seconds, so end-to-end simulation is not demo-safe.
- Fuel is deducted in memory during cognitive execution, but `fuel_ledgers` are not persisted to the database.
- `web.search` is still a placeholder response in `kernel/src/actuators/web.rs`; only `web.read`/fetch is real.
- Voice is only partially wired: the page uses stub/demo transcript flows, never calls `voice_stop_listening` or `voice_transcribe`, and Whisper requires extra setup.
- Browser page is only partially real: navigation is real, but governance counters/sidebar are demo values and the page is not exposing governed browser automation.
- Code editor file IO is real, but the "agent" actions are canned local responses, not backend agent work.
- Messaging connectors exist, but there is no UI wiring for Telegram/Discord/Slack/WhatsApp operations.
- Multi-model/per-phase support is not truly active at runtime; selection metadata exists, but model phase routing is not applied by the main gateway path.

### BACKEND ONLY (needs UI wiring)
- Screen capture, input control, and computer-use backends are implemented, but no page directly drives them.
- Messaging connectors are backend-only; only status/default-agent commands exist and even those are not used in the frontend.
- Hivemind commands exist in Tauri but are not used by any frontend page.
- Evolution/self-evolution command surface exists in Tauri but is not used by any frontend page.
- Economy, payment, tracing, reputation, replay, airgap, neural bridge, Nexus Link, and Ghost protocol command families are registered but unreferenced by frontend code.

### MOCK/FAKE DATA (needs real backend calls)
- `DesignStudio.tsx` is fully local/mock.
- `LearningCenter.tsx` is educational hardcoded content; backend imports appear only in sample code strings.
- `MediaStudio.tsx` is fully local/mock.
- `AgentBrowser.tsx` mixes real backend submodes with demo governance counters and placeholder browser state.
- `VoiceAssistant.tsx` simulates transcript flow in stub mode instead of using real transcription end-to-end.
- `CodeEditor.tsx` uses real file IO but fake agent-assistant responses.
- `DeveloperPortal.tsx` uses a simulated verification timeline even when marketplace publish is real.
- `DistributedAudit.tsx` shows a hardcoded single paired device and synthetic block grouping on top of real audit rows.
- `ClusterStatus.tsx` is a single-node placeholder, not real cluster orchestration.
- `Workflows.tsx` shows real schedules, but workflow creation/history are placeholders.

### DEAD CODE (unused, consider removing)
- `app/src/pages/Dashboard.tsx` is not imported or routed anywhere.
- Persistence table `fuel_ledgers` is defined and load/save helpers exist, but no normal desktop execution path writes to it.

### FULLY WORKING
- Core cognitive loop execution from the main chat flow via `execute_agent_goal`.
- Planner malformed-JSON fallback/retry path.
- Agent memory persistence through `agent_memory`.
- Filesystem actuator, shell actuator, and API actuator.
- Approval Center consent flow, including blocked-wait wakeup.
- Notes, File Manager, Database Manager, Documents/RAG, Model Hub, Time Machine, Trust Dashboard.
- Prebuilt agent discovery/loading on startup.
- Cron scheduler registration and startup initialization.
- Audit chain persistence with SHA-256 hash linking.

### DETAILED AUDIT

## 1. COGNITIVE LOOP
- `loop_runtime.rs`: A (Fully wired for the actual UI path). `assign_agent_goal` only records the goal. The real UI path uses `execute_agent_goal`, and the Tauri wrapper then calls `spawn_cognitive_loop(...)`. `App.tsx` uses `executeAgentGoal(...)`, not `assignAgentGoal(...)`.
- `planner.rs`: A. It retries malformed model output, repairs common JSON issues, extracts array payloads, and falls back to a direct `LlmQuery` plan if parsing still fails.
- `memory_manager.rs`: A. Agent memories are persisted through `DbMemoryStore` into `agent_memory` and are available across sessions.
- `hivemind.rs`: E (Broken). Tauri command is `start_hivemind`, but no frontend page calls it. `AppState` wires `StubHivemindLlm`, which errors with "No LLM provider configured for hivemind...", and execution still simulates subtask completion strings instead of dispatching real agent work.
- `evolution.rs`: A. Post-task evolution runs from the Tauri bridge path after task completion via `run_post_goal_evolution(...)`. It is not driven directly by the kernel loop standalone path.
- `algorithms/evolutionary.rs`: A. `EvolutionEngine` is used outside tests by cognitive loop and simulation runtime.
- `algorithms/swarm.rs`: A. `SwarmCoordinator` is used outside tests by cognitive loop and simulation runtime.
- `algorithms/world_model.rs`: A. `WorldModel` is used outside tests by cognitive loop and simulation runtime.
- `algorithms/adversarial.rs`: A. `AdversarialArena` is used outside tests by cognitive loop.

## 2. ACTUATORS
- `filesystem.rs`: A. Real file read/write/create/delete logic exists via `std::fs`, scoped to the agent working directory. Agent workspaces resolve under `~/.nexus/agents/<agent_id>/workspace`.
- `shell.rs`: A. Real command execution is done through `std::process::Command` with allow/block lists and timeouts.
- `web.rs`: E. `WebFetch`/`web.read` is real. `WebSearch`/`web.search` still returns a structured placeholder (`search_dispatched`) instead of real search results.
- `api.rs`: A. Real HTTP requests are executed through `curl` with governance checks.
- `screen.rs`: B (Backend only). `ScreenCaptureActuator` exists and is wired to real OS capture helpers, but no page uses the related commands. Implementation uses OS commands (`import`, `screencapture`) rather than `screenshots`.
- `input.rs`: B. `InputControlActuator` exists and uses real OS tooling (`xdotool`/`osascript`), but no page uses the related commands. It is not wired through `enigo`.
- `computer_use.rs`: B. `ComputerUseActuator` exists, includes a capture -> vision -> action loop, and uses the Ollama vision path, but there is no UI wiring for it.
- `ActuatorRegistry` in `mod.rs`: A. All requested actuators are registered in `with_defaults()`.

## 3. PERSISTENCE
- Tables clearly written from normal desktop flows:
  - `agents`
  - `audit_events`
  - `permissions`
  - `consent_queue`
  - `checkpoints`
  - `agent_memory`
  - `task_history`
  - `strategy_scores`
  - `algorithm_selections`
  - `simulation_worlds`
  - `simulation_personas`
  - `simulation_events`
- Tables present but not observed being written from normal desktop/UI execution:
  - `fuel_ledgers`
  - `embeddings`
  - `evolution_archive`
  - `swarm_state`
  - `world_model_entities`
  - `world_model_relationships`
  - `adversarial_matches`
- Tables that appear niche/backend-only rather than ordinary UI flow:
  - `hivemind_sessions`
  - `evolution_history`
  - `agent_ecosystems`
  - `l6_cooldown_tracker`
- Agent state restore on restart: E. Persisted agents are reloaded into the supervisor, but they are immediately stopped. Metadata is restored; active execution is not resumed.
- Audit events written during execution: A. `AppState::log_event()` appends a DB row with linked hashes.
- Fuel deductions persisted: E. Fuel is consumed in supervisor memory during execution, but `save_fuel_ledger(...)` is not called from normal app flows.

## 4. SIMULATION
- `kernel/src/simulation/`: A. The module exists.
- Tauri commands: A. `create_simulation`, `start_simulation`, `pause_simulation`, `inject_variable`, `get_simulation_status`, `get_simulation_report`, `chat_with_persona`, `list_simulations`, and `run_parallel_simulations` are registered.
- `WorldSimulation.tsx`: A. The page exists and is routed from `App.tsx`.
- End-to-end user run: E. Static wiring is present, but targeted tests `test_create_simulation_command` and `test_start_simulation_produces_report` did not complete within 15 seconds, so I cannot call simulation demo-safe.

## 5. FRONTEND PAGES (`app/src/pages/`)
- `AgentBrowser.tsx`: PARTIAL. Calls `navigate_to` plus browser subcomponents that call `start_research`, `research_agent_action`, `complete_research`, `start_build`, `build_append_code`, `build_add_message`, `complete_build`, `start_learning`, and `learning_agent_action`. Real backend exists, but governance counters/sidebar are partly demo data and the page is not a real governed browser automation surface.
- `Agents.tsx`: PARTIAL. No direct invoke; receives real App-level data/actions (`list_agents`, `get_audit_log`, `start_agent`, `pause_agent`, `stop_agent`, `create_agent`, `clear_all_agents`). Desktop path is real, browser-mode fallback is mock, and SLM status is stubbed.
- `AiChatHub.tsx`: PARTIAL. Calls `send_chat`, `chat_with_ollama`, `conduct_build`, `list_agents`, `list_provider_models`, `get_provider_status`, `save_api_key`. Real chat/build paths exist, but it also shows locked placeholder models and fallback logic.
- `ApiClient.tsx`: REAL. Calls `api_client_request`. Sample collections are hardcoded, but request execution and response display are real.
- `AppStore.tsx`: REAL. Calls `get_preinstalled_agents`, `marketplace_search`, `marketplace_install`, `start_agent`.
- `ApprovalCenter.tsx`: REAL. Calls `list_pending_consents`, `get_consent_history`, `approve_consent_request`, `deny_consent_request`, `batch_approve_consents`, `batch_deny_consents`, `review_consent_batch`.
- `Audit.tsx`: PARTIAL. Uses App-provided audit rows and calls `get_audit_chain_status`; App refresh path uses `get_audit_log`. Real in desktop mode, mock in browser mode.
- `AuditTimeline.tsx`: PARTIAL. Same pattern as `Audit.tsx`: real desktop data, browser-mode fallback.
- `Chat.tsx`: PARTIAL. Calls `list_provider_models` directly and relies on App-level `send_chat`, `execute_agent_goal`, and `chat_with_ollama`. Desktop path is real; browser mode is mock.
- `ClusterStatus.tsx`: PARTIAL. Calls `get_live_system_metrics`, but the page itself is just a single-node placeholder, not real cluster management.
- `CodeEditor.tsx`: PARTIAL. Calls `file_manager_list`, `file_manager_home`, `get_git_repo_status`, `file_manager_read`, `file_manager_write`. Real file editing works; the "agent actions" are local canned responses.
- `CommandCenter.tsx`: PARTIAL. Calls `list_agents`, `get_audit_log`, `start_agent`, `stop_agent`, `pause_agent`, `resume_agent`. Core agent control is real; autonomy/status summary still has placeholder fields.
- `ComplianceDashboard.tsx`: PARTIAL. Calls `get_compliance_status`, `get_compliance_agents`, `get_audit_log`. Real read paths exist, but erasure/report-generation behaviors are client-only simulations.
- `Dashboard.tsx`: EMPTY. Not imported or routed anywhere.
- `DatabaseManager.tsx`: REAL. Calls `db_connect`, `db_list_tables`, `db_execute_query`.
- `DeployPipeline.tsx`: REAL. Calls `factory_create_project`, `factory_build_project`, `factory_test_project`, `factory_run_pipeline`, `factory_list_projects`, `factory_get_build_history`.
- `DesignStudio.tsx`: MOCK. No backend calls; all data is local canvas/library/history state.
- `DeveloperPortal.tsx`: PARTIAL. Calls `marketplace_publish`, `marketplace_my_agents`. Real publish/load paths exist, but the verification timeline is locally simulated.
- `DistributedAudit.tsx`: PARTIAL. Calls `get_audit_log`, `get_audit_chain_status`. Real audit data exists, but paired devices and block visualization are synthetic.
- `Documents.tsx`: REAL. Calls `index_document`, `chat_with_documents`, `list_indexed_documents`, `remove_indexed_document`, `get_document_governance`, `get_semantic_map`, `get_document_access_log`. Confirmed by passing `test_index_document_end_to_end`.
- `EmailClient.tsx`: PARTIAL. Calls `email_list`, `email_save`, `email_delete`. Real local draft storage works, but this is not a real mail transport client.
- `FileManager.tsx`: REAL. Calls `file_manager_list`, `file_manager_home`, `file_manager_read`, `file_manager_delete`, `file_manager_rename`, `file_manager_create_dir`, `file_manager_write`.
- `Firewall.tsx`: REAL. Calls `get_firewall_status`, `get_firewall_patterns`.
- `Identity.tsx`: PARTIAL. Calls `list_identities`, `get_agent_identity`. Identity read path is real, but the page falls back to plain agent rows when identities are missing.
- `LearningCenter.tsx`: MOCK. No real backend calls. The backend imports appear inside code snippets/tutorial content only.
- `MediaStudio.tsx`: MOCK. No backend calls; all media/filter/generation state is local UI state.
- `ModelHub.tsx`: REAL. Calls `search_models`, `get_model_info`, `check_model_compatibility`, `download_model`, `list_local_models`, `list_provider_models`, `delete_local_model`, `get_system_specs`.
- `NotesApp.tsx`: REAL. Calls `notes_list`, `notes_save`, `notes_delete`.
- `PermissionDashboard.tsx`: REAL. Calls `get_agent_permissions`, `get_permission_history`, `get_capability_request`, `update_agent_permission`, `bulk_update_permissions`, `set_agent_llm_provider`.
- `PolicyManagement.tsx`: PARTIAL. Calls `policy_list`, `policy_validate`, `policy_test`, `policy_detect_conflicts`. Read/validate/test are real; there is no persisted editor save path from the page.
- `ProjectManager.tsx`: PARTIAL. Calls `project_list`, `project_save`, `project_delete`. Real local persistence exists, but the board/sprint/automation engine is mostly client-side state.
- `Protocols.tsx`: REAL. Calls `get_protocols_status`, `get_mcp_tools`, `get_agent_cards`, `get_protocols_requests`.
- `Settings.tsx`: PARTIAL. Gets config/save from App (`get_config`, `save_config`, `check_ollama`, `delete_model`) and calls `check_llm_status`, `get_llm_recommendations`, `test_llm_connection`. Some visualizations are explicitly demo/random.
- `SetupWizard.tsx`: PARTIAL. Uses App-provided `detect_hardware`, `check_ollama`, `ensure_ollama`, `is_ollama_installed`, `pull_model`, `list_available_models`, `set_agent_model`. Desktop path is real; App injects mock fallbacks when runtime is not desktop.
- `SystemMonitor.tsx`: PARTIAL. Calls `get_live_system_metrics`, `list_agents`. Real metrics are displayed, but alerts/audit lines are generated client-side from those metrics.
- `Terminal.tsx`: REAL. Calls `terminal_execute`, `terminal_execute_approved`. Command history/suggestions are local UI, but actual execution is real.
- `TimeMachine.tsx`: REAL. Calls `time_machine_list_checkpoints`, `time_machine_get_checkpoint`, `time_machine_create_checkpoint`, `time_machine_undo`, `time_machine_undo_checkpoint`, `time_machine_redo`, `time_machine_get_diff`, `time_machine_what_if`. Confirmed by passing `test_time_machine_create_and_list_checkpoints`.
- `TrustDashboard.tsx`: REAL. Calls `get_trust_overview`.
- `VoiceAssistant.tsx`: PARTIAL. Calls `voice_get_status`, `voice_start_listening`, `voice_load_whisper_model`. It does not call `voice_stop_listening` or `voice_transcribe`, and transcript behavior is largely simulated.
- `Workflows.tsx`: PARTIAL. Calls `get_scheduled_agents`, `list_agents`. Real schedule display exists, but workflow creation/history are explicitly "not configured".
- `WorldSimulation.tsx`: REAL. Calls `create_simulation`, `start_simulation`, `pause_simulation`, `inject_variable`, `get_simulation_status`, `get_simulation_report`, `chat_with_persona`, `list_simulations`, `run_parallel_simulations`, and listens for simulation events. The page is real; the backend runtime is not yet demo-safe.

Page rating totals:
- REAL: 16
- PARTIAL: 21
- MOCK: 3
- EMPTY: 1

## 6. TAURI COMMANDS (`app/src-tauri/src/main.rs`)
- Registered commands found in `generate_handler!`: 270
- Referenced from frontend `app/src/**/*.ts(x)`: 178
- Registered but not referenced by frontend: 92
- Frontend invoke targets with no backend command: none

Registered commands (grouped, complete list):
```text
Core/runtime: list_agents, create_agent, start_agent, stop_agent, clear_all_agents, get_scheduled_agents, get_preinstalled_agents, pause_agent, resume_agent, get_audit_log, send_chat, get_config, save_config, start_jarvis_mode, stop_jarvis_mode, jarvis_status, transcribe_push_to_talk, tray_status.
Setup/models/providers: detect_hardware, check_ollama, pull_ollama_model, pull_model, ensure_ollama, is_ollama_installed, delete_model, is_setup_complete, run_setup_wizard, list_available_models, list_provider_models, get_provider_status, save_api_key, chat_with_ollama, set_agent_model, check_llm_status, get_llm_recommendations, set_agent_llm_provider, get_provider_usage_stats, test_llm_connection, get_system_info.
Permissions/protocols/identity/firewall: get_agent_permissions, update_agent_permission, get_permission_history, get_capability_request, bulk_update_permissions, get_protocols_status, get_protocols_requests, get_mcp_tools, get_agent_cards, get_agent_identity, list_identities, get_firewall_status, get_firewall_patterns.
Marketplace/browser/research/build/learning: marketplace_search, marketplace_install, marketplace_info, marketplace_publish, marketplace_my_agents, start_learning, learning_agent_action, get_learning_session, get_knowledge_base, navigate_to, get_browser_history, get_agent_activity, start_research, research_agent_action, complete_research, get_research_session, list_research_sessions, start_build, build_append_code, build_add_message, complete_build, get_build_session, get_build_code, get_build_preview.
Policy/RAG/model hub/system/time machine: policy_list, policy_validate, policy_test, policy_detect_conflicts, index_document, search_documents, chat_with_documents, list_indexed_documents, remove_indexed_document, get_document_governance, get_semantic_map, get_document_access_log, get_active_llm_provider, search_models, get_model_info, check_model_compatibility, download_model, list_local_models, delete_local_model, get_system_specs, get_live_system_metrics, time_machine_list_checkpoints, time_machine_get_checkpoint, time_machine_create_checkpoint, time_machine_undo, time_machine_undo_checkpoint, time_machine_redo, time_machine_get_diff, time_machine_what_if.
Peer/evolution/MCP host/ghost/voice/factory/tooling/terminal: nexus_link_status, nexus_link_toggle_sharing, nexus_link_add_peer, nexus_link_remove_peer, nexus_link_list_peers, nexus_link_send_model, evolution_get_status, evolution_register_strategy, evolution_evolve_once, evolution_get_history, evolution_rollback, evolution_get_active_strategy, mcp_host_list_servers, mcp_host_add_server, mcp_host_remove_server, mcp_host_connect, mcp_host_disconnect, mcp_host_list_tools, mcp_host_call_tool, ghost_protocol_status, ghost_protocol_toggle, ghost_protocol_add_peer, ghost_protocol_remove_peer, ghost_protocol_sync_now, ghost_protocol_get_state, voice_start_listening, voice_stop_listening, voice_get_status, voice_transcribe, voice_load_whisper_model, factory_create_project, factory_build_project, factory_test_project, factory_run_pipeline, factory_list_projects, factory_get_build_history, conduct_build, execute_tool, list_tools, terminal_execute, terminal_execute_approved.
Replay/airgap/reputation/trust/computer/neural/economy/tracing/payment: replay_list_bundles, replay_get_bundle, replay_verify_bundle, replay_export_bundle, replay_toggle_recording, airgap_create_bundle, airgap_validate_bundle, airgap_install_bundle, airgap_get_system_info, reputation_register, reputation_record_task, reputation_rate_agent, reputation_get, reputation_top, reputation_export, reputation_import, get_trust_overview, computer_control_capture_screen, computer_control_execute_action, computer_control_get_history, computer_control_toggle, computer_control_status, capture_screen, analyze_screen, start_computer_action, stop_computer_action, get_input_control_status, neural_bridge_status, neural_bridge_toggle, neural_bridge_ingest, neural_bridge_search, neural_bridge_delete, neural_bridge_clear_old, economy_create_wallet, economy_get_wallet, economy_spend, economy_earn, economy_transfer, economy_freeze_wallet, economy_get_history, economy_get_stats, economy_create_contract, economy_complete_contract, economy_list_contracts, economy_dispute_contract, economy_agent_performance, tracing_start_trace, tracing_start_span, tracing_end_span, tracing_end_trace, tracing_list_traces, tracing_get_trace, payment_create_plan, payment_list_plans, payment_create_invoice, payment_pay_invoice, payment_get_revenue_stats, payment_create_payout.
Compliance/storage/cognitive/consent/simulation/hivemind/messaging: get_compliance_status, get_compliance_agents, get_audit_chain_status, get_git_repo_status, verify_governance_invariants, verify_specific_invariant, export_compliance_report, file_manager_list, file_manager_read, file_manager_write, file_manager_create_dir, file_manager_delete, file_manager_rename, file_manager_home, db_connect, db_execute_query, db_list_tables, api_client_request, notes_list, notes_get, notes_save, notes_delete, email_list, email_save, email_delete, project_list, project_get, project_save, project_delete, assign_agent_goal, execute_agent_goal, stop_agent_goal, get_agent_cognitive_status, get_agent_task_history, get_agent_memories, get_self_evolution_metrics, get_self_evolution_strategies, trigger_cross_agent_learning, approve_consent_request, deny_consent_request, batch_approve_consents, review_consent_batch, batch_deny_consents, list_pending_consents, get_consent_history, create_simulation, start_simulation, pause_simulation, inject_variable, get_simulation_status, get_simulation_report, chat_with_persona, list_simulations, run_parallel_simulations, start_hivemind, get_hivemind_status, cancel_hivemind, get_messaging_status, set_default_agent.
```

Registered backend commands with no frontend references:
```text
tray_status
get_active_llm_provider
nexus_link_status
nexus_link_toggle_sharing
nexus_link_add_peer
nexus_link_remove_peer
nexus_link_list_peers
nexus_link_send_model
evolution_get_status
evolution_register_strategy
evolution_evolve_once
evolution_get_history
evolution_rollback
evolution_get_active_strategy
mcp_host_list_servers
mcp_host_add_server
mcp_host_remove_server
mcp_host_connect
mcp_host_disconnect
mcp_host_list_tools
mcp_host_call_tool
ghost_protocol_status
ghost_protocol_toggle
ghost_protocol_add_peer
ghost_protocol_remove_peer
ghost_protocol_sync_now
ghost_protocol_get_state
execute_tool
list_tools
replay_list_bundles
replay_get_bundle
replay_verify_bundle
replay_export_bundle
replay_toggle_recording
airgap_create_bundle
airgap_validate_bundle
airgap_install_bundle
airgap_get_system_info
reputation_register
reputation_record_task
reputation_rate_agent
reputation_get
reputation_top
reputation_export
reputation_import
computer_control_capture_screen
computer_control_execute_action
computer_control_get_history
computer_control_toggle
computer_control_status
neural_bridge_status
neural_bridge_toggle
neural_bridge_ingest
neural_bridge_search
neural_bridge_delete
neural_bridge_clear_old
economy_create_wallet
economy_get_wallet
economy_spend
economy_earn
economy_transfer
economy_freeze_wallet
economy_get_history
economy_get_stats
economy_create_contract
economy_complete_contract
economy_list_contracts
economy_dispute_contract
economy_agent_performance
tracing_start_trace
tracing_start_span
tracing_end_span
tracing_end_trace
tracing_list_traces
tracing_get_trace
payment_create_plan
payment_list_plans
payment_create_invoice
payment_pay_invoice
payment_get_revenue_stats
payment_create_payout
verify_governance_invariants
verify_specific_invariant
export_compliance_report
notes_get
get_agent_cognitive_status
get_agent_task_history
get_agent_memories
get_self_evolution_metrics
get_self_evolution_strategies
start_hivemind
get_hivemind_status
```

Frontend invoke calls that reference commands that do not exist:
- None found.

## 7. PREBUILT AGENTS (`agents/prebuilt/`)
- Total JSON files: 47
- JSON syntax: all 47 files are valid JSON (`jq empty` passed)
- Parse path as `AgentManifest`: A. Loader uses `parse_agent_manifest_json()`, which deserializes into `JsonAgentManifest`, serializes back to TOML, and re-validates with `parse_manifest(...)`.
- Load on startup: A. `AppState::new()` calls `load_prebuilt_agents()` in non-test builds. Confirmed by passing `test_load_prebuilt_agents_registers_every_manifest`.
- Capability match: A. The manifest capability set matches real registered capability names (`fs.read`, `fs.write`, `process.exec`, `web.read`, `web.search`, `mcp.call`, `screen.capture`, `screen.analyze`, `input.mouse`, `input.keyboard`, `input.autonomous`, `computer.use`, `self.modify`, `cognitive_modify`).

## 8. MESSAGING CONNECTORS (`connectors/messaging/`)
- Telegram: B. Real HTTP implementation exists. Needs a real bot token to do real sends/polls. Without config it falls back to mock/no-op behavior.
- Discord: B. Real HTTP implementation exists. Needs `DISCORD_BOT_TOKEN`.
- Slack: B. Real HTTP implementation exists. Needs `SLACK_BOT_TOKEN` and Socket Mode app token for full operation.
- WhatsApp: B. Real HTTP implementation exists. Needs access token, phone number ID, and webhook verification token.
- Callable from UI: B. No page calls messaging operations. Only `get_messaging_status` and `set_default_agent` exist in Tauri, and neither is used by the frontend.

## 9. LLM PROVIDERS (`connectors/llm/`)
- Implemented providers: OpenAI, Anthropic/Claude, Ollama, Gemini, Groq, Mistral, Cohere, Together, Perplexity, DeepSeek, Fireworks, OpenRouter, Mock, plus optional `local-slm`.
- Real API/local call path: A by code path, not by live credential test. These providers make real HTTP or local inference calls when configured. The audit did not include live third-party credential validation.
- Multi-model/per-phase support: E. Backend metadata exists (`set_agent_llm_provider`, model mapping memory), but the main gateway/planner path still resolves from the global provider path rather than true per-phase model routing.

## 10. GOVERNANCE SYSTEMS
- Fuel metering: A for in-memory enforcement, E for persistence. Actions deduct fuel during cognitive execution, but the DB ledger is not saved.
- HITL consent: A. Blocking/approval flow is end-to-end, including persistence and wakeup. Confirmed by consent tests.
- Audit chain: A. Events are hash-linked in DB using SHA-256 over previous hash + sequence + detail.
- Speculative execution: A. `SpeculativeEngine.simulate()` is called before real execution for Tier2+ operations in supervisor/protocol bridge paths.
- Time Machine: A. Checkpoints are created during assignment, execution, approvals, and via explicit time-machine commands.
- Warden: E. Review engine exists, but it is disabled by default and only activates when `governance.enable_warden_review=true` and a running `nexus-warden` agent is present.
- Kill gates: A. Emergency input kill-switch exists and is wired.

## 11. CRON SCHEDULER
- Agent manifests have `schedule` fields: A. `AgentManifest` includes `schedule` and `default_goal`.
- Cron parsing and trigger code: A. `kernel/src/cognitive/scheduler.rs` uses `cron::Schedule` and a background task loop.
- Startup wiring: A. `builder.setup(...)` installs `ScheduledGoalExecutor` and calls `initialize_startup_schedules()`.

## 12. DESKTOP APP FEATURES
- Voice: E. Commands exist, but current UX is not end-to-end real by default. The Voice Assistant page is mostly status/start/load-model plus simulated transcript behavior.
- Terminal: A. Embedded terminal executes real governed commands through backend Tauri commands.
- Code editor: A for real file load/edit/save, E for AI coding assistant behavior. The core editor works on real files; the agent helper does not.
- File manager: A. Real filesystem contents and CRUD are wired.
- Database: A. SQLite connect/list/query path is real.
- Browser: A for loading real sites, E for full governed browser automation UI. The page loads real URLs, but the governance/automation story is partial.
- API client: A. Sends real HTTP requests through backend.

## APPENDIX: Key Notes By System
- `assign_agent_goal` exists but does not spawn the loop by itself; the actual UI uses `execute_agent_goal`.
- `Dashboard.tsx` is present but not routed.
- `VoiceAssistant.tsx` does not call `voice_stop_listening` or `voice_transcribe`.
- `web.search` is still placeholder output in the kernel actuator.
- `hivemind` currently combines three separate issues:
  - no frontend usage
  - stub LLM in `AppState`
  - simulated subtask completion instead of real agent dispatch

