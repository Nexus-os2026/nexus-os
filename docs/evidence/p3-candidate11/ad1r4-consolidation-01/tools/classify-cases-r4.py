#!/usr/bin/env python3
"""AD-1-R4 K.2 classification cases (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01). Derived from AD-1-R3's
classify-cases.py: its rules and its cases 1-7 and x1-x12 are kept; the receipt types of AD-1-R4's F.1 ((c) mutant
witness of C1/C25/C4, (e1)-(e4)), F.2's role identity and G.6's consumption rule are added, with cases 8-14 and
x13-x18. Applies the rules to SYNTHETIC receipt descriptors: nothing here builds, runs, mutates or reads any Nexus
executable or receipt. Each case states its expected raw outcome, outer classification and kill evidence. With the draft
as argument, each numbered case is also matched against the draft's own K.2 table row (its outer classification and
kill-evidence cells must contain the implementation's key phrase). Exit 0 only if every case is as expected.
    classify-cases-r4.py <AD-1-R4>"""
import re, sys

ADMISSIBLE_EXIT = {"pass": 0, "fail": 101}


def classify(r):
    """Returns (raw outcome kept, outer classification, kill evidence)."""
    t = r["type"]
    if t in ("c22-build", "healthy-build"):                # F.1 (d) and (e4); K.2 read for builds
        raw = f"build exit {r['compiler_exit']}; diagnostics {r['diagnostics']}"
        if not r["identity_ok"]:
            return raw, "NOT ADMISSIBLE (identity)", "no"
        if r.get("signal") or r.get("ice") or r.get("oom") or r.get("timeout"):
            return raw, "NOT ADMISSIBLE (compiler crash or resource failure)", "no"
        if t == "healthy-build":
            if r["compiler_exit"] != 0 or not r.get("finished_line"):
                return raw, "NOT ADMISSIBLE (healthy build did not succeed)", "no"
            produced, recorded = set(r["artifacts_produced"]), set(r["artifacts_recorded"])
            if produced - recorded or r.get("marked_not_produced"):
                return raw, "NOT ADMISSIBLE (a produced artifact unrecorded or marked NOT PRODUCED BY DESIGN)", "no"
            return raw, "PASS (type (e4)); no artifact marked NOT PRODUCED BY DESIGN", "—"
        if not r["healthy_build_ok"]:
            return raw, "NOT ADMISSIBLE (no healthy build evidence)", "no"
        if r["compiler_exit"] != 0 and r["diagnostics"] == [("E0283", r["designated_line"])]:
            nproduced = all(r.get(k) is None for k in ("executable_sha256", "test_selection", "test_run"))
            left = set(r.get("artifacts_left", [])) - set(r.get("artifacts_recorded", []))
            if left:
                return raw, "NOT ADMISSIBLE (an artifact the mutant build left is unrecorded)", "no"
            outer = "admissible build-failure receipt (F.1 type (d))"
            return raw, outer + ("; mutant executable and test-run fields NOT PRODUCED BY DESIGN" if nproduced else
                                 "; INVENTED FIELDS PRESENT"), "yes, for C22 only" if nproduced else "no"
        return raw, "NOT ADMISSIBLE (unrelated or misplaced diagnostic)", "no"

    raw = r["printed_outcome"]
    # K.2 point 6 and point 5: the launching command's records and the process end decide first.
    if r.get("signal") or r.get("supervisor_kill") or r.get("timeout") or r.get("oom"):
        return raw, "INCOMPLETE: a run killed from outside (line 855); never PASS", "no"
    if t in ("witness-mutant", "witness-baseline") and r.get("role_executable") != r.get("required_executable"):
        return raw, "NOT ADMISSIBLE (role identity, F.2)", "no"
    if t == "witness-baseline" and r.get("positive_line"):
        return raw, "NOT ADMISSIBLE (role identity, F.2)", "no"
    if t == "witness-mutant" and r.get("healthy_line"):
        return raw, "NOT ADMISSIBLE (role identity, F.2)", "no"
    if not r["identity_ok"]:
        return raw, "NOT ADMISSIBLE (identity mismatch)", "no"
    if not (r["exact"] and r["matches"] == 1 and r["tests_ran"] == 1):
        return raw, "NOT ADMISSIBLE (selection)", "no"
    if r.get("refusal"):                                   # K.1 quotient precondition refusal
        return raw, "INCOMPLETE — VERIFICATION INVALID", "no"
    if not r["completed"]:
        return raw, "INCOMPLETE (no completion evidence); never PASS", "no"
    if r.get("control") == "C4" and t in ("witness-mutant", "witness-baseline") and not r.get("consumption_ok", True):
        return raw, "NOT ADMISSIBLE (G.6)", "no"

    if r["harness"] == "ok":                               # a passed test
        if r["exit"] != ADMISSIBLE_EXIT["pass"]:
            return raw, "NOT ADMISSIBLE (harness and exit disagree)", "no"
        if t in ("explorer-baseline", "explorer-mutant"):
            if raw != "PASS" or not r.get("all_q_lines") or r["count"] > r["ceiling"]:
                return raw, "NOT ADMISSIBLE (PASS text, Q lines or cardinality)", "no"
            return raw, "PASS", "—" if t == "explorer-baseline" else "no"
        if t == "witness-baseline":
            if not r.get("healthy_line"):
                return raw, "NOT ADMISSIBLE (healthy line missing)", "no"
            return raw, "PASS (type (e3))", "a part only: G.3(d) or G.5(C)"
        if t == "unit-baseline":
            return raw, "PASS (type (e2)); explorer fields NOT PRODUCED BY DESIGN", "—"
        return raw, "PASS", "no"

    # harness == "failed": an intended assertion failure must be the named test's own, completed failure
    if r["exit"] != ADMISSIBLE_EXIT["fail"]:
        return raw, "NOT ADMISSIBLE (harness and exit disagree)", "no"
    if r["panic"] not in ("explorer-verdict", "witness-verdict", "unit-check"):
        return raw, "NOT ADMISSIBLE (unrelated failure)", "no"
    if t == "explorer-baseline":
        if raw == "INCOMPLETE":
            return raw, "INCOMPLETE", "no"
        return raw, "FAIL on the unchanged core (lines 940, 942: report and stop)", "no"
    if t == "explorer-mutant":
        if r["panic"] != "explorer-verdict":
            return raw, "NOT ADMISSIBLE (not the explorer's verdict)", "no"
        if raw == "INCOMPLETE":
            return raw, "INCOMPLETE", "no"
        if r["count"] > r["ceiling"]:
            return raw, "INCOMPLETE — BUDGET EXCEEDED", "no" if not r.get("separate_authorisation") else "per that authorisation"
        if r["control"] in ("C1", "C25"):
            return raw, "FAIL, admissible (G.3(a) primary receipt)", "part only: G.3(a)"
        if r["control"] == "C4":
            return raw, "FAIL, admissible (G.5(A) primary receipt; property recorded as printed)", "part only: G.5(A)"
        named = r["named"]
        ok = r["printed_property"] in named if r.get("either") else r["printed_property"] == named
        if ok:
            return raw, "FAIL, admissible (F.1 type (a)); no witness marker is required", "yes"
        return raw, "FAIL, admissible, of another property: not killed (7.5 stop rule)", "no"
    if t == "witness-mutant":
        if r["panic"] != "witness-verdict" or r.get("not_observed") or r.get("fixture"):
            return raw, "NOT ADMISSIBLE (witness rejection, G.3(c) or G.5)", "no"
        if r.get("control") == "C4":
            if not (r.get("p4_line") and r.get("p8_line")):
                return raw, "NOT ADMISSIBLE for G.5(B): both properties are required", "no"
            return raw, "admissible witness receipt: G.5(B) holds", "a part only: with G.5(A), (C), (D) and (E)"
        if not r.get("positive_line"):
            return raw, "NOT ADMISSIBLE (witness rejection, G.3(c) or G.5)", "no"
        return raw, "admissible witness receipt: G.3(b) holds", "a part only: with G.3(a) and G.3(d)"
    if t == "unit-mutant":
        if r["panic"] != "unit-check" or not r.get("location_in_named_test"):
            return raw, "NOT ADMISSIBLE (failure outside the named test's own checks)", "no"
        return raw, "FAIL, admissible (F.1 type (b))", "yes"
    return raw, "NOT ADMISSIBLE (unknown type)", "no"


