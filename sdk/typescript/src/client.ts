import {
  NexusApiError,
  NexusAuthError,
  NexusNotFoundError,
  NexusRateLimitError,
} from "./errors.js";
import type {
  Agent,
  AgentManifest,
  AgentStatus,
  AgentIdentityInfo,
  AnthropicMessageRequest,
  AnthropicMessageResponse,
  AnthropicStreamEvent,
  AuditEvent,
  AuditEventsResponse,
  AuditQuery,
  ChatResponse,
  Checkpoint,
  ComplianceReport,
  ComplianceStatus,
  DocumentGovernance,
  DocumentInfo,
  FirewallStatus,
  HealthResponse,
  MarketplaceEntry,
  NexusClientOptions,
  OpenAiChatCompletion,
  OpenAiChatCompletionRequest,
  OpenAiChatMessage,
  OpenAiEmbeddingResponse,
  OpenAiModelList,
  PermissionCategory,
  RedoResult,
  SearchResult,
  UndoResult,
  UpdatePermissionRequest,
  ToolInvokeRequest,
  ToolInvokeResponse,
  TaskSubmitRequest,
  TaskSubmitResponse,
  A2ATask,
} from "./types.js";

export class NexusClient {
  private baseUrl: string;
  private token: string | null;
  private apiKey: string | null;

