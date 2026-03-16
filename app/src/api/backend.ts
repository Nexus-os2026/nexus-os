import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityMessage,
  AgentCardSummary,
  AgentSummary,
  AuditChainStatusRow,
  AuditEventRow,
  AvailableModel,
  ProviderModel,
  ProviderStatus,
  BrowserHistoryEntry,
  BrowserNavigateResult,
  CapabilityRequest,
  ChatResponse,
  ComplianceAgentRow,
  ComplianceStatusRow,
  FirewallPatterns,
  FirewallStatus,
  HardwareInfo,
  KnowledgeEntry,
  LearningSessionState,
  LearningSource,
  MarketplaceAgent,
  MarketplaceDetail,
  MarketplacePublishResult,
  IdentityInfo,
  PreinstalledAgent,
  LlmRecommendations,
  LlmStatus,
  McpTool,
  NexusConfig,
  OllamaStatus,
  PermissionCategory,
  PermissionHistoryEntry,
  PermissionUpdate,
  ScheduledAgent,
  PolicyConflict,
  PolicyEntry,
  PolicyTestResult,
  ProtocolRequest,
  ProtocolsStatus,
  BuildSessionState,
  ConductorBuildResponse,
  ProviderUsageStats,
  ResearchSessionState,
  SetupResult,
  ScreenRegion,
  SystemInfo,
  TestConnectionResult,
  ConsentNotification,
  TrustOverviewAgent,
  InputControlStatus,
  VoiceRuntimeState,
  PredictionReport,
  SimulationStatus,
  SimulationSummary
} from "../types";

interface TauriWindow extends Window {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
}

function runtimeWindow(): TauriWindow | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window as TauriWindow;
}

export function hasDesktopRuntime(): boolean {
  const runtime = runtimeWindow();
  if (!runtime) {
    return false;
  }
  return runtime.__TAURI__ !== undefined || runtime.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasDesktopRuntime()) {
    throw new Error("desktop runtime unavailable");
  }
  return invoke<T>(command, args);
}

async function invokeJsonDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const payload = await invokeDesktop<string>(command, args);
  return JSON.parse(payload) as T;
}

function agentArgs(agentId: string): Record<string, unknown> {
  return { agentId, agent_id: agentId };
}

export function listAgents(): Promise<AgentSummary[]> {
  return invokeDesktop<AgentSummary[]>("list_agents");
}

export function clearAllAgents(): Promise<number> {
  return invokeDesktop<number>("clear_all_agents");
}

export function createAgent(manifestJson: string): Promise<string> {
  return invokeDesktop<string>("create_agent", { manifestJson, manifest_json: manifestJson });
}

export function getAuditLog(agentId?: string, limit = 200): Promise<AuditEventRow[]> {
  return invokeDesktop<AuditEventRow[]>("get_audit_log", {
    agentId,
    agent_id: agentId,
    limit
  });
}

export function startAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("start_agent", agentArgs(agentId));
}

export function stopAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("stop_agent", agentArgs(agentId));
}

export function pauseAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("pause_agent", agentArgs(agentId));
}

export function resumeAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("resume_agent", agentArgs(agentId));
}

export function sendChat(message: string, modelId?: string): Promise<ChatResponse> {
  return invokeDesktop<ChatResponse>("send_chat", {
    message,
    modelId,
    model_id: modelId,
  });
}

export function getConfig(): Promise<NexusConfig> {
  return invokeDesktop<NexusConfig>("get_config");
}

export function saveConfig(config: NexusConfig): Promise<void> {
  return invokeDesktop<void>("save_config", { config });
}

export function transcribePushToTalk(): Promise<string> {
  return invokeDesktop<string>("transcribe_push_to_talk");
}

export function startJarvisMode(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("start_jarvis_mode");
}

export function stopJarvisMode(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("stop_jarvis_mode");
}

export function jarvisStatus(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("jarvis_status");
}

export function detectHardware(): Promise<HardwareInfo> {
  return invokeDesktop<HardwareInfo>("detect_hardware");
}

export function checkOllama(baseUrl?: string): Promise<OllamaStatus> {
  return invokeDesktop<OllamaStatus>("check_ollama", { baseUrl, base_url: baseUrl });
}

