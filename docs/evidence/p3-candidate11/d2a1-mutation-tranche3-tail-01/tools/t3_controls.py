#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01, mission section 11: test this mission's tooling on SYNTHETIC and
HISTORICAL evidence only, before the freeze. No Cargo, no rustc, no Nexus build, test or executable is run; every
worktree, target and executable outside this mission is read, never written.

    t3_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-t3-controls) receives the synthetic
                                       receipts this tool classifies or links, CONTROLS.txt, CONTROLS.json and
                                       SUMMARY.txt (last line `T3 CONTROLS: ALL AS EXPECTED` or `T3 CONTROLS: FAILED`).
                                       SCRATCH (under dev-local/, never published) holds every copy of historical
                                       evidence (build receipts, tampered input roots), so that no published file
                                       repeats a previous packet's bytes.

A  classify_a (unchanged) and t3.rules on synthetic type-(a) receipts for the ten controls, transformed field by field
   from a historical receipt (CONTINUE-02's C9 run, receipts/38-C9/04-run at ebe42009), each marked SYNTHETIC with this
   mission's name. Positive: every control's first permitted property; C3b's alternative P12; C20 and A5 FAIL L1 after
   a complete search; F-1 exactly at 60,000,000; F-6 exactly at 30,000,000. Negative (mission section 11): C3b wrong
   property; C3c P13 alone; the F-PT composed quotient used as C6's mutant execution; C6 and C7 with A4 OFF; C7's
   informational F-3 receipt presented as C7's kill; F-1 beyond 60,000,000; F-6 beyond 30,000,000; C20 FAIL L1 before a
   complete search (expanded < discovered); A5 INCOMPLETE at the cap; C20 another property; a kill from outside.
B  control.link on the real authenticated linked inputs for the ten controls (each after a synthetic restoration
   receipt): LINK: VERIFIED; negative: a link before restoration (no restoration receipt), after a failed restoration,
   a changed F-1 raw output (C2), another test (A1), other bounds and the F-B header expectation (C6), a changed F-6
   classification (C3b): LINK: FAILED with the intended check failing.
C  expectations.py on tampered inputs (catalog, a patch): PROBLEMS — STOP; on the real inputs: equal to the frozen
   receipts/02-EXPECTATIONS.json.
D  the unchanged general build classifier (classify_build_success): a warning-bearing new build (the historical C5
   build's five warnings, presented as C2's build) -> BUILD: NOT OK with exactly the warning check failing; a copy of
   the accepted A3 build (no diagnostic) -> BUILD: PINNED, its CHECKS.txt and PIN.json byte-identical to the accepted
   ones (written only under SCRATCH).
E  t3.classify_info (C7's informational run): synthetic F-3 receipts: FAIL E11, PASS of a complete search, INCOMPLETE
   at the state cap -> admissible informational outcomes, never a kill label; negative: a FAIL beyond the cap
   (overshoot), a timeout kill, an OOM kill, another executable, the source not C7's mutant, the composed quotient, A4
   ON (F-PT bounds), another test: NOT ADMISSIBLE — STOP.
F  control.main's refusals (C5, C22 accepted; C1, C4, C25 reserved) and control.gate for C24 before C2's RESULT and
   for C2 before the pre-execution gate: refused or closed, nothing written.

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 16, AMENDMENT-01) for this mission's nine controls:
A-F as above for this order (C2's cases moved to C24 or A1; C24 classified as main_c24 does, source EXACTLY_C, and a
negative where it is classified as a patched control would be); B with C24 linked only after its own reauthentication
(negatives: no reauthentication receipt; a restoration receipt in its place); D reruns the unchanged general classifier on
a copy of the historical C24 build receipt (BUILD: NOT OK again, its outputs byte-identical to the historical ones) and
on copies carrying a warning for each of C6, C7, A1, A2, C20 and A5 (BUILD: NOT OK); F the refusals for C2 (accepted) and
the gates before C24's RESULT and before the pre-execution gate. New:
G  the C24 build-admission checker (c24.admit), the preserved-executable pin (c24.pin) and the preserved worktree check
   (c24.worktree_check), the C5 checker's controls for C24: the real preserved receipt ADMISSIBLE; a synthetic unmodified
   copy fails only its synthetic line; tampered copies (the warning absent, an extra warning, a wrong warning (another
   variant), a wrong location, an error plus the warning, a two-warning summary, an unclassified line, the wrong mutant
   source, the wrong executable hash or size, exit 101, an OOM kill, a timeout) REJECTED with exactly the targeted check
   failing; the C24 warning presented for C3b, C3c, C6, C7, A1, A2, C20 and A5: refused; a byte-changed copy in normal
   mode: the identity check fails; the pin on the real executable PINNED; a missing executable, a wrong hash, a wrong size,
   a symlink: C24_PRESERVED_ARTIFACT_BLOCKED; the worktree AUTHENTICATED; another lock reason expected: NOT AUTHENTICATED.
H  the C3b/C3c build-admission checker (c3w.admit, AMENDMENT-01) on synthetic build receipts built from the accepted C2
   build receipt (receipts/66-C2/03-build at 774f457a, over C2's real preserved target; every copy and every general-
   classifier output under SCRATCH): clean C3b and C3c builds ADMISSIBLE (CLEAN); exactly the predicted warning ADMISSIBLE
   (NARROW) for each; REJECTED: C3b Future plus an extra warning, C3b Set, C3c Set plus an extra warning, C3c Future, C3b
   another warning, C3b an error plus the warning, C3b the warning at another location, C3b a two-warning summary, C3b
   with C3c's mutant identity; and for each of C6, C7, A1, A2, C20, A5 with the Future warning: c3w.admit refuses and
   the unchanged general classifier says BUILD: NOT OK.

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (section 5, steps 8-12) for this mission's four controls
(A1 A2 C20 A5). E, G and H are removed (C7 blocked: no informational run; C24, C3b and C3c accepted: c24.py and c3w.py
are not carried). The remaining sections, over the four controls:
A  classify_a and t3.rules (both unchanged) on synthetic type-(a) receipts. Positive: A1 and A2 FAIL P1; C20 and A5
   FAIL L1 after a complete search; A1 exactly at the F-1 ceiling 60,000,000; C20 FAIL L1 exactly at 30,000,000 with
   every state expanded. Wrong property (kills nothing): A1 P4, A2 E6, C20 P12, A5 E11, A5 D1. State-cap overshoot
   (kills nothing): A1 and A2 beyond 60,000,000; C20 and A5 FAIL L1 beyond 30,000,000. The L1 complete-search rule
   rejects prefixes and inference: FAIL L1 with expanded < discovered (C20, A5); FAIL L1 beside an INCOMPLETE line (the
   time cap); a malformed L1 detail; an L1 detail naming more states than were discovered, or none; FAIL L1 at the time
   ceiling; FAIL L1 with a property line. Incomplete runs (kills nothing): INCOMPLETE at the state cap (C20, A5) and at
   the time cap (C20); a run killed from outside (A1, C20). Wrong explorer or test: the F-2 composed quotient used as
   C20's mutant execution; A2 run by full_f2. Every case also records the RESULT label control.main gives it.
B  control.link on the real authenticated linked inputs for the four (after a synthetic restoration receipt): LINK:
   VERIFIED; negative: no restoration receipt, a failed restoration, a changed F-1 raw output (A1), another test (A2), a
   changed F-2 classification (C20), other bounds (A5): LINK: FAILED with the intended check failing.
C  as before.
D  the unchanged general build classifier: the historical C24 build (BUILD: NOT OK reproduced byte for byte); its
   warning presented as each of A1, A2, C20 and A5's builds: BUILD: NOT OK; the accepted A3 build: BUILD: PINNED.
F  control.main refuses C7 by name (MUTATION_CONTROL_BLOCKED), the accepted controls (C24 C3b C3c C6 C2 C5 C22), the
   reserved ones (C1 C4 C25) and an unknown id, writing nothing; no expectation exists for C7; control.gate refuses C7,
   A2 before A1's RESULT, and A1 before the freeze and the pre-execution gate.
"""
import copy
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import t3  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
MARK = "SYNTHETIC (t3_controls.py, P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01): not the output of any run\n"
RSS = 4397  # a distinctive synthetic resource record (this mission's own), so that no synthetic classification repeats
# an earlier packet's (TRANCHE3-EXEC-01 used 4373, TRANCHE3-CONTINUE-02 4381); section A's case directories begin with
# the control id and are numbered from 101 for the same reason (the tranche-3 rules record their own receipt path)
results = []


def record(cid, case, expected, got, ok, detail=""):
    results.append({"control": cid, "case": case, "expected": expected, "got": got, "as_expected": bool(ok),
                    "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {cid:4} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def exe_for(cid):
    sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (t3_controls.py; never built, never run)".encode())
    path = f"/synthetic/nexus-os-p3-c11-mt3t-{cid}/target/release/deps/nexus_governed_control-0123456789abcdef"
    return {"path": path, "sha256": sha, "size": 4373}


def base_raw():
    return K.blob(K.T1, BASE + "/raw-output.txt").decode()


def transform(raw, test, header, bounds, actors, va_line, va_col):
    raw = raw.replace(BASE_TEST, test).replace("EXPLORATION F-B2", header)
    raw = re.sub(r"^  bounds: .*$", lambda m: "  bounds: " + bounds, raw, flags=re.M)
    raw = re.sub(r"^  actors: .*$", lambda m: "  actors: " + actors, raw, flags=re.M)
    return raw.replace("model_tests.rs:4133:5:", f"model_tests.rs:{va_line}:{va_col}:")


def synth(d, ctl, exe, prop=None, states=None, expanded=None, fail_line=None, header=None, test=None, bounds=None,
          actors=None, source_verdict="EXACTLY_C_PLUS_PATCH", raw=None, process=None, sup=None, argv_tail=None,
          mutant_blob=None, post=None):
    """A synthetic receipt directory d (descriptor.json, raw-output.txt) for control ctl (c2_controls.py's
    transformation, with this mission's marker; fail_line replaces the whole FAIL line, e.g. an L1 detail)."""
    os.makedirs(d)
    desc = json.loads(K.blob(K.T1, BASE + "/descriptor.json"))
    t = test or ctl["test"]
    tail = argv_tail or [t, *ctl["argv_tail"][1:]]
    if raw is None:
        va = ctl["verdict_assertion"]
        raw = transform(base_raw(), t, header or f"EXPLORATION {ctl['configuration']}", bounds or ctl["bounds"],
                        actors or ctl["actors"], va["line"], va["col"])
        p = prop or (ctl.get("permitted_properties") or [ctl["named_property"]])[0]
        fl = fail_line or None
        if fl is not None:
            raw = re.sub(r"^  FAIL: E4: .*$", lambda m: "  FAIL: " + fl, raw, flags=re.M)
        else:
            raw = raw.replace("  FAIL: E4: ", f"  FAIL: {p}: ")
        raw = raw.replace('failed: failed: {"E4"}', f'failed: failed: {{"{p}"}}')
        if states is not None:
            raw = re.sub(r"^  states 14 \(expanded 1\)", lambda m: f"  states {states} (expanded "
                         f"{expanded if expanded is not None else 1})", raw, flags=re.M)
        if post is not None:  # TRANCHE3-TAIL-01: one more exact transformation of the report (both of its copies)
            raw = post(raw)
    raw = MARK + raw
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    listing = {"exit": 0, "stdout": f"{t}: test\n\n1 test, 0 benchmarks\n", "stderr": "", "listed": [f"{t}: test"]}
    mb = mutant_blob or ctl["mutant_blob"]
    desc.update({"id": os.path.basename(d) if os.path.basename(d) in ("04-run", "04b-info-F3") else "04-run",
                 "test": t, "argv": [exe["path"], *tail], "configuration": ctl["configuration"], "control": ctl["id"],
                 "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"],
                 "worktree": ctl["worktree"], "limit_seconds": ctl["limit_seconds"],
                 "cwd": os.path.join(ctl["worktree"], K.CRATE), "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "t3_controls.py: a SYNTHETIC receipt transformed from CONTINUE-02 receipts/38-C9/04-run; "
                              "not Nexus evidence of any run",
                 "executable": {"path": exe["path"], "pinned_sha256": exe["sha256"], "pinned_size": exe["size"],
                                "before": ident, "after": ident},
                 "selection": {"listing": dict(listing, argv=[t, "--exact", "--list"]),
                               "ignored_listing": dict(listing, argv=[t, "--exact", "--ignored", "--list"]),
                               "expected_ignored": True, "executable_after_listing": ident}})
    for when in ("source_before", "source_after"):
        s = desc[when]
        s.update({"worktree": ctl["worktree"], "verdict": source_verdict,
                  "mutant_file": dict(s["mutant_file"], path=ctl["source_file"], git_blob=mb,
                                      sha256=ctl["mutant_sha256"], baseline_blob=ctl["baseline_blob"]),
                  "expected_mutant_blob": ctl["mutant_blob"], "expected_mutant_sha256": ctl["mutant_sha256"]})
    if process:
        desc["process"] = process
    desc["supervisor"] = dict(desc["supervisor"], max_rss_kib_largest_process=RSS, **(sup or {}))
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)


