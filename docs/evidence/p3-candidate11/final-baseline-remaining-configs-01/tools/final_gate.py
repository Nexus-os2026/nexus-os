#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01: the final gate, read-only, run once after the last invocation and
before the packet is built.

  final_gate.py          prints one line per check and the mission's final status; exit 0 when every integrity check
                         holds (the status itself may be a BLOCKED one)

Integrity: identities (authenticate.py), the freeze (every frozen file unchanged), the pre-execution gates (vocabulary
checks, controls, supervision self-test, Stage-1 normalization), the order of invocations (no invocation after one
not classified PASS), no process or scope of this mission still running. Status: from the classifications only.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402
import supervise  # noqa: E402

EV = common.EV
lines, fails = [], []


def check(label, ok, detail=""):
    lines.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def first_line(rel):
    p = os.path.join(EV, rel)
    return open(p).read().splitlines()[0] if os.path.exists(p) else None


def last_line(rel):
    p = os.path.join(EV, rel)
    return open(p).read().strip().splitlines()[-1] if os.path.exists(p) else None


def main():
    r = subprocess.run([sys.executable, os.path.join(EV, "tools/authenticate.py")], capture_output=True, text=True)
    check("identities (authenticate.py): C, worktree byte-equal to C, pinned executable, Stage-1 evidence, destination "
          "absent", r.returncode == 0, r.stdout.strip().splitlines()[-1])
    rec = []
    problem = supervise.freeze_check(rec)
    check("freeze intact: every frozen tool, the vocabulary and the expectations unchanged since the freeze",
          problem is None, rec[0] if rec else problem)
    import configs
    exp = json.load(open(os.path.join(EV, "receipts/27-EXPECTATIONS.json")))
    d = configs.derive()
    now = {k: {f: v[f] for f in ("test", "bounds", "actors", "properties", "na")} for k, v in d["required"].items()}
    check("configuration expectations re-derived from C equal the frozen record (27-EXPECTATIONS.json)",
          not d["problems"] and now == exp["configurations"], "equal" if now == exp["configurations"] else "DIFFERENT")
    gates = [("receipts/20-vocabulary-generate.txt", "problems 0"),
             ("receipts/21-vocabulary-compare.txt", "EQUAL"),
             ("receipts/22-check-A-sites.txt", "PASS"),
             ("receipts/23-check-B-eval.txt", "PASS"),
             ("receipts/24-vocabulary-controls.txt", "ALL OK"),
             ("receipts/25-classifier-controls.txt", "ALL OK"),
             ("receipts/26-configs.txt", "CONFIGS: OK"),
             ("receipts/05-supervision-selftest/SELFTEST.txt", "ALL OK"),
             ("normalized/stage1/SUMMARY.txt", "ALL NORMALIZED")]
    for rel, want in gates:
        l = last_line(rel)
        check(f"pre-execution gate {rel}", l is not None and want in l, l)
    statuses = []
    order = [("C1", "receipts/50-witness-C1"), ("C25", "receipts/51-witness-C25"), ("C4", "receipts/52-witness-C4")] + \
            [(supervise.CFG[k], supervise.FULL[k][1]) for k in supervise.ORDER]
    stopped = False
    for name, rel in order:
        exists = os.path.exists(os.path.join(EV, rel))
        cls = first_line(os.path.join(rel, "CLASSIFICATION.txt"))
        outer = (open(os.path.join(EV, rel, "CLASSIFICATION.txt")).read().splitlines()[1]
                 if cls else None)
        if stopped:
            check(f"{name}: not attempted after a stop", not exists, rel if exists else "")
            statuses.append((name, "NOT ATTEMPTED (after a stop)"))
            continue
        if not exists:
            statuses.append((name, "NOT ATTEMPTED"))
            stopped = True
            continue
        check(f"{name}: attempted and classified", cls is not None, cls)
        statuses.append((name, f"{cls} — {outer}"))
        if cls != "CLASSIFICATION: PASS":
            stopped = True
    for name, st in statuses:
        lines.append(f"     {name}: {st}")
    exes, builds, scopes = supervise.conflicts()
    check("no governed-control executable, build or p3c11 scope still running", not exes and not builds and not scopes,
          f"{exes} {builds} {scopes}")
    witnesses_ok = all(s.startswith("CLASSIFICATION: PASS") for n, s in statuses[:3])
    full_ok = all(s.startswith("CLASSIFICATION: PASS") for n, s in statuses[3:])
    agg = os.path.exists(os.path.join(EV, "matrix/AGGREGATE-20.json"))
    check("the aggregate matrix exists exactly when all ten remaining configurations PASS", agg == (witnesses_ok and full_ok),
          f"aggregate {'present' if agg else 'absent'}")
    if not witnesses_ok:
        status = "HEALTHY_WITNESS_RECEIPT_BLOCKED"
    elif not full_ok:
        status = "REMAINING_CONFIGURATION_BLOCKED"
    else:
        status = "READY_FOR_REMAINING_CONFIGS_ARCHITECT_REVIEW"
    lines.append(f"FINAL GATE: {'ALL INTEGRITY CHECKS HOLD' if not fails else 'FAILED: ' + '; '.join(fails)}")
    lines.append(f"MISSION STATUS (from the classifications): {status if not fails else 'BLOCKED'}")
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
