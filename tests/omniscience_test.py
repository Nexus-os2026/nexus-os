"""Tests for System 11 — Computer Omniscience.

These tests verify screen understanding, intent prediction, action execution,
kill switch, and privacy compliance.
"""

import subprocess
import sys

CARGO = ["cargo", "test", "-p", "nexus-kernel", "--lib"]


def run_rust_test(test_name: str) -> bool:
    result = subprocess.run(
        [*CARGO, "--", test_name], capture_output=True, text=True, cwd=".."
    )
    if result.returncode != 0:
        print(f"  FAIL: {test_name}")
        print(result.stdout[-500:] if len(result.stdout) > 500 else result.stdout)
        print(result.stderr[-500:] if len(result.stderr) > 500 else result.stderr)
    return result.returncode == 0


def test_screen_text_extraction():
    """Test 1: Capture screen -> verify text extraction works."""
    print("[Test 1] Screen understanding...")
    assert run_rust_test("omniscience::screen::tests"), "Screen tests failed"
    print("  PASS")


def test_intent_prediction():
    """Test 2: Simulate coding session -> verify intent prediction reasonable."""
    print("[Test 2] Intent prediction...")
    assert run_rust_test("omniscience::intent::tests"), "Intent prediction tests failed"
    print("  PASS")


def test_action_execution():
    """Test 3: Execute typing action -> verify text appears -> verify kill switch."""
    print("[Test 3] Action execution and kill switch...")
    assert run_rust_test("omniscience::executor::tests"), "Executor tests failed"
    print("  PASS")


def test_privacy_check():
    """Test 4: Privacy check -> verify no screen data leaves the machine."""
    print("[Test 4] Privacy compliance...")
    # The assistant module has privacy checks
    assert run_rust_test("omniscience::assistant::tests"), "Privacy tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 11 — Computer Omniscience Tests")
    print("=" * 60)

    tests = [
        test_screen_text_extraction,
        test_intent_prediction,
        test_action_execution,
        test_privacy_check,
    ]

    passed = 0
    failed = 0
    for test in tests:
        try:
            test()
            passed += 1
        except (AssertionError, Exception) as e:
            failed += 1
            print(f"  FAIL: {e}")

    print(f"\nResults: {passed}/{len(tests)} passed, {failed} failed")
    sys.exit(0 if failed == 0 else 1)