BASE = dict(identity_ok=True, exact=True, matches=1, tests_ran=1, completed=True, signal=None, supervisor_kill=False,
            timeout=False, oom=False, refusal=False)
MUTX, BASEX = "mutant-exe", "baseline-exe"
CASES = [  # (id, description, receipt, expected (raw, outer, kill)); numbered ids are the draft's K.2 rows
    ("1", "unchanged-core exploration PASS (e1)",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=0, count=12_602_670,
          ceiling=30_000_000, panic=None),
     ("PASS", "PASS", "—")),
    ("2", "PASS text plus external kill",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=None,
          signal="SIGKILL", supervisor_kill=True, count=100, ceiling=30_000_000, panic=None),
     ("PASS", "INCOMPLETE: a run killed from outside (line 855); never PASS", "no")),
    ("3", "C1/C25 mutant witness, positive line",
     dict(BASE, type="witness-mutant", control="C25", printed_outcome="witness FAIL; NAMED PROPERTY P7: OBSERVED on the world after step 1",
          harness="failed", exit=101, panic="witness-verdict", positive_line=True, role_executable=MUTX, required_executable=MUTX),
     ("witness FAIL; NAMED PROPERTY P7: OBSERVED on the world after step 1",
      "admissible witness receipt: G.3(b) holds", "a part only: with G.3(a) and G.3(d)")),
    ("4", "unrelated panic / nonzero exit",
     dict(BASE, type="explorer-mutant", control="C10", named="E4", printed_outcome="(none)", harness="failed", exit=101,
          panic="fixture", count=10, ceiling=60_000_000),
     ("(none)", "NOT ADMISSIBLE (unrelated failure)", "no")),
    ("5", "original-explorer FAIL without a witness marker",
     dict(BASE, type="explorer-mutant", control="C10", named="E4", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=101, panic="explorer-verdict", count=3, ceiling=60_000_000),
     ("FAIL", "FAIL, admissible (F.1 type (a)); no witness marker is required", "yes")),
    ("6", "C22 with no mutant executable",
     dict(type="c22-build", identity_ok=True, compiler_exit=101, diagnostics=[("E0283", 1089)], designated_line=1089,
          healthy_build_ok=True, executable_sha256=None, test_selection=None, test_run=None),
     ("build exit 101; diagnostics [('E0283', 1089)]",
      "admissible build-failure receipt (F.1 type (d)); mutant executable and test-run fields NOT PRODUCED BY DESIGN", "yes, for C22 only")),
    ("7", "raw FAIL discovered after the authorised cardinality is exceeded",
     dict(BASE, type="explorer-mutant", control="C24", named="P8", printed_outcome="FAIL", printed_property="P8",
          harness="failed", exit=101, panic="explorer-verdict", count=60_000_001, ceiling=60_000_000),
     ("FAIL", "INCOMPLETE — BUDGET EXCEEDED", "no")),
    ("8", "C4 mutant witness: P4 and P8 both observed",
     dict(BASE, type="witness-mutant", control="C4", printed_outcome="witness FAIL; P4 and P8 OBSERVED", harness="failed",
          exit=101, panic="witness-verdict", p4_line=True, p8_line=True, role_executable=MUTX, required_executable=MUTX),
     ("witness FAIL; P4 and P8 OBSERVED", "admissible witness receipt: G.5(B) holds", "a part only: with G.5(A), (C), (D) and (E)")),
    ("9", "C4 mutant witness: only P4 observed",
     dict(BASE, type="witness-mutant", control="C4", printed_outcome="witness FAIL; P4 OBSERVED; P8 NOT OBSERVED", harness="failed",
          exit=101, panic="witness-verdict", p4_line=True, p8_line=False, role_executable=MUTX, required_executable=MUTX),
     ("witness FAIL; P4 OBSERVED; P8 NOT OBSERVED", "NOT ADMISSIBLE for G.5(B): both properties are required", "no")),
    ("10", "healthy witness PASS (e3) on the baseline executable",
     dict(BASE, type="witness-baseline", control="C4", printed_outcome="witness PASS; no violation", harness="ok", exit=0,
          healthy_line=True, role_executable=BASEX, required_executable=BASEX),
     ("witness PASS; no violation", "PASS (type (e3))", "a part only: G.3(d) or G.5(C)")),
    ("11", "healthy witness run on a mutant executable",
     dict(BASE, type="witness-baseline", control="C1", printed_outcome="witness PASS; no violation", harness="ok", exit=0,
          healthy_line=True, role_executable=MUTX, required_executable=BASEX),
     ("witness PASS; no violation", "NOT ADMISSIBLE (role identity, F.2)", "no")),
    ("12", "healthy build success (e4), every artifact recorded",
     dict(type="healthy-build", identity_ok=True, compiler_exit=0, diagnostics=[], finished_line=True,
          artifacts_produced=["deps/nexus_governed_control-x"], artifacts_recorded=["deps/nexus_governed_control-x"]),
     ("build exit 0; diagnostics []", "PASS (type (e4)); no artifact marked NOT PRODUCED BY DESIGN", "—")),
    ("13", "restored named unit test PASS (e2)",
     dict(BASE, type="unit-baseline", printed_outcome="test t_bind ... ok", harness="ok", exit=0),
     ("test t_bind ... ok", "PASS (type (e2)); explorer fields NOT PRODUCED BY DESIGN", "—")),
    ("14", "C4 witness step consuming beyond its offered stream",
     dict(BASE, type="witness-mutant", control="C4", printed_outcome="witness FAIL; consumed 7 choices, beyond the 6 offered",
          harness="failed", exit=101, panic="witness-verdict", p4_line=True, p8_line=True, consumption_ok=False,
          role_executable=MUTX, required_executable=MUTX),
     ("witness FAIL; consumed 7 choices, beyond the 6 offered", "NOT ADMISSIBLE (G.6)", "no")),
    # ---- extra negatives (each guards a K.2 point or an F.1/F.2 rule) ----
    ("x1", "PASS text, identity mismatch",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=0, count=1,
          ceiling=30_000_000, identity_ok=False, panic=None),
     ("PASS", "NOT ADMISSIBLE (identity mismatch)", "no")),
    ("x2", "PASS text, no completion evidence",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=0, count=1,
          ceiling=30_000_000, completed=False, panic=None),
     ("PASS", "INCOMPLETE (no completion evidence); never PASS", "no")),
    ("x3", "PASS text, two tests selected",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=0, count=1,
          ceiling=30_000_000, matches=2, tests_ran=2, panic=None),
     ("PASS", "NOT ADMISSIBLE (selection)", "no")),
    ("x4", "quotient precondition refusal",
     dict(BASE, type="explorer-baseline", printed_outcome="(refusal: symmetry inapplicable)", harness="failed", exit=101,
          refusal=True, panic="other", count=5, ceiling=30_000_000),
     ("(refusal: symmetry inapplicable)", "INCOMPLETE — VERIFICATION INVALID", "no")),
    ("x5", "C22 with an internal compiler error",
     dict(type="c22-build", identity_ok=True, compiler_exit=101, diagnostics=[("ICE", 0)], designated_line=1089,
          healthy_build_ok=True, ice=True, executable_sha256=None, test_selection=None, test_run=None),
     ("build exit 101; diagnostics [('ICE', 0)]", "NOT ADMISSIBLE (compiler crash or resource failure)", "no")),
    ("x6", "C22 with E0283 at another line",
     dict(type="c22-build", identity_ok=True, compiler_exit=101, diagnostics=[("E0283", 1090)], designated_line=1089,
          healthy_build_ok=True, executable_sha256=None, test_selection=None, test_run=None),
     ("build exit 101; diagnostics [('E0283', 1090)]", "NOT ADMISSIBLE (unrelated or misplaced diagnostic)", "no")),
    ("x7", "explorer FAIL of a property other than the named one",
     dict(BASE, type="explorer-mutant", control="C24", named="P8", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=101, panic="explorer-verdict", count=50, ceiling=60_000_000),
     ("FAIL", "FAIL, admissible, of another property: not killed (7.5 stop rule)", "no")),
    ("x8", "C25 primary receipt: FAIL E4 (not the named P7)",
     dict(BASE, type="explorer-mutant", control="C25", named="P7", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=101, panic="explorer-verdict", count=4, ceiling=60_000_000),
     ("FAIL", "FAIL, admissible (G.3(a) primary receipt)", "part only: G.3(a)")),
    ("x9", "witness NOT OBSERVED",
     dict(BASE, type="witness-mutant", control="C25", printed_outcome="witness FAIL; NAMED PROPERTY P7: NOT OBSERVED", harness="failed",
          exit=101, panic="witness-verdict", positive_line=False, not_observed=True, role_executable=MUTX, required_executable=MUTX),
     ("witness FAIL; NAMED PROPERTY P7: NOT OBSERVED", "NOT ADMISSIBLE (witness rejection, G.3(c) or G.5)", "no")),
    ("x10", "unit test fails in shared setup",
     dict(BASE, type="unit-mutant", printed_outcome="(panic in open_core)", harness="failed", exit=101, panic="unit-check",
          location_in_named_test=False),
     ("(panic in open_core)", "NOT ADMISSIBLE (failure outside the named test's own checks)", "no")),
    ("x11", "C3a FAIL of either named property",
     dict(BASE, type="explorer-mutant", control="C3a", named=("P13", "P15"), either=True, printed_outcome="FAIL",
          printed_property="P15", harness="failed", exit=101, panic="explorer-verdict", count=9, ceiling=30_000_000),
     ("FAIL", "FAIL, admissible (F.1 type (a)); no witness marker is required", "yes")),
    ("x12", "FAIL text but process timed out",
     dict(BASE, type="explorer-mutant", control="C10", named="E4", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=None, timeout=True, signal="SIGKILL", panic="explorer-verdict", count=9, ceiling=60_000_000),
     ("FAIL", "INCOMPLETE: a run killed from outside (line 855); never PASS", "no")),
    ("x13", "C4 primary receipt: FAIL P4, recorded as printed, a part only",
     dict(BASE, type="explorer-mutant", control="C4", named=("P4", "P8"), printed_outcome="FAIL", printed_property="P4",
          harness="failed", exit=101, panic="explorer-verdict", count=40, ceiling=30_000_000),
     ("FAIL", "FAIL, admissible (G.5(A) primary receipt; property recorded as printed)", "part only: G.5(A)")),
    ("x14", "mutant witness run on the baseline executable",
     dict(BASE, type="witness-mutant", control="C4", printed_outcome="witness FAIL", harness="failed", exit=101,
          panic="witness-verdict", p4_line=True, p8_line=True, role_executable=BASEX, required_executable=MUTX),
     ("witness FAIL", "NOT ADMISSIBLE (role identity, F.2)", "no")),
    ("x15", "healthy witness printing a positive marker",
     dict(BASE, type="witness-baseline", control="C1", printed_outcome="witness PASS; NAMED PROPERTY P1: OBSERVED", harness="ok",
          exit=0, healthy_line=True, positive_line=True, role_executable=BASEX, required_executable=BASEX),
     ("witness PASS; NAMED PROPERTY P1: OBSERVED", "NOT ADMISSIBLE (role identity, F.2)", "no")),
    ("x16", "healthy build leaving an executable unrecorded",
     dict(type="healthy-build", identity_ok=True, compiler_exit=0, diagnostics=[], finished_line=True,
          artifacts_produced=["deps/nexus_governed_control-x"], artifacts_recorded=[]),
     ("build exit 0; diagnostics []", "NOT ADMISSIBLE (a produced artifact unrecorded or marked NOT PRODUCED BY DESIGN)", "no")),
    ("x17", "healthy build marking its executable NOT PRODUCED BY DESIGN",
     dict(type="healthy-build", identity_ok=True, compiler_exit=0, diagnostics=[], finished_line=True,
          artifacts_produced=["deps/nexus_governed_control-x"], artifacts_recorded=["deps/nexus_governed_control-x"],
          marked_not_produced=True),
     ("build exit 0; diagnostics []", "NOT ADMISSIBLE (a produced artifact unrecorded or marked NOT PRODUCED BY DESIGN)", "no")),
    ("x18", "C22 mutant build leaving an artifact that goes unrecorded",
     dict(type="c22-build", identity_ok=True, compiler_exit=101, diagnostics=[("E0283", 1089)], designated_line=1089,
          healthy_build_ok=True, executable_sha256=None, test_selection=None, test_run=None,
          artifacts_left=["deps/libdep.rlib"], artifacts_recorded=[]),
     ("build exit 101; diagnostics [('E0283', 1089)]", "NOT ADMISSIBLE (an artifact the mutant build left is unrecorded)", "no")),
]
KEY = {  # numbered case -> (phrase that the draft's outer cell must contain, phrase that its kill cell must contain)
    "1": ("PASS", "—"), "2": ("never PASS", "no"), "3": ("G.3(b) holds", "a part only"), "4": ("NOT ADMISSIBLE", "no"),
    "5": ("FAIL, admissible (F.1 type (a))", "yes"), "6": ("NOT PRODUCED BY DESIGN", "yes, for C22 only"),
    "7": ("INCOMPLETE — BUDGET EXCEEDED", "no"), "8": ("G.5(B) holds", "a part only"),
    "9": ("both properties are required", "no"), "10": ("PASS (type (e3))", "a part only"),
    "11": ("NOT ADMISSIBLE (role identity, F.2)", "no"), "12": ("PASS (type (e4))", "—"),
    "13": ("PASS (type (e2))", "—"), "14": ("NOT ADMISSIBLE (G.6)", "no")}
