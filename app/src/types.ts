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
  did?: string;
}

/** Event emitted by backend on agent lifecycle changes. */
export interface AgentStatusEvent {
  agent_id: string;
  status: AgentStatus;
  fuel_remaining: number;
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
  deepseek_api_key: string;
  gemini_api_key: string;
  ollama_url: string;
  routing_strategy?: string;
  providers?: LlmProviderEntry[];
}

export interface LlmProviderEntry {
  id: string;
  provider_type: string;
  display_name: string;
  api_key: string;
  base_url: string;
  enabled: boolean;
  priority: number;
}

export interface AgentLlmAssignment {
  provider_id: string;
  local_only: boolean;
  budget_dollars: number;
  budget_tokens: number;
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
  agent_llm_assignments?: Record<string, AgentLlmAssignment>;
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

/** Status of all configured LLM providers. */
export interface LlmProviderStatusEntry {
  name: string;
  available: boolean;
  is_paid: boolean;
  reason: string;
  latency_ms?: number;
  error_hint?: string;
  setup_command?: string;
  models_installed?: number;
}

export interface LlmStatus {
  active_provider: string;
  providers: LlmProviderStatusEntry[];
  governance_warning?: string;
  has_any_provider: boolean;
}

export type RoutingStrategy = "Priority" | "RoundRobin" | "LowestLatency" | "CostOptimized";

export interface LlmRecommendation {
  provider_type: string;
  display_name: string;
  reason: string;
  setup_command?: string;
  cost_info: string;
  recommended: boolean;
}

export interface LlmRecommendations {
  ram_mb: number;
  gpu: string;
  can_run_local: boolean;
  recommendations: LlmRecommendation[];
}

export interface ProviderUsageStats {
  provider_name: string;
  total_queries: number;
  total_tokens: number;
  estimated_cost_dollars: number;
}

export interface TestConnectionResult {
  provider: string;
  success: boolean;
  latency_ms: number;
  error?: string;
  model_used?: string;
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

export type FsPermissionLevel = "ReadOnly" | "ReadWrite" | "Deny";

export interface FilesystemPermissionScope {
  path_pattern: string;
  permission: FsPermissionLevel;
}

export interface Permission {
  capability_key: string;
  display_name: string;
  description: string;
  risk_level: PermissionRiskLevel;
  enabled: boolean;
  granted_by: string;
  granted_at: number;
  can_user_toggle: boolean;
  filesystem_scopes?: FilesystemPermissionScope[];
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

// ── Policy Engine Types ──

export type PolicyEffect = "Allow" | "Deny";

export interface PolicyConditions {
  min_autonomy_level?: number;
  max_fuel_cost?: number;
  required_approvers?: number;
  time_window?: string;
}

export interface PolicyEntry {
  policy_id: string;
  description: string;
  effect: PolicyEffect;
  principal: string;
  action: string;
  resource: string;
  priority: number;
  conditions: PolicyConditions;
}

export interface PolicyConflict {
  policy_a: string;
  policy_b: string;
  overlap: string;
}

export interface PolicyTestResult {
  decision: string;
  matched_policies: string[];
}

// ── Consent Approval Display (from kernel consent_display::ApprovalDisplay) ──

export interface ApprovalDisplay {
  summary: string;
  details: [string, string][];
  risk_badge: string;
  raw_command: string;
  warnings: string[];
  agent_description?: string;
  agent_provided?: boolean;
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

// ── Identity types ──

export interface IdentityInfo {
  agent_id: string;
  did: string;
  created_at: number;
  public_key_hex: string;
}

// ── Firewall types ──

export interface FirewallStatus {
  status: string;
  mode: string;
  injection_pattern_count: number;
  pii_pattern_count: number;
  exfil_pattern_count: number;
  sensitive_path_count: number;
  ssn_detection: boolean;
  passport_detection: boolean;
  internal_ip_detection: boolean;
  context_overflow_threshold_bytes: number;
  egress_default_deny: boolean;
  egress_rate_limit_per_min: number;
}

export interface FirewallPatterns {
  injection_patterns: string[];
  pii_patterns: string[];
  exfil_patterns: string[];
  sensitive_paths: string[];
  ssn_regex: string;
  passport_regex: string;
  internal_ip_regex: string;
}

// ── Marketplace Types ──

export interface MarketplaceAgent {
  package_id: string;
  name: string;
  description: string;
  author: string;
  tags: string[];
  version: string;
  capabilities: string[];
  price_cents: number;
  downloads: number;
  rating: number;
  review_count: number;
}

export interface MarketplaceReview {
  reviewer: string;
  stars: number;
  comment: string;
  created_at: string;
}

export interface MarketplaceVersion {
  version: string;
  changelog: string;
  created_at: string;
}

export interface MarketplaceDetail {
  agent: MarketplaceAgent;
  reviews: MarketplaceReview[];
  versions: MarketplaceVersion[];
}

export interface MarketplaceCheck {
  name: string;
  passed: boolean;
  findings: string[];
}

export interface MarketplacePublishResult {
  package_id: string;
  name: string;
  version: string;
  verdict: string;
  checks: MarketplaceCheck[];
}

// ── Agent Browser Types ──

export type BrowserMode = "research" | "build" | "learn";

export type ActivityMessageType = "searching" | "reading" | "extracting" | "deciding" | "navigating" | "blocked" | "info" | "merging" | "coding" | "designing";

export interface ActivityMessage {
  id: string;
  timestamp: number;
  agent_id: string;
  agent_name: string;
  message_type: ActivityMessageType;
  content: string;
}

export interface BrowserHistoryEntry {
  url: string;
  title: string;
  timestamp: number;
  agent_id: string | null;
}

export interface BrowserNavigateResult {
  url: string;
  title: string;
  allowed: boolean;
  deny_reason: string | null;
}

// ── Research Mode Types ──

export type ResearchStatus = "idle" | "running" | "merging" | "complete" | "error";

export type SubAgentStatus = "searching" | "reading" | "extracting" | "merging" | "idle" | "done" | "error";

export interface SubAgentState {
  agent_id: string;
  agent_name: string;
  status: SubAgentStatus;
  current_url: string | null;
  query: string;
  findings: string[];
  pages_visited: number;
  fuel_used: number;
}

export interface ResearchSessionState {
  session_id: string;
  topic: string;
  status: ResearchStatus;
  supervisor_message: string;
  sub_agents: SubAgentState[];
  summary: string | null;
  total_fuel_used: number;
  pages_visited: number;
}

// ── Build Mode Types ──

export type BuildStatus = "idle" | "planning" | "coding" | "complete" | "error";

export interface BuildAgentMessage {
  id: string;
  timestamp: number;
  agent_name: string;
  role: "supervisor" | "coder" | "designer";
  content: string;
}

export interface BuildSessionState {
  session_id: string;
  description: string;
  status: BuildStatus;
  code: string;
  preview_html: string;
  messages: BuildAgentMessage[];
  fuel_used: number;
  llm_calls: number;
}

export interface BuildCodeDelta {
  session_id: string;
  delta: string;
  full_code: string;
  agent_name: string;
}

export interface ResearchEvent {
  event_type: "research_started" | "agent_searching" | "agent_reading" | "agent_extracted" | "agents_merging" | "research_complete";
  session_id: string;
  agent_id: string | null;
  agent_name: string | null;
  message: string;
  url: string | null;
  finding: string | null;
  summary: string | null;
}

// ── Learn Mode Types ──

export type LearningStatus = "idle" | "browsing" | "extracting" | "comparing" | "complete" | "error";

export interface LearningSource {
  url: string;
  label: string;
  category: "documentation" | "github" | "blog" | "changelog";
}

export interface KnowledgeEntry {
  id: string;
  title: string;
  source_url: string;
  key_points: string[];
  timestamp: number;
  relevance_score: number;
  category: string;
  is_new: boolean;
  change_summary: string | null;
}

export interface LearningSuggestion {
  id: string;
  title: string;
  description: string;
  source_url: string;
  relevance: "high" | "medium" | "low";
  timestamp: number;
}

export interface LearningSessionState {
  session_id: string;
  status: LearningStatus;
  sources: LearningSource[];
  current_source_idx: number;
  current_url: string | null;
  knowledge_base: KnowledgeEntry[];
  suggestions: LearningSuggestion[];
  fuel_used: number;
  pages_visited: number;
}

export interface LearningEvent {
  event_type: "learning_started" | "agent_browsing" | "agent_extracted" | "knowledge_updated" | "learning_suggestion" | "learning_complete";
  session_id: string;
  message: string;
  url: string | null;
  key_points: string[] | null;
  change_summary: string | null;
}
