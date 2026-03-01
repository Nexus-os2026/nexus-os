export type AgentStatus = "Created" | "Starting" | "Running" | "Paused" | "Stopping" | "Stopped" | "Destroyed";

export interface AgentSummary {
  id: string;
  name: string;
  status: AgentStatus;
  fuel_remaining: number;
  last_action: string;
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
