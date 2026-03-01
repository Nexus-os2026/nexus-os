import unittest

from tts import PiperTTS, TtsConfig


class TtsTests(unittest.TestCase):
    def test_tts_synthesis(self) -> None:
        tts = PiperTTS(TtsConfig(piper_command=None, model_path=None))
        audio = tts.synthesize("Hello World")
        self.assertGreater(len(audio), 0)


if __name__ == "__main__":
    unittest.main()
