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
