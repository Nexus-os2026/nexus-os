#!/usr/bin/env python3
"""Lists every obligation-bearing sentence of AD-1-R1 (outside verbatim blocks and its change table) that does not
occur verbatim in AD-1-R2, so that each can be accounted for by hand. Writes nothing.
    r1-r2-obligation-scan.py <AD-1-R1> <AD-1-R2>"""
import re, sys


def sents(t):
    t = t.split("## Changes from AD-1")[0]
    t = re.sub(r"```text\n.*?\n```", "", t, flags=re.S)
    t = re.sub(r"\s+", " ", t)
    return [s.strip() for s in re.split(r"(?<=[.;:])\s+(?=[A-Z*\"'(\-0-9])", t) if s.strip()]


r1 = sents(open(sys.argv[1], encoding="utf-8").read())
r2 = " ".join(sents(open(sys.argv[2], encoding="utf-8").read()))
kw = re.compile(r"\b(must|never|only|required?|requires|not|no|every|each|unchanged|kept|stays?|INCOMPLETE|ESTABLISHED)\b")
miss = [s for s in r1 if kw.search(s) and s not in r2 and len(s) > 25]
print(f"{len(r1)} AD-1-R1 sentences; {len(miss)} obligation-bearing sentences not verbatim in AD-1-R2")
for i, s in enumerate(miss, 1):
    print(f"{i:3d}. {s[:300]}")
