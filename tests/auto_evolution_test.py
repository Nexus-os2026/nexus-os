#!/usr/bin/env python3
"""
Nexus OS Auto-Evolution Integration Tests

Tests the auto-evolution subsystem:
1. Score tracking — records, averages, and caps scores per agent
2. Evolution triggers — fires when scores drop below threshold
3. Prompt mutation — generates improved prompts via LLM
4. Revert on no improvement — preserves original prompt
5. Genome fitness updated — fitness_history reflects evolved score
6. Cooldown respected — prevents runaway evolution

Uses `cargo test` to run the Rust unit tests, then validates the
overall system via Rust integration tests.
"""

import json
import os
import subprocess
import sys
from datetime import datetime

# ─── Configuration ────────────────────────────────────────────────────────────

REPO_ROOT = os.path.join(os.path.dirname(__file__), "..")

# ─── Helpers ──────────────────────────────────────────────────────────────────

PASS = "\033[92mPASS\033[0m"
FAIL = "\033[91mFAIL\033[0m"
INFO = "\033[94mINFO\033[0m"

results = []


def run(label, func):
    """Run a single test and record result."""
    try:
        ok, detail = func()
        status = PASS if ok else FAIL
        results.append({"test": label, "passed": ok, "detail": detail})
        print(f"  [{status}] {label}: {detail}")
        return ok
    except Exception as e:
        results.append({"test": label, "passed": False, "detail": str(e)})
        print(f"  [{FAIL}] {label}: {e}")
        return False


def cargo_test(filter_pattern, cwd=None):
    """Run cargo test with a filter and return (returncode, stdout, stderr)."""
    cmd = [
        "cargo", "test", "-p", "nexus-kernel", "--",
        filter_pattern, "--nocapture"
    ]
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=120,
        cwd=cwd or REPO_ROOT,
    )
    return proc.returncode, proc.stdout, proc.stderr


# ─── Test 1: Score Tracking ──────────────────────────────────────────────────

def test_score_tracking():
    """Verify score recording, averaging, and cap at 20."""
    rc, stdout, stderr = cargo_test("auto_evolve::tests::tracker_records_scores")
    if rc != 0:
        return False, f"cargo test failed: {stderr[-200:]}"

    rc2, _, stderr2 = cargo_test("auto_evolve::tests::tracker_caps_at_20")
    if rc2 != 0:
        return False, f"cap test failed: {stderr2[-200:]}"

    return True, "Score recording + 20-cap verified"


# ─── Test 2: Evolution Triggers on Low Scores ────────────────────────────────

def test_evolution_triggers():
    """Verify should_evolve fires with 3+ low scores and respects threshold."""
    tests = [
        "auto_evolve::tests::should_evolve_requires_minimum_scores",
        "auto_evolve::tests::should_evolve_respects_threshold",
        "auto_evolve::tests::should_evolve_respects_enabled_flag",
    ]
    for t in tests:
        rc, _, stderr = cargo_test(t)
        if rc != 0:
            return False, f"{t} failed: {stderr[-200:]}"
    return True, "Evolution triggers verified (min scores, threshold, enabled flag)"


# ─── Test 3: Prompt Mutation Improves Score ──────────────────────────────────

def test_prompt_mutation_improves():
    """Verify that evolution attempt with high-scoring LLM improves the agent."""
    rc, _, stderr = cargo_test("auto_evolve::tests::evolution_attempt_improves_score")
    if rc != 0:
        return False, f"Failed: {stderr[-200:]}"
    return True, "Mutation improved score (mock LLM: low -> high)"


# ─── Test 4: Revert on No Improvement ────────────────────────────────────────

def test_revert_on_no_improvement():
    """Verify that mutations are reverted when they don't help."""
    rc, _, stderr = cargo_test("auto_evolve::tests::evolution_attempt_reverts")
    if rc != 0:
        return False, f"Failed: {stderr[-200:]}"
    return True, "Reverted correctly when mutation didn't improve"


# ─── Test 5: Genome Fitness Updated ──────────────────────────────────────────

