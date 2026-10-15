#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01, mission sections 4, 5, 7, 8 and 11: the rules of this tranche that the
unchanged run classifier (classify.classify_a, byte-identical to the accepted tooling's) does not state on its own.
Nothing here builds or runs anything; it reads receipt directories and writes its own records.

  rules(outdir, rdir, ctl)    after classify_a classified the kill run in rdir: the tranche-3 rules of mission
                              sections 4, 7 and 8 for that control, written to outdir/RULES.txt and RULES.json (first line
                              `TRANCHE-3 RULES: HOLD` or `TRANCHE-3 RULES: NOT SATISFIED`). A control is a kill evidence
                              candidate only when classify_a says so AND these rules hold. The rules never admit anything
                              classify_a rejected: they only add requirements.
    every control   the run classified by classify_a; the original explorer (header exactly `EXPLORATION <cfg>`, no
                    quotient or symmetry line, the catalog's own test, never a gc_symmetry_* quotient test); the
                    printed property one the catalog permits for this control (C3b: P13 or P12; C3c: P12 only; every
                    other control its one property); the discovered count at most the control's ceiling (60,000,000
                    for F-1, 30,000,000 otherwise)
    C6, C7          A4 ON: the run is full_fpt with the bounds token ` A4 true;` (F-PT as defined), the expectation
                    requires A4, and the quotient is forbidden for the kill run
    C20, A5         L1 only after a COMPLETE search (AD-1-R4 Q): raw FAIL of L1 with the explorer's own L1 detail
                    (`<k> states lie on or behind a cycle of non-fault steps; one is reached by`, 1 <= k <= states); no
                    INCOMPLETE line; every discovered state expanded (expanded = discovered: the search loop ended by
                    exhausting the frontier, the only way explore() reaches its L1 evaluation); discovered at most
                    30,000,000; elapsed below the time cap; the expectation's L1 rule present
  classify_info(d, info, exe, source_verdict, mutant_blob)
                              C7's informational run without A4 on F-3 (mission section 5), in d: CHECKS.txt,
                              PARSED-RECEIPT.json and CLASSIFICATION.txt, whose first line is `INFORMATIONAL OUTCOME:
                              FAIL <p>`, `INFORMATIONAL OUTCOME: PASS`, `INFORMATIONAL OUTCOME: INCOMPLETE (<reason>)` or
                              `INFORMATIONAL RUN: NOT ADMISSIBLE — STOP (<failed checks>)`; never a kill label. An
                              admissible informational outcome needs: the same pinned C7 mutant executable and the C7
                              mutant source before and after; exactly the full_f3 selection; complete supervision (16 GiB,
                              zero swap, no OOM, no signal, no timeout, within 2700 s); the original explorer on F-3 with
                              A4 OFF and caps 30,000,000 / 2700 s; a complete report; and one of the three permitted forms:
                              FAIL with discovered <= 30,000,000 (a FAIL beyond the cap is a cap overshoot: not admissible,
                              stop); PASS of a complete search (every property line, expanded = discovered <= the cap);
                              INCOMPLETE stopped by the explorer's own state or time ceiling (expanded <= the cap; every
                              property NOT ESTABLISHED). Anything else stops the mission.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import report  # noqa: E402

RULES_OK = "TRANCHE-3 RULES: HOLD"
RULES_FAIL = "TRANCHE-3 RULES: NOT SATISFIED"
INFO_PREFIX = "INFORMATIONAL OUTCOME: "
INFO_STOP = "INFORMATIONAL RUN: NOT ADMISSIBLE — STOP"
L1_DETAIL = re.compile(r"^L1: (\d+) states lie on or behind a cycle of non-fault steps; one is reached by$")
NOT_ESTABLISHED = "NOT ESTABLISHED (incomplete)"


def _checks():
    return classify.Checks()


