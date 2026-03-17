"""Tests for System 9 — Distributed Consciousness Mesh.

These tests verify peer discovery, consciousness sync, agent migration,
distributed execution, and shared knowledge.
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


def test_discover_connect():
    """Test 1: Discover -> connect -> verify handshake."""
    print("[Test 1] Peer discovery and connection...")
    assert run_rust_test("mesh::discovery::tests"), "Discovery tests failed"
    print("  PASS")


def test_state_sync():
    """Test 2: Agent state change on A -> verify replicated to B."""
    print("[Test 2] Consciousness sync...")
    assert run_rust_test("mesh::sync::tests"), "Sync tests failed"
    print("  PASS")


def test_agent_migration():
    """Test 3: Migrate agent from A to B -> verify continues working."""
    print("[Test 3] Agent migration...")
    assert run_rust_test("mesh::migration::tests"), "Migration tests failed"
    print("  PASS")


def test_distributed_task():
    """Test 4: Distributed task -> verify sub-results merge correctly."""
    print("[Test 4] Distributed execution...")
    assert run_rust_test("mesh::execution::tests"), "Distributed execution tests failed"
    print("  PASS")


def test_shared_knowledge():
    """Test 5: Shared knowledge -> index file on A, query from B."""
    print("[Test 5] Shared memory...")
    assert run_rust_test("mesh::shared_memory::tests"), "Shared memory tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 9 — Distributed Consciousness Mesh Tests")
    print("=" * 60)

    tests = [
        test_discover_connect,
        test_state_sync,
        test_agent_migration,
        test_distributed_task,
        test_shared_knowledge,
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
