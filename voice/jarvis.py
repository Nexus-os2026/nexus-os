"""Jarvis mode orchestration loop for fully local voice UX."""

from __future__ import annotations

from dataclasses import dataclass, field
from hashlib import sha256
import time
from typing import Dict, List, Optional

from stt import FasterWhisperSTT
from tts import PiperTTS
from vad import SileroVAD
from wake_word import WakeWordDetector


CONFIRMATION_PHRASE = "confirm approve"


@dataclass
class OverlayState:
    visible: bool = False
    listening: bool = False
    transcription: str = ""
    response_text: str = ""


@dataclass
class JarvisState:
    wake_word_enabled: bool = True
    push_to_talk_enabled: bool = True
    overlay: OverlayState = field(default_factory=OverlayState)


@dataclass
class ConfirmationResult:
    approved: bool
    required_phrase: str
    heard_phrase: str
    readback_prompt: str


@dataclass
class JarvisResult:
    transcription: str
    response_text: str
    latency_total: float


class InMemoryAuditTrail:
    def __init__(self) -> None:
        self.events: List[Dict[str, object]] = []

    def log(self, event_type: str, payload: Dict[str, object]) -> None:
        digest = sha256(str(payload).encode("utf-8")).hexdigest()
        self.events.append(
            {
                "timestamp": time.time(),
                "event_type": event_type,
                "payload": payload,
                "payload_hash": digest,
            }
        )


class JarvisPipeline:
    def __init__(
        self,
        wake_word: Optional[WakeWordDetector] = None,
        vad: Optional[SileroVAD] = None,
        stt: Optional[FasterWhisperSTT] = None,
        tts: Optional[PiperTTS] = None,
        latency_budget_seconds: float = 2.0,
    ) -> None:
        self.wake_word = wake_word or WakeWordDetector()
        self.vad = vad or SileroVAD()
        self.stt = stt or FasterWhisperSTT()
        self.tts = tts or PiperTTS()
        self.latency_budget_seconds = latency_budget_seconds
        self.state = JarvisState()
        self.audit = InMemoryAuditTrail()

    def activate_overlay(self) -> None:
        self.state.overlay.visible = True
        self.state.overlay.listening = True

    def deactivate_overlay(self) -> None:
        self.state.overlay.visible = False
        self.state.overlay.listening = False

    def handle_goodbye_phrase(self, transcript: str) -> bool:
        if transcript.strip().lower() == "goodbye nexus":
            self.deactivate_overlay()
            return True
        return False

    def request_sensitive_confirmation(self, request_text: str, heard_phrase: str) -> ConfirmationResult:
        prompt = (
            f"This agent wants {request_text}. Say '{CONFIRMATION_PHRASE}' to proceed"
        )
        approved = heard_phrase.strip().lower() == CONFIRMATION_PHRASE

        self.audit.log(
            "voice_confirmation",
            {
                "request": request_text,
                "heard_phrase": heard_phrase,
                "approved": approved,
            },
        )

        return ConfirmationResult(
            approved=approved,
            required_phrase=CONFIRMATION_PHRASE,
            heard_phrase=heard_phrase,
            readback_prompt=prompt,
        )

    def handle_latency(self, latency_total: float) -> None:
        if latency_total > self.latency_budget_seconds:
            self.state.wake_word_enabled = False
            self.state.push_to_talk_enabled = True
            self.audit.log(
                "latency_degradation",
                {
                    "latency_total": latency_total,
                    "wake_word_enabled": self.state.wake_word_enabled,
                    "push_to_talk_enabled": self.state.push_to_talk_enabled,
                },
            )

    def run_once(self, transcript_chunks: List[str]) -> JarvisResult:
        self.activate_overlay()

        stt_result = self.stt.transcribe_stream(transcript_chunks)
        self.handle_latency(stt_result.latency_total)

        transcription = stt_result.text
        self.state.overlay.transcription = transcription
        self.audit.log(
            "transcription",
            {
                "text": transcription,
                "model": stt_result.model,
                "latency_total": stt_result.latency_total,
            },
        )

        response_text = f"Acknowledged: {transcription}"
        self.state.overlay.response_text = response_text

        # Stream response audio locally (payload ignored by caller in this scaffold).
        for _ in self.tts.synthesize_stream(response_text):
            pass

        return JarvisResult(
            transcription=transcription,
            response_text=response_text,
            latency_total=stt_result.latency_total,
        )
