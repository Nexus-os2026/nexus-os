from __future__ import annotations


class NexusApiError(Exception):
    """Base error for Nexus OS API calls."""

    def __init__(self, status_code: int, endpoint: str, message: str) -> None:
        self.status_code = status_code
        self.endpoint = endpoint
        super().__init__(f"[{status_code}] {endpoint}: {message}")


class NexusAuthError(NexusApiError):
    """Raised on 401 Unauthorized."""

    pass


class NexusNotFoundError(NexusApiError):
    """Raised on 404 Not Found."""

    pass


class NexusRateLimitError(NexusApiError):
    """Raised on 429 Too Many Requests (fuel exhausted)."""

    pass
