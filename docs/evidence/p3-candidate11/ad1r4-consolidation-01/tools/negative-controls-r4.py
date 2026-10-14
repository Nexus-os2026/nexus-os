#!/usr/bin/env python3
"""Negative controls for the AD-1-R4 checking tools (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01). Derived from AD-1-R3's
checker-negative-controls.py. Each control corrupts a copy of the draft (or of the matrix's landing file) in one named
way, in a fresh temporary directory, and requires the named tool to exit nonzero AND to report the named check:
    check-ad1r4.py       a line starting "FAIL  <id>"
    contradiction-scan   a line "[<id>] ..." followed by a CONTRADICTION unit in that category
    check-matrix.py      a line starting "FAIL  <id>"
    classify-cases-r4    a line "DISAGREES" for the named draft row
It also runs check-ad1r4.py on AD-1-R3, which must fail. Writes only into the temporary directory, removed afterwards.
    negative-controls-r4.py <tools-dir> <draft> <landing> <AD-1-R3> <disposition> <contract> <review-02> <review-03>
                            <protocol.rs> <controls_defs.py> <repo> <source-commit> <evidence-root> <evidence-commit-B>"""
import os, re, subprocess, sys, tempfile

(tools, draft, landing, ad1r3, disp, contract, r02, r03, proto, defs, repo, commit, root, evb) = sys.argv[1:15]
TEXT = open(draft, encoding="utf-8").read()
LAND = open(landing, encoding="utf-8").read()
PY = sys.executable