draft_rows = {}
if len(sys.argv) > 1:
    k2 = open(sys.argv[1], encoding="utf-8").read().split("### K.2", 1)[1].split("\n## L. ", 1)[0]
    draft_rows = {c[0]: c for c in re.findall(r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", k2, flags=re.M)}
bad = 0
print("AD-1-R4 K.2 CLASSIFICATION CASES (synthetic descriptors; no Nexus execution)")
for cid, desc, receipt, expect in CASES:
    got = classify(receipt)
    ok = got == expect
    line = ""
    if cid in KEY and draft_rows:
        row = draft_rows.get(cid)
        outer_k, kill_k = KEY[cid]
        agree = bool(row) and outer_k in row[3] and outer_k in got[1] and row[4].strip().startswith(kill_k) and got[2].startswith(kill_k)
        ok = ok and agree
        line = f"        draft row {cid}: {'agrees' if agree else 'DISAGREES'} ('{outer_k}' / kill '{kill_k}')"
    bad += not ok
    print(f"{'OK  ' if ok else 'BAD '} case {cid:>3}: {desc}")
    print(f"        raw: {got[0]}\n        outer: {got[1]}\n        kill evidence: {got[2]}")
    if line:
        print(line)
    if got != expect:
        print(f"        EXPECTED: {expect}")
if draft_rows:
    missing = sorted(set(KEY) - set(draft_rows), key=int) + sorted(set(draft_rows) - set(KEY), key=int)
    bad += bool(missing)
    print(f"\ndraft K.2 table rows {sorted(draft_rows, key=int)}; unmatched {missing}")
print(f"\nCASES: {len(CASES) - bad} as expected, {bad} not")
sys.exit(1 if bad else 0)
