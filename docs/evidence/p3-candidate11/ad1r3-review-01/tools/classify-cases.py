#!/usr/bin/env python3
"""AD-1-R3 K.2 classification cases (P3-C11-DR01-D2A1-AD1R3-READINESS-01).
Applies AD-1-R3's rules (F.1 receipt types, K.1 ceilings, K.2 process-aware classification) to SYNTHETIC receipt
descriptors. These are document and checker cases only: nothing here builds, runs, mutates or reads any Nexus
executable or receipt. Each case states its expected raw outcome, outer classification and kill evidence; the script
exits 0 only if every case classifies as expected.
    classify-cases.py"""
import sys

ADMISSIBLE_EXIT = {"pass": 0, "fail": 101}


def classify(r):
    """Returns (raw outcome kept, outer classification, kill evidence)."""
    t = r["type"]
    if t == "c22-build":                                   # F.1 (d); K.2 read for C22
        raw = f"build exit {r['compiler_exit']}; diagnostics {r['diagnostics']}"
        if not r["identity_ok"]:
            return raw, "NOT ADMISSIBLE (identity)", "no"
        if r.get("signal") or r.get("ice") or r.get("oom") or r.get("timeout"):
            return raw, "NOT ADMISSIBLE (compiler crash or resource failure)", "no"
        if not r["healthy_build_ok"]:
            return raw, "NOT ADMISSIBLE (no healthy build evidence)", "no"
        if r["compiler_exit"] != 0 and r["diagnostics"] == [("E0283", r["designated_line"])]:
            nproduced = all(r.get(k) is None for k in ("executable_sha256", "test_selection", "test_run"))
            outer = "FAIL (C22 build failure, E0283 at the designated line)"
            return raw, outer + ("; executable and test-run fields NOT PRODUCED BY DESIGN" if nproduced else
                                 "; INVENTED FIELDS PRESENT"), "yes" if nproduced else "no"
        return raw, "NOT ADMISSIBLE (unrelated or misplaced diagnostic)", "no"

    raw = r["printed_outcome"]
    # K.2 point 6 and point 5: the launching command's records and the process end decide first.
    if r.get("signal") or r.get("supervisor_kill") or r.get("timeout") or r.get("oom"):
        return raw, "INCOMPLETE (run killed from outside); never PASS", "no"
    if not r["identity_ok"]:
        return raw, "NOT ADMISSIBLE (identity mismatch)", "no"
    if not (r["exact"] and r["matches"] == 1 and r["tests_ran"] == 1):
        return raw, "NOT ADMISSIBLE (selection)", "no"
    if r.get("refusal"):                                   # K.1 quotient precondition refusal
        return raw, "INCOMPLETE — VERIFICATION INVALID", "no"
    if not r["completed"]:
        return raw, "INCOMPLETE (no completion evidence); never PASS", "no"

    if r["harness"] == "ok":                               # a passed test
        if r["exit"] != ADMISSIBLE_EXIT["pass"]:
            return raw, "NOT ADMISSIBLE (harness and exit disagree)", "no"
        if t in ("explorer-baseline", "explorer-mutant"):
            if raw != "PASS" or not r.get("all_q_lines") or r["count"] > r["ceiling"]:
                return raw, "NOT ADMISSIBLE (PASS text, Q lines or cardinality)", "no"
            return raw, "PASS", "—" if t == "explorer-baseline" else "no"
        if t == "witness-baseline":
            return raw, "PASS (G.3(d))" if r.get("healthy_line") else "NOT ADMISSIBLE (healthy line missing)", "—"
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
        named = r["named"]
        ok = r["printed_property"] in named if r.get("either") else r["printed_property"] == named
        if ok:
            return raw, "FAIL, admissible (F.1 type (a)); no witness marker is required", "yes"
        return raw, "FAIL, admissible, of another property: not killed (7.5 stop rule)", "no"
    if t == "witness-mutant":
        if r["panic"] != "witness-verdict" or not r.get("positive_line") or r.get("not_observed") or r.get("fixture"):
            return raw, "NOT ADMISSIBLE (witness rejection, G.3(c))", "no"
        if not r.get("same_executable_as_a"):
            return raw, "NOT ADMISSIBLE (executable other than G.3(a)'s)", "no"
        return raw, "admissible witness receipt: G.3(b) holds", "part only: with G.3(a) and G.3(d)"
    if t == "unit-mutant":
        if r["panic"] != "unit-check" or not r.get("location_in_named_test"):
            return raw, "NOT ADMISSIBLE (failure outside the named test's own checks)", "no"
        return raw, "FAIL, admissible (F.1 type (b))", "yes"
    return raw, "NOT ADMISSIBLE (unknown type)", "no"


