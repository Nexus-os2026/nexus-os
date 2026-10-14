#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once, before the first baseline
invocation. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 10): the freeze is recorded once before
C14, and it also covers the EXEC-01 tools the repair starts from, the C22 Architect-ruling record and every
carried-forward receipt copy; the pre-freeze receipts are this mission's.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists

Frozen: every tool (tools/*.py, tools/scope-wrap.sh), the EXEC-01 tools (original-tools/), the reused vocabulary, every
authenticated input copy (catalog, definitions record, the twelve patches, contract, AD-1-R4, AD-1-R5, the Stage-2 tool
copies, the linked Stage-2 receipts), the carried-forward expectations (receipts/02-EXPECTATIONS.json: commands,
environments, selections, named properties, ceilings, limits, control order, restoration and link mappings), the
carried-forward baseline, healthy-build and C22 receipts, the C22 Architect-ruling record and the catalog-freeze ruling
record. supervise.py and control.py re-check every one of them before every supervised step.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

POLICY = {
    "control_order": K.RESUME_ORDER,
    "architect_accepted_not_executed": K.ACCEPTED,
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
                   "verify_tree.py EXACTLY_C; the path-aware status policy (verify_mutant.status_policy): no tracked "
                   "change, ignored entries only beneath the worktree's own real target directory",
    "source_verification": "verify_mutant.py: HEAD/tree C, detached, index = HEAD, every tracked byte and mode compared, "
                           "the named file exactly the catalog mutant; the path-aware status policy (resume section 7)",
    "retries": "none: one build per control, one run per control, no rebuild, no rerun; C22 and the baselines are not "
               "rerun",
}


def main():
    fj = os.path.join(K.EV, "receipts/05-FREEZE.json")
    ft = os.path.join(K.EV, "receipts/05-FREEZE-RECORD.txt")
    if os.path.exists(fj) or os.path.exists(ft):
        raise SystemExit("REFUSED: the freeze is already recorded")
    files = {}
    for sub in ("tools", "original-tools", "inputs", "vocabulary") + tuple(c for c in K.CARRIED if c.count("/") == 1
                                                                         and os.path.isdir(os.path.join(K.EV, c))):
        for d, dirs, fs in os.walk(os.path.join(K.EV, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                if not f.endswith(".pyc"):
                    p = os.path.join(d, f)
                    files[os.path.relpath(p, K.EV)] = K.sha256_file(p)
    for rel in ("receipts/02-EXPECTATIONS.json", K.C22_RULING) + tuple(K.CARRIED_DOCS):
        files[rel] = K.sha256_file(os.path.join(K.EV, rel))
    prior = {rel: K.sha256_file(os.path.join(K.EV, rel)) for rel in (
        "receipts/00-authentication.txt", "receipts/01-supervision-selftest/SELFTEST.txt",
        "receipts/03-verifier-controls/SUMMARY.txt", "receipts/04-tool-repair/TOOL-DIFF-CHECK.txt",
        "receipts/04b-render-rehearsal/SUMMARY.txt")}
    for rel in prior:
        last = open(os.path.join(K.EV, rel)).read().rstrip().splitlines()[-1]
        if last not in ("RESULT: ALL CHECKS HOLD", "SELF-TEST RESULT: ALL OK", "VERIFIER CONTROLS: ALL AS EXPECTED",
                        "TOOL DIFF: ALL CHECKS HOLD", "RENDER REHEARSAL: ALL AS EXPECTED"):
            raise SystemExit(f"REFUSED: {rel} does not record success")
    rec = {"mission": K.MISSION, "recorded": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "files": files,
           "pre_freeze_receipts": prior, "policy": POLICY,
           "statement": "No semantic tool repair, patch repair, classifier repair, alternate control encoding or "
                        "automatic retry after this point. A frozen-tool failure stops the mission and preserves its raw "
                        "evidence."}
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