def first_two(d):
    ls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    return ls[0], ls[1].replace("outer class (AD-1-R4 K.2): ", "")


def l1_line(k):
    return f"L1: {k} states lie on or behind a cycle of non-fault steps; one is reached by"


def pass_report(raw):
    """(header, lines) of the one exploration report in an accepted PASS receipt's raw output (report.parse_report)."""
    import report
    rep = report.parse_report(raw)
    return rep["header"], list(rep["lines"])


def incomplete_lines(lines, states, expanded, reason):
    """A PASS report's lines turned into the explorer's own INCOMPLETE form: outcome INCOMPLETE, the counts, the
    INCOMPLETE line after the elapsed line, every property NOT ESTABLISHED (explore()'s text)."""
    out = []
    for l in lines:
        if l == "  outcome: PASS":
            out.append("  outcome: INCOMPLETE")
        elif l.startswith("  states "):
            out.append(re.sub(r"^  states \d+ \(expanded \d+\)", f"  states {states} (expanded {expanded})", l))
        elif l.startswith("  elapsed "):
            out += [l, f"  INCOMPLETE: {reason}. No property is established by this run."]
        elif re.match(r"^  (P\d+|D1|L1|E\d+|KEY): ", l):
            out.append(f"  {l.strip().split(': ', 1)[0]}: NOT ESTABLISHED (incomplete)")
        else:
            out.append(l)
    return out


