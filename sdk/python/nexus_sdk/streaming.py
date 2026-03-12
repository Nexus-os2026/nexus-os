from __future__ import annotations

import json
from typing import Any, Dict, Iterator


class SSEParser:
    """Parse Server-Sent Events from an httpx streaming response.

    Yields each event as a dict with an ``_event`` key holding the event type.
    """

    def __init__(self, response: Any) -> None:
        self.response = response

    def __iter__(self) -> Iterator[Dict[str, Any]]:
        event_type: str | None = None
        data_lines: list[str] = []

        for line in self.response.iter_lines():
            if line.startswith("event:"):
                event_type = line[6:].strip()
            elif line.startswith("data:"):
                data_lines.append(line[5:].strip())
            elif line == "":
                if data_lines:
                    data = "\n".join(data_lines)
                    try:
                        parsed: Dict[str, Any] = json.loads(data)
                        parsed["_event"] = event_type
                        yield parsed
                    except json.JSONDecodeError:
                        yield {"_event": event_type, "_raw": data}
                    event_type = None
                    data_lines = []
