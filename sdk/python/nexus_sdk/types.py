from __future__ import annotations

from dataclasses import dataclass, fields
from typing import Any, Dict, List, Optional


def _from_dict(cls: type, data: Dict[str, Any]) -> Any:
    """Construct a dataclass from a dict, ignoring unknown keys."""
    known = {f.name for f in fields(cls)}
    return cls(**{k: v for k, v in data.items() if k in known})


# ── Agent types ──────────────────────────────────────────────────────────────


@dataclass
class Agent:
    id: str
    name: str
    status: str
    capabilities: List[str]
    fuel_budget: int
    fuel_remaining: int
    autonomy_level: int

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Agent:
        return _from_dict(cls, data)


@dataclass
class AgentManifest:
    name: str
    capabilities: List[str]
    fuel_budget: int
    autonomy_level: int
    version: str = "1.0.0"
    domain_tags: Optional[List[str]] = None
    filesystem_permissions: Optional[List[Dict[str, Any]]] = None
    allowed_endpoints: Optional[List[str]] = None
    llm_model: Optional[str] = None

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> AgentManifest:
        return _from_dict(cls, data)


# ── LLM response types ──────────────────────────────────────────────────────


@dataclass
class LlmResponse:
    output_text: str
    token_count: int
    model_name: str
    tool_calls: List[str]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> LlmResponse:
        return _from_dict(cls, data)


# ── Anthropic-compatible types ───────────────────────────────────────────────


@dataclass
class AnthropicMessage:
    role: str
    content: Any  # str or list[dict]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> AnthropicMessage:
        return _from_dict(cls, data)


@dataclass
class AnthropicResponse:
    id: str
    type: str
    role: str
    content: List[Dict[str, Any]]
    model: str
    stop_reason: Optional[str]
    usage: Dict[str, Any]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> AnthropicResponse:
        return _from_dict(cls, data)


# ── RAG types ────────────────────────────────────────────────────────────────


@dataclass
class RagDocument:
    path: str
    format: str
    chunk_count: int
    indexed_at: str

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> RagDocument:
        return _from_dict(cls, data)


@dataclass
class SearchResult:
    chunk_id: str
    doc_path: str
    content: str
    score: float

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> SearchResult:
        return _from_dict(cls, data)


# ── Time Machine types ───────────────────────────────────────────────────────


@dataclass
class Checkpoint:
    id: str
    label: str
    timestamp: int
    agent_id: Optional[str]
    change_count: int
    undone: bool

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Checkpoint:
        return _from_dict(cls, data)


@dataclass
class UndoResult:
    checkpoint_id: str
    label: str
    actions_applied: int
    files_restored: List[str]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> UndoResult:
        return _from_dict(cls, data)


# ── Permission types ─────────────────────────────────────────────────────────


@dataclass
class Permission:
    capability: str
    granted: bool

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Permission:
        return _from_dict(cls, data)


# ── Audit types ──────────────────────────────────────────────────────────────


@dataclass
class AuditEvent:
    id: str
    agent_id: str
    event_type: str
    timestamp: str
    payload: Dict[str, Any]

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> AuditEvent:
        return _from_dict(cls, data)


# ── System types ─────────────────────────────────────────────────────────────


@dataclass
class SystemInfo:
    total_ram_mb: int
    available_ram_mb: int
    cpu_name: str
    cpu_cores: int

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> SystemInfo:
        return _from_dict(cls, data)
