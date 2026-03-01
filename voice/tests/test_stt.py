import unittest

from stt import select_model_tier


class SttTests(unittest.TestCase):
    def test_hardware_tier_selection(self) -> None:
        with_gpu = select_model_tier(gpu_detected=True, plugged_in=True, battery_mode=False)
        without_gpu = select_model_tier(gpu_detected=False, plugged_in=False, battery_mode=False)

        self.assertEqual(with_gpu, "medium")
        self.assertEqual(without_gpu, "tiny")


if __name__ == "__main__":
    unittest.main()
