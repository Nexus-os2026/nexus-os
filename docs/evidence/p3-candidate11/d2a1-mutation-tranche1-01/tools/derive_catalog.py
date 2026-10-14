#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01, mission sections 6 and 7: derive the D2A-1 control catalog (read-only).

    derive_catalog.py ROOT

Reads only ROOT/inputs/ (written and hashed by authenticate.py) and git objects of the frozen commits. Writes
ROOT/out/catalog/catalog.json, ROOT/out/catalog/CATALOG.md and ROOT/out/catalog/patches/<id>.patch. Deterministic:
no clock, no host path, sorted keys; two runs give identical bytes.

The control set is parsed from contract 7.5's three tables. Each configuration's test is read from model_tests.rs at
C (test function -> configuration constant -> its `name`), and cross-checked against AD-1-R4 E.3. A control is given
an exact definition ONLY from an authorised text:
  - AD-1-R4 G.2 (C1, C25, C4, C9; C10 and C24 retained verbatim from the sealed file),
  - the documentary patches G.2 cites (C1, C25, C4), and
  - the sealed historical controls_defs.py, where applicable (C2, C10, C12, C14, C19, C21, C22, C24, C27, C28; and C1
    as a cross-check). Its compound C25 is superseded by G.2's narrow C25 and is not used.
Every other control is recorded BLOCKED with the executor's observations of blocked_notes.py, and no patch.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from blocked_notes import NOTES  # noqa: E402

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
PARENT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UNIT = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
FILES = {PROTOCOL: "C-protocol.rs", MODEL: "C-protocol-model_tests.rs", UNIT: "C-protocol-unit_tests.rs"}
BLOBS = {PROTOCOL: "a2df2841b99cad5d452008a6d4150e9369d07e66", MODEL: "28085ce232827301104b5d757af042ec933976f8",
         UNIT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
SHA = {"contract": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
       "ad1r4": "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693",
       "ad1r5": "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
       "controls_defs": "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f",
       "fr_proof": "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b"}
DOC_PATCH = {"C1": ("C1-mutation.patch", "5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252"),
             "C25": ("C25-narrow-mutation.patch", "29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732"),
             "C4": ("C4-test1-removed.patch", "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d")}
TRANCHE1 = ["C22", "C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]  # mission 15 order
K1 = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/"
K2 = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/"
RESTORED = {  # configuration -> (type, introducing commit, receipt directory, note)
    "F-1": ("e1", STAGE1, K1 + "45-full-F-1", "original-explorer PASS under the Owner-adopted 60,000,000 ceiling"),
    "F-2": ("e1", STAGE1, K1 + "41-full-F-2", "original-explorer PASS"),
    "F-3": ("e1", STAGE1, K1 + "43-full-F-3", "composed-quotient PASS; cited only for C7 without A4 (AD-1-R4 F)"),
    "F-4": ("e1", STAGE2, K2 + "62-full-F-4", "original-explorer PASS"),
    "F-5": ("e1", STAGE2, K2 + "65-full-F-5", "original-explorer PASS"),
    "F-6": ("e1", STAGE2, K2 + "66-full-F-6", "original-explorer PASS"),
    "F-7": ("e1", STAGE2, K2 + "67-full-F-7", "original-explorer PASS"),
    "F-8": ("e1", STAGE2, K2 + "64-full-F-8", "original-explorer PASS"),
    "F-PT": ("e1", STAGE1, K1 + "44-full-F-PT", "composed-quotient PASS, admissible for F-PT's restored side (AD-1-R4 F)"),
    "F-B": ("e1", STAGE1, K1 + "42-full-F-B", "composed-quotient PASS, admissible for F-B's restored side (AD-1-R4 F)"),
    "F-B2": ("e1", STAGE2, K2 + "61-full-F-B2", "original-explorer PASS"),
    "F-G": ("e1", STAGE2, K2 + "60-full-F-G", "original-explorer PASS"),
    "F-D": ("e1", STAGE2, K2 + "68-full-F-D", "original-explorer PASS"),
}
WITNESS = {"C1": ("c1_witness_p1_at_a_successful_admission", K2 + "50-witness-C1"),
           "C25": ("c25_witness_p7_on_the_resulting_world", K2 + "51-witness-C25"),
           "C4": ("c4_witness_p4_and_p8_on_f8", K2 + "52-witness-C4")}
MODPATH = "ledger::protocol::model_tests::"
UNITPATH = "ledger::protocol::unit_tests::"


def sha(b):
    return hashlib.sha256(b).hexdigest()


def git(*args, check=True):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True)
    if check and r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {r.stderr.decode()}")
    return r.stdout


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def lines_of(text, needle):
    """1-based line numbers at which `needle` starts, for every occurrence."""
    out, start = [], 0
    while True:
        i = text.find(needle, start)
        if i < 0:
            return out
        out.append(text.count("\n", 0, i) + 1)
        start = i + 1


def make_patch(path, base, mutant):
    """A git-format unified diff of one file, made with git diff --no-index in a scratch directory, with every
    configuration source neutralised so that the bytes depend only on the two inputs."""
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", LC_ALL="C")
    with tempfile.TemporaryDirectory() as d:
        for side, data in (("a", base), ("b", mutant)):
            p = os.path.join(d, side, path)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as fh:
                fh.write(data)
            os.chmod(p, 0o644)
        r = subprocess.run(["git", "diff", "--no-index", "--no-color", "--no-ext-diff", "--full-index", "-U3",
                            "--diff-algorithm=myers", "--src-prefix=", "--dst-prefix=", "a/" + path, "b/" + path],
                           cwd=d, capture_output=True, env=env)
        if r.returncode != 1:
            raise SystemExit(f"git diff --no-index: rc {r.returncode} {r.stderr.decode()}")
        return r.stdout


def apply_patch(path, base, patch):
    """GNU patch on a disposable copy; returns the patched bytes."""
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, path)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as fh:
            fh.write(base)
        r = subprocess.run(["patch", "-s", "-p1", "--no-backup-if-mismatch", "-r", "-", "-N"], cwd=d, input=patch,
                           capture_output=True)
        if r.returncode != 0:
            return None
        return open(p, "rb").read()