def compose(test, header, lines, kind, va):
    """A harness transcript around one report, in the executable's own form: kind PASS (ok, exit 0) or INCOMPLETE (the
    explorer's verdict assertion: left Incomplete, right Pass)."""
    text = "\n".join([header] + lines)
    if kind == "PASS":
        return (f"\nrunning 1 test\ntest {test} ... {text}\n\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 "
                f"measured; {K.LISTED - 1} filtered out; finished in 60.00s\n\n")
    return (f"\nrunning 1 test\ntest {test} ... {text}\n\n\nthread '{test}' (4373) panicked at {va['file']}:{va['line']}:"
            f"{va['col']}:\nassertion `left == right` failed: failed: {{}}\n{text}\n\n  left: Incomplete\n right: Pass\n"
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\nFAILED\n\nfailures:\n\n"
            f"failures:\n    {test}\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} "
            "filtered out; finished in 600.00s\n\n")


def result_label(first, outer, rules_ok):
    """The RESULT label control.main gives a run with this classification (its candidate run, restored and linked;
    control.INCOMPLETE_OUTER otherwise): the mission's section-12 vocabulary."""
    if first == classify.CANDIDATE and rules_ok:
        return control.RESULT_OK
    if first != classify.CANDIDATE and ("outer class (AD-1-R4 K.2): " + outer).startswith(control.INCOMPLETE_OUTER):
        return control.RESULT_INCOMPLETE
    return control.RESULT_BLOCKED