def checker(path):
    p = subprocess.run([PY, f"{tools}/check-ad1r4.py", path, contract, r02, r03, proto, defs, repo, commit, root, evb],
                       capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def scan(path):
    p = subprocess.run([PY, f"{tools}/contradiction-scan.py", path], capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def matrix(landing_path, draft_path):
    p = subprocess.run([PY, f"{tools}/check-matrix.py", landing_path, ad1r3, disp, draft_path, f"{tools}/obligation-scan.py"],
                       capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def cases(path):
    p = subprocess.run([PY, f"{tools}/classify-cases-r4.py", path], capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def failed_check(out, cid):
    return [l for l in out.split("\n") if l.startswith(f"FAIL  {cid}")]


def scan_hit(out, cid):
    block = out.split(f"[{cid}] ", 1)[1].split("\n[X", 1)[0] if f"[{cid}] " in out else ""
    return [l for l in block.split("\n") if "CONTRADICTION" in l]


C_CHECK = [  # (name, old, new, check id that must FAIL)
    ("healthy witness put on the mutant executable (F.2)", "It runs on the pinned unchanged-baseline executable of I: the one",
     "It runs on the same pinned mutant executable of I: the one", "C17a"),
    ("G.3(c) executable bullet reverted to the single mutant executable",
     "an executable other than the one its role requires (F.2): for (b), the pinned mutant executable of (a); for (d),",
     "an executable other than the mutant executable of (a); for (d),", "C9b"),
    ("undivided type (e) reintroduced", "Two admissible receipts of type (e1)", "Two admissible receipts of type (e)", "C18b"),
    ("healthy build's executables marked NOT PRODUCED BY DESIGN", "None is\n  marked NOT PRODUCED BY DESIGN.",
     "Its executables are\n  marked NOT PRODUCED BY DESIGN.", "C19b"),
    ("C4's comma read as 'or'", "both P4 and P8 must be shown, and the comma is never read as \"or\".",
     "either P4 or P8 must be shown.", "C20a"),
    ("one of P4 and P8 made sufficient", "**Only P4, or only P8, is insufficient.**", "**Either P4 or P8 is sufficient.**", "C20d"),
    ("the witness made to replace full_f8", "Both P4 AND P8 are required (S-22). The deterministic witness does not replace `full_f8`.",
     "Both P4 AND P8 are required (S-22). The deterministic witness replaces `full_f8`.", "C20d"),
    ("C9's resulting arm drops self.failed() (C9-a)", "                self.failed();\n                Persistence::NotWritten",
     "                Persistence::NotWritten", "C1r-e"),
    ("C10's sealed definition altered", "                let persistence = self.rolled(io.rollback());",
     "                self.rolled(io.rollback());", "C1b"),
    ("E3 relabelled as P1 in G.3", "- E3 is never counted as P1, and E4 never as P7.", "- E3 is counted as P1, and E4 as P7.", "C24a"),
    ("prototype results promoted to final-baseline evidence",
     "They are not final-baseline evidence, and they\ndischarge none of (A) to (E).", "They are final-baseline evidence.", "C20d"),
    ("contract row C4 misquoted", "| C4 | acknowledgement test 1 removed | P4, P8 | F-8 |",
     "| C4 | acknowledgement test 1 removed | P4 or P8 | F-8 |", "C1b"),
    ("C4's definition keeps the U latch", "C and X are empty;\n    U is empty, with the cell-invariant latch when it is not;",
     "C and X are empty; the U latch is kept;", "C20b"),
    ("Oracle fallback accepted", "Silent\nfallback is never accepted as a specified schedule.", "Silent\nfallback is accepted.", "C21b"),
    ("C9/C10/C24 execution authorised", "| Execution | No C9, C10 or C24 execution is authorised. | G.2 |",
     "| Execution | C9, C10 and C24 execution is authorised. | G.2 |", "C15b"),
    ("C22's restored receipt retyped", "(e4) for C22 (the healthy\n   build evidence of F.1 (d))", "(e1) for C22", "C10n"),
    ("classification case 9 removed", "| 9 | C4 witness on the mutant showing only one of P4 and P8", "| x | C4 witness on the mutant showing only one of P4 and P8", "C11i"),
    ("status loses NOT ADOPTED", "**STATUS: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. NOT IN FORCE.**",
     "**STATUS: DRAFT — NOT ACCEPTED — NOT APPLIED. NOT IN FORCE.**", "C9a"),
    ("C4's reading reopened", "10. Closed as to C4's reading", "10. Open as to C4's reading", "C16b"),
    ("a mutation control allowed under a quotient", "No mutation control is ever run under a quotient (F).",
     "A mutation control may run under a quotient (F).", "C14"),
    ("A-1/A-2 discharged", "stay stated and undischarged", "are discharged", "C14"),
    ("supersession row S-25 removed", "| S-25 | 720 |", "| X-25 | 720 |", "C3a"),
    ("Oracle changed", "Oracle and the original replay helpers are not changed.\n\n1.", "Oracle is changed to refuse fallback.\n\n1.", "C21b"),
    ("C4's resulting text keeps a Test 1 check", "        // Test 1.\n        let core = &mut *self.core;\n        let set = snapshot.set;",
     "        // Test 1.\n        let core = &mut *self.core;\n        if core.u.is_some() {\n        let set = snapshot.set;", "C1r-b"),
    ("an operative pointer into AD-1-R3 by line", "There is one taxonomy.", "There is one taxonomy (R3 725).", "C25a"),
    ("Codex reinstated as evidence reviewer", "There is one taxonomy.", "There is one taxonomy. Codex reviews the evidence.", "C13f"),
]
C_SCAN = [  # (name, old, new, category that must show a CONTRADICTION)
    ("a healthy witness on the mutant executable", "Every healthy witness runs on the pinned\nunchanged-baseline executable (F.2).",
     "Every healthy witness runs on the same pinned mutant executable (F.2).", "X1"),
    ("an undivided type (e) reference", "Two admissible receipts of type (e1)", "Two admissible receipts of type (e)", "X2"),
    ("a healthy build's executable not produced", "There is one taxonomy.",
     "There is one taxonomy. A healthy build's executable is NOT PRODUCED BY DESIGN.", "X3"),
    ("either P4 or P8 suffices", "There is one taxonomy.", "There is one taxonomy. For C4, either P4 or P8 suffices.", "X4"),
    ("E3 counted as P1", "There is one taxonomy.", "There is one taxonomy. For C1, E3 is counted as P1.", "X5"),
    ("C9-a selected", "There is one taxonomy.", "There is one taxonomy. C9-a is selected.", "X6"),
    ("Codex as reviewer", "There is one taxonomy.", "There is one taxonomy. Codex reviews the evidence.", "X7"),
    ("an INCOMPLETE run counted as PASS", "There is one taxonomy.", "There is one taxonomy. An INCOMPLETE run is counted as PASS.", "X8"),
    ("a control under a quotient", "There is one taxonomy.", "There is one taxonomy. A control may run under a quotient.", "X9"),
    ("prototype results as final-baseline evidence", "There is one taxonomy.",
     "There is one taxonomy. The prototype results at A are final-baseline evidence.", "X10"),
    ("C4 claimed killed", "There is one taxonomy.", "There is one taxonomy. C4 is killed.", "X11"),
    ("full_f8 authorised", "There is one taxonomy.", "There is one taxonomy. The `full_f8` run is authorised.", "X12"),
]
C_MATRIX = [  # (name, target, old, new, check id that must FAIL)
    ("a matrix anchor no longer in the draft", "draft", "**Only P4, or only P8, is insufficient.**", "**One of them is enough.**", "M4"),
    ("a disposition row dropped", "landing", "\nDISP-ec5c596f20\t", "\nXDISP-ec5c596f20\t", "M1"),
    ("an AD-1-R3 row dropped", "landing", "\nR3-0090cdb04f\t", "\nXR3-0090cdb04f\t", "M2"),
    ("a non-verbatim disposition row marked VERBATIM", "landing", "\nDISP-ec5c596f20\tOPERATIVE\t", "\nDISP-ec5c596f20\tVERBATIM\t", "M5"),
    ("an unknown section label", "landing", "\nA-14\tRULING\tG.2, 0A.1\t", "\nA-14\tRULING\tG.9, 0A.1\t", "M4"),
    ("an AD-1-R3 verbatim obligation dropped from the draft", "draft", "Reproducible builds are not established: two builds of one source have\ndiffered in sha256.",
     "Reproducible builds are assumed.", "M2"),
]
C_CASES = [  # (name, old, new, draft row that must DISAGREE)
    ("case 11 made admissible", "| 11 | A witness on the other role's executable (a mutant witness on the baseline executable, or a healthy witness on a mutant executable), or a positive marker in a healthy-witness receipt, or a healthy line in a mutant-witness receipt | as printed | NOT ADMISSIBLE (role identity, F.2) | no |",
     "| 11 | A witness on the other role's executable (a mutant witness on the baseline executable, or a healthy witness on a mutant executable), or a positive marker in a healthy-witness receipt, or a healthy line in a mutant-witness receipt | as printed | PASS | — |", "11"),
    ("case 9 made a kill", "| as printed | NOT ADMISSIBLE for G.5(B): both properties are required | no |", "| as printed | admissible witness receipt: G.5(B) holds | yes |", "9"),
]

results, bad = [], 0
with tempfile.TemporaryDirectory() as tmp:
    rc, out = checker(ad1r3)
    m = re.search(r"^SUMMARY: \d+ passed, (\d+) failed", out, flags=re.M)
    n = int(m.group(1)) if m else 0
    ok = rc != 0 and n > 0
    bad += not ok
    results.append(f"{'DETECTED' if ok else 'MISSED  '}  check-ad1r4 on AD-1-R3 itself: exit {rc}, {n} checks failed")
    rc, out = checker(draft)
    ok = rc == 0
    bad += not ok
    results.append(f"{'BASELINE' if ok else 'BROKEN  '}  check-ad1r4 on the uncorrupted draft: exit {rc}")

    def corrupt(base, old, new):
        if base.count(old) != 1:
            return None
        return base.replace(old, new)

    for name, old, new, cid in C_CHECK:
        t = corrupt(TEXT, old, new)
        if t is None:
            results.append(f"SETUP-ERROR  check-ad1r4 / {name}: anchor occurs {TEXT.count(old)} times"); bad += 1; continue
        p = os.path.join(tmp, "c.md"); open(p, "w", encoding="utf-8").write(t)
        rc, out = checker(p)
        hit = failed_check(out, cid)
        ok = rc != 0 and bool(hit)
        bad += not ok
        results.append(f"{'DETECTED' if ok else 'MISSED  '}  check-ad1r4 / {name}: exit {rc}; {cid} FAIL lines {len(hit)}"
                       + (f"; first: {hit[0][:100]}" if hit else ""))
    for name, old, new, cid in C_SCAN:
        t = corrupt(TEXT, old, new)
        if t is None:
            results.append(f"SETUP-ERROR  contradiction-scan / {name}: anchor occurs {TEXT.count(old)} times"); bad += 1; continue
        p = os.path.join(tmp, "s.md"); open(p, "w", encoding="utf-8").write(t)
        rc, out = scan(p)
        hit = scan_hit(out, cid)
        ok = rc != 0 and bool(hit)
        bad += not ok
        results.append(f"{'DETECTED' if ok else 'MISSED  '}  contradiction-scan / {name}: exit {rc}; {cid} CONTRADICTION units {len(hit)}")
    for name, target, old, new, cid in C_MATRIX:
        base = TEXT if target == "draft" else LAND
        t = corrupt(base, old, new)
        if t is None:
            results.append(f"SETUP-ERROR  check-matrix / {name}: anchor occurs {base.count(old)} times"); bad += 1; continue
        dp, lp = draft, landing
        if target == "draft":
            dp = os.path.join(tmp, "m.md"); open(dp, "w", encoding="utf-8").write(t)
        else:
            lp = os.path.join(tmp, "l.txt"); open(lp, "w", encoding="utf-8").write(t)
        rc, out = matrix(lp, dp)
        hit = failed_check(out, cid)
        ok = rc != 0 and bool(hit)
        bad += not ok
        results.append(f"{'DETECTED' if ok else 'MISSED  '}  check-matrix / {name}: exit {rc}; {cid} FAIL lines {len(hit)}")
    for name, old, new, row in C_CASES:
        t = corrupt(TEXT, old, new)
        if t is None:
            results.append(f"SETUP-ERROR  classify-cases-r4 / {name}: anchor occurs {TEXT.count(old)} times"); bad += 1; continue
        p = os.path.join(tmp, "k.md"); open(p, "w", encoding="utf-8").write(t)
        rc, out = cases(p)
        hit = [l for l in out.split("\n") if f"draft row {row}: DISAGREES" in l]
        ok = rc != 0 and bool(hit)
        bad += not ok
        results.append(f"{'DETECTED' if ok else 'MISSED  '}  classify-cases-r4 / {name}: exit {rc}; row {row} DISAGREES lines {len(hit)}")
for r in results:
    print(r)
print(f"\nNEGATIVE CONTROLS: {len(results) - bad} as required, {bad} missed, broken or not set up "
      f"({len(C_CHECK)} checker, {len(C_SCAN)} scan, {len(C_MATRIX)} matrix, {len(C_CASES)} cases, 2 baselines)")
sys.exit(1 if bad else 0)
