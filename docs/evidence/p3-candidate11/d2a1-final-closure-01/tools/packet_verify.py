#!/usr/bin/env python3
"""Verify each relied-on Git evidence packet against its own sealed list (SHA256SUMS or MANIFEST.sha256), reading every
file from the Git object store at the evidence head. Read-only. argv[1] = output file."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa

PACKETS = ["final-baseline-known-runs-01", "final-baseline-remaining-configs-01", "fr-stutter-r5-01",
           "final-source-proof-readiness-01", "current-tree-183-review-01", "d2a1-control-definitions-r2-01",
           "d2a1-mutation-tranche1-exec-01", "d2a1-mutation-tranche1-resume-01", "d2a1-mutation-tranche1-continue-02",
           "d2a1-mutation-tranche2-exec-01", "d2a1-mutation-tranche2-continue-02", "d2a1-mutation-tranche3-exec-01",
           "d2a1-mutation-tranche3-continue-02", "d2a1-mutation-tranche3-tail-01", "d2a1-mutation-witness-c1-c25-01",
           "d2a1-mutation-witness-c4-01", "d2a1-c7-adc7w-evidence-exec-01", "d2a1-c7-compliant-exec-02",
           "ad1r4-consolidation-01"]
out, bad = [], 0
for p in PACKETS:
    base = f"{EVB}/{p}"
    lst = next((n for n in ("SHA256SUMS", "MANIFEST.sha256", "PACKET-SHA256SUMS") if exists(HEAD_EV, f"{base}/{n}")), None)
    if not lst:
        out.append(f"FAIL {p}: no sealed list")
        bad += 1
        continue
    lines = [l for l in show(HEAD_EV, f"{base}/{lst}").decode().splitlines() if l.strip()]
    tree = set(x for x in git("ls-tree", "-r", "-z", "--name-only", HEAD_EV, "--", base).split("\0") if x)
    listed, mism, missing = set(), [], []
    for l in lines:
        h, _, f = l.partition("  ")
        f = f.lstrip("*").lstrip("./") if f.startswith("./") else f.lstrip("*")
        path = f"{base}/{f}"
        listed.add(path)
        if path not in tree:
            missing.append(f)
        elif sha(show(HEAD_EV, path)) != h:
            mism.append(f)
    unlisted = sorted(t for t in tree - listed if not t.endswith("/" + lst))
    ok = not mism and not missing
    bad += not ok
    out.append(f"{'OK  ' if ok else 'FAIL'} {p}: {lst} blob {blob_id(HEAD_EV, f'{base}/{lst}')} sha256 "
               f"{sha(show(HEAD_EV, f'{base}/{lst}'))}; {len(lines)} entries; mismatched {len(mism)}; missing {len(missing)}; "
               f"files in Git not listed {len(unlisted)}" + (f" {unlisted[:6]}" if unlisted else "")
               + (f"; mismatched {mism[:5]} missing {missing[:5]}" if not ok else ""))
out.append(f"\nRESULT: {len(PACKETS) - bad} of {len(PACKETS)} packets verify")
open(sys.argv[1], "x").write("\n".join(out) + "\n")
print("\n".join(out))
sys.exit(0 if not bad else 1)
