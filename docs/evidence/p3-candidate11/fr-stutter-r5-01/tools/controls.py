#!/usr/bin/env python3
"""Negative and positive controls for both checkers and the document checker
(mission section 20).

  controls.py WORKDIR DOCDIR DISPOSITION_SUMMARY

Each source control writes one seeded copy of one file under WORKDIR (never
into a repository) and runs the checkers with --override. Each document
control writes a seeded copy of the documents under WORKDIR. A control is
DETECTED only if every detection it expects occurs; a positive control only
if every checker passes. Meta-controls show that each seeded copy differs
from the original and that its anchor occurred exactly once.
"""
import hashlib
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import a_source  # noqa: E402

MT = "ledger/protocol/model_tests.rs"
PR = "ledger/protocol.rs"
PROOF = "FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"
R5 = "P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
ACC = "PROPOSED-20-CONFIGURATION-ACCOUNTING.md"

SOURCE = [
    ("SC01", "an unclassified replay occurrence (step_fw reads w.replays)", MT,
     "    let members = w.fw[i].members.clone();\n    match w.fw[i].pos {",
     "    let members = w.fw[i].members.clone();\n    let _seeded = w.replays;\n    match w.fw[i].pos {",
     {"A": ["occurrences"], "B": ["B1", "B80"]}),
    ("SC02", "a property reading Wk.last (state_properties)", MT,
     "    let word = word_of(&w.core.shared);\n    if word & FLAGS & !HANDOVER_FAULT != expected_flags(&w.core) {",
     "    let word = word_of(&w.core.shared);\n    let _seeded = w.wk.last.is_some();\n    if word & FLAGS & !HANDOVER_FAULT != expected_flags(&w.core) {",
     {"A": ["occurrences", "closure"], "B": ["B1", "B80", "B82"]}),
    ("SC03", "a property reading the replay counter (transition_properties)", MT,
     "    let (wb, wa) = (word_of(&before.core.shared), word_of(&after.core.shared));",
     "    let (wb, wa) = (word_of(&before.core.shared), word_of(&after.core.shared));\n    let _seeded = after.replays;",
     {"A": ["occurrences", "closure"], "B": ["B1", "B80", "B83"]}),
    ("SC04", "terminal reading replay history", MT,
     "        && w.adm == 0\n        && (!config.env || w.env_done);",
     "        && w.adm == 0\n        && w.replays == 0\n        && (!config.env || w.env_done);",
     {"A": ["occurrences", "closure", "cite"], "B": ["B1", "B80", "B81"]}),
    ("SC05", "an ordinary actor changing the replay counter (step_se)", MT,
     "        SePos::NoTicket(_) => {\n            w.se.pos = SePos::Idle;",
     "        SePos::NoTicket(_) => {\n            w.replays = 3;\n            w.se.pos = SePos::Idle;",
     {"A": ["occurrences", "closure", "cite"], "B": ["B1", "B80", "B87"]}),
    ("SC06", "REPLAY failing to decrement", MT,
     "    w.replays -= 1;\n    obs.note(|| \"the worker repeats",
     "    obs.note(|| \"the worker repeats",
     {"A": ["occurrences", "cite"], "B": ["B2", "B30"]}),
    ("SC07", "REPLAY changing the core", MT,
     "    w.replays -= 1;\n    obs.note(|| \"the worker repeats",
     "    w.replays -= 1;\n    w.core.frontier += 1;\n    obs.note(|| \"the worker repeats",
     {"A": ["cite"], "B": ["B30", "B89"]}),
    ("SC08", "REPLAY changing the ledger rows", MT,
     "    w.replays -= 1;\n    obs.note(|| \"the worker repeats",
     "    w.replays -= 1;\n    w.rows.push(SimRow::Fact(0));\n    obs.note(|| \"the worker repeats",
     {"A": ["cite"], "B": ["B30", "B89"]}),
    ("SC09", "REPLAY consuming an Oracle choice", MT,
     "    w.replays -= 1;\n    obs.note(|| \"the worker repeats",
     "    w.replays -= 1;\n    let _ = o.branch(2);\n    obs.note(|| \"the worker repeats",
     {"A": ["cite"], "B": ["B30", "B89"]}),
    ("SC10", "REPLAY creating a fault edge", MT,
     "    w.replays -= 1;\n    obs.note(|| \"the worker repeats",
     "    w.replays -= 1;\n    let _ = o.choose(&[false, true]);\n    obs.note(|| \"the worker repeats",
     {"A": ["cite"], "B": ["B30", "B89"]}),
    ("SC11", "a stale operation reaching storage (Publish::cell opens before the stale test)", PR,
     "        let core = &mut *self.core;\n        if !(core.names(token, Cell::C) || core.names(token, Cell::X)) {",
     "        let _ = io.open();\n        let core = &mut *self.core;\n        if !(core.names(token, Cell::C) || core.names(token, Cell::X)) {",
     {"A": ["cite"], "B": ["B51"]}),
    ("SC12", "REPLAY inserted before an existing actor", MT,
     "    all.push(Actor::Adm);\n    if config.env {\n        all.push(Actor::Env);\n    }\n    if config.replays > 0 {\n        all.push(Actor::Replay);\n    }\n",
     "    if config.replays > 0 {\n        all.push(Actor::Replay);\n    }\n    all.push(Actor::Adm);\n    if config.env {\n        all.push(Actor::Env);\n    }\n",
     {"A": ["cite"], "B": ["B10"]}),
    ("SC13", "an omitted replay action (EmptyX dropped from REPLAY's match)", MT,
     " | Action::EmptyX(t, _) => t,",
     " => t,",
     {"A": ["occurrences", "cite"], "B": ["B2", "B30"]}),
]