def parse_contract(text):
    lines = text.split("\n")
    start = next(i for i, l in enumerate(lines) if l.startswith("### 7.5 Controls"))
    end = next(i for i, l in enumerate(lines) if l.startswith("### 7.6"))
    rows, table = [], None
    for i in range(start, end):
        l = lines[i]
        if l.startswith("**Changes to the core.**"):
            table = "core"
        elif l.startswith("**Changes to the core, killed by a unit test.**"):
            table = "unit"
        elif l.startswith("**Changes to an actor.**"):
            table = "actor"
        m = re.match(r"^\| (C\d+[a-c]?|A\d) \| (.*) \|$", l)
        if m:
            cells = [c.strip() for c in m.group(2).split(" | ")]
            rows.append({"id": m.group(1), "table": table, "line": i + 1, "cells": cells, "text": l})
    return rows, (start + 1, end)


def parse_configs(model):
    consts = dict(re.findall(r'^const (\w+): Config = (?:default_run\()?Config \{\n    name: "([^"]+)"', model, re.M))
    tests = {}
    pat = re.compile(r'^#\[test\]\n(#\[ignore = "[^"]*"\]\n)?fn (\w+)\(\) \{\n    (full|default)_exploration\(&'
                     r'(?:Config \{\n        cap: ([0-9_]+),\n        \.\.(\w+)\n    \}|(\w+))\);\n\}', re.M)
    for m in pat.finditer(model):
        const = m.group(5) or m.group(6)
        name = consts[const]
        tests[name] = {"test": m.group(2), "ignored": m.group(1) is not None, "kind": m.group(3),
                       "const": const, "line": model.count("\n", 0, m.start(2)) + 1,
                       "cap_override": int(m.group(4).replace("_", "")) if m.group(4) else None}
    caps = dict(re.findall(r"^const (DEFAULT_CAP|FULL_CAP): u64 = ([0-9_]+);", model, re.M))
    for name, t in tests.items():
        t["ceiling"] = t["cap_override"] or int(caps["DEFAULT_CAP" if t["kind"] == "default" else "FULL_CAP"]
                                                .replace("_", ""))
    return tests


def parse_e3(ad1r4):
    out = {}
    for m in re.finditer(r"^\| (\d+) \| `(\w+)` \((\d+)\) \| `explore`, (F-\w+)[^|]*\| ([^|]*) \| ([^|]*) \|",
                         ad1r4, re.M):
        out[m.group(2)] = {"row": int(m.group(1)), "config": m.group(4), "mutant_runs": m.group(6).strip(),
                           "line": ad1r4.count("\n", 0, m.start()) + 1}
    return out


