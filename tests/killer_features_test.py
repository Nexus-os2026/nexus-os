#!/usr/bin/env python3
"""
Nexus OS Killer Features Test Suite

Tests the 6 killer features via Rust unit tests (cargo test) and
local logic validation.

Killer 1: Screenshot Clone — vision analysis → project spec
Killer 2: Voice Project — continuous speech → intent → build
Killer 3: Freelance Engine — autonomous job scanning + execution
Killer 4: Stress Simulation — simulated user load testing
Killer 5: One-Click Deploy — cloud deployment config generation
Killer 6: Live Evolution — production monitoring + auto-improvement
"""

import json
import os
import subprocess
import sys

REPO_ROOT = os.path.join(os.path.dirname(__file__), "..")
PASS = "\033[92mPASS\033[0m"
FAIL = "\033[91mFAIL\033[0m"


def run_cargo_test(test_module: str) -> tuple[bool, str]:
    """Run cargo test for a specific module in the kernel crate."""
    result = subprocess.run(
        [
            "cargo", "test",
            "-p", "nexus-kernel",
            "--lib",
            test_module,
            "--", "--nocapture",
        ],
        capture_output=True,
        text=True,
        timeout=120,
        cwd=REPO_ROOT,
    )
    passed = result.returncode == 0
    output = result.stdout + result.stderr
    return passed, output


def count_tests(output: str) -> tuple[int, int]:
    """Extract passed/total from cargo test output."""
    for line in output.split("\n"):
        if "test result:" in line:
            parts = line.split()
            passed = 0
            failed = 0
            for i, p in enumerate(parts):
                if p == "passed;":
                    passed = int(parts[i - 1])
                if p == "failed;":
                    failed = int(parts[i - 1])
            return passed, passed + failed
    return 0, 0


