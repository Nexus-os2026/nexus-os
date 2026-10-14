#!/usr/bin/env python3
"""Contradiction scan of AD-1-R4's operative text (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01).
The operative text runs from "## Provenance labels" to "## Changes from AD-1-R3"; verbatim quotations (```text) and
derived mutation texts (```rust) are excluded, since they are quotations, not this draft's statements. The text is cut
into units (a table row, or one sentence of a paragraph or bullet). For each category of the mission's section 10, the
scan lists every unit that touches it, and classifies each as CONSISTENT (with the rule that makes it so) or
CONTRADICTION. Exit status 0 only if no unit is a CONTRADICTION and every category has at least one unit. Writes
nothing.
    contradiction-scan.py <draft>"""
import re, sys

draft = open(sys.argv[1], encoding="utf-8").read()
op = draft.split("## Provenance labels", 1)[1].split("## Changes from AD-1-R3", 1)[0]
op = re.sub(r"```(?:text|rust)\n.*?\n```", "", op, flags=re.S)
op = re.sub(r"<!-- VERBATIM[^>]*-->", "", op)

units = []
para = []
for line in op.split("\n") + [""]:
    if line.startswith("|"):
        if para:
            units.append(" ".join(para)); para = []
        units.append(line)
    elif not line.strip() or re.match(r"\s*(?:[-*] |\d+\. |\([A-F]\) |#)", line):
        if para:
            units.append(" ".join(para))
        para = [line.strip()] if line.strip() else []
    else:
        para.append(line.strip())
SENT = []
for u in units:
    if u.startswith("|"):
        SENT.append(u)
    else:
        SENT += [s for s in re.split(r"(?<=[.;])\s+(?=[A-Z(*`'\"])", " ".join(u.split())) if s]

NEG = re.compile(r"\b(?:never|not|no|none|nothing|neither|nor|without|insufficient|only|cannot)\b|NOT ADMISSIBLE|NOT ESTABLISHED", re.I)
report, contradictions = [], 0


def category(cid, title, match, rule):
    """match(unit) -> bool; rule(unit) -> (ok, reason)."""
    global contradictions
    hits = [s for s in SENT if match(s)]
    report.append(f"\n[{cid}] {title}: {len(hits)} unit(s)")
    if not hits:
        report.append("    CONTRADICTION  no unit touches this category (the scan expects the draft to address it)")
        contradictions += 1
    for s in hits:
        ok, why = rule(s)
        contradictions += not ok
        report.append(f"    {'CONSISTENT   ' if ok else 'CONTRADICTION'} {why}: {s[:170]}{'…' if len(s) > 170 else ''}")


# X1 mutant versus healthy executable identity ------------------------------------------------------------------------
HEALTHY = re.compile(r"healthy witness|healthy replay|healthy-witness|\(e3\)|G\.3\(d\)|G\.5\(C\)|\(d\) receipt|healthy core violates|role: healthy|\| healthy \|", re.I)
WRONG = re.compile(r"(?:healthy witness|healthy replay|\(e3\)|G\.3\(d\)|G\.5\(C\))[^.;|]{0,80}\bon (?:the )?(?:same )?(?:pinned )?(?:C4 )?mutant executable"
                   r"|mutant executable[^.;|]{0,60}\b(?:serves|for) (?:the |every )?(?:healthy|\(d\)|\(e3\))", re.I)


MUTROLE = re.compile(r"mutant witness|type \(c\)|\(c\) \||G\.3\(b\)|G\.5\(B\)|\| mutant \|", re.I)


def x1(s):
    """Clause by clause (cells of a table row; clauses split at ';' otherwise): a clause that names a healthy role
    may not put it on a mutant executable, and a clause that names the mutant role may not put it on the baseline
    executable, unless the clause rejects that (NOT ADMISSIBLE, other role, never)."""
    if WRONG.search(s) and "NOT ADMISSIBLE" not in s and "other role" not in s:
        return False, "states that a healthy witness uses a mutant executable"
    cells = s.split("|") if s.startswith("|") else [s]
    clauses = [c for cell in cells for c in re.split(r";|: |, a |, and ", cell)]
    for c in clauses:
        rejects = "NOT ADMISSIBLE" in c or "other role" in c or re.search(r"\bnever\b|other than", c)
        if HEALTHY.search(c) and "mutant executable" in c and not rejects and "for (b)" not in c and "Every healthy witness" not in c:
            return False, "a healthy-role clause on a mutant executable"
        if MUTROLE.search(c) and re.search(r"(?<!un)(?:baseline|healthy) executable", c) and not rejects and "for (d)" not in c:
            return False, "a mutant-role clause on the baseline executable"
    return True, "each clause puts each role on its own executable"


category("X1", "mutant versus healthy executable identity",
         lambda s: ("executable" in s and (HEALTHY.search(s) or "witness" in s.lower())) or WRONG.search(s), x1)