DOCS = [
    ("DC14", "an omitted replay branch (the 'empty: Stale' row removed from the disposition)", "disposition", "empty: Stale", None, ["S4a", "S4b"]),
    ("DC15", "an unsupported F-R state-count claim", ACC, "## F-R\n", "## F-R\n\nThe complete F-R graph has 31,000,000 states.\n", ["W2"]),
    ("DC16", "wording that calls F-R PASS", R5, "## H. No general permission\n", "## H. No general permission\n\nF-R PASS is recorded here.\n", ["W1"]),
    ("DC17", "wording that moves C15 to a quotient", R5, "## H. No general permission\n", "## H. No general permission\n\nC15 may use the composed quotient as its failing receipt.\n", ["W3"]),
    ("DC18", "AD-1-R5 altering an unrelated clause (F-1's ceiling)", R5, "| S5-7 |", "| S5-8 | AD-1-R4 A (F-1's ceiling) | REPLACED for F-1: 90,000,000 states | A |\n| S5-7 |", ["S2g"]),
    ("DC19", "AD-1-R5 making another configuration dischargeable", R5, "## H. No general permission\n", "## H. No general permission\n\nF-3 may also be discharged by a refinement proof.\n", ["S2h"]),
    ("DC20", "the proof omitting FR-12's argument", PROOF, "### FR-12 — D1 transfer", "### D1 transfer", ["S1b"]),
    ("DC21", "the F-R receipt's expanded count dropped from the accounting (every occurrence)", ACC, "26,539,923", "", ["W4"], "all"),
    ("DC22", "a forbidden phrase split across two lines", R5, "## H. No general permission\n", "## H. No general permission\n\nThis records F-R\nPASS for the Owner.\n", ["W1"]),
    ("DC23", "a prefix-only observation called a complete measurement", "disposition",
     "MEASURED_IN_FR_PREFIX_ONLY (taken by a transition from an expanded, hence reachable, F-R state; the run is INCOMPLETE and is not complete coverage)",
     "MEASURED_IN_FR_PREFIX_ONLY (a complete F-R measurement)", ["S4c"]),
]


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def failed_ids(out):
    return set(re.findall(r"^FAIL\s+(\S+)", out, re.M))


