#!/usr/bin/env python3
"""Seeded controls for static_gate.py (read-only for ROOT; every seed works on a disposable copy).

    gate_controls.py ROOT

Each seed copies ROOT/inputs and ROOT/out/catalog into a scratch root, injects one defect, runs static_gate.py on
the copy and requires that the named check FAILS. The positive control requires that the unmodified copy fails
exactly the 21 definition-completeness checks and nothing else. Exit 0 only if every control behaves as required.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from derive_catalog import make_patch, PROTOCOL  # noqa: E402

REPO = "/home/nexus/NEXUS/nexus-os"
BASE = subprocess.run(["git", "-C", REPO, "cat-file", "blob", "a2df2841b99cad5d452008a6d4150e9369d07e66"],
                      capture_output=True, check=True).stdout


def entry(cat, cid):
    return next(e for e in cat["controls"] if e["id"] == cid)


def set_patch(root, cat, cid, anchor, repl):
    e = entry(cat, cid)
    e["anchor"], e["replacement"] = anchor, repl
    p = make_patch(PROTOCOL, BASE, BASE.replace(anchor.encode(), repl.encode(), 1))
    e["patch"] = p.decode()
    open(os.path.join(root, "out", "catalog", e["patch_file"]), "wb").write(p)


def s_dup(root, cat):
    entry(cat, "C2")["id"] = "C1"


def s_a4(root, cat):
    entry(cat, "A4")["not_a_mutation"] = False


def s_anchor(root, cat):
    entry(cat, "C21")["anchor"] = "        } else if "


def s_confine(root, cat):
    e = entry(cat, "C14")
    e["anchor"] = e["anchor"][20:]


def s_patch(root, cat):
    e = entry(cat, "C27")
    p = e["patch"].replace("        let mut op = Publish::new(self);\n", "        let mut op = Publish::old(self);\n", 1)
    e["patch"] = p
    open(os.path.join(root, "out", "catalog", e["patch_file"]), "w").write(p)


def s_test(root, cat):
    entry(cat, "C14")["test"] = "ledger::protocol::unit_tests::t_bind_no_such_test"


def s_quotient(root, cat):
    entry(cat, "C9")["test"] = "ledger::protocol::model_tests::gc_symmetry_fb"


def s_link(root, cat):
    entry(cat, "C9")["restored_receipt"]["executable_before_after_equal"] = False


def s_c25(root, cat):
    set_patch(root, cat, "C25",
              "                    self.keep = true;\n                    return refused(Persistence::Uncertain, "
              "Reason::Storage);",
              "                    return GatingOutcome::Committed(seq);")


def s_c9a(root, cat):
    set_patch(root, cat, "C9",
              "                self.failed();\n                Persistence::Uncertain\n            }",
              "                Persistence::NotWritten\n            }")


def s_c22(root, cat):
    entry(cat, "C22")["mutant_executable"] = "0" * 64


def s_blocked(root, cat):
    entry(cat, "C11")["anchor"] = "                    Reason::Constraint\n"


def s_conflate(root, cat):
    entry(cat, "C21")["mutant_blob"] = entry(cat, "C19")["mutant_blob"]


def s_killed(root, cat):
    entry(cat, "C12")["execution_status"] = "KILLED"


SEEDS = [
    ("duplicate id", s_dup, "every id occurs exactly once in the catalog"),
    ("A4 folded into the mutations", s_a4, "A4 separate"),
    ("anchor with several occurrences", s_anchor, "C21: anchor occurs exactly once"),
    ("anchor span not covering the change", s_confine, "C14: changed bytes confined to the anchor's span"),
    ("patch that does not apply", s_patch, "C27: GNU patch on a disposable copy gives the anchor replacement"),
    ("named test absent", s_test, "C14: named test t_bind_no_such_test exists"),
    ("quotient failing receipt", s_quotient, "C9: failing receipt is never a quotient run"),
    ("restored link on another executable", s_link, "C9: e1 link 61-full-F-B2 resolves"),
    ("compound C25 (drops self.keep = true)", s_c25, "C25 is the narrow definition"),
    ("C9-a (drops self.failed())", s_c9a, "C9 is C9-b"),
    ("C22 with a mutant executable", s_c22, "C22 produces no mutant executable by design"),
    ("BLOCKED control given an anchor", s_blocked, "C11: BLOCKED entry carries no anchor"),
    ("two controls with one mutant blob", s_conflate, "C21: mutant_blob distinct from every other control"),
    ("a control marked killed", s_killed, "no control is marked killed or executed"),
]


def run(root, seed):
    with tempfile.TemporaryDirectory() as d:
        shutil.copytree(os.path.join(root, "inputs"), os.path.join(d, "inputs"))
        shutil.copytree(os.path.join(root, "out", "catalog"), os.path.join(d, "out", "catalog"))
        if seed:
            path = os.path.join(d, "out", "catalog", "catalog.json")
            cat = json.load(open(path))
            seed(d, cat)
            json.dump(cat, open(path, "w"), indent=1, sort_keys=True)
        r = subprocess.run([sys.executable, os.path.join(HERE, "static_gate.py"), d], capture_output=True, text=True)
        return r.returncode, [l[5:] for l in r.stdout.splitlines() if l.startswith("FAIL ")]


def main(root):
    ok_all = True
    rc, failed = run(root, None)
    defs = [f for f in failed if "exact definition available from an authorised source" in f]
    other = [f for f in failed if f not in defs]
    ok = rc == 1 and len(defs) == 21 and not other
    ok_all &= ok
    print(f"{'OK  ' if ok else 'FAIL'} positive control (unmodified copy): exit {rc}; definition FAILs {len(defs)} "
          f"(required 21); other FAILs {len(other)} (required 0) {other}")
    for name, seed, label in SEEDS:
        rc, failed = run(root, seed)
        hit = [f for f in failed if f.startswith(label)]
        ok = rc == 1 and bool(hit)
        ok_all &= ok
        print(f"{'OK  ' if ok else 'FAIL'} seed '{name}': exit {rc}; required FAIL '{label}…' "
              f"{'present' if hit else 'ABSENT'}; FAIL lines {len(failed)}")
    print(f"GATE CONTROLS: {'ALL BEHAVE AS REQUIRED' if ok_all else 'NOT ALL AS REQUIRED'} "
          f"({len(SEEDS)} seeds + 1 positive)")
    return 0 if ok_all else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
