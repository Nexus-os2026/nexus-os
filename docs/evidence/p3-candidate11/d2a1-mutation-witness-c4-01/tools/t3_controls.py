#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01, mission sections 5 (steps 7-9) and 11: the required positive and negative
controls of this mission's tooling, on SYNTHETIC, HISTORICAL and AUTHENTICATED evidence only, before the freeze. No
Cargo, no rustc, no Nexus build, test or executable is run; every worktree, target and executable outside this mission
is read, never written. (Rewritten from WITNESS-C1-C25-01's t3_controls.py, whose helpers record, exe_for, base_raw,
transform, synth, first_two, l1_line, pass_report, incomplete_lines, compose, result_label and sub_all are kept
unchanged; its G.3 sections are replaced by the AD-1-R4 G.5 and G.6 controls of C4.) Synthetic evidence never becomes
acceptance evidence: every synthetic file carries the MARK line, every synthetic executable names /synthetic/, and
nothing here is staged as a receipt of any run.

    t3_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-t3-controls) receives the synthetic
                                       receipts this tool classifies or links, CONTROLS.txt, CONTROLS.json and
                                       SUMMARY.txt (last line `T3 CONTROLS: ALL AS EXPECTED` or `T3 CONTROLS: FAILED`).
                                       SCRATCH (under dev-local/, never published) holds every tampered copy of an input.

A  g5.classify_primary (G.5(A)) on synthetic type-(a) receipts (historical base: CONTINUE-02's C9 run; the real
   Stage-2 F-8 PASS report for the PASS and INCOMPLETE forms). Positive: FAIL P4 and FAIL P8, each recorded as printed
   (PRIMARY_OK, never a kill label; the primary alone establishes neither attribution); P4 exactly at the F-8 ceiling.
   Negative: another first property (E3, P1, E11: STOP FOR ARCHITECT RULING); PASS on the mutant; INCOMPLETE at the
   state and time caps; a FAIL beyond 30,000,000 (cap overshoot: INCOMPLETE — BUDGET EXCEEDED); the wrong
   original-explorer configuration (full_f1; F-8's header with F-1's bounds; the composed-quotient header); the wrong
   executable identity; killed from outside (timeout, OOM); truncated and malformed raw output; the witness test as the
   primary; the unchanged core or another control's mutant as the source; a tampered expectation admitting E3 (refused
   by the tool invariant).
W  g5.classify_witness (G.5(B), G.6, F.2) on synthetic type-(c) receipts built from the HISTORICAL C4 prototype's mutant
   transcript (evidence commit B, authenticated by its manifest; Candidate C's source blobs) by exact, counted
   replacements. Positive: P4 and P8 at their loci with G.6 holding; the same with a further property reported
   separately. Negative: P4 missing; P8 missing; P4 NOT OBSERVED; P8 NOT OBSERVED; P4 at another step; P4 in another
   scenario; P8 at another step; P4's locus world not U+X; P8's world with G1 in U; the named property inside its own
   line of other properties; a fixture failure; an unrelated panic; the healthy transcript (exit 0) as the mutant
   witness; G.6: a bad offered stream, a consumed choice not the offered prefix, a wrong option count, an unexpected
   unused suffix, consumption beyond the offered stream (an implicit Oracle-default zero), a partial consumption at the
   changed append, a wrong branch label, a wrong fault budget, a [fault] marker without a fault, the actor order
   changed, a stray line inside the witness; the wrong selection (no --exact, --ignored, two tests listed, another test);
   the baseline executable; a different mutant executable from the primary's; the unchanged core as the source; no
   witness after an unestablished primary; killed from outside (timeout, OOM); truncated output; a malformed header
   count.
X  g5.transcript and g5.g6_check directly, on the real healthy transcript (Stage 2) and the historical mutant one: each
   holds in its own role and fails in the other's; a healthy append that consumes a choice fails G.6.
H  g5.link_healthy (G.5(C), G.6) on the real authenticated healthy receipt, after a synthetic restoration receipt and a
   synthetic mutant chain: HEALTHY WITNESS LINK: VERIFIED; negative: linking before restoration, after a failed
   restoration, a changed healthy-witness raw output, a positive violation marker in the healthy receipt, a healthy
   append consuming its offered choices, the mutant executable presented as the baseline's, a mutant transcript that
   differs before the changed append, a mutant X that is not the healthy core's at P4 step 8.
B  control.link (unchanged; type (e1)) on the real Stage-2 F-8 receipt: LINK: VERIFIED; negative: linking before
   restoration, after a failed restoration, a changed raw output, another test.
R  control.restore_auth read-only on the C1/C25 mission's mtw-C1 worktree (its lock reason that mission's): positive with
   that mission's name; negative: this mission's lock reason absent, the executable changed, the restoration not from the
   mutant blob.
G  g5.obligations: (A)-(D) and (E)'s identity part hold with first property P4 and with P8, (E)'s Architect review
   pending and (F) proposed, never an acceptance; negative: the restoration missing beside the links, the witness on
   another executable, the primary printing E3, one property observed only, a G.6 problem, a failed healthy link, a
   failed unchanged-core link, a restoration not starting from the mutant blob.
C  expectations.py on tampered inputs (the catalog; C4's patch; AD-1-R4's P4 locus): PROBLEMS — STOP; on the real
   inputs: equal to the frozen receipts/02-EXPECTATIONS.json.
D  the unchanged general build classifier: the historical C24 build (BUILD: NOT OK reproduced byte for byte); its warning
   presented as C4's build: BUILD: NOT OK; the accepted A3 build: BUILD: PINNED.
F  control.main refuses C7 (blocked) by name, every previously accepted control (C1 C25 A1 A2 C20 A5 C24 C6 C2 C22
   among them), A4 and an unknown id, writing nothing; no expectation exists for C7 or any accepted control;
   control.gate refuses C7, C1 and C25, and C4 before the freeze and the pre-execution gate.
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
import g5  # noqa: E402
import supervise  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
MARK = "SYNTHETIC (t3_controls.py, P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01): not the output of any run\n"
RSS = 4421  # a distinctive synthetic resource record (this mission's own), so that no synthetic classification repeats
# an earlier packet's (TRANCHE3-EXEC-01 used 4373, TRANCHE3-CONTINUE-02 4381, TRANCHE3-TAIL-01 4397, WITNESS-C1-C25-01
# 4403); section A's case directories begin with the control id and are numbered from 101 for the same reason
HIST_B = "a464a36621523f539d2554a6b176d678c7f885ea"
HIST_MUTANT = "docs/evidence/p3-candidate11/c4-witness-01/receipts/12-run-mutant/raw-output.txt"
HIST_MUTANT_SHA = "5e8c54a49e885e170446e84ea54daf200cccb8d15fe86534a58458f6fc8fd443"
results = []


