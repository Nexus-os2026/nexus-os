"""Voice activity detection helpers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, List, Sequence


@dataclass
class SpeechSegment:
    start_ms: int
    end_ms: int
    chunks: int
    audio_bytes: bytes
    duration_ms: int


class SileroVAD:
    """Silero-backed VAD when available, with local fallback."""

    def __init__(
        self,
        energy_threshold: float = 0.015,
        speech_probability_threshold: float = 0.5,
    ) -> None:
        self.energy_threshold = energy_threshold
        self.speech_probability_threshold = speech_probability_threshold
        self._silero_model = self._load_silero_model()

    def _load_silero_model(self) -> object | None:
        try:
            from silero_vad import load_silero_vad  # type: ignore

            return load_silero_vad()
        except Exception:
            return None

    def is_speech(self, chunk: Sequence[float]) -> bool:
        if not chunk:
            return False
        silero_decision = self._silero_predict(chunk)
        if silero_decision is not None:
            return silero_decision
        avg_energy = sum(abs(value) for value in chunk) / len(chunk)
        return avg_energy >= self.energy_threshold

    def _silero_predict(self, chunk: Sequence[float]) -> bool | None:
        if self._silero_model is None:
            return None
        try:
            import numpy as np  # type: ignore
            import torch  # type: ignore
        except Exception:
            return None

        try:
            audio = np.asarray(chunk, dtype=np.float32)
            audio_tensor = torch.tensor(audio)
            probability = float(self._silero_model(audio_tensor, 16000).item())
            return probability >= self.speech_probability_threshold
        except Exception:
            return None

    def segment_speech(self, chunks: Iterable[Sequence[float]], chunk_ms: int = 30) -> List[SpeechSegment]:
        segments: List[SpeechSegment] = []
        in_segment = False
        segment_start = 0
        chunk_index = 0
        segment_chunks = 0
        buffered_audio = bytearray()

        for chunk in chunks:
            speaking = self.is_speech(chunk)
            if speaking and not in_segment:
                in_segment = True
                segment_start = chunk_index * chunk_ms
                segment_chunks = 1
                buffered_audio.extend(chunk_to_pcm16(chunk))
            elif speaking and in_segment:
                segment_chunks += 1
                buffered_audio.extend(chunk_to_pcm16(chunk))
            elif not speaking and in_segment:
                end_ms = chunk_index * chunk_ms
                segments.append(
                    SpeechSegment(
                        start_ms=segment_start,
                        end_ms=end_ms,
                        chunks=segment_chunks,
                        audio_bytes=bytes(buffered_audio),
                        duration_ms=end_ms - segment_start,
                    )
                )
                in_segment = False
                segment_chunks = 0
                buffered_audio.clear()

            chunk_index += 1

        if in_segment:
            end_ms = chunk_index * chunk_ms
            segments.append(
                SpeechSegment(
                    start_ms=segment_start,
                    end_ms=end_ms,
                    chunks=segment_chunks,
                    audio_bytes=bytes(buffered_audio),
                    duration_ms=end_ms - segment_start,
                )
            )

        return segments


def chunk_to_pcm16(chunk: Sequence[float]) -> bytes:
    pcm = bytearray()
    for value in chunk:
        clipped = max(-1.0, min(1.0, float(value)))
        sample = int(clipped * 32767)
        pcm.extend(sample.to_bytes(2, byteorder="little", signed=True))
    return bytes(pcm)
