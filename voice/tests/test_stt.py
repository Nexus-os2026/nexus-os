import unittest
from pathlib import Path
import sys

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from stt import select_model_tier


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


if __name__ == "__main__":
    unittest.main()
