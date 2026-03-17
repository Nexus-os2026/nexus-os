"""Tests for System 10 — Self-Rewriting Kernel.

These tests verify performance profiling, code analysis, patch generation,
patch testing, application, and auto-rollback.
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


def test_profiler_bottleneck():
    """Test 1: Profile -> detect slow function -> verify bottleneck identified."""
    print("[Test 1] Performance profiling...")
    assert run_rust_test("self_rewrite::profiler::tests"), "Profiler tests failed"
    print("  PASS")


def test_patch_generation():
    """Test 2: Generate patch -> verify valid Rust code -> verify it compiles."""
    print("[Test 2] Patch generation...")
    assert run_rust_test("self_rewrite::patch::tests"), "Patch generation tests failed"
    print("  PASS")


def test_patch_testing():
    """Test 3: Test patch -> verify all tests pass -> verify it's faster."""
    print("[Test 3] Patch testing...")
    assert run_rust_test("self_rewrite::tester::tests"), "Patch testing tests failed"
    print("  PASS")


def test_patch_apply():
    """Test 4: Apply patch -> verify system still works."""
    print("[Test 4] Patch application...")
    assert run_rust_test("self_rewrite::patcher::tests"), "Patch application tests failed"
    print("  PASS")


def test_auto_rollback():
    """Test 5: Inject bad patch -> verify auto-rollback triggers."""
    print("[Test 5] Auto-rollback...")
    assert run_rust_test("self_rewrite::rollback::tests"), "Rollback tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 10 — Self-Rewriting Kernel Tests")
    print("=" * 60)

    tests = [
        test_profiler_bottleneck,
        test_patch_generation,
        test_patch_testing,
        test_patch_apply,
        test_auto_rollback,
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
