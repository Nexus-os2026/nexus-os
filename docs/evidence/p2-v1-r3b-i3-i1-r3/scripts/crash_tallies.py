#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R3 crash-model tallies: the counts the six crash-model
recovery tests print (r3x1 to r3x6), taken from the store tests' --nocapture
output, as one table per test. Nothing is recomputed: each count is the
test's own, and each test also asserts its total against design section
15.2's coverage where the design gives one.

Usage: crash_tallies.py <store-tests --nocapture output> <out file>
"""
import pathlib
import re
import sys

TITLES = [
    ("r3x1", "R3 succession recovery over the crash model",
     "P-SUCCESSOR, the ordinary succession: 30 crash points, 400 states"),
    ("r3x2", "R3 repeat recovery over the crash model",
     "The repeat after F1 at operation 6 (step 2a skipped): 24 crash points"),
    ("r3x3", "R3 cleanup recovery over the crash model",
     "R-SUCCESSOR, one pool file (12 crash points) and two (13)"),
    ("r3x4", "R3 revocation recovery over the crash model",
     "P-REVOKE: 4 crash points, 30 states; R-REVOKE's own crash points from each split"),
    ("r3x5", "R3 provisioning recovery over the crash model",
     "P-PROV: 24 crash points, 338 states"),
    ("r3x6", "R3 re-qualification recovery over the crash model",
     "P-REQUALIFY: 6 crash points, 52 states"),
]


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    text = pathlib.Path(sys.argv[1]).read_text()
    lines = ["# Crash-model tallies (P2-V1-R3B-I3-I1-R3)", "",
             "Each state is a crash point of the procedure, after F1 (process death) or after F2 (power loss) "
             "under one ordered or per-directory schedule of the pending metadata log, pending data old or new. "
             "Each count is the number of states that took that recovery path, as the test printed it.", ""]
    total_ok = True
    for test, title, what in TITLES:
        start = text.find(title)
        if start < 0:
            lines += [f"## {test}: NOT FOUND", ""]
            total_ok = False
            continue
        end = text.find("\n}", start)
        body = text[start:end]
        rows = re.findall(r'^\s+"(.*)": (\d+),$', body, re.M)
        lines += [f"## {test}: {what}", "", "| path | states |", "|---|---|"]
        for path, count in rows:
            lines.append(f"| {path.replace('|', '/')} | {count} |")
        lines += [f"| **total** | **{sum(int(c) for _, c in rows)}** |", ""]
    pathlib.Path(sys.argv[2]).write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if total_ok else 1


if __name__ == "__main__":
    sys.exit(main())