def g2_c9(ad1r4):
    """C9's healthy arm and resulting arm, verbatim from AD-1-R4 G.2."""
    sec = ad1r4[ad1r4.index("- **C9, \"`Rollback::Unknown` is treated as `Undone`\"**"):]
    sec = sec[:sec.index("- **C10,")]
    healthy = re.search(r"<!-- VERBATIM PROTOCOL 1292,1293,1294,1295 -->\n```text\n(.*?)\n```", sec, re.S).group(1)
    mutant = re.search(r"resulting arm:\n\n```rust\n(.*?)\n```", sec, re.S).group(1)
    return healthy, mutant, ad1r4.count("\n", 0, ad1r4.index("- **C9, ")) + 1


def g2_verbatim_defs(ad1r4, first, last):
    marker = "<!-- VERBATIM DEFS " + ",".join(str(n) for n in range(first, last + 1)) + " -->\n```text\n"
    i = ad1r4.index(marker) + len(marker)
    return ad1r4[i:ad1r4.index("\n```", i)]


def load_defs(data):
    ns = {}
    exec(compile(data.decode(), "controls_defs.py", "exec"), ns)  # authenticated sealed data file (sha256 checked)
    return ns["CONTROLS"]


def receipt_link(commit, rdir, want_test, kind):
    """Facts of a linked receipt, read from the evidence parent's tree (and equal to its introducing commit's)."""
    files = {}
    for line in git("ls-tree", PARENT, rdir + "/").decode().splitlines():
        meta, path = line.split("\t")
        files[os.path.basename(path)] = meta.split()[2]
    intro = {}
    for line in git("ls-tree", commit, rdir + "/").decode().splitlines():
        meta, path = line.split("\t")
        intro[os.path.basename(path)] = meta.split()[2]
    link = {"commit": commit, "directory": rdir, "files_equal_at_introducing_commit": files == intro and bool(files)}
    if kind == "build":
        rec = git("cat-file", "blob", files["RECEIPT.txt"]).decode()
        pin = json.loads(git("cat-file", "blob", files["PIN.json"]))
        link.update({"receipt_blob": files["RECEIPT.txt"], "pin_blob": files["PIN.json"],
                     "pin_executable_sha256": pin.get("sha256") or pin.get("executable_sha256"),
                     "pin_keys": sorted(pin)[:20], "receipt_first_line": rec.splitlines()[0]})
        return link
    cls = git("cat-file", "blob", files["CLASSIFICATION.txt"]).decode().splitlines()
    desc = json.loads(git("cat-file", "blob", files["descriptor.json"]))
    raw = git("cat-file", "blob", files["raw-output.txt"])
    argv = desc.get("argv", [])
    exe = desc.get("executable", {})
    # Two descriptor schemas: Stage-2 records `test` and `before`/`after` objects; Stage-1 records the test only in
    # argv[1] and the digests as `sha256_before`/`sha256_after`. Both are read; neither is preferred.
    test = desc.get("test", argv[1] if len(argv) > 1 else None)
    before = exe.get("before", {}).get("sha256", exe.get("sha256_before"))
    after = exe.get("after", {}).get("sha256", exe.get("sha256_after"))
    link.update({"classification_blob": files["CLASSIFICATION.txt"], "classification_first_line": cls[0],
                 "outer_class": cls[1] if len(cls) > 1 else None, "raw_output_blob": files["raw-output.txt"],
                 "raw_output_sha256": sha(raw), "descriptor_raw_output_sha256": desc.get("raw_output_sha256"),
                 "descriptor_test": test, "descriptor_argv_executable": argv[0] if argv else None,
                 "descriptor_argv_flags": argv[2:],
                 "executable_pinned_sha256": exe.get("pinned_sha256"),
                 "executable_before": before, "executable_after": after,
                 "executable_before_after_equal": before == after == EXE_SHA,
                 "test_matches": test == want_test})
    return link


