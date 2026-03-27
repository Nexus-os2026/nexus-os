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

export function sendChat(message: string, modelId?: string, agentName?: string): Promise<ChatResponse> {
  return invokeDesktop<ChatResponse>("send_chat", {
    message,
    modelId,
    model_id: modelId,
    agent_name: agentName,
  });
}

// ── Auto-Evolution API ──

export interface AgentPerformanceTracker {
  agent_id: string;
  recent_scores: number[];
  running_average: number;
  improvement_threshold: number;
  evolution_cooldown_secs: number;
  total_tasks: number;
  total_evolutions: number;
  successful_evolutions: number;
  evolution_enabled: boolean;
}

export interface EvolutionEvent {
  agent_id: string;
  timestamp: number;
  old_score: number;
  new_score: number;
  success: boolean;
  prompt_diff_summary: string;
}

export interface EvolutionResult {
  agent_id: string;
  improved: boolean;
  old_score: number;
  new_score: number;
  message: string;
}

export function getAgentPerformance(agentId: string): Promise<AgentPerformanceTracker> {
  return invokeDesktop<AgentPerformanceTracker>("get_agent_performance", { agent_id: agentId });
}

export function getAutoEvolutionLog(agentId: string, limit: number = 20): Promise<EvolutionEvent[]> {
  return invokeDesktop<EvolutionEvent[]>("get_auto_evolution_log", { agent_id: agentId, limit });
}

export function setAutoEvolutionConfig(
  agentId: string,
  enabled: boolean,
  threshold: number,
  cooldownSeconds: number,
): Promise<void> {
  return invokeDesktop<void>("set_auto_evolution_config", {
    agent_id: agentId,
    enabled,
    threshold,
    cooldown_seconds: cooldownSeconds,
  });
}

