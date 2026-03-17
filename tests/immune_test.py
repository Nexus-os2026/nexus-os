"""Tests for System 5 — Immune System.

These tests verify threat detection, privacy scanning, antibody spawning,
immune memory, adversarial arena, and hive immunity.
"""

import json
import subprocess
import sys

CARGO = ["cargo", "test", "-p", "nexus-kernel", "--lib"]


def run_rust_test(test_name: str) -> bool:
    """Run a single Rust unit test and return True if it passes."""
    result = subprocess.run(
        [*CARGO, "--", test_name],
        capture_output=True,
        text=True,
        cwd="..",
    )
    passed = result.returncode == 0
    if not passed:
        print(f"  FAIL: {test_name}")
        print(result.stdout[-500:] if len(result.stdout) > 500 else result.stdout)
        print(result.stderr[-500:] if len(result.stderr) > 500 else result.stderr)
    return passed


def test_prompt_injection_detection():
    """Test 1: Inject a prompt injection -> verify detection and blocking."""
    print("[Test 1] Prompt injection detection...")
    # The ThreatDetector in immune/detector.rs should detect injection patterns
    # from firewall::patterns. We test via the Rust unit tests.
    assert run_rust_test("immune::detector::tests"), "Prompt injection detection failed"
    print("  PASS")


def test_data_exfiltration_detection():
    """Test 2: Simulate data exfiltration attempt -> verify privacy scanner catches it."""
    print("[Test 2] Data exfiltration / privacy scanner...")
    assert run_rust_test("immune::privacy::tests"), "Privacy scanner tests failed"
    print("  PASS")


def test_antibody_spawning():
    """Test 3: Unknown threat -> verify antibody spawned and stored in memory."""
    print("[Test 3] Antibody spawning...")
    assert run_rust_test("immune::antibody::tests"), "Antibody spawning tests failed"
    print("  PASS")


def test_immune_memory():
    """Test 4: Re-inject same threat -> verify instant recognition (immune memory)."""
    print("[Test 4] Immune memory recall...")
    assert run_rust_test("immune::memory::tests"), "Immune memory tests failed"
    print("  PASS")


def test_adversarial_arena():
    """Test 5: Run adversarial session -> verify both attacker and defender improve."""
    print("[Test 5] Adversarial arena...")
    assert run_rust_test("immune::arena::tests"), "Adversarial arena tests failed"
    print("  PASS")


def test_hive_immunity():
    """Test 6: Hive immunity -> threat blocked on agent A, verify agent B also blocks."""
    print("[Test 6] Hive immunity propagation...")
    assert run_rust_test("immune::hive::tests"), "Hive immunity tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 5 — Immune System Tests")
    print("=" * 60)

    tests = [
        test_prompt_injection_detection,
        test_data_exfiltration_detection,
        test_antibody_spawning,
        test_immune_memory,
        test_adversarial_arena,
        test_hive_immunity,
    ]

    passed = 0
    failed = 0
    for test in tests:
        try:
            test()
            passed += 1
        except AssertionError as e:
            failed += 1
            print(f"  FAIL: {e}")
        except Exception as e:
            failed += 1
            print(f"  ERROR: {e}")

    print(f"\nResults: {passed}/{len(tests)} passed, {failed} failed")
    sys.exit(0 if failed == 0 else 1)
