#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission section 10: positive control, checker agreement and seeded
negative controls (read-only with respect to the repository).

    negative_controls.py ROOT

Positive control: both checkers run on the real candidate catalog; both must PASS and their per-id verdict maps must
be identical. Negative controls: each seeds one defect into a disposable copy of the catalog directory (under
ROOT/dev-local, deleted afterwards). A seeded definition defect is made fully consistent first: its patch, patch
sha256, mutant blob and sha256 and anchor lines are re-derived from the defective edits, so that only the defect
itself can be caught. Each seed must be detected by BOTH checkers: exit status 1 and at least one failing check for
the seeded id in one of the expected check classes. Any disagreement or any undetected seed fails this tool.
"""
import copy
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from derive_catalog import make_patch, removed_added  # noqa: E402  (patch generation for seeded definitions only)

PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
INPUT = {PROTOCOL: "C-protocol.rs", MODEL: "C-protocol-model_tests.rs"}
TOOLS = os.path.dirname(os.path.abspath(__file__))


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


class Seed:
    def __init__(self, root, catdir):
        self.root, self.catdir = root, catdir
        self.cat = json.load(open(os.path.join(catdir, "catalog.json")))

    def by(self, cid):
        return next(e for e in self.cat["controls"] if e["id"] == cid)

    def base(self, path):
        return open(os.path.join(self.root, "inputs", INPUT[path]), "rb").read().decode()

    def redefine(self, cid, edits):
        """Replace a control's edits and re-derive every identity so that only the seeded defect remains."""
        e = self.by(cid)
        path = e["mutation_source_file"]
        bt = self.base(path)
        mt = bt
        for a, r in edits:
            assert mt.count(a) == 1, (cid, a)
            mt = mt.replace(a, r, 1)
        patch = make_patch(path, bt.encode(), mt.encode())
        e["edits"] = [{"anchor": a, "replacement": r, "expected_count": 1,
                       "anchor_first_line_at_C": bt.count("\n", 0, bt.index(a)) + 1} for a, r in edits]
        e["anchor"], e["replacement"] = (edits[0] if len(edits) == 1 else (None, None))
        e.update({"patch": patch.decode(), "patch_sha256": sha(patch), "mutant_blob": blob_id(mt.encode()),
                  "mutant_sha256": sha(mt.encode()), "patch_lines_removed_added": removed_added(patch)})
        self.write_patch(cid, patch)

    def edits(self, cid):
        return [(x["anchor"], x["replacement"]) for x in self.by(cid)["edits"]]

    def write_patch(self, cid, patch):
        with open(os.path.join(self.catdir, "patches", f"{cid}.patch"), "wb") as fh:
            fh.write(patch)

    def save(self):
        with open(os.path.join(self.catdir, "catalog.json"), "w") as fh:
            json.dump(self.cat, fh, indent=1, sort_keys=True, ensure_ascii=False)
            fh.write("\n")


