#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: rehearse render.py on SYNTHETIC receipts only.

    render_rehearsal.py OUTDIR TOOLCONTROLS     OUTDIR must not exist

A temporary evidence root under OUTDIR is populated with tool_controls.py's synthetic receipts in the layout of the
real receipts (baselines, healthy build, and three control directories: C22 (d), C14 (b), C12 (a)); render.py's
functions are pointed at it and must render every row without error, with the derived status
MUTATION_CONTROL_BLOCKED (three of twelve). Nothing real is read but the expectations.
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"


def main(outdir, tc):
    if os.path.exists(outdir):
        raise SystemExit("REFUSED: output exists")
    root = os.path.join(outdir, "ev")
    r = os.path.join(root, "receipts")
    os.makedirs(r)
    shutil.copy(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json"), r)
    shutil.copytree(os.path.join(tc, "run-e1-x1-pass"), os.path.join(r, "10-baseline-X-1"))
    for n, key in enumerate(["T-BIND", "T-CELL", "W6", "T-PREC", "T-RES"], start=11):
        shutil.copytree(os.path.join(tc, "run-e2-tbind-pass"), os.path.join(r, f"{n}-baseline-{key}"))
    shutil.copytree(os.path.join(tc, "build-healthy-as-e4"), os.path.join(r, "20-healthy-build"))
    layout = {"31-C22": ("build-c22", None), "32-C14": ("build-healthy-as-pin-ok", "run-b-c14-own-check"),
              "37-C12": ("build-healthy-as-pin-ok", "run-a-C12-x1-fail-E2")}
    for d, (b, run) in layout.items():
        os.makedirs(os.path.join(r, d, "02-apply"))
        open(os.path.join(r, d, "02-apply/APPLY.txt"), "w").write("APPLY: EXACTLY_C_PLUS_PATCH\n(synthetic)\n")
        shutil.copytree(os.path.join(tc, b), os.path.join(r, d, "03-build"))
        if run:
            shutil.copytree(os.path.join(tc, run), os.path.join(r, d, "04-run"))
        os.makedirs(os.path.join(r, d, "05-restore"))
        open(os.path.join(r, d, "05-restore/RESTORE.txt"), "w").write("RESTORE: EXACTLY_C\n(synthetic)\n")
        os.makedirs(os.path.join(r, d, "06-link"))
        open(os.path.join(r, d, "06-link/LINK.txt"), "w").write("LINK: VERIFIED\ncontrol: synthetic link\n")
        open(os.path.join(r, d, "RESULT.txt"), "w").write(OK + "\n(synthetic)\n")
    K.EV = root
    import render
    render.R = r
    out = os.path.join(outdir, "rendered")
    os.makedirs(out)
    render.main(out)
    s = open(os.path.join(out, "TRANCHE-SUMMARY.md")).read()
    i = open(os.path.join(out, "INDEX.md")).read()
    checks = [("status MUTATION_CONTROL_BLOCKED", "**MUTATION_CONTROL_BLOCKED**" in s),
              ("three candidates rendered", "**3** of 12" in s),
              ("C22 row: NOT PRODUCED BY DESIGN executable and E0283 named", "### C22" in s
               and "mutant executable: NOT PRODUCED BY DESIGN" in s and "E0283 at" in s),
              ("C14 row: panic location rendered", "### C14" in s and "panic at" in s),
              ("C12 row: discovered count of the 250,000 ceiling", "### C12" in s and "of 250,000" in s),
              ("not attempted listed", "C19, C21, C27, C28, C9, C11, C17, C18a, C18b" in s),
              ("X-1 baseline facts rendered", "branch accounting NORMALIZED" in s),
              ("INDEX names the status", "MUTATION_CONTROL_BLOCKED" in i)]
    lines = [f"{'OK ' if ok else 'BAD'} {n}" for n, ok in checks]
    lines.append("RENDER REHEARSAL: " + ("ALL AS EXPECTED" if all(ok for _, ok in checks) else "FAILED"))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if all(ok for _, ok in checks) else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
