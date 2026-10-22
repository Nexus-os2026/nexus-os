#!/usr/bin/env python3
"""Obligation scan (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01). Derived from AD-1-R3's r2-r3-obligation-scan.py, whose
sentence splitter and obligation keywords it keeps. Lists every obligation-bearing sentence of a source document
(AD-1-R3, or the disposition sheet) with a stable id (the first 10 hex digits of the sha256 of the whitespace-normalised
sentence), and whether it occurs verbatim in AD-1-R4. Text inside fenced blocks and the change-history tables is left
out, as in the earlier scans. Writes nothing.
With a fourth argument "all", every sentence counts (used for the disposition sheet, which is consolidated rather
than copied, so each of its sentences is accounted for).
    obligation-scan.py <source> <AD-1-R4> <label> [all]"""
import hashlib, re, sys


def sents(t):
    t = t.split("## Changes from AD-1")[0]
    t = re.sub(r"```(?:text|rust)\n.*?\n```", "", t, flags=re.S)
    t = re.sub(r"<!-- VERBATIM[^>]*-->", "", t)
    t = re.sub(r"\s+", " ", t)
    return [s.strip() for s in re.split(r"(?<=[.;:])\s+(?=[A-Z*\"'(\-0-9])", t) if s.strip()]


src, r4, label = sys.argv[1:4]
ALL = len(sys.argv) > 4 and sys.argv[4] == "all"
S = sents(open(src, encoding="utf-8").read())
R4 = " ".join(sents(open(r4, encoding="utf-8").read()))
kw = re.compile(r"\b(must|never|only|required?|requires|not|no|every|each|unchanged|kept|stays?|INCOMPLETE|ESTABLISHED)\b")
obl = [s for s in S if ALL or (kw.search(s) and len(s) > 25)]
seen, rows = set(), []
for s in obl:
    sid = hashlib.sha256(s.encode()).hexdigest()[:10]
    if sid in seen:
        continue
    seen.add(sid)
    rows.append((sid, s in R4, s))
carried = sum(1 for r in rows if r[1])
print(f"{label}: {len(S)} sentences; {len(rows)} distinct {'sentences (all counted)' if ALL else 'obligation-bearing'}; {carried} verbatim in AD-1-R4; "
      f"{len(rows) - carried} not verbatim in AD-1-R4")
for sid, v, s in rows:
    print(f"{label}-{sid} {'VERBATIM' if v else 'NOT-VERBATIM'} {s[:400]}")
