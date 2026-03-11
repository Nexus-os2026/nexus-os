import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityMessage,
  AgentCardSummary,
  AgentSummary,
  AuditEventRow,
  AvailableModel,
  BrowserHistoryEntry,
  BrowserNavigateResult,
  CapabilityRequest,
  ChatResponse,
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
  McpTool,
  NexusConfig,
  OllamaStatus,
  PermissionCategory,
  PermissionHistoryEntry,
  PermissionUpdate,
  PolicyConflict,
  PolicyEntry,
  PolicyTestResult,
  ProtocolRequest,
  ProtocolsStatus,
  BuildSessionState,
  ResearchSessionState,
  SetupResult,
  SystemInfo,
  VoiceRuntimeState
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

function agentArgs(agentId: string): Record<string, unknown> {
  return { agentId, agent_id: agentId };
}

export function listAgents(): Promise<AgentSummary[]> {
  return invokeDesktop<AgentSummary[]>("list_agents");
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

export function sendChat(message: string): Promise<ChatResponse> {
  return invokeDesktop<ChatResponse>("send_chat", { message });
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

export function getSystemInfo(): Promise<SystemInfo> {
  return invokeDesktop<SystemInfo>("get_system_info");
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

// ── Marketplace API ──

export function marketplaceSearch(query: string): Promise<MarketplaceAgent[]> {
  return invokeDesktop<MarketplaceAgent[]>("marketplace_search", { query });
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