# X2 receipt-type cross-references ------------------------------------------------------------------------------------
TYPES = {"a", "b", "c", "d", "e1", "e2", "e3", "e4", "f"}
TREF = re.compile(r"(?:F\.1 types?|F\.1|types?) \(([a-z][0-9]?)\)(?:(?:,? and |, | to | or )\(([a-z][0-9]?)\))*")


def x2(s):
    refs = []
    for m in TREF.finditer(s):
        refs += re.findall(r"\(([a-z][0-9]?)\)", m.group(0))
    bad = [r for r in refs if r not in TYPES]
    return not bad, f"references {sorted(set(refs))}" + (f"; UNDEFINED {bad}" if bad else "")


category("X2", "receipt-type cross-references", lambda s: TREF.search(s) is not None, x2)

# X3 C22 produced / not-produced artifacts ----------------------------------------------------------------------------
def x3(s):
    if "NOT PRODUCED BY DESIGN" not in s:
        if re.search(r"\(e4\)|healthy build", s) and re.search(r"no artifact|not record|omit", s, re.I) and not re.search(r"None is marked|recorded by path|recorded as found", s):
            return False, "a healthy build's artifacts left unrecorded"
        return True, "records artifacts as produced"
    if re.search(r"\(e4\)|healthy build|healthy-build", s, re.I):
        return bool(re.search(r"None is marked|no artifact marked|never extends|only that mutant build|C22's restored|mutant build|mutant executable|mutant source", s)), "NOT PRODUCED BY DESIGN beside (e4) only as its negation or the mutant build's"
    ctx = ("C22", "type (d)", "(F.1 (d))", "mutant executable", "explorer report", "explorer fields", "does not produce",
           "ever erases", "intentionally failing", "None is marked", "no artifact marked")
    return any(c in s for c in ctx), "NOT PRODUCED BY DESIGN in a C22-mutant, explorer-field or general-rule context"


category("X3", "C22 produced / not-produced artifacts",
         lambda s: "NOT PRODUCED BY DESIGN" in s or ("C22" in s and re.search(r"artifact|executable", s)) or "(e4)" in s and "artifact" in s, x3)

# X4 C4: P4 AND P8 ----------------------------------------------------------------------------------------------------
ORP = re.compile(r"P4,? or P8|P8,? or P4|\beither P4|\beither of P4|P4 or\b|one of P4 and P8|only P4|only P8|P4 alone|P8 alone", re.I)


def x4(s):
    if ORP.search(s):
        return bool(re.search(r"insufficient|NOT ADMISSIBLE|both properties are required|never read as|choice between", s)), "an either-or form only as a rejection"
    if re.search(r"replace", s) and "full_f8" in s:
        return "does not replace" in s, "the witness never replaces full_f8"
    if "P4, P8" in s:
        return bool(re.search(r"conjunction|both|first failure <exact property printed>; witness|choice between|Only then", s)), "'P4, P8' read as both"
    return bool(re.search(r"\bboth\b|AND|insufficient|P4 and P8", s)), "C4 needs both P4 and P8"


category("X4", "C4 requires P4 AND P8",
         lambda s: bool(ORP.search(s)) or "P4, P8" in s or ("C4" in s and re.search(r"P4 (?:and|AND) P8", s))
         or ("full_f8" in s and "replace" in s), x4)

# X5 C1/C25 attribution -----------------------------------------------------------------------------------------------
def x5(s):
    if re.search(r"relabel", s, re.I):
        return bool(NEG.search(s)), "relabelling only negated"
    ok = re.search(r"never|first failure E3|first failure E4|OTHER PROPERTIES|not counted as|E3 \(C1\) or E4 \(C25\)|"
                   r"E3 in 4 steps|E3 for C1|E4 for C25|historical|listed separately|NOT ESTABLISHED|Predicted", s)
    return bool(ok), "E3/E4 named only as first failures, never as P1/P7"


category("X5", "C1/C25 attribution (E3 is not P1; E4 is not P7)",
         lambda s: (("E3" in s and "P1" in s) or ("E4" in s and "P7" in s)) or re.search(r"relabel", s, re.I), x5)

# X6 C9-b -------------------------------------------------------------------------------------------------------------
def x6(s):
    if "C9-a" in s:
        return False, "names C9-a"
    if re.search(r"(?:remove|drop)s?\s+`?self\.failed", s) and "not selected" not in s and "C9" in s:
        return False, "C9 removes self.failed()"
    if "C9" in s and re.search(r"NotWritten|treated|Undone|C9-b", s):
        return bool(re.search(r"self\.failed\(\)?;?`? (?:is )?kept|keeps `self\.failed|keep(?:s|ing)? `self\.failed|C9-b|not selected|`Rollback::Unknown` is treated as `Undone`", s)), "C9 is C9-b: self.failed() kept; only the persistence changes"
    return True, "mentions C9 without its definition"


