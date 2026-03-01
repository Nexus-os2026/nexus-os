"""Local TTS wrapper for Piper-like streaming output."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Generator


@dataclass
class TtsConfig:
    voice: str = "en_US-lessac-medium"
    speed: float = 1.0
    personality: str = "neutral"


class PiperTTS:
    def __init__(self, config: TtsConfig | None = None) -> None:
        self.config = config or TtsConfig()

    def synthesize_stream(self, text: str) -> Generator[bytes, None, None]:
        for token in text.split():
            yield token.encode("utf-8")
