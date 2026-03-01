"""Offline pytest shim using unittest discovery.

This allows `python3 -m pytest` in environments without external pytest package.
"""

from __future__ import annotations

import argparse
import sys
import unittest


def main() -> None:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("-q", action="store_true")
    parser.add_argument("-x", action="store_true")
    parser.parse_known_args()

    loader = unittest.TestLoader()
    suite = loader.discover("tests")
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    main()
