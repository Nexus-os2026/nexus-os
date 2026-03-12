"""Nexus OS Python SDK — governed deterministic agent OS."""

from __future__ import annotations

from .errors import NexusApiError, NexusAuthError, NexusNotFoundError, NexusRateLimitError
from .streaming import SSEParser
from .types import (
    Agent,
    AgentManifest,
    AnthropicMessage,
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

__version__ = "1.0.0"

# NexusClient requires httpx — import lazily so types/errors are always usable.
try:
    from .client import NexusClient
except ImportError:
    pass

__all__ = [
    "NexusClient",
    "NexusApiError",
    "NexusAuthError",
    "NexusNotFoundError",
    "NexusRateLimitError",
    "SSEParser",
    "Agent",
    "AgentManifest",
    "AnthropicMessage",
    "AnthropicResponse",
    "AuditEvent",
    "Checkpoint",
    "LlmResponse",
    "Permission",
    "RagDocument",
    "SearchResult",
    "SystemInfo",
    "UndoResult",
]