def rules(outdir, rdir, ctl):
    """The tranche-3 rules for the kill run in rdir (already classified by classify_a). Returns True iff they hold."""
    os.makedirs(outdir)
    ck = _checks()
    desc = json.load(open(os.path.join(rdir, "descriptor.json")))
    cls = open(os.path.join(rdir, "CLASSIFICATION.txt")).read().splitlines() if os.path.exists(
        os.path.join(rdir, "CLASSIFICATION.txt")) else []
    ck("the kill run is classified by the unchanged classify_a (its CLASSIFICATION.txt exists)", bool(cls), cls[:1])
    text = None
    if "not_started" not in desc and desc.get("output_path"):
        raw = open(os.path.join(rdir, desc["output_path"]), "rb").read()
        text = raw.decode("utf-8", errors="replace")
        ck("raw output sha256 equals the launcher's record", K.sha256_bytes(raw) == desc.get("raw_output_sha256"))
    rep = report.parse_report(text) if text else None
    cfg, cap, limit = ctl["configuration"], ctl["ceiling_states"], ctl["limit_seconds"]
    allowed = ctl.get("permitted_properties", [ctl["named_property"]])
    ck(f"the catalog's own test {ctl['test']} on the original explorer (never a gc_symmetry_* quotient test)",
       desc.get("test") == ctl["test"] and desc.get("argv", [None])[1:] == ctl["argv_tail"]
       and "gc_symmetry" not in ctl["test"] and ctl["runner"] == "full_exploration", desc.get("argv", [])[1:2])
    prop = None
    if rep is not None:
        ck(f"header exactly 'EXPLORATION {cfg}': no quotient or symmetry line (the original explorer)",
           rep["header"] == f"EXPLORATION {cfg}" and "quotient_line" not in rep and "symmetry_line" not in rep
           and "quotient" not in rep["header"] and "symmetry" not in rep["header"], rep["header"])
        prop = (rep["fail"] or "").split(": ", 1)[0] if rep["fail"] else None
        ck(f"the printed first property is permitted for {ctl['id']}: {' or '.join(allowed)}"
           + (" (no other property, P13 alone included)" if allowed == ["P12"] else ""), prop in allowed, rep["fail"])
        ck(f"discovered count at most the ceiling {cap:,} ({cfg})", rep.get("states") is not None
           and rep["states"] <= cap and rep.get("cap") == cap, f"{rep.get('states')} of {rep.get('cap')}")
        ck("printed bounds are the expectation's (the frozen configuration)", rep.get("bounds") == ctl["bounds"],
           rep.get("bounds"))
    else:
        ck("an exploration report is printed", False, "no report")
    if ctl["id"] in K.A4_ON:
        a4 = ctl.get("a4") or {}
        ck("A4 ON (F-PT as defined): the expectation requires A4 and forbids the quotient for the kill run",
           a4.get("required") is True and a4.get("quotient_forbidden") is True and cfg == "F-PT", a4)
        ck("A4 ON in the run: the printed bounds carry ' A4 true;' and the test is full_fpt",
           rep is not None and " A4 true;" in (rep.get("bounds") or "") and " A4 false;" not in (rep.get("bounds") or "")
           and ctl["test"] == K.TEST_PREFIX + "full_fpt", (rep or {}).get("bounds", "")[-120:])
    if ctl["id"] in K.L1_CONTROLS:
        lr = ctl.get("l1_rule") or {}
        ck("the expectation carries the L1 complete-search rule (AD-1-R4 Q)", lr.get("complete_search_required") is True,
           lr.get("rule"))
        ok_rep = rep is not None
        ck("L1: raw outcome FAIL of L1, with the explorer's own L1 detail",
           ok_rep and rep.get("outcome") == "FAIL" and prop == "L1" and L1_DETAIL.match(rep["fail"] or "") is not None,
           (rep or {}).get("fail"))
        ck("L1: no INCOMPLETE line (no state or time ceiling ended the search)", ok_rep and rep["incomplete"] is None,
           (rep or {}).get("incomplete"))
        have = ok_rep and all(k in rep for k in ("states", "expanded", "transitions", "fault_edges", "no_step"))
        ck("L1: complete search: every discovered state was expanded (expanded = discovered; explore() evaluates L1 "
           "only after its search loop exhausted the frontier)", have and rep["expanded"] == rep["states"],
           f"states {(rep or {}).get('states')}, expanded {(rep or {}).get('expanded')}")
        ck(f"L1: discovered = expanded at most {cap:,}", have and rep["states"] <= cap, (rep or {}).get("states"))
        m = L1_DETAIL.match((rep or {}).get("fail") or "")
        ck("L1: the states on or behind a cycle are between 1 and the discovered count",
           m is not None and have and 1 <= int(m.group(1)) <= rep["states"], m.group(1) if m else None)
        ck(f"L1: elapsed below the {limit} s time ceiling", ok_rep and rep.get("elapsed_s") is not None
           and rep["elapsed_s"] < limit, (rep or {}).get("elapsed_s"))
        ck("L1: no property line (a FAIL prints only its failing property; L1 is not inferred from a prefix)",
           ok_rep and rep["properties"] == [], (rep or {}).get("properties", [])[:2])
    good = ck.ok()
    lines = [RULES_OK if good else RULES_FAIL, f"control {ctl['id']} ({cfg}, {ctl['test']}); kill run {os.path.relpath(rdir, K.EV)}",
             f"classify_a: {cls[0] if cls else 'NOT CLASSIFIED'}",
             "these rules only add requirements to classify_a; a control is a candidate only when both hold"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + d) if d else ''}" for l, o, d in ck.items]
    open(os.path.join(outdir, "RULES.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "holds": good, "checks": ck.items, "run": os.path.relpath(rdir, K.EV),
               "printed_property": prop, "report": {k: (rep or {}).get(k) for k in (
                   "header", "outcome", "fail", "incomplete", "states", "expanded", "transitions", "elapsed_s", "cap")}},
              open(os.path.join(outdir, "RULES.json"), "w"), indent=1, default=str)
    print("\n".join(lines))
    return good


