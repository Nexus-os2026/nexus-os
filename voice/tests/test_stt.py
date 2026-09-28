import importlib.abc
import subprocess
import types
import unittest
from pathlib import Path
import sys
from unittest.mock import patch

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from stt import detect_gpu, select_model_tier


class SttTests(unittest.TestCase):
    def test_whisper_model_selection(self) -> None:
        with_gpu = select_model_tier(
            gpu_detected=True,
            plugged_in=True,
            battery_mode=False,
            is_apple=False,
        )
        without_gpu = select_model_tier(
            gpu_detected=False,
            plugged_in=True,
            battery_mode=False,
            is_apple=False,
        )

        self.assertEqual(with_gpu, "medium")
        self.assertEqual(without_gpu, "tiny")


class _ImportRecorder(importlib.abc.MetaPathFinder):
    """Records every module name the import system is asked to find."""

    def __init__(self) -> None:
        self.names: list[str] = []

    def find_spec(self, fullname, path=None, target=None):  # type: ignore[no-untyped-def]
        self.names.append(fullname)
        return None


def _ctranslate2_stub(count_or_error: "int | Exception") -> types.ModuleType:
    module = types.ModuleType("ctranslate2")

    def get_cuda_device_count() -> int:
        if isinstance(count_or_error, Exception):
            raise count_or_error
        return count_or_error

    module.get_cuda_device_count = get_cuda_device_count  # type: ignore[attr-defined]
    return module


class DetectGpuTests(unittest.TestCase):
    """detect_gpu asks CTranslate2, which runs the model, and never torch."""

    def _detect(self, ctranslate2_module, which=None, run=None):  # type: ignore[no-untyped-def]
        recorder = _ImportRecorder()
        with patch.dict(sys.modules):
            # Any import of torch below must go through the import system.
            for name in [n for n in sys.modules if n == "torch" or n.startswith("torch.")]:
                del sys.modules[name]
            # None makes `import ctranslate2` fail, as when it is not installed.
            sys.modules["ctranslate2"] = ctranslate2_module
            with patch.object(sys, "meta_path", [recorder, *sys.meta_path]), patch(
                "stt.shutil.which", return_value=which
            ) as which_mock, patch("stt.subprocess.run", side_effect=run) as run_mock:
                result = detect_gpu()
        self.assertNotIn("torch", recorder.names)
        return result, which_mock, run_mock

    def test_ctranslate2_device_count_decides(self) -> None:
        for count, expected in ((2, True), (1, True), (0, False)):
            with self.subTest(count=count):
                result, which_mock, run_mock = self._detect(_ctranslate2_stub(count))
                self.assertIs(result, expected)
                which_mock.assert_not_called()
                run_mock.assert_not_called()

    def test_failed_ctranslate2_probe_means_cpu(self) -> None:
        error = RuntimeError(
            "CUDA failed with error forward compatibility was attempted on non supported HW"
        )
        result, which_mock, run_mock = self._detect(_ctranslate2_stub(error))
        self.assertIs(result, False)
        which_mock.assert_not_called()
        run_mock.assert_not_called()

    def test_without_ctranslate2_nvidia_smi_is_only_a_tier_hint(self) -> None:
        def listed(*_args, **_kwargs):  # type: ignore[no-untyped-def]
            return subprocess.CompletedProcess(["nvidia-smi", "-L"], 0, "GPU 0: Example\n", "")

        def mismatch(*_args, **_kwargs):  # type: ignore[no-untyped-def]
            return subprocess.CompletedProcess(
                ["nvidia-smi", "-L"], 18, "", "Driver/library version mismatch\n"
            )

        result, _, run_mock = self._detect(None, which="/usr/bin/nvidia-smi", run=listed)
        self.assertIs(result, True)
        self.assertEqual(run_mock.call_args.args[0], ["nvidia-smi", "-L"])

        result, _, _ = self._detect(None, which="/usr/bin/nvidia-smi", run=mismatch)
        self.assertIs(result, False)

        result, _, run_mock = self._detect(None, which=None)
        self.assertIs(result, False)
        run_mock.assert_not_called()


if __name__ == "__main__":
    unittest.main()