def main(root):
    inp = lambda n: open(os.path.join(root, "inputs", n), "rb").read()  # noqa: E731
    contract, ad1r4, ad1r5 = inp("CONTRACT-02-R1.md"), inp("AD1-R4.md"), inp("AD1-R5.md")
    defs_raw = inp("controls_defs.py.txt")
    for key, data in (("contract", contract), ("ad1r4", ad1r4), ("ad1r5", ad1r5), ("controls_defs", defs_raw)):
        if sha(data) != SHA[key]:
            raise SystemExit(f"input {key} is not the authenticated one")
    src = {p: inp(n) for p, n in FILES.items()}
    for p, data in src.items():
        if blob_id(data) != BLOBS[p]:
            raise SystemExit(f"{p} is not C's blob")
    contract_t, ad1r4_t = contract.decode(), ad1r4.decode()
    protocol_t, model_t, unit_t = (src[p].decode() for p in (PROTOCOL, MODEL, UNIT))
    rows, (c_from, c_to) = parse_contract(contract_t)
    tests = parse_configs(model_t)
    e3 = parse_e3(ad1r4_t)
    defs = load_defs(defs_raw)
    defs_lines = defs_raw.decode().split("\n")
    os.makedirs(os.path.join(root, "out", "catalog", "patches"), exist_ok=True)
    controls = []
    for r in rows:
        cid, cells = r["id"], r["cells"]
        e = {"id": cid, "contract_table": r["table"], "contract_line": r["line"], "contract_row": r["text"],
             "architect_review_status": "pending", "tranche1": cid in TRANCHE1,
             "tranche1_order": TRANCHE1.index(cid) + 1 if cid in TRANCHE1 else None,
             "execution_status": "NOT ATTEMPTED"}
        if cid == "A4":
            e.update({"kind": "comparison", "description": cells[0], "named_property": cells[1],
                      "configuration": cells[2], "not_a_mutation": True,
                      "receipt_type": "(f) the A4 comparison (AD-1-R4 F.1)",
                      "evidence": {"commit": STAGE1, "directory": K1 + "50-A4",
                                   "files": {os.path.basename(l.split("\t")[1]): l.split()[2] for l in
                                             git("ls-tree", PARENT, K1 + "50-A4/").decode().splitlines()}},
                      "status": "SATISFIED — ARCHITECT ACCEPTED",
                      "citations": [f"contract line {r['line']}", "AD-1-R4 F (A4)", "AD-1-R4 F.1 (f)",
                                    "AD-1-R4 E.3 row 9"]})
            e["execution_status"] = "NOT RERUN (mission 16)"
            controls.append(e)
            continue
        if r["table"] == "unit":
            desc, killed_by = cells
            token = re.match(r"(T-[A-Z]+|W\d+)", killed_by).group(1)
            prefix = token.lower().replace("-", "_") + "_"
            fns = re.findall(rf"^fn ({re.escape(prefix)}\w*)\(\)", unit_t, re.M)
            e.update({"description": desc, "named_test_id": token, "killed_by_text": killed_by,
                      "named_test_candidates_at_C": fns})
            test = fns[0] if len(fns) == 1 else None
            if cid == "C22":
                line = lines_of(unit_t, "    <Ticket as NotClone<_>>::check();")
                e.update({"kind": "build", "configuration": "none (build)", "test": UNITPATH + test if test else None,
                          "receipt_type": "(d) intentional mutant compile failure; restored (e4) healthy build",
                          "expected_primary_failure": {
                              "mechanism": "intentional nonzero compiler exit",
                              "diagnostic": "error[E0283]",
                              "location": f"{UNIT}:{line[0]}" if len(line) == 1 else None,
                              "assertion_text": "<Ticket as NotClone<_>>::check();",
                              "assertion_occurrences_at_C": len(line),
                              "rejected": ["internal compiler error", "signal, OOM kill or timeout",
                                           "any other error diagnostic", "E0283 at another line"]},
                          "mutant_executable": "NOT PRODUCED BY DESIGN",
                          "mutant_test_run": "NOT PRODUCED BY DESIGN",
                          "requires_mutant_executable": False})
            else:
                e.update({"kind": "unit", "configuration": "named unit test (not exploration)",
                          "test": UNITPATH + test if test else None, "ignored_at_C": False,
                          "receipt_type": "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
                          "expected_primary_failure": {
                              "mechanism": "the named test fails at one of its own checks (panic message and "
                                           "unit_tests.rs:<line>), 1 failed, exit 101 by a normal exit",
                              "rejected": ["a failure in shared setup or a fixture before the test's own checks",
                                           "a failure elsewhere", "a pass", "a signal, OOM or timeout"]},
                          "time_limit_seconds": 180, "requires_mutant_executable": True})
            e["named_property"] = token
        else:
            desc, fails, on = cells
            props = re.findall(r"\b(P\d+|E\d+|L1|D1)\b", fails)
            t = tests.get(on)
            e.update({"kind": "explore", "description": desc, "named_property_text": fails,
                      "named_property": props, "configuration": on,
                      "test": MODPATH + t["test"] if t else None, "test_line_at_C": t["line"] if t else None,
                      "ignored_at_C": t["ignored"] if t else None, "explorer": "original (`explore`)",
                      "ceiling_states": t["ceiling"] if t else None,
                      "time_limit_seconds": 2700 if t and t["kind"] == "full" else 180,
                      "receipt_type": "(a) original-explorer mutant FAIL; restored (e1)",
                      "requires_mutant_executable": True,
                      "expected_primary_failure": {
                          "mechanism": "the original explorer's own verdict assertion in "
                                       f"{'full' if t and t['kind'] == 'full' else 'default'}_exploration "
                                       "(`failed: {\"<property>\"}`), exit 101 by a normal exit",
                          "raw_output": ["outcome: FAIL", "FAIL: <property>: <detail>",
                                         "states <n> (expanded <m>)", "shortest trace (<n> steps):"],
                          "printed_property_must_be": ("either of " + " / ".join(props)) if "or" in fails.split()
                          else (" and ".join(props) if len(props) > 1 else props[0]),
                          "discovered_at_most": t["ceiling"] if t else None,
                          "beyond_ceiling": "INCOMPLETE — BUDGET EXCEEDED; kills nothing",
                          "incomplete": "kills nothing"}})
            if t:
                row = e3.get(t["test"])
                e["ad1r4_e3"] = row
                if row is None and on != "X-1":
                    e["ad1r4_e3_problem"] = "configuration test not in AD-1-R4 E.3"
        # restored-receipt source
        if e["kind"] == "explore":
            if e["configuration"] == "X-1":
                e["restored_receipt"] = {"type": "e1", "status": "REQUIRED — standalone X-1 PASS on the pinned "
                                         "baseline executable (mission 10A); NOT PRODUCED: the catalog is blocked",
                                         "supporting_only": K1 + "30-suite (X-1 inside the accepted suite)"}
            elif e["configuration"] == "F-R":
                e["restored_receipt"] = {"type": "AD-1-R5 F refinement discharge record (not a run, never PASS)",
                                         "status": "LINKABLE only after verified restoration (AD-1-R5 F)",
                                         "proof_sha256": SHA["fr_proof"], "ad1r5_sha256": SHA["ad1r5"],
                                         "f2_receipt": receipt_link(STAGE1, K1 + "41-full-F-2",
                                                                    MODPATH + "full_f2", "run")}
            else:
                typ, commit, rdir, note = RESTORED[e["configuration"]]
                link = receipt_link(commit, rdir, e["test"] if e["configuration"] not in ("F-PT", "F-B")
                                    else MODPATH + {"F-PT": "gc_symmetry_fpt", "F-B": "gc_symmetry_fb"}
                                    [e["configuration"]], "run")
                e["restored_receipt"] = {"type": typ, "note": note, **link}
        elif e["kind"] == "unit":
            e["restored_receipt"] = {"type": "e2", "status": "REQUIRED — standalone PASS of the named test on the "
                                     "pinned baseline executable (mission 10B); NOT PRODUCED: the catalog is blocked",
                                     "supporting_only": K1 + "30-suite (the test inside the accepted suite)"}
        else:
            e["restored_receipt"] = {"type": "e4", "note": "the accepted Stage-1 healthy build (mission 10)",
                                     **receipt_link(STAGE1, K1 + "10-build", None, "build")}
        # witness and special rules
        special = []
        if cid in ("C1", "C25"):
            special.append("AD-1-R4 G.3 (kill criterion for C1 and C25 only): type (a) primary on full_f1 is "
                           "recorded as printed; attribution from the type (c) mutant witness; type (e3) healthy "
                           "witness required")
        if cid == "C4":
            special.append("AD-1-R4 G.5 and G.6 (C4 only): P4 AND P8; type (a) primary on full_f8 recorded as "
                           "printed; attribution from the type (c) mutant witness; type (e3) healthy witness; G.6 "
                           "offered-versus-consumed Oracle rule")
        if cid in ("C3a", "C3b", "A6"):
            special.append("AD-1-R4 F.1(a): either property the row names")
        if cid == "C15":
            special.append("AD-1-R5 F: original explorer, E8, 30,000,000; FAIL beyond the cap is INCOMPLETE — "
                           "BUDGET EXCEEDED; restored side may cite the F-R refinement discharge record only after "
                           "verified restoration")
        if cid == "C22":
            special.append("contract 7.5 line 731 and AD-1-R4 F.1(d): killed by a build failure, E0283 at the "
                           "T-OWN assertion's line")
        if cid in ("C6", "C7"):
            special.append("contract 7.5: 'with A4' — run on F-PT (A4 on), original explorer (AD-1-R4 F)")
        if cid == "C7":
            special.append("AD-1-R4 F (line 748): C7 without A4 on F-3 is an added run, not required to kill")
        if cid in ("C20", "A5"):
            special.append("L1 is evaluated only after a complete search (AD-1-R4 Q)")
        e["special_attribution_rules"] = special
        if cid in WITNESS:
            name, rdir = WITNESS[cid]
            e["witness_required"] = True
            e["witness"] = {"test": MODPATH + name, "mutant_witness": "type (c), on the same pinned mutant "
                            "executable as the type (a) primary (AD-1-R4 F.2)",
                            "healthy_witness": {"type": "e3", **receipt_link(STAGE2, rdir, MODPATH + name, "run")}}
        else:
            e["witness_required"] = False
        # definition
        d = definition(cid, e, protocol_t, src[PROTOCOL], defs, defs_lines, ad1r4_t, root)
        e.update(d)
        cites = [f"contract 7.5 line {r['line']}", "AD-1-R4 F", "AD-1-R4 F.1", "AD-1-R4 I"]
        if e.get("ad1r4_e3"):
            cites.append(f"AD-1-R4 E.3 row {e['ad1r4_e3']['row']}")
        if cid in ("C1", "C25", "C4", "C9", "C10", "C24"):
            cites.append("AD-1-R4 G.2")
        if cid in ("C1", "C25"):
            cites.append("AD-1-R4 G.3")
        if cid == "C4":
            cites += ["AD-1-R4 G.5", "AD-1-R4 G.6"]
        if cid == "C15":
            cites += ["AD-1-R5 F", "AD-1-R5 S5-7"]
        e["citations"] = cites
        controls.append(e)
    kill = [c for c in controls if c["id"] != "A4"]
    ready = [c["id"] for c in kill if c["executor_status"] == "READY"]
    blocked = [c["id"] for c in kill if c["executor_status"] == "BLOCKED"]
    catalog = {
        "name": "D2A-1 exact-final control catalog (contract 7.5)",
        "version": "d2a1-controls-v1",
        "mission": "P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01",
        "catalog_status": "CONTROL_CATALOG_BLOCKED" if blocked else "READY_FOR_FREEZE",
        "freeze_status": "NOT FROZEN — the static catalog gate cannot pass while any definition is blocked "
                         "(mission 7, 8)" if blocked else "eligible for freeze after the static gate",
        "namespace": "contract 7.5 C/A controls; separate from the historical M01-M183 namespace (no M entry is "
                     "created for any C/A control)",
        "baseline": {"commit": C, "tree": C_TREE, "blobs": BLOBS},
        "sources": {"contract": {"sha256": SHA["contract"], "section_7_5_lines": [c_from, c_to]},
                    "ad1r4": {"sha256": SHA["ad1r4"], "status": "Owner-adopted, governing"},
                    "ad1r5": {"sha256": SHA["ad1r5"], "status": "Owner-adopted in full (mission 1); supplements "
                              "AD-1-R4 for F-R and C15's restored side only"},
                    "controls_defs": {"sha256": SHA["controls_defs"], "status": "historical sealed definitions, "
                                      "applicable where not superseded; holds a partial set only"},
                    "documentary_patches": {k: {"file": v[0], "sha256": v[1]} for k, v in DOC_PATCH.items()}},
        "counts": {"kill_required": len(kill), "comparison": len(controls) - len(kill),
                   "ready": len(ready), "blocked": len(blocked)},
        "ready": ready, "blocked": blocked,
        "tranche1_execution_order": TRANCHE1,
        "tranche1_blocked": [c for c in TRANCHE1 if c in blocked],
        "controls": controls,
    }
    out = os.path.join(root, "out", "catalog")
    with open(os.path.join(out, "catalog.json"), "w") as fh:
        json.dump(catalog, fh, indent=1, sort_keys=True, ensure_ascii=False)
        fh.write("\n")
    with open(os.path.join(out, "CATALOG.md"), "w") as fh:
        fh.write(render(catalog))
    print(f"catalog: {len(controls)} entries ({len(kill)} kill-required + {len(controls) - len(kill)} comparison); "
          f"READY {len(ready)}: {' '.join(ready)}")
    print(f"BLOCKED {len(blocked)}: {' '.join(blocked)}")
    print(f"tranche 1 blocked: {' '.join(catalog['tranche1_blocked']) or 'none'}")
    print(f"catalog.json sha256 {sha(open(os.path.join(out, 'catalog.json'), 'rb').read())}")
    print(f"catalog status: {catalog['catalog_status']}")
    return 0