def main(work, docdir, summary):
    if os.path.exists(work):
        shutil.rmtree(work)
    os.makedirs(work)
    src = a_source.Source()
    proof = os.path.join(docdir, PROOF)
    lines, bad = [], 0
    py = sys.executable

    def A(tag, extra):
        out = os.path.join(work, tag)
        res = {}
        res["occurrences"] = run([py, f"{HERE}/checker_a.py", "occurrences", out + "/occ", *extra])
        res["closure"] = run([py, f"{HERE}/checker_a.py", "closure", out + "/clo", *extra])
        res["cite"] = run([py, f"{HERE}/checker_a.py", "cite", proof, *extra])
        return res

    def B(tag, extra):
        out = os.path.join(work, tag)
        main_ = run([py, f"{HERE}/checker_b.py", out + "/b", *extra])
        cite = run([py, f"{HERE}/checker_b.py", "cite", proof, *extra])
        return main_, cite

    # positive control
    a = A("PC1", [])
    b, bc = B("PC1", [])
    ok = all(v[0] == 0 for v in a.values()) and b[0] == 0 and bc[0] == 0
    lines.append(f"{'OK  ' if ok else 'FAIL'}  PC1  unmodified sources: A occurrences/closure/cite and B checks/cite all pass")
    bad += 0 if ok else 1
    for cid, what, rel, anchor, repl, expect in SOURCE:
        data, txt = src.text(rel)
        n = txt.count(anchor)
        seeded = txt.replace(anchor, repl, 1)
        path = os.path.join(work, cid, os.path.basename(rel))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "w", encoding="utf-8").write(seeded)
        meta = n == 1 and hashlib.sha256(seeded.encode()).hexdigest() != hashlib.sha256(txt.encode()).hexdigest()
        extra = ["--override", f"{rel}={path}"]
        a = A(cid, extra)
        b, bc = B(cid, extra)
        a_hit = [k for k in expect["A"] if a[k][0] != 0]
        b_fail = failed_ids(b[1]) | ({"cite"} if bc[0] != 0 else set())
        b_hit = [k for k in expect["B"] if k in b_fail]
        det = meta and a_hit == expect["A"] and b_hit == expect["B"]
        lines.append(f"{'OK  ' if det else 'FAIL'}  {cid}  {what}: anchor x{n}, seeded copy differs: {meta}; "
                     f"A detects by {a_hit} (expected {expect['A']}); B fails {sorted(b_fail)} (expected ⊇ {expect['B']})")
        bad += 0 if det else 1
    # document controls
    rc, out = run([py, f"{HERE}/doc_check.py", docdir, "--summary", summary])
    ok = rc == 0
    lines.append(f"{'OK  ' if ok else 'FAIL'}  PC2  unmodified documents: doc_check passes")
    bad += 0 if ok else 1
    for entry in DOCS:
        cid, what, target, anchor, repl, expect = entry[:6]
        every = len(entry) > 6 and entry[6] == "all"
        d = os.path.join(work, cid)
        shutil.copytree(docdir, d)
        disp = os.path.join(d, "FR-BRANCH-DISPOSITION.tsv")
        if target == "disposition":
            t = open(disp, encoding="utf-8").read()
            if repl is None:
                keep = [l for l in t.split("\n") if f"\t{anchor}\t" not in l]
                n = len(t.split("\n")) - len(keep)
                new = "\n".join(keep)
            else:
                n = t.count(anchor)
                new = t.replace(anchor, repl, 1)
            open(disp, "w", encoding="utf-8").write(new)
        else:
            p = os.path.join(d, target)
            t = open(p, encoding="utf-8").read()
            n = t.count(anchor)
            new = t.replace(anchor, repl) if every else t.replace(anchor, repl, 1)
            open(p, "w", encoding="utf-8").write(new)
        meta = n >= 1 and new != t
        rc, out = run([py, f"{HERE}/doc_check.py", d, "--summary", summary])
        fails = failed_ids(out)
        det = meta and rc != 0 and all(e in fails for e in expect)
        lines.append(f"{'OK  ' if det else 'FAIL'}  {cid}  {what}: anchor x{n}, seeded copy differs: {meta}; doc_check fails {sorted(fails)} (expected ⊇ {expect})")
        bad += 0 if det else 1
    for l in lines:
        print(l)
    total = len(lines)
    print(f"CONTROLS: {total - bad} of {total} behave as required ({len(SOURCE)} source, {len(DOCS)} document, 2 positive)")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
