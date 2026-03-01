import unittest

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
