#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission section 11: the frozen final gate, run once before staging
(read-only for the repository; no Cargo, no rustc, no build, no test).

    final_gate.py ROOT

Requires every packet file to equal the freeze record, then re-runs every tool from scratch and requires
byte-identical results: authentication, two fresh derivations, both checkers, the syntax check, the negative
controls and the documents. Then checks, independently of the checkers: Candidate C unchanged; the R1 evidence (commit,
tree, local branch, worktree) unchanged; 34 patches byte-identical to the R1 commit's patch blobs and A6's different;
the absence of every Architect-defined mutant blob (R1's twenty and R2's A6) from the object store; the pinned
executable's identity; the packet wording; the INDEX's cited values. Writes ROOT/receipts/60-gate-authentication.txt;
prints its own receipt (kept as ROOT/receipts/60-final-gate.txt). Exit 0 only if every check holds.
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
R1 = "f81159046bf0b62666851bc1185992318302d31f"
R1_TREE = "234ca0990434485f59ebda0ceff3086d4e66d797"
R1_BRANCH = "refs/heads/evidence/p3-c11-d2a1-control-definitions-r1-01"
R1_WT = "/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-control-definitions-r1-01-evidence"
R1_PKT = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/"
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


def git(*args, cwd=REPO):
    return subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True)


