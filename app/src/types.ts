export type AgentStatus = "Created" | "Starting" | "Running" | "Paused" | "Stopping" | "Stopped" | "Destroyed";

export type SandboxRuntime = "wasmtime" | "in-process" | "none";

export interface AgentSummary {
  id: string;
  name: string;
  status: AgentStatus;
  fuel_remaining: number;
  fuel_budget?: number;
  last_action: string;
  isSystem?: boolean;
  sandbox_runtime?: SandboxRuntime;
  memory_usage_bytes?: number;
  capabilities?: string[];
}

export interface AuditEventRow {
  event_id: string;
  timestamp: number;
  agent_id: string;
  event_type: string;
  payload: Record<string, unknown>;
  hash: string;
  previous_hash: string;
}

export interface VoiceRuntimeState {
  wake_word_enabled: boolean;
  push_to_talk_enabled: boolean;
  overlay_visible: boolean;
}

export interface LlmConfig {
  default_model: string;
  anthropic_api_key: string;
  openai_api_key: string;
  ollama_url: string;
}

export interface SearchConfig {
  brave_api_key: string;
}

export interface SocialConfig {
  x_api_key: string;
  x_api_secret: string;
  x_access_token: string;
  x_access_secret: string;
  facebook_page_token: string;
  instagram_access_token: string;
}

export interface MessagingConfig {
  telegram_bot_token: string;
  whatsapp_business_id: string;
  whatsapp_api_token: string;
  discord_bot_token: string;
  slack_bot_token: string;
}

export interface VoiceConfig {
  whisper_model: string;
  wake_word: string;
  tts_voice: string;
}

export interface PrivacyConfig {
  telemetry: boolean;
  audit_retention_days: number;
}

export interface NexusConfig {
  llm: LlmConfig;
  search: SearchConfig;
  social: SocialConfig;
  messaging: MessagingConfig;
  voice: VoiceConfig;
  privacy: PrivacyConfig;
  hardware?: HardwareConfig;
  ollama?: OllamaConfigSection;
  models?: ModelsConfig;
  agents?: Record<string, AgentLlmConfig>;
}

export interface ChatResponse {
  text: string;
  model: string;
  token_count: number;
  cost: number;
  latency_ms: number;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  model?: string;
  streaming?: boolean;
}

export type ConnectionStatus = "connected" | "mock";

export interface HardwareInfo {
  gpu: string;
  vram_mb: number;
  ram_mb: number;
  detected_at: string;
  tier: string;
  recommended_primary: string;
  recommended_fast: string;
}

export interface OllamaModelInfo {
  name: string;
  size: number;
}

export interface OllamaStatus {
  connected: boolean;
  base_url: string;
  models: OllamaModelInfo[];
}

export interface SetupResult {
  hardware: HardwareInfo;
  ollama: OllamaStatus;
  config_saved: boolean;
}

export interface HardwareConfig {
  gpu: string;
  vram_mb: number;
  ram_mb: number;
  detected_at: string;
}

export interface OllamaConfigSection {
  base_url: string;
  status: string;
}

export interface ModelsConfig {
  primary: string;
  fast: string;
}

export interface AgentLlmConfig {
  model: string;
  temperature: number;
  max_tokens: number;
}

export interface ModelPullProgress {
  model: string;
  status: string;
  percent: number;
  completed_bytes: number;
  total_bytes: number;
  error?: string;
}

export interface AvailableModel {
  id: string;
  name: string;
  size_gb: number;
  context: string;
  capabilities: string[];
  recommended: boolean;
  tag: string;
  installed: boolean;
  description: string;
}

export interface ChatTokenEvent {
  token: string;
  full: string;
  done: boolean;
  error?: string;
}

export interface SystemInfo {
  cpu_usage_percent: number;
  ram_used_gb: number;
  ram_total_gb: number;
  cpu_name: string;
}

export type GovernanceRouting = "local" | "cloud" | "fallback";

export interface SlmStatus {
  loaded: boolean;
  model_id: string | null;
  ram_usage_mb: number;
  avg_latency_ms: number;
  total_queries: number;
  governance_routing: GovernanceRouting;
}

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface ResourceImpact {
  disk_bytes_delta: number;
  fuel_cost: number;
  llm_calls: number;
  network_calls: number;
  file_operations: number;
}

export interface ActionPreviewItem {
  type: "file_change" | "network_call" | "data_modification" | "llm_call";
  path?: string;
  change_kind?: string;
  size_before?: number;
  size_after?: number;
  preview?: string;
  target?: string;
  method?: string;
  estimated_bytes?: number;
  resource?: string;
  description?: string;
  prompt_len?: number;
  max_tokens?: number;
  estimated_fuel?: number;
}

export interface SimulationPreview {
  simulation_id: string;
  agent_id: string;
  operation: string;
  predicted_changes: ActionPreviewItem[];
  resource_impact: ResourceImpact;
  risk_level: RiskLevel;
  summary: string;
}

// ── Permission Dashboard Types ──

export type PermissionRiskLevel = "low" | "medium" | "high" | "critical";

export interface Permission {
  capability_key: string;
  display_name: string;
  description: string;
  risk_level: PermissionRiskLevel;
  enabled: boolean;
  granted_by: string;
  granted_at: number;
  can_user_toggle: boolean;
}

export interface PermissionCategory {
  id: string;
  display_name: string;
  icon: string;
  permissions: Permission[];
}

export type PermissionActionType = "granted" | "revoked" | "escalated" | "downgraded" | "locked_by_admin" | "unlocked_by_admin";

export interface PermissionHistoryEntry {
  capability_key: string;
  action: PermissionActionType;
  changed_by: string;
  timestamp: number;
  reason: string | null;
}

export interface CapabilityRequest {
  agent_id: string;
  requested_capability: string;
  reason: string;
  risk_level: PermissionRiskLevel;
  current_capabilities: string[];
  requested_capabilities: string[];
}

export interface PermissionUpdate {
  capability_key: string;
  enabled: boolean;
}

// ── Protocols Dashboard Types ──

export interface ProtocolsStatus {
  a2a_status: string;
  a2a_version: string;
  a2a_peers: number;
  a2a_tasks_processed: number;
  mcp_status: string;
  mcp_registered_tools: number;
  mcp_invocations: number;
  gateway_port: number | null;
  governance_bridge_active: boolean;
  audit_integrity: boolean;
}

export interface ProtocolRequest {
  id: string;
  timestamp: number;
  protocol: string;
  method: string;
  sender: string;
  agent: string;
  status: string;
  fuel_consumed: number;
  governance_decision: string;
}

export interface McpTool {
  name: string;
  description: string;
  agent: string;
  fuel_cost: number;
  requires_hitl: boolean;
  invocations: number;
}

export interface AgentCardSummary {
  agent_name: string;
  url: string;
  skills_count: number;
  auth_scheme: string;
  rate_limit_rpm: number;
  card_json: Record<string, unknown>;
}
