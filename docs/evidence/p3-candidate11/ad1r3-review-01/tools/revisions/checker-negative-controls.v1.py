#!/usr/bin/env python3
"""Negative controls for check-ad1r3.py: each corrupts a copy of a draft in one named way (in a temporary directory)
and requires the checker to exit nonzero and to report the named check as FAIL. Also runs the checker on AD-1-R2,
which must fail. Writes only into a fresh temporary directory, removed afterwards.
    checker-negative-controls.py <checker> <draft> <ad1r2> <contract> <review-02> <review-03> <repo> <commit> <root>"""
import os, re, subprocess, sys, tempfile

checker, draft, ad1r2, contract, r02, r03, repo, commit, root = sys.argv[1:10]
text = open(draft, encoding="utf-8").read()


def run(path):
    p = subprocess.run([sys.executable, checker, path, contract, r02, r03, repo, commit, root],
                       capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


CORRUPTIONS = [  # (name, old, new, a check id that must FAIL)
    ("blanket G.3(c) restored in S-6", "G.3(c) applies only to the C1/C25 witness invocation, never to any other receipt.",
     "G.3(c)'s rejections apply to every failing receipt.", "C10a"),
    ("text-only classification restored", "AD-1-R2's rule \"classification is by text, never by exit status\" is withdrawn.",
     "Classification is by text, never by exit status.", "C9b"),
    ("raw FAIL beyond C made a final FAIL", "its outer gate\n  is **INCOMPLETE — BUDGET EXCEEDED**",
     "it\n  is a FAIL of the explored graph", "C11a"),
    ("classification case 7 removed", "\n| 7 | Original explorer:", "\n| x | Original explorer:", "C11i"),
    ("witness line required of the primary receipt", "No witness line is required\n  of it.",
     "Its output must also\n  contain `NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)`.", "C10j"),
    ("C22 executable hash invented", "NOT PRODUCED BY DESIGN: the mutant test executable",
     "Required: the mutant test executable", "C10f"),
    ("O.4 alternative reading restored", "4. **Line 609's phrase \"so that a violation has a shortest trace\" is REPLACED (S-19).**",
     "4. **Line 609's phrase.** If the Architect reads it as claiming global minimality, it is replaced.", "C12c"),
    ("a verbatim contract line altered", "| C25 | a gating append answers Committed on `Commit::Unknown` | P7 | F-1 |",
     "| C25 | a gating append answers Committed | P7 | F-1 |", "C1b"),
    ("Codex required outside the mapping", "- **PROPOSED CLARIFICATION**:", "- Codex must review this draft.\n- **PROPOSED CLARIFICATION**:", "C13f"),
    ("line-929 discrepancy dropped", "**Line 929.**", "**Lines.**", "C13c"),
    ("receipt linking loses per-control restoration", "its own verified baseline restoration", "a shared restoration", "C14"),
    ("C4 reading silently settled", "Whether C4's \"P4, P8\" is a list or a\n    conjunction", "C4's \"P4 or P8\" is a list", "C16b"),
    ("a control claimed killed", "No final mutation kill. | G |", "C25 is killed. | G |", "C9c"),
    ("open item renumbered away", "\n12. The typed receipt requirements", "\n13. The typed receipt requirements", "C16a"),
    ("VERIFICATION INVALID dropped", "**INCOMPLETE —\nVERIFICATION INVALID**", "**INCOMPLETE**", "C11b"),
]
results, bad = [], 0
with tempfile.TemporaryDirectory() as tmp:
    rc, out = run(ad1r2)
    ok = rc != 0 and re.search(r"^SUMMARY: \d+ passed, (\d+) failed", out, flags=re.M)
    nfail = int(ok.group(1)) if ok else 0
    results.append(f"{'DETECTED' if rc != 0 and nfail > 0 else 'MISSED  '}  AD-1-R2 itself: exit {rc}, {nfail} checks failed")
    bad += not (rc != 0 and nfail > 0)
    for name, old, new, cid in CORRUPTIONS:
        if text.count(old) != 1:
            results.append(f"SETUP-ERROR  {name}: anchor occurs {text.count(old)} times")
            bad += 1
            continue
        path = os.path.join(tmp, "corrupt.md")
        with open(path, "w", encoding="utf-8") as f:
            f.write(text.replace(old, new))
        rc, out = run(path)
        hit = [l for l in out.split("\n") if l.startswith(f"FAIL  {cid}")]
        det = rc != 0 and bool(hit)
        bad += not det
        results.append(f"{'DETECTED' if det else 'MISSED  '}  {name}: exit {rc}; {cid} FAIL lines {len(hit)}"
                       + (f"; first: {hit[0][:110]}" if hit else ""))
        os.remove(path)
for r in results:
    print(r)
print(f"\nNEGATIVE CONTROLS: {len(results) - bad} detected, {bad} missed or not set up")
sys.exit(1 if bad else 0)
