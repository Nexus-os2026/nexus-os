"""Unit tests for the Nexus OS Python SDK (no server required)."""

from __future__ import annotations

import sys
import os
import unittest

# Ensure the SDK package is importable without installation.
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from nexus_sdk.types import (
    Agent,
    AgentManifest,
    AnthropicResponse,
    AuditEvent,
    Checkpoint,
    LlmResponse,
    Permission,
    RagDocument,
    SearchResult,
    SystemInfo,
    UndoResult,
)
from nexus_sdk.errors import NexusApiError, NexusAuthError, NexusNotFoundError, NexusRateLimitError
from nexus_sdk.streaming import SSEParser


# ── Type deserialization tests ───────────────────────────────────────────────


class TestAgent(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "id": "abc-123",
            "name": "test-agent",
            "status": "running",
            "capabilities": ["fs.read", "llm.query"],
            "fuel_budget": 10000,
            "fuel_remaining": 8000,
            "autonomy_level": 2,
        }
        agent = Agent.from_dict(data)
        self.assertEqual(agent.id, "abc-123")
        self.assertEqual(agent.name, "test-agent")
        self.assertEqual(agent.status, "running")
        self.assertEqual(len(agent.capabilities), 2)
        self.assertEqual(agent.fuel_remaining, 8000)

    def test_from_dict_extra_fields(self) -> None:
        data = {
            "id": "x",
            "name": "a",
            "status": "stopped",
            "capabilities": [],
            "fuel_budget": 0,
            "fuel_remaining": 0,
            "autonomy_level": 0,
            "unknown_field": "should be ignored",
            "another_extra": 42,
        }
        agent = Agent.from_dict(data)
        self.assertEqual(agent.id, "x")
        self.assertFalse(hasattr(agent, "unknown_field"))


class TestAgentManifest(unittest.TestCase):
    def test_from_dict_minimal(self) -> None:
        data = {
            "name": "my-agent",
            "capabilities": ["web.search"],
            "fuel_budget": 5000,
            "autonomy_level": 1,
        }
        m = AgentManifest.from_dict(data)
        self.assertEqual(m.name, "my-agent")
        self.assertEqual(m.version, "1.0.0")
        self.assertIsNone(m.filesystem_permissions)

    def test_from_dict_full(self) -> None:
        data = {
            "name": "full-agent",
            "capabilities": ["fs.read", "fs.write"],
            "fuel_budget": 20000,
            "autonomy_level": 3,
            "version": "2.0.0",
            "domain_tags": ["coding"],
            "llm_model": "llama3",
        }
        m = AgentManifest.from_dict(data)
        self.assertEqual(m.version, "2.0.0")
        self.assertEqual(m.llm_model, "llama3")


class TestAnthropicResponse(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "id": "msg_abc123",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello there!"}],
            "model": "llama3",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
        }
        resp = AnthropicResponse.from_dict(data)
        self.assertEqual(resp.id, "msg_abc123")
        self.assertEqual(resp.type, "message")
        self.assertEqual(resp.role, "assistant")
        self.assertEqual(len(resp.content), 1)
        self.assertEqual(resp.content[0]["text"], "Hello there!")
        self.assertEqual(resp.stop_reason, "end_turn")
        self.assertEqual(resp.usage["input_tokens"], 10)

    def test_from_dict_no_stop_reason(self) -> None:
        data = {
            "id": "msg_x",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "gpt-4o",
            "stop_reason": None,
            "usage": {"input_tokens": 0, "output_tokens": 0},
        }
        resp = AnthropicResponse.from_dict(data)
        self.assertIsNone(resp.stop_reason)


class TestCheckpoint(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "id": "cp-001",
            "label": "before deploy",
            "timestamp": 1709251200,
            "agent_id": None,
            "change_count": 3,
            "undone": False,
        }
        cp = Checkpoint.from_dict(data)
        self.assertEqual(cp.label, "before deploy")
        self.assertEqual(cp.change_count, 3)
        self.assertFalse(cp.undone)
        self.assertIsNone(cp.agent_id)