def seeds():
    S = []

    def seed(name, cid, classes):
        def deco(fn):
            S.append((name, cid, set(classes), fn))
            return fn
        return deco

    @seed("omission of one of the 35 ids (C17 removed)", "catalog", ["ids"])
    def _(s):
        s.cat["controls"] = [e for e in s.cat["controls"] if e["id"] != "C17"]

    @seed("duplicate id (C18a twice)", "catalog", ["ids"])
    def _(s):
        s.cat["controls"].append(copy.deepcopy(s.by("C18a")))

    @seed("A4 counted as a mutation", "A4", ["a4"])
    def _(s):
        a4 = s.by("A4")
        a4.update({"counted_as_kill_required": True, "definition_status": "CANDIDATE_READY"})

    @seed("changed property (C11 names E5)", "C11", ["property"])
    def _(s):
        s.by("C11")["named_property"] = ["E5"]

    @seed("changed configuration (C17 on F-B)", "C17", ["property"])
    def _(s):
        s.by("C17")["configuration"] = "F-B"

    @seed("anchor cardinality mismatch (C23's anchor shortened to a text occurring five times)", "C23", ["anchor"])
    def _(s):
        e = s.by("C23")
        e["edits"][0]["anchor"] = "            | bit("
        e["anchor"] = "            | bit("

    @seed("patch touching a second source file (C23 also edits model_tests.rs)", "C23", ["single-file"])
    def _(s):
        e = s.by("C23")
        bt = s.base(MODEL)
        extra = make_patch(MODEL, bt.encode(), bt.replace("fn bit(i: u8) -> u8 {", "fn bit(i: u8) -> u8  {", 1)
                           .encode())
        patch = e["patch"].encode() + extra
        e.update({"patch": patch.decode(), "patch_sha256": sha(patch)})
        s.write_patch("C23", patch)

    @seed("failed reverse restoration (C26's recorded mutant is not the patch's post-image)", "C26",
          ["restoration"])
    def _(s):
        e = s.by("C26")
        r = e["edits"][0]["replacement"].replace("self.core.latch = None;", "self.core.latch = None; ")
        e["edits"][0]["replacement"] = r
        e["replacement"] = r

    @seed("C3b removing only one comparison", "C3b", ["semantic"])
    def _(s):
        s.redefine("C3b", [("        } else if set.top > set.head || set.head > now.head {\n",
                            "        } else if set.top > set.head {\n")])

    @seed("C6 changing Core::known globally", "C6", ["semantic", "confine-item"])
    def _(s):
        s.redefine("C6", [("        if let Some(cause) = self.refusals.get(key) {\n"
                           "            return Some(Known::Cause(*cause));\n        }\n"
                           "        if let Some((_, Occupant::Explanation { original, cause }, stage)) = &self.x {\n",
                           "        if let Some((_, Occupant::Explanation { original, cause }, stage)) = &self.x {\n")])

    @seed("C7 changing Publish::settle", "C7", ["semantic", "confine-item"])
    def _(s):
        s.redefine("C7", s.edits("C7") + [("                self.core.refusals.remove(&original);\n",
                                           "                self.core.refusals.clear();\n")])

    @seed("C8 retaining the separate insertion loop", "C8", ["semantic"])
    def _(s):
        (a, r), = s.edits("C8")
        step5 = a[a.index("        // Step 5.\n"):]
        s.redefine("C8", [(a, r + "        let pending = [false; BATCH_MAX];\n" + step5)])

    @seed("C11 changing a non-fact insert path (gating's constraint arm too)", "C11", ["semantic", "confine-item"])
    def _(s):
        s.redefine("C11", s.edits("C11") + [
            ("            Insert::Constraint => {\n                self.core.latch(LatchCause::Constraint);\n"
             "                Reason::Constraint\n            }\n",
             "            Insert::Constraint => {\n                self.core.latch(LatchCause::Constraint);\n"
             "                self.failed();\n                self.rolled(io.rollback());\n"
             "                return refused(Persistence::NotWritten, Reason::Constraint);\n            }\n")])

    @seed("C15 replacing occupancy equality with is_some", "C15", ["semantic"])
    def _(s):
        s.redefine("C15", [("            && self.occupancy(cell) == Some(token.occupancy)\n",
                            "            && self.occupancy(cell).is_some()\n")])

    @seed("C16 lowering live", "C16", ["semantic"])
    def _(s):
        (a, r), = s.edits("C16")
        s.redefine("C16", [(a, r.replace("count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false)",
                                         "count_step(count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false), "
                                         "LIVE_SHIFT, false)"))])

    @seed("C16 removing HANDOVER_FAULT", "C16", ["semantic"])
    def _(s):
        (a, r), = s.edits("C16")
        s.redefine("C16", [(a, r.replace("word | HANDOVER_FAULT", "word"))])

    @seed("C17 changing self.keep", "C17", ["semantic"])
    def _(s):
        s.redefine("C17", s.edits("C17") + [("            self.keep = self.issued;\n",
                                             "            self.keep = false;\n")])

    @seed("C18a changing all storage-failure paths", "C18a", ["semantic"])
    def _(s):
        s.redefine("C18a", s.edits("C18a") + [
            ("        self.hold = Hold::Gating;\n        if io.open() != Read::Got(()) {\n            self.failed();\n",
             "        self.hold = Hold::Gating;\n        if io.open() != Read::Got(()) {\n            self.keep = true;\n"
             "            self.failed();\n"),
            ("                self.read_failed(failed == Read::Corrupt);\n                self.rolled(io.rollback());\n"
             "                return refused(Persistence::NotWritten, Reason::Storage);\n",
             "                self.read_failed(failed == Read::Corrupt);\n                self.keep = true;\n"
             "                self.rolled(io.rollback());\n"
             "                return refused(Persistence::NotWritten, Reason::Storage);\n")])

    @seed("C18b removing X explanation", "C18b", ["semantic"])
    def _(s):
        (a, r), = s.edits("C18b")
        s.redefine("C18b", [(a, r.replace("            if self.x.is_none() {\n"
                                          "                self.explanation(member.key, cause);\n            }\n", ""))])

    @seed("C20 changing the inserted-and-Durable X path", "C20", ["semantic"])
    def _(s):
        s.redefine("C20", s.edits("C20") + [
            ("            Commit::Durable => {\n                self.settle(cell, seq);\n",
             "            Commit::Durable => {\n                if cell == Cell::C {\n"
             "                    self.settle(cell, seq);\n                }\n")])

    @seed("A1 changing settlement", "A1", ["semantic", "confine-item"])
    def _(s):
        s.redefine("A1", s.edits("A1") + [
            ("                SeHeld::Fact(Held::Counted(counted)) => record_counted(w, &[f], counted),\n",
             "                SeHeld::Fact(Held::Counted(_)) => {}\n")])

    @seed("A2 using the second view's low bits", "A2", ["semantic"])
    def _(s):
        (a, r), = s.edits("A2")
        s.redefine("A2", [(a, r.replace("word: kept | with_field(", "word: (second.word & FLAGS) | with_field("))])

    @seed("A2 decrementing adm in step 1", "A2", ["semantic"])
    def _(s):
        (a, r), = s.edits("A2")
        s.redefine("A2", [(a, r.replace("        w.adm_kept = Some(first.word & FLAGS);\n",
                                        "        w.adm_kept = Some(first.word & FLAGS);\n        w.adm -= 1;\n"))])

    @seed("A3 raising H before worker_op", "A3", ["semantic"])
    def _(s):
        (a1, r1), (a2, r2) = s.edits("A3")
        s.redefine("A3", [(a1, r1), (a2, a2.replace(
            "            worker_op(config, w, &action, o, obs);\n",
            "            if let Action::EmptyX(_, seq) = &action {\n                w.table.h = w.table.h.max(*seq);\n"
            "            }\n            worker_op(config, w, &action, o, obs);\n"))])

    @seed("A5 removing the X-empty check", "A5", ["semantic"])
    def _(s):
        (a, r), = s.edits("A5")
        s.redefine("A5", [(a, "        Class::Conflict => Some(Claim::Explain),\n")])

    @seed("A6 omitting its flag from encoding (and decoding)", "A6", ["semantic"])
    def _(s):
        s.redefine("A6", [x for x in s.edits("A6") if "e.b(" not in x[1] and "d.b()" not in x[1]])

    @seed("A6 replacing P12/P15's numeric checks", "A6", ["semantic", "confine-item"])
    def _(s):
        s.redefine("A6", s.edits("A6") + [("            if seq_at(at) > durable {\n",
                                           "            if w.table.recovery_required {\n")])

    @seed("C22 marked restored from the Stage-1 no -j 2 build", "C22", ["c22"])
    def _(s):
        e = s.by("C22")
        h = e["restored_receipt"]["historical_stage1_build_not_the_e4_receipt"]
        e["restored_receipt"] = {"type": "e4", "restored_receipt_status": "SATISFIED", "directory": h["directory"],
                                 "commit": h["commit"], "note": "the accepted Stage-1 healthy build"}

    @seed("catalog as a whole called frozen", "catalog", ["wording"])
    def _(s):
        s.cat["catalog_status"] = "FROZEN — ARCHITECT ACCEPTED"

    return S