def test_screenshot_clone() -> bool:
    """
    Test 1: Screenshot Clone
    - Verify: UI spec extracted with components, colors, layout
    - Verify: project spec generation with CRUD endpoints
    - Verify: visual match threshold checking
    """
    print("\n📸 Test 1: Screenshot Clone")
    passed, output = run_cargo_test("autopilot::screenshot_clone")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 4:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ Analysis prompt generation")
        print(f"   ✓ Project spec from analysis (CRUD endpoints, auth detection)")
        print(f"   ✓ Visual match threshold checking")
        print(f"   ✓ JSON parsing (valid + invalid)")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def test_voice_project() -> bool:
    """
    Test 2: Voice Project (simulated)
    - Verify: intent accumulator builds correct feature list
    - Verify: confidence increases trigger autopilot
    - Verify: reanalyze timing logic
    """
    print("\n🎤 Test 2: Voice Project Builder")
    passed, output = run_cargo_test("autopilot::voice_project")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 6:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ Start/stop listening lifecycle")
        print(f"   ✓ Transcript chunk accumulation")
        print(f"   ✓ Reanalyze timing logic (30s interval)")
        print(f"   ✓ Intent update → autopilot trigger (confidence > 0.8)")
        print(f"   ✓ No trigger on low confidence")
        print(f"   ✓ No trigger with ambiguities")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def test_freelance_engine() -> bool:
    """
    Test 3: Freelance Scanner
    - Verify: engine evaluates difficulty, estimates cost, calculates profit
    - Verify: only profitable jobs selected (bounty > 1.5x api_cost)
    - Verify: HITL modes work correctly
    """
    print("\n💰 Test 3: Freelance Engine")
    passed, output = run_cargo_test("economy::freelancer")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 8:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ Profitable job evaluation (ratio > 1.5x)")
        print(f"   ✓ Unprofitable job rejection")
        print(f"   ✓ Low confidence job rejection")
        print(f"   ✓ Capacity limit enforcement")
        print(f"   ✓ HITL modes: RequireApproval, AutoBidBelow, FullAuto")
        print(f"   ✓ Revenue tracking (earned, cost, profit, success rate)")
        print(f"   ✓ Rejected job cost accounting")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def test_stress_simulation() -> bool:
    """
    Test 4: Stress Simulation
    - Verify: personas generated with realistic behaviors
    - Verify: action sequences include edge cases (XSS, SQL injection, rage clicks)
    - Verify: report evaluation (critical failures, error rate thresholds)
    """
    print("\n🔥 Test 4: Stress Simulation")
    passed, output = run_cargo_test("autopilot::stress_test")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 6:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ Default persona generation (7 behavior types)")
        print(f"   ✓ Adversarial actions (XSS, SQL injection, large payloads)")
        print(f"   ✓ Impatient actions (rapid clicks, back-nav)")
        print(f"   ✓ Report evaluation: pass on healthy metrics")
        print(f"   ✓ Report evaluation: fail on critical failures")
        print(f"   ✓ Report evaluation: fail on high error rate")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def test_deploy() -> bool:
    """
    Test 5: One-Click Deploy
    - Verify: Dockerfile generation with health checks
    - Verify: deploy config validation
    - Verify: platform-specific deploy commands
    """
    print("\n🚀 Test 5: One-Click Deploy")
    passed, output = run_cargo_test("autopilot::deploy")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 7:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ Dockerfile generation (multi-stage, health check, SSL)")
        print(f"   ✓ Docker Compose with auto-restart")
        print(f"   ✓ Railway deploy commands")
        print(f"   ✓ Fly.io deploy commands")
        print(f"   ✓ Custom VPS deploy (SCP + SSH)")
        print(f"   ✓ Config validation (name, port, health path)")
        print(f"   ✓ VPS-specific validation (host, SSH key)")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def test_live_evolution() -> bool:
    """
    Test 6: Live Evolution
    - Verify: issue detection from degraded metrics
    - Verify: improvement calculation
    - Verify: health assessment (Healthy, Degraded, Down)
    """
    print("\n🧬 Test 6: Live App Evolution")
    passed, output = run_cargo_test("autopilot::live_evolution")
    test_passed, test_total = count_tests(output)

    if passed and test_total >= 6:
        print(f"   Rust unit tests: {test_passed}/{test_total} passed")
        print(f"   ✓ App registration and lookup")
        print(f"   ✓ No issues on healthy metrics")
        print(f"   ✓ Issue detection: error rate, response time, memory, CPU")
        print(f"   ✓ Positive improvement calculation (degraded → healthy)")
        print(f"   ✓ Negative improvement calculation (healthy → degraded)")
        print(f"   ✓ Health assessment: Healthy / Degraded / Down")
        print(f"   Result: {PASS}")
        return True
    else:
        print(f"   Rust tests failed: {output[-500:]}")
        print(f"   Result: {FAIL}")
        return False


def main():
    print("=" * 60)
    print("  NEXUS OS — KILLER FEATURES TEST SUITE")
    print("=" * 60)

    tests = [
        ("Screenshot Clone", test_screenshot_clone),
        ("Voice Project", test_voice_project),
        ("Freelance Engine", test_freelance_engine),
        ("Stress Simulation", test_stress_simulation),
        ("One-Click Deploy", test_deploy),
        ("Live Evolution", test_live_evolution),
    ]

    results = []
    total_pass = 0
    total_fail = 0

    for name, test_fn in tests:
        try:
            passed = test_fn()
            results.append((name, passed))
            if passed:
                total_pass += 1
            else:
                total_fail += 1
        except Exception as e:
            print(f"   ERROR: {e}")
            results.append((name, False))
            total_fail += 1

    # Summary
    print("\n" + "=" * 60)
    print("  SUMMARY")
    print("=" * 60)
    print()
    print(f"  {'Test':<25} {'Result':<10}")
    print(f"  {'-'*25} {'-'*10}")
    for name, passed in results:
        status = PASS if passed else FAIL
        print(f"  {name:<25} {status}")

    print()
    print(f"  Total: {total_pass}/{len(tests)} passed")

    if total_fail > 0:
        print(f"\n  {total_fail} test(s) FAILED")
        sys.exit(1)
    else:
        print(f"\n  All killer features operational! 🎯")
        sys.exit(0)


if __name__ == "__main__":
    main()