def record(cid, case, expected, got, ok, detail=""):
    results.append({"control": cid, "case": case, "expected": expected, "got": got, "as_expected": bool(ok),
                    "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {cid:4} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def exe_for(cid):
    sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (t3_controls.py; never built, never run)".encode())
    path = f"/synthetic/nexus-os-p3-c11-mtw-{cid}/target/release/deps/nexus_governed_control-0123456789abcdef"
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


# ---- WITNESS-C4-01: the G.5 and G.6 sections ----------------------------------------------------------------------

def g5_label(first, outer):
    """The RESULT label control.main gives a primary with this classification when it is not admissible: INCOMPLETE —
    ARCHITECT REVIEW REQUIRED for an INCOMPLETE primary, MUTATION_CONTROL_BLOCKED otherwise; an admissible primary
    continues to the witness (no label yet: 'continues')."""
    if first == g5.PRIMARY_OK:
        return "continues to the witness"
    if ("outer class (AD-1-R4 K.2): " + outer).startswith(control.INCOMPLETE_OUTER):
        return control.RESULT_INCOMPLETE
    return control.RESULT_BLOCKED


def fails_of(d, name="CHECKS.txt"):
    return [l for l in open(os.path.join(d, name)).read().splitlines() if l.startswith("FAIL")]


def section_a(out, exp):
    print("A. g5.classify_primary (G.5(A)) on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run; the real "
          "Stage-2 F-8 PASS report); the primary is recorded as printed and never establishes P4 and P8 by itself")
    ctl = exp["controls"]["C4"]
    OK = "FAIL — admissible type-(a) original-explorer mutant FAIL; first failure"
    BUDGET = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    RULING = "STOP FOR ARCHITECT RULING (AD-1-R4 G.5(A))"
    R_OK, R_BL, R_IN = "continues to the witness", control.RESULT_BLOCKED, control.RESULT_INCOMPLETE
    P, N = g5.PRIMARY_OK, g5.PRIMARY_NOT
    f8 = open(K.inputs("linked", "S2-64-full-F-8", "raw-output.txt"), encoding="utf-8").read()
    f1_bounds = [l[len("  bounds: "):] for l in open(K.inputs("linked", "S1-45-full-F-1", "raw-output.txt"),
                                                      encoding="utf-8").read().splitlines()
                 if l.startswith("  bounds: ")] if os.path.exists(K.inputs("linked", "S1-45-full-F-1",
                                                                                  "raw-output.txt")) else []
    other_bounds = ctl["bounds"].replace("faults 2 ", "faults 3 ")
    base_exe = {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE}
    cases = [
        ("P-first-failure-P4-recorded-as-printed", {"prop": "P4"}, P, OK + " P4 recorded as printed", R_OK, None),
        ("P-first-failure-P8-recorded-as-printed", {"prop": "P8"}, P, OK + " P8 recorded as printed", R_OK, None),
        ("P-P4-exactly-at-the-F-8-ceiling-30000000", {"prop": "P4", "states": 30000000}, P, OK, R_OK, None),
        ("N-wrong-first-property-E3", {"prop": "E3"}, N, "FAIL of E3 as the first printed property, neither P4 nor P8",
         R_BL, "G.5(A): the first printed property"),
        ("N-wrong-first-property-P1", {"prop": "P1"}, N, "FAIL of P1 as the first printed property, neither P4 nor P8",
         R_BL, "G.5(A): the first printed property"),
        ("N-wrong-first-property-E11", {"prop": "E11"}, N, "FAIL of E11 as the first printed property", R_BL,
         "G.5(A): the first printed property"),
        ("N-PASS-on-the-mutant", {"raw": "PASS"}, N, "PASS on the mutant (the control survives F-8): kills nothing", R_BL,
         "raw outcome FAIL"),
        ("N-incomplete-search (INCOMPLETE at the state cap)", {"raw": "INCOMPLETE"}, N,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing",
         R_IN, "raw outcome FAIL"),
        ("N-incomplete-search (INCOMPLETE at the time cap)", {"raw": "INCOMPLETE-TIME"}, N,
         "INCOMPLETE (the time cap of 2700s was reached. No property is established by this run.): kills nothing", R_IN,
         "raw outcome FAIL"),
        ("N-F-8-FAIL-beyond-30000000 (cap overshoot)", {"prop": "P4", "states": 30000001}, N, BUDGET, R_IN,
         "discovered count <= the ceiling"),
        ("N-wrong-original-explorer-configuration (full_f1, F-1's test)", {"prop": "P4", "test": K.TEST_PREFIX + "full_f1"},
         N, "NOT ADMISSIBLE (", R_BL, "the catalog's own test"),
        ("N-wrong-original-explorer-configuration (F-8's header with another configuration's bounds)",
         {"prop": "P4", "bounds": f1_bounds[0] if f1_bounds else other_bounds}, N, "NOT ADMISSIBLE (", R_BL,
         "printed bounds equal the configuration's"),
        ("N-wrong-explorer (the composed-quotient header for F-8)",
         {"prop": "P4", "header": "EXPLORATION F-8 (gating-caller symmetry quotient composed with the admission observer "
                                  "quotient: experimental, not the contract's explorer)"}, N, "NOT ADMISSIBLE (", R_BL,
         "header 'EXPLORATION F-8' exactly"),
        ("N-wrong-executable-identity (the receipt's executable is not the pinned C4 mutant's)", {"prop": "P4",
                                                                                                 "run_exe": base_exe},
         N, "NOT ADMISSIBLE (", R_BL, "executable is the pinned one"),
        ("N-killed-from-outside (timeout kill)", {"prop": "P4", "process": {
            "exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
            "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"}, N,
         "INCOMPLETE (killed from outside or abnormal termination): kills nothing", R_IN, "exit 101"),
        ("N-killed-from-outside (OOM)", {"prop": "P4", "process": {
            "exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
            "sup": {"oom_kill": 1}, "raw": "TRUNCATED"}, N,
         "INCOMPLETE (killed from outside or abnormal termination): kills nothing", R_IN, "launching command"),
        ("N-truncated-raw-output (no report; a normal exit 101)", {"prop": "P4", "raw": "TRUNCATED"}, N, "NOT ADMISSIBLE (",
         R_BL, "the report is printed"),
        ("N-malformed-raw-output (the assertion's operands edited)",
         {"prop": "P4", "post": lambda raw: raw.replace("  left: Fail", "  left: Fail (edited)")}, N, "NOT ADMISSIBLE (",
         R_BL, "the assertion's operands"),
        ("N-the-witness-test-run-as-the-primary", {"prop": "P4", "test": ctl["g5"]["witness"]["test"]}, N,
         "NOT ADMISSIBLE (", R_BL, "the catalog's own test"),
        ("N-incorrect-source (EXACTLY_C: the unchanged core, not the mutant)", {"prop": "P4",
                                                                                  "source_verdict": "EXACTLY_C"},
         N, "NOT ADMISSIBLE (", R_BL, "source EXACTLY_C_PLUS_PATCH"),
        ("N-incorrect-source (C1's accepted mutant blob)", {"prop": "P4",
                                                           "mutant_blob": "7d95dd425fcdf17b12dda2fdce461f97ba7b9a66"},
         N, "NOT ADMISSIBLE (", R_BL, "source EXACTLY_C_PLUS_PATCH"),
    ]
    for n, (case, kw, want_first, want_outer, want_label, want_fail) in enumerate(cases, 101):
        exe = exe_for("C4")
        kw = dict(kw)
        run_exe = kw.pop("run_exe", None)
        if kw.get("raw") in ("INCOMPLETE", "INCOMPLETE-TIME", "PASS"):
            h, ls = pass_report(f8)
            if kw["raw"] == "INCOMPLETE":
                ls = incomplete_lines(ls, 30000001, 26000000, "the state cap of 30000000 was reached")
                kw["raw"] = compose(ctl["test"], h, ls, "INCOMPLETE", ctl["verdict_assertion"])
            elif kw["raw"] == "INCOMPLETE-TIME":
                ls = incomplete_lines(ls, 21000000, 20999000, "the time cap of 2700s was reached")
                kw["raw"] = compose(ctl["test"], h, ls, "INCOMPLETE", ctl["verdict_assertion"])
            else:
                kw["raw"] = compose(ctl["test"], h, ls, "PASS", ctl["verdict_assertion"])
            kw["process"] = ({"exit_status": 101, "signal": None, "supervisor_rc": 101, "wrapper_recorded_status": "101"}
                             if kw["raw"].find("test result: FAILED") > 0 else
                             {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"})
        elif kw.get("raw") == "TRUNCATED":
            kw["raw"] = "\n".join(base_raw().replace(BASE_TEST, ctl["test"]).splitlines()[:6]) + "\n"
            kw.setdefault("process", {"exit_status": 101, "signal": None, "supervisor_rc": 101,
                                      "wrapper_recorded_status": "101"})
        d = os.path.join(out, "A", f"C4-{n:03d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}", "04-run")
        synth(d, ctl, run_exe or exe, **kw)
        g5.classify_primary(d, ctl, exe)
        first, outer = first_two(d)
        label = g5_label(first, outer)
        fl = fails_of(d)
        ok = (first == want_first and (outer.startswith(want_outer) or (want_outer in outer and len(want_outer) > 30))
              and label == want_label and (want_fail is None and not fl or want_fail is not None
                                           and any(want_fail in l for l in fl)))
        if want_outer.startswith("FAIL of") and not outer.startswith("FAIL — "):
            ok = ok and RULING in outer
        record("C4", "A " + case, f"{want_first} / {want_outer[:70]}…; {want_label}"
               + (f"; failing check '{want_fail}'" if want_fail else ""), f"{first} / {outer[:110]}; {label}", ok,
               [x[:90] for x in fl][:2])
        if case.startswith("P-first-failure"):
            prop = kw["prop"]
            other = "P8" if prop == "P4" else "P4"
            record("C4", f"A N-{prop}-relabelled-or-taken-as-both: the G.5(A) primary is never a kill label, records "
                   f"{prop} as printed and says that it does not establish P4 and P8", f"first line not "
                   f"{classify.CANDIDATE}; the printed property {prop}; 'does not establish P4 and P8'",
                   f"{first[:60]}…; {outer[-110:]}", first != classify.CANDIDATE and "KILL_EVIDENCE_CANDIDATE" not in first
                   and f"first failure {prop} recorded as printed" in outer and "does not establish P4 and P8" in outer
                   and json.load(open(os.path.join(d, "PARSED-RECEIPT.json")))["printed_property"] == prop
                   and other not in outer.split("first failure", 1)[1].split("recorded", 1)[0])
    # A tampered expectation admitting E3 as a first property: the tool invariant refuses it.
    d3 = os.path.join(out, "A", "C4-199-N-expectation-admitting-E3", "04-run")
    ctl3 = copy.deepcopy(ctl)
    ctl3["g5"]["permitted_primary"] = ["P4", "P8", "E3"]
    synth(d3, ctl3, exe_for("C4"), prop="E3")
    g5.classify_primary(d3, ctl3, exe_for("C4"))
    f3, o3 = first_two(d3)
    fl = fails_of(d3)
    record("C4", "A N-a-tampered-expectation-admitting-E3 (a property other than P4 or P8 as admissible): refused by the "
           "tool invariant", f"{g5.PRIMARY_NOT}; the invariant check fails", f"{f3}; {fl[:1]}",
           f3 == g5.PRIMARY_NOT and any("tool invariant" in l for l in fl))


WIT_RSS = 4423


def hist_mutant():
    """The HISTORICAL C4 prototype's mutant transcript (evidence commit B; Candidate C's source blobs; another
    executable): authenticated by its manifest's sha256. Only the base of synthetic tooling controls."""
    raw = K.blob(HIST_B, HIST_MUTANT)
    assert K.sha256_bytes(raw) == HIST_MUTANT_SHA, "the historical prototype transcript is not the authenticated one"
    return raw.decode("utf-8")


def healthy_text(ctl):
    return open(os.path.join(K.EV, ctl["g5"]["healthy"]["input_dir"], "raw-output.txt"), encoding="utf-8").read()


def rep_all(text, edits):
    """Counted exact replacements: each (old, new, n) must occur exactly n times."""
    for old, new, n in edits:
        assert text.count(old) == n, (old[:80], text.count(old), n)
        text = text.replace(old, new)
    return text


def panic_replaced(text, at, message):
    """The verdict panic's location and whole message replaced (a fixture failure or an unrelated panic)."""
    ls = text.split("\n")
    i = next(k for k, l in enumerate(ls) if l.startswith("thread ") and " panicked at " in l)
    j = next(k for k in range(i + 1, len(ls)) if ls[k].startswith("note: run with `RUST_BACKTRACE=1`"))
    head = re.sub(r" panicked at \S+:$", f" panicked at {at}:", ls[i])
    return "\n".join(ls[:i] + [head] + message + ls[j:])


def witness_raw(ctl, edits=(), panic=None, truncate=None, healthy=False):
    text = healthy_text(ctl) if healthy else hist_mutant()
    text = rep_all(text, edits)
    if panic:
        text = panic_replaced(text, *panic)
    if truncate:
        text = "\n".join(text.split("\n")[:truncate]) + "\n"
    return MARK + text


def synth_witness(d, ctl, exe, raw, source_verdict="EXACTLY_C_PLUS_PATCH", process=None, sup=None, argv_tail=None,
                  listed=None, ignored_listed=None, test=None):
    """A synthetic type-(c) receipt directory d (descriptor.json, raw-output.txt) for control ctl."""
    os.makedirs(d)
    w = ctl["g5"]["witness"]
    desc = json.loads(K.blob(K.T1, BASE + "/descriptor.json"))
    t = test or w["test"]
    tail = argv_tail or w["argv_tail"]
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    lst = listed if listed is not None else [f"{t}: test"]
    listing = {"exit": 0, "stdout": "".join(f"{x}\n" for x in lst) + f"\n{len(lst)} tests, 0 benchmarks\n", "stderr": "",
               "listed": lst}
    ign = ignored_listed if ignored_listed is not None else []
    desc.update({"id": "04c-witness", "test": t, "argv": [exe["path"], *tail], "control": ctl["id"],
                 "configuration": "F-8 (the witness's two fixed scenarios from initial(F8))",
                 "role": "mutant witness (type (c))", "mutant_blob": ctl["mutant_blob"],
                 "patch_sha256": ctl["patch"]["sha256"], "worktree": ctl["worktree"], "limit_seconds": w["limit_seconds"],
                 "cwd": os.path.join(ctl["worktree"], K.CRATE), "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "t3_controls.py: a SYNTHETIC mutant-witness receipt built from the historical C4 prototype "
                              "transcript; not Nexus evidence of any run",
                 "executable": {"path": exe["path"], "pinned_sha256": exe["sha256"], "pinned_size": exe["size"],
                                "before": ident, "after": ident},
                 "selection": {"listing": dict(listing, argv=[t, "--exact", "--list"]),
                               "ignored_listing": dict(listing, argv=[t, "--exact", "--ignored", "--list"], listed=ign),
                               "expected_ignored": False, "executable_after_listing": ident}})
    for when in ("source_before", "source_after"):
        s = desc[when]
        s.update({"worktree": ctl["worktree"], "verdict": source_verdict,
                  "mutant_file": dict(s["mutant_file"], path=ctl["source_file"], git_blob=ctl["mutant_blob"],
                                      sha256=ctl["mutant_sha256"], baseline_blob=ctl["baseline_blob"]),
                  "expected_mutant_blob": ctl["mutant_blob"], "expected_mutant_sha256": ctl["mutant_sha256"]})
    desc["process"] = process or {"exit_status": 101, "signal": None, "supervisor_rc": 101,
                                  "wrapper_recorded_status": "101"}
    desc["supervisor"] = dict(desc["supervisor"], max_rss_kib_largest_process=WIT_RSS, time_limit_seconds=w["limit_seconds"],
                              **(sup or {}))
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)


def primary_side(base, ctl, exe, primary_first=None):
    """The primary side a witness classification reads: 03-build/PIN.json and 04-run (descriptor with the pinned
    executable before and after; CLASSIFICATION.txt's first line)."""
    os.makedirs(os.path.join(base, "03-build"))
    json.dump(dict(exe, synthetic=MARK.strip()), open(os.path.join(base, "03-build/PIN.json"), "w"))
    os.makedirs(os.path.join(base, "04-run"))
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    json.dump({"synthetic": MARK.strip(), "executable": {"path": exe["path"], "before": ident, "after": ident}},
              open(os.path.join(base, "04-run/descriptor.json"), "w"))
    open(os.path.join(base, "04-run/CLASSIFICATION.txt"), "w").write((primary_first or g5.PRIMARY_OK) + "\n" + MARK)


P4_8 = ("C4 WITNESS F-8 P4 step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [(0, 4), (0, 4), (0, 4), (1, 3), (0, 2), "
        "(0, 3)] unused [] faults left 1: Ack(0): append_acknowledgement")
P8_12 = ("C4 WITNESS F-8 P8 step 12/12 Ack(0) offered [0, 0, 0, 0] consumed [(0, 4), (0, 4), (0, 4), (0, 3)] unused [] "
         "faults left 1: Ack(0): append_acknowledgement -> Committed [Open:Ok Recheck:Ok Insert:Ok Commit:Ok]")
HDR2 = "C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; 2 violations"


def section_w(out, exp):
    print("W. g5.classify_witness (G.5(B), G.6, F.2) on synthetic type-(c) receipts built from the historical C4 "
          "prototype's mutant transcript by exact, counted replacements")
    ctl = exp["controls"]["C4"]
    w = ctl["g5"]["witness"]
    pos = {s["named"]: s["positive"] for s in w["scenarios"]}
    OKW, NOT = g5.WITNESS_OK, g5.WITNESS_NOT
    OK_OUT = "FAIL — admissible type-(c) named-property mutant-witness receipt (G.5(B))"
    NA = "NOT ADMISSIBLE ("
    G6 = "NOT ADMISSIBLE (G.6 breached"
    G6C = "G.6 from the raw output"
    ONE = "NOT ADMISSIBLE (only "
    INC = "INCOMPLETE (killed from outside or abnormal termination): establishes nothing"
    other_exe = {"path": "/synthetic/nexus-os-p3-c11-mwc4-C4/target/release/deps/nexus_governed_control-fedcba9876543210",
                 "sha256": K.sha256_bytes(b"SYNTHETIC another C4 mutant executable (t3_controls.py)"), "size": 4381}
    base_exe = {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE}
    c1w = K.TEST_PREFIX + "c1_witness_p1_at_a_successful_admission"
    extra = [("    VIOLATION after: P4: U is occupied together with C or X\n",
              "    VIOLATION after: P4: U is occupied together with C or X\n    VIOLATION after: P13: a synthetic detail\n", 1),
             ("not counted as P4: []", 'not counted as P4: ["P13"]', 1), (HDR2, HDR2.replace("; 2 violations", "; 3 violations"), 1),
             ("  P4 step 8 after: P4: U is occupied together with C or X\n",
              "  P4 step 8 after: P4: U is occupied together with C or X\n  P4 step 8 after: P13: a synthetic detail\n", 1)]
    cases = [
        ("P-P4-and-P8-at-their-frozen-loci; G.6 holds in both scenarios", {}, {}, OKW, OK_OUT, None),
        ("P-a-further-property-reported-separately (P13 at P4 step 8, never counted as P4)", {"edits": extra}, {}, OKW,
         OK_OUT, None),
        ("N-P4-missing (only P8 shown)", {"edits": [(pos["P4"] + "\n", "", 1)]}, {}, NOT, ONE + "P8 shown",
         "the exact positive line of P4"),
        ("N-P8-missing (only P4 shown)", {"edits": [(pos["P8"] + "\n", "", 1)]}, {}, NOT, ONE + "P4 shown",
         "the exact positive line of P8"),
        ("N-P4-NOT-OBSERVED", {"edits": [("NAMED PROPERTY P4: OBSERVED", "NAMED PROPERTY P4: NOT OBSERVED", 1)]}, {}, NOT,
         "NOT ADMISSIBLE (a named property NOT OBSERVED", "the exact positive line of P4"),
        ("N-P8-NOT-OBSERVED", {"edits": [("NAMED PROPERTY P8: OBSERVED", "NAMED PROPERTY P8: NOT OBSERVED", 1)]}, {}, NOT,
         "NOT ADMISSIBLE (a named property NOT OBSERVED", "the exact positive line of P8"),
        ("N-P4-at-another-step (after step 7 of scenario P4)",
         {"edits": [("after step 8 of scenario P4 (", "after step 7 of scenario P4 (", 1)]}, {}, NOT, ONE + "P8 shown",
         "the exact positive line of P4"),
        ("N-P4-in-another-scenario (after step 8 of scenario P8)",
         {"edits": [("after step 8 of scenario P4 (", "after step 8 of scenario P8 (", 1)]}, {}, NOT, ONE + "P8 shown",
         "the exact positive line of P4"),
        ("N-P8-at-another-step (after step 11 of scenario P8)",
         {"edits": [("after step 12 of scenario P8 (", "after step 11 of scenario P8 (", 1)]}, {}, NOT, ONE + "P4 shown",
         "the exact positive line of P8"),
        ("N-P4-wrong-locus-world (U does not hold the acknowledgement at P4 step 8)",
         {"edits": [("    U Some(Acknowledgement {", "    U Some(Correction {", 1)]}, {}, NOT, NA,
         "P4's locus in the printed world after step 8"),
        ("N-P8-wrong-locus-world (G1's attempt still in U at P8 step 12)",
         {"edits": [("insert issued true, written true, told Committed false, in U false",
                     "insert issued true, written true, told Committed false, in U true", 1)]}, {}, NOT, NA,
         "P8's locus in the printed world after step 12"),
        ("N-P4-counted-in-its-own-line-of-other-properties",
         {"edits": [("not counted as P4: []", 'not counted as P4: ["P4"]', 1)]}, {}, NOT, NA,
         "the separate line of other properties of scenario P4"),
        ("N-fixture-failure (the changed append ends as neither planned end)",
         {"panic": (f"{K.MODEL}:{w['fixture_panic']['line']}:{w['fixture_panic']['col']}",
                    ["WITNESS FIXTURE FAILURE (kills nothing): scenario P4 step 8/8 (Ack(0)) ends as neither planned "
                     "end: consumed [(0, 4)], branches [], 2 faults left"])}, {}, NOT,
         "NOT ADMISSIBLE (a WITNESS FIXTURE FAILURE", "the witness's own verdict panic"),
        ("N-unrelated-panic (an unwrap on None in protocol.rs)",
         {"panic": (f"{K.PROTOCOL}:1650:21", ["called `Option::unwrap()` on a `None` value"])}, {}, NOT, NA,
         "the witness's own verdict panic"),
        ("N-the-healthy-transcript-as-the-mutant-witness (exit 0)", {"healthy": True},
         {"process": {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"}}, NOT,
         "NOT ADMISSIBLE (a healthy result in a mutant-witness receipt", "no healthy line in a mutant-witness receipt"),
        ("N-G.6-bad-offered-choice-sequence (P4 step 8 offered [0, 0, 0, 0, 0, 0])",
         {"edits": [("step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed",
                     "step  8/8 Ack(0) offered [0, 0, 0, 0, 0, 0] consumed", 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-consumed-choice-not-the-offered-prefix (P8 step 12 consumed (1, 4) where 0 is offered)",
         {"edits": [(P8_12, P8_12.replace("consumed [(0, 4), (0, 4),", "consumed [(0, 4), (1, 4),"), 1)]}, {}, NOT, G6,
         G6C),
        ("N-G.6-wrong-option-count (P4 step 6 consumed (0, 5) where the frozen count is 4)",
         {"edits": [("C4 WITNESS F-8 P4 step  6/8 Ack(0) offered [0] consumed [(0, 4)]",
                     "C4 WITNESS F-8 P4 step  6/8 Ack(0) offered [0] consumed [(0, 5)]", 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-unexpected-unused-suffix (P8 step 12 consumes 3 of 4: partial consumption at the changed append)",
         {"edits": [(P8_12, P8_12.replace("consumed [(0, 4), (0, 4), (0, 4), (0, 3)] unused []",
                                          "consumed [(0, 4), (0, 4), (0, 4)] unused [0]"), 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-implicit-Oracle-default-consumption (P4 step 8 consumes a seventh choice beyond the six offered)",
         {"edits": [(P4_8, P4_8.replace("(0, 2), (0, 3)] unused []", "(0, 2), (0, 3), (0, 2)] unused []"), 1)]}, {}, NOT,
         G6, G6C),
        ("N-G.6-wrong-branch (P8 step 11's gating branch changed)",
         {"edits": [('    branches ["append_gating: Refused Uncertain Storage"]',
                     '    branches ["append_gating: Refused NotWritten Storage"]', 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-wrong-fault-budget (P4 step 5 leaves 1 fault, not 2)",
         {"edits": [("C4 WITNESS F-8 P4 step  5/8 Wk offered [0, 0, 0, 0] consumed [(0, 3), (0, 3), (0, 2), (0, 2)] unused "
                     "[] faults left 2", "C4 WITNESS F-8 P4 step  5/8 Wk offered [0, 0, 0, 0] consumed [(0, 3), (0, 3), "
                                         "(0, 2), (0, 2)] unused [] faults left 1", 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-fault-marker-without-a-fault (P8 step 12 marked [fault], the budget unchanged)",
         {"edits": [(P8_12 + "\n", P8_12 + "   [fault]\n", 1)]}, {}, NOT, G6, G6C),
        ("N-G.6-actor-order-changed (P4 step 3 by Se, not Wk)",
         {"edits": [("C4 WITNESS F-8 P4 step  3/8 Wk offered", "C4 WITNESS F-8 P4 step  3/8 Se offered", 1)]}, {}, NOT, G6,
         G6C),
        ("N-G.6-a-stray-line-inside-the-witness",
         {"edits": [("C4 WITNESS F-8 P4 step  4/8", "    an unexpected line inside the witness\nC4 WITNESS F-8 P4 step  4/8",
                     1)]}, {}, NOT, G6, G6C),
        ("N-wrong-selection (no --exact)", {}, {"argv_tail": [w["test"], "--nocapture", "--test-threads=1"]}, NOT, NA,
         "argv"),
        ("N-wrong-selection (--ignored passed)", {}, {"argv_tail": [w["test"], "--exact", "--ignored", "--nocapture",
                                                                    "--test-threads=1"]}, NOT, NA, "argv"),
        ("N-wrong-selection (the listing shows two tests)", {}, {"listed": [w["test"] + ": test", c1w + ": test"]}, NOT, NA,
         "selection listing"),
        ("N-another-test (C1's witness test selected and listed)", {},
         {"test": c1w, "argv_tail": [c1w, "--exact", "--nocapture", "--test-threads=1"]}, NOT, NA, "argv"),
        ("N-executable-role (the mutant witness on the unchanged-baseline executable)", {}, {"exe": base_exe}, NOT, NA,
         "F.2 role"),
        ("N-different-mutant-executable-between-primary-and-witness", {}, {"exe": other_exe}, NOT, NA, "F.2 role"),
        ("N-incorrect-source (EXACTLY_C: the unchanged core)", {}, {"source_verdict": "EXACTLY_C"}, NOT, NA,
         "source EXACTLY_C_PLUS_PATCH"),
        ("N-primary-not-established (no witness after an unestablished primary)", {}, {"primary": g5.PRIMARY_NOT}, NOT, NA,
         "the primary of the same control is classified PRIMARY_OK"),
        ("N-killed-from-outside (timeout)", {"truncate": 8},
         {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
          "sup": {"timed_out": True, "wall_seconds": 2700.0}}, NOT, INC, "exit 101"),
        ("N-killed-from-outside (OOM)", {"truncate": 3},
         {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
          "sup": {"oom_kill": 1}}, NOT, INC, "launching command"),
        ("N-truncated-raw-output (cut inside scenario P8; exit 101)", {"truncate": 60}, {}, NOT, NA, G6C),
        ("N-malformed-header (the violation count disagrees)",
         {"edits": [(HDR2, HDR2.replace("; 2 violations", "; 3 violations"), 1)]}, {}, NOT, NA, "'every violation:'"),
    ]
    for n, (case, ro, so, want_first, want_outer, want_fail) in enumerate(cases, 201):
        so = dict(so)
        exe = so.pop("exe", exe_for("C4"))
        pexe = exe_for("C4")
        base = os.path.join(out, "W", f"C4-{n:03d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}")
        primary_side(base, ctl, pexe, so.pop("primary", None))
        raw = witness_raw(ctl, **ro)
        synth_witness(os.path.join(base, "04c-witness"), ctl, exe, raw, **so)
        g5.classify_witness(os.path.join(base, "04c-witness"), ctl, exe, os.path.join(base, "04-run"))
        first, outer = first_two(os.path.join(base, "04c-witness"))
        fl = fails_of(os.path.join(base, "04c-witness"))
        ok = first == want_first and outer.startswith(want_outer) and (
            want_fail is None and not fl or want_fail is not None and any(want_fail in l for l in fl))
        record("C4", "W " + case, f"{want_first} / {want_outer[:70]}…" + (f"; failing check '{want_fail}'" if want_fail
                                                                          else ""),
               f"{first} / {outer[:110]}", ok, [f[:90] for f in fl][:2])


def section_x(out, exp):
    print("X. g5.transcript and g5.g6_check directly: each real transcript holds G.6 in its own role only")
    ctl = exp["controls"]["C4"]
    w = ctl["g5"]["witness"]
    os.makedirs(os.path.join(out, "X"))
    healthy, mutant = healthy_text(ctl), hist_mutant()
    for name, text, role, want_ok in (("the real healthy transcript (Stage 2) as healthy", healthy, "healthy", True),
                                      ("the real healthy transcript as mutant", healthy, "mutant", False),
                                      ("the historical mutant transcript as mutant", mutant, "mutant", True),
                                      ("the historical mutant transcript as healthy", mutant, "healthy", False),
                                      ("a healthy transcript whose P4 append consumes one choice", rep_all(healthy, [(
                                          "step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [] unused [0, 0, 0, 1, "
                                          "0, 0]", "step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [(0, 4)] unused "
                                                   "[0, 0, 1, 0, 0]", 1)]), "healthy", False)):
        tr = g5.transcript(g5.body_lines(text, w["test"]), w["config_name"])
        probs, facts = g5.g6_check(tr, w, role)
        json.dump({"case": name, "role": role, "problems": probs, "facts": facts},
                  open(os.path.join(out, "X", re.sub(r"[^A-Za-z0-9]+", "-", name)[:60] + ".json"), "w"), indent=1)
        record("C4", f"X G.6 on {name}", "holds" if want_ok else "problems", f"{len(probs)} problems; {len(facts)} facts; "
               f"{len(tr['scenarios'])} scenarios; preamble {len(tr['preamble'])} lines", (not probs) == want_ok
               and len(tr["scenarios"]) == 2, probs[:2])


def chain(base, ctl, exe, restore_first="RESTORE: EXACTLY_C", prop="P4"):
    """A synthetic control directory with an admissible primary (first property prop), an admissible witness and
    (optionally) a restoration receipt with its record: the inputs of link_healthy and obligations."""
    primary_side(base, ctl, exe)
    os.remove(os.path.join(base, "04-run/CLASSIFICATION.txt"))
    synth(os.path.join(base, "04-run-src"), ctl, exe, prop=prop)
    for f in ("descriptor.json", "raw-output.txt"):
        shutil.copyfile(os.path.join(base, "04-run-src", f), os.path.join(base, "04-run", f))
    shutil.rmtree(os.path.join(base, "04-run-src"))
    g5.classify_primary(os.path.join(base, "04-run"), ctl, exe)
    synth_witness(os.path.join(base, "04c-witness"), ctl, exe, witness_raw(ctl))
    g5.classify_witness(os.path.join(base, "04c-witness"), ctl, exe, os.path.join(base, "04-run"))
    os.makedirs(os.path.join(base, "02-apply"))
    json.dump({"synthetic": MARK.strip(), "patch_sha256": ctl["patch"]["sha256"],
               "catalog_patch_sha256": ctl["patch"]["sha256"],
               "verify_mutant": {"verdict": "EXACTLY_C_PLUS_PATCH", "mutant_file": {
                   "git_blob": ctl["mutant_blob"], "sha256": ctl["mutant_sha256"]}}},
              open(os.path.join(base, "02-apply/APPLY.json"), "w"), indent=1)
    if restore_first:
        os.makedirs(os.path.join(base, "05-restore"))
        open(os.path.join(base, "05-restore/RESTORE.txt"), "w").write(
            f"{restore_first}\n{MARK}a synthetic restoration receipt for a tooling control of the link steps\n")
        json.dump(restore_json(ctl), open(os.path.join(base, "05-restore/RESTORE.json"), "w"), indent=1)


def restore_json(ctl):
    return {"synthetic": MARK.strip(), "applied_before_restoration": True,
            "before": {"git_blob": ctl["mutant_blob"], "sha256": ctl["mutant_sha256"]},
            "check": {"rc": 0}, "reverse": {"rc": 0},
            "after": {"git_blob": ctl["baseline_blob"], "sha256": ctl["baseline_sha256"]},
            "verify_tree": {"verdict": "EXACTLY_C", "head": K.C, "tree": K.C_TREE}}


def scratch_inputs(saved, root):
    if os.path.exists(root):
        shutil.rmtree(root)
    shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
    for a, _, fs in os.walk(root):
        for f in fs:
            os.chmod(os.path.join(a, f), 0o644)
    K.EV = root


def tamper_in(root, rel, old, new):
    p = os.path.join(root, rel)
    b = open(p, "rb").read()
    assert old in b, (rel, old)
    open(p, "wb").write(b.replace(old, new, 1))


def section_h(out, scratch, exp):
    print("H. g5.link_healthy (G.5(C), G.6, F.2) on the real authenticated healthy receipt (after a synthetic restoration "
          "receipt and a synthetic mutant chain), and negatives")
    ctl = exp["controls"]["C4"]
    base = os.path.join(out, "H", "C4-real-inputs")
    chain(base, ctl, exe_for("C4"))
    ok, ref = g5.link_healthy(ctl, os.path.join(base, "06a-healthy-witness"))
    first = supervise.first_line(os.path.join(base, "06a-healthy-witness/HEALTHY.txt"))
    record("C4", "H the accepted healthy witness linked on the real authenticated inputs, after a restoration receipt; "
           "G.6 on it and against the mutant", g5.HEALTHY_OK, first, first == g5.HEALTHY_OK and ok, ref[:90])
    root = os.path.join(scratch, "H-ev")
    saved = K.EV
    hraw = "inputs/linked/S2-52-witness-C4/raw-output.txt"
    restoration = "this control's own restoration receipt exists and records RESTORE: EXACTLY_C"
    files_ok = "the linked files (9) are exactly the accepted receipt's"

    def mutate_mutant(path, fn):
        j = json.load(open(path))
        fn(j["transcript"])
        json.dump(j, open(path, "w"), indent=1)
    neg = [
        ("N-linking-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("N-linking-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("N-changed-healthy-witness-source (raw output byte changed)",
         lambda: tamper_in(root, hraw, b"consuming 0 of its 6 offered choices", b"consuming 0 of its 7 offered choices"),
         None, "RESTORE: EXACTLY_C", files_ok),
        ("N-positive-violation-marker-in-the-healthy-receipt",
         lambda: tamper_in(root, hraw, b"    ledger after [Prepared(0), Holder(0), Prepared(1), Explanation(0), Gating(0)]"
                                       b"\nC4 WITNESS F-8 P4: 8 steps, each",
                           b"    ledger after [Prepared(0), Holder(0), Prepared(1), Explanation(0), Gating(0)]\n    "
                           b"VIOLATION after: P8: G1's attempt is written, uncorrected and not in U\nC4 WITNESS F-8 P4: 8 "
                           b"steps, each"), None, "RESTORE: EXACTLY_C", "no positive marker"),
        ("N-healthy-append-consuming-its-offered-choices (G.6 healthy end)",
         lambda: tamper_in(root, hraw, b"step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [] unused [0, 0, 0, 1, 0, 0]",
                           b"step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [(0, 4)] unused [0, 0, 1, 0, 0]"), None,
         "RESTORE: EXACTLY_C", "G.6 on the healthy transcript"),
        ("N-executable-role (the mutant executable is the unchanged-baseline executable)", None, "BASE-EXE",
         "RESTORE: EXACTLY_C", "F.2 role: the pinned unchanged-baseline executable"),
        ("N-mutant-transcript-differs-before-the-changed-append (P8 step 11's faults)", None, "PRE-CHANGE",
         "RESTORE: EXACTLY_C", "G.6 items 1 and 2"),
        ("N-mutant-X-not-the-healthy-core's-at-P4-step-8", None, "X-DIFFERS", "RESTORE: EXACTLY_C", "G.6 items 1 and 2"),
    ]
    try:
        for case, files, mod, rfirst, want in neg:
            scratch_inputs(saved, root)
            if files:
                files()
            b = os.path.join(out, "H", f"C4-{case[:60]}")
            K.EV = saved
            chain(b, ctl, exe_for("C4"), rfirst)
            K.EV = root
            wpar = os.path.join(b, "04c-witness/PARSED-RECEIPT.json")
            if mod == "BASE-EXE":  # the MARK keeps every synthetic file this mission's own (no previous packet's bytes)
                json.dump({"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE, "synthetic": MARK.strip()},
                          open(os.path.join(b, "03-build/PIN.json"), "w"))
            elif mod == "PRE-CHANGE":
                mutate_mutant(wpar, lambda t: t["scenarios"]["P8"]["steps"][10].__setitem__("faults", 2))
            elif mod == "X-DIFFERS":
                mutate_mutant(wpar, lambda t: t["scenarios"]["P4"]["steps"][7]["cells"].__setitem__("X", "None"))
            ok, ref = g5.link_healthy(ctl, os.path.join(b, "06a-healthy-witness"))
            text = open(os.path.join(b, "06a-healthy-witness/HEALTHY.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record("C4", "H " + case, f"{g5.HEALTHY_FAIL} with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == g5.HEALTHY_FAIL and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_b(out, scratch, exp):
    print("B. control.link (unchanged) on the authenticated Stage-2 F-8 receipt (after a synthetic restoration receipt), "
          "and negatives")
    ctl = exp["controls"]["C4"]

    def restore_rec(base, first="RESTORE: EXACTLY_C"):
        os.makedirs(os.path.join(base, "05-restore"))
        open(os.path.join(base, "05-restore", "RESTORE.txt"), "w").write(
            f"{first}\n{MARK}a synthetic restoration receipt for a tooling control of the link step\n")
    base = os.path.join(out, "B", "C4-real-inputs")
    restore_rec(base)
    ok, ref = control.link(ctl, exp, os.path.join(base, "06b-link"), None)
    first = supervise.first_line(os.path.join(base, "06b-link/LINK.txt"))
    record("C4", "B link (type (e1), G.5(D)) on the real authenticated Stage-2 F-8 receipt, after a restoration receipt",
           "LINK: VERIFIED", first, first == "LINK: VERIFIED" and ok, ref[:90])
    root = os.path.join(scratch, "B-ev")
    saved = K.EV
    restoration = "this control's own restoration receipt exists and records RESTORE: EXACTLY_C"
    neg = [
        ("N-linking-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("N-linking-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("N-F-8-raw-output-byte-changed", lambda: tamper_in(root, "inputs/linked/S2-64-full-F-8/raw-output.txt",
                                                            b"outcome: PASS", b"outcome: PASs"), None, "RESTORE: EXACTLY_C",
         "its classification blob and raw output identity are the catalog's"),
        ("N-another-test-in-the-expectation (the witness test as the restored side's test)", None,
         lambda c: c["argv_tail"].__setitem__(0, c["g5"]["witness"]["test"]), "RESTORE: EXACTLY_C",
         "exact configuration: the same test and flags as this control's run (F-8)"),
    ]
    try:
        for case, files, ctlmod, rfirst, want in neg:
            scratch_inputs(saved, root)
            if files:
                files()
            c = copy.deepcopy(ctl)
            if ctlmod:
                ctlmod(c)
            b = os.path.join(out, "B", f"C4-{case[:60]}")
            os.makedirs(b)
            if rfirst:
                restore_rec(b, rfirst)
            ok, ref = control.link(c, exp, os.path.join(b, "06b-link"), None)
            text = open(os.path.join(b, "06b-link/LINK.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record("C4", "B " + case, f"LINK: FAILED with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == "LINK: FAILED" and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_r(out, exp):
    print("R. control.restore_auth read-only on the C1/C25 mission's locked mtw-C1 worktree (that mission's lock reason; "
          "its pinned C1 executable), with synthetic restoration records")
    ctl = copy.deepcopy(exp["controls"]["C4"])
    ctl["id"] = "C1"  # the role names the worktree's control; every identity checked is C4's frozen one
    wt = K.ACCEPTED_WT["C1"]
    pin = json.loads(K.blob(K.W1, K.W1_PACKET + "/receipts/81-C1/03-build/PIN.json"))
    saved = K.MISSION
    cases = [("P-ownership-lock-and-executable-hold (the lock reason is that mission's own)", "WITNESS-C1-C25", None,
              None, True, None),
             ("N-this-mission's-lock-reason-absent (the worktree is not this mission's)", None, None, None, False,
              "locked for review with this mission's reason"),
             ("N-the-pinned-executable-changed (sha256 differs from 03-build/PIN.json)", "WITNESS-C1-C25",
              dict(pin, sha256="0" * 64), None, False, "the pinned C4 mutant executable is unchanged"),
             ("N-the-restoration-did-not-start-from-the-mutant-blob", "WITNESS-C1-C25", None,
              lambda r: r["before"].__setitem__("git_blob", ctl["baseline_blob"]), False,
              "the restoration started from the C4 mutant blob")]
    try:
        for n, (case, mission, p, rmod, want_ok, want) in enumerate(cases, 401):
            base = os.path.join(out, "R", f"C4-{n}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:50].strip('-')}")
            os.makedirs(os.path.join(base, "05-restore"))
            r = restore_json(ctl)
            if rmod:
                rmod(r)
            json.dump(r, open(os.path.join(base, "05-restore/RESTORE.json"), "w"), indent=1)
            K.MISSION = "P3-C11-DR01-D2A1-MUTATION-" + mission + "-01" if mission else saved
            ok = control.restore_auth(wt, ctl, os.path.join(base, "05b-restore-auth"), p or pin)
            K.MISSION = saved
            text = open(os.path.join(base, "05b-restore-auth/RESTORE-AUTH.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want and want in l]
            record("C4", "R " + case, "RESTORE AUTH: OK" if want_ok else f"RESTORE AUTH: FAILED with '{want[:60]}…'",
                   f"{text[0]}; {len(fails)} failed", ok == want_ok and (want_ok and not fails or not want_ok and hit
                                                                         and len(fails) == 1), fails[:2])
    finally:
        K.MISSION = saved


def section_g(out, exp):
    print("G. g5.obligations (G.5 (A)-(E); (F) proposed) over synthetic control directories")
    ctl = exp["controls"]["C4"]

    def full(base, prop="P4"):
        chain(base, ctl, exe_for("C4"), prop=prop)
        g5.link_healthy(ctl, os.path.join(base, "06a-healthy-witness"))
        control.link(ctl, exp, os.path.join(base, "06b-link"), None)
    for prop in ("P4", "P8"):
        base = os.path.join(out, "G", f"C4-all-obligations-first-{prop}")
        full(base, prop)
        ok = g5.obligations(ctl, os.path.join(base, "07-g5"))
        text = open(os.path.join(base, "07-g5/G5.txt")).read()
        rec = f"C4: P4, P8 (first failure {prop}; witness)"
        record("C4", f"G every obligation holds with the primary's first property {prop}: (E)'s Architect review "
               "pending; (F) proposed for Architect review, never an acceptance",
               f"{g5.G5_OK}; '(F) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): '{rec}''",
               text.splitlines()[0], ok and text.startswith(g5.G5_OK) and f"PROPOSED FOR ARCHITECT REVIEW (not an "
                                                                         f"acceptance): '{rec}'" in text
               and "(E) the Sole Architect's independent review: PENDING" in text and "ACCEPTED" not in text)

    def edit(rel, fn):
        def act(b):
            p = os.path.join(b, rel)
            j = json.load(open(p))
            fn(j)
            json.dump(j, open(p, "w"), indent=1)
        return act
    negs = [
        ("N-linking-before-restoration (links present, the restoration receipt removed)",
         lambda b: os.remove(os.path.join(b, "05-restore/RESTORE.txt")), "(E) restoration"),
        ("N-the-witness-on-another-executable (the witness descriptor names another executable)",
         edit("04c-witness/PARSED-RECEIPT.json", lambda j: j["executable"]["before"].__setitem__("sha256", "0" * 64)),
         "(B) F.2"),
        ("N-the-primary-printed-E3", edit("04-run/PARSED-RECEIPT.json", lambda j: j.__setitem__("printed_property", "E3")),
         "(A)"),
        ("N-only-P4-observed (P8 not)", edit("04c-witness/PARSED-RECEIPT.json",
                                             lambda j: j["observed"].__setitem__("P8", False)), "(B) the mutant witness"),
        ("N-a-G.6-problem-recorded", edit("04c-witness/PARSED-RECEIPT.json",
                                          lambda j: j.__setitem__("g6_problems", ["a synthetic G.6 problem"])),
         "(B) G.6 holds"),
        ("N-the-healthy-link-failed", lambda b: open(os.path.join(b, "06a-healthy-witness/HEALTHY.txt"), "w").write(
            g5.HEALTHY_FAIL + "\n" + MARK), "(C)"),
        ("N-the-unchanged-core-link-failed", lambda b: open(os.path.join(b, "06b-link/LINK.txt"), "w").write(
            "LINK: FAILED\n" + MARK), "(D)"),
        ("N-the-restoration-did-not-start-from-the-mutant-blob",
         edit("05-restore/RESTORE.json", lambda j: j["before"].__setitem__("git_blob", ctl["baseline_blob"])), "(E) identity"),
    ]
    for case, act, want in negs:
        base = os.path.join(out, "G", f"C4-{case[:60]}")
        full(base)
        act(base)
        ok = g5.obligations(ctl, os.path.join(base, "07-g5"))
        text = open(os.path.join(base, "07-g5/G5.txt")).read().splitlines()
        hit = [l for l in text if l.startswith("FAIL") and want in l]
        record("C4", "G " + case, f"{g5.G5_FAIL}; the '{want}' obligation fails; no recording", f"{text[0]}; {hit[:1]}",
               not ok and text[0] == g5.G5_FAIL and hit and "(F) no recording: G.5 is not established" in text)


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed (C4's printed-property field)", "inputs/catalog.json", b'"P4 and P8"',
             b'"P4 or P8"', "authenticated catalog copy sha256"),
            ("N-unauthorized-patch-change (C4's patch keeps the Healthy check)", "inputs/patches/C4.patch",
             b"-        if core.phase != Phase::Healthy {", b"         if core.phase != Phase::Healthy {",
             "C4 authenticated patch copy sha256"),
            ("N-AD-1-R4-copy-changed (another locus for P4)", "inputs/AD1-R4.md", b"after step 8 of scenario P4 (",
             b"after step 7 of scenario P4 (", "authenticated AD-1-R4 copy sha256")):
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
    print("D. the unchanged general build classifier: the historical C24 build; a warning on C4; an accepted warning-free "
          "build")
    C = exp["controls"]

    def copy_receipt(commit, packet_dir, name):
        d = os.path.join(scratch, "D", name)
        os.makedirs(d)
        for f in K.git("ls-tree", "--name-only", commit, packet_dir + "/").decode().split():
            open(os.path.join(d, os.path.basename(f)), "wb").write(K.blob(commit, f))
        return d
    g = copy_receipt(K.T3X, K.C24_BUILD_DIR, "C24-historical-general-classifier-rerun")
    for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"):
        os.remove(os.path.join(g, f))
    rc = classify.classify_build_success(g, "pin", "EXACTLY_C_PLUS_PATCH", K.C24_MUTANT_BLOB,
                                         expect_target=os.path.join(K.C24_WT, "target"))
    same = {f: K.sha256_file(os.path.join(g, f)) == K.sha256_bytes(K.blob(K.T3X, f"{K.C24_BUILD_DIR}/{f}"))
            for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json")}
    first = supervise.first_line(os.path.join(g, "CLASSIFICATION.txt"))
    record("C24", "D the general classifier (classify_build_success, unchanged) rerun on a copy of the historical C24 "
           "build receipt (C24 is Architect-accepted; its historical build is the warning-bearing base here)",
           "BUILD: NOT OK; CHECKS.txt, CLASSIFICATION.txt, PARSED-RECEIPT.json byte-identical to the historical ones; no "
           "PIN.json", f"{first}; rc {rc}; identical {same}; PIN.json {os.path.exists(os.path.join(g, 'PIN.json'))}",
           first == "BUILD: NOT OK" and rc == 1 and all(same.values()) and not os.path.exists(os.path.join(g, "PIN.json")))
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
    print("F. control.main refusals (C7 blocked, by name; every previously accepted control; unknown) and control.gate "
          "(read-only)")
    before = sorted(os.listdir(os.path.join(K.EV, "receipts")))
    for cid, want in [("C7", "REFUSED: C7 is MUTATION_CONTROL_BLOCKED by the Architect and expressly excluded")] + [
            (c, f"REFUSED: {c} is not executed") for c in ("C1", "C25", "A1", "A2", "C20", "A5", "C24", "C6", "C2", "C22",
                                                           "C5", "C9", "A4", "X9")]:
        try:
            control.main(cid)
            got = "NOT REFUSED"
        except SystemExit as e:
            got = str(e)
        record(cid, "F control.main refuses; nothing written under receipts/", want, got[:110],
               got.startswith(want) and sorted(os.listdir(os.path.join(K.EV, "receipts"))) == before)
    record("—", "F no expectation exists for C7 or any of the 33 accepted controls (nothing can select, build or run "
           "them); exactly C4 is derived", f"controls {K.ORDER}", sorted(exp["controls"]),
           not ({"C7"} | set(K.ACCEPTED)) & set(exp["controls"]) and sorted(exp["controls"]) == ["C4"]
           and exp["blocked_not_executed"] == ["C7"] and exp["not_authorized"] == [] and len(K.ACCEPTED) == 33)
    for cid in ("C7", "C1", "C25"):
        problems, _ = control.gate(cid, exp)
        record(cid, f"F control.gate refuses {cid}", "closed: not in this mission's order", "; ".join(problems)[:120],
               any(f"{cid} is not in this mission's order" in p for p in problems))
    problems, _ = control.gate("C4", exp)
    record("C4", "F control.gate before the freeze and the pre-execution gate", "closed",
           "; ".join(problems)[:120], any("06-PRE-EXECUTION-GATE" in p for p in problems)
           and any("freeze" in p for p in problems))


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = supervise.expectations()
    print(f"T3 CONTROLS — {K.MISSION} (synthetic, historical and authenticated evidence only; no Cargo, rustc, test or "
          "Nexus executable)")
    section_a(out, exp)
    section_w(out, exp)
    section_x(out, exp)
    section_h(out, scratch, exp)
    section_b(out, scratch, exp)
    section_r(out, exp)
    section_g(out, exp)
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
