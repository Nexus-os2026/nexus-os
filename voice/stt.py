"""Local streaming speech-to-text using faster-whisper with safe fallbacks."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import time
from typing import Any, Iterable, List, Optional, Sequence


@dataclass
class HardwareProfile:
    gpu_detected: bool
    plugged_in: bool
    battery_mode: bool
    is_apple_silicon: bool = False


@dataclass
class TranscriptionResult:
    text: str
    language: str
    confidence: float
    latency_ms: float
    model: str
    sentence_chunks: List[str]

    @property
    def latency_total(self) -> float:
        # Backward compatibility for older callers/tests.
        return self.latency_ms / 1000.0


def detect_gpu() -> bool:
    try:
        import torch  # type: ignore

        if bool(torch.cuda.is_available()):
            return True
    except Exception:
        pass

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

    return False


def detect_apple_silicon() -> bool:
    if platform.system() != "Darwin":
        return False
    processor = platform.processor().lower()
    machine = platform.machine().lower()
    return "arm" in processor or machine in {"arm64", "aarch64"}


def select_model_tier(
    gpu_detected: bool,
    plugged_in: bool = True,
    battery_mode: bool = False,
    is_apple: bool = False,
) -> str:
    if battery_mode:
        return "tiny"
    if gpu_detected or is_apple:
        return "medium"
    if not plugged_in:
        return "tiny"
    return "tiny"


def list_whisper_models() -> List[str]:
    return ["tiny", "base", "small", "medium", "large-v3"]


class FasterWhisperSTT:
    """STT adapter that prefers faster-whisper and falls back to CLI backends."""

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
                is_apple_silicon=detect_apple_silicon(),
            )
        self.profile = profile
        self.model = whisper_model or select_model_tier(
            gpu_detected=profile.gpu_detected,
            plugged_in=profile.plugged_in,
            battery_mode=profile.battery_mode,
            is_apple=profile.is_apple_silicon,
        )
        self.simulated_latency = simulated_latency
        self.whisper_command = whisper_command or discover_whisper_command()
        self.whisper_model_path = whisper_model_path
        self.timeout_seconds = timeout_seconds
        self._faster_whisper_model: Any = None
        self._faster_whisper_error: Optional[str] = None
        self._init_faster_whisper()

    def _init_faster_whisper(self) -> None:
        try:
            from faster_whisper import WhisperModel  # type: ignore
        except Exception as error:
            self._faster_whisper_error = str(error)
            return

        device = "cuda" if self.profile.gpu_detected else "cpu"
        compute_type = "float16" if self.profile.gpu_detected else "int8"
        try:
            self._faster_whisper_model = WhisperModel(
                self.model,
                device=device,
                compute_type=compute_type,
            )
        except Exception as error:
            self._faster_whisper_error = str(error)
            self._faster_whisper_model = None

    def transcribe_stream(self, chunks: Iterable[str | bytes]) -> TranscriptionResult:
        start = time.perf_counter()
        collected_parts = [normalize_chunk(part) for part in chunks]
        collected = " ".join(part for part in collected_parts if part).strip()
        sentence_chunks = [part.strip() for part in split_sentences(collected) if part.strip()]

        if self.simulated_latency is not None:
            latency_ms = self.simulated_latency * 1000.0
        else:
            latency_ms = (time.perf_counter() - start) * 1000.0

        confidence = 1.0 if collected else 0.0
        return TranscriptionResult(
            text=collected,
            language="en",
            confidence=confidence,
            latency_ms=latency_ms,
            model=self.model,
            sentence_chunks=sentence_chunks,
        )

    def transcribe_audio_file(self, audio_path: str) -> TranscriptionResult:
        start = time.perf_counter()
        self._ensure_backend_available()
        audio_file = Path(audio_path)
        if not audio_file.exists():
            raise FileNotFoundError(f"audio file not found: {audio_file}")

        if self._faster_whisper_model is not None:
            text, language, confidence = self._run_faster_whisper(audio_file)
        elif self.whisper_command:
            backend = whisper_backend_kind(self.whisper_command)
            text = (
                self._run_whisper_cpp(self.whisper_command, audio_file)
                if backend == "whisper_cpp"
                else self._run_whisper_python_cli(self.whisper_command, audio_file)
            )
            language = "unknown"
            confidence = 0.0
        else:
            raise RuntimeError(
                "No Whisper backend found. Install faster-whisper or set NEXUS_WHISPER_CMD."
            )

        if self.simulated_latency is not None:
            latency_ms = self.simulated_latency * 1000.0
        else:
            latency_ms = (time.perf_counter() - start) * 1000.0
        return TranscriptionResult(
            text=text,
            language=language,
            confidence=confidence,
            latency_ms=latency_ms,
            model=self.model,
            sentence_chunks=split_sentences(text),
        )

    def _ensure_backend_available(self) -> None:
        if self._faster_whisper_model is not None or self.whisper_command:
            return
        details = (
            f" faster-whisper init error: {self._faster_whisper_error}"
            if self._faster_whisper_error
            else ""
        )
        raise RuntimeError(
            "No Whisper backend found. Install faster-whisper or set NEXUS_WHISPER_CMD."
            f"{details}"
        )

    def _run_faster_whisper(self, audio_file: Path) -> tuple[str, str, float]:
        assert self._faster_whisper_model is not None
        segments, info = self._faster_whisper_model.transcribe(str(audio_file))
        text_parts: List[str] = []
        confidence_samples: List[float] = []
        for segment in segments:
            segment_text = str(getattr(segment, "text", "")).strip()
            if segment_text:
                text_parts.append(segment_text)
            confidence_samples.append(segment_confidence(segment))
        average_confidence = sum(confidence_samples) / len(confidence_samples) if confidence_samples else 0.0
        language = str(getattr(info, "language", "unknown"))
        return (" ".join(text_parts).strip(), language, average_confidence)

    def _run_whisper_cpp(self, command: str, audio_file: Path) -> str:
        model_path = self.whisper_model_path or os_environ("NEXUS_WHISPER_MODEL_PATH")
        if not model_path:
            raise RuntimeError(
                "whisper-cli requires NEXUS_WHISPER_MODEL_PATH or whisper_model_path."
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
            subprocess.run(
                run,
                check=True,
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds,
            )
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
                self.model,
                "--output_format",
                "txt",
                "--output_dir",
                tmpdir,
            ]
            subprocess.run(
                run,
                check=True,
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds,
            )
            output_file = Path(tmpdir) / f"{audio_file.stem}.txt"
            if not output_file.exists():
                raise RuntimeError("whisper CLI did not produce transcript output")
            return output_file.read_text(encoding="utf-8").strip()


def normalize_chunk(chunk: str | bytes | Sequence[float]) -> str:
    if isinstance(chunk, bytes):
        return chunk.decode("utf-8", errors="ignore").strip()
    if isinstance(chunk, str):
        return chunk.strip()
    return ""


def segment_confidence(segment: Any) -> float:
    no_speech_prob = getattr(segment, "no_speech_prob", None)
    if isinstance(no_speech_prob, (int, float)):
        return max(0.0, min(1.0, 1.0 - float(no_speech_prob)))
    avg_logprob = getattr(segment, "avg_logprob", None)
    if isinstance(avg_logprob, (int, float)):
        # Soft map logprob (-inf..0) to (0..1).
        return max(0.0, min(1.0, float(avg_logprob) + 1.0))
    return 0.0


def split_sentences(text: str) -> List[str]:
    if not text.strip():
        return []
    parts = re.split(r"(?<=[.!?])\s+", text.strip())
    return [part for part in parts if part]


def discover_whisper_command() -> Optional[str]:
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
    import os

    return os.environ.get(key)