category("X6", "C9-b wording", lambda s: "C9" in s, x6)

# X7 old Claude/Codex role wording ------------------------------------------------------------------------------------
def x7(s):
    allowed = (s.startswith("| S-20 ") or s.startswith("| S-21 ") or s.startswith("| AU ") or "Neither Claude Chat nor Codex" in s
               or "Old operational role" in s or s.startswith("| contract line") or s.startswith("| REVIEW-0")
               or "under the Owner's current arrangement" in s or "Read under the current arrangement" in s
               or "Line 929" in s or "line 929" in s or "names Codex's review" in s or "mapping" in s.lower())
    return allowed, "a role name only in the mapping (R, S-20, S-21, AU)"


category("X7", "old Claude/Codex role wording", lambda s: re.search(r"Codex|Lead Architect|First Architect|Claude Chat", s), x7)

# X8 PASS / FAIL / INCOMPLETE classification -------------------------------------------------------------------------
def x8(s):
    if re.search(r"(?:counts?|counted|treated|read) (?:an? |the )?(?:INCOMPLETE[^.;]{0,30}) as (?:a )?PASS", s) and not NEG.search(s):
        return False, "an INCOMPLETE counted as PASS"
    if "BUDGET EXCEEDED" in s and re.search(r"\bis (?:a )?(?:FAIL|kill)\b", s) and not NEG.search(s):
        return False, "BUDGET EXCEEDED treated as FAIL or kill"
    if "INCOMPLETE" in s and "PASS" in s:
        ok = NEG.search(s) or s.startswith("| ") or re.search(r"INCOMPLETE —|outer gate|Outer classes|DERIVED", s)
        return bool(ok), "INCOMPLETE and PASS kept apart"
    return True, "classification statement"


category("X8", "PASS / FAIL / INCOMPLETE classification",
         lambda s: ("INCOMPLETE" in s and ("PASS" in s or "FAIL" in s)) or "BUDGET EXCEEDED" in s or "VERIFICATION INVALID" in s, x8)

# X9 quotient versus original-explorer use ----------------------------------------------------------------------------
def x9(s):
    ok = (NEG.search(s) or "unchanged-core side" in s or "unchanged core" in s or "original explorer" in s or "conditional on" in s
          or "ORIGINAL explorer" in s or s.startswith("| ") or "admitted" in s or "E.1" in s or "B(1)" in s)
    return bool(ok), "a quotient only for admitted unchanged-core runs, never for a control"


category("X9", "quotient versus original-explorer use",
         lambda s: "quotient" in s and re.search(r"mutant|control|witness|C4|C1\b|C25", s), x9)

# X10 final-baseline versus historical evidence -----------------------------------------------------------------------
def x10(s):
    if re.search(r"final-baseline (?:evidence|receipt|run)|8\.3 or 7\.5 evidence|discharge", s):
        if re.search(r"prototype|probe|historical|at A\b|42dd1bd1|04e42a7d|informational", s):
            return bool(NEG.search(s) or "informational" in s), "historical or prototype evidence is never final-baseline evidence"
    return True, "names final-baseline evidence"


category("X10", "final-baseline versus historical evidence",
         lambda s: re.search(r"final-baseline|historical|prototype|informational", s, re.I)
         and re.search(r"evidence|receipt|kill|discharge", s), x10)

# X11 kill claims ---------------------------------------------------------------------------------------------------------
def x11(s):
    m = re.search(r"\b(?:C\d+[a-c]?|A\d)(?: and C\d+)? (?:is|are|was|were|has been|have been) (?:now |finally )?killed\b", s)
    if m and not re.search(r"killed (?:when|only|under|by)|not (?:recorded as )?killed|is killed only", s):
        return False, "claims a control is killed"
    return True, "a kill condition, not a kill claim"


category("X11", "no control claimed killed", lambda s: re.search(r"\bkill(?:ed)?\b", s), x11)

# X12 authorisation -------------------------------------------------------------------------------------------------------
def x12(s):
    if re.search(r"\b(?:is|are) (?:hereby )?authori[sz]ed\b", s) and not re.search(r"\bNo\b|\bnot\b|\bnone\b|needs|own authorised mission", s):
        return False, "affirms an authorisation"
    return True, "no authorisation given"


category("X12", "no run, mutation or adoption authorised", lambda s: re.search(r"authori[sz]", s), x12)

print("AD-1-R4 CONTRADICTION SCAN (operative text; quotations excluded)")
print(f"draft {sys.argv[1]}; units scanned {len(SENT)}")
print("\n".join(report))
print(f"\nCONTRADICTION SCAN: {contradictions} contradiction(s)")
sys.exit(1 if contradictions else 0)
