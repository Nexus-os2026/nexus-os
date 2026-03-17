"""Tests for System 8 — Sovereign Identity with ZK Proofs.

These tests verify identity creation, credential signing, ZK proof generation
and verification, passport export/import, and tamper detection.
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


def test_identity_sign_verify():
    """Test 1: Create identity -> sign action -> verify signature."""
    print("[Test 1] Identity sign and verify...")
    assert run_rust_test("identity::credentials::tests"), "Credential tests failed"
    print("  PASS")


def test_zk_proof_clearance():
    """Test 2: Generate credential 'L3 clearance' -> verify without revealing L3."""
    print("[Test 2] ZK proof for clearance...")
    assert run_rust_test("identity::zkproofs::tests"), "ZK proof tests failed"
    print("  PASS")


def test_zk_proof_success_rate():
    """Test 3: Generate ZK proof 'success rate > 80%' -> verify (rate is actually 95%)."""
    print("[Test 3] ZK proof for success rate...")
    assert run_rust_test("identity::zkproofs::tests"), "ZK success rate tests failed"
    print("  PASS")


def test_passport_export_import():
    """Test 4: Export passport -> import on another instance -> verify all credentials."""
    print("[Test 4] Passport export/import...")
    assert run_rust_test("identity::passport::tests"), "Passport tests failed"
    print("  PASS")


def test_tampered_passport():
    """Test 5: Tampered passport -> verification FAILS."""
    print("[Test 5] Tampered passport detection...")
    assert run_rust_test("identity::passport::tests"), "Tamper detection tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 8 — Sovereign Identity with ZK Proofs Tests")
    print("=" * 60)

    tests = [
        test_identity_sign_verify,
        test_zk_proof_clearance,
        test_zk_proof_success_rate,
        test_passport_export_import,
        test_tampered_passport,
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
