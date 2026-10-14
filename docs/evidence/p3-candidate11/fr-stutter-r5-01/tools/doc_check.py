#!/usr/bin/env python3
"""Checks the packet's documents (mission sections 17–21).

  doc_check.py DOCDIR [--disposition TSV] [--summary JSON]

DOCDIR holds FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md,
P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md,
PROPOSED-20-CONFIGURATION-ACCOUNTING.md, C15-DISPOSITION.md and (when present)
INDEX.md. Rules:

  W1  forbidden wording, anywhere outside a fenced block whose info string
      contains `prohibited-wording`
  W2  no unsupported F-R state or transition count
  W3  C15 never placed under a quotient, projection or refinement as its
      failing receipt; the original explorer and 30,000,000 stated
  W4  the F-R receipt preserved as INCOMPLETE with its printed counts
  S1  the proof's required sections, FR-1 … FR-17 and its status table
  S2  AD-1-R5's status, governing sentence, recording language, prohibited
      block, sections A–H, prerequisites, and a supersession table whose every
      row is scoped to F-R, full_fr or C15 and names only allowed clauses
  S3  the accounting's twenty rows and F-R's row
  S4  the branch disposition: 193 rows, the vocabulary's labels, every row a
      status and a provenance, none NOT_ESTABLISHED, counts consistent
Exit 0 only if every rule holds.
"""
import csv
import hashlib
import json
import os
import re
import subprocess
import sys

PROOF = "FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"
R5 = "P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
ACC = "PROPOSED-20-CONFIGURATION-ACCOUNTING.md"
C15 = "C15-DISPOSITION.md"
INDEX = "INDEX.md"
FORBIDDEN = [
    (r"\bF-R\s+PASS(ES|ED)?\b", re.I),
    (r"\bPROOF\s+ACCEPTED\b", re.I),
    (r"\bORIGINAL\s+F-R\s+GRAPH\s+(PASSED|EXPLORED|COMPLETED)\b", re.I),
    (r"\bF-R\s+ORIGINAL\s+GRAPH\s+(PASSED|EXPLORED|COMPLETED)\b", re.I),
    (r"\bAD-1-R5\s+(IS\s+)?ADOPTED\b", re.I),
    (r"\bD2A-1\s+(IS\s+)?ACCEPTED\b", re.I),
    (r"\bPHASE\s+THREE\s+(IS\s+)?COMPLETE\b", re.I),
    (r"\bINTEGRATED\b", 0),
    (r"\bF-R\b[^\n]{0,60}\boutcome:\s*PASS\b", re.I),
]
FR_NUMBERS = {"30,000,001", "26,539,923", "125,538,060", "9,877,119", "1,920", "115,660,941", "30,000,000",
              "30000001", "26539923", "125538060", "9877119", "30000000"}
CONFIGS = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7", "F-8", "F-PT",
           "F-B2", "F-B", "F-G", "F-W", "F-D"]
NEG = re.compile(r"\b(never|not|no|cannot|nothing|none|neither|nor)\b", re.I)
VOCAB_SHA = "db8734e0bd2743f22a27fb4001e8deb0f6381b393aecc33c781d710ee57fa932"
AD1R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"


class R:
    def __init__(self):
        self.lines, self.fail = [], 0

    def check(self, cid, ok, what):
        self.lines.append(f"{'PASS' if ok else 'FAIL'}  {cid}  {what}")
        self.fail += 0 if ok else 1


def outside_prohibited(text):
    """Units of text outside prohibited-wording fences: each table cell, and
    each sentence of each prose paragraph (lines joined, so that a phrase
    split across lines is still seen). Returned as (first line number, unit)."""
    units, fence, para, para_start = [], None, [], None

    def flush():
        nonlocal para, para_start
        if para:
            joined = " ".join(x.strip() for x in para)
            for sent in re.split(r"(?<=[.!?])\s+(?=[A-Z*\"(`\[])", joined):
                units.append((para_start, sent))
        para, para_start = [], None

    for n, line in enumerate(text.split("\n"), 1):
        st = line.strip()
        if st.startswith("```"):
            flush()
            fence = (("prohibited" if "prohibited-wording" in st else "other") if fence is None else None)
            continue
        if fence == "prohibited":
            continue
        if fence == "other":
            units.append((n, line))
            continue
        if st.startswith("|"):
            flush()
            for cell in st.strip("|").split("|"):
                units.append((n, cell))
            continue
        if not st or st.startswith("#"):
            flush()
            if st:
                units.append((n, st))
            continue
        if para_start is None:
            para_start = n
        para.append(line)
    flush()
    return units