def sub_all(old, new):
    def f(raw):
        assert old in raw, old[:60]
        return raw.replace(old, new)
    return f


def section_a(out, exp):
    print("A. classify_a (unchanged) and t3.rules on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run)")
    C = exp["controls"]
    OK = "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property"
    BUDGET = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    QH = ("EXPLORATION F-2 (gating-caller symmetry quotient composed with the admission observer quotient: "
          "experimental, not the contract's explorer)")
    R_OK, R_BL, R_IN = control.RESULT_OK, control.RESULT_BLOCKED, control.RESULT_INCOMPLETE
    l1ok = {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 120}
    ELAPSED = "  elapsed 0.0 s; explorer tables 0 MiB"
    cases = [(cid, "P-first-permitted-property", {} if cid not in K.L1_CONTROLS else dict(l1ok), classify.CANDIDATE,
              OK, True, R_OK) for cid in K.ORDER]
    cases += [
        ("A1", "P-FAIL-exactly-at-the-F-1-ceiling-60000000", {"states": 60000000}, classify.CANDIDATE, OK + " P1", True,
         R_OK),
        ("C20", "P-L1-after-a-complete-search-exactly-at-the-30000000-ceiling",
         {"prop": "L1", "fail_line": l1_line(9), "states": 30000000, "expanded": 30000000}, classify.CANDIDATE,
         OK + " L1", True, R_OK),
        ("A1", "N-wrong-property-P4", {"prop": "P4"}, classify.NOT_CANDIDATE,
         "FAIL of P4, not the named property P1: kills nothing (no attribution exception)", False, R_BL),
        ("A2", "N-wrong-property-E6", {"prop": "E6"}, classify.NOT_CANDIDATE,
         "FAIL of E6, not the named property P1: kills nothing (no attribution exception)", False, R_BL),
        ("C20", "N-wrong-property-P12-instead-of-L1", {"prop": "P12"}, classify.NOT_CANDIDATE,
         "FAIL of P12, not the named property L1: kills nothing (no attribution exception)", False, R_BL),
        ("A5", "N-wrong-property-E11-instead-of-L1", {"prop": "E11"}, classify.NOT_CANDIDATE,
         "FAIL of E11, not the named property L1: kills nothing (no attribution exception)", False, R_BL),
        ("A5", "N-wrong-property-D1-instead-of-L1", {"prop": "D1"}, classify.NOT_CANDIDATE,
         "FAIL of D1, not the named property L1: kills nothing (no attribution exception)", False, R_BL),
        ("A1", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"states": 60000001}, classify.NOT_CANDIDATE, BUDGET, False,
         R_IN),
        ("A2", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"states": 60000001}, classify.NOT_CANDIDATE, BUDGET, False,
         R_IN),
        ("C20", "N-L1-FAIL-beyond-30000000 (cap overshoot)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 30000001, "expanded": 30000001}, classify.NOT_CANDIDATE,
         BUDGET, False, R_IN),
        ("A5", "N-L1-FAIL-beyond-30000000 (cap overshoot)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 30000001, "expanded": 30000001}, classify.NOT_CANDIDATE,
         BUDGET, False, R_IN),
        ("C20", "N-L1-from-a-search-prefix (expanded < discovered)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 119}, classify.CANDIDATE, OK, False, R_BL),
        ("A5", "N-L1-from-a-search-prefix (expanded < discovered)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 7}, classify.CANDIDATE, OK, False, R_BL),
        ("C20", "N-L1-beside-an-INCOMPLETE-line (the time cap ended the search)",
         dict(l1ok, post=sub_all(ELAPSED + "\n", ELAPSED + "\n  INCOMPLETE: the time cap of 2700s was reached. No "
                                                           "property is established by this run.\n")),
         classify.NOT_CANDIDATE, "NOT ADMISSIBLE (", False, R_BL),
        ("A5", "N-L1-detail-malformed (not the explorer's own L1 text)",
         dict(l1ok, fail_line="L1: a cycle of non-fault steps exists"), classify.CANDIDATE, OK, False, R_BL),
        ("C20", "N-L1-detail-names-more-states-than-were-discovered", dict(l1ok, fail_line=l1_line(121)),
         classify.CANDIDATE, OK, False, R_BL),
        ("A5", "N-L1-detail-names-no-state", dict(l1ok, fail_line=l1_line(0)), classify.CANDIDATE, OK, False, R_BL),
        ("C20", "N-L1-at-the-time-ceiling (elapsed 2700 s)",
         dict(l1ok, post=sub_all(ELAPSED, "  elapsed 2700.0 s; explorer tables 0 MiB")), classify.CANDIDATE, OK, False,
         R_BL),
        ("A5", "N-L1-with-a-property-line (L1 inferred beside an established property)",
         dict(l1ok, post=sub_all(ELAPSED + "\n", ELAPSED + "\n  D1: HOLDS\n")), classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (", False, R_BL),
        ("C20", "N-incomplete-search (INCOMPLETE at the state cap)", {"raw": "INCOMPLETE"}, classify.NOT_CANDIDATE,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing",
         False, R_IN),
        ("A5", "N-incomplete-search (INCOMPLETE at the state cap)", {"raw": "INCOMPLETE"}, classify.NOT_CANDIDATE,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing",
         False, R_IN),
        ("C20", "N-incomplete-search (INCOMPLETE at the time cap)", {"raw": "INCOMPLETE-TIME"}, classify.NOT_CANDIDATE,
         "INCOMPLETE (the time cap of 2700s was reached. No property is established by this run.): kills nothing",
         False, R_IN),
        ("A1", "N-killed-from-outside", {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                     "wrapper_recorded_status": "137"},
                                         "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"},
         classify.NOT_CANDIDATE, "INCOMPLETE (killed from outside or abnormal termination): kills nothing", False, R_IN),
        ("C20", "N-killed-from-outside (OOM)", {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                            "wrapper_recorded_status": "137"},
                                                "sup": {"oom_kill": 1}, "raw": "TRUNCATED"},
         classify.NOT_CANDIDATE, "INCOMPLETE (killed from outside or abnormal termination): kills nothing", False, R_IN),
        ("C20", "N-F-2-composed-quotient-used-as-the-mutant-execution", dict(
            l1ok, header=QH, test=K.TEST_PREFIX + "gc_symmetry_f2"), classify.NOT_CANDIDATE, "NOT ADMISSIBLE (", False,
         R_BL),
        ("A2", "N-another-test (full_f2)", {"test": K.TEST_PREFIX + "full_f2"}, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (", False, R_BL),
    ]
    fr = open(K.inputs("linked", "S1-41-full-F-2", "raw-output.txt"), encoding="utf-8").read()
    for n, (cid, case, kw, want_first, want_outer, want_rules, want_result) in enumerate(cases, 101):
        ctl = C[cid]
        exe = exe_for(cid)
        kw = dict(kw)
        if kw.get("raw") in ("INCOMPLETE", "INCOMPLETE-TIME"):
            # An INCOMPLETE F-2 report in the explorer's own form, from the accepted F-2 PASS receipt's report.
            h, ls = pass_report(fr)
            if kw["raw"] == "INCOMPLETE":
                ls = incomplete_lines(ls, 30000001, 26000000, "the state cap of 30000000 was reached")
            else:
                ls = incomplete_lines(ls, 21000000, 20999000, "the time cap of 2700s was reached")
            kw["raw"] = compose(ctl["test"], h, ls, "INCOMPLETE", ctl["verdict_assertion"])
            kw["process"] = {"exit_status": 101, "signal": None, "supervisor_rc": 101, "wrapper_recorded_status": "101"}
        elif kw.get("raw") == "TRUNCATED":
            kw["raw"] = "\n".join(base_raw().replace(BASE_TEST, ctl["test"]).splitlines()[:6]) + "\n"
        d = os.path.join(out, "A", f"{cid}-{n:03d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}", "04-run")
        synth(d, ctl, exe, **kw)
        classify.classify_a(d, ctl, exe)
        first, outer = first_two(d)
        rok = t3.rules(os.path.join(os.path.dirname(d), "04a-rules"), d, ctl)
        candidate = first == classify.CANDIDATE and rok
        label = result_label(first, outer, rok)
        ok = first == want_first and outer.startswith(want_outer) and rok == want_rules and label == want_result
        failed = [l for l in open(os.path.join(os.path.dirname(d), "04a-rules/RULES.txt")).read().splitlines()
                  if l.startswith("FAIL")]
        record(cid, "A " + case, f"{want_first} / {want_outer}…; rules {'HOLD' if want_rules else 'NOT SATISFIED'}; "
               f"candidate {want_first == classify.CANDIDATE and want_rules}; {want_result}",
               f"{first} / {outer[:100]}; rules {'HOLD' if rok else 'NOT SATISFIED'}; candidate {candidate}; {label}", ok,
               [f[:90] for f in failed][:3])


def report_props():
    import report
    return report.model_constants()[0]


def section_b(out, scratch, exp):
    print("B. control.link on the authenticated linked inputs (after a synthetic restoration receipt), and negatives")
    C = exp["controls"]

    def restore_rec(base, first="RESTORE: EXACTLY_C", sub="05-restore", name="RESTORE.txt"):
        os.makedirs(os.path.join(base, sub))
        open(os.path.join(base, sub, name), "w").write(
            f"{first}\n{MARK}a synthetic restoration receipt for a tooling control of the link step\n")
    for cid in K.ORDER:
        base = os.path.join(out, "B", f"{cid}-real-inputs")
        restore_rec(base)
        ok, ref = control.link(C[cid], exp, os.path.join(base, "06-link"), None)
        first = supervise.first_line(os.path.join(base, "06-link/LINK.txt"))
        record(cid, "B link on the real authenticated inputs, after a restoration receipt", "LINK: VERIFIED", first,
               first == "LINK: VERIFIED" and ok, ref[:90])
    root = os.path.join(scratch, "B-ev")
    saved = K.EV

    def fresh():
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
        for a, _, fs in os.walk(root):
            for f in fs:
                os.chmod(os.path.join(a, f), 0o644)
        K.EV = root

    def tamper(rel, old, new):
        p = os.path.join(root, rel)
        b = open(p, "rb").read()
        assert old in b, (rel, old)
        open(p, "wb").write(b.replace(old, new, 1))
    restoration = "this control's own restoration receipt exists and records RESTORE: EXACTLY_C"
    neg = [
        ("A1", "N-linked-receipt-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("C20", "N-linked-receipt-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("A5", "N-a-reauthentication-receipt-instead-of-the-restoration-receipt", None, None, "C24 REAUTHENTICATION AFTER "
         "THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)", restoration),
        ("A1", "N-F-1-raw-output-byte-changed", lambda: tamper("inputs/linked/S1-45-full-F-1/raw-output.txt",
                                                            b"outcome: PASS", b"outcome: PASs"), None, "RESTORE: EXACTLY_C",
         "its classification blob and raw output identity are the catalog's"),
        ("A2", "N-another-test-in-the-expectation", None, lambda c: c["argv_tail"].__setitem__(0, K.TEST_PREFIX + "full_f2"),
         "RESTORE: EXACTLY_C", "exact configuration: the same test and flags as this control's run (F-1)"),
        ("C20", "N-F-2-classification-byte-changed", lambda: tamper("inputs/linked/S1-41-full-F-2/CLASSIFICATION.txt",
                                                                  b"CLASSIFICATION: PASS", b"CLASSIFICATION: PASs"), None,
         "RESTORE: EXACTLY_C", "its classification blob and raw output identity are the catalog's"),
        ("A5", "N-the-F-1-receipt-linked-for-an-F-2-control", None, lambda c: c.__setitem__("linked", dict(
            C["A1"]["linked"])), "RESTORE: EXACTLY_C", "exact configuration: the same test and flags as this control's run"),
    ]
    try:
        for cid, case, files, ctlmod, rfirst, want in neg:
            fresh()
            if files:
                files()
            ctl = copy.deepcopy(C[cid])
            if ctlmod:
                ctlmod(ctl)
            base = os.path.join(out, "B", f"{cid}-{case[:60]}")
            os.makedirs(base)
            if rfirst:
                restore_rec(base, rfirst)
            ok, ref = control.link(ctl, exp, os.path.join(base, "06-link"), None)
            text = open(os.path.join(base, "06-link/LINK.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record(cid, "B " + case, f"LINK: FAILED with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == "LINK: FAILED" and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed", "inputs/catalog.json", b'"P13 or P12"', b'"P13 or P15"',
             "authenticated catalog copy sha256"),
            ("N-patch-copy-changed", "inputs/patches/C20.patch", b"if cell == Cell::C {", b"if cell == Cell::X {",
             "C20 authenticated patch copy sha256")):
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
        p = os.path.join(root, rel)
        os.chmod(p, 0o644)
        b = open(p, "rb").read()
        assert old in b, (rel, old)
        open(p, "wb").write(b.replace(old, new, 1))
        K.EV = root
        E.problems.clear()
        try:
            try:
                rc = E.main(os.path.join(scratch, f"C-{case}.json"))
            except Exception as ex:  # a tampered input may stop the derivation itself: still a STOP
                rc, E.problems[:] = 2, E.problems + [f"exception: {ex!r}"]
        finally:
            K.EV = saved
        hit = [p for p in E.problems if want in p]
        record("—", "C " + case, f"PROBLEMS — STOP naming '{want}'", f"rc {rc}; {len(E.problems)} problems",
               rc != 0 and hit and not os.path.exists(os.path.join(scratch, f"C-{case}.json")), hit[:1])
    E.problems.clear()
    rc = E.main(os.path.join(scratch, "C-real-inputs-EXPECTATIONS.json"))
    same = K.sha256_file(os.path.join(scratch, "C-real-inputs-EXPECTATIONS.json")) == K.sha256_file(
        os.path.join(K.EV, "receipts/02-EXPECTATIONS.json"))
    record("—", "C P-real-inputs", "DERIVED, equal to receipts/02-EXPECTATIONS.json", f"rc {rc}; equal {same}",
           rc == 0 and same)


def section_d(out, scratch, exp):
    print("D. the unchanged general build classifier: the historical C24 build; warnings on A1 A2 C20 A5; an "
          "accepted warning-free build")
    C = exp["controls"]

    def copy_receipt(commit, packet_dir, name):
        d = os.path.join(scratch, "D", name)
        os.makedirs(d)
        for f in K.git("ls-tree", "--name-only", commit, packet_dir + "/").decode().split():
            open(os.path.join(d, os.path.basename(f)), "wb").write(K.blob(commit, f))
        return d
    # The historical C24 build: the general classifier (unchanged) reproduces BUILD: NOT OK, byte for byte.
    g = copy_receipt(K.T3X, K.C24_BUILD_DIR, "C24-historical-general-classifier-rerun")
    for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"):
        os.remove(os.path.join(g, f))
    rc = classify.classify_build_success(g, "pin", "EXACTLY_C_PLUS_PATCH", K.C24_MUTANT_BLOB,
                                         expect_target=os.path.join(K.C24_WT, "target"))
    same = {f: K.sha256_file(os.path.join(g, f)) == K.sha256_bytes(K.blob(K.T3X, f"{K.C24_BUILD_DIR}/{f}"))
            for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json")}
    first = supervise.first_line(os.path.join(g, "CLASSIFICATION.txt"))
    record("C24", "D the general classifier (classify_build_success, unchanged) rerun on a copy of the historical C24 "
           "build receipt (C24 is Architect-accepted; its historical build is the warning-bearing base here)", "BUILD: NOT OK; CHECKS.txt, CLASSIFICATION.txt, PARSED-RECEIPT.json byte-identical to the "
           "historical ones; no PIN.json", f"{first}; rc {rc}; identical {same}; PIN.json "
           f"{os.path.exists(os.path.join(g, 'PIN.json'))}", first == "BUILD: NOT OK" and rc == 1 and all(same.values())
           and not os.path.exists(os.path.join(g, "PIN.json")))
    # The C24 warning presented as each zero-warning control's build: rejected by the unchanged general classifier.
    for cid in K.ORDER:
        w = copy_receipt(K.T3X, K.C24_BUILD_DIR, f"{cid}-with-a-warning")
        for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"):
            os.remove(os.path.join(w, f))
        pth = os.path.join(w, "descriptor.json")
        j = json.load(open(pth))
        for when in ("source_before", "source_after"):
            j[when]["mutant_file"]["git_blob"] = C[cid]["mutant_blob"]
        json.dump(j, open(pth, "w"), indent=1)
        rc = classify.classify_build_success(w, "pin", "EXACTLY_C_PLUS_PATCH", C[cid]["mutant_blob"],
                                             expect_target=os.path.join(K.C24_WT, "target"))
        fails = [l[5:].split(":")[0] for l in open(os.path.join(w, "CHECKS.txt")).read().splitlines()
                 if l.startswith("FAIL")]
        first = supervise.first_line(os.path.join(w, "CLASSIFICATION.txt"))
        record(cid, "D a warning-bearing build presented as " + cid + "'s: rejected (zero warnings required), no PIN.json",
               "BUILD: NOT OK with exactly the check 'no error or warning diagnostic' failing",
               f"{first}; failing {fails}; PIN.json {os.path.exists(os.path.join(w, 'PIN.json'))}",
               first == "BUILD: NOT OK" and rc == 1 and fails == ["no error or warning diagnostic"]
               and not os.path.exists(os.path.join(w, "PIN.json")))
    a3dir = f"{K.T2C_PACKET}/{K.T2C_RECEIPTS['A3']}/03-build"
    a = copy_receipt(K.T2C, a3dir, "accepted-A3-build-rerun")
    for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json", "PIN.json"):
        os.remove(os.path.join(a, f))
    rc = classify.classify_build_success(a, "pin", "EXACTLY_C_PLUS_PATCH",
                                         json.loads(K.blob(K.T2C, a3dir + "/descriptor.json"))["mutant_blob"],
                                         expect_target=K.ACCEPTED_WT["A3"] + "/target")
    same = {f: K.sha256_file(os.path.join(a, f)) == K.sha256_bytes(K.blob(K.T2C, f"{a3dir}/{f}"))
            for f in ("CHECKS.txt", "CLASSIFICATION.txt")}
    pin_new, pin_old = json.load(open(os.path.join(a, "PIN.json"))), json.loads(K.blob(K.T2C, f"{a3dir}/PIN.json"))
    same["PIN.json (every field but build_receipt, the receipt directory's own path)"] = (
        {k: v for k, v in pin_new.items() if k != "build_receipt"} == {k: v for k, v in pin_old.items()
                                                                      if k != "build_receipt"})
    first = supervise.first_line(os.path.join(a, "CLASSIFICATION.txt"))
    record("A3", "D the unchanged classifier rerun on a copy of the accepted A3 build (no diagnostic)",
           "BUILD: PINNED; CHECKS.txt and CLASSIFICATION.txt byte-identical to the accepted ones; PIN.json equal but "
           "for its own receipt path",
           f"{first}; rc {rc}; identical {same}", first == "BUILD: PINNED" and rc == 0 and all(same.values()))


def section_f(exp):
    print("F. control.main refusals (C7 blocked by name; accepted; reserved; unknown) and control.gate (read-only)")
    before = sorted(os.listdir(os.path.join(K.EV, "receipts")))
    for cid, want in (("C7", "REFUSED: C7 is MUTATION_CONTROL_BLOCKED by the Architect and expressly excluded"),
                      ("C24", "REFUSED: C24 is not executed"), ("C3b", "REFUSED: C3b is not executed"),
                      ("C3c", "REFUSED: C3c is not executed"), ("C6", "REFUSED: C6 is not executed"),
                      ("C2", "REFUSED: C2 is not executed"), ("C5", "REFUSED: C5 is not executed"),
                      ("C22", "REFUSED: C22 is not executed"), ("C1", "REFUSED: C1 is not executed"),
                      ("C4", "REFUSED: C4 is not executed"), ("C25", "REFUSED: C25 is not executed"),
                      ("A4", "REFUSED: A4 is not executed"), ("X9", "REFUSED: X9 is not executed")):
        try:
            control.main(cid)
            got = "NOT REFUSED"
        except SystemExit as e:
            got = str(e)
        record(cid, "F control.main refuses; nothing written under receipts/", want, got[:110],
               got.startswith(want) and sorted(os.listdir(os.path.join(K.EV, "receipts"))) == before)
    record("C7", "F no expectation exists for C7 (nothing can select, build or run it); exactly the four controls are "
           "derived", f"controls {K.ORDER}", sorted(exp["controls"]),
           "C7" not in exp["controls"] and sorted(exp["controls"]) == sorted(K.ORDER) and exp["blocked_not_executed"]
           == ["C7"])
    problems, _ = control.gate("C7", exp)
    record("C7", "F control.gate refuses C7", "closed: not in this mission's order", "; ".join(problems)[:120],
           any("C7 is not in this mission's order" in p and "MUTATION_CONTROL_BLOCKED" in p for p in problems))
    problems, _ = control.gate("A2", exp)
    record("A2", "F control.gate before A1's RESULT exists", "closed: the previous control's RESULT is not a candidate",
           "; ".join(problems)[:120], any(exp["controls"]["A1"]["dir"] + "/RESULT.txt" in p for p in problems))
    problems, _ = control.gate("A1", exp)
    record("A1", "F control.gate before the freeze and the pre-execution gate", "closed",
           "; ".join(problems)[:120], any("06-PRE-EXECUTION-GATE" in p for p in problems)
           and any("freeze" in p for p in problems))


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = supervise.expectations()
    print(f"T3 CONTROLS — {K.MISSION} (synthetic and historical evidence only; no Cargo, rustc, test or Nexus executable)")
    section_a(out, exp)
    section_b(out, scratch, exp)
    section_c(out, scratch, exp)
    section_d(out, scratch, exp)
    section_f(exp)
    bad = [r for r in results if not r["as_expected"]]
    lines = [f"{'AS EXPECTED' if r['as_expected'] else 'UNEXPECTED '} {r['control']:4} {r['case']}: expected "
             f"{r['expected']}; got {r['got']}{(' — ' + r['detail']) if r['detail'] else ''}" for r in results]
    open(os.path.join(out, "CONTROLS.txt"), "w").write("\n".join(lines) + "\n")
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1)
    verdict = "T3 CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED")
    open(os.path.join(out, "SUMMARY.txt"), "w").write(
        f"{len(results)} controls; {len(results) - len(bad)} as expected; {len(bad)} unexpected\n"
        + "".join(f"UNEXPECTED: {r['control']} {r['case']}: {r['got']}\n" for r in bad) + verdict + "\n")
    print(verdict)
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
