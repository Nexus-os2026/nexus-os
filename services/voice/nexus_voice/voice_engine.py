"""Core voice processing engine for Nexus OS.

Handles wake word detection, speech-to-text transcription, and text-to-speech
synthesis. Designed to run locally without cloud dependencies.
"""

from __future__ import annotations

import hashlib
import struct
import time
from typing import Optional


class VoiceEngine:
    """Local-first voice processing engine.

    Transcription is currently stubbed — real transcription will route through
    the LLM gateway (local SLM or cloud provider).
    """

    def __init__(
        self,
        model_path: Optional[str] = None,
        wake_word: str = "nexus",
    ) -> None:
        self.wake_word = wake_word.lower()
        self.is_listening = False
        self.sample_rate = 16000
        self.model_path = model_path
        self._model_loaded = model_path is not None
        self._last_transcript: str = ""

    # ── Listening lifecycle ───────────────────────────────────────────

    def start_listening(self) -> None:
        """Begin continuous audio capture from microphone."""
        self.is_listening = True

    def stop_listening(self) -> None:
        """Stop audio capture."""
        self.is_listening = False

    # ── Wake word detection ──────────────────────────────────────────

    def detect_wake_word(self, audio_chunk: bytes) -> bool:
        """Check if audio contains the wake word.

        Uses a simple energy-threshold heuristic: if the audio chunk has
        sufficient energy (non-silence) we treat it as a potential wake-word
        trigger.  A production implementation would use a lightweight keyword
        spotting model (e.g. OpenWakeWord or Porcupine).
        """
        if len(audio_chunk) < 2:
            return False

        # Interpret raw bytes as 16-bit signed PCM samples.
        n_samples = len(audio_chunk) // 2
        if n_samples == 0:
            return False

        samples = struct.unpack(f"<{n_samples}h", audio_chunk[: n_samples * 2])
        rms = (sum(s * s for s in samples) / n_samples) ** 0.5

        # Energy threshold — wake word requires audible speech.
        energy_threshold = 500.0

        # Deterministic stand-in: hash the chunk and check the lowest byte
        # against the wake word length so tests are reproducible.
        chunk_hash = hashlib.sha256(audio_chunk).digest()
        hash_trigger = chunk_hash[0] < 32  # ~12.5 % probability

        return rms > energy_threshold and hash_trigger

    # ── Transcription ────────────────────────────────────────────────

    def transcribe(self, audio_data: bytes) -> str:
        """Convert speech to text.

        Currently returns a placeholder.  Real transcription will be handled
        by the LLM gateway (local Whisper model via Candle or cloud STT).
        """
        if not audio_data:
            return ""

        # Stub: return a deterministic placeholder so callers can integrate.
        size_kb = len(audio_data) / 1024
        self._last_transcript = (
            f"[transcription placeholder — {size_kb:.1f} KB audio received]"
        )
        return self._last_transcript

    # ── Synthesis ────────────────────────────────────────────────────

    def synthesize(self, text: str) -> bytes:
        """Convert text to speech audio.

        TODO: Integrate a local TTS engine (e.g. Piper, Coqui TTS) or route
        through the LLM gateway for cloud TTS.
        """
        # Return empty bytes — the frontend can fall back to browser TTS.
        _ = text
        return b""

    # ── Status ───────────────────────────────────────────────────────

    def get_status(self) -> dict:
        """Return current engine status."""
        return {
            "is_listening": self.is_listening,
            "wake_word": self.wake_word,
            "sample_rate": self.sample_rate,
            "model_loaded": self._model_loaded,
            "last_transcript": self._last_transcript,
            "timestamp": int(time.time()),
        }
