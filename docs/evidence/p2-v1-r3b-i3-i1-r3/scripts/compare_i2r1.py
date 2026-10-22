#!/usr/bin/env python3
"""Compare two as-run summaries of the I2-R1 suite, control by control: the
same counted controls and informational runs, with the same outcome fields.

Usage: compare_i2r1.py <reference summary.json> <rerun summary.json>
"""
import json
import sys

FIELDS = ("ok", "compiled", "failed", "passed", "files_restored", "expect", "marker_in_assertion")


def rows(summary):
    out = {}
    for key in ("counted", "informational"):
        for r in summary.get(key, []):
            out[(key, r["id"], r.get("test"))] = {k: r.get(k) for k in FIELDS}
    return out


def main():
    reference = rows(json.load(open(sys.argv[1])))
    rerun = rows(json.load(open(sys.argv[2])))
    print(f"reference: {sys.argv[1]} ({len(reference)} entries)")
    print(f"rerun:     {sys.argv[2]} ({len(rerun)} entries)")
    differences = sorted(set(reference) ^ set(rerun)) + sorted(
        k for k in set(reference) & set(rerun) if reference[k] != rerun[k])
    for key, (kind, cid, test) in enumerate(sorted(reference)):
        same = reference.get((kind, cid, test)) == rerun.get((kind, cid, test))
        print(f"  {'same' if same else 'DIFFERENT'} {kind} {cid} {test}")
    print("RESULT:", "IDENTICAL" if not differences else f"DIFFERENT {differences}")
    return 0 if not differences else 1


if __name__ == "__main__":
    sys.exit(main())