def classify_info(d, info, exe, source_verdict="EXACTLY_C_PLUS_PATCH", mutant_blob=None):
    """C7's informational F-3 run (mission section 5). Returns (admissible, outcome text)."""
    classify.refuse_twice(d)
    ck = _checks()
    desc, text = classify.load(d)
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        classify.write(d, f"{INFO_STOP} (not started)", "informational run not started: the mission stops", ck,
                       {"control": "C7", "informational": True})
        return False, "not started"
    proc, sup = desc["process"], desc["supervisor"]
    cap, limit, test = info["ceiling_states"], info["limit_seconds"], info["test"]
    classify.check_identity(ck, desc, exe, source_verdict, mutant_blob)
    classify.check_selection(ck, desc, exe, test, info["argv_tail"], info["ignored"])
    starts, running, result, failed_names = classify.harness(text, test)
    ck("the harness ran exactly that one test", running == ["1"] and starts == [test], f"running {running}; {starts}")
    classify.check_supervision(ck, sup, limit)
    ck("no signal; the process exited normally", proc["signal"] is None and proc["exit_status"] is not None,
       f"exit {proc['exit_status']}, signal {proc['signal']}")
    ck("no fixture failure; no quotient or symmetry refusal", "WITNESS FIXTURE FAILURE" not in text
       and "inapplicable" not in text)
    rep = report.parse_report(text)
    outcome = rep.get("outcome") if rep else None
    lines, heads, anyp = classify.panics(text)
    msg = classify.panic_message(lines, heads[0][0]) if len(heads) == 1 else []
    ck("an exploration report is printed", rep is not None)
    prop = None
    if rep is not None:
        ck("header exactly 'EXPLORATION F-3' (the original explorer; no quotient or symmetry line)",
           rep["header"] == "EXPLORATION F-3" and "quotient_line" not in rep and "symmetry_line" not in rep,
           rep["header"])
        ck("printed bounds are F-3's: A4 OFF", rep.get("bounds") == info["bounds"] and " A4 false;" in rep.get(
            "bounds", ""), rep.get("bounds"))
        ck("printed actors are F-3's", rep.get("actors") == info["actors"], rep.get("actors"))
        ck(f"caps printed: {cap:,} states, {limit} s", rep.get("cap") == cap and rep.get("time_cap_s") == limit,
           f"{rep.get('cap')}, {rep.get('time_cap_s')}")
        have = all(k in rep for k in ("states", "expanded", "transitions", "fault_edges", "no_step"))
        ck("counts printed", have)
        props, branches = report.model_constants()
        labels = [lab for _, lab in rep["branches"]]
        ck("the report is complete: branch list (every BRANCHES label, in order) and the key-fields line",
           labels == branches and rep["key_fields"] is not None, f"{len(labels)} of {len(branches)}")
        ck("the raw outcome is one of FAIL, PASS, INCOMPLETE", outcome in ("FAIL", "PASS", "INCOMPLETE"), outcome)
        va = info["verdict_assertion"]
        if outcome == "PASS":
            want = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
            ck("PASS: harness ok, exit 0, no panic", proc["exit_status"] == 0 and len(result) == 1
               and result[0].startswith(want) and not heads and not anyp, result)
            ck("PASS: no INCOMPLETE or FAIL line; every discovered state expanded, within the cap (a complete search)",
               rep["incomplete"] is None and rep["fail"] is None and have and rep["expanded"] == rep["states"] <= cap,
               f"{rep.get('states')} / {rep.get('expanded')}")
            ck("PASS: one line for each of the 25 PROPERTIES, in order",
               [k for k, _ in rep["properties"]] == props, len(rep["properties"]))
        elif outcome in ("FAIL", "INCOMPLETE"):
            want = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; "
            ck("harness FAILED with exactly that test; exit 101 by a normal exit", proc["exit_status"] == 101
               and len(result) == 1 and result[0].startswith(want) and failed_names == [test], f"{result}; {failed_names}")
            ck(f"exactly one panic, the explorer's own verdict assertion ({va['file']}:{va['line']}:{va['col']})",
               len(heads) == 1 and len(anyp) == 1 and heads[0][1].group(1) == test
               and (heads[0][1].group(2), int(heads[0][1].group(3)), int(heads[0][1].group(4))) == (
                   va["file"], va["line"], va["col"]), [l for l in anyp][:2])
            ck("the report is printed, then repeated verbatim in the assertion message",
               text.count("EXPLORATION ") == 2 and "\n".join(msg[1:-3]) == "\n".join([rep["header"]] + rep["lines"]))
            if outcome == "FAIL":
                prop = (rep["fail"] or "").split(": ", 1)[0] if rep["fail"] else None
                ck("FAIL: the assertion's operands: left Fail, right Pass; the message names the printed property",
                   msg[-2:] == ["  left: Fail", " right: Pass"]
                   and msg[:1] == [f'assertion `left == right` failed: failed: {{"{prop}"}}'], msg[:1] + msg[-2:])
                ck("FAIL: one of PROPERTIES; no INCOMPLETE line; the shortest trace printed", prop in props
                   and rep["incomplete"] is None and re.fullmatch(r"  shortest trace \((\d+) steps\):",
                                                                  rep["trace_steps"] or "") is not None, rep["fail"])
                ck(f"FAIL: discovered at most {cap:,} (a FAIL beyond the cap is a cap overshoot: not admissible)",
                   have and rep["states"] <= cap, rep.get("states"))
            else:
                reasons = (f"the state cap of {cap} was reached. No property is established by this run.",
                           f"the time cap of {limit}s was reached. No property is established by this run.")
                ck("INCOMPLETE: the assertion's operands: left Incomplete, right Pass; nothing failed",
                   msg[-2:] == ["  left: Incomplete", " right: Pass"]
                   and msg[:1] == ["assertion `left == right` failed: failed: {}"], msg[:1] + msg[-2:])
                ck("INCOMPLETE: stopped by the explorer's own state or time ceiling (its INCOMPLETE line), no FAIL line",
                   rep["incomplete"] in reasons and rep["fail"] is None, rep["incomplete"])
                ck(f"INCOMPLETE: expanded at most {cap:,}; at the state ceiling the explorer stops as soon as the "
                   "discovered count exceeds it (its stop rule), never later",
                   have and rep["expanded"] <= cap and rep["expanded"] < rep["states"]
                   and (rep["states"] > cap) == (rep["incomplete"] == reasons[0]),
                   f"discovered {rep.get('states')}, expanded {rep.get('expanded')}")
                ck("INCOMPLETE: every property NOT ESTABLISHED (no property is claimed)",
                   [k for k, _ in rep["properties"]] == props
                   and all(v == NOT_ESTABLISHED for _, v in rep["properties"]), rep["properties"][:2])
    good = ck.ok()
    if good and outcome == "FAIL":
        first = f"{INFO_PREFIX}FAIL {prop}"
    elif good and outcome == "PASS":
        first = f"{INFO_PREFIX}PASS"
    elif good and outcome == "INCOMPLETE":
        first = f"{INFO_PREFIX}INCOMPLETE ({rep['incomplete'].split('.')[0]})"
    else:
        first = f"{INFO_STOP} ({'; '.join(ck.failed())[:300]})"
    outer = ("informational (mission section 5): C7 without A4 on F-3, the original explorer; NOT the C7 kill receipt "
             "and never counted as a kill" + ("" if good else "; not admissible: the mission stops"))
    extra = []
    if rep:
        extra.append(f"raw outcome (as printed): outcome {rep.get('outcome')}; FAIL {rep.get('fail')}; INCOMPLETE "
                     f"{rep.get('incomplete')}; harness {result}")
        if "states" in rep:
            extra.append(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}) of the {cap:,} ceiling; "
                         f"transitions {rep.get('transitions')}; elapsed {rep.get('elapsed_s')} s")
    extra.append(f"configuration F-3 (A4 off); test {test}; same pinned C7 mutant executable {exe['sha256']}")
    extra.append("kill label: none (informational)")
    extra.append(classify.resources_line(sup, limit))
    classify.write(d, first, outer, ck,
                   {"control": "C7", "informational": True, "receipt_type": "informational (mission section 5)",
                    "required_to_kill": False, "test": test, "configuration": "F-3", "a4": False,
                    "ceiling_states": cap, "time_limit_seconds": limit, "admissible": good, "outcome": outcome,
                    "report": {k: rep.get(k) for k in ("header", "outcome", "bounds", "actors", "states", "expanded",
                                                       "transitions", "fault_edges", "no_step", "elapsed_s", "cap",
                                                       "time_cap_s", "fail", "incomplete", "trace_steps")} if rep else None,
                    "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
                    "selection": desc["selection"], "raw_output_sha256": desc["raw_output_sha256"]}, extra)
    return good, first