BASE = dict(identity_ok=True, exact=True, matches=1, tests_ran=1, completed=True, signal=None, supervisor_kill=False,
            timeout=False, oom=False, refusal=False)
CASES = [  # (id, description, receipt, expected (raw, outer, kill))
    ("1", "valid completed PASS",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=0, count=12_602_670,
          ceiling=30_000_000, panic=None),
     ("PASS", "PASS", "—")),
    ("2", "PASS text plus external kill",
     dict(BASE, type="explorer-baseline", printed_outcome="PASS", all_q_lines=True, harness="ok", exit=None,
          signal="SIGKILL", supervisor_kill=True, count=100, ceiling=30_000_000, panic=None),
     ("PASS", "INCOMPLETE (run killed from outside); never PASS", "no")),
    ("3", "correct witness verdict plus expected completed assertion failure",
     dict(BASE, type="witness-mutant", printed_outcome="witness FAIL; NAMED PROPERTY P7: OBSERVED on the world after step 1",
          harness="failed", exit=101, panic="witness-verdict", positive_line=True, same_executable_as_a=True),
     ("witness FAIL; NAMED PROPERTY P7: OBSERVED on the world after step 1",
      "admissible witness receipt: G.3(b) holds", "part only: with G.3(a) and G.3(d)")),
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
      "FAIL (C22 build failure, E0283 at the designated line); executable and test-run fields NOT PRODUCED BY DESIGN", "yes")),
    ("7", "raw FAIL discovered after the authorised cardinality is exceeded",
     dict(BASE, type="explorer-mutant", control="C24", named="P8", printed_outcome="FAIL", printed_property="P8",
          harness="failed", exit=101, panic="explorer-verdict", count=60_000_001, ceiling=60_000_000),
     ("FAIL", "INCOMPLETE — BUDGET EXCEEDED", "no")),
    # ---- extra negatives (not required by the mission; each guards a K.2 point) ----
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
    ("x7", "explorer FAIL of a property other than the named one (non-C1/C25)",
     dict(BASE, type="explorer-mutant", control="C24", named="P8", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=101, panic="explorer-verdict", count=50, ceiling=60_000_000),
     ("FAIL", "FAIL, admissible, of another property: not killed (7.5 stop rule)", "no")),
    ("x8", "C25 primary receipt: FAIL E4 (not the named P7)",
     dict(BASE, type="explorer-mutant", control="C25", named="P7", printed_outcome="FAIL", printed_property="E4",
          harness="failed", exit=101, panic="explorer-verdict", count=4, ceiling=60_000_000),
     ("FAIL", "FAIL, admissible (G.3(a) primary receipt)", "part only: G.3(a)")),
    ("x9", "witness NOT OBSERVED",
     dict(BASE, type="witness-mutant", printed_outcome="witness FAIL; NAMED PROPERTY P7: NOT OBSERVED", harness="failed",
          exit=101, panic="witness-verdict", positive_line=False, not_observed=True, same_executable_as_a=True),
     ("witness FAIL; NAMED PROPERTY P7: NOT OBSERVED", "NOT ADMISSIBLE (witness rejection, G.3(c))", "no")),
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
     ("FAIL", "INCOMPLETE (run killed from outside); never PASS", "no")),
]
bad = 0
print("AD-1-R3 K.2 CLASSIFICATION CASES (synthetic descriptors; no Nexus execution)")
for cid, desc, receipt, expect in CASES:
    got = classify(receipt)
    ok = got == expect
    bad += not ok
    print(f"{'OK  ' if ok else 'BAD '} case {cid:>3}: {desc}")
    print(f"        raw: {got[0]}\n        outer: {got[1]}\n        kill evidence: {got[2]}")
    if not ok:
        print(f"        EXPECTED: {expect}")
print(f"\nCASES: {len(CASES) - bad} as expected, {bad} not")
sys.exit(1 if bad else 0)
