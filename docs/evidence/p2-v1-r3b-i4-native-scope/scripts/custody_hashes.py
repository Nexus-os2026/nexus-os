#!/usr/bin/env python3
"""The accepted custody store's immutable-hash verification: every file the
accepted R3 store controls pin (docs/evidence/p2-v1-r3b-i3-i1-r3/controls/
store/summary.json, `original`) and every file the I2-R1 rerun pins
(.../controls/i2r1-rerun/summary.json) hashes, in the checkout, to exactly
the value recorded when R3 was accepted.

Usage: custody_hashes.py <checkout>
"""
import hashlib
import json
import pathlib
import sys

R3 = "docs/evidence/p2-v1-r3b-i3-i1-r3/controls"


def main():
    root = pathlib.Path(sys.argv[1]).resolve()
    pinned = {}
    for summary in (f"{R3}/store/summary.json", f"{R3}/i2r1-rerun/summary.json"):
        pinned.update(json.loads((root / summary).read_text())["summary"]["original"])
    ok = True
    for path, digest in sorted(pinned.items()):
        actual = hashlib.sha256((root / path).read_bytes()).hexdigest()
        same = actual == digest
        ok &= same
        print(f"{'same' if same else 'CHANGED'} {digest} {path}")
    print(f"files: {len(pinned)}")
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
