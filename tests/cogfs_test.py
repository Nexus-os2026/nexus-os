"""Tests for System 6 — Cognitive Filesystem.

These tests verify semantic indexing, knowledge graph, natural language queries,
code file analysis, and context building.
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


def test_index_text_file():
    """Test 1: Index a text file -> verify entities extracted correctly."""
    print("[Test 1] Index text file and extract entities...")
    assert run_rust_test("cogfs::indexer::tests"), "Indexer tests failed"
    print("  PASS")


def test_knowledge_graph_links():
    """Test 2: Index 3 related files -> verify knowledge graph links them."""
    print("[Test 2] Knowledge graph linking...")
    assert run_rust_test("cogfs::graph::tests"), "Knowledge graph tests failed"
    print("  PASS")


def test_natural_language_query():
    """Test 3: Natural language query -> verify correct files returned."""
    print("[Test 3] Natural language query...")
    assert run_rust_test("cogfs::query::tests"), "Natural query tests failed"
    print("  PASS")


def test_code_file_indexing():
    """Test 4: Index code file -> verify functions, classes, imports extracted."""
    print("[Test 4] Code file indexing...")
    # Code file parsing is part of the indexer tests
    assert run_rust_test("cogfs::indexer::tests"), "Code indexing tests failed"
    print("  PASS")


def test_context_builder():
    """Test 5: Context builder -> verify rich context generated for agent."""
    print("[Test 5] Context builder...")
    assert run_rust_test("cogfs::context::tests"), "Context builder tests failed"
    print("  PASS")


if __name__ == "__main__":
    print("=" * 60)
    print("System 6 — Cognitive Filesystem Tests")
    print("=" * 60)

    tests = [
        test_index_text_file,
        test_knowledge_graph_links,
        test_natural_language_query,
        test_code_file_indexing,
        test_context_builder,
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
