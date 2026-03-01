import unittest
from pathlib import Path
import sys

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from wake_word import WakeWordDetector


class WakeWordTests(unittest.TestCase):
    def test_wake_word_detection(self) -> None:
        detector = WakeWordDetector("Hey NEXUS")

        positive = detector.detect("Please start now. Hey NEXUS, begin listening.")
        negative = detector.detect("Hello system, start listening")

        self.assertTrue(positive.detected)
        self.assertFalse(negative.detected)


if __name__ == "__main__":
    unittest.main()
