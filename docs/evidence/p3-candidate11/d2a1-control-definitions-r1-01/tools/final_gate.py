#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 9, 10, 13 and 16: the final gate before staging
(read-only for the repository; no Cargo, no rustc, no build, no test).

    final_gate.py ROOT

Re-runs every tool from scratch and requires byte-identical results: authentication, two fresh derivations, both
checkers, the syntax check, the negative controls and the documents. Then checks the frozen baseline, the absence
of every new mutant blob from the object store, the pinned executable's identity, the packet wording and the INDEX's
cited values. Writes ROOT/receipts/60-gate-authentication.txt; prints its own receipt (kept as
ROOT/receipts/60-final-gate.txt). Exit 0 only if every check holds.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
OWNER_HEAD = "f727f5c39fab8d5c729a55eb28ad576d3d56ce47"
OWNER_BRANCH = "rebuild/phase0-trust-boundary"
EXE = ("/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/"
       "nexus_governed_control-2faa0b9718d3e6a3")
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_MTIME_NS = 1791979773968059672
EXE_INODE = 62417761
HERE = os.path.dirname(os.path.abspath(__file__))
NEW = ["C3a", "C3b", "C3c", "C5", "C6", "C7", "C8", "C11", "C15", "C16", "C17", "C18a", "C18b", "C20", "C23", "C26",
       "A1", "A2", "A3", "A5", "A6"]
FORBIDDEN = ["CATALOG ACCEPTED", "CATALOG FROZEN", "MUTATION CAMPAIGN STARTED", "ALL MUTATIONS COMPLETE",
             "D2A-1 ACCEPTED", "INTEGRATED", "PHASE THREE COMPLETE", "READY FOR EXECUTION"]
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def run(args, out=None):
    r = subprocess.run([sys.executable, *args], capture_output=True, text=True)
    if out:
        with open(out, "w") as fh:
            fh.write(r.stdout + r.stderr)
    return r


def walk(base):
    for b, dirs, names in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for n in sorted(names):
            yield os.path.join(b, n)


def same_dir(a, b):
    la = sorted(os.path.relpath(p, a) for p in walk(a))
    lb = sorted(os.path.relpath(p, b) for p in walk(b))
    return la == lb and all(open(os.path.join(a, r), "rb").read() == open(os.path.join(b, r), "rb").read()
                            for r in la)


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, text=True)


