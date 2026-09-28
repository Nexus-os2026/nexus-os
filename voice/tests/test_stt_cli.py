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
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Optional

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


def _run_cli(
    *args: str, env: Optional[dict[str, str]] = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(VOICE_DIR / "stt.py"), *args],
        cwd=VOICE_DIR,
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )


# The warning torch writes to stderr when its CUDA probe fails, as seen on a
# GPU host whose NVIDIA kernel module and user-space driver differ.
TORCH_CUDA_WARNING = (
    "CUDA initialization: Unexpected error from cudaGetDeviceCount(). Did you run "
    "some cuda functions before calling NumCudaDevices() that might have already set "
    "an error? Error 804: forward compatibility was attempted on non supported HW"
)

STUB_TORCH = f'''"""Stand-in for torch on a host whose CUDA driver is unusable."""
import warnings


class _Cuda:
    @staticmethod
    def is_available():
        warnings.warn({TORCH_CUDA_WARNING!r}, UserWarning)
        return False

    @staticmethod
    def device_count():
        warnings.warn({TORCH_CUDA_WARNING!r}, UserWarning)
        return 0


cuda = _Cuda()
'''


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


class SttCliGpuProbeContractTests(unittest.TestCase):
    """The CLI keeps its stderr contract where torch's CUDA probe warns.

    torch is not the Whisper backend; the voice stack installs it for VAD. A
    stand-in that warns exactly as torch does on a host with an unusable CUDA
    driver is placed first on the import path. If the CLI ever probes CUDA
    through torch, that warning reaches stderr and these tests fail.
    """

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="nexus-stub-torch-")
        package = Path(self._tmp.name) / "torch"
        package.mkdir()
        (package / "__init__.py").write_text(STUB_TORCH, encoding="utf-8")
        self.env = dict(os.environ)
        self.env["PYTHONPATH"] = os.pathsep.join(
            entry for entry in (self._tmp.name, self.env.get("PYTHONPATH", "")) if entry
        )

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_stand_in_torch_warns_like_torch(self) -> None:
        # Control: the stand-in shadows any installed torch in the CLI's
        # environment and writes the CUDA warning to stderr when probed.
        result = subprocess.run(
            [sys.executable, "-c", "import torch; torch.cuda.is_available()"],
            cwd=VOICE_DIR,
            env=self.env,
            capture_output=True,
            text=True,
            timeout=120,
        )
        self.assertEqual(result.returncode, 0, msg=result.stderr)
        self.assertIn("Error 804", result.stderr)

    def test_health_keeps_the_stderr_contract(self) -> None:
        result = _run_cli("--health", env=self.env)
        if result.returncode == 0:
            payload = json.loads(result.stdout)
            self.assertEqual(payload["status"], "ok")
            self.assertIsInstance(payload["model"], str)
            self.assertEqual(result.stderr.strip(), "")
        else:
            self.assertEqual(result.stdout, "")
            payload = json.loads(result.stderr)
            self.assertEqual(payload["status"], "error")
            self.assertIn("reason", payload)

    def test_transcribe_failure_is_one_json_object_on_stderr(self) -> None:
        # With or without a backend this fails (missing file, or no backend);
        # either way stderr must hold exactly one JSON error object.
        result = _run_cli("transcribe", "/nonexistent/path.wav", env=self.env)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        payload = json.loads(result.stderr)
        self.assertEqual(payload["status"], "error")
        self.assertIn("reason", payload)


if __name__ == "__main__":
    unittest.main()
