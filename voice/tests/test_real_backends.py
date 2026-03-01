import tempfile
import unittest
import wave
from pathlib import Path
import sys
from unittest.mock import patch

VOICE_DIR = Path(__file__).resolve().parents[1]
if str(VOICE_DIR) not in sys.path:
    sys.path.insert(0, str(VOICE_DIR))

from stt import FasterWhisperSTT, discover_whisper_command, whisper_backend_kind
from tts import PiperTTS, TtsConfig, discover_piper_command


class RealBackendTests(unittest.TestCase):
    def _write_valid_pcm_wav(self, path: Path) -> None:
        sample_rate = 16_000
        duration_seconds = 1
        total_frames = sample_rate * duration_seconds
        silence_frame = (0).to_bytes(2, byteorder="little", signed=True)

        with wave.open(str(path), "wb") as wav_file:
            wav_file.setnchannels(1)
            wav_file.setsampwidth(2)
            wav_file.setframerate(sample_rate)
            wav_file.writeframes(silence_frame * total_frames)

    def test_whisper_backend_kind(self) -> None:
        self.assertEqual(whisper_backend_kind("whisper-cli"), "whisper_cpp")
        self.assertEqual(whisper_backend_kind("/usr/local/bin/whisper"), "whisper_python")

    @patch.dict("os.environ", {"NEXUS_WHISPER_CMD": "/opt/bin/whisper-cli"}, clear=False)
    def test_discover_whisper_command_env_override(self) -> None:
        self.assertEqual(discover_whisper_command(), "/opt/bin/whisper-cli")

    @patch.dict("os.environ", {"NEXUS_PIPER_CMD": "/opt/bin/piper"}, clear=False)
    def test_discover_piper_command_env_override(self) -> None:
        self.assertEqual(discover_piper_command(), "/opt/bin/piper")

    @patch("stt.discover_whisper_command", return_value=None)
    def test_transcribe_audio_file_requires_backend(self, _discover_mock) -> None:
        stt = FasterWhisperSTT()
        stt._faster_whisper_model = None
        stt.whisper_command = None

        with tempfile.TemporaryDirectory() as temp_dir:
            audio_path = Path(temp_dir) / "silence.wav"
            self._write_valid_pcm_wav(audio_path)
            with self.assertRaises(RuntimeError):
                stt.transcribe_audio_file(str(audio_path))

    def test_synthesize_stream_falls_back_when_piper_unavailable(self) -> None:
        tts = PiperTTS(TtsConfig(piper_command=None, model_path=None))
        chunks = list(tts.synthesize_stream("hello nexus. test"))
        self.assertGreaterEqual(len(chunks), 1)
        self.assertTrue(all(len(chunk) > 0 for chunk in chunks))

    @patch("tts.subprocess.run")
    def test_synthesize_to_wav_executes_command(self, run_mock) -> None:
        tts = PiperTTS(
            TtsConfig(
                piper_command="piper",
                model_path="/models/en_US-lessac-medium.onnx",
                speed=1.0,
                speaker=2,
            )
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            output = Path(tmpdir) / "sample.wav"

            def _fake_run(*args, **kwargs):
                output.write_bytes(b"RIFF....WAVE")
                return None

            run_mock.side_effect = _fake_run
            result = tts.synthesize_to_wav("hello", output)

        self.assertEqual(result.name, "sample.wav")
        called = run_mock.call_args[0][0]
        self.assertIn("--model", called)
        self.assertIn("--output_file", called)
        self.assertIn("--speaker", called)


if __name__ == "__main__":
    unittest.main()