def main(root):
    rec = os.path.join(root, "receipts")
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01 — FINAL GATE (before staging; no build, no test)")
    fr = json.load(open(os.path.join(rec, "50-FREEZE-RECORD.json")))["files"]
    now = {r: sha(os.path.join(root, r)) for r in fr}
    check("G0 every packet file equals the freeze record", now == fr, f"{len(fr)} files")
    r = run([os.path.join(HERE, "authenticate.py"), root], os.path.join(rec, "60-gate-authentication.txt"))
    check("G1 input authentication re-run: ALL CHECKS OK", r.returncode == 0 and "RESULT: ALL CHECKS OK" in r.stdout)
    tmp = tempfile.mkdtemp(dir=os.path.join(root, "dev-local"), prefix="gate-")
    try:
        outs = []
        for k in (1, 2):
            d = os.path.join(tmp, f"derive{k}")
            rr = run([os.path.join(HERE, "derive_catalog.py"), root, d])
            outs.append((d, rr.returncode))
        check("G2 two fresh derivations are byte-identical", outs[0][1] == outs[1][1] == 0 and
              same_dir(outs[0][0], outs[1][0]), f"{len(list(walk(outs[0][0])))} files")
        check("G2 the derivation equals the catalog to be published", same_dir(outs[0][0],
                                                                               os.path.join(root, "out", "catalog")))
        for name, tool, receipt in (("checker A", "checker_a.py", "20-checker-a.txt"),
                                    ("checker B", "checker_b.py", "21-checker-b.txt"),
                                    ("syntax check", "syntax_check.py", "30-syntax-check.txt")):
            rr = run([os.path.join(HERE, tool), root])
            check(f"G3 {name} re-run: exit 0 and output byte-identical to receipts/{receipt}",
                  rr.returncode == 0 and rr.stdout == open(os.path.join(rec, receipt)).read())
        va = [l for l in open(os.path.join(rec, "20-checker-a.txt")) if l.startswith("VERDICTS ")]
        vb = [l for l in open(os.path.join(rec, "21-checker-b.txt")) if l.startswith("VERDICTS ")]
        vm = json.loads(va[0][9:]) if va else {}
        check("G3 the two checkers' per-id verdict maps are identical and all PASS", va == vb and len(vm) == 38 and
              set(vm.values()) == {"PASS"})
        syn = open(os.path.join(rec, "30-syntax-check.txt")).read()
        check("G3 all 21 SYNTAX_CHECKED_NOT_TYPECHECKED", all(re.search(rf"^OK   {c}: .*-> SYNTAX_CHECKED_NOT_"
                                                                     rf"TYPECHECKED$", syn, re.M) for c in NEW))
        rr = run([os.path.join(HERE, "negative_controls.py"), root])
        check("G4 negative controls re-run: exit 0 and output byte-identical to receipts/40-negative-controls.txt",
              rr.returncode == 0 and rr.stdout == open(os.path.join(rec, "40-negative-controls.txt")).read())
        neg = open(os.path.join(rec, "40-negative-controls.txt")).read()
        check("G4 29 seeds, each detected by both checkers; positive control agrees",
              neg.count("\nOK   seed: ") == 29 and "seeds 29; undetected or disagreeing 0" in neg and
              "OK   positive control" in neg)
        droot = os.path.join(tmp, "docs-root")
        for d in ("tools", "receipts"):
            shutil.copytree(os.path.join(root, d), os.path.join(droot, d), ignore=shutil.ignore_patterns("__pycache__"))
        shutil.copytree(os.path.join(root, "out", "catalog"), os.path.join(droot, "out", "catalog"))
        rr = run([os.path.join(HERE, "render_docs.py"), droot])
        check("G5 documents re-rendered byte-identical", rr.returncode == 0 and
              same_dir(os.path.join(droot, "out", "docs"), os.path.join(root, "out", "docs")))
    finally:
        shutil.rmtree(tmp)
    print("[baseline, repository and executable]")
    check("G6 Candidate C unchanged (commit, tree)", git("cat-file", "-t", C).stdout.strip() == "commit" and
          git("rev-parse", C + "^{tree}").stdout.strip() == C_TREE)
    check("G6 the owner checkout is untouched: HEAD, branch and a clean status",
          git("rev-parse", "HEAD").stdout.strip() == OWNER_HEAD and
          git("symbolic-ref", "--short", "HEAD").stdout.strip() == OWNER_BRANCH and git("status", "--porcelain").stdout == "")
    check("G6 the owner checkout's index has nothing staged", git("diff", "--cached", "--name-only").stdout == "")
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    by = {e["id"]: e for e in cat["controls"]}
    present = [c for c in NEW if subprocess.run(["git", "-C", REPO, "cat-file", "-e", by[c]["mutant_blob"]],
                                                capture_output=True).returncode == 0]
    check("G7 none of the 21 new mutant blobs exists in the object store (never written, staged or committed)",
          not present, present)
    old = [c for c in cat["carried_forward"] if subprocess.run(["git", "-C", REPO, "cat-file", "-e",
                                                                by[c]["mutant_blob"]], capture_output=True).returncode == 0]
    print(f"     info: carried-forward mutant blobs already in the object store from earlier missions: {old or 'none'}")
    st = os.stat(EXE)
    check("G8 pinned baseline executable untouched (sha256, size, inode, mtime as Stage 2 recorded)",
          sha(EXE) == EXE_SHA and st.st_size == 8984464 and st.st_ino == EXE_INODE and st.st_mtime_ns == EXE_MTIME_NS)
    print("[wording and cited values]")
    docs = [os.path.join(root, "docs-src", "INDEX.md")] + list(walk(os.path.join(root, "out", "docs"))) + \
        list(walk(os.path.join(root, "out", "catalog")))
    bad = []
    for p in docs:
        t = open(p).read()
        for w in FORBIDDEN:
            if w in t:
                bad.append((os.path.relpath(p, root), w))
        for l in t.splitlines():
            if re.search(r"architect accepted", l, re.I) and not re.search(r"\bA4\b|\"status\": \"SATISFIED", l):
                bad.append((os.path.relpath(p, root), l[:80]))
            if re.search(r"catalog\W+(is\W+)?(frozen|accepted)|(frozen|accepted)\W+catalog", l, re.I):
                bad.append((os.path.relpath(p, root), l[:80]))
    check("G9 no forbidden status wording; 'architect accepted' only for A4", not bad, bad[:4])
    index = open(os.path.join(root, "docs-src", "INDEX.md")).read()
    cblob = subprocess.run(["git", "hash-object", os.path.join(root, "out", "catalog", "catalog.json")],
                           capture_output=True, text=True).stdout.strip()
    dpath = os.path.join(root, "out", "docs", "ARCHITECT-CONTROL-DEFINITIONS-R1.md")
    dblob = subprocess.run(["git", "hash-object", dpath], capture_output=True, text=True).stdout.strip()
    cited = {"catalog sha256": sha(os.path.join(root, "out", "catalog", "catalog.json")), "catalog blob": cblob,
             "definitions record sha256": sha(dpath), "definitions record blob": dblob,
             "version": "d2a1-controls-v1-r1-candidate", "status": "READY_FOR_CONTROL_DEFINITION_ARCHITECT_REVIEW",
             "C22": "PENDING_EXACT_COMMAND_HEALTHY_BUILD"}
    miss = [k for k, v in cited.items() if v not in index]
    check("G10 INDEX.md cites the catalog's and the definitions record's sha256 and blob, the version, the status "
          "and C22's status", not miss, miss)
    check("G10 INDEX.md cites all 35 patch sha256 values", all(e["patch_sha256"] in index for e in cat["controls"]
                                                                if e["id"] != "A4"))
    print()
    print(f"failing {len(fails)}")
    print("FINAL GATE: " + ("PASS" if not fails else "FAIL"))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