class TestRagDocument(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "path": "/docs/architecture.md",
            "format": "Markdown",
            "chunk_count": 12,
            "indexed_at": "2025-01-15T10:30:00Z",
        }
        doc = RagDocument.from_dict(data)
        self.assertEqual(doc.path, "/docs/architecture.md")
        self.assertEqual(doc.chunk_count, 12)


class TestSearchResult(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "chunk_id": "c-42",
            "doc_path": "/docs/test.md",
            "content": "Governance ensures safety",
            "score": 0.92,
        }
        sr = SearchResult.from_dict(data)
        self.assertAlmostEqual(sr.score, 0.92)
        self.assertEqual(sr.chunk_id, "c-42")


class TestUndoResult(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "checkpoint_id": "cp-001",
            "label": "before deploy",
            "actions_applied": 5,
            "files_restored": ["/a.txt", "/b.rs"],
        }
        ur = UndoResult.from_dict(data)
        self.assertEqual(ur.actions_applied, 5)
        self.assertEqual(len(ur.files_restored), 2)


class TestPermission(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {"capability": "fs.write", "granted": True}
        p = Permission.from_dict(data)
        self.assertTrue(p.granted)
        self.assertEqual(p.capability, "fs.write")


class TestAuditEvent(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "id": "evt-001",
            "agent_id": "agent-abc",
            "event_type": "UserAction",
            "timestamp": "2025-03-12T00:00:00Z",
            "payload": {"action": "create_agent"},
        }
        evt = AuditEvent.from_dict(data)
        self.assertEqual(evt.event_type, "UserAction")
        self.assertEqual(evt.payload["action"], "create_agent")


class TestSystemInfo(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "total_ram_mb": 16384,
            "available_ram_mb": 8192,
            "cpu_name": "AMD Ryzen 9",
            "cpu_cores": 16,
        }
        si = SystemInfo.from_dict(data)
        self.assertEqual(si.cpu_cores, 16)
        self.assertEqual(si.total_ram_mb, 16384)


class TestLlmResponse(unittest.TestCase):
    def test_from_dict(self) -> None:
        data = {
            "output_text": "Hello world",
            "token_count": 42,
            "model_name": "llama3",
            "tool_calls": [],
        }
        lr = LlmResponse.from_dict(data)
        self.assertEqual(lr.output_text, "Hello world")
        self.assertEqual(lr.token_count, 42)


# ── Error tests ──────────────────────────────────────────────────────────────


class TestErrors(unittest.TestCase):
    def test_api_error_message(self) -> None:
        err = NexusApiError(500, "/api/agents", "Internal server error")
        self.assertIn("500", str(err))
        self.assertIn("/api/agents", str(err))
        self.assertEqual(err.status_code, 500)
        self.assertEqual(err.endpoint, "/api/agents")

    def test_auth_error_is_subclass(self) -> None:
        err = NexusAuthError(401, "/api/agents", "Unauthorized")
        self.assertIsInstance(err, NexusApiError)
        self.assertEqual(err.status_code, 401)

    def test_not_found_error(self) -> None:
        err = NexusNotFoundError(404, "/api/agents/xyz", "Not found")
        self.assertIsInstance(err, NexusApiError)
        self.assertEqual(err.status_code, 404)
        self.assertIn("xyz", str(err))

    def test_rate_limit_error(self) -> None:
        err = NexusRateLimitError(429, "/v1/messages", "Fuel exhausted")
        self.assertIsInstance(err, NexusApiError)
        self.assertEqual(err.status_code, 429)


# ── SSE Parser tests ─────────────────────────────────────────────────────────


class FakeSSEResponse:
    """Simulate an httpx streaming response for SSE parsing."""

    def __init__(self, lines: list[str]) -> None:
        self._lines = lines

    def iter_lines(self):  # type: ignore[no-untyped-def]
        yield from self._lines


class TestSSEParser(unittest.TestCase):
    def test_parse_multiple_events(self) -> None:
        lines = [
            "event: message_start",
            'data: {"type":"message_start","message":{"id":"msg_1"}}',
            "",
            "event: content_block_delta",
            'data: {"type":"content_block_delta","delta":{"text":"Hello"}}',
            "",
            "event: message_stop",
            'data: {"type":"message_stop"}',
            "",
        ]
        events = list(SSEParser(FakeSSEResponse(lines)))
        self.assertEqual(len(events), 3)
        self.assertEqual(events[0]["_event"], "message_start")
        self.assertEqual(events[0]["message"]["id"], "msg_1")
        self.assertEqual(events[1]["_event"], "content_block_delta")
        self.assertEqual(events[1]["delta"]["text"], "Hello")
        self.assertEqual(events[2]["_event"], "message_stop")

    def test_parse_empty_response(self) -> None:
        events = list(SSEParser(FakeSSEResponse([])))
        self.assertEqual(len(events), 0)

    def test_parse_malformed_json(self) -> None:
        lines = [
            "event: error",
            "data: not valid json",
            "",
        ]
        events = list(SSEParser(FakeSSEResponse(lines)))
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["_event"], "error")
        self.assertEqual(events[0]["_raw"], "not valid json")

    def test_parse_multiline_data(self) -> None:
        lines = [
            "event: big",
            "data: {",
            'data: "key": "value"',
            "data: }",
            "",
        ]
        events = list(SSEParser(FakeSSEResponse(lines)))
        # Multi-line data is joined — but this won't be valid JSON in general.
        # The parser yields _raw for malformed JSON.
        self.assertEqual(len(events), 1)

    def test_parse_no_event_type(self) -> None:
        lines = [
            'data: {"type":"ping"}',
            "",
        ]
        events = list(SSEParser(FakeSSEResponse(lines)))
        self.assertEqual(len(events), 1)
        self.assertIsNone(events[0]["_event"])
        self.assertEqual(events[0]["type"], "ping")


