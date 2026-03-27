"""Local Piper TTS wrapper with sentence streaming and offline fallback."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import io
import math
import re
import shutil
import subprocess
import tempfile
import wave
from typing import Generator, List


@dataclass
class TtsConfig:
    voice: str = "en_US-lessac-medium"
    speed: float = 1.0
    volume: float = 1.0
    piper_command: str | None = None
    model_path: str | None = None
    speaker: int | None = None
    sample_rate: int = 22050


class PiperTTS:
    def __init__(self, config: TtsConfig | None = None) -> None:
        self.config = config or TtsConfig()
        self._piper_disabled = False  # Set True after first failure — no retry loop
        if self.config.piper_command is None:
            self.config.piper_command = discover_piper_command()
        if self.config.model_path is None:
            self.config.model_path = discover_piper_model_path()
        # Validate piper is actually usable at init time
        if self.config.piper_command and not shutil.which(self.config.piper_command):
            import sys
            print(
                f"[tts] WARNING: piper command '{self.config.piper_command}' not found in PATH "
                f"— disabling piper TTS, using silent fallback",
                file=sys.stderr,
            )
            self._piper_disabled = True

    def synthesize(self, text: str) -> bytes:
        if self.config.piper_command and self.config.model_path and not self._piper_disabled:
            with tempfile.NamedTemporaryFile(prefix="nexus-tts-", suffix=".wav", delete=False) as tmp:
                output = Path(tmp.name)
            try:
                self.synthesize_to_wav(text, output)
                return output.read_bytes()
            except (FileNotFoundError, subprocess.CalledProcessError, OSError, RuntimeError) as exc:
                # Piper failed — disable for the rest of this session to prevent crash loop
                import sys
                print(f"[tts] piper synthesis failed: {exc} — disabling for session", file=sys.stderr)
                self._piper_disabled = True
                output.unlink(missing_ok=True)
            except Exception:
                output.unlink(missing_ok=True)
        return fallback_wav_bytes(text, sample_rate=self.config.sample_rate, volume=self.config.volume)

    def synthesize_stream(self, text: str) -> Generator[bytes, None, None]:
        for sentence in split_sentences(text):
            if sentence.strip():
                yield self.synthesize(sentence.strip())

    def synthesize_to_wav(self, text: str, output_path: str | Path) -> Path:
        output = Path(output_path)
        command = self.config.piper_command
        model = self.config.model_path
        if command and model and not self._piper_disabled:
            run = [
                command,
                "--model",
                model,
                "--output_file",
                str(output),
                "--length_scale",
                str(max(0.1, 2.0 - self.config.speed)),
            ]
            if self.config.speaker is not None:
                run.extend(["--speaker", str(self.config.speaker)])
            try:
                subprocess.run(
                    run,
                    check=True,
                    capture_output=True,
                    text=True,
                    input=text,
                    timeout=90.0,
                )
            except (FileNotFoundError, subprocess.CalledProcessError, OSError) as exc:
                import sys
                print(f"[tts] piper execution failed: {exc} — disabling for session", file=sys.stderr)
                self._piper_disabled = True
                # Fall through to fallback below
            else:
                if not output.exists():
                    raise RuntimeError("Piper did not produce an output wav file")
                return output

        output.write_bytes(
            fallback_wav_bytes(text, sample_rate=self.config.sample_rate, volume=self.config.volume)
        )
        return output


def split_sentences(text: str) -> List[str]:
    return [part for part in re.split(r"(?<=[.!?])\s+", text.strip()) if part]


def fallback_wav_bytes(text: str, sample_rate: int = 22050, volume: float = 1.0) -> bytes:
    # Generate a deterministic short tone if Piper is unavailable in local/offline CI.
    duration_seconds = max(0.2, min(1.5, len(text) * 0.02))
    frames = int(sample_rate * duration_seconds)
    amplitude = max(0.05, min(1.0, volume)) * 32767.0
    frequency = 440.0

    with io.BytesIO() as buffer:
        with wave.open(buffer, "wb") as writer:
            writer.setnchannels(1)
            writer.setsampwidth(2)
            writer.setframerate(sample_rate)
            pcm = bytearray()
            for index in range(frames):
                sample = int(amplitude * math.sin((2 * math.pi * frequency * index) / sample_rate))
                pcm.extend(sample.to_bytes(2, byteorder="little", signed=True))
            writer.writeframes(bytes(pcm))
        return buffer.getvalue()


def discover_piper_command() -> str | None:
    explicit = os_environ("NEXUS_PIPER_CMD")
    if explicit:
        return explicit
    return "piper" if shutil.which("piper") else None


def discover_piper_model_path() -> str | None:
    explicit = os_environ("NEXUS_PIPER_MODEL_PATH")
    if explicit:
        return explicit
    default_model = Path.home() / ".nexus" / "voice" / "piper" / "en_US-lessac-medium.onnx"
    if default_model.exists():
        return str(default_model)
    return None


def os_environ(key: str) -> str | None:
    import os

    return os.environ.get(key)