def run(checker, root, catdir):
    r = subprocess.run([sys.executable, os.path.join(TOOLS, checker), root, catdir], capture_output=True, text=True)
    fails = []
    for l in r.stdout.splitlines():
        m = re.match(r"FAIL \[([\w-]+)\] (\S+): ", l) or re.match(r"FAIL (\S+) <([\w-]+)> ", l)
        if m:
            cls, cid = (m.group(1), m.group(2)) if l.startswith("FAIL [") else (m.group(2), m.group(1))
            fails.append((cid, cls))
    verd = next((json.loads(l[9:]) for l in r.stdout.splitlines() if l.startswith("VERDICTS ")), None)
    return r.returncode, fails, verd, r.stdout


def main(root):
    real = os.path.join(root, "out", "catalog")
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01 — POSITIVE CONTROL, CHECKER AGREEMENT, NEGATIVE CONTROLS")
    bad = 0
    ra, fa, va, _ = run("checker_a.py", root, real)
    rb, fb, vb, _ = run("checker_b.py", root, real)
    ok = ra == 0 and rb == 0 and not fa and not fb and va == vb and va is not None and \
        all(v == "PASS" for v in va.values())
    print(f"{'OK  ' if ok else 'FAIL'} positive control: checker A rc {ra}, checker B rc {rb}; per-id verdicts "
          f"{'identical' if va == vb else 'DIFFER'} ({len(va or {})} ids, all PASS: "
          f"{all(v == 'PASS' for v in (va or {}).values())})")
    bad += not ok
    print()
    for name, cid, classes, fn in seeds():
        with tempfile.TemporaryDirectory(dir=os.path.join(root, "dev-local")) as d:
            cd = os.path.join(d, "catalog")
            shutil.copytree(real, cd)
            s = Seed(root, cd)
            fn(s)
            s.save()
            ra, fa, va, _ = run("checker_a.py", root, cd)
            rb, fb, vb, _ = run("checker_b.py", root, cd)
        hit_a = sorted({c for i, c in fa if i == cid and c in classes})
        hit_b = sorted({c for i, c in fb if i == cid and c in classes})
        ok = ra == 1 and rb == 1 and bool(hit_a) and bool(hit_b)
        agree = (va or {}).get(cid) == (vb or {}).get(cid) == "FAIL"
        crashed = [k for k, v in (("A", va), ("B", vb)) if v is None]
        print(f"{'OK  ' if ok and agree else 'FAIL'} seed: {name}")
        if crashed:
            print(f"       checker(s) {' and '.join(crashed)} ended without a verdict map (crash)")
        print(f"       expected: {cid} in {sorted(classes)}; A rc {ra} detects {hit_a or 'NOTHING'} "
              f"(failing checks {len(fa)}); B rc {rb} detects {hit_b or 'NOTHING'} (failing checks {len(fb)}); "
              f"both verdicts for {cid} FAIL: {agree}")
        bad += not (ok and agree)
    print()
    n = len(seeds())
    print(f"seeds {n}; undetected or disagreeing {bad}")
    print("NEGATIVE CONTROLS: " + ("PASS — every seeded defect detected by both checkers; positive control agrees"
                                   if not bad else "FAIL"))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
