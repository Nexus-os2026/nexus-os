import unittest

from jarvis import JarvisPipeline
from stt import FasterWhisperSTT, HardwareProfile


class JarvisTests(unittest.TestCase):
    def test_voice_confirmation(self) -> None:
        pipeline = JarvisPipeline()

        approved = pipeline.request_sensitive_confirmation(
            request_text="screen capture access",
            heard_phrase="confirm approve",
        )
        rejected = pipeline.request_sensitive_confirmation(
            request_text="screen capture access",
            heard_phrase="approve",
        )

        self.assertTrue(approved.approved)
        self.assertFalse(rejected.approved)
        self.assertIn("confirm approve", approved.readback_prompt.lower())

    def test_latency_degradation(self) -> None:
        stt = FasterWhisperSTT(
            profile=HardwareProfile(gpu_detected=False, plugged_in=False, battery_mode=True),
            simulated_latency=2.5,
        )
        pipeline = JarvisPipeline(stt=stt, latency_budget_seconds=2.0)

        _ = pipeline.run_once(["hey nexus", "start agent now"])

        self.assertFalse(pipeline.state.wake_word_enabled)
        self.assertTrue(pipeline.state.push_to_talk_enabled)


if __name__ == "__main__":
    unittest.main()
