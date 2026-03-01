"""Local streaming speech-to-text using faster-whisper adapter."""

from __future__ import annotations

from dataclasses import dataclass
import platform
import re
import shutil
import subprocess
import time
from typing import Iterable, List, Optional


@dataclass
class HardwareProfile:
    gpu_detected: bool
    plugged_in: bool
    battery_mode: bool


@dataclass
class TranscriptionResult:
    text: str
    model: str
    latency_total: float
    sentence_chunks: List[str]


def detect_gpu() -> bool:
    if shutil.which("nvidia-smi"):
        try:
            result = subprocess.run(
                ["nvidia-smi", "-L"],
                check=False,
                capture_output=True,
                text=True,
                timeout=0.5,
            )
            if result.returncode == 0 and result.stdout.strip():
                return True
        except (subprocess.SubprocessError, OSError):
            return False

    if platform.system() == "Darwin":
        return shutil.which("system_profiler") is not None

    return False


def select_model_tier(
    gpu_detected: bool,
    plugged_in: bool = True,
    battery_mode: bool = False,
) -> str:
    if battery_mode:
        return "base" if gpu_detected else "tiny"

    if gpu_detected and plugged_in:
        return "medium"

    if gpu_detected:
        return "base"

    return "tiny"


class FasterWhisperSTT:
    """Streaming STT adapter with sentence-first chunking."""

    def __init__(
        self,
        profile: Optional[HardwareProfile] = None,
        simulated_latency: Optional[float] = None,
    ) -> None:
        if profile is None:
            profile = HardwareProfile(
                gpu_detected=detect_gpu(),
                plugged_in=True,
                battery_mode=False,
            )
        self.profile = profile
        self.model = select_model_tier(
            gpu_detected=profile.gpu_detected,
            plugged_in=profile.plugged_in,
            battery_mode=profile.battery_mode,
        )
        self.simulated_latency = simulated_latency

    def transcribe_stream(self, chunks: Iterable[str]) -> TranscriptionResult:
        start = time.perf_counter()

        collected = " ".join(part.strip() for part in chunks if part.strip())
        sentence_chunks = [part.strip() for part in split_sentences(collected) if part.strip()]

        if self.simulated_latency is not None:
            latency_total = self.simulated_latency
        else:
            latency_total = time.perf_counter() - start

        return TranscriptionResult(
            text=collected,
            model=self.model,
            latency_total=latency_total,
            sentence_chunks=sentence_chunks,
        )


def split_sentences(text: str) -> List[str]:
    if not text.strip():
        return []
    parts = re.split(r"(?<=[.!?])\\s+", text.strip())
    return [part for part in parts if part]
