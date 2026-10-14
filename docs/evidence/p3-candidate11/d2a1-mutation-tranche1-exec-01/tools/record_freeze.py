#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once, before the first baseline
invocation.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists

Frozen: every tool (tools/*.py, tools/scope-wrap.sh), the reused vocabulary, every authenticated input copy (catalog,
definitions record, the twelve patches, contract, AD-1-R4, AD-1-R5, the Stage-2 tool copies, the linked Stage-2
receipts), the derived expectations (receipts/02-EXPECTATIONS.json: commands, environments, selections, named
properties, ceilings, limits, control order, restoration and link mappings) and the catalog-freeze ruling record.
supervise.py, healthy.py and control.py re-check every one of them before every supervised step.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

POLICY = {
    "control_order": K.ORDER,
    "not_authorized": K.NOT_AUTHORIZED,
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"baseline tests": 180, "builds": K.BUILD_LIMIT, "unit-test controls": 180, "C12 (X-1)": 180,
                       "F-B2 and F-G controls": 2700},
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "git apply -R of the same frozen patch in the disposable worktree; changed file byte-equal to C; "
                   "verify_tree.py EXACTLY_C; status clean but for the worktree-local target",
    "retries": "none: one baseline invocation each, one build per control, one run per control, no rebuild, no rerun",
}


def main():
    fj = os.path.join(K.EV, "receipts/05-FREEZE.json")
    ft = os.path.join(K.EV, "receipts/05-FREEZE-RECORD.txt")
    if os.path.exists(fj) or os.path.exists(ft):
        raise SystemExit("REFUSED: the freeze is already recorded")
    files = {}
    for sub in ("tools", "inputs", "vocabulary"):
        for d, dirs, fs in os.walk(os.path.join(K.EV, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                if not f.endswith(".pyc"):
                    p = os.path.join(d, f)
                    files[os.path.relpath(p, K.EV)] = K.sha256_file(p)
    for rel in ("receipts/02-EXPECTATIONS.json", "documents/ARCHITECT-RULING-CATALOG-FREEZE.md"):
        files[rel] = K.sha256_file(os.path.join(K.EV, rel))
    prior = {rel: K.sha256_file(os.path.join(K.EV, rel)) for rel in (
        "receipts/00-authentication.txt", "receipts/01-supervision-selftest/SELFTEST.txt",
        "receipts/03-tool-controls/SUMMARY.txt", "receipts/04-rehearsal/SUMMARY.txt",
        "receipts/04b-render-rehearsal/SUMMARY.txt")}
    for rel in prior:
        first = open(os.path.join(K.EV, rel)).read()
        if not any(x in first for x in ("ALL CHECKS HOLD", "SELF-TEST RESULT: ALL OK", "ALL AS EXPECTED")):
            raise SystemExit(f"REFUSED: {rel} does not record success")
    rec = {"mission": K.MISSION, "recorded": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "files": files,
           "pre_freeze_receipts": prior, "policy": POLICY,
           "statement": "No semantic patch repair, classifier repair, alternate control encoding or automatic rerun "
                        "after this point. A frozen-tool failure stops the mission and preserves its raw evidence."}
    json.dump(rec, open(fj, "w"), indent=1)
    lines = ["FREEZE: RECORDED", f"recorded: {rec['recorded']}", f"frozen files: {len(files)}",
             f"05-FREEZE.json sha256 {K.sha256_file(fj)}"]
    lines += [f"  {h}  {p}" for p, h in sorted(files.items())]
    lines += ["pre-freeze receipts (success recorded):"] + [f"  {h}  {p}" for p, h in prior.items()]
    lines += ["policy:"] + [f"  {k}: {v}" for k, v in POLICY.items()]
    lines.append(rec["statement"])
    open(ft, "w").write("\n".join(lines) + "\n")
    print("\n".join(lines[:4]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