def test_genome_fitness_updated():
    """Verify that apply_evolution returns genome with updated fitness."""
    rc, _, stderr = cargo_test("auto_evolve::tests::apply_evolution_returns_mutated_genome")
    if rc != 0:
        return False, f"Failed: {stderr[-200:]}"
    return True, "Genome fitness updated with evolved prompt"


# ─── Test 6: Cooldown Respected ──────────────────────────────────────────────

def test_cooldown_respected():
    """Verify cooldown prevents rapid evolution, and force bypasses it."""
    tests = [
        "auto_evolve::tests::should_evolve_respects_cooldown",
        "auto_evolve::tests::force_evolve_bypasses_cooldown",
    ]
    for t in tests:
        rc, _, stderr = cargo_test(t)
        if rc != 0:
            return False, f"{t} failed: {stderr[-200:]}"
    return True, "Cooldown respected + force bypass verified"


# ─── Test 7: Full Rust Test Suite ─────────────────────────────────────────────

def test_full_rust_suite():
    """Run all auto_evolve tests to confirm zero regressions."""
    rc, stdout, stderr = cargo_test("auto_evolve")
    # Count passed tests
    for line in (stdout + stderr).splitlines():
        if "test result:" in line and "passed" in line:
            return "0 failed" in line, line.strip()
    return rc == 0, f"Return code: {rc}"


# ─── Test 8: Manager Handles Multiple Agents ─────────────────────────────────

def test_multi_agent():
    """Verify separate tracking for multiple agents."""
    rc, _, stderr = cargo_test("auto_evolve::tests::manager_handles_multiple_agents")
    if rc != 0:
        return False, f"Failed: {stderr[-200:]}"
    return True, "Multi-agent tracking verified"


# ─── Test 9: Evolution Log ───────────────────────────────────────────────────

def test_evolution_log():
    """Verify evolution log records attempts and caps at 200."""
    tests = [
        "auto_evolve::tests::evolution_log_records_attempts",
        "auto_evolve::tests::evolution_log_caps_at_200",
    ]
    for t in tests:
        rc, _, stderr = cargo_test(t)
        if rc != 0:
            return False, f"{t} failed: {stderr[-200:]}"
    return True, "Evolution log recording + 200-cap verified"


# ─── Test 10: Config Updates ─────────────────────────────────────────────────

def test_config_updates():
    """Verify set_evolution_config updates tracker params."""
    rc, _, stderr = cargo_test("auto_evolve::tests::set_evolution_config_updates_tracker")
    if rc != 0:
        return False, f"Failed: {stderr[-200:]}"
    return True, "Config updates (enabled, threshold, cooldown) verified"


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print(f"\n{'=' * 60}")
    print(f"  Nexus OS Auto-Evolution Test Suite")
    print(f"  Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"{'=' * 60}\n")

    run("1. Score tracking", test_score_tracking)
    run("2. Evolution triggers", test_evolution_triggers)
    run("3. Prompt mutation improves score", test_prompt_mutation_improves)
    run("4. Revert on no improvement", test_revert_on_no_improvement)
    run("5. Genome fitness updated", test_genome_fitness_updated)
    run("6. Cooldown respected", test_cooldown_respected)
    run("7. Full Rust test suite", test_full_rust_suite)
    run("8. Multi-agent tracking", test_multi_agent)
    run("9. Evolution log", test_evolution_log)
    run("10. Config updates", test_config_updates)

    # ─── Summary ──────────────────────────────────────────────────────────
    passed = sum(1 for r in results if r["passed"])
    total = len(results)
    print(f"\n{'=' * 60}")
    print(f"  Results: {passed}/{total} passed")
    print(f"{'=' * 60}")

    # Save results
    results_path = os.path.join(os.path.dirname(__file__), "auto_evolution_results.json")
    with open(results_path, "w") as f:
        json.dump({
            "date": datetime.now().isoformat(),
            "tests": results,
            "passed": passed,
            "total": total,
        }, f, indent=2)
    print(f"  Results saved to {results_path}\n")

    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main())
