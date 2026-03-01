"""Jarvis mode orchestration loop for fully local voice UX."""

from __future__ import annotations

from dataclasses import dataclass, field
from hashlib import sha256
import argparse
import time
from typing import Callable, Dict, Iterable, List, Optional, Sequence

from stt import FasterWhisperSTT, list_whisper_models
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
    latency_ms: float

    @property
    def latency_total(self) -> float:
        # Backward compatibility for existing tests.
        return self.latency_ms / 1000.0


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


def default_llm_response(prompt: str) -> str:
    if not prompt.strip():
        return "I did not catch that. Please try again."
    return f"Acknowledged: {prompt}"


class JarvisPipeline:
    def __init__(
        self,
        wake_word: Optional[WakeWordDetector] = None,
        vad: Optional[SileroVAD] = None,
        stt: Optional[FasterWhisperSTT] = None,
        tts: Optional[PiperTTS] = None,
        llm_responder: Optional[Callable[[str], str]] = None,
        latency_budget_seconds: Optional[float] = None,
    ) -> None:
        self.wake_word = wake_word or WakeWordDetector()
        self.vad = vad or SileroVAD()
        self.stt = stt or FasterWhisperSTT()
        self.tts = tts or PiperTTS()
        if latency_budget_seconds is None:
            latency_budget_seconds = 2.0 if self.stt.profile.gpu_detected else 4.0
        self.latency_budget_seconds = latency_budget_seconds
        self.state = JarvisState()
        self.audit = InMemoryAuditTrail()
        self.llm_responder = llm_responder or default_llm_response
        self.interaction_latencies_ms: List[float] = []

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

    def is_confirmation_approved(self, heard_phrase: str) -> bool:
        return heard_phrase.strip().lower() == CONFIRMATION_PHRASE

    def request_sensitive_confirmation(self, request_text: str, heard_phrase: str) -> ConfirmationResult:
        prompt = (
            f"Agent wants {request_text}. Say '{CONFIRMATION_PHRASE}' to proceed."
        )
        approved = self.is_confirmation_approved(heard_phrase)
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

    def handle_latency(self, latency_ms: float) -> None:
        budget_ms = self.latency_budget_seconds * 1000.0
        if latency_ms > budget_ms:
            self.state.wake_word_enabled = False
            self.state.push_to_talk_enabled = True
            self.audit.log(
                "latency_degradation",
                {
                    "latency_ms": latency_ms,
                    "budget_ms": budget_ms,
                    "wake_word_enabled": self.state.wake_word_enabled,
                    "push_to_talk_enabled": self.state.push_to_talk_enabled,
                },
            )

    def run_once(self, transcript_chunks: List[str | bytes]) -> JarvisResult:
        self.activate_overlay()
        stt_result = self.stt.transcribe_stream(transcript_chunks)
        transcription = stt_result.text.strip()
        self.state.overlay.transcription = transcription
        self.audit.log(
            "transcription",
            {
                "text": transcription,
                "language": stt_result.language,
                "confidence": stt_result.confidence,
                "model": stt_result.model,
                "latency_ms": stt_result.latency_ms,
            },
        )

        response_text = self.llm_responder(transcription)
        self.state.overlay.response_text = response_text

        tts_start = time.perf_counter()
        audio_started_at: Optional[float] = None
        for _audio_chunk in self.tts.synthesize_stream(response_text):
            if audio_started_at is None:
                audio_started_at = time.perf_counter()
        if audio_started_at is None:
            audio_started_at = time.perf_counter()

        tts_start_latency_ms = max(0.0, (audio_started_at - tts_start) * 1000.0)
        total_latency_ms = stt_result.latency_ms + tts_start_latency_ms
        self.interaction_latencies_ms.append(total_latency_ms)
        self.handle_latency(total_latency_ms)
        self.audit.log(
            "response",
            {
                "response_text": response_text,
                "total_latency_ms": total_latency_ms,
            },
        )

        return JarvisResult(
            transcription=transcription,
            response_text=response_text,
            latency_ms=total_latency_ms,
        )

    def run_vad_cycle(self, audio_chunks: Iterable[Sequence[float]]) -> Optional[JarvisResult]:
        segments = self.vad.segment_speech(audio_chunks)
        if not segments:
            return None
        # For local integration scaffolding we map VAD segments to transcript chunks.
        transcript_chunks = [f"speech segment {segment.duration_ms}ms" for segment in segments]
        return self.run_once(transcript_chunks)

    def start_foreground(self) -> None:
        print("Jarvis voice pipeline started. Type text to simulate voice input, Ctrl+C to stop.")
        while True:
            try:
                line = input("> ").strip()
            except (EOFError, KeyboardInterrupt):
                print("\nStopping Jarvis.")
                break
            if not line:
                continue
            if self.handle_goodbye_phrase(line):
                print("Goodbye.")
                break
            wake = self.wake_word.detect(line)
            if wake.detected:
                print("Wake word detected.")
            result = self.run_once([line])
            print(f"User: {result.transcription}")
            print(f"NEXUS: {result.response_text}")


def cmd_start() -> int:
    JarvisPipeline().start_foreground()
    return 0


def cmd_test() -> int:
    pipeline = JarvisPipeline()
    result = pipeline.run_once(["hello nexus this is a voice test"])
    print(f"transcription={result.transcription}")
    print(f"response={result.response_text}")
    print(f"latency_ms={result.latency_ms:.2f}")
    return 0


def cmd_models() -> int:
    stt = FasterWhisperSTT()
    print(f"available_models={', '.join(list_whisper_models())}")
    print(f"selected_model={stt.model}")
    return 0


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="jarvis")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("start", help="Start the local Jarvis loop")
    subparsers.add_parser("test", help="Run a short local voice self-test")
    subparsers.add_parser("models", help="List Whisper models and current selection")
    args = parser.parse_args(argv)

    if args.command == "start":
        return cmd_start()
    if args.command == "test":
        return cmd_test()
    if args.command == "models":
        return cmd_models()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
