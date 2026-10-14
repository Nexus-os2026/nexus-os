#!/usr/bin/env python3
"""Records the sha256 of every tool, document source and development record (ROOT/receipts/50-TOOLS-RECORD.json).

    record_tools.py ROOT

This is a record of the tooling used for the blocker packet, made before the final gate, which checks every file
against it. It is NOT a freeze of the control catalog: the catalog is blocked and not frozen.
"""
import hashlib
import json
import os
import sys


def walk(base):
    for b, dirs, names in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for n in sorted(names):
            yield os.path.join(b, n)


root = sys.argv[1]
files = {os.path.relpath(p, root): hashlib.sha256(open(p, "rb").read()).hexdigest()
         for d in ("tools", "docs-src", "development") for p in walk(os.path.join(root, d))}
record = {"what": "tools record for P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01's blocker packet (not a catalog freeze)",
          "files": files}
with open(os.path.join(root, "receipts", "50-TOOLS-RECORD.json"), "w") as fh:
    json.dump(record, fh, indent=1, sort_keys=True)
    fh.write("\n")
print(f"recorded {len(files)} files")
