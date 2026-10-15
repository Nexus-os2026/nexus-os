#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01, mission sections 3 (steps 6 and 7) and 7: the required positive and
negative controls of this mission's tooling, on SYNTHETIC, HISTORICAL and AUTHENTICATED evidence only, before the freeze.
No Cargo, no rustc, no Nexus build, test or executable is run; every worktree, target and executable outside this
mission is read, never written. (Rewritten from TRANCHE3-TAIL-01's t3_controls.py, whose helpers record, base_raw,
transform, synth, first_two, l1_line, pass_report, incomplete_lines, compose and sub_all are kept unchanged; its
sections A-D and F are replaced or retargeted for the AD-1-R4 G.3 controls C1 and C25.)

    t3_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-t3-controls) receives the synthetic
                                       receipts this tool classifies or links, CONTROLS.txt, CONTROLS.json and
                                       SUMMARY.txt (last line `T3 CONTROLS: ALL AS EXPECTED` or `T3 CONTROLS: FAILED`).
                                       SCRATCH (under dev-local/, never published) holds every tampered copy of an input.

A  g3.classify_primary (G.3(a)) on synthetic type-(a) receipts. Positive: C1 FAIL E3 and C25 FAIL E4, recorded as
   printed (PRIMARY_OK, never a kill label); E3 exactly at the F-1 ceiling. Beside each positive: the unchanged
   classify_a on the same receipt refuses E3 as P1 (E4 as P7), the primary's outer class says the first failure is not
   counted as the named property, and a tampered expectation predicting the named property is refused by the tool
   invariant (mission section 7: E3/E4 incorrectly counted as P1/P7). Negative: the named property printed first (P1,
   P7), another first property (P4; E3 for C25): ARCHITECT RULING REQUIRED (MUTATION_CONTROL_BLOCKED); a cap overshoot
   (INCOMPLETE — BUDGET EXCEEDED); INCOMPLETE at the state and time caps; killed from outside (timeout, OOM); truncated
   and malformed raw output; another test, the witness test as the primary; the unchanged core or another control's
   mutant as the source; a PASS on the mutant.
