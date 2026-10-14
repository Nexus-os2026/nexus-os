#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01: the final gate before the blocker packet is staged (read-only for Nexus).

    final_gate.py ROOT

Re-runs every tool and checks that the packet's contents are reproducible and that nothing was built or run.
Writes ROOT/receipts/20-static-gate.txt, 21-gate-controls.txt, 60-gate-authentication.txt and prints its own
receipt (kept as ROOT/receipts/60-final-gate.txt). Exit 0 only if every check holds; the expected state is
CONTROL_CATALOG_BLOCKED, so the static gate itself is required to FAIL in exactly its 21 definition checks.
"""
import filecmp
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
OWNER_HEAD = "f727f5c39fab8d5c729a55eb28ad576d3d56ce47"
BASE_WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"
EVIDENCE_WT = "/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-mutation-tranche1-01-evidence"
EXE = BASE_WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_MTIME_NS = 1791979773968059672   # as Stage-2's descriptors record it (receipts/61-full-F-B2/descriptor.json)
EXE_INODE = 62417761
HERE = os.path.dirname(os.path.abspath(__file__))
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


def git(*args, cwd=REPO):
    return subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True).stdout


def main(root):
    rec = os.path.join(root, "receipts")
    print("P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01 — FINAL GATE (before staging)")
    tools = json.load(open(os.path.join(root, "receipts", "50-TOOLS-RECORD.json")))
    now = {os.path.relpath(p, root): sha(p) for d in ("tools", "docs-src", "development")
           for p in walk(os.path.join(root, d))}
    check("every tool, document source and development record equals the tools record", now == tools["files"],
          f"{len(now)} files")
    r = run([os.path.join(HERE, "authenticate.py"), root], os.path.join(rec, "60-gate-authentication.txt"))
    check("G1 input authentication re-run: ALL CHECKS OK", r.returncode == 0 and "RESULT: ALL CHECKS OK" in r.stdout)
    outs = []
    for k in (1, 2):
        d = tempfile.mkdtemp(prefix=f"derive{k}-")
        shutil.copytree(os.path.join(root, "inputs"), os.path.join(d, "inputs"))
        rr = run([os.path.join(HERE, "derive_catalog.py"), d])
        outs.append((d, rr.returncode))
    cmp = filecmp.dircmp(os.path.join(outs[0][0], "out", "catalog"), os.path.join(outs[1][0], "out", "catalog"))
    same = lambda a, b: sorted(os.path.relpath(p, a) for p in walk(a)) == sorted(  # noqa: E731
        os.path.relpath(p, b) for p in walk(b)) and all(
        open(p, "rb").read() == open(os.path.join(b, os.path.relpath(p, a)), "rb").read() for p in walk(a))
    check("G2 two fresh derivations give byte-identical catalogs and patches", outs[0][1] == outs[1][1] == 0 and
          same(os.path.join(outs[0][0], "out", "catalog"), os.path.join(outs[1][0], "out", "catalog")),
          f"{len(list(walk(os.path.join(outs[0][0], 'out', 'catalog'))))} files")
    check("G2 the derivation equals the catalog to be published", same(os.path.join(outs[0][0], "out", "catalog"),
                                                                        os.path.join(root, "out", "catalog")))
    rr = run([os.path.join(HERE, "render_docs.py"), outs[0][0]])
    check("G2 the rendered blocker and tranche summary equal the documents to be published", rr.returncode == 0 and
          same(os.path.join(outs[0][0], "out", "packet"), os.path.join(root, "out", "packet")))
    del cmp
    for d, _ in outs:
        shutil.rmtree(d)
    r = run([os.path.join(HERE, "static_gate.py"), root], os.path.join(rec, "20-static-gate.txt"))
    f = [l for l in r.stdout.splitlines() if l.startswith("FAIL ")]
    defs = [l for l in f if "exact definition available from an authorised source" in l]
    check("G3 static catalog gate: exit 1, exactly the 21 definition checks FAIL, nothing else",
          r.returncode == 1 and len(defs) == 21 and len(f) == 21 and "STATIC CATALOG GATE: FAIL — "
          "CONTROL_CATALOG_BLOCKED" in r.stdout, f"{len(f)} FAIL lines")
    check("G3 the gate's OK lines", True, sum(1 for l in r.stdout.splitlines() if l.startswith("OK  ")))
    n_ok = sum(1 for l in r.stdout.splitlines() if l.startswith("OK  "))
    index = open(os.path.join(root, "docs-src", "INDEX.md")).read()
    adoption = os.path.join(root, "docs-src", "ad1r5-adoption", "OWNER-ADOPTION-2026-10-14.md")
    adoption_blob = subprocess.run(["git", "hash-object", adoption], capture_output=True, text=True).stdout.strip()
    cited = {"catalog sha256": sha(os.path.join(root, "out", "catalog", "catalog.json")),
             "adoption record sha256": sha(adoption), "adoption record blob": adoption_blob,
             "static gate OK count": f"{n_ok} checks hold", "status": "CONTROL_CATALOG_BLOCKED"}
    text = open(adoption).read()
    required = ["This file is audit evidence of the Owner's decision.\nA string is not authority.\n",
                "adopted AD-1-R5 in full", "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
                "ae3cabdb45627f9b3179a098c50d27e50c55101f", "c1e46fc404a74ed76b848b709a7a246b59f3347f",
                "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b", "`Config.name` is non-semantic",
                "remains **INCOMPLETE**: 30,000,001 discovered, 26,539,923 expanded",
                "> F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF\n",
                "original explorer, named property E8, configuration F-R, under 30,000,000 states",
                "no general permission", "It does not accept D2A-1.", "**Phase Three remains active.**",
                "none is claimed"]
    absent = [q for q in required if q not in text]
    check("G3 the R5 adoption record carries the exact required sentence and every required item", not absent,
          absent or f"{len(required)} items")
    missing = [k for k, v in cited.items() if v not in index]
    check("G3 INDEX.md cites the actual catalog digest, adoption record identity and gate count", not missing,
          missing or cited)
    r = run([os.path.join(HERE, "gate_controls.py"), root], os.path.join(rec, "21-gate-controls.txt"))
    check("G4 gate controls: 14 seeds and the positive control behave as required",
          r.returncode == 0 and "GATE CONTROLS: ALL BEHAVE AS REQUIRED (14 seeds + 1 positive)" in r.stdout)
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    check("G5 catalog: CONTROL_CATALOG_BLOCKED, not frozen, 35 + A4, READY 14, BLOCKED 21",
          cat["catalog_status"] == "CONTROL_CATALOG_BLOCKED" and cat["freeze_status"].startswith("NOT FROZEN") and
          cat["counts"] == {"kill_required": 35, "comparison": 1, "ready": 14, "blocked": 21})
    check("G5 tranche-1 controls without a definition: C11 C17 C18a C18b",
          cat["tranche1_blocked"] == ["C11", "C17", "C18a", "C18b"])
    st = os.stat(EXE)
    check("G6 the pinned executable is untouched (sha256, size, inode and mtime as Stage-2 recorded them)",
          sha(EXE) == EXE_SHA and st.st_size == 8984464 and st.st_ino == EXE_INODE and st.st_mtime_ns == EXE_MTIME_NS,
          f"mtime_ns {st.st_mtime_ns}")
    check("G6 no target directory under the mission root", not any("target" in dirs for _, dirs, _ in
                                                                   os.walk(root)))
    # The one worktree this mission may create is its evidence worktree (mission 18), at the evidence parent on its
    # own branch. Any other added, removed or changed worktree fails (development D5).
    ref = open(os.path.join(rec, "01-worktrees-reference.txt")).read().strip("\n").split("\n\n")
    cur = git("worktree", "list", "--porcelain").strip("\n").split("\n\n")
    allowed = (f"worktree {EVIDENCE_WT}\nHEAD c1e46fc404a74ed76b848b709a7a246b59f3347f\n"
               "branch refs/heads/evidence/p3-c11-d2a1-mutation-tranche1-01")
    extra = [b for b in cur if b not in ref]
    gone = [b for b in ref if b not in cur]
    check("G6 no git worktree created or removed by this mission, except its evidence worktree at c1e46fc4",
          not gone and extra in ([], [allowed]), f"{len(ref)} in the reference; added {len(extra)}; removed {len(gone)}")
    check("G6 no mutant worktree path exists (the evidence worktree is the only mission path)",
          [p for p in os.listdir("/home/nexus/NEXUS") if "tranche1" in p] in
          ([], [os.path.basename(EVIDENCE_WT)]))
    if os.path.exists(EVIDENCE_WT):
        st = git("status", "--porcelain", "--untracked-files=all", cwd=EVIDENCE_WT).splitlines()
        check("G6 the evidence worktree holds nothing but this mission's two allowed paths, uncommitted",
              all(l.startswith(("?? docs/evidence/p3-candidate11/ad1r5-adoption/",
                                "?? docs/evidence/p3-candidate11/d2a1-mutation-tranche1-01/")) for l in st) and
              git("rev-parse", "HEAD", cwd=EVIDENCE_WT).strip() == "c1e46fc404a74ed76b848b709a7a246b59f3347f",
              f"{len(st)} entries")
    check("G7 the baseline worktree is exactly at C and clean",
          git("rev-parse", "HEAD", cwd=BASE_WT).strip() == C and git("status", "--porcelain", cwd=BASE_WT) == "")
    check("G7 Candidate C's tree is unchanged", git("rev-parse", C + "^{tree}").strip() == C_TREE)
    check("G7 the Owner worktree is unchanged (rebuild/phase0-trust-boundary at f727f5c3, clean)",
          git("rev-parse", "HEAD").strip() == OWNER_HEAD and git("status", "--porcelain") == "")
    srcs = set()
    for line in git("ls-tree", "-r", C, "crates").splitlines():
        srcs.add(line.split()[2])
    mutants = {e["mutant_blob"] for e in cat["controls"] if e.get("mutant_blob")}
    bad = []
    files = [p for d in ("out", "development", "tools", "docs-src") for p in walk(os.path.join(root, d))]
    files += [os.path.join(rec, n) for n in ("00-authentication.txt", "20-static-gate.txt", "21-gate-controls.txt")]
    for p in files:
        data = open(p, "rb").read()
        blob = subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True).stdout.decode().strip()
        if p.endswith((".rs", ".pyc")) or b"\0" in data or data[:4] == b"\x7fELF" or blob in srcs or blob in mutants:
            bad.append(os.path.relpath(p, root))
    check("G8 publishable files: text only; no .rs; no ELF; no copy of any crate source at C; no mutant source",
          not bad, bad or f"{len(files)} files")
    print()
    print("FINAL GATE: " + ("ALL CHECKS HOLD — the blocker packet may be staged" if not fails else
                            "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
