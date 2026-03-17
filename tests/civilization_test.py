"""Tests for System 7 — Agent Civilization.

These tests verify parliament voting, token economy, elections,
dispute resolution, and bankruptcy handling.
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


def test_propose_and_vote():
    """Test 1: Propose rule -> agents vote -> rule passes -> verify enforcement."""
    print("[Test 1] Propose rule and vote...")
    assert run_rust_test("civilization::parliament::tests"), "Parliament tests failed"
    print("  PASS")


def test_token_economy():
    """Test 2: Agent completes task -> earns tokens -> spends tokens for help."""
    print("[Test 2] Token economy...")
    assert run_rust_test("civilization::economy::tests"), "Economy tests failed"
    print("  PASS")


def test_election():
    """Test 3: Election -> verify highest-reputation agent wins coordinator role."""
    print("[Test 3] Elections...")
    assert run_rust_test("civilization::roles::tests"), "Election tests failed"
    print("  PASS")


def test_dispute_resolution():
    """Test 4: Dispute -> arbiter resolves -> verify decision logged."""
    print("[Test 4] Dispute resolution...")
    assert run_rust_test("civilization::disputes::tests"), "Dispute tests failed"
    print("  PASS")


def test_bankruptcy():
    """Test 5: Bankruptcy -> agent with zero tokens gets genome reset."""
    print("[Test 5] Bankruptcy detection...")
    assert run_rust_test("civilization::economy::tests"), "Bankruptcy tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 7 — Agent Civilization Tests")
    print("=" * 60)

    tests = [
        test_propose_and_vote,
        test_token_economy,
        test_election,
        test_dispute_resolution,
        test_bankruptcy,
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