def definition(cid, e, protocol_t, protocol_b, defs, defs_lines, ad1r4_t, root):
    """The exact definition from an authorised text, or BLOCKED with the executor's observations."""
    base = protocol_b
    d = {"mutation_source_file": PROTOCOL, "baseline_blob": BLOBS[PROTOCOL], "expected_changed_blob_count": 1}
    anchor = replacement = None
    origin = []
    if cid in ("C2", "C10", "C12", "C14", "C19", "C21", "C22", "C24", "C27", "C28", "C1"):
        meaning, path, edits, kind, target, expected = defs[cid]
        if len(edits) != 1 or path != PROTOCOL:
            return {"executor_status": "BLOCKED", "blocked_reason": "controls_defs entry not one edit of protocol.rs"}
        anchor, replacement = edits[0]
        origin.append(f"controls_defs.py entry {cid!r} (sha256 {SHA['controls_defs']}): {meaning}")
        d["controls_defs_target"] = {"kind": kind, "target": target if isinstance(target, str) else list(target),
                                     "expected": expected}
        if cid in ("C10", "C24"):
            first, last = {"C10": (37, 47), "C24": (48, 66)}[cid]
            g2 = g2_verbatim_defs(ad1r4_t, first, last)
            d["ad1r4_g2_verbatim_equals_controls_defs_lines"] = g2 == "\n".join(defs_lines[first - 1:last])
            origin.insert(0, f"AD-1-R4 G.2: ARCHITECT-RULED, the sealed definition retained unchanged "
                             f"(controls_defs.py lines {first}-{last} quoted verbatim)")
        if cid == "C1":
            origin.insert(0, "AD-1-R4 G.2: C1's encoding; documentary patch C1-mutation.patch (the controls_defs.py "
                             "anchor below is a cross-check)")
    if cid == "C25":
        anchor = "                    return refused(Persistence::Uncertain, Reason::Storage);"
        replacement = "                    return GatingOutcome::Committed(seq);"
        origin.append("AD-1-R4 G.2: the narrow C25 (only `return refused(Persistence::Uncertain, Reason::Storage);` "
                      "replaced; `self.failed();`, the rollback and `self.keep = true;` preserved); documentary patch "
                      "C25-narrow-mutation.patch")
        origin.append("controls_defs.py's compound C25 (which also drops `self.keep = true;`) is SUPERSEDED and not "
                      "used")
    if cid == "C4":
        lines = protocol_t.split("\n")
        anchor = "\n".join(lines[1626:1642]) + "\n"
        replacement = ""
        origin.append("AD-1-R4 G.2 (ARCHITECT-RULED): Test 1's five checks removed, lines 1627-1642, none added; "
                      "documentary patch C4-test1-removed.patch (evidence commit B a464a366)")
        d["g2_removed_lines"] = [1627, 1642]
    if cid == "C9":
        healthy, mutant, line = g2_c9(ad1r4_t)
        anchor, replacement = healthy, mutant
        origin.append(f"AD-1-R4 G.2 (ARCHITECT-RULED: C9-b only), line {line}: in `Publish::rolled`'s arm "
                      "`Rollback::Unknown`, `self.failed();` kept, `Persistence::Uncertain` replaced with "
                      "`Persistence::NotWritten`")
        d["g2_healthy_equals_C_lines_1292_1295"] = healthy == "\n".join(protocol_t.split("\n")[1291:1295])
    if anchor is None:
        n = NOTES[cid]
        sites = []
        for path, text in n["sites"]:
            body = open(os.path.join(root, "inputs", FILES[path]), "rb").read().decode()
            sites.append({"file": path, "text": text, "lines_at_C": lines_of(body, text),
                          "occurrences_at_C": body.count(text)})
        return {"executor_status": "BLOCKED", "mutation_source_file": None,
                "blocked_reason": "NO AUTHENTICATED EXACT DEFINITION: the contract gives only the one-line change; "
                                  "neither AD-1-R4, AD-1-R5 nor the sealed controls_defs.py gives an anchor, a "
                                  "replacement or a patch for it (mission 7: CONTROL_CATALOG_BLOCKED rather than "
                                  "inventing a definition)",
                "anchor": None, "replacement": None, "patch": None,
                "executor_observations": {"status": "EXECUTOR OBSERVATION — NOT A DEFINITION — NOT ARCHITECT "
                                                    "REVIEWED", "candidate_sites": sites, "open_questions": n["open"]}}
    count = protocol_t.count(anchor)
    mutant_t = protocol_t.replace(anchor, replacement, 1)
    mutant_b = mutant_t.encode()
    if cid in DOC_PATCH:
        patch = open(os.path.join(root, "inputs", DOC_PATCH[cid][0]), "rb").read()
        via_patch = apply_patch(PROTOCOL, base, patch)
        d["documentary_patch_applies_to_C"] = via_patch is not None
        d["documentary_patch_result_equals_anchor_replacement"] = via_patch == mutant_b
        d["patch_origin"] = f"documentary patch {DOC_PATCH[cid][0]} (authenticated, unchanged bytes)"
    else:
        patch = make_patch(PROTOCOL, base, mutant_b)
        d["patch_origin"] = "generated by derive_catalog.py from the authorised anchor and replacement " \
                            "(git diff --no-index, neutral configuration)"
    os.makedirs(os.path.join(root, "out", "catalog", "patches"), exist_ok=True)
    with open(os.path.join(root, "out", "catalog", "patches", f"{cid}.patch"), "wb") as fh:
        fh.write(patch)
    mblob = blob_id(mutant_b)
    m = re.search(rb"^index ([0-9a-f]+)\.\.([0-9a-f]+)", patch, re.M)
    d.update({"executor_status": "READY" if count == 1 else "BLOCKED", "definition_origin": origin,
              "anchor": anchor, "replacement": replacement, "expected_anchor_count": 1,
              "actual_anchor_count_at_C": count, "anchor_first_line_at_C": lines_of(protocol_t, anchor)[:1],
              "mutant_blob": mblob, "mutant_sha256": sha(mutant_b),
              "patch": patch.decode(), "patch_file": f"patches/{cid}.patch", "patch_sha256": sha(patch),
              "patch_index_line": [x.decode() for x in m.groups()] if m else None,
              "patch_index_matches": bool(m) and BLOBS[PROTOCOL].startswith(m.group(1).decode()) and
              mblob.startswith(m.group(2).decode()),
              "patch_lines_removed_added": [sum(1 for l in patch.decode().split("\n") if l.startswith("-") and
                                                not l.startswith("---")),
                                            sum(1 for l in patch.decode().split("\n") if l.startswith("+") and
                                                not l.startswith("+++"))]})
    if count != 1:
        d["blocked_reason"] = f"anchor occurs {count} times at C, not exactly once"
    return d


