#!/usr/bin/env python3
"""Checks the consolidation matrix (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01) against AD-1-R3, the disposition sheet
and AD-1-R4. The matrix's source is matrix/landing.txt: one row per obligation, tab-separated:
    ID  KIND  SECTIONS  ANCHORS  NOTE
SECTIONS is a comma-separated list of AD-1-R4 section labels (header, 0, 0A.1-0A.5, 1, A-R, E.1-E.4, F.1, F.2,
G.1-G.6, H.1-H.6, K.1, K.2, open). ANCHORS are phrases separated by " ‖ "; each must occur, whitespace-normalised, in
at least one of the named sections. The checks:
  M1  the DISP rows are exactly every sentence of the disposition sheet (obligation-scan.py ... all);
  M2  the R3 rows are exactly the obligation-bearing AD-1-R3 sentences that are not verbatim in AD-1-R4; the others
      (verbatim) are counted as carried and located;
  M3  every RULING/DIRECTIVE row (part A) has a unique id and at least one anchor;
  M4  every anchor is found in a named section; rows of kind DESCRIPTIVE or MARKER need a note and may have none;
  M5  a DISP row marked VERBATIM is verbatim in AD-1-R4, and every verbatim DISP sentence is so marked or is a MARKER;
  M6  every kind is one of the declared kinds for its part.
Writes nothing. Exit status 0 only if every check passes.
    check-matrix.py <landing.txt> <AD-1-R3> <disposition> <AD-1-R4> <obligation-scan.py>"""
import re, subprocess, sys

landing, r3, disp, r4, scan = sys.argv[1:6]
fails, notes = [], []


def check(ok, what):
    (notes if ok else fails).append(("PASS  " if ok else "FAIL  ") + what)


def flat(s):
    return " ".join(s.split())


def run_scan(src, label, mode=None):
    out = subprocess.run([sys.executable, scan, src, r4, label] + ([mode] if mode else []), check=True,
                         capture_output=True, text=True).stdout.split("\n")
    rows = {}
    for l in out[1:]:
        m = re.match(rf"^({label}-[0-9a-f]{{10}}) (VERBATIM|NOT-VERBATIM) (.*)$", l)
        if m:
            rows[m.group(1)] = (m.group(2), m.group(3))
    return out[0], rows


text = open(r4, encoding="utf-8").read()


def sections(t):
    S = {"header": t.split("## Provenance labels", 1)[0],
         "open": t.split("## Open items", 1)[1].split("## Changes from AD-1-R3", 1)[0]}
    for m in re.finditer(r"^## (0|0A|1|[A-R])\. .*$", t, flags=re.M):
        body = t[m.end():]
        nxt = re.search(r"^## ", body, flags=re.M)
        S[m.group(1)] = body[: nxt.start()] if nxt else body
    for m in re.finditer(r"^### ((?:0A|[A-R])\.\d) .*$", t, flags=re.M):
        body = t[m.end():]
        nxt = re.search(r"^(?:###? |---$)", body, flags=re.M)
        S[m.group(1)] = body[: nxt.start()] if nxt else body
    return {k: flat(v) for k, v in S.items()}


SEC = sections(text)
KINDS = {"A": {"RULING", "DIRECTIVE"},
         "DISP": {"OPERATIVE", "VERBATIM", "DEFECT", "POINTER", "PROVENANCE", "QUESTION", "DESCRIPTIVE", "MARKER"},
         "R3": {"AMENDED", "RECARRIED", "REPLACED-BY-RULING", "HISTORY"}}
rows = []
for n, line in enumerate(open(landing, encoding="utf-8").read().split("\n"), 1):
    if not line.strip() or line.startswith("#"):
        continue
    parts = line.split("\t")
    if len(parts) != 5:
        fails.append(f"FAIL  M0 landing line {n} has {len(parts)} fields, not 5: {line[:80]}")
        continue
    rows.append((n, *parts))
ids = [r[1] for r in rows]
check(len(ids) == len(set(ids)), f"M0 landing ids unique ({len(ids)} rows)")

