"""Voice activity detection helpers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, List, Sequence


@dataclass
class SpeechSegment:
    start_ms: int
    end_ms: int
    chunks: int


class SileroVAD:
    """Lightweight local VAD fallback with Silero-compatible shape."""

    def __init__(self, energy_threshold: float = 0.015) -> None:
        self.energy_threshold = energy_threshold

    def is_speech(self, chunk: Sequence[float]) -> bool:
        if not chunk:
            return False
        avg_energy = sum(abs(value) for value in chunk) / len(chunk)
        return avg_energy >= self.energy_threshold

    def segment_speech(self, chunks: Iterable[Sequence[float]], chunk_ms: int = 30) -> List[SpeechSegment]:
        segments: List[SpeechSegment] = []

        in_segment = False
        segment_start = 0
        chunk_index = 0
        segment_chunks = 0

        for chunk in chunks:
            speaking = self.is_speech(chunk)
            if speaking and not in_segment:
                in_segment = True
                segment_start = chunk_index * chunk_ms
                segment_chunks = 1
            elif speaking and in_segment:
                segment_chunks += 1
            elif not speaking and in_segment:
                segments.append(
                    SpeechSegment(
                        start_ms=segment_start,
                        end_ms=chunk_index * chunk_ms,
                        chunks=segment_chunks,
                    )
                )
                in_segment = False
                segment_chunks = 0

            chunk_index += 1

        if in_segment:
            segments.append(
                SpeechSegment(
                    start_ms=segment_start,
                    end_ms=chunk_index * chunk_ms,
                    chunks=segment_chunks,
                )
            )

        return segments
