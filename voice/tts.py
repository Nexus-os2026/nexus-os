"""Local TTS wrapper for Piper-like streaming output."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import shutil
import subprocess
import tempfile
from typing import Generator


@dataclass
class TtsConfig:
    voice: str = "en_US-lessac-medium"
    speed: float = 1.0
    personality: str = "neutral"
    piper_command: str | None = None
    model_path: str | None = None
    speaker: int | None = None


class PiperTTS:
    def __init__(self, config: TtsConfig | None = None) -> None:
        self.config = config or TtsConfig()
        if self.config.piper_command is None:
            self.config.piper_command = discover_piper_command()

    def synthesize_stream(self, text: str) -> Generator[bytes, None, None]:
        if self.config.piper_command and self.config.model_path:
            with tempfile.NamedTemporaryFile(prefix="nexus-tts-", suffix=".wav", delete=False) as tmp:
                output = Path(tmp.name)
            try:
                self.synthesize_to_wav(text, output)
                with output.open("rb") as handle:
                    while True:
                        chunk = handle.read(4096)
                        if not chunk:
                            break
                        yield chunk
                return
            finally:
                output.unlink(missing_ok=True)

        # Predictable offline fallback keeps tests deterministic.
        for token in text.split():
            yield token.encode("utf-8")

    def synthesize_to_wav(self, text: str, output_path: str | Path) -> Path:
        command = self.config.piper_command
        model = self.config.model_path or os_environ("NEXUS_PIPER_MODEL_PATH")
        if not command:
            raise RuntimeError("Piper command not found. Set NEXUS_PIPER_CMD or install piper.")
        if not model:
            raise RuntimeError("Piper model path not set. Configure NEXUS_PIPER_MODEL_PATH.")

        output = Path(output_path)
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

        subprocess.run(
            run,
            check=True,
            capture_output=True,
            text=True,
            input=text,
            timeout=90.0,
        )
        if not output.exists():
            raise RuntimeError("Piper did not produce an output wav file")
        return output


def discover_piper_command() -> str | None:
    explicit = os_environ("NEXUS_PIPER_CMD")
    if explicit:
        return explicit
    return "piper" if shutil.which("piper") else None


def os_environ(key: str) -> str | None:
    import os

    return os.environ.get(key)