# ── Client construction tests ────────────────────────────────────────────────

try:
    import httpx as _httpx  # noqa: F401

    _HAS_HTTPX = True
except ImportError:
    _HAS_HTTPX = False


@unittest.skipUnless(_HAS_HTTPX, "httpx not installed")
class TestClientConstruction(unittest.TestCase):
    def test_default_base_url(self) -> None:
        from nexus_sdk.client import NexusClient

        c = NexusClient()
        self.assertEqual(c.base_url, "http://localhost:3000")
        c.close()

    def test_custom_base_url_strips_trailing_slash(self) -> None:
        from nexus_sdk.client import NexusClient

        c = NexusClient(base_url="http://example.com:8080/")
        self.assertEqual(c.base_url, "http://example.com:8080")
        c.close()

    def test_headers_with_api_key(self) -> None:
        from nexus_sdk.client import NexusClient

        c = NexusClient(api_key="test-key-123")
        headers = c._headers()
        self.assertEqual(headers["x-api-key"], "test-key-123")
        self.assertNotIn("Authorization", headers)
        c.close()

    def test_headers_with_token(self) -> None:
        from nexus_sdk.client import NexusClient

        c = NexusClient(token="jwt-token-abc")
        headers = c._headers()
        self.assertEqual(headers["Authorization"], "Bearer jwt-token-abc")
        self.assertNotIn("x-api-key", headers)
        c.close()

    def test_api_key_takes_precedence(self) -> None:
        from nexus_sdk.client import NexusClient

        c = NexusClient(token="jwt", api_key="key")
        headers = c._headers()
        self.assertIn("x-api-key", headers)
        self.assertNotIn("Authorization", headers)
        c.close()

    def test_context_manager(self) -> None:
        from nexus_sdk.client import NexusClient

        with NexusClient() as c:
            self.assertIsNotNone(c._client)


if __name__ == "__main__":
    unittest.main()
