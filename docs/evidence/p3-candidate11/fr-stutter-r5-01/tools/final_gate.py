#!/usr/bin/env python3
"""The declared static final gate, run once after the freeze (mission section 21).

  final_gate.py ROOT WORKTREE

Re-runs every frozen tool from the frozen files into ROOT/gate, requires each
to succeed and every regenerated output to equal its frozen copy byte for
byte, checks the repository state, stages the packet into ROOT/stage and
checks its confinement and integrity. Writes ROOT/receipts/60-final-gate.txt.
A semantic failure is reported and the mission stops; nothing is revised or
rerun.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PY = sys.executable
REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
PARENT = "89326b859792a442c1853c3d35d30a48301c009e"
PACKET_REL = "docs/evidence/p3-candidate11/fr-stutter-r5-01"
AD1R4 = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
AD1R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
LIMIT = 2 * 1024 * 1024
DOCS = ["FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md", "P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md",
        "PROPOSED-20-CONFIGURATION-ACCOUNTING.md", "C15-DISPOSITION.md"]
SECRET = re.compile(r"BEGIN [A-Z ]*PRIVATE KEY|\bghp_[A-Za-z0-9]{20,}|\bgithub_pat_[A-Za-z0-9_]{20,}|\bAKIA[0-9A-Z]{16}\b|\bsk-[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}")

lines, fails = [], 0


def check(cid, ok, what):
    global fails
    lines.append(f"{'PASS' if ok else 'FAIL'}  {cid}  {what}")
    print(lines[-1], flush=True)
    fails += 0 if ok else 1


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def git(*a):
    return subprocess.run(["git", "-C", REPO, *a], capture_output=True, check=True).stdout


def main(root, worktree):
    if os.path.exists(os.path.join(root, "receipts", "60-final-gate.txt")):
        raise SystemExit("REFUSED: the final gate has already run")
    frozen = json.load(open(os.path.join(root, "receipts", "40-FREEZE.json")))["files"]
    # G01 freeze integrity
    bad = [k for k, h in frozen.items() if not os.path.exists(os.path.join(root, k)) or sha(os.path.join(root, k)) != h]
    check("G01", not bad, f"all {len(frozen)} frozen files unchanged ({bad[:5]})")
    for k in ("tools/final_gate.py", "tools/build_packet.py", "tools/controls.py", "tools/checker_a.py", "tools/checker_b.py",
              "tools/occurrence_spec.py", "docs-src/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.src.md",
              "out/packet/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md", "out/packet/REPLAY-OCCURRENCE-MATRIX.tsv"):
        if k not in frozen:
            check("G01", False, f"{k} was not frozen")
    gate = os.path.join(root, "gate")
    if os.path.exists(gate):
        raise SystemExit("REFUSED: gate directory exists")
    # G02..G12 regenerate everything from the frozen files
    rc, out = run([PY, f"{HERE}/prepare.py", gate])
    open(os.path.join(root, "receipts", "60-gate-prepare-console.txt"), "w").write(out)
    check("G02", rc == 0, "every frozen tool re-ran successfully from the frozen inputs (authentication, checker A occurrences "
          "and closure, checker B, agreement, branch disposition, proof render, accounting, citations by A and by B, "
          "document checks, controls)")
    fr = os.path.join(root, "out")
    regen = {}
    for sub in ("packet", "receipts"):
        for base, dirs, names in os.walk(os.path.join(gate, sub)):
            for n in names:
                p = os.path.join(base, n)
                rel = os.path.relpath(p, gate)
                regen[rel] = sha(p)
    frozen_out = {k[len("out/"):]: h for k, h in frozen.items() if k.startswith("out/")}
    diff = sorted(k for k in set(regen) | set(frozen_out) if regen.get(k) != frozen_out.get(k))
    check("G03", not diff, f"every regenerated output equals its frozen copy byte for byte ({len(regen)} files; differing {diff[:8]})")

    def receipt(name):
        return open(os.path.join(gate, "receipts", name)).read()
    a = receipt("10-checker-a-occurrences.txt")
    check("G04", "CHECKER A occurrence verdict: REPLAY_OCCURRENCE_CLOSURE_COMPLETE" in a and "233 target occurrences in 15 of 49 files" in a,
          "complete occurrence closure: REPLAY_OCCURRENCE_CLOSURE_COMPLETE, 233 occurrences, 49 inputs, none unclassified")
    b = receipt("12-checker-b.txt")
    check("G05", "CHECKER B: 62 checks, 0 failed; 233 target occurrences" in b, "checker B: 62 of 62 checks hold")
    ag = receipt("13-agreement.txt")
    check("G06", "AGREEMENT: BOTH ANALYSES AGREE" in ag and "only A 0, only B 0; status differences 0" in ag,
          "both independent analyses agree: the same 233 occurrences, statuses and explore closure")
    ca, cb = receipt("17-cite-checker-a.txt"), receipt("18-cite-checker-b.txt")
    na = re.search(r"CHECKER A citations: (\d+) citations in 4 documents; (\d+) distinct items; 0 unresolved", ca)
    nb = re.search(r"CHECKER B citations: (\d+) citations in 4 documents; 0 unresolved", cb)
    check("G07", bool(na and nb and na.group(1) == nb.group(1) and int(na.group(1)) > 50),
          f"every source citation resolves, by A and independently by B ({na.group(1) if na else '?'} citations)")
    dc = receipt("19-doc-check.txt")
    for cid, what in (("S1b", "FR-1 … FR-17 each argued"), ("S1c", "FR-1 … FR-17 each in the status table"), ("W1", "F-R is never called PASS (no forbidden wording)"),
                      ("W2", "no unsupported F-R count"), ("W4", "the F-R receipt preserved as INCOMPLETE"),
                      ("W3", "C15 stays original-explorer, 30,000,000"), ("S2g", "AD-1-R5's rows are scoped to F-R, full_fr or C15"),
                      ("S2h", "AD-1-R5 makes no other configuration dischargeable"), ("S4a", "complete branch disposition")):
        check(f"G08-{cid}", re.search(rf"^PASS\s+{cid}\b", dc, re.M) is not None, what)
    check("G08", "DOC CHECK: 25 checks, 0 failed" in dc, "document checks: 25 of 25")
    ct = receipt("20-controls.txt")
    check("G09", "CONTROLS: 25 of 25 behave as required (13 source, 10 document, 2 positive)" in ct,
          "every negative control detected and both positive controls pass (25 of 25)")
    au = receipt("00-authentication.txt")
    check("G10", "AUTHENTICATION: ALL INPUTS AUTHENTICATED" in au, "all source and evidence provenance authenticated")
    bd = receipt("14-branch-disposition.txt")
    check("G11", "none NOT_ESTABLISHED" in bd and "'TAKEN': 57, 'NOT TAKEN': 106, 'NOT APPLICABLE': 30" in bd,
          "branch accounting complete: 193 rows, 57 / 106 / 30, none NOT_ESTABLISHED")
    # repository state
    ad = git("show", f"{PARENT}:{AD1R4}")
    check("G12", hashlib.sha256(ad).hexdigest() == AD1R4_SHA, "AD-1-R4 unchanged at the evidence parent (sha256 00ced6d5…)")
    r5 = open(os.path.join(fr, "packet", DOCS[1]), encoding="utf-8").read()
    check("G13", AD1R4_SHA in r5 and "AD-1-R4 remains governing until the Owner adopts R5." in r5,
          "AD-1-R5 is a separate supplement; it names AD-1-R4 as governing and changes no unrelated rule (S2g, S2h)")
    head = subprocess.run(["git", "-C", worktree, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    status = subprocess.run(["git", "-C", worktree, "status", "--porcelain", "--untracked-files=all"], capture_output=True, text=True).stdout.split("\n")
    status = [s for s in status if s]
    check("G14", head == PARENT and all(s.startswith("?? " + PACKET_REL + "/") for s in status),
          f"evidence worktree at the parent 89326b85 with no tracked change ({len(status)} untracked entries, all under the packet path)")
    check("G15", git("rev-parse", f"{C}^{{tree}}").decode().strip() == C_TREE and
          git("diff", "--name-only", C, PARENT, "--", ".", ":!docs/evidence").decode().strip() == "",
          "Candidate C unchanged; nothing outside docs/evidence differs from C at the parent")
    # stage the packet
    stage = os.path.join(root, "stage")
    if os.path.exists(stage):
        raise SystemExit("REFUSED: stage exists")
    rc, out = run([PY, f"{HERE}/build_packet.py", "stage", root, stage])
    open(os.path.join(root, "receipts", "60-gate-stage-console.txt"), "w").write(out)
    check("G16", rc == 0, "packet staged from the frozen files")
    srcs = set()
    for l in git("ls-tree", "-r", C, "crates/nexus-governed-control/src").decode().splitlines():
        srcs.add(hashlib.sha256(git("cat-file", "blob", l.split()[2])).hexdigest())
    problems, total, n = [], 0, 0
    for base, dirs, names in os.walk(stage):
        for f in names:
            p = os.path.join(base, f)
            rel = os.path.relpath(p, stage)
            data = open(p, "rb").read()
            n += 1
            total += len(data)
            if b"\0" in data:
                problems.append(f"binary {rel}")
            if rel.endswith((".rs", ".pyc", ".so", ".exe", ".bin")):
                problems.append(f"refused name {rel}")
            if hashlib.sha256(data).hexdigest() in srcs:
                problems.append(f"source copy {rel}")
            if os.stat(p).st_mode & 0o111:
                problems.append(f"executable bit {rel}")
            if SECRET.search(data.decode("utf-8", "replace")):
                problems.append(f"credential-like text {rel}")
            if os.path.islink(p):
                problems.append(f"symlink {rel}")
    check("G17", not problems and total <= LIMIT - 64 * 1024,
          f"staged packet confined and clean: {n} files, {total} bytes (limit {LIMIT}, 64 KiB kept for INDEX, the final-gate "
          f"receipt and SHA256SUMS); text only; no source copy, binary, executable bit, symlink or credential ({problems[:5]})")
    verdict = "FINAL GATE: ALL CHECKS HOLD" if fails == 0 else f"FINAL GATE: {fails} CHECK(S) FAILED — MISSION STOPS"
    lines.append(verdict)
    with open(os.path.join(root, "receipts", "60-final-gate.txt"), "w") as fh:
        fh.write("FINAL GATE — P3-C11-DR01-D2A1-FR-STUTTER-R5-01 (run once, after the freeze)\n")
        fh.write("\n".join(lines) + "\n")
    print(verdict)
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
