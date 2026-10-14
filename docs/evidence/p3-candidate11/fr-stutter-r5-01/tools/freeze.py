#!/usr/bin/env python3
"""Freezes the mission's materials before the final gate (mission section 21).

  freeze.py ROOT

Records the sha256 of every file under ROOT/tools, ROOT/docs-src,
ROOT/development, ROOT/out/packet and ROOT/out/receipts in
ROOT/receipts/40-FREEZE.json and a readable ROOT/receipts/40-FREEZE-RECORD.txt.
Refuses if a freeze record already exists.
"""
import hashlib
import json
import os
import sys
import time

GROUPS = ["tools", "docs-src", "development", "out/packet", "out/receipts"]


def main(root):
    rec = os.path.join(root, "receipts", "40-FREEZE.json")
    if os.path.exists(rec):
        raise SystemExit("REFUSED: a freeze record exists")
    files = {}
    for g in GROUPS:
        for base, dirs, names in os.walk(os.path.join(root, g)):
            dirs[:] = sorted(d for d in dirs if d != "__pycache__")
            for n in sorted(names):
                p = os.path.join(base, n)
                files[os.path.relpath(p, root)] = hashlib.sha256(open(p, "rb").read()).hexdigest()
    os.makedirs(os.path.dirname(rec), exist_ok=True)
    stamp = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    json.dump({"frozen_at": stamp, "groups": GROUPS, "files": files}, open(rec, "w"), indent=1, sort_keys=True)
    with open(os.path.join(root, "receipts", "40-FREEZE-RECORD.txt"), "w") as fh:
        fh.write(f"FREEZE — P3-C11-DR01-D2A1-FR-STUTTER-R5-01, {stamp}\n")
        fh.write("Frozen before the final gate: source and evidence identities (out/receipts/00-authentication.txt), the "
                 "occurrence matrix, both static checkers, the proof, the AD-1-R5 draft, the configuration accounting, "
                 "the negative controls and the packet builder. The final gate runs once, on these exact files.\n")
        for k in sorted(files):
            fh.write(f"{files[k]}  {k}\n")
        fh.write(f"{len(files)} files\n")
    print(f"FREEZE: {len(files)} files recorded at {stamp}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
