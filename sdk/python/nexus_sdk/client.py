from __future__ import annotations

from dataclasses import asdict
from typing import Any, Dict, Iterator, List, Optional

import httpx

from .errors import NexusApiError, NexusAuthError, NexusNotFoundError, NexusRateLimitError
from .streaming import SSEParser
from .types import (
    Agent,
    AgentManifest,
    AnthropicResponse,
    AuditEvent,
    Checkpoint,
    Permission,
    RagDocument,
    SearchResult,
    SystemInfo,
    UndoResult,
)


class NexusClient:
    """Python SDK for Nexus OS.

    Provides sync methods for all Nexus OS HTTP gateway endpoints including
    OpenAI-compatible and Anthropic-compatible LLM APIs.

    Usage::

        client = NexusClient(base_url="http://localhost:3000", api_key="key")
        resp = client.messages(
            messages=[{"role": "user", "content": "Hello!"}],
            model="llama3",
        )
        print(resp.content)
    """

    def __init__(
        self,
        base_url: str = "http://localhost:3000",
        token: Optional[str] = None,
        api_key: Optional[str] = None,
        timeout: float = 120.0,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.api_key = api_key
        self._client = httpx.Client(timeout=timeout)

    # ── Private helpers ──────────────────────────────────────────────────

    def _headers(self) -> Dict[str, str]:
        headers: Dict[str, str] = {"Content-Type": "application/json"}
        if self.api_key:
            headers["x-api-key"] = self.api_key
        elif self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def _request(
        self,
        method: str,
        path: str,
        body: Any = None,
    ) -> Any:
        url = f"{self.base_url}{path}"
        response = self._client.request(
            method,
            url,
            headers=self._headers(),
            json=body,
        )
        if response.status_code == 401:
            raise NexusAuthError(401, path, response.text)
        if response.status_code == 404:
            raise NexusNotFoundError(404, path, response.text)
        if response.status_code == 429:
            raise NexusRateLimitError(429, path, response.text)
        if response.status_code >= 400:
            raise NexusApiError(response.status_code, path, response.text)
        if not response.text:
            return {}
        return response.json()

    def close(self) -> None:
        """Close the underlying HTTP client."""
        self._client.close()

    def __enter__(self) -> NexusClient:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    # ── System ───────────────────────────────────────────────────────────

    def health(self) -> Dict[str, Any]:
        """GET /health — system health check."""
        return self._request("GET", "/health")

    def system_info(self) -> SystemInfo:
        """GET /system/info — system information."""
        return SystemInfo.from_dict(self._request("GET", "/system/info"))

    def metrics(self) -> str:
        """GET /metrics — Prometheus metrics (raw text)."""
        url = f"{self.base_url}/metrics"
        response = self._client.get(url, headers=self._headers())
        return response.text

    # ── Agents ───────────────────────────────────────────────────────────

    def list_agents(self) -> List[Agent]:
        """GET /api/agents — list all registered agents."""
        data = self._request("GET", "/api/agents")
        agents = data.get("agents", data) if isinstance(data, dict) else data
        return [Agent.from_dict(a) for a in agents]

    def create_agent(self, manifest: AgentManifest | Dict[str, Any]) -> Dict[str, Any]:
        """POST /api/agents — create an agent from a manifest."""
        body: Dict[str, Any]
        if isinstance(manifest, AgentManifest):
            body = {"manifest": asdict(manifest)}
        else:
            body = {"manifest": manifest}
        return self._request("POST", "/api/agents", body)

    def start_agent(self, agent_id: str) -> Dict[str, Any]:
        """POST /api/agents/{id}/start — start an agent."""
        return self._request("POST", f"/api/agents/{agent_id}/start")

    def stop_agent(self, agent_id: str) -> Dict[str, Any]:
        """POST /api/agents/{id}/stop — stop an agent."""
        return self._request("POST", f"/api/agents/{agent_id}/stop")

    def get_agent_status(self, agent_id: str) -> Dict[str, Any]:
        """GET /api/agents/{id}/status — get agent status."""
        return self._request("GET", f"/api/agents/{agent_id}/status")

    # ── Permissions ──────────────────────────────────────────────────────

    def get_permissions(self, agent_id: str) -> List[Dict[str, Any]]:
        """GET /api/agents/{id}/permissions — get permission categories."""
        return self._request("GET", f"/api/agents/{agent_id}/permissions")

    def update_permission(
        self, agent_id: str, capability_key: str, enabled: bool
    ) -> Dict[str, Any]:
        """PUT /api/agents/{id}/permissions — update a single permission."""
        body = {"capability_key": capability_key, "enabled": enabled}
        return self._request("PUT", f"/api/agents/{agent_id}/permissions", body)

    def bulk_update_permissions(
        self,
        agent_id: str,
        updates: List[Dict[str, Any]],
        reason: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST /api/agents/{id}/permissions/bulk — bulk update permissions."""
        body: Dict[str, Any] = {"updates": updates}
        if reason:
            body["reason"] = reason
        return self._request(
            "POST", f"/api/agents/{agent_id}/permissions/bulk", body
        )

    # ── Audit ────────────────────────────────────────────────────────────

    def query_audit_log(
        self,
        agent_id: Optional[str] = None,
        limit: int = 50,
        offset: int = 0,
    ) -> Dict[str, Any]:
        """GET /api/audit/events — paginated audit events."""
        params = f"?limit={limit}&offset={offset}"
        if agent_id:
            params += f"&agent_id={agent_id}"
        return self._request("GET", f"/api/audit/events{params}")

    def get_audit_event(self, event_id: str) -> Dict[str, Any]:
        """GET /api/audit/events/{id} — get a single audit event."""
        return self._request("GET", f"/api/audit/events/{event_id}")

    # ── Compliance ───────────────────────────────────────────────────────

    def compliance_status(self) -> Dict[str, Any]:
        """GET /api/compliance/status — overall compliance status."""
        return self._request("GET", "/api/compliance/status")

    def compliance_report(self, agent_id: str) -> Dict[str, Any]:
        """GET /api/compliance/report/{agent_id} — transparency report."""
        return self._request("GET", f"/api/compliance/report/{agent_id}")

    def compliance_erase(
        self, agent_id: str, encryption_key_ids: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """POST /api/compliance/erase/{agent_id} — GDPR cryptographic erasure."""
        body = {"encryption_key_ids": encryption_key_ids or []}
        return self._request("POST", f"/api/compliance/erase/{agent_id}", body)

    # ── Marketplace ──────────────────────────────────────────────────────

    def search_marketplace(self, query: Optional[str] = None) -> Dict[str, Any]:
        """GET /api/marketplace/search — search marketplace agents."""
        qs = f"?q={query}" if query else ""
        return self._request("GET", f"/api/marketplace/search{qs}")

    def get_marketplace_agent(self, agent_id: str) -> Dict[str, Any]:
        """GET /api/marketplace/agents/{id} — marketplace agent detail."""
        return self._request("GET", f"/api/marketplace/agents/{agent_id}")

    def install_marketplace_agent(self, agent_id: str) -> Dict[str, Any]:
        """POST /api/marketplace/install/{id} — install marketplace agent."""
        return self._request("POST", f"/api/marketplace/install/{agent_id}")

    # ── Identity ─────────────────────────────────────────────────────────

    def list_identities(self) -> Dict[str, Any]:
        """GET /api/identity/agents — list agent DID identities."""
        return self._request("GET", "/api/identity/agents")

    def get_identity(self, agent_id: str) -> Dict[str, Any]:
        """GET /api/identity/agents/{id} — get agent DID identity."""
        return self._request("GET", f"/api/identity/agents/{agent_id}")

    # ── Firewall ─────────────────────────────────────────────────────────

    def firewall_status(self) -> Dict[str, Any]:
        """GET /api/firewall/status — prompt firewall status."""
        return self._request("GET", "/api/firewall/status")

    # ── A2A ──────────────────────────────────────────────────────────────

    def submit_task(
        self,
        agent: str,
        message: str,
        correlation_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST /a2a — submit an A2A task."""
        body: Dict[str, Any] = {"agent": agent, "message": message}
        if correlation_id:
            body["correlation_id"] = correlation_id
        return self._request("POST", "/a2a", body)

    def get_task_status(self, task_id: str) -> Dict[str, Any]:
        """GET /a2a/tasks/{id} — get A2A task status."""
        return self._request("GET", f"/a2a/tasks/{task_id}")

    # ── MCP ──────────────────────────────────────────────────────────────

    def list_tools(self, agent_name: str) -> Dict[str, Any]:
        """GET /mcp/tools/list — list governed MCP tools for an agent."""
        return self._request("GET", f"/mcp/tools/list?agent={agent_name}")

    def invoke_tool(
        self,
        agent: str,
        tool: str,
        params: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """POST /mcp/tools/invoke — invoke a governed MCP tool."""
        body: Dict[str, Any] = {"agent": agent, "tool": tool}
        if params:
            body["params"] = params
        return self._request("POST", "/mcp/tools/invoke", body)

    # ── LLM — OpenAI compatible ──────────────────────────────────────────

    def chat_completion(
        self,
        messages: List[Dict[str, str]],
        model: str = "nexus-governed",
        max_tokens: int = 1024,
        **kwargs: Any,
    ) -> Dict[str, Any]:
        """POST /v1/chat/completions — OpenAI Chat Completions compatible."""
        body: Dict[str, Any] = {
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
            **kwargs,
        }
        return self._request("POST", "/v1/chat/completions", body)

    def embeddings(
        self,
        input: str | List[str],
        model: str = "text-embedding-ada-002",
    ) -> Dict[str, Any]:
        """POST /v1/embeddings — OpenAI Embeddings compatible."""
        body = {"model": model, "input": input}
        return self._request("POST", "/v1/embeddings", body)

    def list_models(self) -> Dict[str, Any]:
        """GET /v1/models — list available models."""
        return self._request("GET", "/v1/models")

    # ── LLM — Anthropic compatible ───────────────────────────────────────

    def messages(
        self,
        messages: List[Dict[str, Any]],
        model: str = "nexus-governed",
        max_tokens: int = 1024,
        system: Optional[str] = None,
        **kwargs: Any,
    ) -> AnthropicResponse:
        """POST /v1/messages — Anthropic Messages API compatible."""
        body: Dict[str, Any] = {
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
            **kwargs,
        }
        if system:
            body["system"] = system
        data = self._request("POST", "/v1/messages", body)
        return AnthropicResponse.from_dict(data)

    def messages_stream(
        self,
        messages: List[Dict[str, Any]],
        model: str = "nexus-governed",
        max_tokens: int = 1024,
        system: Optional[str] = None,
        **kwargs: Any,
    ) -> Iterator[Dict[str, Any]]:
        """POST /v1/messages (stream=true) — yields parsed SSE events.

        Usage::

            for event in client.messages_stream(
                messages=[{"role": "user", "content": "Hello"}],
            ):
                if "delta" in event:
                    print(event["delta"].get("text", ""), end="")
        """
        body: Dict[str, Any] = {
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
            "stream": True,
            **kwargs,
        }
        if system:
            body["system"] = system
        url = f"{self.base_url}/v1/messages"
        with self._client.stream(
            "POST", url, headers=self._headers(), json=body
        ) as response:
            if response.status_code >= 400:
                response.read()
                raise NexusApiError(
                    response.status_code, "/v1/messages", response.text
                )
            for event in SSEParser(response):
                yield event

    # ── RAG ───────────────────────────────────────────────────────────────

    def index_document(self, file_path: str) -> RagDocument:
        """POST /rag/index — index a document for RAG retrieval."""
        data = self._request("POST", "/rag/index", {"file_path": file_path})
        return RagDocument.from_dict(data)

    def search_documents(
        self, query: str, top_k: int = 5
    ) -> List[SearchResult]:
        """POST /rag/search — search indexed documents."""
        data = self._request(
            "POST", "/rag/search", {"query": query, "top_k": top_k}
        )
        results = data if isinstance(data, list) else data.get("results", [])
        return [SearchResult.from_dict(r) for r in results]

    def chat_with_documents(self, question: str) -> Dict[str, Any]:
        """POST /rag/chat — chat with indexed documents."""
        return self._request("POST", "/rag/chat", {"question": question})

    # ── Time Machine ─────────────────────────────────────────────────────

    def list_checkpoints(self) -> List[Checkpoint]:
        """GET /time-machine/checkpoints — list all checkpoints."""
        data = self._request("GET", "/time-machine/checkpoints")
        items = data if isinstance(data, list) else data.get("checkpoints", [])
        return [Checkpoint.from_dict(c) for c in items]

    def undo(self) -> UndoResult:
        """POST /time-machine/undo — undo to previous checkpoint."""
        return UndoResult.from_dict(
            self._request("POST", "/time-machine/undo")
        )

    def redo(self) -> UndoResult:
        """POST /time-machine/redo — redo a previously undone checkpoint."""
        return UndoResult.from_dict(
            self._request("POST", "/time-machine/redo")
        )

    def create_checkpoint(self, label: str) -> Dict[str, Any]:
        """POST /time-machine/checkpoint — create a named checkpoint."""
        return self._request(
            "POST", "/time-machine/checkpoint", {"label": label}
        )
