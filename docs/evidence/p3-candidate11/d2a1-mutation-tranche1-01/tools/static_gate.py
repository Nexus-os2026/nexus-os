#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01, mission section 8: the static catalog gate (read-only; no build, no test).

    static_gate.py ROOT

Re-checks ROOT/out/catalog/catalog.json against git objects of the frozen commits, independently of the deriver:
anchors are recounted in C's blob read from git; every READY patch is applied to a disposable byte copy with GNU
patch and with `git apply`, compared, reversed and compared with C's blob; the changed bytes are confined to the
anchor's span; the named tests are found in C's source and in the pinned executable's accepted listings; every
restored-evidence link is re-read. Each check prints one line. The gate passes only if every check holds AND no
definition is BLOCKED.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
PARENT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UNIT = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
PROTOCOL_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
INPUT_SHA = {"CONTRACT-02-R1.md": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
             "AD1-R4.md": "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693",
             "AD1-R5.md": "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
             "controls_defs.py.txt": "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f",
             "C1-mutation.patch": "5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252",
             "C25-narrow-mutation.patch": "29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732",
             "C4-test1-removed.patch": "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d"}
TRANCHE1 = ["C22", "C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
LIST_ALL = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/20-list-all/raw-output.txt"
LIST_IGN = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/21-list-ignored/raw-output.txt"
QUOTIENT = re.compile(r"(gc_symmetry|quotient|symmetric|explore_experimental)")
fails, blocked_defs = [], []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, check=True).stdout


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def with_copy(data, fn):
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, PROTOCOL)
        os.makedirs(os.path.dirname(p))
        with open(p, "wb") as fh:
            fh.write(data)
        return fn(d, p)


def gnu_patch(data, patch, reverse=False):
    def run(d, p):
        r = subprocess.run(["patch", "-s", "-p1", "--no-backup-if-mismatch", "-r", "-", "-N"] + (["-R"] if reverse
                           else []), cwd=d, input=patch, capture_output=True)
        return open(p, "rb").read() if r.returncode == 0 else None
    return with_copy(data, run)


def git_apply(data, patch, reverse=False):
    def run(d, p):
        env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", GIT_DIR=os.path.join(d, ".nogit"))
        r = subprocess.run(["git", "apply", "--whitespace=nowarn"] + (["-R"] if reverse else []), cwd=d, input=patch,
                           capture_output=True, env=env)
        return open(p, "rb").read() if r.returncode == 0 else None
    return with_copy(data, run)


def contract_ids(text):
    """An independent reading: every table row of 7.5 whose first cell is a control id, by a line scan."""
    sec = text[text.index("### 7.5 Controls"):text.index("### 7.6")]
    return [l.split("|")[1].strip() for l in sec.splitlines()
            if l.startswith("| ") and re.fullmatch(r"(C\d+[a-c]?|A\d)", l.split("|")[1].strip())]


