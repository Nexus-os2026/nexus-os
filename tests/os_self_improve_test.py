"""Tests for Self-Improving OS — 8-layer self-improvement system.

Tests verify routing intelligence, performance tracking, security evolution,
knowledge accumulation, OS fitness scoring, and dream cycle optimization.
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


def test_routing_learns():
    """Test 1: Routing learns best agent per category."""
    print("[Test 1] Routing intelligence...")
    assert run_rust_test("self_improve::routing::tests"), "Routing tests failed"
    print("  PASS")


def test_performance_tracking():
    """Test 2: Performance tracking detects regressions."""
    print("[Test 2] Performance tracking...")
    assert run_rust_test(
        "self_improve::performance::tests"
    ), "Performance tests failed"
    print("  PASS")


def test_security_evolution():
    """Test 3: Security evolves rules from accuracy data."""
    print("[Test 3] Security evolution...")
    assert run_rust_test(
        "self_improve::security::tests"
    ), "Security evolution tests failed"
    print("  PASS")


def test_knowledge_accumulation():
    """Test 4: Knowledge accumulates user profile."""
    print("[Test 4] Knowledge accumulation...")
    assert run_rust_test(
        "self_improve::knowledge::tests"
    ), "Knowledge tests failed"
    print("  PASS")


def test_os_fitness():
    """Test 5: OS fitness score computes and trends upward."""
    print("[Test 5] OS fitness tracking...")
    assert run_rust_test("self_improve::fitness::tests"), "Fitness tests failed"
    print("  PASS")


def test_dream_cycle():
    """Test 6: Dream cycle covers OS-level optimization."""
    print("[Test 6] OS dream cycle...")
    assert run_rust_test(
        "self_improve::os_dreams::tests"
    ), "OS dream tests failed"
    print("  PASS")


def test_ui_learning():
    """Test 7: UI adaptation learns from usage patterns."""
    print("[Test 7] UI adaptation...")
    assert run_rust_test(
        "self_improve::ui_learning::tests"
    ), "UI learning tests failed"
    print("  PASS")


def test_orchestrator():
    """Test 8: SelfImprovingOS orchestrator integrates all layers."""
    print("[Test 8] Orchestrator integration...")
    assert run_rust_test("self_improve::tests"), "Orchestrator tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("Self-Improving OS — 8-Layer System Tests")
    print("=" * 60)

    tests = [
        test_routing_learns,
        test_performance_tracking,
        test_security_evolution,
        test_knowledge_accumulation,
        test_os_fitness,
        test_dream_cycle,
        test_ui_learning,
        test_orchestrator,
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