export function pullOllamaModel(modelName: string, baseUrl?: string): Promise<string> {
  return invokeDesktop<string>("pull_ollama_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function runSetupWizard(ollamaUrl?: string): Promise<SetupResult> {
  return invokeDesktop<SetupResult>("run_setup_wizard", {
    ollamaUrl,
    ollama_url: ollamaUrl
  });
}

export function pullModel(modelName: string, baseUrl?: string): Promise<string> {
  return invokeDesktop<string>("pull_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function ensureOllama(baseUrl?: string): Promise<boolean> {
  return invokeDesktop<boolean>("ensure_ollama", {
    baseUrl,
    base_url: baseUrl
  });
}

export function isOllamaInstalled(): Promise<boolean> {
  return invokeDesktop<boolean>("is_ollama_installed");
}

export function deleteModel(modelName: string, baseUrl?: string): Promise<void> {
  return invokeDesktop<void>("delete_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function isSetupComplete(): Promise<boolean> {
  return invokeDesktop<boolean>("is_setup_complete");
}

export function listAvailableModels(): Promise<AvailableModel[]> {
  return invokeDesktop<AvailableModel[]>("list_available_models");
}

export function listProviderModels(): Promise<ProviderModel[]> {
  return invokeDesktop<ProviderModel[]>("list_provider_models");
}

export function getProviderStatus(): Promise<ProviderStatus> {
  return invokeDesktop<ProviderStatus>("get_provider_status");
}

export function captureScreen(region?: ScreenRegion): Promise<string> {
  return invokeDesktop<string>("capture_screen", { region });
}

export function analyzeScreen(query: string): Promise<string> {
  return invokeDesktop<string>("analyze_screen", { query });
}

export function analyzeMediaFile(path: string, query: string): Promise<string> {
  return invokeDesktop<string>("analyze_media_file", { path, query });
}

export function startComputerAction(description: string, maxSteps = 20): Promise<string> {
  return invokeDesktop<string>("start_computer_action", {
    description,
    maxSteps,
    max_steps: maxSteps
  });
}

export function stopComputerAction(agentId: string): Promise<void> {
  return invokeDesktop<void>("stop_computer_action", {
    agentId,
    agent_id: agentId
  });
}

export function getInputControlStatus(): Promise<InputControlStatus> {
  return invokeDesktop<InputControlStatus>("get_input_control_status");
}

export function computerControlCaptureScreen(region?: string): Promise<Record<string, unknown>> {
  return invokeJsonDesktop<Record<string, unknown>>("computer_control_capture_screen", { region });
}

export function computerControlExecuteAction(
  actionJson: string,
): Promise<Record<string, unknown>> {
  return invokeJsonDesktop<Record<string, unknown>>("computer_control_execute_action", {
    actionJson,
    action_json: actionJson,
  });
}

export function computerControlGetHistory(): Promise<Record<string, unknown>[]> {
  return invokeJsonDesktop<Record<string, unknown>[]>("computer_control_get_history");
}

export function computerControlToggle(
  enabled: boolean,
): Promise<Record<string, unknown>> {
  return invokeJsonDesktop<Record<string, unknown>>("computer_control_toggle", { enabled });
}

export function computerControlStatus(): Promise<Record<string, unknown>> {
  return invokeJsonDesktop<Record<string, unknown>>("computer_control_status");
}

export function saveApiKey(provider: string, apiKey: string): Promise<void> {
  return invokeDesktop<void>("save_api_key", {
    provider,
    apiKey,
    api_key: apiKey,
  });
}

export function chatWithOllama(
  messages: Array<{ role: string; content: string }>,
  model: string,
  baseUrl?: string
): Promise<string> {
  return invokeDesktop<string>("chat_with_ollama", {
    messages,
    model,
    baseUrl,
    base_url: baseUrl
  });
}

export function setAgentModel(agent: string, model: string): Promise<void> {
  return invokeDesktop<void>("set_agent_model", { agent, model });
}

export function conductBuild(
  prompt: string,
  outputDir?: string,
  model?: string
): Promise<ConductorBuildResponse> {
  return invokeDesktop<ConductorBuildResponse>("conduct_build", {
    prompt,
    outputDir,
    output_dir: outputDir,
    model,
  });
}

export function getTrustOverview(): Promise<TrustOverviewAgent[]> {
  return invokeDesktop<TrustOverviewAgent[]>("get_trust_overview");
}

export function checkLlmStatus(): Promise<LlmStatus> {
  return invokeDesktop<LlmStatus>("check_llm_status");
}

export function getSystemInfo(): Promise<SystemInfo> {
  return invokeDesktop<SystemInfo>("get_system_info");
}

export interface TerminalCommandResult {
  stdout: string;
  stderr: string;
  exit_code: number;
  duration_ms: number;
  tool: string;
  needs_approval: boolean;
  fuel_cost: number;
}

export async function terminalExecute(
  command: string,
  cwd: string,
): Promise<TerminalCommandResult> {
  const payload = await invokeDesktop<string>("terminal_execute", { command, cwd });
  return JSON.parse(payload) as TerminalCommandResult;
}

export async function terminalExecuteApproved(
  command: string,
  cwd: string,
): Promise<TerminalCommandResult> {
  const payload = await invokeDesktop<string>("terminal_execute_approved", { command, cwd });
  return JSON.parse(payload) as TerminalCommandResult;
}

// ── Permission Dashboard API ──

export function getAgentPermissions(agentId: string): Promise<PermissionCategory[]> {
  return invokeDesktop<PermissionCategory[]>("get_agent_permissions", agentArgs(agentId));
}

export function updateAgentPermission(
  agentId: string,
  capabilityKey: string,
  enabled: boolean
): Promise<void> {
  return invokeDesktop<void>("update_agent_permission", {
    agentId,
    agent_id: agentId,
    capabilityKey,
    capability_key: capabilityKey,
    enabled
  });
}

export function getPermissionHistory(agentId: string): Promise<PermissionHistoryEntry[]> {
  return invokeDesktop<PermissionHistoryEntry[]>("get_permission_history", agentArgs(agentId));
}

export function getCapabilityRequest(agentId: string): Promise<CapabilityRequest[]> {
  return invokeDesktop<CapabilityRequest[]>("get_capability_request", agentArgs(agentId));
}

export function bulkUpdatePermissions(
  agentId: string,
  updates: PermissionUpdate[],
  reason?: string
): Promise<void> {
  return invokeDesktop<void>("bulk_update_permissions", {
    agentId,
    agent_id: agentId,
    updates,
    reason: reason ?? null
  });
}

// ── Protocols Dashboard API ──

export function getProtocolsStatus(): Promise<ProtocolsStatus> {
  return invokeDesktop<ProtocolsStatus>("get_protocols_status");
}

export function getProtocolsRequests(): Promise<ProtocolRequest[]> {
  return invokeDesktop<ProtocolRequest[]>("get_protocols_requests");
}

export function getMcpTools(): Promise<McpTool[]> {
  return invokeDesktop<McpTool[]>("get_mcp_tools");
}

export function getAgentCards(): Promise<AgentCardSummary[]> {
  return invokeDesktop<AgentCardSummary[]>("get_agent_cards");
}

export function getScheduledAgents(): Promise<ScheduledAgent[]> {
  return invokeDesktop<ScheduledAgent[]>("get_scheduled_agents");
}

// ── Identity API ──

export function getAgentIdentity(agentId: string): Promise<IdentityInfo> {
  return invokeDesktop<IdentityInfo>("get_agent_identity", agentArgs(agentId));
}

export function listIdentities(): Promise<IdentityInfo[]> {
  return invokeDesktop<IdentityInfo[]>("list_identities");
}

// ── Firewall API ──

export function getFirewallStatus(): Promise<FirewallStatus> {
  return invokeDesktop<FirewallStatus>("get_firewall_status");
}

export function getFirewallPatterns(): Promise<FirewallPatterns> {
  return invokeDesktop<FirewallPatterns>("get_firewall_patterns");
}

// ── Compliance Dashboard API ──

export function getComplianceStatus(): Promise<ComplianceStatusRow> {
  return invokeDesktop<ComplianceStatusRow>("get_compliance_status");
}

export function getComplianceAgents(): Promise<ComplianceAgentRow[]> {
  return invokeDesktop<ComplianceAgentRow[]>("get_compliance_agents");
}

// ── Audit Chain API ──

export function getAuditChainStatus(): Promise<AuditChainStatusRow> {
  return invokeDesktop<AuditChainStatusRow>("get_audit_chain_status");
}

// ── Marketplace API ──

export function marketplaceSearch(query: string): Promise<MarketplaceAgent[]> {
  return invokeDesktop<MarketplaceAgent[]>("marketplace_search", { query });
}

export function getPreinstalledAgents(): Promise<PreinstalledAgent[]> {
  return invokeDesktop<PreinstalledAgent[]>("get_preinstalled_agents");
}

export function marketplaceInstall(packageId: string): Promise<MarketplaceAgent> {
  return invokeDesktop<MarketplaceAgent>("marketplace_install", {
    packageId,
    package_id: packageId
  });
}

export function marketplaceInfo(agentId: string): Promise<MarketplaceDetail> {
  return invokeDesktop<MarketplaceDetail>("marketplace_info", {
    agentId,
    agent_id: agentId
  });
}

export function marketplacePublish(bundleJson: string): Promise<MarketplacePublishResult> {
  return invokeDesktop<MarketplacePublishResult>("marketplace_publish", {
    bundleJson,
    bundle_json: bundleJson
  });
}

export function marketplaceMyAgents(author: string): Promise<MarketplaceAgent[]> {
  return invokeDesktop<MarketplaceAgent[]>("marketplace_my_agents", { author });
}

// ── Agent Browser API ──

export function navigateTo(url: string): Promise<BrowserNavigateResult> {
  return invokeDesktop<BrowserNavigateResult>("navigate_to", { url });
}

export function getBrowserHistory(): Promise<BrowserHistoryEntry[]> {
  return invokeDesktop<BrowserHistoryEntry[]>("get_browser_history");
}

export function getAgentActivity(): Promise<ActivityMessage[]> {
  return invokeDesktop<ActivityMessage[]>("get_agent_activity");
}

// ── Research Mode API ──

export function startResearch(topic: string, numAgents: number): Promise<ResearchSessionState> {
  return invokeDesktop<ResearchSessionState>("start_research", {
    topic,
    num_agents: numAgents,
  });
}

export function researchAgentAction(
  sessionId: string,
  agentId: string,
  action: string,
  url?: string,
  content?: string,
): Promise<ResearchSessionState> {
  return invokeDesktop<ResearchSessionState>("research_agent_action", {
    session_id: sessionId,
    agent_id: agentId,
    action,
    url: url ?? null,
    content: content ?? null,
  });
}

export function completeResearch(sessionId: string): Promise<ResearchSessionState> {
  return invokeDesktop<ResearchSessionState>("complete_research", {
    session_id: sessionId,
  });
}

export function getResearchSession(sessionId: string): Promise<ResearchSessionState> {
  return invokeDesktop<ResearchSessionState>("get_research_session", {
    session_id: sessionId,
  });
}

export function listResearchSessions(): Promise<ResearchSessionState[]> {
  return invokeDesktop<ResearchSessionState[]>("list_research_sessions");
}

// ── Build Mode API ──

export function startBuild(description: string): Promise<BuildSessionState> {
  return invokeDesktop<BuildSessionState>("start_build", { description });
}

export function buildAppendCode(
  sessionId: string,
  delta: string,
  agentName: string,
): Promise<BuildSessionState> {
  return invokeDesktop<BuildSessionState>("build_append_code", {
    session_id: sessionId,
    delta,
    agent_name: agentName,
  });
}

export function buildAddMessage(
  sessionId: string,
  agentName: string,
  role: string,
  content: string,
): Promise<BuildSessionState> {
  return invokeDesktop<BuildSessionState>("build_add_message", {
    session_id: sessionId,
    agent_name: agentName,
    role,
    content,
  });
}

export function completeBuild(sessionId: string): Promise<BuildSessionState> {
  return invokeDesktop<BuildSessionState>("complete_build", {
    session_id: sessionId,
  });
}

export function getBuildSession(sessionId: string): Promise<BuildSessionState> {
  return invokeDesktop<BuildSessionState>("get_build_session", {
    session_id: sessionId,
  });
}

export function getBuildCode(sessionId: string): Promise<string> {
  return invokeDesktop<string>("get_build_code", { session_id: sessionId });
}

export function getBuildPreview(sessionId: string): Promise<string> {
  return invokeDesktop<string>("get_build_preview", { session_id: sessionId });
}

// ── Learn Mode API ──

export function startLearning(sources: LearningSource[]): Promise<LearningSessionState> {
  return invokeDesktop<LearningSessionState>("start_learning", { sources });
}

export function getKnowledgeBase(): Promise<KnowledgeEntry[]> {
  return invokeDesktop<KnowledgeEntry[]>("get_knowledge_base");
}

export function getLearningSession(sessionId: string): Promise<LearningSessionState> {
  return invokeDesktop<LearningSessionState>("get_learning_session", {
    session_id: sessionId,
  });
}

// ── Policy Engine API ──

export function policyList(): Promise<PolicyEntry[]> {
  return invokeDesktop<PolicyEntry[]>("policy_list");
}

export function policyValidate(tomlContent: string): Promise<PolicyEntry> {
  return invokeDesktop<PolicyEntry>("policy_validate", { tomlContent, toml_content: tomlContent });
}

export function policyTest(
  tomlContent: string,
  principal: string,
  action: string,
  resource: string
): Promise<PolicyTestResult> {
  return invokeDesktop<PolicyTestResult>("policy_test", {
    tomlContent,
    toml_content: tomlContent,
    principal,
    action,
    resource,
  });
}

export function policyDetectConflicts(): Promise<PolicyConflict[]> {
  return invokeDesktop<PolicyConflict[]>("policy_detect_conflicts");
}

export function learningAgentAction(
  sessionId: string,
  action: string,
  url?: string,
  content?: string,
): Promise<LearningSessionState> {
  return invokeDesktop<LearningSessionState>("learning_agent_action", {
    session_id: sessionId,
    action,
    url: url ?? null,
    content: content ?? null,
  });
}

// ── LLM Provider Management API ──

export function getLlmRecommendations(): Promise<LlmRecommendations> {
  return invokeDesktop<LlmRecommendations>("get_llm_recommendations");
}

export function setAgentLlmProvider(
  agentId: string,
  providerId: string,
  localOnly: boolean,
  budgetDollars: number,
  budgetTokens: number,
): Promise<void> {
  return invokeDesktop<void>("set_agent_llm_provider", {
    agentId,
    agent_id: agentId,
    providerId,
    provider_id: providerId,
    localOnly,
    local_only: localOnly,
    budgetDollars,
    budget_dollars: budgetDollars,
    budgetTokens,
    budget_tokens: budgetTokens,
  });
}

export function getProviderUsageStats(): Promise<ProviderUsageStats[]> {
  return invokeDesktop<ProviderUsageStats[]>("get_provider_usage_stats");
}

export function testLlmConnection(providerName: string): Promise<TestConnectionResult> {
  return invokeDesktop<TestConnectionResult>("test_llm_connection", {
    providerName,
    provider_name: providerName,
  });
}

// ── RAG Pipeline API ──

export function indexDocument(filePath: string): Promise<string> {
  return invokeDesktop<string>("index_document", {
    filePath,
    file_path: filePath,
  });
}

export function searchDocuments(query: string, topK?: number): Promise<string> {
  return invokeDesktop<string>("search_documents", {
    query,
    topK: topK ?? null,
    top_k: topK ?? null,
  });
}

export function chatWithDocuments(question: string): Promise<string> {
  return invokeDesktop<string>("chat_with_documents", { question });
}

export function listIndexedDocuments(): Promise<string> {
  return invokeDesktop<string>("list_indexed_documents");
}

export function removeIndexedDocument(docPath: string): Promise<string> {
  return invokeDesktop<string>("remove_indexed_document", {
    docPath,
    doc_path: docPath,
  });
}

export function getDocumentGovernance(docPath: string): Promise<string> {
  return invokeDesktop<string>("get_document_governance", {
    docPath,
    doc_path: docPath,
  });
}

export function getSemanticMap(): Promise<string> {
  return invokeDesktop<string>("get_semantic_map");
}

export function getDocumentAccessLog(docPath: string): Promise<string> {
  return invokeDesktop<string>("get_document_access_log", {
    docPath,
    doc_path: docPath,
  });
}

// ── Model Hub API ──

export function searchModels(query: string, limit?: number): Promise<string> {
  return invokeDesktop<string>("search_models", { query, limit: limit ?? null });
}

export function getModelInfo(modelId: string): Promise<string> {
  return invokeDesktop<string>("get_model_info", {
    modelId,
    model_id: modelId,
  });
}

export function checkModelCompatibility(fileSizeBytes: number): Promise<string> {
  return invokeDesktop<string>("check_model_compatibility", {
    fileSizeBytes,
    file_size_bytes: fileSizeBytes,
  });
}

export function downloadModel(modelId: string, filename: string): Promise<string> {
  return invokeDesktop<string>("download_model", {
    modelId,
    model_id: modelId,
    filename,
  });
}

export function listLocalModels(): Promise<string> {
  return invokeDesktop<string>("list_local_models");
}

export function deleteLocalModel(modelId: string): Promise<string> {
  return invokeDesktop<string>("delete_local_model", {
    modelId,
    model_id: modelId,
  });
}

export function getSystemSpecs(): Promise<string> {
  return invokeDesktop<string>("get_system_specs");
}

export function getLiveSystemMetrics(): Promise<string> {
  return invokeDesktop<string>("get_live_system_metrics");
}

export function getLiveSystemMetricsJson<T = Record<string, unknown>>(): Promise<T> {
  return invokeJsonDesktop<T>("get_live_system_metrics");
}

// ── Time Machine API ──

export function timeMachineListCheckpoints(): Promise<string> {
  return invokeDesktop<string>("time_machine_list_checkpoints");
}

export function timeMachineGetCheckpoint(id: string): Promise<string> {
  return invokeDesktop<string>("time_machine_get_checkpoint", { id });
}

export function timeMachineCreateCheckpoint(label: string): Promise<string> {
  return invokeDesktop<string>("time_machine_create_checkpoint", { label });
}

export function timeMachineUndo(): Promise<string> {
  return invokeDesktop<string>("time_machine_undo");
}

export function timeMachineUndoCheckpoint(id: string): Promise<string> {
  return invokeDesktop<string>("time_machine_undo_checkpoint", { id });
}

export function timeMachineRedo(): Promise<string> {
  return invokeDesktop<string>("time_machine_redo");
}

export function timeMachineGetDiff(id: string): Promise<string> {
  return invokeDesktop<string>("time_machine_get_diff", { id });
}

export function timeMachineWhatIf(id: string, variableKey: string, variableValue: string): Promise<string> {
  return invokeDesktop<string>("time_machine_what_if", {
    id,
    variableKey,
    variable_key: variableKey,
    variableValue,
    variable_value: variableValue
  });
}

// ── Voice Assistant API ──

export function voiceStartListening(): Promise<string> {
  return invokeDesktop<string>("voice_start_listening");
}

export function voiceStopListening(): Promise<string> {
  return invokeDesktop<string>("voice_stop_listening");
}

export function voiceGetStatus(): Promise<string> {
  return invokeDesktop<string>("voice_get_status");
}

export function voiceTranscribe(audioBase64: string): Promise<string> {
  return invokeDesktop<string>("voice_transcribe", {
    audioBase64,
    audio_base64: audioBase64,
  });
}

export function voiceLoadWhisperModel(modelPath: string): Promise<string> {
  return invokeDesktop<string>("voice_load_whisper_model", {
    modelPath,
    model_path: modelPath,
  });
}

// ── Email Client API ──

export function emailList(): Promise<string> {
  return invokeDesktop<string>("email_list");
}

export function emailSave(id: string, dataJson: string): Promise<string> {
  return invokeDesktop<string>("email_save", { id, data_json: dataJson });
}

export function emailDelete(id: string): Promise<string> {
  return invokeDesktop<string>("email_delete", { id });
}

// ── Project Manager API ──

export function projectList(): Promise<string> {
  return invokeDesktop<string>("project_list");
}

export function projectGet(id: string): Promise<string> {
  return invokeDesktop<string>("project_get", { id });
}

export function projectSave(id: string, dataJson: string): Promise<string> {
  return invokeDesktop<string>("project_save", { id, data_json: dataJson });
}

export function fileManagerList<T = Record<string, unknown>>(path: string): Promise<T[]> {
  return invokeJsonDesktop<T[]>("file_manager_list", { path });
}

export function fileManagerWrite(path: string, content: string): Promise<string> {
  return invokeDesktop<string>("file_manager_write", { path, content });
}

export function fileManagerRead(path: string): Promise<string> {
  return invokeDesktop<string>("file_manager_read", { path });
}

export function fileManagerCreateDir(path: string): Promise<string> {
  return invokeDesktop<string>("file_manager_create_dir", { path });
}

export function projectDelete(id: string): Promise<string> {
  return invokeDesktop<string>("project_delete", { id });
}

// ── Factory Pipeline API ──

export function factoryCreateProject(
  name: string,
  language: string,
  sourceDir: string,
): Promise<string> {
  return invokeDesktop<string>("factory_create_project", {
    name,
    language,
    source_dir: sourceDir,
  });
}

export function factoryBuildProject(projectId: string): Promise<string> {
  return invokeDesktop<string>("factory_build_project", {
    projectId,
    project_id: projectId,
  });
}

export function factoryTestProject(projectId: string): Promise<string> {
  return invokeDesktop<string>("factory_test_project", {
    projectId,
    project_id: projectId,
  });
}

export function factoryRunPipeline(projectId: string): Promise<string> {
  return invokeDesktop<string>("factory_run_pipeline", {
    projectId,
    project_id: projectId,
  });
}

export function factoryListProjects(): Promise<string> {
  return invokeDesktop<string>("factory_list_projects");
}

export function factoryGetBuildHistory(projectId: string): Promise<string> {
  return invokeDesktop<string>("factory_get_build_history", {
    projectId,
    project_id: projectId,
  });
}

// ── Cognitive Runtime API ──

export function assignAgentGoal(
  agentId: string,
  goalDescription: string,
  priority: number,
): Promise<string> {
  return invokeDesktop<string>("assign_agent_goal", {
    agentId,
    agent_id: agentId,
    goalDescription,
    goal_description: goalDescription,
    priority,
  });
}

/** Execute an agent goal end-to-end: assigns the goal and runs the cognitive
 *  loop in the background. Listen to `agent-cognitive-cycle` and
 *  `agent-goal-completed` Tauri events for progress updates. */
export function executeAgentGoal(
  agentId: string,
  goalDescription: string,
  priority: number,
): Promise<string> {
  return invokeDesktop<string>("execute_agent_goal", {
    agentId,
    agent_id: agentId,
    goalDescription,
    goal_description: goalDescription,
    priority,
  });
}

export function stopAgentGoal(agentId: string): Promise<void> {
  return invokeDesktop<void>("stop_agent_goal", {
    agentId,
    agent_id: agentId,
  });
}

export function getAgentCognitiveStatus(
  agentId: string,
): Promise<Record<string, unknown>> {
  return invokeDesktop<Record<string, unknown>>("get_agent_cognitive_status", {
    agentId,
    agent_id: agentId,
  });
}

export function getAgentTaskHistory(
  agentId: string,
  limit = 50,
): Promise<Record<string, unknown>[]> {
  return invokeDesktop<Record<string, unknown>[]>("get_agent_task_history", {
    agentId,
    agent_id: agentId,
    limit,
  });
}

export function getAgentMemories(
  agentId: string,
  memoryType: string | null = null,
  limit = 50,
): Promise<Record<string, unknown>[]> {
  return invokeDesktop<Record<string, unknown>[]>("get_agent_memories", {
    agentId,
    agent_id: agentId,
    memoryType,
    memory_type: memoryType,
    limit,
  });
}

// ── Agent Memory API ──

export function agentMemoryRemember(
  agentId: string,
  content: string,
  memoryType: string,
  importance: number,
  tags: string[],
): Promise<string> {
  return invokeDesktop<string>("agent_memory_remember", {
    agentId,
    agent_id: agentId,
    content,
    memoryType,
    memory_type: memoryType,
    importance,
    tags,
  });
}

export function agentMemoryRecall(
  agentId: string,
  query: string,
  maxResults?: number,
): Promise<string> {
  return invokeDesktop<string>("agent_memory_recall", {
    agentId,
    agent_id: agentId,
    query,
    maxResults: maxResults ?? null,
    max_results: maxResults ?? null,
  });
}

export function agentMemoryRecallByType(
  agentId: string,
  memoryType: string,
  maxResults?: number,
): Promise<string> {
  return invokeDesktop<string>("agent_memory_recall_by_type", {
    agentId,
    agent_id: agentId,
    memoryType,
    memory_type: memoryType,
    maxResults: maxResults ?? null,
    max_results: maxResults ?? null,
  });
}

export function agentMemoryForget(agentId: string, memoryId: string): Promise<string> {
  return invokeDesktop<string>("agent_memory_forget", {
    agentId,
    agent_id: agentId,
    memoryId,
    memory_id: memoryId,
  });
}

export function agentMemoryGetStats(agentId: string): Promise<string> {
  return invokeDesktop<string>("agent_memory_get_stats", agentArgs(agentId));
}

export function agentMemorySave(agentId: string): Promise<string> {
  return invokeDesktop<string>("agent_memory_save", agentArgs(agentId));
}

export function agentMemoryClear(agentId: string): Promise<string> {
  return invokeDesktop<string>("agent_memory_clear", agentArgs(agentId));
}

// ── Messaging Gateway API ──

export function getMessagingStatus<T = Record<string, unknown>>(): Promise<T[]> {
  return invokeDesktop<T[]>("get_messaging_status");
}

export function setDefaultAgent(userId: string, agentId: string): Promise<void> {
  return invokeDesktop<void>("set_default_agent", {
    userId,
    user_id: userId,
    agentId,
    agent_id: agentId,
  });
}

// ── Self-Evolution API ──

export function getSelfEvolutionMetrics(
  agentId: string,
): Promise<Record<string, unknown>> {
  return invokeDesktop<Record<string, unknown>>("get_self_evolution_metrics", {
    agentId,
    agent_id: agentId,
  });
}

export function getSelfEvolutionStrategies(
  agentId: string,
): Promise<Record<string, unknown>[]> {
  return invokeDesktop<Record<string, unknown>[]>("get_self_evolution_strategies", {
    agentId,
    agent_id: agentId,
  });
}

export function triggerCrossAgentLearning(): Promise<number> {
  return invokeDesktop<number>("trigger_cross_agent_learning");
}

// ── Hivemind Orchestration API ──

export function startHivemind(
  goal: string,
  agentIds: string[],
): Promise<Record<string, unknown>> {
  return invokeDesktop<Record<string, unknown>>("start_hivemind", {
    goal,
    agentIds,
    agent_ids: agentIds,
  });
}

export function getHivemindStatus(
  sessionId: string,
): Promise<Record<string, unknown>> {
  return invokeDesktop<Record<string, unknown>>("get_hivemind_status", {
    sessionId,
    session_id: sessionId,
  });
}

export function cancelHivemind(sessionId: string): Promise<void> {
  return invokeDesktop<void>("cancel_hivemind", {
    sessionId,
    session_id: sessionId,
  });
}

// ── Consent / HITL Approval API ──

export function approveConsentRequest(
  consentId: string,
  approvedBy: string,
): Promise<void> {
  return invokeDesktop<void>("approve_consent_request", {
    consentId,
    consent_id: consentId,
    approvedBy,
    approved_by: approvedBy,
  });
}

export function denyConsentRequest(
  consentId: string,
  deniedBy: string,
  reason?: string,
): Promise<void> {
  return invokeDesktop<void>("deny_consent_request", {
    consentId,
    consent_id: consentId,
    deniedBy,
    denied_by: deniedBy,
    reason: reason ?? null,
  });
}

export function batchApproveConsents(
  goalId: string,
  approvedBy: string,
): Promise<void> {
  return invokeDesktop<void>("batch_approve_consents", {
    goalId,
    goal_id: goalId,
    approvedBy,
    approved_by: approvedBy,
  });
}

export function reviewConsentBatch(
  consentId: string,
  reviewedBy: string,
): Promise<void> {
  return invokeDesktop<void>("review_consent_batch", {
    consentId,
    consent_id: consentId,
    reviewedBy,
    reviewed_by: reviewedBy,
  });
}

export function batchDenyConsents(
  goalId: string,
  deniedBy: string,
  reason?: string,
): Promise<void> {
  return invokeDesktop<void>("batch_deny_consents", {
    goalId,
    goal_id: goalId,
    deniedBy,
    denied_by: deniedBy,
    reason: reason ?? null,
  });
}

export function listPendingConsents(): Promise<ConsentNotification[]> {
  return invokeDesktop<ConsentNotification[]>("list_pending_consents");
}

export function getConsentHistory(limit = 20): Promise<ConsentNotification[]> {
  return invokeDesktop<ConsentNotification[]>("get_consent_history", { limit });
}

export function createSimulation(
  name: string,
  seedText: string,
  personaCount: number,
  maxTicks: number,
  tickIntervalMs?: number,
): Promise<string> {
  return invokeDesktop<string>("create_simulation", {
    name,
    seedText,
    seed_text: seedText,
    personaCount,
    persona_count: personaCount,
    maxTicks,
    max_ticks: maxTicks,
    tickIntervalMs,
    tick_interval_ms: tickIntervalMs,
  });
}

export function startSimulation(worldId: string): Promise<void> {
  return invokeDesktop<void>("start_simulation", {
    worldId,
    world_id: worldId,
  });
}

export function pauseSimulation(worldId: string): Promise<void> {
  return invokeDesktop<void>("pause_simulation", {
    worldId,
    world_id: worldId,
  });
}

export function injectSimulationVariable(
  worldId: string,
  key: string,
  value: string,
): Promise<void> {
  return invokeDesktop<void>("inject_variable", {
    worldId,
    world_id: worldId,
    key,
    value,
  });
}

export function getSimulationStatus(worldId: string): Promise<SimulationStatus> {
  return invokeDesktop<SimulationStatus>("get_simulation_status", {
    worldId,
    world_id: worldId,
  });
}

export function getSimulationReport(worldId: string): Promise<PredictionReport> {
  return invokeDesktop<PredictionReport>("get_simulation_report", {
    worldId,
    world_id: worldId,
  });
}

export function chatWithSimulationPersona(
  worldId: string,
  personaId: string,
  message: string,
): Promise<string> {
  return invokeDesktop<string>("chat_with_persona", {
    worldId,
    world_id: worldId,
    personaId,
    persona_id: personaId,
    message,
  });
}

export function listSimulations(): Promise<SimulationSummary[]> {
  return invokeDesktop<SimulationSummary[]>("list_simulations");
}

export function runParallelSimulations(
  seedText: string,
  variantCount: number,
): Promise<PredictionReport[]> {
  return invokeDesktop<PredictionReport[]>("run_parallel_simulations", {
    seedText,
    seed_text: seedText,
    variantCount,
    variant_count: variantCount,
  });
}
