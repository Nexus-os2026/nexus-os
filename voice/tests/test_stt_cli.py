"""Track C #3 commit 1: CLI entrypoint tests for stt.py.

Covers the JSON contract that the Tauri subprocess bridge depends on:
  - python3 stt.py --health      → {"status": "ok" | "error", ...}
  - python3 stt.py transcribe X  → TranscriptionResult JSON

The transcribe path requires a real Whisper backend (faster-whisper
or a whisper-cli on PATH). Those tests skip when no backend is
available — the JSON-shape and CLI-error tests still run.
"""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from stt import FasterWhisperSTT  # noqa: E402

FIXTURES_DIR = Path(__file__).parent / "fixtures"
SILENCE_WAV = FIXTURES_DIR / "silence_500ms.wav"


def _has_whisper_backend() -> bool:
    """True iff faster-whisper or whisper-cli is available."""
    stt = FasterWhisperSTT()
    return stt._faster_whisper_model is not None or stt.whisper_command is not None


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(VOICE_DIR / "stt.py"), *args],
        cwd=VOICE_DIR,
        capture_output=True,
        text=True,
        timeout=120,
    )


class SttCliHealthTests(unittest.TestCase):
    def test_health_emits_json_to_stdout_or_stderr(self) -> None:
        result = _run_cli("--health")
        # Must emit ONE JSON object on stdout (success) or stderr (failure).
        # Exit code mirrors which side it landed on.
        if result.returncode == 0:
            payload = json.loads(result.stdout)
            self.assertEqual(payload["status"], "ok")
            self.assertIn("model", payload)
            self.assertIsInstance(payload["model"], str)
            self.assertEqual(result.stderr.strip(), "")
        else:
            payload = json.loads(result.stderr)
            self.assertEqual(payload["status"], "error")
            self.assertIn("reason", payload)


class SttCliErrorPathsTests(unittest.TestCase):
    def test_no_subcommand_returns_usage_error(self) -> None:
        result = _run_cli()
        self.assertEqual(result.returncode, 1)
        payload = json.loads(result.stderr)
        self.assertEqual(payload["status"], "error")
        self.assertIn("usage", payload["reason"])

    def test_unknown_subcommand_returns_error(self) -> None:
        result = _run_cli("frobnicate")
        self.assertEqual(result.returncode, 1)
        payload = json.loads(result.stderr)
        self.assertEqual(payload["status"], "error")
        self.assertIn("unknown subcommand", payload["reason"])

    def test_transcribe_without_path_returns_error(self) -> None:
        result = _run_cli("transcribe")
        self.assertEqual(result.returncode, 1)
        payload = json.loads(result.stderr)
        self.assertEqual(payload["status"], "error")
        self.assertIn("wav path", payload["reason"])

    def test_transcribe_missing_file_returns_missing_file_error(self) -> None:
        if not _has_whisper_backend():
            # The CLI's transcribe path requires a backend; the
            # FileNotFoundError branch only executes after backend init
            # succeeds. Skip when no backend so we don't conflate the
            # two error classes.
            self.skipTest("no whisper backend available")
        result = _run_cli("transcribe", "/nonexistent/path.wav")
        self.assertEqual(result.returncode, 1)
        payload = json.loads(result.stderr)
        self.assertEqual(payload["status"], "error")
        self.assertIn("missing_file", payload["reason"])


class SttCliTranscribeTests(unittest.TestCase):
    def setUp(self) -> None:
        if not SILENCE_WAV.exists():
            self.fail(f"fixture missing: {SILENCE_WAV}")
        if not _has_whisper_backend():
            self.skipTest("no whisper backend available")

    def test_transcribe_silence_returns_valid_json_shape(self) -> None:
        result = _run_cli("transcribe", str(SILENCE_WAV))
        self.assertEqual(
            result.returncode, 0, msg=f"stderr={result.stderr}"
        )
        payload = json.loads(result.stdout)
        # Required keys (TranscriptionResult dataclass).
        for key in (
            "text",
            "language",
            "confidence",
            "latency_ms",
            "model",
            "sentence_chunks",
        ):
            self.assertIn(key, payload, f"missing key: {key}")
        self.assertIsInstance(payload["text"], str)
        self.assertIsInstance(payload["latency_ms"], (int, float))
        self.assertIsInstance(payload["model"], str)
        self.assertIsInstance(payload["sentence_chunks"], list)


if __name__ == "__main__":
    unittest.main()
