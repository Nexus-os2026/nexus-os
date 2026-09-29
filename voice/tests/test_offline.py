import _offline  # noqa: F401  (first: no test may download a model)

import ast
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

from _offline import OFFLINE_ENVIRONMENT

TESTS_DIR = Path(__file__).resolve().parent

CHILD_ENVIRONMENT = (
    "import json, os, sys; "
    "json.dump({name: os.environ.get(name) for name in sys.argv[1:]}, sys.stdout)"
)


class OfflineTestEnvironmentTests(unittest.TestCase):
    """No voice test may download a model: the model hubs are offline for
    every test module, in the test process and in the stt.py processes the
    tests start."""

    def test_every_test_module_imports_the_offline_switches_first(self) -> None:
        modules = sorted(TESTS_DIR.glob("test_*.py"))
        self.assertGreaterEqual(len(modules), 8)
        for path in modules:
            tree = ast.parse(path.read_text(encoding="utf-8"))
            imports = [
                node
                for node in tree.body
                if isinstance(node, (ast.Import, ast.ImportFrom))
                and not (isinstance(node, ast.ImportFrom) and node.module == "__future__")
            ]
            self.assertTrue(imports, path.name)
            first = imports[0]
            self.assertIsInstance(first, ast.Import, path.name)
            self.assertEqual([alias.name for alias in first.names], ["_offline"], path.name)

    def test_the_switches_reach_this_process_and_its_child_processes(self) -> None:
        self.assertEqual(OFFLINE_ENVIRONMENT.get("HF_HUB_OFFLINE"), "1")
        for name, value in OFFLINE_ENVIRONMENT.items():
            self.assertEqual(os.environ.get(name), value, name)
        child = subprocess.run(
            [sys.executable, "-c", CHILD_ENVIRONMENT, *OFFLINE_ENVIRONMENT],
            capture_output=True,
            text=True,
            timeout=60,
        )
        self.assertEqual(child.returncode, 0, child.stderr)
        self.assertEqual(json.loads(child.stdout), OFFLINE_ENVIRONMENT)

    def test_the_model_hub_library_is_offline(self) -> None:
        try:
            from huggingface_hub import constants  # type: ignore
        except ImportError:
            self.skipTest("huggingface_hub is not installed")
        self.assertTrue(constants.HF_HUB_OFFLINE)
        self.assertTrue(constants.HF_HUB_DISABLE_TELEMETRY)


if __name__ == "__main__":
    unittest.main()
