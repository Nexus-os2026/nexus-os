#!/usr/bin/env python3
"""Renders the consolidation matrix for reading (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01): joins
matrix/CONSOLIDATION-MATRIX.tsv with the two obligation-scan outputs (the source sentence of each DISP and R3 id) into
one Markdown table per part. The TSV is the checked source (check-matrix.py); this rendering adds only the source
sentence text and changes nothing. Refuses to overwrite.
    render-matrix.py <matrix.tsv> <scan-R3.txt> <scan-DISP.txt> <out.md>"""
import os, re, sys

tsv, scan_r3, scan_disp, out = sys.argv[1:5]
text = {}
for p in (scan_r3, scan_disp):
    for l in open(p, encoding="utf-8").read().split("\n"):
        m = re.match(r"^((?:R3|DISP)-[0-9a-f]{10}) (?:VERBATIM|NOT-VERBATIM) (.*)$", l)
        if m:
            text[m.group(1)] = m.group(2)


def cell(s, n):
    s = " ".join(s.split()).replace("|", "\\|")
    return s if len(s) <= n else s[: n - 1] + "…"


parts = {"A": [], "DISP": [], "R3": []}
for l in open(tsv, encoding="utf-8").read().split("\n"):
    if not l.strip() or l.startswith("#"):
        continue
    rid, kind, secs, anchors, note = l.split("\t")
    part = "A" if rid.startswith("A-") else rid.split("-")[0]
    first = anchors.split(" ‖ ")[0] if anchors.strip() else ""
    more = len([a for a in anchors.split(" ‖ ") if a.strip()]) - 1
    parts[part].append((rid, kind, secs, first, more, note))
lines = ["# Consolidation matrix — AD-1-R3 and AD1R3-DISPOSITION-01 into AD-1-R4", "",
         "Rendered from `matrix/CONSOLIDATION-MATRIX.tsv` (the checked source; `checks/03-check-matrix.txt`) and the",
         "obligation scans (`checks/06-obligation-scan-r3.txt`, `checks/07-obligation-scan-disposition.txt`). "
         "\"Landed in\" names AD-1-R4 sections; the anchor is a phrase that stands, whitespace-normalised, in one of them",
         "(further anchors of a row: see the TSV). Kinds are explained in `matrix/README.md`.", ""]
heads = {"A": ("Part A — the Architect's rulings and the mission's directives", "Item"),
         "DISP": ("Part B — every sentence of AD1R3-DISPOSITION-01", "Source sentence"),
         "R3": ("Part C — every obligation-bearing sentence of AD-1-R3 not carried verbatim", "Source sentence")}
for part in ("A", "DISP", "R3"):
    title, col = heads[part]
    lines += [f"## {title} ({len(parts[part])} rows)", "", f"| Id | Kind | {col} | Landed in | Anchor | Note |", "|---|---|---|---|---|---|"]
    for rid, kind, secs, first, more, note in parts[part]:
        src = note if part == "A" else text.get(rid, "?")
        nt = "" if part == "A" else note
        lines.append(f"| {rid} | {kind} | {cell(src, 150)} | {secs or '—'} | {cell(first, 90)}{f' (+{more})' if more > 0 else ''} | {cell(nt, 110)} |")
    lines.append("")
if os.path.exists(out):
    sys.exit(f"REFUSED: {out} exists")
open(out, "x", encoding="utf-8").write("\n".join(lines))
print(f"rendered {out}: part A {len(parts['A'])}, DISP {len(parts['DISP'])}, R3 {len(parts['R3'])} rows; "
      f"source texts found {sum(1 for p in ('DISP', 'R3') for r in parts[p] if r[0] in text)}")
