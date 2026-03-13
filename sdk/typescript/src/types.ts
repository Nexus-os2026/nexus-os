// ── Agent types ──────────────────────────────────────────────────────────────

export interface AgentManifest {
  name: string;
  version: string;
  capabilities: string[];
  fuel_budget: number;
  autonomy_level?: number | null;
  domain_tags?: string[];
  consent_policy_path?: string | null;
  llm_model?: string | null;
  allowed_endpoints?: string[] | null;
  filesystem_permissions?: FilesystemPermission[];
}

export interface FilesystemPermission {
  path: string;
  read: boolean;
  write: boolean;
}

export interface Agent {
  id: string;
  name: string;
  status: string;
  fuel_remaining: number;
  last_action: string;
}

export interface AgentStatus {
  id: string;
  name: string;
  status: string;
  fuel_remaining: number;
  autonomy_level: number;
  capabilities: string[];
  last_action: string;
}

// ── Permissions ──────────────────────────────────────────────────────────────

export interface Permission {
  key: string;
  label: string;
  enabled: boolean;
  risk_level: string;
}

export interface PermissionCategory {
  category: string;
  permissions: Permission[];
}

export interface UpdatePermissionRequest {
  capability_key: string;
  enabled: boolean;
}

export interface BulkPermissionUpdate {
  capability_key: string;
  enabled: boolean;
}

// ── Audit ────────────────────────────────────────────────────────────────────

export interface AuditEvent {
  event_id: string;
  timestamp: number;
  agent_id: string;
  event_type: string;
  payload: Record<string, unknown>;
  hash: string;
  previous_hash: string | null;
}

export interface AuditQuery {
  agent_id?: string;
  limit?: number;
  offset?: number;
}

export interface AuditEventsResponse {
  events: AuditEvent[];
  total: number;
  limit: number;
  offset: number;
}

// ── Compliance ───────────────────────────────────────────────────────────────

export interface ComplianceStatus {
  status: string;
  agents_checked: number;
  violations: unknown[];
}

export interface ComplianceReport {
  agent_name: string;
  risk_tier: string;
  capabilities: string[];
}

// ── Marketplace ──────────────────────────────────────────────────────────────

export interface MarketplaceEntry {
  package_id: string;
  name: string;
  description: string;
  author: string;
  tags: string[];
}

// ── Identity ─────────────────────────────────────────────────────────────────

export interface AgentIdentityInfo {
  agent_id: string;
  did: string;
  created_at: number;
  public_key_hex: string;
}

// ── Firewall ─────────────────────────────────────────────────────────────────

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

// ── Health ────────────────────────────────────────────────────────────────────

export interface HealthResponse {
  status: string;
  version: string;
  agents_registered: number;
  tasks_in_flight: number;
  started_at: number;
  uptime_seconds: number;
  agents_active: number;
  total_tests_passed: number;
  audit_chain_valid: boolean;
  compliance_status: string;
  memory_usage_bytes: number;
  wasm_cache_hit_rate: number;
}

// ── Anthropic-compatible types ───────────────────────────────────────────────

export interface AnthropicMessageRequest {
  model: string;
  max_tokens: number;
  messages: AnthropicChatMessage[];
  system?: string;
  stream?: boolean;
  temperature?: number;
  top_p?: number;
  metadata?: Record<string, unknown>;
}

export interface AnthropicChatMessage {
  role: "user" | "assistant";
  content: string | AnthropicContentBlock[];
}

export interface AnthropicContentBlock {
  type: "text";
  text: string;
}

export interface AnthropicMessageResponse {
  id: string;
  type: "message";
  role: "assistant";
  content: AnthropicContentBlock[];
  model: string;
  stop_reason: "end_turn" | "max_tokens" | "stop_sequence" | null;
  stop_sequence: string | null;
  usage: AnthropicUsage;
}

export interface AnthropicUsage {
  input_tokens: number;
  output_tokens: number;
}