  constructor(options: NexusClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? "http://localhost:3000").replace(
      /\/$/,
      ""
    );
    this.token = options.token ?? null;
    this.apiKey = options.apiKey ?? null;
  }

  // ── Private helpers ──────────────────────────────────────────────────

  private authHeaders(): Record<string, string> {
    const headers: Record<string, string> = {};
    if (this.apiKey) {
      headers["x-api-key"] = this.apiKey;
    } else if (this.token) {
      headers["authorization"] = `Bearer ${this.token}`;
    }
    return headers;
  }

  private static readonly RETRYABLE_STATUSES = new Set([429, 502, 503, 504]);
  private static readonly MAX_RETRIES = 3;
  private static readonly BACKOFF_DELAYS = [1000, 2000, 4000]; // ms

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    extraHeaders?: Record<string, string>
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = {
      ...this.authHeaders(),
      ...extraHeaders,
    };

    if (body !== undefined) {
      headers["content-type"] = "application/json";
    }

    let lastError: Error | null = null;

    for (let attempt = 0; attempt < NexusClient.MAX_RETRIES; attempt++) {
      const resp = await fetch(url, {
        method,
        headers,
        body: body !== undefined ? JSON.stringify(body) : undefined,
      });

      if (resp.ok) {
        if (resp.status === 204) {
          return null as T;
        }
        return resp.json() as Promise<T>;
      }

      const text = await resp.text().catch(() => "");
      const message = text || resp.statusText;

      // Non-retryable errors — throw immediately
      if (!NexusClient.RETRYABLE_STATUSES.has(resp.status)) {
        switch (resp.status) {
          case 401:
            throw new NexusAuthError(path, message);
          case 404:
            throw new NexusNotFoundError(path, message);
          default:
            throw new NexusApiError(resp.status, path, message);
        }
      }

      // Retryable error — store and maybe retry
      lastError =
        resp.status === 429
          ? new NexusRateLimitError(path, message)
          : new NexusApiError(resp.status, path, message);

      if (attempt < NexusClient.MAX_RETRIES - 1) {
        let delay = NexusClient.BACKOFF_DELAYS[attempt];
        if (resp.status === 429) {
          const retryAfter = resp.headers.get("retry-after");
          if (retryAfter) {
            const parsed = Number(retryAfter);
            if (!Number.isNaN(parsed)) {
              delay = parsed * 1000;
            }
          }
        }
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }

    throw lastError!;
  }

  // ── System ───────────────────────────────────────────────────────────

  async health(): Promise<HealthResponse> {
    return this.request("GET", "/health");
  }

  async metrics(): Promise<string> {
    const resp = await fetch(`${this.baseUrl}/metrics`, {
      headers: this.authHeaders(),
    });
    return resp.text();
  }

  // ── Agents ───────────────────────────────────────────────────────────

  async listAgents(): Promise<Agent[]> {
    const resp = await this.request<{ agents: Agent[] }>("GET", "/api/agents");
    return resp.agents;
  }

  async createAgent(
    manifest: AgentManifest
  ): Promise<{ agent_id: string; name: string; status: string }> {
    return this.request("POST", "/api/agents", { manifest });
  }

  async startAgent(id: string): Promise<{ status: string; agent_id: string }> {
    return this.request("POST", `/api/agents/${id}/start`);
  }

  async stopAgent(id: string): Promise<{ status: string; agent_id: string }> {
    return this.request("POST", `/api/agents/${id}/stop`);
  }

  async getAgentStatus(id: string): Promise<AgentStatus> {
    return this.request("GET", `/api/agents/${id}/status`);
  }

  // ── Permissions ──────────────────────────────────────────────────────

  async getPermissions(agentId: string): Promise<PermissionCategory[]> {
    return this.request("GET", `/api/agents/${agentId}/permissions`);
  }

  async updatePermission(
    agentId: string,
    update: UpdatePermissionRequest
  ): Promise<void> {
    await this.request("PUT", `/api/agents/${agentId}/permissions`, update);
  }

  async bulkUpdatePermissions(
    agentId: string,
    updates: Array<{ capability_key: string; enabled: boolean }>,
    reason?: string
  ): Promise<{ status: string; count: number }> {
    return this.request(
      "POST",
      `/api/agents/${agentId}/permissions/bulk`,
      { updates, reason }
    );
  }

  // ── Audit ────────────────────────────────────────────────────────────

  async queryAuditLog(query?: AuditQuery): Promise<AuditEventsResponse> {
    const params = new URLSearchParams();
    if (query?.agent_id) params.set("agent_id", query.agent_id);
    if (query?.limit !== undefined) params.set("limit", String(query.limit));
    if (query?.offset !== undefined) params.set("offset", String(query.offset));
    const qs = params.toString();
    return this.request("GET", `/api/audit/events${qs ? `?${qs}` : ""}`);
  }

  async getAuditEvent(id: string): Promise<AuditEvent> {
    return this.request("GET", `/api/audit/events/${id}`);
  }

  // ── Compliance ───────────────────────────────────────────────────────

  async complianceStatus(): Promise<ComplianceStatus> {
    return this.request("GET", "/api/compliance/status");
  }

  async complianceReport(agentId: string): Promise<ComplianceReport> {
    return this.request("GET", `/api/compliance/report/${agentId}`);
  }

  async complianceErase(
    agentId: string,
    encryptionKeyIds: string[] = []
  ): Promise<unknown> {
    return this.request("POST", `/api/compliance/erase/${agentId}`, {
      encryption_key_ids: encryptionKeyIds,
    });
  }

  // ── Marketplace ──────────────────────────────────────────────────────

  async searchMarketplace(query?: string): Promise<MarketplaceEntry[]> {
    const qs = query ? `?q=${encodeURIComponent(query)}` : "";
    const resp = await this.request<{ results: MarketplaceEntry[] }>(
      "GET",
      `/api/marketplace/search${qs}`
    );
    return resp.results;
  }

  async getMarketplaceAgent(id: string): Promise<unknown> {
    return this.request("GET", `/api/marketplace/agents/${id}`);
  }

  async installMarketplaceAgent(id: string): Promise<unknown> {
    return this.request("POST", `/api/marketplace/install/${id}`);
  }

  // ── Identity ─────────────────────────────────────────────────────────

  async listIdentities(): Promise<AgentIdentityInfo[]> {
    const resp = await this.request<{ identities: AgentIdentityInfo[] }>(
      "GET",
      "/api/identity/agents"
    );
    return resp.identities;
  }

  async getIdentity(agentId: string): Promise<AgentIdentityInfo> {
    return this.request("GET", `/api/identity/agents/${agentId}`);
  }

  // ── Firewall ─────────────────────────────────────────────────────────

  async firewallStatus(): Promise<FirewallStatus> {
    return this.request("GET", "/api/firewall/status");
  }

  // ── A2A ──────────────────────────────────────────────────────────────

  async submitTask(req: TaskSubmitRequest): Promise<TaskSubmitResponse> {
    return this.request("POST", "/a2a", req);
  }

  async getTaskStatus(taskId: string): Promise<A2ATask> {
    return this.request("GET", `/a2a/tasks/${taskId}`);
  }

  // ── MCP ──────────────────────────────────────────────────────────────

  async listTools(
    agentName: string
  ): Promise<Array<{ name: string; description: string }>> {
    const resp = await this.request<{
      tools: Array<{ name: string; description: string }>;
    }>("GET", `/mcp/tools/list?agent=${encodeURIComponent(agentName)}`);
    return resp.tools;
  }

  async invokeTool(req: ToolInvokeRequest): Promise<ToolInvokeResponse> {
    return this.request("POST", "/mcp/tools/invoke", req);
  }

  // ── LLM — Anthropic compatible ───────────────────────────────────────

  async messages(
    request: AnthropicMessageRequest
  ): Promise<AnthropicMessageResponse> {
    return this.request("POST", "/v1/messages", { ...request, stream: false });
  }

  async *messagesStream(
    request: AnthropicMessageRequest
  ): AsyncGenerator<AnthropicStreamEvent> {
    const url = `${this.baseUrl}/v1/messages`;
    const headers: Record<string, string> = {
      "content-type": "application/json",
      ...this.authHeaders(),
    };

    const resp = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify({ ...request, stream: true }),
    });

    if (!resp.ok) {
      const text = await resp.text().catch(() => "");
      throw new NexusApiError(resp.status, "/v1/messages", text);
    }

    if (!resp.body) {
      return;
    }

    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      let currentData = "";
      for (const line of lines) {
        if (line.startsWith("data: ")) {
          currentData = line.slice(6);
        } else if (line === "" && currentData) {
          try {
            yield JSON.parse(currentData) as AnthropicStreamEvent;
          } catch {
            // skip malformed JSON
          }
          currentData = "";
        }
      }
    }
  }

  // ── LLM — OpenAI compatible ──────────────────────────────────────────

  async chatCompletion(
    messages: OpenAiChatMessage[],
    options?: Omit<OpenAiChatCompletionRequest, "messages">
  ): Promise<OpenAiChatCompletion> {
    return this.request("POST", "/v1/chat/completions", {
      model: options?.model ?? "nexus-governed",
      messages,
      ...options,
    });
  }

  async embeddings(
    input: string | string[],
    model?: string
  ): Promise<OpenAiEmbeddingResponse> {
    return this.request("POST", "/v1/embeddings", {
      model: model ?? "text-embedding-ada-002",
      input,
    });
  }

  async listModels(): Promise<OpenAiModelList> {
    return this.request("GET", "/v1/models");
  }

  // ── RAG ────────────────────────────────────────────────────────────────

  async indexDocument(
    filePath: string,
    options?: { collection?: string }
  ): Promise<DocumentInfo> {
    const body: Record<string, unknown> = { file_path: filePath };
    if (options?.collection) {
      body.collection = options.collection;
    }
    return this.request("POST", "/rag/index", body);
  }

  async searchDocuments(
    query: string,
    options?: { collection?: string; topK?: number }
  ): Promise<SearchResult[]> {
    const body: Record<string, unknown> = { query };
    if (options?.collection) {
      body.collection = options.collection;
    }
    if (options?.topK !== undefined) {
      body.top_k = options.topK;
    }
    const data = await this.request<SearchResult[] | { results: SearchResult[] }>(
      "POST",
      "/rag/search",
      body
    );
    return Array.isArray(data) ? data : data.results;
  }

  async chatWithDocuments(
    query: string,
    options?: { collection?: string }
  ): Promise<ChatResponse> {
    const body: Record<string, unknown> = { question: query };
    if (options?.collection) {
      body.collection = options.collection;
    }
    return this.request("POST", "/rag/chat", body);
  }

  async listIndexedDocuments(collection?: string): Promise<DocumentInfo[]> {
    const qs = collection
      ? `?collection=${encodeURIComponent(collection)}`
      : "";
    const data = await this.request<
      DocumentInfo[] | { documents: DocumentInfo[] }
    >("GET", `/rag/documents${qs}`);
    return Array.isArray(data) ? data : data.documents;
  }

  async removeDocument(documentId: string): Promise<void> {
    await this.request("DELETE", `/rag/documents/${documentId}`);
  }

  async getDocumentGovernance(
    documentId: string
  ): Promise<DocumentGovernance> {
    return this.request(
      "GET",
      `/rag/documents/${documentId}/governance`
    );
  }

  // ── Time Machine ──────────────────────────────────────────────────────

  async listCheckpoints(): Promise<Checkpoint[]> {
    const data = await this.request<
      Checkpoint[] | { checkpoints: Checkpoint[] }
    >("GET", "/time-machine/checkpoints");
    return Array.isArray(data) ? data : data.checkpoints;
  }

  async createCheckpoint(description: string): Promise<Checkpoint> {
    return this.request("POST", "/time-machine/checkpoint", {
      label: description,
    });
  }

  async undo(): Promise<UndoResult> {
    return this.request("POST", "/time-machine/undo");
  }

  async redo(): Promise<RedoResult> {
    return this.request("POST", "/time-machine/redo");
  }
}