export function forceEvolveAgent(agentId: string): Promise<EvolutionResult> {
  return invokeDesktop<EvolutionResult>("force_evolve_agent", { agent_id: agentId });
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

export interface AvailableProvider {
  id: string;
  name: string;
  status: string;
  available: boolean;
  model: string | null;
  models: string[];
  model_paths: string[];
}

export function getAvailableProviders(): Promise<AvailableProvider[]> {
  return invokeDesktop<AvailableProvider[]>("get_available_providers");
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

// ── A2A Client API ──

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function a2aDiscoverAgent(url: string): Promise<any> {
  return invokeDesktop("a2a_discover_agent", { url });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function a2aSendTask(agentUrl: string, message: string): Promise<any> {
  return invokeDesktop("a2a_send_task", { agentUrl: agentUrl, agent_url: agentUrl, message });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function a2aGetTaskStatus(agentUrl: string, taskId: string): Promise<any> {
  return invokeDesktop("a2a_get_task_status", { agentUrl: agentUrl, agent_url: agentUrl, taskId: taskId, task_id: taskId });
}

export function a2aCancelTask(agentUrl: string, taskId: string): Promise<void> {
  return invokeDesktop("a2a_cancel_task", { agentUrl: agentUrl, agent_url: agentUrl, taskId: taskId, task_id: taskId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function a2aKnownAgents(): Promise<any> {
  return invokeDesktop("a2a_known_agents");
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

// ── Email OAuth2 API ──

export function emailStartOauth(provider: string): Promise<string> {
  return invokeDesktop<string>("email_start_oauth", { provider });
}

export function emailOauthStatus(): Promise<string> {
  return invokeDesktop<string>("email_oauth_status");
}

export function emailFetchMessages(provider: string, folder: string, page: number): Promise<string> {
  return invokeDesktop<string>("email_fetch_messages", { provider, folder, page });
}

export function emailSendMessage(provider: string, to: string, subject: string, body: string): Promise<string> {
  return invokeDesktop<string>("email_send_message", { provider, to, subject, body });
}

export function emailSearchMessages(provider: string, query: string): Promise<string> {
  return invokeDesktop<string>("email_search_messages", { provider, query });
}

export function emailDisconnect(provider: string): Promise<string> {
  return invokeDesktop<string>("email_disconnect", { provider });
}

// ── Messaging Platform API ──

export function messagingConnectPlatform(platform: string, tokenValue: string): Promise<string> {
  return invokeDesktop<string>("messaging_connect_platform", { platform, token_value: tokenValue });
}

export function messagingSend(platform: string, channel: string, text: string): Promise<string> {
  return invokeDesktop<string>("messaging_send", { platform, channel, text });
}

export function messagingPollMessages(platform: string, channel: string, lastId: string): Promise<string> {
  return invokeDesktop<string>("messaging_poll_messages", { platform, channel, last_id: lastId });
}

// ── Integration OAuth API ──

export function integrationStartOauth(providerId: string): Promise<string> {
  return invokeDesktop<string>("integration_start_oauth", { provider_id: providerId });
}

// ── Marketplace GitLab API ──

export function marketplaceSearchGitlab(query: string): Promise<string> {
  return invokeDesktop<string>("marketplace_search_gitlab", { query });
}

// ── Agent Output API ──

export function getAgentOutputs(agentId: string, limit: number): Promise<string> {
  return invokeDesktop<string>("get_agent_outputs", { agent_id: agentId, limit });
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

/** Start an autonomous agent loop — the agent runs its default goal on a
 *  recurring interval. If intervalSeconds is omitted, defaults to 60s. */
export function startAutonomousLoop(
  agentId: string,
  intervalSeconds?: number,
  goalOverride?: string,
): Promise<void> {
  return invokeDesktop<void>("start_autonomous_loop", {
    agentId,
    agent_id: agentId,
    intervalSeconds,
    interval_seconds: intervalSeconds,
    goalOverride,
    goal_override: goalOverride,
  });
}

/** Stop an autonomous agent loop (unregister from scheduler). */
export function stopAutonomousLoop(agentId: string): Promise<void> {
  return invokeDesktop<void>("stop_autonomous_loop", {
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

export function setAgentReviewMode(agentId: string, reviewEach: boolean): Promise<void> {
  return invokeDesktop<void>("set_agent_review_mode", {
    agentId,
    agent_id: agentId,
    reviewEach,
    review_each: reviewEach,
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

export interface HitlStats {
  pending_count: number;
  approval_rate: number;
  avg_response_time_ms: number;
  total_decisions_today: number;
  total_approvals: number;
  total_denials: number;
}

export function hitlStats(): Promise<HitlStats> {
  return invokeDesktop<HitlStats>("hitl_stats");
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

// ── Self-Improving OS ───────────────────────────────────────────────────────

export function getOsFitness(): Promise<string> {
  return invokeDesktop<string>("get_os_fitness");
}

export function getFitnessHistory(days: number): Promise<string> {
  return invokeDesktop<string>("get_fitness_history", { days });
}

export function getRoutingStats(): Promise<string> {
  return invokeDesktop<string>("get_routing_stats");
}

export function getUiAdaptations(): Promise<string> {
  return invokeDesktop<string>("get_ui_adaptations");
}

export function getUserProfile(): Promise<string> {
  return invokeDesktop<string>("get_user_profile");
}

export function recordPageVisit(page: string): Promise<void> {
  return invokeDesktop<void>("record_page_visit", { page });
}

export function recordFeatureUse(feature: string): Promise<void> {
  return invokeDesktop<void>("record_feature_use", { feature });
}

export function overrideSecurityBlock(
  eventId: string,
  ruleId: string,
): Promise<void> {
  return invokeDesktop<void>("override_security_block", {
    eventId,
    event_id: eventId,
    ruleId,
    rule_id: ruleId,
  });
}

export function getOsImprovementLog(limit: number): Promise<string> {
  return invokeDesktop<string>("get_os_improvement_log", { limit });
}

export function getMorningOsBriefing(): Promise<string> {
  return invokeDesktop<string>("get_morning_os_briefing");
}

export function recordRoutingOutcome(
  category: string,
  agentId: string,
  score: number,
): Promise<void> {
  return invokeDesktop<void>("record_routing_outcome", {
    category,
    agentId,
    agent_id: agentId,
    score,
  });
}

export function recordOperationTiming(
  operation: string,
  latencyMs: number,
): Promise<void> {
  return invokeDesktop<void>("record_operation_timing", {
    operation,
    latencyMs,
    latency_ms: latencyMs,
  });
}

export function getPerformanceReport(): Promise<string> {
  return invokeDesktop<string>("get_performance_report");
}

export function getSecurityEvolutionReport(): Promise<string> {
  return invokeDesktop<string>("get_security_evolution_report");
}

export function recordKnowledgeInteraction(
  topic: string,
  languages: string[],
  score: number,
): Promise<void> {
  return invokeDesktop<void>("record_knowledge_interaction", {
    topic,
    languages,
    score,
  });
}

export function getOsDreamStatus(): Promise<string> {
  return invokeDesktop<string>("get_os_dream_status");
}

export function setSelfImproveEnabled(enabled: boolean): Promise<void> {
  return invokeDesktop<void>("set_self_improve_enabled", { enabled });
}

// ── Killer Features: Screenshot Clone ──

export function screenshotAnalyze(imagePath: string): Promise<string> {
  return invokeDesktop<string>("screenshot_analyze", { imagePath });
}

export function screenshotGenerateSpec(
  analysisJson: string,
  projectName: string,
): Promise<string> {
  return invokeDesktop<string>("screenshot_generate_spec", {
    analysisJson,
    projectName,
  });
}

// ── Killer Features: Voice Project ──

export function voiceProjectStart(): Promise<void> {
  return invokeDesktop<void>("voice_project_start");
}

export function voiceProjectStop(): Promise<string> {
  return invokeDesktop<string>("voice_project_stop");
}

export function voiceProjectAddChunk(
  text: string,
  timestamp: number,
): Promise<void> {
  return invokeDesktop<void>("voice_project_add_chunk", { text, timestamp });
}

export function voiceProjectGetStatus(): Promise<string> {
  return invokeDesktop<string>("voice_project_get_status");
}

export function voiceProjectGetPrompt(): Promise<string> {
  return invokeDesktop<string>("voice_project_get_prompt");
}

export function voiceProjectUpdateIntent(
  response: string,
  timestamp: number,
): Promise<string> {
  return invokeDesktop<string>("voice_project_update_intent", {
    response,
    timestamp,
  });
}

// ── Killer Features: Stress Test ──

export function stressGeneratePersonas(count: number): Promise<string> {
  return invokeDesktop<string>("stress_generate_personas", { count });
}

export function stressGenerateActions(personaJson: string): Promise<string> {
  return invokeDesktop<string>("stress_generate_actions", { personaJson });
}

export function stressEvaluateReport(reportJson: string): Promise<string> {
  return invokeDesktop<string>("stress_evaluate_report", { reportJson });
}

// ── Killer Features: Deploy ──

export function deployGenerateDockerfile(configJson: string): Promise<string> {
  return invokeDesktop<string>("deploy_generate_dockerfile", { configJson });
}

export function deployValidateConfig(configJson: string): Promise<string> {
  return invokeDesktop<string>("deploy_validate_config", { configJson });
}

export function deployGetCommands(configJson: string): Promise<string> {
  return invokeDesktop<string>("deploy_get_commands", { configJson });
}

// ── Killer Features: Live Evolution ──

export function evolverRegisterApp(appJson: string): Promise<void> {
  return invokeDesktop<void>("evolver_register_app", { appJson });
}

export function evolverUnregisterApp(projectId: string): Promise<boolean> {
  return invokeDesktop<boolean>("evolver_unregister_app", { projectId });
}

export function evolverListApps(): Promise<string> {
  return invokeDesktop<string>("evolver_list_apps");
}

export function evolverDetectIssues(metricsJson: string): Promise<string> {
  return invokeDesktop<string>("evolver_detect_issues", { metricsJson });
}

// ── Killer Features: Freelance Engine ──

export function freelanceGetStatus(): Promise<string> {
  return invokeDesktop<string>("freelance_get_status");
}

export function freelanceStartScanning(): Promise<void> {
  return invokeDesktop<void>("freelance_start_scanning");
}

export function freelanceStopScanning(): Promise<void> {
  return invokeDesktop<void>("freelance_stop_scanning");
}

export function freelanceEvaluateJob(jobJson: string): Promise<string> {
  return invokeDesktop<string>("freelance_evaluate_job", { jobJson });
}

export function freelanceGetRevenue(): Promise<string> {
  return invokeDesktop<string>("freelance_get_revenue");
}

// ── Experience Layer: Conversational Builder, Remix, Teach Mode ──

export function startConversationalBuild(message: string): Promise<string> {
  return invokeDesktop<string>("start_conversational_build", { message });
}

export function builderRespond(message: string): Promise<string> {
  return invokeDesktop<string>("builder_respond", { message });
}

export function getLivePreview(projectId: string): Promise<string> {
  return invokeDesktop<string>("get_live_preview", { projectId });
}

export function remixProject(projectId: string, change: string): Promise<string> {
  return invokeDesktop<string>("remix_project", { projectId, change });
}

export function analyzeProblem(problem: string): Promise<string> {
  return invokeDesktop<string>("analyze_problem", { problem });
}

export function publishToMarketplace(projectId: string, pricing: string): Promise<string> {
  return invokeDesktop<string>("publish_to_marketplace", { projectId, pricing });
}

export function installFromMarketplace(listingId: string): Promise<string> {
  return invokeDesktop<string>("install_from_marketplace", { listingId });
}

export function startTeachMode(projectId: string): Promise<string> {
  return invokeDesktop<string>("start_teach_mode", { projectId });
}

export function teachModeRespond(projectId: string, response: string): Promise<string> {
  return invokeDesktop<string>("teach_mode_respond", { projectId, response });
}

// ============ AIRGAP ============

export function airgapCreateBundle(
  targetOs: string,
  targetArch: string,
  outputPath: string,
  components?: string,
): Promise<string> {
  return invokeDesktop<string>("airgap_create_bundle", {
    targetOs, target_os: targetOs,
    targetArch, target_arch: targetArch,
    outputPath, output_path: outputPath,
    components: components ?? null,
  });
}

export function airgapGetSystemInfo(): Promise<string> {
  return invokeDesktop<string>("airgap_get_system_info");
}

export function airgapInstallBundle(bundlePath: string, installDir: string): Promise<string> {
  return invokeDesktop<string>("airgap_install_bundle", {
    bundlePath, bundle_path: bundlePath,
    installDir, install_dir: installDir,
  });
}

export function airgapValidateBundle(bundlePath: string): Promise<string> {
  return invokeDesktop<string>("airgap_validate_bundle", {
    bundlePath, bundle_path: bundlePath,
  });
}

// ============ CIVILIZATION (extra) ============

export function civGetEconomyStatus(): Promise<string> {
  return invokeJsonDesktop<string>("civ_get_economy_status");
}

export function civGetGovernanceLog(limit = 100): Promise<string> {
  return invokeJsonDesktop<string>("civ_get_governance_log", { limit });
}

export function civGetParliamentStatus(): Promise<string> {
  return invokeJsonDesktop<string>("civ_get_parliament_status");
}

export function civGetRoles(): Promise<string> {
  return invokeJsonDesktop<string>("civ_get_roles");
}

export function civProposeRule(proposerId: string, ruleText: string): Promise<string> {
  return invokeJsonDesktop<string>("civ_propose_rule", {
    proposerId, proposer_id: proposerId,
    ruleText, rule_text: ruleText,
  });
}

export function civResolveDispute(agentA: string, agentB: string, issue: string): Promise<string> {
  return invokeJsonDesktop<string>("civ_resolve_dispute", {
    agentA, agent_a: agentA,
    agentB, agent_b: agentB,
    issue,
  });
}

// ============ COGFS (extra) ============

export function cogfsGetContext(topic: string): Promise<string> {
  return invokeJsonDesktop<string>("cogfs_get_context", { topic });
}

export function cogfsGetEntities(filePath: string): Promise<string> {
  return invokeJsonDesktop<string>("cogfs_get_entities", { filePath, file_path: filePath });
}

export function cogfsGetGraph(filePath: string): Promise<string> {
  return invokeJsonDesktop<string>("cogfs_get_graph", { filePath, file_path: filePath });
}

// ============ ECONOMY ============

export function economyCreateWallet(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_create_wallet", agentArgs(agentId));
}

export function economyGetWallet(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_get_wallet", agentArgs(agentId));
}

export function economyEarn(agentId: string, amount: number, description: string): Promise<string> {
  return invokeDesktop<string>("economy_earn", {
    agentId, agent_id: agentId, amount, description,
  });
}

export function economySpend(
  agentId: string, amount: number, txType: string, description: string,
): Promise<string> {
  return invokeDesktop<string>("economy_spend", {
    agentId, agent_id: agentId, amount,
    txType, tx_type: txType, description,
  });
}

export function economyTransfer(
  from: string, to: string, amount: number, description: string,
): Promise<string> {
  return invokeDesktop<string>("economy_transfer", { from, to, amount, description });
}

export function economyFreezeWallet(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_freeze_wallet", agentArgs(agentId));
}

export function economyGetHistory(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_get_history", agentArgs(agentId));
}

export function economyGetStats(): Promise<string> {
  return invokeDesktop<string>("economy_get_stats");
}

export function economyCreateContract(
  agentId: string, clientId: string, description: string,
  criteriaJson: string, reward: number, penalty: number, deadline?: number,
): Promise<string> {
  return invokeDesktop<string>("economy_create_contract", {
    agentId, agent_id: agentId,
    clientId, client_id: clientId,
    description, criteriaJson, criteria_json: criteriaJson,
    reward, penalty, deadline: deadline ?? null,
  });
}

export function economyCompleteContract(
  contractId: string, success: boolean, evidence?: string,
): Promise<string> {
  return invokeDesktop<string>("economy_complete_contract", {
    contractId, contract_id: contractId,
    success, evidence: evidence ?? null,
  });
}

export function economyListContracts(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_list_contracts", agentArgs(agentId));
}

export function economyDisputeContract(contractId: string, reason: string): Promise<string> {
  return invokeDesktop<string>("economy_dispute_contract", {
    contractId, contract_id: contractId, reason,
  });
}

export function economyAgentPerformance(agentId: string): Promise<string> {
  return invokeDesktop<string>("economy_agent_performance", agentArgs(agentId));
}

// ============ EVOLUTION ============

export function evolutionGetStatus(): Promise<string> {
  return invokeDesktop<string>("evolution_get_status");
}

export function evolutionEvolveOnce(agentId: string): Promise<string> {
  return invokeDesktop<string>("evolution_evolve_once", agentArgs(agentId));
}

export function evolutionGetHistory(agentId: string): Promise<string> {
  return invokeDesktop<string>("evolution_get_history", agentArgs(agentId));
}

export function evolutionGetActiveStrategy(agentId: string): Promise<string> {
  return invokeDesktop<string>("evolution_get_active_strategy", agentArgs(agentId));
}

export function evolutionRegisterStrategy(
  agentId: string, name: string, parameters: string,
): Promise<string> {
  return invokeDesktop<string>("evolution_register_strategy", {
    agentId, agent_id: agentId, name, parameters,
  });
}

export function evolutionRollback(agentId: string): Promise<string> {
  return invokeDesktop<string>("evolution_rollback", agentArgs(agentId));
}

export function evolvePopulation(
  agentIds: string[], task: string, generations: number,
): Promise<string> {
  return invokeDesktop<string>("evolve_population", {
    agentIds, agent_ids: agentIds, task, generations,
  });
}

// ============ GENOME (extra) ============

export function breedAgents(parentA: string, parentB: string): Promise<string> {
  return invokeDesktop<string>("breed_agents", {
    parentA, parent_a: parentA, parentB, parent_b: parentB,
  });
}

export function getAgentGenome(agentId: string): Promise<string> {
  return invokeDesktop<string>("get_agent_genome", agentArgs(agentId));
}

export function getAgentLineage(agentId: string): Promise<string> {
  return invokeDesktop<string>("get_agent_lineage", agentArgs(agentId));
}

export function generateAllGenomes(): Promise<string> {
  return invokeDesktop<string>("generate_all_genomes");
}

// ============ GENESIS ============

export function genesisAnalyzeGap(userRequest: string): Promise<string> {
  return invokeDesktop<string>("genesis_analyze_gap", {
    userRequest, user_request: userRequest,
  });
}

export function genesisPreviewAgent(userRequest: string, llmResponse: string): Promise<string> {
  return invokeDesktop<string>("genesis_preview_agent", {
    userRequest, user_request: userRequest,
    llmResponse, llm_response: llmResponse,
  });
}

export function genesisCreateAgent(specJson: string, systemPrompt: string): Promise<string> {
  return invokeDesktop<string>("genesis_create_agent", {
    specJson, spec_json: specJson,
    systemPrompt, system_prompt: systemPrompt,
  });
}

export function genesisDeleteAgent(agentName: string): Promise<string> {
  return invokeDesktop<string>("genesis_delete_agent", {
    agentName, agent_name: agentName,
  });
}

export function genesisListGenerated(): Promise<string> {
  return invokeDesktop<string>("genesis_list_generated");
}

export function genesisStorePattern(
  specJson: string, missingCapabilities: string[], testScore: number,
): Promise<string> {
  return invokeDesktop<string>("genesis_store_pattern", {
    specJson, spec_json: specJson,
    missingCapabilities, missing_capabilities: missingCapabilities,
    testScore, test_score: testScore,
  });
}

// ============ CONSCIOUSNESS ============

export function getAgentConsciousness(agentId: string): Promise<string> {
  return invokeDesktop<string>("get_agent_consciousness", agentArgs(agentId));
}

export function getConsciousnessHeatmap(): Promise<string> {
  return invokeJsonDesktop<string>("get_consciousness_heatmap");
}

export function getConsciousnessHistory(agentId: string, limit = 50): Promise<string> {
  return invokeDesktop<string>("get_consciousness_history", {
    agentId, agent_id: agentId, limit,
  });
}

export function getUserBehaviorState(): Promise<string> {
  return invokeDesktop<string>("get_user_behavior_state");
}

export function reportUserKeystroke(isDeletion: boolean, timestamp: number): Promise<void> {
  return invokeDesktop<void>("report_user_keystroke", {
    isDeletion, is_deletion: isDeletion, timestamp,
  });
}

// ============ LLM PROVIDER ============

export function getActiveLlmProvider(): Promise<string> {
  return invokeDesktop<string>("get_active_llm_provider");
}

// ============ DREAMS ============

export function getDreamStatus(): Promise<string> {
  return invokeDesktop<string>("get_dream_status");
}

export function getDreamQueue(): Promise<string> {
  return invokeDesktop<string>("get_dream_queue");
}

export function getDreamHistory(limit = 50): Promise<string> {
  return invokeDesktop<string>("get_dream_history", { limit });
}

export function getMorningBriefing(): Promise<string> {
  return invokeDesktop<string>("get_morning_briefing");
}

// ============ TEMPORAL (extra) ============

export function getTemporalHistory(limit = 50): Promise<string> {
  return invokeDesktop<string>("get_temporal_history", { limit });
}

export function setTemporalConfig(
  maxForks: number, evalStrategy: string, budgetTokens: number,
): Promise<void> {
  return invokeDesktop<void>("set_temporal_config", {
    maxForks, max_forks: maxForks,
    evalStrategy, eval_strategy: evalStrategy,
    budgetTokens, budget_tokens: budgetTokens,
  });
}

// ============ IMMUNE (extra) ============

export function getImmuneStatus(): Promise<string> {
  return invokeJsonDesktop<string>("get_immune_status");
}

export function getImmuneMemory(): Promise<string> {
  return invokeJsonDesktop<string>("get_immune_memory");
}

export function setPrivacyRules(rules: unknown): Promise<void> {
  return invokeDesktop<void>("set_privacy_rules", { rules });
}

export function runAdversarialSession(
  attackerId: string, defenderId: string, rounds: number,
): Promise<string> {
  return invokeJsonDesktop<string>("run_adversarial_session", {
    attackerId, attacker_id: attackerId,
    defenderId, defender_id: defenderId,
    rounds,
  });
}

// ============ GHOST PROTOCOL ============

export function ghostProtocolToggle(enabled: boolean): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_toggle", { enabled });
}

export function ghostProtocolStatus(): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_status");
}

export function ghostProtocolGetState(): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_get_state");
}

export function ghostProtocolAddPeer(address: string, name: string): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_add_peer", { address, name });
}

export function ghostProtocolRemovePeer(deviceId: string): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_remove_peer", {
    deviceId, device_id: deviceId,
  });
}

export function ghostProtocolSyncNow(): Promise<string> {
  return invokeDesktop<string>("ghost_protocol_sync_now");
}

// ============ IDENTITY (extra) ============

export function identityGetAgentPassport(agentId: string): Promise<string> {
  return invokeJsonDesktop<string>("identity_get_agent_passport", {
    agentId, agent_id: agentId,
  });
}

export function identityExportPassport(agentId: string): Promise<string> {
  return invokeJsonDesktop<string>("identity_export_passport", {
    agentId, agent_id: agentId,
  });
}

export function identityGenerateProof(agentId: string, claim: string): Promise<string> {
  return invokeJsonDesktop<string>("identity_generate_proof", {
    agentId, agent_id: agentId, claim,
  });
}

export function identityVerifyProof(proof: unknown): Promise<boolean> {
  return invokeDesktop<boolean>("identity_verify_proof", { proof });
}

// ============ MCP HOST ============

export function mcpHostAddServer(
  name: string, url: string, transport: string, authToken?: string,
): Promise<string> {
  return invokeDesktop<string>("mcp_host_add_server", {
    name, url, transport,
    authToken: authToken ?? null, auth_token: authToken ?? null,
  });
}

export function mcpHostRemoveServer(serverId: string): Promise<string> {
  return invokeDesktop<string>("mcp_host_remove_server", {
    serverId, server_id: serverId,
  });
}

export function mcpHostListServers(): Promise<string> {
  return invokeDesktop<string>("mcp_host_list_servers");
}

export function mcpHostConnect(serverId: string): Promise<string> {
  return invokeDesktop<string>("mcp_host_connect", {
    serverId, server_id: serverId,
  });
}

export function mcpHostDisconnect(serverId: string): Promise<string> {
  return invokeDesktop<string>("mcp_host_disconnect", {
    serverId, server_id: serverId,
  });
}

export function mcpHostListTools(): Promise<string> {
  return invokeDesktop<string>("mcp_host_list_tools");
}

export function mcpHostCallTool(toolName: string, args: string): Promise<string> {
  return invokeDesktop<string>("mcp_host_call_tool", {
    toolName, tool_name: toolName,
    arguments: args,
  });
}

// ============ MESH (extra) ============

export function meshDiscoverPeers(): Promise<string> {
  return invokeJsonDesktop<string>("mesh_discover_peers");
}

export function meshGetPeers(): Promise<string> {
  return invokeJsonDesktop<string>("mesh_get_peers");
}

export function meshGetSyncStatus(): Promise<string> {
  return invokeJsonDesktop<string>("mesh_get_sync_status");
}

export function meshDistributeTask(task: string, agentIds: string[]): Promise<string> {
  return invokeJsonDesktop<string>("mesh_distribute_task", {
    task, agentIds, agent_ids: agentIds,
  });
}

export function meshMigrateAgent(agentId: string, targetPeer: string): Promise<string> {
  return invokeJsonDesktop<string>("mesh_migrate_agent", {
    agentId, agent_id: agentId,
    targetPeer, target_peer: targetPeer,
  });
}

// ============ NEURAL BRIDGE ============

export function neuralBridgeIngest(
  sourceType: string, content: string, metadata: unknown,
): Promise<string> {
  return invokeDesktop<string>("neural_bridge_ingest", {
    sourceType, source_type: sourceType, content, metadata,
  });
}

export function neuralBridgeSearch(
  query: string, timeRange?: [number, number],
  sourceFilter?: string[], maxResults?: number,
): Promise<string> {
  return invokeDesktop<string>("neural_bridge_search", {
    query,
    timeRange: timeRange ?? null, time_range: timeRange ?? null,
    sourceFilter: sourceFilter ?? null, source_filter: sourceFilter ?? null,
    maxResults: maxResults ?? null, max_results: maxResults ?? null,
  });
}

export function neuralBridgeStatus(): Promise<string> {
  return invokeDesktop<string>("neural_bridge_status");
}

export function neuralBridgeToggle(enabled: boolean): Promise<string> {
  return invokeDesktop<string>("neural_bridge_toggle", { enabled });
}

export function neuralBridgeDelete(id: string): Promise<string> {
  return invokeDesktop<string>("neural_bridge_delete", { id });
}

export function neuralBridgeClearOld(beforeTimestamp: number): Promise<string> {
  return invokeDesktop<string>("neural_bridge_clear_old", {
    beforeTimestamp, before_timestamp: beforeTimestamp,
  });
}

// ============ NEXUS LINK ============

export function nexusLinkStatus(): Promise<string> {
  return invokeDesktop<string>("nexus_link_status");
}

export function nexusLinkToggleSharing(enabled: boolean): Promise<string> {
  return invokeDesktop<string>("nexus_link_toggle_sharing", { enabled });
}

export function nexusLinkAddPeer(address: string, name: string): Promise<string> {
  return invokeDesktop<string>("nexus_link_add_peer", { address, name });
}

export function nexusLinkRemovePeer(deviceId: string): Promise<string> {
  return invokeDesktop<string>("nexus_link_remove_peer", {
    deviceId, device_id: deviceId,
  });
}

export function nexusLinkListPeers(): Promise<string> {
  return invokeDesktop<string>("nexus_link_list_peers");
}

export function nexusLinkSendModel(
  peerAddress: string, modelId: string, filename: string,
): Promise<string> {
  return invokeDesktop<string>("nexus_link_send_model", {
    peerAddress, peer_address: peerAddress,
    modelId, model_id: modelId, filename,
  });
}

// ============ NOTES (extra) ============

export function notesGet(id: string): Promise<string> {
  return invokeDesktop<string>("notes_get", { id });
}

// ============ OMNISCIENCE ============

export function omniscienceEnable(intervalMs = 5000): Promise<void> {
  return invokeDesktop<void>("omniscience_enable", {
    intervalMs, interval_ms: intervalMs,
  });
}

export function omniscienceDisable(): Promise<void> {
  return invokeDesktop<void>("omniscience_disable");
}

export function omniscienceGetScreenContext(): Promise<string> {
  return invokeJsonDesktop<string>("omniscience_get_screen_context");
}

export function omniscienceGetPredictions(): Promise<string> {
  return invokeJsonDesktop<string>("omniscience_get_predictions");
}

export function omniscienceGetAppContext(appName: string): Promise<string> {
  return invokeJsonDesktop<string>("omniscience_get_app_context", {
    appName, app_name: appName,
  });
}

export function omniscienceExecuteAction(action: unknown): Promise<string> {
  return invokeJsonDesktop<string>("omniscience_execute_action", { action });
}

// ============ PAYMENT ============

export function paymentCreatePlan(
  name: string, priceCents: number, interval: string, features: string[],
): Promise<string> {
  return invokeDesktop<string>("payment_create_plan", {
    name, priceCents, price_cents: priceCents,
    interval, features,
  });
}

export function paymentListPlans(): Promise<string> {
  return invokeDesktop<string>("payment_list_plans");
}

export function paymentCreateInvoice(planId: string, buyerId: string): Promise<string> {
  return invokeDesktop<string>("payment_create_invoice", {
    planId, plan_id: planId, buyerId, buyer_id: buyerId,
  });
}

export function paymentPayInvoice(invoiceId: string): Promise<string> {
  return invokeDesktop<string>("payment_pay_invoice", {
    invoiceId, invoice_id: invoiceId,
  });
}

export function paymentGetRevenueStats(): Promise<string> {
  return invokeDesktop<string>("payment_get_revenue_stats");
}

export function paymentCreatePayout(
  developerId: string, agentId: string, amountCents: number, period: string,
): Promise<string> {
  return invokeDesktop<string>("payment_create_payout", {
    developerId, developer_id: developerId,
    agentId, agent_id: agentId,
    amountCents, amount_cents: amountCents,
    period,
  });
}

// ============ REPLAY ============

export function replayToggleRecording(enabled: boolean): Promise<string> {
  return invokeDesktop<string>("replay_toggle_recording", { enabled });
}

export function replayListBundles(agentId?: string, limit?: number): Promise<string> {
  return invokeDesktop<string>("replay_list_bundles", {
    agentId: agentId ?? null, agent_id: agentId ?? null,
    limit: limit ?? null,
  });
}

export function replayGetBundle(bundleId: string): Promise<string> {
  return invokeDesktop<string>("replay_get_bundle", {
    bundleId, bundle_id: bundleId,
  });
}

export function replayVerifyBundle(bundleId: string): Promise<string> {
  return invokeDesktop<string>("replay_verify_bundle", {
    bundleId, bundle_id: bundleId,
  });
}

export function replayExportBundle(bundleId: string): Promise<string> {
  return invokeDesktop<string>("replay_export_bundle", {
    bundleId, bundle_id: bundleId,
  });
}

// ============ REPUTATION ============

export function reputationRegister(did: string, name: string): Promise<string> {
  return invokeDesktop<string>("reputation_register", { did, name });
}

export function reputationGet(did: string): Promise<string> {
  return invokeDesktop<string>("reputation_get", { did });
}

export function reputationTop(limit?: number): Promise<string> {
  return invokeDesktop<string>("reputation_top", { limit: limit ?? null });
}

export function reputationRateAgent(
  did: string, raterDid: string, score: number, comment?: string,
): Promise<string> {
  return invokeDesktop<string>("reputation_rate_agent", {
    did, raterDid, rater_did: raterDid,
    score, comment: comment ?? null,
  });
}

export function reputationRecordTask(did: string, success: boolean): Promise<string> {
  return invokeDesktop<string>("reputation_record_task", { did, success });
}

export function reputationExport(did: string): Promise<string> {
  return invokeDesktop<string>("reputation_export", { did });
}

export function reputationImport(json: string): Promise<string> {
  return invokeDesktop<string>("reputation_import", { json });
}

// ============ SELF-REWRITE (extra) ============

export function selfRewriteGetHistory(): Promise<string> {
  return invokeJsonDesktop<string>("self_rewrite_get_history");
}

export function selfRewritePreviewPatch(patchId: string): Promise<string> {
  return invokeJsonDesktop<string>("self_rewrite_preview_patch", {
    patchId, patch_id: patchId,
  });
}

export function selfRewriteTestPatch(patchId: string): Promise<string> {
  return invokeJsonDesktop<string>("self_rewrite_test_patch", {
    patchId, patch_id: patchId,
  });
}

// ============ TRACING ============

export function tracingStartTrace(operationName: string, agentId?: string): Promise<string> {
  return invokeDesktop<string>("tracing_start_trace", {
    operationName, operation_name: operationName,
    agentId: agentId ?? null, agent_id: agentId ?? null,
  });
}

export function tracingEndTrace(traceId: string): Promise<string> {
  return invokeDesktop<string>("tracing_end_trace", {
    traceId, trace_id: traceId,
  });
}

export function tracingStartSpan(
  traceId: string, parentSpanId: string, operationName: string, agentId?: string,
): Promise<string> {
  return invokeDesktop<string>("tracing_start_span", {
    traceId, trace_id: traceId,
    parentSpanId, parent_span_id: parentSpanId,
    operationName, operation_name: operationName,
    agentId: agentId ?? null, agent_id: agentId ?? null,
  });
}

export function tracingEndSpan(
  spanId: string, status: string, errorMessage?: string,
): Promise<string> {
  return invokeDesktop<string>("tracing_end_span", {
    spanId, span_id: spanId,
    status,
    errorMessage: errorMessage ?? null, error_message: errorMessage ?? null,
  });
}

export function tracingGetTrace(traceId: string): Promise<string> {
  return invokeDesktop<string>("tracing_get_trace", {
    traceId, trace_id: traceId,
  });
}

export function tracingListTraces(limit?: number): Promise<string> {
  return invokeDesktop<string>("tracing_list_traces", { limit: limit ?? null });
}

// ============ TOOLS ============

export function listTools(): Promise<string> {
  return invokeDesktop<string>("list_tools");
}

export function executeTool(toolJson: string): Promise<string> {
  return invokeDesktop<string>("execute_tool", {
    toolJson, tool_json: toolJson,
  });
}

// ============ GOVERNANCE ============

export function verifyGovernanceInvariants(): Promise<string> {
  return invokeDesktop<string>("verify_governance_invariants");
}

export function verifySpecificInvariant(invariantName: string): Promise<string> {
  return invokeDesktop<string>("verify_specific_invariant", {
    invariantName, invariant_name: invariantName,
  });
}

export function exportComplianceReport(): Promise<string> {
  return invokeDesktop<string>("export_compliance_report");
}

// ============ TRAY ============

export function trayStatus(): Promise<string> {
  return invokeJsonDesktop<string>("tray_status");
}

// ============ DILATED TIME / ADVERSARIAL ============

export function runDilatedSession(
  task: string, agentIds: string[], maxIterations: number,
): Promise<string> {
  return invokeDesktop<string>("run_dilated_session", {
    task, agentIds, agent_ids: agentIds,
    maxIterations, max_iterations: maxIterations,
  });
}

// ============ LEARNING ============

export function getLearningPaths(): Promise<string> {
  return invokeDesktop<string>("get_learning_session");
}

export function completeLearningStep(pathId: string, stepId: string): Promise<void> {
  return invokeDesktop<void>("learning_agent_action", {
    pathId, path_id: pathId, stepId, step_id: stepId,
  });
}

// ============ ADMIN CONSOLE ============

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminOverview(): Promise<any> {
  return invokeJsonDesktop("admin_overview");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminUsersList(): Promise<any[]> {
  return invokeJsonDesktop("admin_users_list");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminUserCreate(email: string, name: string, role: string): Promise<any> {
  return invokeJsonDesktop("admin_user_create", { email, name, role });
}

export function adminUserUpdateRole(userId: string, role: string): Promise<void> {
  return invokeDesktop<void>("admin_user_update_role", { user_id: userId, role });
}

export function adminUserDeactivate(userId: string): Promise<void> {
  return invokeDesktop<void>("admin_user_deactivate", { user_id: userId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminFleetStatus(): Promise<any> {
  return invokeJsonDesktop("admin_fleet_status");
}

export function adminAgentStopAll(workspaceId: string): Promise<number> {
  return invokeDesktop<number>("admin_agent_stop_all", { workspace_id: workspaceId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminAgentBulkUpdate(agentDids: string[], update: Record<string, unknown>): Promise<any> {
  return invokeJsonDesktop("admin_agent_bulk_update", { agent_dids: agentDids, update: JSON.stringify(update) });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminPolicyGet(scope: string): Promise<any> {
  return invokeJsonDesktop("admin_policy_get", { scope });
}

export function adminPolicyUpdate(scope: string, policy: Record<string, unknown>): Promise<void> {
  return invokeDesktop<void>("admin_policy_update", { scope, policy: JSON.stringify(policy) });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminPolicyHistory(scope: string): Promise<any[]> {
  return invokeJsonDesktop("admin_policy_history", { scope });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminComplianceStatus(): Promise<any> {
  return invokeJsonDesktop("admin_compliance_status");
}

export function adminComplianceExport(format: string): Promise<string> {
  return invokeDesktop<string>("admin_compliance_export", { format });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function adminSystemHealth(): Promise<any> {
  return invokeJsonDesktop("admin_system_health");
}

// ── Integration commands ──────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function integrationsList(): Promise<any> {
  return invokeJsonDesktop("integrations_list");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function integrationTest(providerId: string): Promise<any> {
  return invokeJsonDesktop("integration_test", { providerId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function integrationConfigure(providerId: string, settings: Record<string, unknown>): Promise<any> {
  return invokeJsonDesktop("integration_configure", { providerId, settings });
}

// ── Audit & Compliance Dashboard commands ───────────────────────

export interface AuditSearchQuery {
  text?: string;
  agent_id?: string;
  event_type?: string;
  severity?: string;
  time_range?: string;
  limit?: number;
  offset?: number;
}

export interface AuditSearchResult {
  entries: AuditEventRow[];
  total: number;
  offset: number;
  has_more: boolean;
}

export interface AuditStatistics {
  total_entries: number;
  entries_by_action: Record<string, number>;
  entries_by_agent: Record<string, number>;
  hitl_approvals: number;
  hitl_denials: number;
  hitl_timeouts: number;
  capability_denials: number;
  pii_redactions: number;
  firewall_blocks: number;
  total_fuel_consumed: number;
  severity_counts: Record<string, number>;
}

export interface ChainVerifyResult {
  verified: boolean;
  chain_length: number;
  verification_time_ms: number;
  first_break_at: number | null;
  last_verified_at: number;
}

export interface GovernanceMetrics {
  hitl_approval_rate: number;
  capability_denial_rate: number;
  pii_redaction_count: number;
  firewall_block_count: number;
  total_fuel_consumed: number;
  total_events: number;
  autonomy_distribution: Record<string, number>;
  events_per_hour: [number, number][];
}

export interface SecurityEvent {
  timestamp: number;
  event_type: string;
  severity: string;
  agent_id: string;
  description: string;
}

export function auditSearch(query: AuditSearchQuery): Promise<AuditSearchResult> {
  return invokeJsonDesktop<AuditSearchResult>("audit_search", { query });
}

export function auditStatistics(timeRange: string): Promise<AuditStatistics> {
  return invokeJsonDesktop<AuditStatistics>("audit_statistics", { timeRange, time_range: timeRange });
}

export function auditVerifyChain(): Promise<ChainVerifyResult> {
  return invokeJsonDesktop<ChainVerifyResult>("audit_verify_chain");
}

export function auditExportReport(format: string, timeRange: string): Promise<string> {
  return invokeDesktop<string>("audit_export_report", { format, time_range: timeRange });
}

export function complianceGovernanceMetrics(timeRange: string): Promise<GovernanceMetrics> {
  return invokeJsonDesktop<GovernanceMetrics>("compliance_governance_metrics", { timeRange, time_range: timeRange });
}

export function complianceSecurityEvents(timeRange: string): Promise<SecurityEvent[]> {
  return invokeJsonDesktop<SecurityEvent[]>("compliance_security_events", { timeRange, time_range: timeRange });
}

// ── Auth commands (nexus-auth) ──────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function authLogin(): Promise<any> {
  return invokeJsonDesktop("auth_login");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function authSessionInfo(sessionId: string): Promise<any> {
  return invokeJsonDesktop("auth_session_info", { session_id: sessionId });
}

export function authLogout(sessionId: string): Promise<void> {
  return invokeDesktop<void>("auth_logout", { session_id: sessionId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function authConfigGet(): Promise<any> {
  return invokeJsonDesktop("auth_config_get");
}

// ── Workspace commands (nexus-tenancy) ──────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function workspaceList(): Promise<any[]> {
  return invokeJsonDesktop("workspace_list");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function workspaceCreate(name: string): Promise<any> {
  return invokeJsonDesktop("workspace_create", { name });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function workspaceGet(workspaceId: string): Promise<any> {
  return invokeJsonDesktop("workspace_get", { workspace_id: workspaceId });
}

export function workspaceAddMember(workspaceId: string, userId: string, role: string): Promise<void> {
  return invokeDesktop<void>("workspace_add_member", { workspace_id: workspaceId, user_id: userId, role });
}

export function workspaceRemoveMember(workspaceId: string, userId: string): Promise<void> {
  return invokeDesktop<void>("workspace_remove_member", { workspace_id: workspaceId, user_id: userId });
}

export function workspaceSetPolicy(workspaceId: string, policyJson: string): Promise<void> {
  return invokeDesktop<void>("workspace_set_policy", { workspace_id: workspaceId, policy_json: policyJson });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function workspaceUsage(workspaceId: string): Promise<any> {
  return invokeJsonDesktop("workspace_usage", { workspace_id: workspaceId });
}

// ── Telemetry commands (nexus-telemetry) ────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function telemetryStatus(): Promise<any> {
  return invokeJsonDesktop("telemetry_status");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function telemetryHealth(): Promise<any> {
  return invokeJsonDesktop("telemetry_health");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function telemetryConfigGet(): Promise<any> {
  return invokeJsonDesktop("telemetry_config_get");
}

export function telemetryConfigUpdate(configJson: string): Promise<void> {
  return invokeDesktop<void>("telemetry_config_update", { config_json: configJson });
}

// ── Metering commands (nexus-metering) ──────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function meteringUsageReport(workspaceId: string, period: string): Promise<any> {
  return invokeJsonDesktop("metering_usage_report", { workspace_id: workspaceId, period });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function meteringCostBreakdown(workspaceId: string, period: string): Promise<any> {
  return invokeJsonDesktop("metering_cost_breakdown", { workspace_id: workspaceId, period });
}

export function meteringExportCsv(workspaceId: string, period: string): Promise<string> {
  return invokeDesktop<string>("metering_export_csv", { workspace_id: workspaceId, period });
}

export function meteringSetBudgetAlert(workspaceId: string, threshold: number): Promise<void> {
  return invokeDesktop<void>("metering_set_budget_alert", { workspace_id: workspaceId, threshold });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function meteringBudgetAlerts(workspaceId: string): Promise<any[]> {
  return invokeJsonDesktop("metering_budget_alerts", { workspace_id: workspaceId });
}

// ── Backup commands ─────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function backupList(): Promise<any[]> {
  return invokeJsonDesktop("backup_list");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function backupCreate(): Promise<any> {
  return invokeJsonDesktop("backup_create");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function backupVerify(id: string): Promise<any> {
  return invokeJsonDesktop("backup_verify", { id });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function backupRestore(id: string): Promise<any> {
  return invokeJsonDesktop("backup_restore", { id });
}

// ============ DATABASE MANAGER API ============

export function dbConnect(connectionString: string): Promise<string> {
  return invokeDesktop<string>("db_connect", { connectionString, connection_string: connectionString });
}

export function dbListTables(connectionString: string): Promise<string> {
  return invokeDesktop<string>("db_list_tables", { connectionString, connection_string: connectionString });
}

export function dbExecuteQuery(connectionString: string, query: string): Promise<string> {
  return invokeDesktop<string>("db_execute_query", { connectionString, connection_string: connectionString, query });
}

// ============ FILE MANAGER (extra) ============

export function fileManagerHome(): Promise<string> {
  return invokeDesktop<string>("file_manager_home");
}

export function fileManagerDelete(path: string): Promise<string> {
  return invokeDesktop<string>("file_manager_delete", { path });
}

export function fileManagerRename(from: string, to: string): Promise<string> {
  return invokeDesktop<string>("file_manager_rename", { from, to });
}

// ============ TIMELINE VIEWER API ============

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function temporalSelectFork(forkId: string): Promise<any> {
  return invokeDesktop("temporal_select_fork", { forkId, fork_id: forkId });
}

// ============ API CLIENT API ============

export function apiClientRequest(
  method: string, url: string, headersJson: string, body: string,
): Promise<string> {
  return invokeDesktop<string>("api_client_request", { method, url, headersJson, headers_json: headersJson, body });
}

export function apiClientListCollections(): Promise<string> {
  return invokeDesktop<string>("api_client_list_collections");
}

export function apiClientSaveCollections(dataJson: string): Promise<void> {
  return invokeDesktop<void>("api_client_save_collections", { dataJson, data_json: dataJson });
}

// ============ LEARNING PROGRESS API ============

export function learningSaveProgress(dataJson: string): Promise<void> {
  return invokeDesktop<void>("learning_save_progress", { dataJson, data_json: dataJson });
}

export function learningGetProgress(): Promise<string> {
  return invokeDesktop<string>("learning_get_progress");
}

export function learningExecuteChallenge(
  challengeId: string, code: string, language: string,
): Promise<string> {
  return invokeDesktop<string>("learning_execute_challenge", {
    challengeId, challenge_id: challengeId,
    code,
    language,
  });
}

// ============ DATABASE EXPORT API ============

export function dbDisconnect(dbPath: string): Promise<void> {
  return invokeDesktop<void>("db_disconnect", { dbPath, db_path: dbPath });
}

export function dbExportTable(
  connectionString: string, tableName: string, format: string,
): Promise<string> {
  return invokeDesktop<string>("db_export_table", {
    connectionString, connection_string: connectionString,
    tableName, table_name: tableName,
    format,
  });
}

// ============ DREAM ENGINE (extra) ============

export function triggerDreamNow(): Promise<void> {
  return invokeDesktop<void>("trigger_dream_now");
}

export function setDreamConfig(
  enabled: boolean, idleTriggerMinutes: number, tokenBudget: number, apiCallBudget: number,
): Promise<void> {
  return invokeDesktop<void>("set_dream_config", {
    enabled,
    idleTriggerMinutes, idle_trigger_minutes: idleTriggerMinutes,
    tokenBudget, token_budget: tokenBudget,
    apiCallBudget, api_call_budget: apiCallBudget,
  });
}

// ============ IMMUNE ENGINE (extra) ============

export function triggerImmuneScan(): Promise<void> {
  return invokeDesktop<void>("trigger_immune_scan");
}

export function getThreatLog(limit?: number): Promise<string> {
  return invokeJsonDesktop<string>("get_threat_log", limit != null ? { limit } : undefined);
}

// ── Background Scheduler ────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function schedulerCreate(entry: any): Promise<string> {
  return invokeDesktop<string>("scheduler_create", { entry });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function schedulerList(): Promise<any> {
  return invokeDesktop("scheduler_list");
}

export function schedulerEnable(id: string): Promise<void> {
  return invokeDesktop<void>("scheduler_enable", { id });
}

export function schedulerDisable(id: string): Promise<void> {
  return invokeDesktop<void>("scheduler_disable", { id });
}

export function schedulerDelete(id: string): Promise<void> {
  return invokeDesktop<void>("scheduler_delete", { id });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function schedulerHistory(id: string): Promise<any> {
  return invokeDesktop("scheduler_history", { id });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function schedulerTriggerNow(id: string): Promise<any> {
  return invokeDesktop("scheduler_trigger_now", { id });
}

/** Get live status of all schedules in the background runner. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function schedulerRunnerStatus(): Promise<any[]> {
  return invokeDesktop("scheduler_runner_status");
}

// ── Team Orchestration API ──────────────────────────────────────────

/** Execute a team workflow: Director assigns tasks to workers, collects results. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function executeTeamWorkflow(
  directorId: string,
  goal: string,
  memberIds: string[],
): Promise<any> {
  return invokeDesktop("execute_team_workflow", {
    directorId,
    director_id: directorId,
    goal,
    memberIds,
    member_ids: memberIds,
  });
}

/** Transfer fuel from one agent to another (Director privilege). */
export function transferAgentFuel(
  fromAgentId: string,
  toAgentId: string,
  amount: number,
): Promise<void> {
  return invokeDesktop<void>("transfer_agent_fuel", {
    fromAgentId,
    from_agent_id: fromAgentId,
    toAgentId,
    to_agent_id: toAgentId,
    amount,
  });
}

// ── Content Pipeline API ────────────────────────────────────────────

/** Run the full content pipeline: scan → research → write → publish → analytics. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function runContentPipeline(agentId: string): Promise<any> {
  return invokeDesktop("run_content_pipeline", {
    agentId,
    agent_id: agentId,
  });
}

// ── Flash Inference API ────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashDetectHardware(): Promise<any> {
  return invokeDesktop("flash_detect_hardware");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashProfileModel(modelPath: string): Promise<any> {
  return invokeDesktop("flash_profile_model", { modelPath, model_path: modelPath });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashAutoConfigure(
  modelPath: string,
  targetContextLen: number,
  priority: "speed" | "context" | "balanced",
// eslint-disable-next-line @typescript-eslint/no-explicit-any
): Promise<any> {
  return invokeDesktop("flash_auto_configure", {
    modelPath, model_path: modelPath,
    targetContextLen, target_context_len: targetContextLen,
    priority,
  });
}

export function flashCreateSession(
  modelPath: string,
  targetContextLen: number,
  priority: "speed" | "context" | "balanced",
): Promise<string> {
  return invokeDesktop<string>("flash_create_session", {
    modelPath, model_path: modelPath,
    targetContextLen, target_context_len: targetContextLen,
    priority,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashListSessions(): Promise<any[]> {
  return invokeDesktop("flash_list_sessions");
}

export function flashUnloadSession(sessionId: string): Promise<void> {
  return invokeDesktop<void>("flash_unload_session", {
    sessionId, session_id: sessionId,
  });
}

export function flashClearSessions(): Promise<void> {
  return invokeDesktop<void>("flash_clear_sessions");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashGetMetrics(sessionId: string): Promise<any> {
  return invokeDesktop("flash_get_metrics", {
    sessionId, session_id: sessionId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashSystemMetrics(): Promise<any> {
  return invokeDesktop("flash_system_metrics");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashEstimatePerformance(modelPath: string): Promise<any> {
  return invokeDesktop("flash_estimate_performance", {
    modelPath, model_path: modelPath,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashCatalogRecommend(): Promise<any[]> {
  return invokeDesktop("flash_catalog_recommend");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashCatalogSearch(query: string): Promise<any[]> {
  return invokeDesktop("flash_catalog_search", { query });
}

// ── Flash Inference — Speculative Decoding ─────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashEnableSpeculative(
  draftModelPath: string,
  draftTokens?: number,
): Promise<any> {
  return invokeDesktop("flash_enable_speculative", {
    draftModelPath,
    draft_model_path: draftModelPath,
    draftTokens,
    draft_tokens: draftTokens,
  });
}

export function flashDisableSpeculative(): Promise<void> {
  return invokeDesktop("flash_disable_speculative");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashSpeculativeStatus(): Promise<any> {
  return invokeDesktop("flash_speculative_status");
}

// ── Flash Inference — Download & Model Management ─────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashListLocalModels(): Promise<any[]> {
  return invokeDesktop("flash_list_local_models");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashDownloadModel(hfRepo: string, filename: string): Promise<any> {
  return invokeDesktop("flash_download_model", {
    hfRepo, hf_repo: hfRepo,
    filename,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashDownloadMulti(hfRepo: string, filenames: string[]): Promise<any> {
  return invokeDesktop("flash_download_multi", {
    hfRepo, hf_repo: hfRepo,
    filenames,
  });
}

export function flashDeleteLocalModel(filename: string): Promise<void> {
  return invokeDesktop<void>("flash_delete_local_model", { filename });
}

export function flashAvailableDiskSpace(): Promise<number> {
  return invokeDesktop<number>("flash_available_disk_space");
}

export function flashGetModelDir(): Promise<string> {
  return invokeDesktop<string>("flash_get_model_dir");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function flashGenerate(
  sessionId: string,
  prompt: string,
  maxTokens?: number,
// eslint-disable-next-line @typescript-eslint/no-explicit-any
): Promise<any> {
  return invokeDesktop("flash_generate", {
    sessionId, session_id: sessionId,
    prompt,
    maxTokens: maxTokens ?? 2048,
    max_tokens: maxTokens ?? 2048,
  });
}

// ── Capability Measurement ──────────────────────────────────────────────────

export function cmStartSession(
  agentId: string,
  agentAutonomyLevel: number,
): Promise<string> {
  return invokeDesktop<string>("cm_start_session", {
    agentId, agent_id: agentId,
    agentAutonomyLevel, agent_autonomy_level: agentAutonomyLevel,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetSession(sessionId: string): Promise<any> {
  return invokeDesktop("cm_get_session", {
    sessionId, session_id: sessionId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetScorecard(agentId: string): Promise<any> {
  return invokeDesktop("cm_get_scorecard", {
    agentId, agent_id: agentId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmListSessions(): Promise<any[]> {
  return invokeDesktop("cm_list_sessions");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetProfile(agentId: string): Promise<any> {
  return invokeDesktop("cm_get_profile", {
    agentId, agent_id: agentId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetGamingFlags(sessionId: string): Promise<any[]> {
  return invokeDesktop("cm_get_gaming_flags", {
    sessionId, session_id: sessionId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmCompareAgents(agentIds: string[]): Promise<any[]> {
  return invokeDesktop("cm_compare_agents", {
    agentIds, agent_ids: agentIds,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetBatteries(): Promise<any[]> {
  return invokeDesktop("cm_get_batteries");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmTriggerFeedback(agentId: string): Promise<any> {
  return invokeDesktop("cm_trigger_feedback", {
    agentId, agent_id: agentId,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmEvaluateResponse(problemId: string, agentResponse: string): Promise<any> {
  return invokeDesktop("cm_evaluate_response", {
    problemId, problem_id: problemId,
    agentResponse, agent_response: agentResponse,
  });
}

// ── Capability Boundary ─────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetBoundaryMap(): Promise<any[]> {
  return invokeDesktop("cm_get_boundary_map");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetCalibration(): Promise<any> {
  return invokeDesktop("cm_get_calibration");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetCensus(): Promise<any> {
  return invokeDesktop("cm_get_census");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetGamingReportBatch(): Promise<any> {
  return invokeDesktop("cm_get_gaming_report_batch");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmUploadDarwin(): Promise<any> {
  return invokeDesktop("cm_upload_darwin");
}

// ── Validation Runs ─────────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmExecuteValidationRun(runLabel: string, enableRouting: boolean): Promise<any> {
  return invokeDesktop("cm_execute_validation_run", {
    runLabel, run_label: runLabel,
    enableRouting, enable_routing: enableRouting,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmListValidationRuns(): Promise<any[]> {
  return invokeDesktop("cm_list_validation_runs");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmGetValidationRun(runLabel: string): Promise<any> {
  return invokeDesktop("cm_get_validation_run", { runLabel, run_label: runLabel });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmThreeWayComparison(run1Label: string, run2Label: string): Promise<any> {
  return invokeDesktop("cm_three_way_comparison", {
    run1Label, run1_label: run1Label,
    run2Label, run2_label: run2Label,
  });
}

// ── A/B Validation ──────────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cmRunAbValidation(agentIds: string[]): Promise<any> {
  return invokeDesktop("cm_run_ab_validation", { agentIds, agent_ids: agentIds });
}

// ── Predictive Router ───────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function routerRouteTask(agentId: string, taskText: string): Promise<any> {
  return invokeDesktop("router_route_task", { agentId, agent_id: agentId, taskText, task_text: taskText });
}

export function routerRecordOutcome(decisionId: string, success: boolean, modelWasSufficient: boolean, shouldHaveStaged: boolean): Promise<void> {
  return invokeDesktop("router_record_outcome", {
    decisionId, decision_id: decisionId,
    success, modelWasSufficient, model_was_sufficient: modelWasSufficient,
    shouldHaveStaged, should_have_staged: shouldHaveStaged,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function routerGetAccuracy(): Promise<any> { return invokeDesktop("router_get_accuracy"); }

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function routerGetModels(): Promise<any[]> { return invokeDesktop("router_get_models"); }

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function routerEstimateDifficulty(taskText: string): Promise<any> {
  return invokeDesktop("router_estimate_difficulty", { taskText, task_text: taskText });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function routerGetFeedback(): Promise<any> { return invokeDesktop("router_get_feedback"); }

// ── Browser Agent ───────────────────────────────────────────────────────────

export function browserCreateSession(agentId: string, autonomyLevel: number): Promise<string> {
  return invokeDesktop<string>("browser_create_session", { agentId, agent_id: agentId, autonomyLevel, autonomy_level: autonomyLevel });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function browserExecuteTask(sessionId: string, task: string, maxSteps?: number, modelId?: string): Promise<any> {
  return invokeDesktop("browser_execute_task", { sessionId, session_id: sessionId, task, maxSteps, max_steps: maxSteps, modelId, model_id: modelId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function browserNavigate(sessionId: string, url: string): Promise<any> {
  return invokeDesktop("browser_navigate", { sessionId, session_id: sessionId, url });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function browserGetContent(sessionId: string): Promise<any> {
  return invokeDesktop("browser_get_content", { sessionId, session_id: sessionId });
}

export function browserCloseSession(sessionId: string): Promise<void> {
  return invokeDesktop("browser_close_session", { sessionId, session_id: sessionId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function browserGetPolicy(): Promise<any> { return invokeDesktop("browser_get_policy"); }

export function browserSessionCount(): Promise<number> { return invokeDesktop<number>("browser_session_count"); }

// ── Governance Oracle ───────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function oracleStatus(): Promise<any> {
  return invokeDesktop("oracle_status");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function oracleVerifyToken(tokenJson: string): Promise<any> {
  return invokeDesktop("oracle_verify_token", {
    tokenJson, token_json: tokenJson,
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function oracleGetAgentBudget(agentId: string): Promise<any> {
  return invokeDesktop("oracle_get_agent_budget", {
    agentId, agent_id: agentId,
  });
}

// ── Token Economy ─────────────────────────────────────────────────────────────

export function tokenGetWallet(agentId: string): Promise<any> {
  return invokeDesktop("token_get_wallet", { agentId, agent_id: agentId });
}

export function tokenGetAllWallets(): Promise<any> {
  return invokeDesktop("token_get_all_wallets");
}

export function tokenCreateWallet(agentId: string, initialBalance: number, autonomyLevel: number): Promise<any> {
  return invokeDesktop("token_create_wallet", {
    agentId, agent_id: agentId,
    initialBalance, initial_balance: initialBalance,
    autonomyLevel, autonomy_level: autonomyLevel,
  });
}

export function tokenGetLedger(agentId?: string, limit?: number): Promise<any> {
  return invokeDesktop("token_get_ledger", {
    agentId, agent_id: agentId,
    limit,
  });
}

export function tokenGetSupply(): Promise<any> {
  return invokeDesktop("token_get_supply");
}

export function tokenCalculateBurn(modelId: string, inputTokens: number, outputTokens: number): Promise<any> {
  return invokeDesktop("token_calculate_burn", {
    modelId, model_id: modelId,
    inputTokens, input_tokens: inputTokens,
    outputTokens, output_tokens: outputTokens,
  });
}

export function tokenCalculateReward(quality: number, difficulty: number, completionSecs: number): Promise<any> {
  return invokeDesktop("token_calculate_reward", {
    quality, difficulty,
    completionSecs, completion_secs: completionSecs,
  });
}

export function tokenCalculateSpawn(parentId: string, fraction?: number): Promise<any> {
  return invokeDesktop("token_calculate_spawn", {
    parentId, parent_id: parentId,
    fraction,
  });
}

export function tokenCreateDelegation(
  requesterId: string, providerId: string, task: string,
  payment: number, threshold: number, timeout: number
): Promise<any> {
  return invokeDesktop("token_create_delegation", {
    requesterId, requester_id: requesterId,
    providerId, provider_id: providerId,
    task, payment, threshold, timeout,
  });
}

export function tokenGetDelegations(agentId: string): Promise<any> {
  return invokeDesktop("token_get_delegations", { agentId, agent_id: agentId });
}

export function tokenGetPricing(): Promise<any> {
  return invokeDesktop("token_get_pricing");
}

// ── Governed Computer Control ─────────────────────────────────────────────────

export function ccExecuteAction(
  agentId: string, autonomyLevel: number, capabilities: string[], actionJson: string
): Promise<any> {
  return invokeDesktop("cc_execute_action", {
    agentId, agent_id: agentId,
    autonomyLevel, autonomy_level: autonomyLevel,
    capabilities,
    actionJson, action_json: actionJson,
  });
}

export function ccGetActionHistory(agentId: string): Promise<any> {
  return invokeDesktop("cc_get_action_history", { agentId, agent_id: agentId });
}

export function ccGetCapabilityBudget(agentId: string): Promise<any> {
  return invokeDesktop("cc_get_capability_budget", { agentId, agent_id: agentId });
}

export function ccVerifyActionSequence(agentId: string): Promise<any> {
  return invokeDesktop("cc_verify_action_sequence", { agentId, agent_id: agentId });
}

export function ccGetScreenContext(agentId: string): Promise<any> {
  return invokeDesktop("cc_get_screen_context", { agentId, agent_id: agentId });
}

// ── World Simulation ──────────────────────────────────────────────────────────

export function simSubmit(agentId: string, description: string, actionsJson: string): Promise<string> {
  return invokeDesktop<string>("sim_submit", {
    agentId, agent_id: agentId, description,
    actionsJson, actions_json: actionsJson,
  });
}

export function simRun(scenarioId: string): Promise<any> {
  return invokeDesktop("sim_run", { scenarioId, scenario_id: scenarioId });
}

export function simGetResult(scenarioId: string): Promise<any> {
  return invokeDesktop("sim_get_result", { scenarioId, scenario_id: scenarioId });
}

export function simGetHistory(agentId: string): Promise<any> {
  return invokeDesktop("sim_get_history", { agentId, agent_id: agentId });
}

export function simGetPolicy(): Promise<any> {
  return invokeDesktop("sim_get_policy");
}

export function simGetRisk(scenarioId: string): Promise<any> {
  return invokeDesktop("sim_get_risk", { scenarioId, scenario_id: scenarioId });
}

export function simBranch(
  parentId: string, divergeAtStep: number, alternativeJson: string, remainingJson: string
): Promise<string> {
  return invokeDesktop<string>("sim_branch", {
    parentId, parent_id: parentId,
    divergeAtStep, diverge_at_step: divergeAtStep,
    alternativeJson, alternative_json: alternativeJson,
    remainingJson, remaining_json: remainingJson,
  });
}