W  g3.classify_witness (G.3(b), (c); F.2) on synthetic type-(c) receipts built from the accepted healthy receipts' real
   step records. Positive: C1 (P1 at step 11, admission: admitted; E3 separate), C25 (P7 after step 1, Committed
   (inserted); E4 separate), C1 with a further property reported separately. Negative: a missing positive marker; NOT
   OBSERVED (C1, C25); the marker at another step or locus; the wrong branch; the wrong witness property; E3 not reported
   separately; E4 attributed as P7; a fixture failure; an unrelated panic; a healthy marker in the mutant witness; the
   wrong selection (no --exact, --ignored, two tests listed, another witness test); the wrong executable role (the
   baseline executable; another mutant executable than the primary's); the wrong source; no witness after an
   unestablished primary; killed from outside (timeout, OOM); truncated output; a malformed header count; another
   schedule.
H  g3.link_healthy (G.3(d)) on the real authenticated healthy receipts: HEALTHY WITNESS LINK: VERIFIED; negative: linking
   before restoration, after a failed restoration, a changed raw output or classification, a positive marker in the
   healthy receipt, the other control's healthy receipt, the mutant executable presented as the baseline's, another
   schedule than the mutant witness's.
B  control.link (unchanged; type (e1)) on the real Stage-1 F-1 receipt: LINK: VERIFIED; negative: linking before
   restoration, after a failed restoration, a changed raw output, another test.
G  g3.obligations: every obligation holds (the (e) recording proposed for Architect review, never an acceptance);
   negative: the restoration receipt missing beside the links (linking before restoration), the witness on another
   executable, the primary printing P1, a failed healthy link, a failed unchanged-core link.
C  expectations.py on tampered inputs (the catalog; C1's patch; C25's narrow patch made compound; AD-1-R4): PROBLEMS —
   STOP; on the real inputs: equal to the frozen receipts/02-EXPECTATIONS.json.
D  the unchanged general build classifier: the historical C24 build (BUILD: NOT OK reproduced byte for byte); its
   warning presented as C1's and C25's builds: BUILD: NOT OK; the accepted A3 build: BUILD: PINNED.
F  control.main refuses C7 (blocked) and C4 (reserved) by name, the accepted controls (A1 A2 C20 A5 C24 C6 C2 C22), A4
   and an unknown id, writing nothing; no expectation exists for C4 or C7; control.gate refuses C4, C7, C25 before C1's
   RESULT, and C1 before the freeze and the pre-execution gate.
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
import g3  # noqa: E402
import supervise  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
MARK = "SYNTHETIC (t3_controls.py, P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01): not the output of any run\n"
RSS = 4403  # a distinctive synthetic resource record (this mission's own), so that no synthetic classification repeats
# an earlier packet's (TRANCHE3-EXEC-01 used 4373, TRANCHE3-CONTINUE-02 4381, TRANCHE3-TAIL-01 4397); section A's case directories begin with
# the control id and are numbered from 101 for the same reason (the tranche-3 rules record their own receipt path)
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


# ---- WITNESS-C1-C25-01: the G.3 sections ------------------------------------------------------------------------------

def g3_label(first, outer):
    """The RESULT label control.main gives a primary with this classification when it is not admissible: INCOMPLETE —
    ARCHITECT REVIEW REQUIRED for an INCOMPLETE primary, MUTATION_CONTROL_BLOCKED otherwise; an admissible primary
    continues to the witness (no label yet: 'continues')."""
    if first == g3.PRIMARY_OK:
        return "continues to the witness"
    if ("outer class (AD-1-R4 K.2): " + outer).startswith(control.INCOMPLETE_OUTER):
        return control.RESULT_INCOMPLETE
    return control.RESULT_BLOCKED


def section_a(out, exp):
    print("A. g3.classify_primary (G.3(a)) on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run); the "
          "unchanged classify_a beside it never counts E3 as P1 or E4 as P7")
    C = exp["controls"]
    OK = "FAIL — admissible type-(a) original-explorer mutant FAIL; first failure"
    BUDGET = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    R_OK, R_BL, R_IN = "continues to the witness", control.RESULT_BLOCKED, control.RESULT_INCOMPLETE
    P, N = g3.PRIMARY_OK, g3.PRIMARY_NOT
    cases = [
        ("C1", "P-first-failure-E3-recorded-as-printed", {"prop": "E3"}, P, OK + " E3 recorded as printed", R_OK),
        ("C25", "P-first-failure-E4-recorded-as-printed", {"prop": "E4"}, P, OK + " E4 recorded as printed", R_OK),
        ("C1", "P-E3-exactly-at-the-F-1-ceiling-60000000", {"prop": "E3", "states": 60000000}, P, OK, R_OK),
        ("C1", "N-wrong-first-property-P1-(the-named-property-printed-first)", {"prop": "P1"}, N,
         "FAIL of P1 as the first printed property, not the G.3(a) prediction E3", R_BL),
        ("C25", "N-wrong-first-property-P7-(the-named-property-printed-first)", {"prop": "P7"}, N,
         "FAIL of P7 as the first printed property, not the G.3(a) prediction E4", R_BL),
        ("C1", "N-wrong-first-property-P4", {"prop": "P4"}, N, "FAIL of P4 as the first printed property", R_BL),
        ("C25", "N-wrong-first-property-E3 (C1's prediction, not C25's)", {"prop": "E3"}, N,
         "FAIL of E3 as the first printed property, not the G.3(a) prediction E4", R_BL),
        ("C1", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"prop": "E3", "states": 60000001}, N, BUDGET, R_IN),
        ("C25", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"prop": "E4", "states": 60000001}, N, BUDGET, R_IN),
        ("C1", "N-incomplete-search (INCOMPLETE at the state cap)", {"raw": "INCOMPLETE"}, N,
         "INCOMPLETE (the state cap of 60000000 was reached. No property is established by this run.): kills nothing",
         R_IN),
        ("C25", "N-incomplete-search (INCOMPLETE at the time cap)", {"raw": "INCOMPLETE-TIME"}, N,
         "INCOMPLETE (the time cap of 2700s was reached. No property is established by this run.): kills nothing", R_IN),
        ("C1", "N-killed-from-outside (timeout kill)", {"prop": "E3", "process": {
            "exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
            "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"}, N,
         "INCOMPLETE (killed from outside or abnormal termination): kills nothing", R_IN),
        ("C25", "N-killed-from-outside (OOM)", {"prop": "E4", "process": {
            "exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
            "sup": {"oom_kill": 1}, "raw": "TRUNCATED"}, N,
         "INCOMPLETE (killed from outside or abnormal termination): kills nothing", R_IN),
        ("C1", "N-truncated-raw-output (no report; a normal exit 101)", {"prop": "E3", "raw": "TRUNCATED"}, N,
         "NOT ADMISSIBLE (", R_BL),
        ("C25", "N-malformed-raw-output (the assertion's operands edited)",
         {"prop": "E4", "post": lambda raw: raw.replace("  left: Fail", "  left: Fail (edited)")}, N, "NOT ADMISSIBLE (",
         R_BL),
        ("C1", "N-another-test (full_f2)", {"prop": "E3", "test": K.TEST_PREFIX + "full_f2"}, N, "NOT ADMISSIBLE (", R_BL),
        ("C25", "N-the-witness-test-run-as-the-primary", {"prop": "E4", "test": C["C25"]["g3"]["witness"]["test"]}, N,
         "NOT ADMISSIBLE (", R_BL),
        ("C1", "N-incorrect-source (EXACTLY_C: the unchanged core, not the mutant)", {"prop": "E3",
                                                                                       "source_verdict": "EXACTLY_C"},
         N, "NOT ADMISSIBLE (", R_BL),
        ("C25", "N-incorrect-source (another control's mutant blob)", {"prop": "E4",
                                                                       "mutant_blob": C["C1"]["mutant_blob"]},
         N, "NOT ADMISSIBLE (", R_BL),
        ("C1", "N-PASS-on-the-mutant", {"raw": "PASS"}, N, "PASS on the mutant (the control survives F-1): kills nothing",
         R_BL),
    ]
    f1 = open(K.inputs("linked", "S1-45-full-F-1", "raw-output.txt"), encoding="utf-8").read()
    for n, (cid, case, kw, want_first, want_outer, want_label) in enumerate(cases, 101):
        ctl = C[cid]
        exe = exe_for(cid)
        kw = dict(kw)
        if kw.get("raw") in ("INCOMPLETE", "INCOMPLETE-TIME", "PASS"):
            h, ls = pass_report(f1)
            if kw["raw"] == "INCOMPLETE":
                ls = incomplete_lines(ls, 60000001, 52000000, "the state cap of 60000000 was reached")
                kw["raw"] = compose(ctl["test"], h, ls, "INCOMPLETE", ctl["verdict_assertion"])
            elif kw["raw"] == "INCOMPLETE-TIME":
                ls = incomplete_lines(ls, 41000000, 40999000, "the time cap of 2700s was reached")
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
        d = os.path.join(out, "A", f"{cid}-{n:03d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}", "04-run")
        synth(d, ctl, exe, **kw)
        g3.classify_primary(d, ctl, exe)
        first, outer = first_two(d)
        label = g3_label(first, outer)
        ok = (first == want_first and (outer.startswith(want_outer) or (want_outer in outer and len(want_outer) > 30))
              and label == want_label)
        record(cid, "A " + case, f"{want_first} / {want_outer[:70]}…; {want_label}", f"{first} / {outer[:110]}; {label}",
               ok)
        if case.startswith("P-first-failure"):
            # The unchanged F.1(a) classifier on a copy of the same receipt: E3 (E4) is never a kill of P1 (P7).
            d2 = os.path.join(out, "A", f"{cid}-{n:03d}-the-unchanged-classify_a-on-the-same-receipt", "04-run")
            os.makedirs(d2)
            for f in ("descriptor.json", "raw-output.txt"):
                shutil.copyfile(os.path.join(d, f), os.path.join(d2, f))
            classify.classify_a(d2, ctl, exe)
            f2, o2 = first_two(d2)
            want2 = f"FAIL of {kw['prop']}, not the named property {ctl['named_property']}: kills nothing"
            record(cid, f"A N-{kw['prop']}-counted-as-{ctl['named_property']}: the unchanged classify_a refuses it "
                   "(no kill of the named property from the primary)", f"{classify.NOT_CANDIDATE} / {want2}…",
                   f"{f2} / {o2[:100]}", f2 == classify.NOT_CANDIDATE and o2.startswith(want2))
            record(cid, f"A N-{kw['prop']}-counted-as-{ctl['named_property']}: the G.3(a) primary is never a kill label "
                   "and says so", f"first line not {classify.CANDIDATE}; 'is not counted as {ctl['named_property']}'",
                   f"{first[:60]}…; {outer[-120:]}", first != classify.CANDIDATE and "KILL_EVIDENCE_CANDIDATE" not in first
                   and f"{kw['prop']} is not counted as {ctl['named_property']}" in outer)
            # A tampered expectation that makes the named property the prediction: the tool invariant refuses it.
            d3 = os.path.join(out, "A", f"{cid}-{n:03d}-N-expectation-predicting-the-named-property", "04-run")
            ctl3 = copy.deepcopy(ctl)
            ctl3["g3"]["predicted"] = ctl["named_property"]
            synth(d3, ctl3, exe, prop=ctl["named_property"])
            g3.classify_primary(d3, ctl3, exe)
            f3, o3 = first_two(d3)
            fails = [l for l in open(os.path.join(d3, "CHECKS.txt")).read().splitlines() if l.startswith("FAIL")]
            record(cid, f"A N-a-tampered-expectation-predicting-{ctl['named_property']} (counting the named property as "
                   "the primary): refused by the tool invariant", f"{g3.PRIMARY_NOT}; the invariant check fails",
                   f"{f3}; {fails[:1]}", f3 == g3.PRIMARY_NOT and any("tool invariant" in l for l in fails))


WIT_RSS = 4409


def witness_raw(ctl, kind="mutant", **o):
    """A SYNTHETIC mutant-witness transcript in the executable's own form, built from the REAL accepted healthy receipt's
    step records (authenticated input): the changed step's branch, the violations, run_witness's own panic message at
    its location, 1 failed. Options change exactly one thing (each negative control)."""
    w, cid = ctl["g3"]["witness"], ctl["id"]
    healthy = open(os.path.join(K.EV, ctl["g3"]["healthy"]["input_dir"], "raw-output.txt"), encoding="utf-8").read()
    if kind == "healthy":
        return MARK + healthy
    body = g3.body_lines(healthy, w["test"])
    rx = g3.step_re(w)
    out, step = [], 0
    vio = o.get("violations") or ([(4, "step", "E3", "a synthetic detail"), (11, "step", "P1", "a synthetic detail")]
                                  if cid == "C1" else [(1, "step", "E4", "a synthetic detail"),
                                                       (1, "after", "P7", "a synthetic detail")])
    for line in body:
        if line.startswith(f"{cid} WITNESS {w['config_name']}: ") or line in ("ok",) or line.startswith("test result:"):
            break
        m = rx.match(line)
        if not m and not step:
            continue  # the harness's own lines before the first step record
        if m:
            step = int(m.group(1))
            if o.get("choices_step") == step:
                line = line.replace(f"choices {m.group(4)}", "choices [9]")
            out.append(line)
            continue
        if line.startswith("    branches ") and step == w["at"]:
            line = line.replace(f'["{w["healthy"]}"]', f'["{o.get("branch", w["changed"])}"]')
        out.append(line)
        if line.startswith("    ledger after ") and step:
            out += [f"    VIOLATION {pl}: {p}: {dt}" for s, pl, p, dt in vio if s == step]
    lines_all = [f"step {s} {pl}: {p}: {dt}" for s, pl, p, dt in vio]
    n = o.get("n", len(lines_all))
    others = sorted({p for _, _, p, _ in vio if p != ctl["named_property"]})
    positive = o.get("positive", w["positive"])
    msg = [f"{w['header_prefix']}{n} violations"] + ([positive] if positive is not None else []) + [
        o.get("others_line", f"{w['others_prefix']}{others!r}".replace("'", '"')), "every violation:"] + [
        "  " + x for x in lines_all]
    pl = o.get("panic_at", f"{w['panic']['file']}:{w['panic']['line']}:{w['panic']['col']}")
    msg_text = "\n".join(o.get("message", msg))
    text = "\n".join(out)
    test = o.get("test", w["test"])
    if o.get("truncate"):
        return MARK + f"\nrunning 1 test\ntest {test} ... " + "\n".join(out[:o["truncate"]]) + "\n"
    return (MARK + f"\nrunning 1 test\ntest {test} ... {text}\n\nthread '{test}' ({WIT_RSS}) panicked at {pl}:\n{msg_text}\n"
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\nFAILED\n\nfailures:\n\n"
            f"failures:\n    {test}\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} "
            "filtered out; finished in 0.00s\n\n")


def synth_witness(d, ctl, exe, raw, source_verdict="EXACTLY_C_PLUS_PATCH", process=None, sup=None, argv_tail=None,
                  listed=None, ignored_listed=None, test=None):
    """A synthetic type-(c) receipt directory d (descriptor.json, raw-output.txt) for control ctl."""
    os.makedirs(d)
    w = ctl["g3"]["witness"]
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
                 "configuration": "F-1 (the witness's fixed schedule)", "role": "mutant witness (type (c))",
                 "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"], "worktree": ctl["worktree"],
                 "limit_seconds": w["limit_seconds"], "cwd": os.path.join(ctl["worktree"], K.CRATE),
                 "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "t3_controls.py: a SYNTHETIC mutant-witness receipt built from the accepted healthy "
                              "receipt's step records; not Nexus evidence of any run",
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
    open(os.path.join(base, "04-run/CLASSIFICATION.txt"), "w").write((primary_first or g3.PRIMARY_OK) + "\n" + MARK)