def main(root):
    print("P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01 — STATIC CATALOG GATE (mission 8; no build, no test)")
    for name, digest in INPUT_SHA.items():
        check(f"input {name} authenticated", hashlib.sha256(open(os.path.join(root, "inputs", name), "rb").read())
              .hexdigest() == digest)
    base = git("cat-file", "blob", PROTOCOL_BLOB)
    check("C's protocol.rs read from git is blob a2df2841", blob_id(base) == PROTOCOL_BLOB and
          git("rev-parse", f"{C}:{PROTOCOL}").decode().strip() == PROTOCOL_BLOB)
    model = git("show", f"{C}:{MODEL}").decode()
    unit = git("show", f"{C}:{UNIT}").decode()
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    entries = cat["controls"]
    ids = [e["id"] for e in entries]
    contract = open(os.path.join(root, "inputs", "CONTRACT-02-R1.md")).read()
    cids = contract_ids(contract)
    print("[ids]")
    check("contract 7.5 rows read independently: 36 ids, each once", len(cids) == 36 and len(set(cids)) == 36, cids)
    check("catalog ids equal the contract's, in order", ids == cids)
    check("every id occurs exactly once in the catalog", len(ids) == len(set(ids)))
    kill = [e for e in entries if e["id"] != "A4"]
    check("35 kill-required controls", len(kill) == 35, len(kill))
    a4 = [e for e in entries if e["id"] == "A4"]
    check("A4 separate: one comparison entry, not a mutation, SATISFIED — ARCHITECT ACCEPTED",
          len(a4) == 1 and a4[0].get("not_a_mutation") is True and a4[0].get("status") ==
          "SATISFIED — ARCHITECT ACCEPTED" and "patch" not in a4[0])
    check("A4 evidence directory is Stage-1's receipts/50-A4 at f023cc3d",
          a4[0]["evidence"]["commit"] == STAGE1 and a4[0]["evidence"]["directory"].endswith("receipts/50-A4") and
          bool(a4[0]["evidence"]["files"]))
    check("no C13 in the contract set (mission 16: 'if present')", "C13" not in cids)
    check("no M-namespace entry", not any(i.startswith("M") for i in ids))
    check("every tranche-1 id is in the catalog", all(t in ids for t in TRANCHE1))
    check("no control is marked killed or executed", all(e["execution_status"] in ("NOT ATTEMPTED",
                                                                                "NOT RERUN (mission 16)")
                                                       for e in entries))
    check("every Architect review status is pending", all(e["architect_review_status"] == "pending" for e in entries))
    print("[definitions]")
    ready = [e for e in kill if e["executor_status"] == "READY"]
    seen = {}
    for e in kill:
        cid = e["id"]
        if e["executor_status"] != "READY":
            blocked_defs.append(cid)
            check(f"{cid}: BLOCKED entry carries no anchor, replacement or patch",
                  e.get("anchor") is None and e.get("replacement") is None and e.get("patch") is None and
                  not os.path.exists(os.path.join(root, "out", "catalog", "patches", f"{cid}.patch")))
            continue
        path = e["mutation_source_file"]
        check(f"{cid}: source file exists at C", subprocess.run(["git", "-C", REPO, "cat-file", "-e", f"{C}:{path}"])
              .returncode == 0 and path == PROTOCOL, path)
        anchor, repl = e["anchor"].encode(), e["replacement"].encode()
        check(f"{cid}: anchor occurs exactly once in C's blob (recounted)", base.count(anchor) == 1 ==
              e["actual_anchor_count_at_C"] == e["expected_anchor_count"], base.count(anchor))
        mutant = base.replace(anchor, repl, 1)
        pfile = open(os.path.join(root, "out", "catalog", e["patch_file"]), "rb").read()
        check(f"{cid}: patch file equals the catalog's patch text and its sha256",
              pfile == e["patch"].encode() and hashlib.sha256(pfile).hexdigest() == e["patch_sha256"])
        g = gnu_patch(base, pfile)
        a = git_apply(base, pfile)
        check(f"{cid}: GNU patch on a disposable copy gives the anchor replacement", g == mutant)
        check(f"{cid}: git apply on a disposable copy gives the same bytes", a == mutant)
        check(f"{cid}: mutant blob as recorded", blob_id(mutant) == e["mutant_blob"], e["mutant_blob"])
        check(f"{cid}: reversal (patch -R) restores C's blob exactly", g is not None and
              blob_id(gnu_patch(g, pfile, reverse=True) or b"") == PROTOCOL_BLOB)
        check(f"{cid}: reversal (git apply -R) restores C's blob exactly", a is not None and
              blob_id(git_apply(a, pfile, reverse=True) or b"") == PROTOCOL_BLOB)
        # Confinement: every byte before the anchor's span and every byte after it is identical in the patched
        # copy (g), and only the span itself differs. (A byte-wise prefix/suffix scan is not used: for a pure
        # deletion it may align the removed block a few bytes away from the anchor; see development D2.)
        start, end = base.index(anchor), base.index(anchor) + len(anchor)
        tail = len(base) - end
        check(f"{cid}: changed bytes confined to the anchor's span",
              g is not None and g[:start] == base[:start] and g[len(g) - tail:] == base[end:] and
              g[start:len(g) - tail] == repl, f"anchor span [{start}, {end}) of {len(base)} bytes; "
              f"replacement {len(repl)} bytes")
        files = re.findall(rb"^diff --git a/(\S+) b/(\S+)$", pfile, re.M)
        check(f"{cid}: the patch touches exactly one file, protocol.rs (expected changed blob count 1)",
              files == [(PROTOCOL.encode(), PROTOCOL.encode())] and e["expected_changed_blob_count"] == 1)
        for key in ("patch_sha256", "mutant_blob"):
            if e[key] in seen:
                check(f"{cid}: {key} distinct from every other control (no silent conflation)", False, seen[e[key]])
            seen[e[key]] = cid
        check(f"{cid}: definition origin recorded", bool(e.get("definition_origin")))
    pairs = {(e["anchor"], e["replacement"]) for e in ready}
    check("no two READY controls share an anchor and replacement", len(pairs) == len(ready))
    check("every READY patch sha256 and mutant blob distinct", len({e["patch_sha256"] for e in ready}) == len(ready) ==
          len({e["mutant_blob"] for e in ready}))
    print("[adopted and special definitions]")
    by = {e["id"]: e for e in entries}
    check("C9 is C9-b: `self.failed();` kept, only Persistence::Uncertain replaced",
          by["C9"]["anchor"].count("self.failed();") == 1 and by["C9"]["replacement"].count("self.failed();") == 1 and
          "Persistence::NotWritten" in by["C9"]["replacement"] and by["C9"]["patch_lines_removed_added"] == [1, 1])
    m25 = base.replace(by["C25"]["anchor"].encode(), by["C25"]["replacement"].encode(), 1)
    arm = (b"                Commit::Unknown => {\n                    self.failed();\n"
           b"                    self.rolled(io.rollback());\n                    self.keep = true;\n"
           b"                    return GatingOutcome::Committed(seq);\n                }")
    check("C25 is the narrow definition: in gating's Commit::Unknown arm `self.failed();`, the rollback and "
          "`self.keep = true;` are preserved and only the return is replaced",
          m25.count(arm) == 1 and m25.index(arm) > m25.index(b"    fn gating(") and
          by["C25"]["patch_lines_removed_added"] == [1, 1])
    check("C25's patch is the documentary narrow patch (29a2c1e5)", by["C25"]["patch_sha256"] ==
          INPUT_SHA["C25-narrow-mutation.patch"])
    check("C4 is the complete Test-1 removal: lines 1627-1642 removed, none added; mutant blob f3793c2f",
          by["C4"]["patch_lines_removed_added"] == [16, 0] and by["C4"]["mutant_blob"] ==
          "f3793c2ff61f8adbf43fe17d6b5237b49bc507b0" and by["C4"]["patch_sha256"] ==
          INPUT_SHA["C4-test1-removed.patch"])
    check("C4 names P4 AND P8 on F-8", by["C4"]["named_property"] == ["P4", "P8"] and by["C4"]["configuration"] ==
          "F-8" and "and" in by["C4"]["expected_primary_failure"]["printed_property_must_be"])
    check("C1's patch is the documentary C1 patch (5ecd716a) and equals controls_defs' anchor replacement",
          by["C1"]["patch_sha256"] == INPUT_SHA["C1-mutation.patch"] and
          by["C1"].get("documentary_patch_result_equals_anchor_replacement") is True)
    check("C10 and C24 equal AD-1-R4 G.2's verbatim quotation of the sealed file",
          by["C10"].get("ad1r4_g2_verbatim_equals_controls_defs_lines") is True and
          by["C24"].get("ad1r4_g2_verbatim_equals_controls_defs_lines") is True)
    for cid, rule in (("C1", "G.3"), ("C25", "G.3"), ("C4", "G.5")):
        check(f"{cid}: special rule {rule} recorded and witness required",
              by[cid]["witness_required"] is True and any(rule in s for s in by[cid]["special_attribution_rules"]))
    check("C4: G.6 recorded", any("G.6" in s for s in by["C4"]["special_attribution_rules"]))
    check("only C1, C25 and C4 require a witness", sorted(e["id"] for e in kill if e["witness_required"]) ==
          ["C1", "C25", "C4"])
    check("C15: original explorer, E8, 30,000,000, AD-1-R5 F rule recorded",
          by["C15"]["test"].endswith("::full_fr") and by["C15"]["named_property"] == ["E8"] and
          by["C15"]["ceiling_states"] == 30000000 and any("AD-1-R5 F" in s for s in
                                                          by["C15"]["special_attribution_rules"]))
    t_own = [i + 1 for i, l in enumerate(unit.split("\n")) if l == "    <Ticket as NotClone<_>>::check();"]
    check("C22: build-only; E0283 at the T-OWN assertion's line, derived again at C", by["C22"]["kind"] == "build" and
          t_own == [1089] and by["C22"]["expected_primary_failure"]["location"] == f"{UNIT}:1089" and
          by["C22"]["expected_primary_failure"]["diagnostic"] == "error[E0283]", t_own)
    check("C22 produces no mutant executable by design", by["C22"]["mutant_executable"] == "NOT PRODUCED BY DESIGN"
          and by["C22"]["mutant_test_run"] == "NOT PRODUCED BY DESIGN" and by["C22"]["requires_mutant_executable"]
          is False)
    check("every other control requires a mutant executable",
          all(e["requires_mutant_executable"] is True for e in kill if e["id"] != "C22"))
    print("[configurations and named tests]")
    listing = git("show", f"{PARENT}:{LIST_ALL}").decode()
    ignored = git("show", f"{PARENT}:{LIST_IGN}").decode()
    for e in kill:
        test = e["test"]
        name = test.rsplit("::", 1)[1] if test else None
        src = model if e["kind"] == "explore" else unit
        m = re.search(rf"^#\[test\]\n(#\[ignore[^\n]*\n)?fn {re.escape(name)}\(\)", src, re.M) if name else None
        ok = bool(m) and f"{test}: test" in listing
        if e["kind"] == "explore":
            ok = ok and (m.group(1) is not None) == e["ignored_at_C"] and ((f"{test}: test" in ignored) ==
                                                                          e["ignored_at_C"])
        check(f"{e['id']}: named test {name} exists at C and in the pinned executable's listing"
              + (" (ignored status agrees)" if e["kind"] == "explore" else ""), ok)
        check(f"{e['id']}: failing receipt is never a quotient run", not QUOTIENT.search(test or "x") and
              e["receipt_type"][:3] in ("(a)", "(b)", "(d)"), e["receipt_type"][:3])
    print("[restored-evidence links]")
    for e in kill:
        rr = e["restored_receipt"]
        if "directory" not in rr:
            if e["id"] == "C15":
                f2 = rr["f2_receipt"]
                check("C15: restored side = AD-1-R5 F discharge record; its F-2 receipt resolves and is PASS",
                      f2["classification_first_line"] == "CLASSIFICATION: PASS" and f2["test_matches"] and
                      f2["executable_before_after_equal"] and f2["raw_output_sha256"] ==
                      "985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5")
            else:
                print(f"     {e['id']}: restored receipt {rr['type']} to be produced (mission 10): {rr['status']}")
            continue
        if rr["type"] == "e4":
            check(f"{e['id']}: e4 link resolves (Stage-1 healthy build; pin sha256 equals the pinned executable)",
                  rr["files_equal_at_introducing_commit"] and rr["pin_executable_sha256"] == EXE_SHA)
            continue
        check(f"{e['id']}: {rr['type']} link {rr['directory'].rsplit('/', 1)[1]} resolves: PASS, its test, the pinned "
              "executable before and after, raw-output digest as recorded, files equal at the introducing commit",
              rr["classification_first_line"] == "CLASSIFICATION: PASS" and rr["test_matches"] and
              rr["executable_before_after_equal"] and rr["raw_output_sha256"] == rr["descriptor_raw_output_sha256"]
              and rr["files_equal_at_introducing_commit"])
        if e.get("witness"):
            h = e["witness"]["healthy_witness"]
            check(f"{e['id']}: e3 healthy-witness link resolves", h["classification_first_line"] ==
                  "CLASSIFICATION: PASS" and h["test_matches"] and h["executable_before_after_equal"] and
                  h["raw_output_sha256"] == h["descriptor_raw_output_sha256"])
    print("[definition completeness — mission 7]")
    for cid in blocked_defs:
        check(f"{cid}: exact definition available from an authorised source", False,
              "NO AUTHENTICATED EXACT DEFINITION (contract one-line change only)")
    tb = [t for t in TRANCHE1 if t in blocked_defs]
    print(f"     tranche-1 controls without a definition: {' '.join(tb) or 'none'}")
    print()
    print(f"definitions READY {len(ready)}, BLOCKED {len(blocked_defs)}; failing checks {len(fails)}")
    if fails:
        print("STATIC CATALOG GATE: FAIL — CONTROL_CATALOG_BLOCKED (no Rust build, baseline test or mutant is "
              "authorised; mission 8, 19)")
        return 1
    print("STATIC CATALOG GATE: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