export interface AnthropicErrorResponse {
  type: "error";
  error: {
    type: string;
    message: string;
  };
}

export interface AnthropicStreamEvent {
  type: string;
  index?: number;
  delta?: { type: string; text?: string; stop_reason?: string };
  content_block?: AnthropicContentBlock;
  message?: AnthropicMessageResponse;
  usage?: AnthropicUsage;
}

// ── OpenAI-compatible types ──────────────────────────────────────────────────

export interface OpenAiChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface OpenAiChatCompletionRequest {
  model: string;
  messages: OpenAiChatMessage[];
  max_tokens?: number;
  temperature?: number;
  stream?: boolean;
}

export interface OpenAiChatCompletion {
  id: string;
  object: "chat.completion";
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: { role: string; content: string };
    finish_reason: string;
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

export interface OpenAiEmbeddingRequest {
  model: string;
  input: string | string[];
}

export interface OpenAiEmbeddingResponse {
  object: "list";
  data: Array<{
    object: "embedding";
    index: number;
    embedding: number[];
  }>;
  model: string;
  usage: {
    prompt_tokens: number;
    total_tokens: number;
  };
}

export interface OpenAiModel {
  id: string;
  object: "model";
  created: number;
  owned_by: string;
}

export interface OpenAiModelList {
  object: "list";
  data: OpenAiModel[];
}

// ── MCP types ────────────────────────────────────────────────────────────────

export interface McpTool {
  name: string;
  description: string;
  fuel_cost: number;
  min_autonomy_level: number;
  requires_hitl: boolean;
}

export interface ToolInvokeRequest {
  agent: string;
  tool: string;
  params?: Record<string, unknown>;
}

export interface ToolInvokeResponse {
  is_error: boolean;
  content: unknown;
  fuel_consumed: number;
  audit_hash: string;
}

// ── A2A types ────────────────────────────────────────────────────────────────

export interface TaskSubmitRequest {
  agent: string;
  message: string;
  correlation_id?: string;
}

export interface TaskSubmitResponse {
  task_id: string;
  status: string;
  agent: string;
  sender: string;
}

export interface A2ATask {
  id: string;
  sender: string;
  receiver: string;
  status: "submitted" | "working" | "completed" | "failed" | "canceled";
  governance?: GovernanceContext;
}

export interface GovernanceContext {
  autonomy_level: number;
  fuel_budget: number;
  fuel_consumed: number;
  required_capabilities: string[];
  hitl_approved: boolean;
  audit_hash: string | null;
}

// ── WebSocket events ─────────────────────────────────────────────────────────

export interface WsEvent {
  type:
    | "agent_status_changed"
    | "fuel_consumed"
    | "audit_event"
    | "compliance_alert"
    | "firewall_block"
    | "speculation_decision";
  data: Record<string, unknown>;
  timestamp: number;
}

// ── RAG types ────────────────────────────────────────────────────────────────

export interface DocumentInfo {
  path: string;
  format: string;
  chunk_count: number;
  indexed_at: string;
}

export interface SearchResult {
  chunk_id: string;
  doc_path: string;
  content: string;
  score: number;
}

export interface ChatResponse {
  answer: string;
  sources: SearchResult[];
}

export interface DocumentGovernance {
  document_id: string;
  access_log: AuditEvent[];
  retention_policy: string;
}

// ── Time Machine types ──────────────────────────────────────────────────────

export interface Checkpoint {
  id: string;
  label: string;
  timestamp: number;
  agent_id: string | null;
  change_count: number;
  undone: boolean;
}

export interface UndoResult {
  checkpoint_id: string;
  label: string;
  actions_applied: number;
  files_restored: string[];
}

export interface RedoResult {
  checkpoint_id: string;
  label: string;
  actions_applied: number;
  files_restored: string[];
}

// ── Client options ───────────────────────────────────────────────────────────

export interface NexusClientOptions {
  baseUrl?: string;
  token?: string;
  apiKey?: string;
}
