#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission section 13: the packet freeze record.

    record_freeze.py ROOT

Writes ROOT/receipts/50-FREEZE-RECORD.json: the sha256 of every file the packet will carry, except this record and
the final gate's two receipts, as they stand when the final gate starts. It freezes the evidence packet's bytes for
review; it gives the catalog no status (the catalog stays a CANDIDATE; its acceptance belongs to the Sole Architect).
"""
import hashlib
import json
import os
import sys

DIRS = ("tools", "development", "out/catalog", "out/docs")
FILES = ("docs-src/INDEX.md", "receipts/00-authentication.txt", "receipts/20-checker-a.txt",
         "receipts/21-checker-b.txt", "receipts/30-syntax-check.txt", "receipts/40-negative-controls.txt")


def walk(base):
    for b, dirs, names in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for n in sorted(names):
            yield os.path.join(b, n)


def main(root):
    rec = {}
    for d in DIRS:
        for p in walk(os.path.join(root, d)):
            rec[os.path.relpath(p, root)] = hashlib.sha256(open(p, "rb").read()).hexdigest()
    for f in FILES:
        rec[f] = hashlib.sha256(open(os.path.join(root, f), "rb").read()).hexdigest()
    doc = {"mission": "P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01",
           "what": "sha256 of every packet file at the start of the final gate (this record and receipts/60-* "
                   "excluded); the packet's evidence bytes, not a catalog status",
           "catalog_status": "CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING",
           "files": dict(sorted(rec.items()))}
    with open(os.path.join(root, "receipts", "50-FREEZE-RECORD.json"), "w") as fh:
        json.dump(doc, fh, indent=1, sort_keys=True)
        fh.write("\n")
    print(f"freeze record: {len(rec)} files")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
