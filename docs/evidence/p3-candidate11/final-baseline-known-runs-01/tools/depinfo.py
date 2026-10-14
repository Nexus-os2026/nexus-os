#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: the pinned test executable's own source inputs, from rustc's dep-info
file written by the one build, each bound to its blob at Candidate C. Read-only.

  depinfo.py > receipts/11-pin/DEP-INFO.txt

Replaces the informational dep-info line of PIN-RECEIPT.txt, whose regular expression crossed line breaks and printed
a garbled list (that line was not one of the pin's checks).
"""
import hashlib
import re
import subprocess

WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/"
D = WT + "target/release/deps/nexus_governed_control-2faa0b9718d3e6a3.d"
C = "d5964ee5326c2733177307db6f507328023d361a"

txt = open(D).read()
first = txt.split("\n", 1)[0]
target, _, deps = first.partition(": ")
deps = [d[len(WT):] if d.startswith(WT) else d for d in deps.split()]
envs = re.findall(r"^# env-dep:(.*)$", txt, re.M)
print("DEP-INFO of the pinned test executable (rustc's own record of its source inputs)")
print(f"file: {D}")
print(f"sha256: {hashlib.sha256(txt.encode()).hexdigest()}")
print(f"inputs: {len(deps)}; every one under crates/nexus-governed-control/src/: "
      f"{all(d.startswith('crates/nexus-governed-control/src/') for d in deps)}; any under docs/: "
      f"{any(d.startswith('docs/') or '/docs/' in d for d in deps)}")
print(f"environment dependencies recorded by rustc: {envs}")
bad = 0
for d in deps:
    blob = subprocess.run(["git", "-C", WT, "rev-parse", f"{C}:{d}"], capture_output=True, text=True).stdout.strip()
    disk = subprocess.run(["git", "-C", WT, "hash-object", d], capture_output=True, text=True).stdout.strip()
    same = blob == disk and blob != ""
    bad += not same
    print(f"  {'OK ' if same else 'BAD'} {d}  blob at C {blob}  on disk {disk}")
print(f"RESULT: {'every input is C’s blob' if not bad else f'{bad} input(s) differ from C'}")
