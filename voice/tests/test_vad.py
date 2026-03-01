import unittest
from pathlib import Path
import sys

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from vad import SileroVAD


class VadTests(unittest.TestCase):
    def test_vad_detects_speech(self) -> None:
        vad = SileroVAD(energy_threshold=0.01)

        speech_chunk = [0.2] * 160
        silence_chunk = [0.0] * 160
        segments = vad.segment_speech([silence_chunk, speech_chunk, speech_chunk, silence_chunk])
        self.assertGreater(len(segments), 0)

        empty = vad.segment_speech([silence_chunk, silence_chunk, silence_chunk])
        self.assertEqual(empty, [])


if __name__ == "__main__":
    unittest.main()