def main(root):
    rec = os.path.join(root, "receipts")
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01 — FINAL GATE (frozen; run once; no build, no test)")
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
            rr = run([os.path.join(HERE, "derive_catalog.py"), root, os.path.join(d, "catalog")])
            outs.append((d, rr.returncode))
        check("G2 two fresh derivations (catalog and comparison) are byte-identical",
              outs[0][1] == outs[1][1] == 0 and same_dir(outs[0][0], outs[1][0]), f"{len(list(walk(outs[0][0])))} files")
        check("G2 the derivation equals the catalog and comparison to be published",
              same_dir(os.path.join(outs[0][0], "catalog"), os.path.join(root, "out", "catalog")) and
              same_dir(os.path.join(outs[0][0], "comparison"), os.path.join(root, "out", "comparison")))
        for name, tool, receipt in (("checker A", "checker_a.py", "20-checker-a.txt"),
                                    ("checker B", "checker_b.py", "21-checker-b.txt"),
                                    ("syntax check", "syntax_check.py", "30-syntax-check.txt")):
            rr = run([os.path.join(HERE, tool), root])
            check(f"G3 {name} re-run: exit 0 and output byte-identical to receipts/{receipt}",
                  rr.returncode == 0 and rr.stdout == open(os.path.join(rec, receipt)).read())
        va = [l for l in open(os.path.join(rec, "20-checker-a.txt")) if l.startswith("VERDICTS ")]
        vb = [l for l in open(os.path.join(rec, "21-checker-b.txt")) if l.startswith("VERDICTS ")]
        vm = json.loads(va[0][9:]) if va else {}
        check("G3 the two checkers' per-id verdict maps are identical and all PASS (checker agreement)",
              va == vb and len(vm) == 38 and set(vm.values()) == {"PASS"})
        syn = open(os.path.join(rec, "30-syntax-check.txt")).read()
        check("G3 all 21 SYNTAX_CHECKED_NOT_TYPECHECKED (A6 in its R2 form)",
              all(re.search(rf"^OK   {c}: .*-> SYNTAX_CHECKED_NOT_TYPECHECKED$", syn, re.M) for c in NEW))
        rr = run([os.path.join(HERE, "negative_controls.py"), root])
        check("G4 negative controls re-run: exit 0 and output byte-identical to receipts/40-negative-controls.txt",
              rr.returncode == 0 and rr.stdout == open(os.path.join(rec, "40-negative-controls.txt")).read())
        neg = open(os.path.join(rec, "40-negative-controls.txt")).read()
        r2seeds = re.findall(r"^OK   seed: R2-", neg, re.M)
        check("G4 42 seeds (R1's 29, R2's 13), each detected by both checkers; positive control agrees",
              neg.count("\nOK   seed: ") == 42 and len(r2seeds) == 13 and
              "seeds 42; undetected or disagreeing 0" in neg and "OK   positive control" in neg)
        droot = os.path.join(tmp, "docs-root")
        for d in ("tools", "receipts", os.path.join("inputs", "r1")):
            shutil.copytree(os.path.join(root, d), os.path.join(droot, d), ignore=shutil.ignore_patterns("__pycache__"))
        for d in ("catalog", "comparison"):
            shutil.copytree(os.path.join(root, "out", d), os.path.join(droot, "out", d))
        rr = run([os.path.join(HERE, "render_docs.py"), droot])
        ri = run([os.path.join(HERE, "render_index.py"), droot])
        check("G5 documents and INDEX re-rendered byte-identical", rr.returncode == 0 and ri.returncode == 0 and
              same_dir(os.path.join(droot, "out", "docs"), os.path.join(root, "out", "docs")) and
              open(os.path.join(droot, "docs-src", "INDEX.md"), "rb").read() ==
              open(os.path.join(root, "docs-src", "INDEX.md"), "rb").read())
    finally:
        shutil.rmtree(tmp)
    print("[baseline, R1 evidence, repository and executable]")
    check("G6 Candidate C unchanged (commit, tree)", git("cat-file", "-t", C).stdout.strip() == "commit" and
          git("rev-parse", C + "^{tree}").stdout.strip() == C_TREE)
    check("G6 the owner checkout is untouched: HEAD, branch and a clean status",
          git("rev-parse", "HEAD").stdout.strip() == OWNER_HEAD and
          git("symbolic-ref", "--short", "HEAD").stdout.strip() == OWNER_BRANCH and
          git("status", "--porcelain").stdout == "")
    check("G6 the owner checkout's index has nothing staged", git("diff", "--cached", "--name-only").stdout == "")
    check("G7 R1 evidence unchanged: commit tree, local branch, worktree HEAD and a clean worktree",
          git("rev-parse", R1 + "^{tree}").stdout.strip() == R1_TREE and
          git("rev-parse", R1_BRANCH).stdout.strip() == R1 and git("rev-parse", "HEAD", cwd=R1_WT).stdout.strip() == R1
          and git("status", "--porcelain", cwd=R1_WT).stdout == "")
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    by = {e["id"]: e for e in cat["controls"]}
    ls = {l.split("\t")[1].rsplit("/", 1)[1]: l.split()[2] for l in
          git("ls-tree", R1, R1_PKT + "catalog/patches/").stdout.splitlines()}
    same, differ = [], []
    for cid, e in by.items():
        if cid == "A4":
            continue
        data = open(os.path.join(root, "out", "catalog", "patches", f"{cid}.patch"), "rb").read()
        r1data = subprocess.run(["git", "-C", REPO, "cat-file", "blob", ls[f"{cid}.patch"]], capture_output=True).stdout
        (same if data == r1data else differ).append(cid)
    check("G8 34 patches byte-identical to the R1 commit's patch blobs; A6's the only one different",
          len(same) == 34 and differ == ["A6"], f"same {len(same)}, differ {differ}")
    r1cat = json.loads(subprocess.run(["git", "-C", REPO, "show", f"{R1}:{R1_PKT}catalog/catalog.json"],
                                      capture_output=True).stdout)
    r1by = {e["id"]: e for e in r1cat["controls"]}
    check("G8 35 entries (34 controls and A4) equal R1's; A6's differs", sorted(c for c in by if by[c] != r1by[c]) ==
          ["A6"] and len(by) == 36)
    blobs = {c: by[c]["mutant_blob"] for c in NEW} | {"A6 (R1)": r1by["A6"]["mutant_blob"]}
    present = [c for c, b in blobs.items() if subprocess.run(["git", "-C", REPO, "cat-file", "-e", b],
                                                             capture_output=True).returncode == 0]
    check("G9 no Architect-defined mutant blob (R1's 21, R2's A6) exists in the object store (never written, staged "
          "or committed)", not present, present)
    st = os.stat(EXE)
    check("G9 pinned baseline executable untouched (sha256, size, inode, mtime as Stage 2 recorded)",
          sha(EXE) == EXE_SHA and st.st_size == 8984464 and st.st_ino == EXE_INODE and st.st_mtime_ns == EXE_MTIME_NS)
    check("G9 counts: 35 kill-required controls plus A4; no blocked definition; nothing executed",
          cat["counts"]["kill_required"] == 35 and cat["counts"]["comparison"] == 1 and cat["counts"]["blocked"] == 0
          and all(e.get("definition_status") == "CANDIDATE_READY" and e.get("execution_status") == "NOT_ATTEMPTED"
                  for c, e in by.items() if c != "A4"))
    print("[wording and cited values]")
    docs = [os.path.join(root, "docs-src", "INDEX.md")] + list(walk(os.path.join(root, "out", "docs"))) + \
        list(walk(os.path.join(root, "out", "catalog"))) + list(walk(os.path.join(root, "out", "comparison")))
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
    check("G10 no forbidden status wording; 'architect accepted' only for A4", not bad, bad[:4])
    index = open(os.path.join(root, "docs-src", "INDEX.md")).read()
    cblob = subprocess.run(["git", "hash-object", os.path.join(root, "out", "catalog", "catalog.json")],
                           capture_output=True, text=True).stdout.strip()
    dpath = os.path.join(root, "out", "docs", "ARCHITECT-CONTROL-DEFINITIONS-R2.md")
    dblob = subprocess.run(["git", "hash-object", dpath], capture_output=True, text=True).stdout.strip()
    defs = open(dpath).read()
    cited = {"catalog sha256": sha(os.path.join(root, "out", "catalog", "catalog.json")), "catalog blob": cblob,
             "definitions record sha256": sha(dpath), "definitions record blob": dblob,
             "version": "d2a1-controls-v1-r2-candidate", "status": "READY_FOR_CONTROL_DEFINITION_R2_ARCHITECT_REVIEW",
             "C22": "PENDING_EXACT_COMMAND_HEALTHY_BUILD",
             "R1 catalog sha256": "a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479"}
    miss = [k for k, v in cited.items() if v not in index]
    check("G11 INDEX.md cites the catalog's and the definitions record's sha256 and blob, the version, the status, "
          "C22's status and the R1 catalog", not miss, miss)
    check("G11 INDEX.md cites all 35 patch sha256 values", all(e["patch_sha256"] in index for e in cat["controls"]
                                                                if e["id"] != "A4"))
    check("G11 the definitions record is labelled and states the A6 points of mission section 8",
          "**ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING**" in defs and
          all(s in defs for s in ("The semantic mutation is unchanged from R1.",
                                  "recovery_required is part of the state identity.",
                                  "KEY_FIELDS now names recovery_required.",
                                  "The new hunk repairs raw receipt completeness.",
                                  "The hunk does not alter executable model semantics beyond the already-defined A6 "
                                  "mutation.")))
    print()
    print(f"failing {len(fails)}")
    print("FINAL GATE: " + ("PASS" if not fails else "FAIL"))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
