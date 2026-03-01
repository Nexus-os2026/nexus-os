"""Local streaming speech-to-text using faster-whisper adapter."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
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
        whisper_command: Optional[str] = None,
        whisper_model: Optional[str] = None,
        whisper_model_path: Optional[str] = None,
        timeout_seconds: float = 90.0,
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
        self.whisper_command = whisper_command or discover_whisper_command()
        self.whisper_model = whisper_model or self.model
        self.whisper_model_path = whisper_model_path
        self.timeout_seconds = timeout_seconds

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

    def transcribe_audio_file(self, audio_path: str) -> TranscriptionResult:
        """Transcribe an audio file using a real Whisper backend if available."""
        start = time.perf_counter()
        command = self.whisper_command
        if not command:
            raise RuntimeError(
                "No Whisper backend found. Set NEXUS_WHISPER_CMD or install whisper-cli/whisper."
            )

        backend = whisper_backend_kind(command)
        audio_file = Path(audio_path)
        if not audio_file.exists():
            raise FileNotFoundError(f"audio file not found: {audio_file}")

        text = (
            self._run_whisper_cpp(command, audio_file)
            if backend == "whisper_cpp"
            else self._run_whisper_python_cli(command, audio_file)
        )

        latency_total = time.perf_counter() - start
        return TranscriptionResult(
            text=text,
            model=self.whisper_model,
            latency_total=latency_total,
            sentence_chunks=split_sentences(text),
        )

    def _run_whisper_cpp(self, command: str, audio_file: Path) -> str:
        model_path = self.whisper_model_path or os_environ("NEXUS_WHISPER_MODEL_PATH")
        if not model_path:
            raise RuntimeError(
                "whisper-cli requires a model path via NEXUS_WHISPER_MODEL_PATH "
                "or FasterWhisperSTT(whisper_model_path=...)."
            )

        with tempfile.TemporaryDirectory(prefix="nexus-whisper-") as tmpdir:
            output_prefix = str(Path(tmpdir) / "transcript")
            run = [
                command,
                "-m",
                model_path,
                "-f",
                str(audio_file),
                "-otxt",
                "-of",
                output_prefix,
            ]
            subprocess.run(run, check=True, capture_output=True, text=True, timeout=self.timeout_seconds)
            output_file = Path(f"{output_prefix}.txt")
            if not output_file.exists():
                raise RuntimeError("whisper-cli did not produce transcript output")
            return output_file.read_text(encoding="utf-8").strip()

    def _run_whisper_python_cli(self, command: str, audio_file: Path) -> str:
        with tempfile.TemporaryDirectory(prefix="nexus-whisper-") as tmpdir:
            run = [
                command,
                str(audio_file),
                "--model",
                self.whisper_model,
                "--output_format",
                "txt",
                "--output_dir",
                tmpdir,
            ]
            subprocess.run(run, check=True, capture_output=True, text=True, timeout=self.timeout_seconds)
            output_file = Path(tmpdir) / f"{audio_file.stem}.txt"
            if not output_file.exists():
                raise RuntimeError("whisper CLI did not produce transcript output")
            return output_file.read_text(encoding="utf-8").strip()


def split_sentences(text: str) -> List[str]:
    if not text.strip():
        return []
    parts = re.split(r"(?<=[.!?])\\s+", text.strip())
    return [part for part in parts if part]


def discover_whisper_command() -> Optional[str]:
    """Resolve a Whisper command from env override or local PATH."""
    explicit = os_environ("NEXUS_WHISPER_CMD")
    if explicit:
        return explicit

    for candidate in ("whisper-cli", "whisper"):
        if shutil.which(candidate):
            return candidate
    return None


def whisper_backend_kind(command: str) -> str:
    executable = Path(command).name
    if executable == "whisper-cli":
        return "whisper_cpp"
    return "whisper_python"


def os_environ(key: str) -> Optional[str]:
    # Localized indirection keeps environment access easy to patch in tests.
    import os

    return os.environ.get(key)
