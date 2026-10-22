#!/usr/bin/env python3
"""Checks that the R3 -> R4 semantic review covers every section the diff touches (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01).
Each line of AD-1-R3 and AD-1-R4 is labelled with the section it stands in (header, provenance, 0, 0A.1..0A.5, 1,
A..R with their subsections, open, changes). A section is touched if a line of it is added, removed or changed
(difflib, line by line). The review must hold a heading "### <label>" for every touched label, and none for an
untouched one. Writes nothing.
    check-review.py <AD-1-R3> <AD-1-R4> <review>"""
import difflib, re, sys

r3, r4, review = sys.argv[1:4]


def labels(lines):
    out, cur = [], "header"
    for l in lines:
        m = re.match(r"^(#{2,3}) (.*)$", l)
        if m:
            h = m.group(2)
            if h.startswith("Provenance labels"):
                cur = "provenance"
            elif h.startswith("Open items"):
                cur = "open"
            elif h.startswith("Changes from"):
                cur = "changes"
            elif h.startswith(("Verbatim text of each superseded clause", "Not superseded")):
                cur = "1"
            else:
                mm = re.match(r"^((?:0A|[A-R])\.\d|0A|0|1|[A-R])\. |^((?:0A|[A-R])\.\d) ", h)
                if mm:
                    cur = mm.group(1) or mm.group(2)
        out.append(cur)
    return out


A = open(r3, encoding="utf-8").read().split("\n")
B = open(r4, encoding="utf-8").read().split("\n")
LA, LB = labels(A), labels(B)
touched = set()
for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=A, b=B, autojunk=False).get_opcodes():
    if tag == "equal":
        continue
    touched |= set(LA[i1:i2]) | set(LB[j1:j2])
    if i1 == i2 and j1 == j2:
        continue
rv = open(review, encoding="utf-8").read()
covered = set(re.findall(r"^### (\S+)", rv, flags=re.M))
print(f"sections touched by the R3 -> R4 diff ({len(touched)}): {', '.join(sorted(touched))}")
print(f"sections the review addresses ({len(covered)}): {', '.join(sorted(covered))}")
missing, extra = sorted(touched - covered), sorted(covered - touched)
print(f"missing from the review: {missing}\naddressed but untouched: {extra}")
ok = not missing and not extra
print(f"\nREVIEW COVERAGE: {'COMPLETE' if ok else 'INCOMPLETE'}")
sys.exit(0 if ok else 1)
