#!/usr/bin/env python3
"""Produces every pre-freeze output of the mission into OUTDIR (receipts and
packet files), by running the tools in order. Read-only with respect to the
repository: it builds nothing, runs no test and no model.

  prepare.py OUTDIR

OUTDIR/receipts/NN-*.txt     each tool's console output, with its exit status
OUTDIR/packet/               the packet's documents, matrices and checker outputs
Stops at the first tool that fails.
"""
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
PY = sys.executable


def step(out, name, cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    text = r.stdout + r.stderr
    shown = [c.replace(os.path.abspath(out), "<OUT>").replace(ROOT, "<ROOT>").replace(PY, "python3") for c in cmd]
    with open(os.path.join(out, "receipts", name), "w") as fh:
        fh.write(f"$ {' '.join(shown)}\n")
        fh.write(text)
        fh.write(f"exit status {r.returncode}\n")
    print(f"{name}: exit {r.returncode}")
    if r.returncode != 0:
        print(text[-3000:])
        raise SystemExit(f"STOP at {name}")
    return text


def main(out):
    out = os.path.abspath(out)
    if os.path.exists(out):
        raise SystemExit(f"REFUSED: {out} exists")
    pk = os.path.join(out, "packet")
    ck = os.path.join(pk, "checks")
    for d in (os.path.join(out, "receipts"), ck, os.path.join(out, "work")):
        os.makedirs(d)
    w = os.path.join(out, "work")
    step(out, "00-authentication.txt", [PY, f"{HERE}/authenticate.py", "--remote"])
    step(out, "10-checker-a-occurrences.txt", [PY, f"{HERE}/checker_a.py", "occurrences", f"{w}/a"])
    step(out, "11-checker-a-closure.txt", [PY, f"{HERE}/checker_a.py", "closure", f"{w}/a"])
    step(out, "12-checker-b.txt", [PY, f"{HERE}/checker_b.py", f"{w}/b"])
    step(out, "13-agreement.txt", [PY, f"{HERE}/agree.py", f"{w}/a", f"{w}/b"])
    step(out, "14-branch-disposition.txt", [PY, f"{HERE}/branch_disposition.py", f"{w}/bd"])
    step(out, "15-render-proof.txt", [PY, f"{HERE}/render.py", f"{ROOT}/docs-src/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.src.md",
                                      f"{pk}/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"])
    shutil.copyfile(f"{ROOT}/docs-src/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.src.md",
                    f"{pk}/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md")
    shutil.copyfile(f"{ROOT}/docs-src/C15-DISPOSITION.src.md", f"{pk}/C15-DISPOSITION.md")
    for f in ("FR-BRANCH-DISPOSITION.tsv", "FR-BRANCH-DISPOSITION-SUMMARY.json"):
        shutil.copyfile(f"{w}/bd/{f}", f"{pk}/{f}")
    step(out, "16-accounting.txt", [PY, f"{HERE}/accounting.py", f"{ROOT}/docs-src/PROPOSED-20-CONFIGURATION-ACCOUNTING.template.md",
                                    f"{pk}/PROPOSED-20-CONFIGURATION-ACCOUNTING.md", f"{ck}/ACCOUNTING-DATA.json",
                                    f"{w}/bd/FR-BRANCH-DISPOSITION-SUMMARY.json"])
    docs = [f"{pk}/{f}" for f in ("FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md", "P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md",
                                  "PROPOSED-20-CONFIGURATION-ACCOUNTING.md", "C15-DISPOSITION.md")]
    step(out, "17-cite-checker-a.txt", [PY, f"{HERE}/checker_a.py", "cite", *docs, "--matrix", f"{pk}/SOURCE-ITEM-IDENTITY-MATRIX.tsv"])
    step(out, "18-cite-checker-b.txt", [PY, f"{HERE}/checker_b.py", "cite", *docs])
    step(out, "19-doc-check.txt", [PY, f"{HERE}/doc_check.py", pk, "--summary", f"{pk}/FR-BRANCH-DISPOSITION-SUMMARY.json"])
    step(out, "20-controls.txt", [PY, f"{HERE}/controls.py", f"{w}/controls", pk, f"{pk}/FR-BRANCH-DISPOSITION-SUMMARY.json"])
    for f in ("REPLAY-OCCURRENCE-MATRIX.tsv", "REPLAY-OCCURRENCE-MATRIX.md"):
        shutil.copyfile(f"{w}/a/{f}", f"{pk}/{f}")
    for f in ("A-occurrences.json", "A-occurrence-summary.json", "A-explore-closure.txt", "A-closure-occurrences.txt"):
        shutil.copyfile(f"{w}/a/{f}", f"{ck}/{f}")
    for f in ("B-checks.txt", "B-occurrences.json", "B-explore-closure.txt"):
        shutil.copyfile(f"{w}/b/{f}", f"{ck}/{f}")
    print("PREPARE: every step exit 0")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
