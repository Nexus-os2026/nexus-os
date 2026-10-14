#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: rehearse render.py on SYNTHETIC receipts only.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19) for the revised render.py.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 18) for this mission's render.py.

    render_rehearsal.py OUTDIR TOOLCONTROLS     OUTDIR must not exist; TOOLCONTROLS: a copy of the synthetic tool-control
                                               receipts of EXEC-01 (receipts/03-tool-controls of that packet)

Temporary evidence roots OUTSIDE OUTDIR (under dev-local/, never published) hold the carried-forward expectations and
baselines, both ruling records and the tool directories (byte copies from this mission's evidence root), and SYNTHETIC
control directories in the layout of the real receipts. Three scenarios, each rendered by render.py without error:
  A  C28 a candidate (synthetic admission and pin, the synthetic type-(b) run), C12 a candidate (a), C9 a non-candidate
     (a synthetic overshoot): MUTATION_CONTROL_BLOCKED, two candidates of seven, the rest not attempted
  B  C28's pin blocked: C28_PRESERVED_ARTIFACT_BLOCKED
  C  all seven candidates: READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
BLOCKED = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED"
ART = "CONTROL RESULT: C28_PRESERVED_ARTIFACT_BLOCKED"
EV0 = K.EV


def root_for(name, tc, layout):
    root = os.path.join(EV0, "dev-local", "render-rehearsal-root-" + name)
    if os.path.exists(root):
        raise SystemExit("REFUSED: output exists")
    r = os.path.join(root, "receipts")
    os.makedirs(r)
    for c in K.CARRIED:
        src = os.path.join(EV0, c)
        (shutil.copytree if os.path.isdir(src) else shutil.copy)(src, os.path.join(root, c))
    os.makedirs(os.path.join(root, "documents"))
    for d in (K.RULING, K.C28_RULING):
        shutil.copy(os.path.join(EV0, d), os.path.join(root, d))
    for d in ("tools", "original-tools"):
        shutil.copytree(os.path.join(EV0, d), os.path.join(root, d), ignore=shutil.ignore_patterns("__pycache__"))
    for d, (run, result, pin_ok) in layout.items():
        if d.endswith("C28"):
            os.makedirs(os.path.join(r, d, "01-admission"))
            open(os.path.join(r, d, "01-admission/ADMISSION.txt"), "w").write(
                "C28 BUILD: ARCHITECT-ADMISSIBLE — NARROW C28 RULING\n(synthetic)\n")
            os.makedirs(os.path.join(r, d, "02-pin"))
            open(os.path.join(r, d, "02-pin/PIN.txt"), "w").write(
                ("PIN: PINNED — EXISTING C28 BUILD (NOT REBUILT)" if pin_ok else "PIN: C28_PRESERVED_ARTIFACT_BLOCKED")
                + "\n(synthetic)\n")
            if pin_ok:
                open(os.path.join(r, d, "02-pin/PIN.json"), "w").write(
                    '{"path": "/synthetic/nexus_governed_control-2faa0b9718d3e6a3", "sha256": "synthetic", "size": 1}\n')
        else:
            os.makedirs(os.path.join(r, d, "02-apply"))
            open(os.path.join(r, d, "02-apply/APPLY.txt"), "w").write("APPLY: EXACTLY_C_PLUS_PATCH\n(synthetic)\n")
            shutil.copytree(os.path.join(tc, "build-healthy-as-pin-ok"), os.path.join(r, d, "03-build"))
        if run:
            shutil.copytree(os.path.join(tc, run), os.path.join(r, d, "04-run"))
        os.makedirs(os.path.join(r, d, "05-restore"))
        open(os.path.join(r, d, "05-restore/RESTORE.txt"), "w").write("RESTORE: EXACTLY_C\n(synthetic)\n")
        if result == OK:
            os.makedirs(os.path.join(r, d, "06-link"))
            open(os.path.join(r, d, "06-link/LINK.txt"), "w").write("LINK: VERIFIED\ncontrol: synthetic link\n")
        open(os.path.join(r, d, "RESULT.txt"), "w").write(result + "\n(synthetic)\n")
    return root, r


def render_in(name, tc, layout, outdir):
    root, r = root_for(name, tc, layout)
    K.EV = root
    import render
    render.R = r
    out = os.path.join(outdir, "rendered-" + name)
    os.makedirs(out)
    render.main(out)
    K.EV = EV0
    return (open(os.path.join(out, "TRANCHE-SUMMARY.md")).read(), open(os.path.join(out, "INDEX.md")).read(),
            open(os.path.join(out, "CARRIED-FORWARD.md")).read())


def main(outdir, tc):
    if os.path.exists(outdir):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(outdir)
    a, ai, acf = render_in("A", tc, {"36-C28": ("run-b-c14-own-check", OK, True),
                                     "37-C12": ("run-a-C12-x1-fail-E2", OK, True),
                                     "38-C9": ("run-a-neg-fb2-overshoot", BLOCKED, True)}, outdir)
    b, bi, _ = render_in("B", tc, {"36-C28": (None, ART, False)}, outdir)
    full = {"36-C28": ("run-b-c14-own-check", OK, True), "37-C12": ("run-a-C12-x1-fail-E2", OK, True)}
    full.update({d: ("run-a-C9-fb2-fail-E4", OK, True) for d in ("38-C9", "39-C11", "40-C17")})
    full.update({d: ("run-a-C18a-fg-fail-E7", OK, True) for d in ("41-C18a", "42-C18b")})
    c, ci, _ = render_in("C", tc, full, outdir)
    n_unchanged = sum(1 for t in os.listdir(os.path.join(EV0, "original-tools"))
                      if os.path.exists(os.path.join(EV0, "tools", t))
                      and K.sha256_file(os.path.join(EV0, "tools", t)) == K.sha256_file(os.path.join(EV0, "original-tools", t)))
    checks = [("A: status MUTATION_CONTROL_BLOCKED", "**MUTATION_CONTROL_BLOCKED**" in a),
              ("A: the five Architect-accepted controls rendered with RESUME-01 / EXEC-01 references",
               "**C22, C14, C19, C21, C27 — MUTATION CONTROL KILL ACCEPTED**" in a
               and a.count(K.RESUME_PACKET + "/receipts/3") >= 16 and K.PREV_PACKET + "/receipts/31-C22/RESULT.txt" in a),
              ("A: C28 build evidence referenced, never rebuilt", "NOT REPEATED — the existing RESUME-01 build" in a
               and K.RESUME_PACKET + "/receipts/36-C28/03-build/build.stderr" in a),
              ("A: two new candidates of seven rendered", "**2** of the 7 remaining controls (C28, C12)" in a),
              ("A: C28 row: panic location rendered", "### C28" in a and "panic at" in a),
              ("A: C12 row: discovered count of the 250,000 ceiling", "### C12" in a and "of 250,000" in a),
              ("A: C9 row: non-candidate with its outer class", "### C9" in a and BLOCKED in a and "BUDGET EXCEEDED" in a),
              ("A: not attempted listed", "not attempted (a stop ends the tranche): C11, C17, C18a, C18b" in a),
              ("A: X-1 and T-RES baselines and the Stage-2 links rendered", a.count(K.PREV_PACKET + "/receipts/1") == 2
               and "61-full-F-B2" in a and "60-full-F-G" in a),
              ("A: CARRIED-FORWARD lists the carried copies, the RESUME-01 files and every unchanged tool",
               acf.count("receipts/36-C28/") >= 15 and acf.count("| `") >= 22 + 15 + 7 + n_unchanged),
              ("A: INDEX names the status and both previous evidence commits", "MUTATION_CONTROL_BLOCKED" in ai
               and K.PREV in ai and K.RESUME in ai),
              ("B: status C28_PRESERVED_ARTIFACT_BLOCKED", "**C28_PRESERVED_ARTIFACT_BLOCKED**" in b
               and "C28_PRESERVED_ARTIFACT_BLOCKED" in bi and "PIN: C28_PRESERVED_ARTIFACT_BLOCKED" in b),
              ("C: status READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW with seven candidates",
               "**READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW**" in c
               and "**7** of the 7 remaining controls (C28, C12, C9, C11, C17, C18a, C18b)" in c),
              ("no rendering claims a final acceptance by the executor", all(
                  "ALL MUTATIONS COMPLETE" not in x and "D2A-1 ACCEPTED" not in x and "PHASE THREE COMPLETE" not in x
                  for x in (a, ai, b, bi, c, ci)))]
    lines = [f"{'OK ' if ok else 'BAD'} {n}" for n, ok in checks]
    lines.append("RENDER REHEARSAL: " + ("ALL AS EXPECTED" if all(ok for _, ok in checks) else "FAILED"))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if all(ok for _, ok in checks) else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