def render(cat):
    out = [f"# {cat['name']} — {cat['version']}", "",
           f"Catalog status: **{cat['catalog_status']}**. {cat['freeze_status']}.", "",
           f"Baseline: Candidate C `{cat['baseline']['commit']}` (tree `{cat['baseline']['tree']}`).", "",
           f"Kill-required: {cat['counts']['kill_required']}; separate comparison: {cat['counts']['comparison']} (A4). "
           f"READY: {cat['counts']['ready']}; BLOCKED: {cat['counts']['blocked']}.", "",
           "| Id | Change (contract 7.5) | Named | On / test | Receipt type | Executor status | Tranche 1 |",
           "|---|---|---|---|---|---|---|"]
    for c in cat["controls"]:
        named = c.get("named_property")
        named = " / ".join(named) if isinstance(named, list) else named
        on = c.get("test") or c.get("configuration")
        out.append(f"| {c['id']} | {c['description']} | {named} | `{on}` | {c['receipt_type'].split(';')[0]} | "
                   f"{c.get('executor_status', c.get('status'))} | "
                   f"{c['tranche1_order'] if c['tranche1'] else '—'} |")
    out += ["", "Every control's Architect review status: pending. Execution status: NOT ATTEMPTED (A4: not rerun).",
            "", "The machine-readable record, with anchors, replacements, patches, counts, links and the executor's "
            "observations for each BLOCKED control, is `catalog.json`.", ""]
    return "\n".join(out)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