def section_w(out, exp):
    print("W. g3.classify_witness (G.3(b), (c); F.2) on synthetic type-(c) receipts built from the accepted healthy "
          "receipts' step records")
    C = exp["controls"]
    OKW, NOT = g3.WITNESS_OK, g3.WITNESS_NOT
    OK_OUT = "FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b))"
    NA = "NOT ADMISSIBLE ("
    INC = "INCOMPLETE (killed from outside or abnormal termination): establishes nothing"
    W1, W25 = C["C1"]["g3"]["witness"], C["C25"]["g3"]["witness"]
    other_exe = {"path": "/synthetic/nexus-os-p3-c11-mtw-C1/target/release/deps/nexus_governed_control-fedcba9876543210",
                 "sha256": K.sha256_bytes(b"SYNTHETIC another mutant executable (t3_controls.py)"), "size": 4381}
    base_exe = {"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE}
    cases = [
        ("C1", "P-positive-line-at-step-11-with-admission-admitted; E3 reported separately", {}, {}, OKW, OK_OUT),
        ("C25", "P-positive-line-on-the-world-after-step-1-with-Committed-(inserted); E4 reported separately", {}, {}, OKW,
         OK_OUT),
        ("C1", "P-E3-and-a-further-property-reported-separately (P15 too)",
         {"violations": [(4, "step", "E3", "a synthetic detail"), (11, "step", "P15", "a synthetic detail"),
                         (11, "step", "P1", "a synthetic detail")]}, {}, OKW, OK_OUT),
        ("C1", "N-missing-positive-marker (no NAMED PROPERTY line)", {"positive": None}, {}, NOT, NA),
        ("C1", "N-NOT-OBSERVED", {"positive": W1["positive"].replace("OBSERVED", "NOT OBSERVED")}, {}, NOT,
         "NOT ADMISSIBLE (NAMED PROPERTY P1: NOT OBSERVED"),
        ("C25", "N-NOT-OBSERVED", {"positive": W25["positive"].replace("OBSERVED", "NOT OBSERVED")}, {}, NOT,
         "NOT ADMISSIBLE (NAMED PROPERTY P7: NOT OBSERVED"),
        ("C1", "N-marker-at-another-step (step 10)", {"positive": W1["positive"].replace("step 11", "step 10")}, {}, NOT,
         NA),
        ("C25", "N-marker-at-another-locus (at step 1, not on the world after it)",
         {"positive": W25["positive"].replace("on the world after", "at"),
          "violations": [(1, "step", "E4", "a synthetic detail"), (1, "step", "P7", "a synthetic detail")]}, {}, NOT, NA),
        ("C1", "N-wrong-admission-branch (step 11 shows 'admission: refused')", {"branch": "admission: refused"}, {}, NOT,
         NA),
        ("C25", "N-wrong-branch (step 1 shows 'append_gating: Committed (absent)')",
         {"branch": "append_gating: Committed (absent)"}, {}, NOT, NA),
        ("C1", "N-wrong-witness-property (the named property's violation at step 11 is P12, not P1)",
         {"violations": [(4, "step", "E3", "a synthetic detail"), (11, "step", "P12", "a synthetic detail")]}, {}, NOT, NA),
        ("C1", "N-E3-not-reported-separately (the line of other properties lacks E3)",
         {"others_line": W1["others_prefix"] + '["P15"]'}, {}, NOT, NA),
        ("C25", "N-E4-attributed-as-P7 (the line of other properties names P7)",
         {"others_line": W25["others_prefix"] + '["E4", "P7"]'}, {}, NOT, NA),
        ("C1", "N-fixture-failure (a step not enabled)",
         {"message": ["WITNESS FIXTURE FAILURE (kills nothing): step 5 (Fw(1)) is not enabled"],
          "panic_at": f"{K.MODEL}:8328:9"}, {}, NOT, "NOT ADMISSIBLE (a WITNESS FIXTURE FAILURE"),
        ("C25", "N-unrelated-panic (index out of bounds in protocol.rs)",
         {"message": ["index out of bounds: the len is 2 but the index is 7"], "panic_at": f"{K.PROTOCOL}:1480:21"}, {},
         NOT, NA),
        ("C1", "N-healthy-marker-in-the-mutant-witness (the healthy transcript, exit 0)", "HEALTHY",
         {"process": {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"}}, NOT,
         "NOT ADMISSIBLE (a healthy result in a mutant-witness receipt"),
        ("C1", "N-wrong-selection (no --exact)", {}, {"argv_tail": [W1["test"], "--nocapture", "--test-threads=1"]}, NOT,
         NA),
        ("C25", "N-wrong-selection (--ignored passed)", {}, {"argv_tail": [W25["test"], "--exact", "--ignored",
                                                                          "--nocapture", "--test-threads=1"]}, NOT, NA),
        ("C1", "N-wrong-selection (the listing shows two tests)", {},
         {"listed": [W1["test"] + ": test", W25["test"] + ": test"]}, NOT, NA),
        ("C1", "N-another-witness-test (C25's witness run for C1)", {"test": W25["test"]}, {"test": W25["test"]}, NOT, NA),
        ("C1", "N-executable-role (the mutant witness on the unchanged-baseline executable)", {}, {"exe": base_exe}, NOT,
         NA),
        ("C25", "N-executable-role (another mutant executable than the primary's)", {}, {"exe": other_exe}, NOT, NA),
        ("C1", "N-incorrect-source (EXACTLY_C: the unchanged core)", {}, {"source_verdict": "EXACTLY_C"}, NOT, NA),
        ("C25", "N-primary-not-established (no witness after an unestablished primary)", {}, {"primary": g3.PRIMARY_NOT},
         NOT, NA),
        ("C1", "N-killed-from-outside (timeout)", {"truncate": 6},
         {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
          "sup": {"timed_out": True, "wall_seconds": 2700.0}}, NOT, INC),
        ("C25", "N-killed-from-outside (OOM)", {"truncate": 1},
         {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9, "wrapper_recorded_status": "137"},
          "sup": {"oom_kill": 1}}, NOT, INC),
        ("C1", "N-truncated-raw-output (cut after step 6; exit 101)", {"truncate": 18}, {}, NOT, NA),
        ("C1", "N-malformed-header (the violation count disagrees)", {"n": 5}, {}, NOT, NA),
        ("C1", "N-schedule-mismatch (step 2 took other choices)", {"choices_step": 2}, {}, NOT, NA),
    ]
    for n, (cid, case, ro, so, want_first, want_outer) in enumerate(cases, 201):
        ctl = C[cid]
        so = dict(so)
        exe = so.pop("exe", exe_for(cid))
        pexe = exe_for(cid)
        base = os.path.join(out, "W", f"{cid}-{n:03d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}")
        primary_side(base, ctl, pexe, so.pop("primary", None))
        raw = witness_raw(ctl, "healthy") if ro == "HEALTHY" else witness_raw(ctl, **ro)
        synth_witness(os.path.join(base, "04c-witness"), ctl, exe, raw, **so)
        g3.classify_witness(os.path.join(base, "04c-witness"), ctl, exe, os.path.join(base, "04-run"))
        first, outer = first_two(os.path.join(base, "04c-witness"))
        fails = [l for l in open(os.path.join(base, "04c-witness/CHECKS.txt")).read().splitlines() if l.startswith("FAIL")]
        record(cid, "W " + case, f"{want_first} / {want_outer[:70]}…", f"{first} / {outer[:110]}",
               first == want_first and outer.startswith(want_outer), [f[:90] for f in fails][:2])


def chain(base, ctl, exe, restore_first="RESTORE: EXACTLY_C"):
    """A synthetic control directory with an admissible primary, an admissible witness and (optionally) a restoration
    receipt: the inputs of link_healthy and obligations."""
    primary_side(base, ctl, exe)
    os.remove(os.path.join(base, "04-run/CLASSIFICATION.txt"))
    synth(os.path.join(base, "04-run-src"), ctl, exe, prop=ctl["g3"]["predicted"])
    for f in ("descriptor.json", "raw-output.txt"):
        shutil.copyfile(os.path.join(base, "04-run-src", f), os.path.join(base, "04-run", f))
    shutil.rmtree(os.path.join(base, "04-run-src"))
    g3.classify_primary(os.path.join(base, "04-run"), ctl, exe)
    synth_witness(os.path.join(base, "04c-witness"), ctl, exe, witness_raw(ctl))
    g3.classify_witness(os.path.join(base, "04c-witness"), ctl, exe, os.path.join(base, "04-run"))
    if restore_first:
        os.makedirs(os.path.join(base, "05-restore"))
        open(os.path.join(base, "05-restore/RESTORE.txt"), "w").write(
            f"{restore_first}\n{MARK}a synthetic restoration receipt for a tooling control of the link steps\n")


def section_h(out, scratch, exp):
    print("H. g3.link_healthy (G.3(d), F.2) on the real authenticated healthy receipts (after a synthetic restoration "
          "receipt), and negatives")
    C = exp["controls"]
    for cid in K.ORDER:
        base = os.path.join(out, "H", f"{cid}-real-inputs")
        chain(base, C[cid], exe_for(cid))
        ok, ref = g3.link_healthy(C[cid], os.path.join(base, "06a-healthy-witness"))
        first = supervise.first_line(os.path.join(base, "06a-healthy-witness/HEALTHY.txt"))
        record(cid, "H the accepted healthy witness linked on the real authenticated inputs, after a restoration receipt",
               g3.HEALTHY_OK, first, first == g3.HEALTHY_OK and ok, ref[:90])
    root = os.path.join(scratch, "H-ev")
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
    files_ok = "the linked files (9) are exactly the accepted receipt's"
    neg = [
        ("C1", "N-linking-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("C25", "N-linking-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("C1", "N-healthy-raw-output-byte-changed", lambda: tamper("inputs/linked/S2-50-witness-C1/raw-output.txt",
                                                                 b"admission: refused'", b"admission: refuseD'"), None,
         "RESTORE: EXACTLY_C", files_ok),
        ("C25", "N-healthy-witness-containing-a-positive-marker",
         lambda: tamper("inputs/linked/S2-51-witness-C25/raw-output.txt", b"\nok\n",
                        b"\nNAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))"
                        b"\nok\n"), None, "RESTORE: EXACTLY_C", files_ok),
        ("C1", "N-healthy-classification-byte-changed", lambda: tamper("inputs/linked/S2-50-witness-C1/CLASSIFICATION.txt",
                                                                     b"CLASSIFICATION: PASS", b"CLASSIFICATION: PASs"),
         None, "RESTORE: EXACTLY_C", files_ok),
        ("C1", "N-the-C25-healthy-receipt-linked-for-C1 (another witness test)", None,
         lambda c: c["g3"].__setitem__("healthy", dict(C["C25"]["g3"]["healthy"])), "RESTORE: EXACTLY_C",
         "the same witness test alone"),
        ("C25", "N-executable-role (the mutant executable is the unchanged-baseline executable)", None, "BASE-EXE",
         "RESTORE: EXACTLY_C", "F.2 role: the pinned unchanged-baseline executable"),
        ("C1", "N-the-mutant-witness-ran-another-schedule (the healthy receipt's schedule differs)", None, "OTHER-SCHEDULE",
         "RESTORE: EXACTLY_C", "the same schedule as this control's own mutant witness"),
    ]
    try:
        for cid, case, files, ctlmod, rfirst, want in neg:
            fresh()
            if files:
                files()
            ctl = copy.deepcopy(C[cid])
            if callable(ctlmod):
                ctlmod(ctl)
            base = os.path.join(out, "H", f"{cid}-{case[:60]}")
            K.EV = saved
            chain(base, C[cid], exe_for(cid), rfirst)
            K.EV = root
            if ctlmod == "BASE-EXE":
                json.dump({"path": K.EXE, "sha256": K.EXE_SHA256, "size": K.EXE_SIZE},
                          open(os.path.join(base, "03-build/PIN.json"), "w"))
            if ctlmod == "OTHER-SCHEDULE":
                p = os.path.join(base, "04c-witness/PARSED-RECEIPT.json")
                j = json.load(open(p))
                j["step_records"][1]["choices"] = "[0]"
                json.dump(j, open(p, "w"), indent=1)
            ok, ref = g3.link_healthy(ctl, os.path.join(base, "06a-healthy-witness"))
            text = open(os.path.join(base, "06a-healthy-witness/HEALTHY.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record(cid, "H " + case, f"{g3.HEALTHY_FAIL} with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == g3.HEALTHY_FAIL and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_b(out, scratch, exp):
    print("B. control.link (unchanged) on the authenticated Stage-1 F-1 receipt (after a synthetic restoration receipt), "
          "and negatives")
    C = exp["controls"]

    def restore_rec(base, first="RESTORE: EXACTLY_C"):
        os.makedirs(os.path.join(base, "05-restore"))
        open(os.path.join(base, "05-restore", "RESTORE.txt"), "w").write(
            f"{first}\n{MARK}a synthetic restoration receipt for a tooling control of the link step\n")
    for cid in K.ORDER:
        base = os.path.join(out, "B", f"{cid}-real-inputs")
        restore_rec(base)
        ok, ref = control.link(C[cid], exp, os.path.join(base, "06b-link"), None)
        first = supervise.first_line(os.path.join(base, "06b-link/LINK.txt"))
        record(cid, "B link (type (e1)) on the real authenticated inputs, after a restoration receipt", "LINK: VERIFIED",
               first, first == "LINK: VERIFIED" and ok, ref[:90])
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
        ("C1", "N-linking-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("C25", "N-linking-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("C1", "N-F-1-raw-output-byte-changed", lambda: tamper("inputs/linked/S1-45-full-F-1/raw-output.txt",
                                                            b"outcome: PASS", b"outcome: PASs"), None, "RESTORE: EXACTLY_C",
         "its classification blob and raw output identity are the catalog's"),
        ("C25", "N-another-test-in-the-expectation (the witness test as the restored side's test)", None,
         lambda c: c["argv_tail"].__setitem__(0, c["g3"]["witness"]["test"]), "RESTORE: EXACTLY_C",
         "exact configuration: the same test and flags as this control's run (F-1)"),
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
            ok, ref = control.link(ctl, exp, os.path.join(base, "06b-link"), None)
            text = open(os.path.join(base, "06b-link/LINK.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record(cid, "B " + case, f"LINK: FAILED with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == "LINK: FAILED" and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_g(out, exp):
    print("G. g3.obligations (G.3 (a)-(e)) over synthetic control directories")
    C = exp["controls"]

    def full(base, ctl):
        chain(base, ctl, exe_for(ctl["id"]))
        g3.link_healthy(ctl, os.path.join(base, "06a-healthy-witness"))
        control.link(ctl, exp, os.path.join(base, "06b-link"), None)
    for cid in K.ORDER:
        base = os.path.join(out, "G", f"{cid}-all-obligations")
        full(base, C[cid])
        ok = g3.obligations(C[cid], os.path.join(base, "07-g3"))
        text = open(os.path.join(base, "07-g3/G3.txt")).read()
        record(cid, "G every obligation holds: the (e) recording is proposed for Architect review, never an acceptance",
               f"{g3.G3_OK}; '(e) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): "
               f"'{C[cid]['g3']['recording']}''", text.splitlines()[0],
               ok and text.startswith(g3.G3_OK) and f"PROPOSED FOR ARCHITECT REVIEW (not an acceptance): "
                                                    f"'{C[cid]['g3']['recording']}'" in text and "ACCEPTED" not in text)
    negs = [
        ("C1", "N-linking-before-restoration (links present, the restoration receipt removed)",
         lambda b: os.remove(os.path.join(b, "05-restore/RESTORE.txt")), "restoration"),
        ("C25", "N-the-witness-on-another-executable (the witness descriptor names another executable)",
         lambda b: (lambda p: json.dump(dict(json.load(open(p)), executable=dict(json.load(open(p))["executable"], before=dict(
             json.load(open(p))["executable"]["before"], sha256="0" * 64))), open(p, "w")))(
             os.path.join(b, "04c-witness/PARSED-RECEIPT.json")), "(b) F.2"),
        ("C1", "N-the-primary-printed-P1 (E3 counted as P1 would be needed)",
         lambda b: (lambda p: json.dump(dict(json.load(open(p)), printed_property="P1"), open(p, "w")))(
             os.path.join(b, "04-run/PARSED-RECEIPT.json")), "(a)"),
        ("C25", "N-the-healthy-link-failed", lambda b: open(os.path.join(b, "06a-healthy-witness/HEALTHY.txt"), "w").write(
            g3.HEALTHY_FAIL + "\n"), "(d)"),
        ("C1", "N-the-unchanged-core-link-failed", lambda b: open(os.path.join(b, "06b-link/LINK.txt"), "w").write(
            "LINK: FAILED\n"), "unchanged-core type-(e1)"),
    ]
    for cid, case, act, want in negs:
        base = os.path.join(out, "G", f"{cid}-{case[:60]}")
        full(base, C[cid])
        act(base)
        ok = g3.obligations(C[cid], os.path.join(base, "07-g3"))
        text = open(os.path.join(base, "07-g3/G3.txt")).read().splitlines()
        hit = [l for l in text if l.startswith("FAIL") and want in l]
        record(cid, "G " + case, f"{g3.G3_FAIL}; the '{want}' obligation fails; no recording", f"{text[0]}; {hit[:1]}",
               not ok and text[0] == g3.G3_FAIL and hit and "(e) no recording: G.3 is not established" in text)


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed", "inputs/catalog.json", b'"P13 or P12"', b'"P13 or P15"',
             "authenticated catalog copy sha256"),
            ("N-unauthorized-patch-change (C1's patch tests another field)", "inputs/patches/C1.patch",
             b"if shift == UNAPPLIED_SHIFT {", b"if shift == LIVE_SHIFT {", "C1 authenticated patch copy sha256"),
            ("N-unauthorized-patch-change (C25's narrow patch made compound: self.keep = true removed)",
             "inputs/patches/C25.patch", b"\n                     self.keep = true;\n",
             b"\n-                    self.keep = true;\n", "C25 authenticated patch copy sha256"),
            ("N-AD-1-R4-copy-changed (another locus for P1)", "inputs/AD1-R4.md", b"OBSERVED at step 11", b"OBSERVED at step 10",
             "authenticated AD-1-R4 copy sha256")):
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
    print("D. the unchanged general build classifier: the historical C24 build; warnings on C1 and C25; an accepted "
          "warning-free build")
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
    print("F. control.main refusals (C7 blocked and C4 reserved, by name; the accepted; unknown) and control.gate "
          "(read-only)")
    before = sorted(os.listdir(os.path.join(K.EV, "receipts")))
    for cid, want in (("C7", "REFUSED: C7 is MUTATION_CONTROL_BLOCKED by the Architect and expressly excluded"),
                      ("C4", "REFUSED: C4 is reserved and not authorized"),
                      ("A1", "REFUSED: A1 is not executed"), ("A2", "REFUSED: A2 is not executed"),
                      ("C20", "REFUSED: C20 is not executed"), ("A5", "REFUSED: A5 is not executed"),
                      ("C24", "REFUSED: C24 is not executed"), ("C6", "REFUSED: C6 is not executed"),
                      ("C2", "REFUSED: C2 is not executed"), ("C22", "REFUSED: C22 is not executed"),
                      ("A4", "REFUSED: A4 is not executed"), ("X9", "REFUSED: X9 is not executed")):
        try:
            control.main(cid)
            got = "NOT REFUSED"
        except SystemExit as e:
            got = str(e)
        record(cid, "F control.main refuses; nothing written under receipts/", want, got[:110],
               got.startswith(want) and sorted(os.listdir(os.path.join(K.EV, "receipts"))) == before)
    record("—", "F no expectation exists for C4 or C7 (nothing can select, build or run either); exactly C1 and C25 are "
           "derived", f"controls {K.ORDER}", sorted(exp["controls"]),
           not {"C4", "C7"} & set(exp["controls"]) and sorted(exp["controls"]) == sorted(K.ORDER)
           and exp["blocked_not_executed"] == ["C7"] and exp["not_authorized"] == ["C4"])
    for cid in ("C7", "C4"):
        problems, _ = control.gate(cid, exp)
        record(cid, f"F control.gate refuses {cid}", "closed: not in this mission's order", "; ".join(problems)[:120],
               any(f"{cid} is not in this mission's order" in p for p in problems))
    problems, _ = control.gate("C25", exp)
    record("C25", "F control.gate before C1's RESULT exists (C25 only after a C1 candidate)",
           "closed: the previous control's RESULT is not a candidate", "; ".join(problems)[:120],
           any(exp["controls"]["C1"]["dir"] + "/RESULT.txt" in p for p in problems))
    problems, _ = control.gate("C1", exp)
    record("C1", "F control.gate before the freeze and the pre-execution gate", "closed",
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
    section_h(out, scratch, exp)
    section_b(out, scratch, exp)
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
