#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: rehearse render.py on SYNTHETIC receipts only.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19) for the revised render.py.

    render_rehearsal.py OUTDIR TOOLCONTROLS     OUTDIR must not exist; TOOLCONTROLS: a copy of the synthetic tool-control
                                               receipts of EXEC-01 (receipts/03-tool-controls of the previous packet)

A temporary evidence root under OUTDIR holds the carried-forward expectations, baselines, healthy build and C22
receipts and the C22 ruling record (byte copies from this mission's evidence root), and three SYNTHETIC control
directories in the layout of the real receipts: C14 (b) and C12 (a) as candidates, C9 (a) as a non-candidate whose
run is a synthetic overshoot. render.py's functions are pointed at it and must render every row without error, with
the derived status MUTATION_CONTROL_BLOCKED (two new candidates of eleven), C22 rendered as Architect-accepted with
its previous-packet references, and every carried-forward file listed with its previous blob and sha256.
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
BLOCKED = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED"


def main(outdir, tc):
    if os.path.exists(outdir):
        raise SystemExit("REFUSED: output exists")
    root = os.path.join(outdir, "ev")
    r = os.path.join(root, "receipts")
    os.makedirs(r)
    for c in K.CARRIED:
        src = os.path.join(K.EV, c)
        (shutil.copytree if os.path.isdir(src) else shutil.copy)(src, os.path.join(root, c))
    os.makedirs(os.path.join(root, "documents"))
    shutil.copy(os.path.join(K.EV, K.C22_RULING), os.path.join(root, K.C22_RULING))
    layout = {"32-C14": ("build-healthy-as-pin-ok", "run-b-c14-own-check", OK),
              "37-C12": ("build-healthy-as-pin-ok", "run-a-C12-x1-fail-E2", OK),
              "38-C9": ("build-healthy-as-pin-ok", "run-a-neg-fb2-overshoot", BLOCKED)}
    for d, (b, run, result) in layout.items():
        os.makedirs(os.path.join(r, d, "02-apply"))
        open(os.path.join(r, d, "02-apply/APPLY.txt"), "w").write("APPLY: EXACTLY_C_PLUS_PATCH\n(synthetic)\n")
        shutil.copytree(os.path.join(tc, b), os.path.join(r, d, "03-build"))
        shutil.copytree(os.path.join(tc, run), os.path.join(r, d, "04-run"))
        os.makedirs(os.path.join(r, d, "05-restore"))
        open(os.path.join(r, d, "05-restore/RESTORE.txt"), "w").write("RESTORE: EXACTLY_C\n(synthetic)\n")
        if result == OK:
            os.makedirs(os.path.join(r, d, "06-link"))
            open(os.path.join(r, d, "06-link/LINK.txt"), "w").write("LINK: VERIFIED\ncontrol: synthetic link\n")
        open(os.path.join(r, d, "RESULT.txt"), "w").write(result + "\n(synthetic)\n")
    K.EV = root
    import render
    render.R = r
    out = os.path.join(outdir, "rendered")
    os.makedirs(out)
    render.main(out)
    s = open(os.path.join(out, "TRANCHE-SUMMARY.md")).read()
    i = open(os.path.join(out, "INDEX.md")).read()
    cf = open(os.path.join(out, "CARRIED-FORWARD.md")).read()
    n_carried = sum(len(fs) for c in K.CARRIED if os.path.isdir(os.path.join(root, c))
                    for _, _, fs in os.walk(os.path.join(root, c))) + 1 + len(K.CARRIED_DOCS)
    checks = [("status MUTATION_CONTROL_BLOCKED", "**MUTATION_CONTROL_BLOCKED**" in s),
              ("C22 rendered as Architect-accepted, with previous-packet blob references",
               "**C22 — MUTATION CONTROL KILL ACCEPTED**" in s and K.PREV_PACKET + "/receipts/31-C22/03-build/build.stderr" in s),
              ("two new candidates of the eleven rendered", "**2** of the 11 remaining controls (C14, C12)" in s),
              ("C14 row: panic location rendered", "### C14" in s and "panic at" in s),
              ("C12 row: discovered count of the 250,000 ceiling", "### C12" in s and "of 250,000" in s),
              ("C9 row: non-candidate with its outer class", "### C9" in s and BLOCKED in s and "BUDGET EXCEEDED" in s),
              ("not attempted listed", "C19, C21, C27, C28, C11, C17, C18a, C18b" in s),
              ("baselines rendered as previous-packet references", s.count(K.PREV_PACKET + "/receipts/1") == 6),
              ("every carried-forward file listed with blob and sha256", cf.count("\n| `") == n_carried),
              ("INDEX names the status and the previous evidence commit", "MUTATION_CONTROL_BLOCKED" in i and K.PREV in i)]
    lines = [f"{'OK ' if ok else 'BAD'} {n}" for n, ok in checks]
    lines.append("RENDER REHEARSAL: " + ("ALL AS EXPECTED" if all(ok for _, ok in checks) else "FAILED"))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if all(ok for _, ok in checks) else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