disp_head, DISP = run_scan(disp, "DISP", "all")
r3_head, R3 = run_scan(r3, "R3")
print(disp_head)
print(r3_head)
disp_rows = {r[1]: r for r in rows if r[1].startswith("DISP-")}
r3_rows = {r[1]: r for r in rows if r[1].startswith("R3-")}
a_rows = [r for r in rows if re.match(r"^A-\d{2}$", r[1])]
check(set(disp_rows) == set(DISP), f"M1 DISP rows = every disposition-sheet sentence ({len(disp_rows)} rows, {len(DISP)} sentences; "
      f"missing {sorted(set(DISP) - set(disp_rows))[:5]}, extra {sorted(set(disp_rows) - set(DISP))[:5]})")
r3_nv = {k for k, v in R3.items() if v[0] == "NOT-VERBATIM"}
r3_v = {k for k, v in R3.items() if v[0] == "VERBATIM"}
check(set(r3_rows) == r3_nv, f"M2 R3 rows = every obligation sentence of AD-1-R3 not verbatim in AD-1-R4 ({len(r3_rows)} rows, {len(r3_nv)} sentences; "
      f"missing {sorted(r3_nv - set(r3_rows))[:5]}, extra {sorted(set(r3_rows) - r3_nv)[:5]})")
op = flat(text.split("## Changes from AD-1-R3", 1)[0])
lost = [k for k in r3_v if flat(R3[k][1]) not in op]
check(not lost, f"M2b the {len(r3_v)} verbatim AD-1-R3 obligation sentences all stand in AD-1-R4's operative text ({lost[:3]})")
check(len(a_rows) >= 40 and len({r[1] for r in a_rows}) == len(a_rows), f"M3 part A rows (rulings and directives): {len(a_rows)}")
other = [r[1] for r in rows if r not in a_rows and not r[1].startswith(("DISP-", "R3-"))]
check(not other, f"M3b no row of unknown part ({other[:3]})")

for n, rid, kind, secs, anchors, note in rows:
    part = "A" if rid.startswith("A-") else rid.split("-")[0]
    before = len(fails)
    if kind not in KINDS.get(part, set()):
        fails.append(f"FAIL  M6 {rid}: kind {kind} not allowed for part {part}")
    labels = [s.strip() for s in secs.split(",") if s.strip()]
    unknown = [s for s in labels if s not in SEC]
    anc = [a.strip() for a in anchors.split(" ‖ ") if a.strip()]
    if unknown:
        fails.append(f"FAIL  M4 {rid}: unknown section label(s) {unknown}")
    if kind in ("DESCRIPTIVE", "MARKER"):
        if not note.strip():
            fails.append(f"FAIL  M4 {rid}: {kind} without a note")
    elif not anc or not labels:
        fails.append(f"FAIL  M4 {rid}: an operative row needs sections and at least one anchor")
    missing = [a for a in anc if not any(s in SEC and flat(a) in SEC[s] for s in labels)]
    for a in missing:
        fails.append(f"FAIL  M4 {rid}: anchor not found in {labels}: {a[:90]}")
    if rid.startswith("DISP-") and not ((kind == "VERBATIM") == (DISP[rid][0] == "VERBATIM")
                                        or (kind == "MARKER" and DISP[rid][0] == "VERBATIM")):
        fails.append(f"FAIL  M5 {rid}: VERBATIM marking disagrees with the scan ({kind}, {DISP[rid][0]})")
    if len(fails) == before:
        notes.append(f"PASS  M4-M6 {rid} ({kind}; {', '.join(labels) or '-'}): {len(anc)} anchor(s) found")
for rid, (v, s) in DISP.items():
    if v == "VERBATIM" and rid in disp_rows:
        check(disp_rows[rid][2] in ("VERBATIM", "MARKER"), f"M5b {rid}: a verbatim disposition sentence is marked VERBATIM or MARKER")

print()
for x in notes:
    print(x)
for x in fails:
    print(x)
print(f"\nMATRIX: {len(rows)} rows (part A {len(a_rows)}, DISP {len(disp_rows)}, R3 {len(r3_rows)}); "
      f"AD-1-R3 obligation sentences carried verbatim: {len(r3_v)}; {len(notes)} passed, {len(fails)} failed")
sys.exit(1 if fails else 0)