def strip_citations(line):
    line = re.sub(r"\[[^\[\]]*· blob [0-9a-f]{8} · item [0-9a-f]{16}\]", "", line)
    return re.sub(r"\b[0-9a-f]{12,64}\b", "", line)


def main(argv):
    d = argv[1]
    disp_tsv = argv[argv.index("--disposition") + 1] if "--disposition" in argv else os.path.join(d, "FR-BRANCH-DISPOSITION.tsv")
    summary = json.load(open(argv[argv.index("--summary") + 1])) if "--summary" in argv else None
    docs = {n: open(os.path.join(d, n), encoding="utf-8").read() for n in (PROOF, R5, ACC, C15, INDEX)
            if os.path.exists(os.path.join(d, n))}
    rep = R()
    rep.check("D0", all(n in docs for n in (PROOF, R5, ACC, C15)), f"documents present: {sorted(docs)}")
    # W1
    hits = []
    for name, text in docs.items():
        for n, line in outside_prohibited(text):
            for pat, fl in FORBIDDEN:
                if re.search(pat, line, fl):
                    hits.append(f"{name}:{n}: /{pat}/ in: {line.strip()[:90]}")
    rep.check("W1", not hits, f"no forbidden wording outside a prohibited-wording block ({hits})")
    # W2
    bad = []
    for name, text in docs.items():
        for n, line in outside_prohibited(text):
            ln = strip_citations(line)
            for m in re.finditer(r"F-R(?![-\w])", ln):
                seg = ln[m.end():m.end() + 160]
                for x in re.finditer(r"\b(\d{1,3}(?:,\d{3})+|\d{4,})\s+(states|transitions|keys|edges|end states|no-step)\b", seg):
                    between = seg[:x.start()]
                    if any(re.search(rf"(?<![-\w]){re.escape(c)}(?![-\w])", between) for c in CONFIGS) or re.search(r"[.|;]\s", between):
                        continue
                    if x.group(1) not in FR_NUMBERS:
                        bad.append(f"{name}:{n}: {x.group(0)} after F-R")
            if re.search(r"F-R(?![-\w])", ln) and re.search(r"\b(total|complete|whole)\s+(state|transition)s?\s+(count|number)", ln, re.I):
                nums = [y for y in re.findall(r"\b(\d{1,3}(?:,\d{3})+|\d{4,})\b", ln) if y not in FR_NUMBERS]
                if nums or not NEG.search(ln) and "not stated" not in ln:
                    bad.append(f"{name}:{n}: a total count for F-R: {line.strip()[:90]}")
            m2 = re.search(r"F-R\s+has\s+([\d,]+)", ln)
            if m2 and m2.group(1) not in FR_NUMBERS:
                bad.append(f"{name}:{n}: F-R has {m2.group(1)}")
    rep.check("W2", not bad, f"no unsupported F-R state or transition count ({bad})")
    # W3
    bad3 = []
    for name, text in docs.items():
        for n, line in outside_prohibited(text):
            if "C15" not in line:
                continue
            if re.search(r"quotient|projection", line, re.I) and not NEG.search(line):
                bad3.append(f"{name}:{n}: {line.strip()[:100]}")
            if re.search(r"refinement", line, re.I) and not (NEG.search(line) or re.search(r"restor|unchanged-core side", line, re.I)):
                bad3.append(f"{name}:{n}: {line.strip()[:100]}")
    for name in (R5, C15):
        if name in docs:
            paras = re.split(r"\n\s*\n", docs[name])
            if not any("C15" in p and re.search(r"original\s+explorer", p, re.I) and "30,000,000" in p for p in paras):
                bad3.append(f"{name}: no paragraph keeps C15 on the original explorer under 30,000,000")
            if not any("C15" in p and re.search(r"FAIL[s]? E8 within the cap", p) for p in paras):
                bad3.append(f"{name}: no paragraph requires C15 to FAIL E8 within the cap")
    rep.check("W3", not bad3, f"C15 stays original-explorer, 30,000,000, E8 within the cap; never a quotient or projection receipt ({bad3})")
    # W4
    miss = [n for n in (PROOF, R5, ACC) if n in docs and not all(s in docs[n] for s in ("INCOMPLETE", "30,000,001", "26,539,923"))]
    rep.check("W4", not miss, f"proof, AD-1-R5 and accounting keep the F-R receipt INCOMPLETE with 30,000,001 / 26,539,923 ({miss})")
    # S1 proof
    p = docs.get(PROOF, "")
    heads = [l for l in p.split("\n") if l.startswith("### ") or l.startswith("## ")]
    fr_in_heads = {int(x) for h in heads if h.startswith("### ") for x in re.findall(r"FR-(\d+)(?![\d.])", h)}
    table = {int(m.group(1)): m.group(2) for m in re.finditer(r"^\| FR-(\d+) [^|]*\| ([^|]+)\|", p, re.M)}
    need = ["## 2. Definitions", "## 3. Exact source facts", "### PROJ-1", "### FR-RS", "## 5. Dependency graph",
            "## 6. Safety", "## 7. D1", "## 8. L1", "## 9. Branch accounting", "## 10. Scope and limitations",
            "## 11. What would invalidate it", "## 12. Status of every obligation"]
    rep.check("S1a", "**EXECUTOR ARGUMENT — NOT ARCHITECT ACCEPTED.**" in p, "proof labelled EXECUTOR ARGUMENT — NOT ARCHITECT ACCEPTED")
    rep.check("S1b", fr_in_heads >= set(range(1, 18)), f"FR-1 … FR-17 each argued under a heading (missing {sorted(set(range(1, 18)) - fr_in_heads)})")
    rep.check("S1c", set(table) == set(range(1, 18)) and all(v.strip().startswith(("ESTABLISHED", "REJECTED")) for v in table.values()),
              f"status table has FR-1 … FR-17, each ESTABLISHED or REJECTED ({sorted(table)})")
    rep.check("S1d", all(any(h.startswith(x) for h in heads) for x in need), f"required sections present (missing {[x for x in need if not any(h.startswith(x) for h in heads)]})")
    rep.check("S1e", "FR-17.X" in p and "REPLAY_OCCURRENCE_CLOSURE_COMPLETE" in p, "FR-17.X argued; occurrence-closure result stated")
    # S2 R5
    r5 = docs.get(R5, "")
    rep.check("S2a", "**STATUS: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. NOT IN FORCE.**" in r5, "R5 status: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED")
    rep.check("S2b", "AD-1-R4 remains governing until the Owner adopts R5." in r5 and AD1R4_SHA in r5, "R5: AD-1-R4 remains governing; its sha256 cited")
    rep.check("S2c", "> F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF" in r5, "R5: the required recording language")
    blocks = re.findall(r"```[^\n]*prohibited-wording\n(.*?)```", r5, re.S)
    rep.check("S2d", blocks and all(x in blocks[0] for x in ("F-R PASS", "F-R ORIGINAL GRAPH PASSED", "F-R ORIGINAL GRAPH EXPLORED")),
              "R5: the prohibited wording listed (F-R PASS, F-R ORIGINAL GRAPH PASSED, F-R ORIGINAL GRAPH EXPLORED)")
    secs = [s for s in "ABCDEFGH" if re.search(rf"^## {s}\. ", r5, re.M)]
    rep.check("S2e", secs == list("ABCDEFGH"), f"R5 sections A–H ({secs})")
    pre = re.search(r"^## C\..*?(?=^## D\.)", r5, re.S | re.M)
    pre_t = pre.group(0) if pre else ""
    needed = ["exact frozen baseline", "F-2 original-explorer PASS", "diagnostic", "source-occurrence closure",
              "FR-1 to FR-17 proof", "branch accounting", "Sole Architect review", "Owner adoption"]
    rep.check("S2f", all(x in pre_t for x in needed), f"R5 C lists every prerequisite (missing {[x for x in needed if x not in pre_t]})")
    rows = re.findall(r"^\| (S5-\d+) \| ([^|]+) \| ([^|]+) \|", r5, re.M)
    allowed_clause = re.compile(r"E\.3 row 15|E\.2|line 863|line 923|AD-1-R4 Q|7\.4|line 853")
    bad_rows = [r[0] for r in rows if not (re.search(r"F-R|full_fr|C15", r[2]) and "only" in r[2] and allowed_clause.search(r[1]))]
    rep.check("S2g", len(rows) >= 7 and not bad_rows,
              f"every R5 supersession row is scoped to F-R, full_fr or C15 only and names an allowed clause ({len(rows)} rows; bad {bad_rows})")
    disc = []
    for n, line in outside_prohibited(r5):
        if re.search(r"discharg", line, re.I):
            others = [c for c in CONFIGS if c != "F-2" and re.search(rf"(?<![-\w]){re.escape(c)}(?![-\w])", line)]
            if others and not NEG.search(line):
                disc.append(f"{n}: {others}")
    rep.check("S2h", not disc and "No other configuration" in r5 and "no general permission" in r5.lower(),
              f"R5 makes no other configuration proof-dischargeable and creates no general permission ({disc})")
    # S3 accounting
    acc = docs.get(ACC, "")
    rows3 = re.findall(r"^\| (\d+) \| (X-\d|F-[A-Z0-9]+) \|", acc, re.M)
    fr_row = [l for l in acc.split("\n") if re.match(r"^\| 20 \| F-R \|", l)]
    rep.check("S3a", [r[1] for r in rows3] == ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7",
                                              "F-8", "F-PT", "F-B", "F-B2", "F-G", "F-W", "F-D", "F-R"],
              f"accounting lists the twenty in order ({[r[1] for r in rows3]})")
    rep.check("S3b", bool(fr_row) and "INCOMPLETE" in fr_row[0] and "PROOF-DISCHARGED" in fr_row[0] and "pending" in fr_row[0]
              and "not a total" in fr_row[0], "accounting's F-R row: INCOMPLETE; PROOF-DISCHARGED pending; no total")
    rep.check("S3c", acc.count("| PASS |") == 19 and "**not stated**" in acc, "accounting: nineteen PASS rows; F-R totals not stated")
    # S4 disposition
    vocab = json.loads(subprocess.run(["git", "-C", "/home/nexus/NEXUS/nexus-os", "show",
                                       "89326b859792a442c1853c3d35d30a48301c009e:docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/vocabulary/VOCABULARY.json"],
                                      check=True, capture_output=True).stdout)
    vsha = hashlib.sha256(json.dumps(vocab).encode()).hexdigest()  # not the file hash; labels are compared below
    vlabels = [r["label"] for r in vocab["rows"]]
    drows = list(csv.reader(open(disp_tsv), delimiter="\t"))[1:]
    dl = [r[2] for r in drows]
    statuses = {}
    for r in drows:
        statuses[r[5]] = statuses.get(r[5], 0) + 1
    ok4 = (len(drows) == 193 and dl == vlabels and all(r[5] in ("TAKEN", "NOT TAKEN", "NOT APPLICABLE") and r[6] for r in drows)
           and not any("NOT_ESTABLISHED" in r[6] for r in drows))
    rep.check("S4a", ok4, f"disposition: 193 rows in vocabulary order, each with status and provenance, none NOT_ESTABLISHED ({len(drows)}; {statuses})")
    replay = {r[2]: r[5] for r in drows if r[2] in ("resolve: Stale", "append_cell C: Stale", "append_cell X: Stale", "empty: Stale")}
    rep.check("S4b", replay == {"resolve: Stale": "TAKEN", "append_cell C: Stale": "TAKEN", "append_cell X: Stale": "NOT TAKEN", "empty: Stale": "TAKEN"},
              f"the four replay labels are each disposed ({replay})")
    prefix_rows = [r for r in drows if "MEASURED_IN_FR_PREFIX_ONLY" in r[6] and "complete" in r[6].lower() and "not complete" not in r[6].lower() and "not coverage" not in r[6].lower() and "corroboration" not in r[6].lower() and "establishes nothing" not in r[6].lower()]
    rep.check("S4c", not prefix_rows, f"no prefix-only observation is described as complete measurement ({[r[2] for r in prefix_rows]})")
    t = statuses.get("TAKEN", 0)
    nt = statuses.get("NOT TAKEN", 0)
    na = statuses.get("NOT APPLICABLE", 0)
    cons = [n for n in (PROOF, ACC) if n in docs and not re.search(rf"\b{t}\s+TAKEN,\s+{nt}\s+NOT\s+TAKEN,\s+{na}\s+NOT\s+APPLICABLE", docs[n].replace("\n", " "))]
    if summary is not None and summary.get("fr_status_by_proof") != {"TAKEN": t, "NOT TAKEN": nt, "NOT APPLICABLE": na}:
        cons.append("summary")
    rep.check("S4d", not cons, f"the documents state the disposition's own counts ({t} TAKEN, {nt} NOT TAKEN, {na} NOT APPLICABLE) ({cons})")
    for line in rep.lines:
        print(line)
    print(f"DOC CHECK: {len(rep.lines)} checks, {rep.fail} failed")
    return 0 if rep.fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
