#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01, mission sections 4-8: the AD-1-R4 G.3 kill criterion, for C1 and C25
only (AD-1-R4 F.1 types (a), (c), (e3); F.2; G.3; G.4; K.1; K.2). Nothing here builds or runs anything: it reads
receipt directories and the authenticated input copies and writes its own records. Every function keeps the raw outcome
(as printed) apart from the outer class, and fails closed.

  classify_primary(d, ctl, exe)         G.3(a): the type-(a) primary, full_f1 on the original explorer on the pinned
                                        mutant executable. Every check of the unchanged classify_a, with G.3(a)'s
                                        attribution in place of F.1(a)'s: the first printed property is recorded as
                                        printed and must be the predicted first failure (E3 for C1, E4 for C25); it is
                                        never counted as the named property (P1, P7). Another first property is reported
                                        as printed and stops the mission for an Architect ruling. First line of
                                        CLASSIFICATION.txt: PRIMARY_OK (a part only; never a kill by itself) or
                                        PRIMARY_NOT.
  classify_witness(d, ctl, exe, primary_dir)
                                        G.3(b) and (c): the type-(c) mutant witness, the control's witness test alone
                                        with --exact (no --ignored) on the SAME pinned mutant executable as the primary
                                        (F.2). Required: the witness's own verdict panic (run_witness's panic!), 1
                                        failed, exit 101 by a normal exit; the header with the schedule's step count; the
                                        exact positive line at its locus (G.4 item 9: P1 in step 11's own violations
                                        with 'admission: admitted'; P7 on the world after step 1 with 'append_gating:
                                        Committed (inserted)'); E3 (E4) in the line of other properties, not counted as
                                        P1 (P7); every scheduled step printed with its frozen actor, choices and fixed
                                        branches. Rejected (G.3(c)): NOT OBSERVED, another step or locus, a fixture
                                        failure, an unrelated panic, a healthy line, another selection or executable, a
                                        compilation failure (no executable), incomplete output or abnormal termination.
                                        First line: WITNESS_OK (a part only) or WITNESS_NOT.
  link_healthy(ctl, outdir)             G.3(d): the accepted Stage-2 healthy type-(e3) receipt, linked (never rerun) only
                                        after this control's own restoration receipt records RESTORE: EXACTLY_C;
                                        authenticated again here (every file against Stage 2's SHA256SUMS; the catalog's
                                        identities; the pinned unchanged-baseline executable, hashed now; the exact
                                        healthy line, no positive marker; the same frozen schedule as this control's
                                        mutant witness). First line HEALTHY_OK or HEALTHY_FAIL.
  obligations(ctl, outdir)              G.3 (a), (b), (d), none of (c), the restoration and the type-(e1) link, read from
                                        this control's own receipts; then (e)'s recording, PROPOSED FOR ARCHITECT REVIEW
                                        (never an acceptance). First line G3_OK or G3_FAIL.

The expectations (ctl["g3"]) are derived by expectations.py from the frozen catalog, AD-1-R4 and Candidate C's source,
and cross-checked against the mission's own statement (common.G3).
"""
import ast
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import report  # noqa: E402
import supervise  # noqa: E402

PRIMARY_OK = ("CLASSIFICATION: G.3(a) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL "
              "BY ITSELF)")
PRIMARY_NOT = "CLASSIFICATION: G.3(a) PRIMARY — NOT ESTABLISHED"
WITNESS_OK = "CLASSIFICATION: G.3(b) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT (A PART ONLY)"
WITNESS_NOT = "CLASSIFICATION: G.3(b) MUTANT WITNESS — NOT ADMISSIBLE"
HEALTHY_OK = "HEALTHY WITNESS LINK: VERIFIED"
HEALTHY_FAIL = "HEALTHY WITNESS LINK: FAILED"
G3_OK = "G.3: ALL OBLIGATIONS HOLD"
G3_FAIL = "G.3: NOT ESTABLISHED"
RULING_REQUIRED = "ARCHITECT RULING REQUIRED (AD-1-R4 G.3(a))"
POSITIVE_MARKERS = ("NAMED PROPERTY", "OBSERVED", "VIOLATION", "WITNESS FIXTURE FAILURE", "OTHER PROPERTIES VIOLATED",
                    "every violation:", "panicked")
HEALTHY_FILES = 9  # the Stage-2 healthy receipt directory: its files as Stage 2's SHA256SUMS lists them


def step_re(w):
    return re.compile(rf"^{re.escape(w['control'])} WITNESS {re.escape(w['config_name'])} step +(\d+)/(\d+) (\S+) "
                      r"choices (\[[^\]]*\]): (.*)$")


BRANCHES = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) \(live (\d+), unapplied (\d+)\)$")
VIOLATION = re.compile(r"^    VIOLATION (step|after): (\w+): (.*)$")


def body_lines(text, test):
    """The output's lines, with the test's first printed line split off the harness's `test <name> ... ` line."""
    return text.replace(f"test {test} ... ", f"test {test} ... \n", 1).splitlines()


def step_records(lines, w):
    """Every printed step record of the witness: {n, total, actor, choices, branches, word, violations: [(place, p)]}."""
    rx, out = step_re(w), []
    for i, line in enumerate(lines):
        m = rx.match(line)
        if not m:
            continue
        b = BRANCHES.match(lines[i + 1]) if i + 1 < len(lines) else None
        vio = []
        for nxt in lines[i + 1:]:
            if rx.match(nxt) or not nxt.startswith("    "):
                break
            v = VIOLATION.match(nxt)
            if v:
                vio.append((v.group(1), v.group(2)))
        try:
            branches = ast.literal_eval(b.group(1)) if b else None
        except (ValueError, SyntaxError):
            branches = None
        out.append({"n": int(m.group(1)), "total": int(m.group(2)), "actor": m.group(3), "choices": m.group(4),
                    "note": m.group(5), "branches": branches, "word": b.group(2) if b else None, "violations": vio})
    return out


def schedule_check(recs, w):
    """(problems, facts): the printed records against the frozen schedule (G.4 items 5-7)."""
    problems = []
    if [r["n"] for r in recs] != list(range(1, w["total"] + 1)) or any(r["total"] != w["total"] for r in recs):
        problems.append(f"step records numbered {[(r['n'], r['total']) for r in recs]}, not 1..{w['total']} of "
                        f"{w['total']}")
        return problems, []
    for r, (actor, choices, fixed) in zip(recs, w["steps"]):
        if r["actor"] != actor or r["choices"] != choices:
            problems.append(f"step {r['n']}: {r['actor']} {r['choices']}, not {actor} {choices}")
        if r["branches"] is None:
            problems.append(f"step {r['n']}: no branches line")
        elif fixed is not None and r["branches"] != fixed:
            problems.append(f"step {r['n']}: branches {r['branches']}, not the fixed {fixed}")
    facts = [f"step {r['n']}/{r['total']} {r['actor']} choices {r['choices']}: branches {r['branches']}; word "
             f"{r['word']}; violations {r['violations']}" for r in recs]
    return problems, facts


# ---- G.3(a): the primary ------------------------------------------------------------------------------------------

def classify_primary(d, ctl, exe, source_verdict="EXACTLY_C_PLUS_PATCH"):
    classify.refuse_twice(d)
    ck = classify.Checks()
    desc, text = classify.load(d)
    g = ctl["g3"]
    PRED, N = g["predicted"], ctl["named_property"]
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return classify.write(d, PRIMARY_NOT, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    cfg, cap, limit = ctl["configuration"], ctl["ceiling_states"], ctl["limit_seconds"]
    ck("tool invariant: the predicted first failure is not the named property (G.3(a): E3 is never counted as P1, E4 "
       "never as P7)", PRED != N and PRED in report.model_constants()[0], f"{PRED} / {N}")
    classify.check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    classify.check_selection(ck, desc, exe, ctl["test"], ctl["argv_tail"], ctl["ignored"])
    ck(f"the catalog's own test {ctl['test']} on the original explorer (full_exploration; never a gc_symmetry_* "
       "quotient test)", desc.get("test") == ctl["test"] and ctl["runner"] == "full_exploration"
       and "gc_symmetry" not in ctl["test"] and ctl["test"].endswith("::full_f1"), desc.get("test"))
    starts, running, result, failed_names = classify.harness(text, ctl["test"])
    ck("the harness ran exactly that one test", running == ["1"] and starts == [ctl["test"]],
       f"running {running}; started {starts}")
    want = f"test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: '{want}…' and the failures list names exactly that test", len(result) == 1
       and result[0].startswith(want) and failed_names == [ctl["test"]], f"{result}; failures {failed_names}")
    ck("exit 101 by a normal exit, no signal", proc["exit_status"] == 101 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    classify.check_supervision(ck, sup, limit)
    lines, heads, anyp = classify.panics(text)
    va = ctl["verdict_assertion"]
    msg = []
    ck("exactly one panic, of that test's thread", len(heads) == 1 and len(anyp) == 1
       and heads[0][1].group(1) == ctl["test"], anyp[:3])
    rep = report.parse_report(text)
    prop = (rep["fail"] or "").split(": ", 1)[0] if rep and rep["fail"] else None
    if len(heads) == 1:
        h = heads[0][1]
        ck(f"the panic is the explorer's own verdict assertion ({ctl['runner']}, {va['file']}:{va['line']}:{va['col']})",
           (h.group(2), int(h.group(3)), int(h.group(4))) == (va["file"], va["line"], va["col"]),
           f"{h.group(2)}:{h.group(3)}:{h.group(4)}")
        msg = classify.panic_message(lines, heads[0][0])
        ck("the assertion message carries failed: {\"<p>\"} for the printed property p, and only it",
           prop is not None and msg[:1] == [f'assertion `left == right` failed: failed: {{"{prop}"}}'], msg[:1])
        ck("the assertion's operands: left Fail, right Pass", msg[-2:] == ["  left: Fail", " right: Pass"], msg[-2:])
    ck("no quotient or symmetry refusal; no fixture failure; no witness line (F.1 (a): none is required)",
       "inapplicable" not in text and "WITNESS FIXTURE FAILURE" not in text and " WITNESS " not in text
       and "NAMED PROPERTY" not in text)
    ck("the report is printed, then repeated verbatim in the assertion message", rep is not None
       and text.count("EXPLORATION ") == 2 and len(heads) == 1 and "\n".join(msg[1:-3]) == "\n".join(
           [rep["header"]] + rep["lines"]), f"{text.count('EXPLORATION ')} EXPLORATION headers")
    if rep is not None:
        ck(f"header 'EXPLORATION {cfg}' exactly (the original explorer; no quotient or symmetry line)",
           rep["header"] == f"EXPLORATION {cfg}" and "quotient_line" not in rep and "symmetry_line" not in rep,
           rep["header"])
        ck("printed bounds equal the configuration's (source-derived)", rep.get("bounds") == ctl["bounds"],
           rep.get("bounds"))
        ck("printed actors equal the configuration's", rep.get("actors") == ctl["actors"], rep.get("actors"))
        ck(f"caps printed: {cap:,} states, {limit} s (the configuration's adopted ceiling)",
           rep.get("cap") == cap and rep.get("time_cap_s") == limit, f"{rep.get('cap')}, {rep.get('time_cap_s')}")
        ck("raw outcome FAIL; no INCOMPLETE line", rep.get("outcome") == "FAIL" and rep["incomplete"] is None,
           f"{rep.get('outcome')}; {rep['incomplete']}")
        ck(f"G.3(a): the first printed property, recorded as printed, is the predicted first failure {PRED} (mission "
           f"section {4 if ctl['id'] == 'C1' else 5}); it is not counted as {N}; another first property is reported as "
           "printed and requires an Architect ruling", prop == PRED, rep["fail"])
        have = all(k in rep for k in ("states", "expanded"))
        ck("'states <n> (expanded <m>)' printed", have)
        if have:
            ck(f"discovered count <= the ceiling {cap:,} (K.1, inclusive)", rep["states"] <= cap, f"{rep['states']:,}")
        ck("the trace is printed under 'shortest trace (<n> steps):'",
           re.fullmatch(r"  shortest trace \((\d+) steps\):", rep["trace_steps"] or "") is not None, rep["trace_steps"])
        ck("a FAIL prints only its failing property (no property lines)", rep["properties"] == [], rep["properties"][:3])
        props, branches = report.model_constants()
        ck("the failing property is one of PROPERTIES", prop in props, prop)
        labels = [lab for _, lab in rep["branches"]]
        ck("the report is complete: branch list (every BRANCHES label, in order) and the key-fields line",
           labels == branches and rep["key_fields"] is not None, f"{len(labels)} of {len(branches)}")
    good = ck.ok()
    other_first = bool(rep and rep.get("outcome") == "FAIL" and prop not in (None, PRED)
                       and rep.get("states", 0) <= cap)
    if good:
        outer = (f"FAIL — admissible type-(a) original-explorer mutant FAIL; first failure {prop} recorded as printed "
                 f"(AD-1-R4 G.3(a) primary of {ctl['id']}); {prop} is not counted as {N}; {N}'s attribution comes only "
                 "from the type-(c) witness (G.3(b))")
    elif rep is not None and rep.get("outcome") == "FAIL" and rep.get("states", 0) > cap:
        outer = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    elif rep is not None and rep.get("outcome") == "INCOMPLETE":
        outer = f"INCOMPLETE ({rep.get('incomplete')}): kills nothing"
    elif "inapplicable" in text:
        outer = "INCOMPLETE — VERIFICATION INVALID: kills nothing"
    elif any(l.startswith(("exit 101", "launching command")) for l in ck.failed()) and (
            proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]):
        outer = "INCOMPLETE (killed from outside or abnormal termination): kills nothing"
    elif rep is not None and rep.get("outcome") == "PASS":
        outer = f"PASS on the mutant (the control survives {cfg}): kills nothing"
    elif other_first:
        outer = (f"FAIL of {prop} as the first printed property, not the G.3(a) prediction {PRED}: reported as printed, "
                 f"not reinterpreted; {RULING_REQUIRED}; kills nothing")
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): kills nothing"
    extra = []
    if rep:
        extra.append(f"raw outcome (as printed): outcome {rep.get('outcome')}; FAIL {rep.get('fail')}; "
                     f"{rep.get('trace_steps', '').strip() if rep.get('trace_steps') else 'no trace'}; harness {result}")
        if "states" in rep:
            extra.append(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}) of the {cap:,} ceiling; "
                         f"transitions {rep.get('transitions')}; elapsed {rep.get('elapsed_s')} s")
    extra.append(f"G.3(a): predicted first failure {PRED} (AD-1-R4: in {g['predicted_steps']} steps; the step count is "
                 f"recorded, not required); named property {N}, attributed only by G.3(b); configuration {cfg}; test "
                 f"{ctl['test']}")
    extra.append("kill label: none (the primary is a part of G.3 only)")
    extra.append(classify.resources_line(sup, limit))
    classify.write(d, PRIMARY_OK if good else PRIMARY_NOT, outer, ck,
                   {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (a), the G.3(a) primary", "test": ctl["test"],
                    "configuration": cfg, "named_property": N, "predicted_first_failure": PRED,
                    "printed_property": prop, "other_first_property": other_first, "ceiling_states": cap,
                    "time_limit_seconds": limit,
                    "report": {k: rep.get(k) for k in ("header", "outcome", "bounds", "actors", "states", "expanded",
                                                        "transitions", "fault_edges", "no_step", "elapsed_s", "cap",
                                                        "time_cap_s", "fail", "incomplete", "trace_steps")}
                    if rep else None,
                    "panic": {"header": lines[heads[0][0]] if len(heads) == 1 else [lines[i] for i, _ in heads],
                              "message_first_line": msg[:1]},
                    "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
                    "selection": desc["selection"], "raw_output_sha256": desc["raw_output_sha256"],
                    "not_required": "any witness line, locus or verdict (F.1 (a))"}, extra)
    return 0 if good else 1


# ---- G.3(b), (c): the mutant witness ------------------------------------------------------------------------------

def classify_witness(d, ctl, exe, primary_dir, source_verdict="EXACTLY_C_PLUS_PATCH"):
    classify.refuse_twice(d)
    ck = classify.Checks()
    desc, text = classify.load(d)
    w = ctl["g3"]["witness"]
    test, N, OTHER = w["test"], ctl["named_property"], ctl["g3"]["predicted"]
    if not ck("the witness started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return classify.write(d, WITNESS_NOT, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    limit = w["limit_seconds"]
    classify.check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    pdesc = json.load(open(os.path.join(primary_dir, "descriptor.json")))
    pin = json.load(open(os.path.join(os.path.dirname(primary_dir), "03-build/PIN.json")))
    pid = [pdesc["executable"]["before"], pdesc["executable"]["after"]]
    ck("F.2 role: the SAME pinned mutant executable as this control's type-(a) primary (03-build/PIN.json and the "
       "primary's descriptor before and after: path, sha256, size), never the unchanged-baseline executable",
       exe["path"] == pin["path"] == pdesc["executable"]["path"] and exe["sha256"] == pin["sha256"]
       and exe["size"] == pin["size"] and all(i["path"] == exe["path"] and i["sha256"] == exe["sha256"]
                                               and i["size"] == exe["size"] for i in pid)
       and exe["sha256"] != K.EXE_SHA256 and exe["path"] != K.EXE, f"{exe['sha256'][:16]}… {exe['size']}")
    ck("the primary of the same control is classified PRIMARY_OK (no witness follows an unestablished primary)",
       supervise.first_line(os.path.join(primary_dir, "CLASSIFICATION.txt")) == PRIMARY_OK,
       supervise.first_line(os.path.join(primary_dir, "CLASSIFICATION.txt")))
    classify.check_selection(ck, desc, exe, test, w["argv_tail"], False)
    ck("argv tail exactly: the witness test, --exact, --nocapture, --test-threads=1 (no --ignored: an ordinary test)",
       w["argv_tail"] == [test, "--exact", "--nocapture", "--test-threads=1"] and desc["argv"][1:] == w["argv_tail"],
       desc["argv"][1:])
    starts, running, result, failed_names = classify.harness(text, test)
    ck("the harness ran exactly that one test", running == ["1"] and starts == [test],
       f"running {running}; started {starts}")
    want = f"test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    ck(f"harness: '{want}…' (1 failed) and the failures list names exactly that test", len(result) == 1
       and result[0].startswith(want) and failed_names == [test], f"{result}; failures {failed_names}")
    ck("exit 101 by a normal exit, no signal", proc["exit_status"] == 101 and proc["signal"] is None,
       f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    classify.check_supervision(ck, sup, limit)
    lines, heads, anyp = classify.panics(text)
    pa = w["panic"]
    ck("exactly one panic, of the witness test's thread", len(heads) == 1 and len(anyp) == 1
       and heads[0][1].group(1) == test, anyp[:3])
    msg, n_vio = [], None
    hdr = re.compile(rf"^{re.escape(w['header_prefix'])}(\d+) violations$")
    if len(heads) == 1:
        h = heads[0][1]
        ck(f"the panic is the witness's own verdict panic (run_witness's panic!, {pa['file']}:{pa['line']}:{pa['col']}), "
           "not a fixture assertion or an unrelated panic",
           (h.group(2), int(h.group(3)), int(h.group(4))) == (pa["file"], pa["line"], pa["col"]),
           f"{h.group(2)}:{h.group(3)}:{h.group(4)}")
        msg = classify.panic_message(lines, heads[0][0])
        m = hdr.match(msg[0]) if msg else None
        n_vio = int(m.group(1)) if m else None
        ck(f"the header: '{w['header_prefix']}<n> violations', n at least 1 (G.3(b))", m is not None and n_vio >= 1,
           msg[:1])
        ck(f"the exact positive line follows the header: '{w['positive']}' (G.3(b))", msg[1:2] == [w["positive"]],
           msg[1:2])
        others = None
        if len(msg) > 2 and msg[2].startswith(w["others_prefix"]):
            try:
                others = ast.literal_eval(msg[2][len(w["others_prefix"]):])
            except (ValueError, SyntaxError):
                others = None
        ck(f"the line of other properties, each reported separately and not counted as {N}, names {OTHER} and not {N} "
           "(G.3(b))", isinstance(others, list) and OTHER in others and N not in others
           and all(isinstance(x, str) for x in others), msg[2:3])
        vio = msg[4:]
        ck("'every violation:' then exactly n violation lines (the header's count), each indented",
           msg[3:4] == ["every violation:"] and n_vio is not None and len(vio) == n_vio
           and all(v.startswith("  ") and not v.startswith("   ") for v in vio), f"{len(vio)} lines; n {n_vio}")
        locus = f"  step {w['at']} {'step' if w['in_step'] else 'after'}: {N}: "
        ck(f"the named property's violation is listed at its locus ('{locus.strip()} …'; G.4 item 9)",
           sum(v.startswith(locus) for v in vio) >= 1, [v[:60] for v in vio if f": {N}: " in v])
        ck(f"{OTHER} is listed among the violations, separately ({OTHER} is never counted as {N})",
           any(re.match(rf"^  (initial state|step \d+ (step|after)): {OTHER}: ", v) for v in vio), "")
    ck("the exact positive line occurs exactly once in the whole output, as a whole line; exactly one NAMED PROPERTY "
       "line; no 'NOT OBSERVED'", lines.count(w["positive"]) == 1 and text.count(w["positive"]) == 1
       and sum("NAMED PROPERTY" in l for l in lines) == 1 and "NOT OBSERVED" not in text)
    ck("no healthy line in a mutant-witness receipt (F.2; G.3(c)): neither the healthy line nor 'no violation' nor the "
       "healthy-branch assertion", w["healthy_line"] not in text and "no violation" not in text
       and "WITNESS: no violation" not in text and "shows the healthy core's" not in text)
    ck("no WITNESS FIXTURE FAILURE (G.3(c)); no quotient or symmetry refusal; no explorer report (NOT PRODUCED BY "
       "DESIGN for type (c))", "WITNESS FIXTURE FAILURE" not in text and "inapplicable" not in text
       and "EXPLORATION " not in text)
    body = body_lines(text, test)
    recs = step_records(body, w)
    probs, facts = schedule_check(recs, w)
    ck(f"every scheduled step printed, numbered 1..{w['total']}, with its frozen actor and choices and its fixed "
       "branches (G.4 items 5-7)", not probs, probs[:3])
    if len(recs) == w["total"] and not probs:
        at = recs[w["at"] - 1]
        ck(f"at step {w['at']} (the step the control changes) the branch '{w['changed']}' is shown, as run_witness's own "
           "predicate requires (G.3(b), G.4 item 9)", w["changed"] in (at["branches"] or []), at["branches"])
        place = "step" if w["in_step"] else "after"
        ck(f"step {w['at']}'s record prints the violation of {N} in its {'own violations' if w['in_step'] else 'state after it'}"
           f" (VIOLATION {place}: {N})", (place, N) in at["violations"], at["violations"])
        vio_lines = sum(len(r["violations"]) for r in recs)
        initial = sum(v.startswith("  initial state: ") for v in msg[4:])
        ck("the header's count equals the printed per-step violation lines plus the initial state's listed ones",
           n_vio is not None and vio_lines + initial == n_vio, f"{vio_lines} printed + {initial} initial; header {n_vio}")
    good = ck.ok()
    if good:
        outer = (f"FAIL — admissible type-(c) named-property mutant-witness receipt (G.3(b)): '{w['positive']}'; "
                 f"{w['header_prefix']}{n_vio} violations; {OTHER} reported separately, not counted as {N}")
    elif proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]:
        outer = "INCOMPLETE (killed from outside or abnormal termination): establishes nothing"
    elif "WITNESS FIXTURE FAILURE" in text:
        outer = "NOT ADMISSIBLE (a WITNESS FIXTURE FAILURE: establishes nothing; G.3(c))"
    elif "NOT OBSERVED" in text:
        outer = f"NOT ADMISSIBLE (NAMED PROPERTY {N}: NOT OBSERVED at its locus: establishes nothing; G.3(c))"
    elif proc["exit_status"] == 0 or w["healthy_line"] in text:
        outer = "NOT ADMISSIBLE (a healthy result in a mutant-witness receipt: establishes nothing; F.2, G.3(c))"
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): establishes nothing"
    extra = [f"raw outcome (as printed): harness {result}; panic message {msg[:3]}",
             f"positive line: {w['positive']!r} — {'present at its locus' if good else 'NOT ESTABLISHED'}",
             f"executable: the pinned mutant executable of the primary, {exe['path']} sha256 {exe['sha256']} size "
             f"{exe['size']}",
             "explorer report, counts, property lines and branch list: NOT PRODUCED BY DESIGN (F.1 (c))",
             "kill label: none (the witness is a part of G.3 only)", classify.resources_line(sup, limit)]
    classify.write(d, WITNESS_OK if good else WITNESS_NOT, outer, ck,
                   {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (c), the G.3(b) mutant witness", "test": test,
                    "named_property": N, "positive_line": w["positive"], "header_violations": n_vio,
                    "other_property_required": OTHER, "panic_message": msg, "step_records": recs,
                    "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
                    "primary_executable": pdesc["executable"], "selection": desc["selection"],
                    "raw_output_sha256": desc["raw_output_sha256"],
                    "not_produced_by_design": "the explorer report, counts, property lines and branch list (F.1 (c))"},
                   extra + [f"step: {f}" for f in facts])
    return 0 if good else 1


# ---- G.3(d): the accepted healthy witness, linked -------------------------------------------------------------------

def link_healthy(ctl, outdir):
    os.makedirs(outdir)
    hw, w = ctl["g3"]["healthy"], ctl["g3"]["witness"]
    checks = []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))
    own = os.path.join(os.path.dirname(outdir), "05-restore/RESTORE.txt")
    chk("this control's own restoration receipt exists and records RESTORE: EXACTLY_C (no link before restoration)",
        supervise.first_line(own) == "RESTORE: EXACTLY_C", f"{os.path.relpath(own, K.EV)}: {supervise.first_line(own)!r}")
    d = os.path.join(K.EV, hw["input_dir"])
    sums = K.blob(hw["commit"], K.STAGE2_PACKET + "/SHA256SUMS").decode()
    listed = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}
    rel = hw["directory"][len(K.STAGE2_PACKET) + 1:]
    names = sorted(os.listdir(d)) if os.path.isdir(d) else []
    want_names = sorted(p[len(rel) + 1:] for p in listed if p.startswith(rel + "/"))
    bad = [f for f in names if listed.get(f"{rel}/{f}") != K.sha256_file(os.path.join(d, f))]
    chk(f"the linked files ({len(names)}) are exactly the accepted receipt's ({hw['directory']} in "
        f"{hw['commit'][:8]}'s SHA256SUMS), byte for byte", not bad and names == want_names
        and len(names) == HEALTHY_FILES, bad or names)
    chk("the receipt is the one the Stage-2 evidence commit introduced, unchanged at this commit's parent (every file's "
        "blob)", all(K.git("rev-parse", f"{hw['commit']}:{hw['directory']}/{f}") == K.git(
            "rev-parse", f"{K.PARENT}:{hw['directory']}/{f}") for f in want_names), K.PARENT[:8])
    if bad or names != want_names:
        return _write_healthy(outdir, ctl, checks, None)
    raw_b = open(os.path.join(d, "raw-output.txt"), "rb").read()
    raw = raw_b.decode("utf-8")
    cls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    chk("its classification blob and raw-output blob and sha256 are the catalog's (healthy_witness)",
        K.git_blob_id(open(os.path.join(d, "CLASSIFICATION.txt"), "rb").read()) == hw["classification_blob"]
        and K.git_blob_id(raw_b) == hw["raw_output_blob"] and K.sha256_bytes(raw_b) == hw["raw_output_sha256"], "")
    chk("accepted healthy-witness type-(e3) PASS (classification, as the catalog records it)",
        cls[:2] == ["CLASSIFICATION: PASS", hw["outer_class"]]
        and hw["outer_class"] == "outer class (AD-1-R4 K.2): PASS — admissible healthy-witness type-(e3) PASS", cls[:2])
    h = json.load(open(os.path.join(d, "descriptor.json")))
    chk("the same witness test alone, --exact --nocapture --test-threads=1, no --ignored, on the pinned unchanged-baseline "
        "executable (catalog descriptor fields)", h["argv"] == [K.EXE, w["test"], *hw["descriptor_argv_flags"]]
        and hw["descriptor_argv_flags"] == ["--exact", "--nocapture", "--test-threads=1"]
        and h["test"] == w["test"] == hw["descriptor_test"] and h["argv"][1:] == w["argv_tail"], h["argv"])
    sel = h.get("selection", {})
    chk("its selection listing shows exactly that one test, and it is not an ignored test",
        sel.get("listing", {}).get("listed") == [w["test"] + ": test"] and sel.get("ignored_listing", {}).get("listed")
        == [], sel.get("listing", {}).get("listed"))
    ex = h["executable"]
    now = K.sha256_file(K.EXE)
    mutant = json.load(open(os.path.join(os.path.dirname(outdir), "03-build/PIN.json")))
    chk("F.2 role: the pinned unchanged-baseline executable, unchanged before and after the healthy run and hashed now "
        "equal; never this control's mutant executable",
        ex["before"]["sha256"] == ex["after"]["sha256"] == K.EXE_SHA256 == now == hw["executable_sha256"]
        and ex["path"] == K.EXE and mutant["sha256"] != K.EXE_SHA256 and mutant["path"] != K.EXE, now)
    chk("exact Candidate C: the source exactly C before and after the healthy run",
        h["source_before"]["verdict"] == "EXACTLY_C" == h["source_after"]["verdict"]
        and h["source_before"]["head"] == K.C and h["source_before"]["tree"] == K.C_TREE, "")
    pr, sp = h["process"], h["supervisor"]
    chk("its process and launching command: exit 0 by a normal exit; 16 GiB, zero swap, no OOM, no kill, no timeout, "
        "complete", pr["exit_status"] == 0 and pr["signal"] is None and sp["memory_max"] == K.MEMORY_MAX
        and sp["swap_max"] == "0" and sp["oom_kill"] == 0 and sp["oom"] == 0 and not sp["timed_out"] and sp["completed"],
        f"exit {pr['exit_status']}, signal {pr['signal']}")
    body = body_lines(raw, w["test"])
    chk(f"the exact healthy line of G.3(d), exactly once, as a whole line: '{w['healthy_line']}'",
        body.count(w["healthy_line"]) == 1 and raw.count(w["healthy_line"]) == 1)
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    res = re.findall(r"^test result: .*$", raw, re.M)
    chk("harness: 'ok', one test, 1 passed, 0 failed", len(res) == 1 and res[0].startswith(want)
        and re.search(r"\nok\n\ntest result: ok\.", raw) is not None, res)
    marks = [l for l in body if any(m in l for m in POSITIVE_MARKERS)]
    chk("no positive marker, violation, fixture failure or panic in the healthy receipt (F.2)", not marks, marks[:2])
    recs = step_records(body, w)
    probs, _ = schedule_check(recs, w)
    chk(f"the same frozen schedule: {w['total']} step records, each with its actor, choices and fixed branches",
        not probs, probs[:3])
    if not probs and len(recs) == w["total"]:
        chk(f"no violation on the healthy core, and step {w['at']} shows the healthy branch '{w['healthy']}' (G.3(d))",
            all(not r["violations"] for r in recs) and recs[w["at"] - 1]["branches"] == [w["healthy"]],
            recs[w["at"] - 1]["branches"])
        wdesc = os.path.join(os.path.dirname(outdir), "04c-witness", "PARSED-RECEIPT.json")
        mrecs = json.load(open(wdesc)).get("step_records", []) if os.path.exists(wdesc) else []
        chk("the same schedule as this control's own mutant witness: every step's actor and choices equal, step by step",
            [(r["actor"], r["choices"]) for r in recs] == [(r["actor"], r["choices"]) for r in mrecs], len(mrecs))
    return _write_healthy(outdir, ctl, checks, hw)


def _write_healthy(outdir, ctl, checks, hw):
    ok = bool(checks) and all(o for _, o, _ in checks)
    ref = (f"{hw['directory']} at {hw['commit']} (accepted healthy-witness type-(e3) PASS on the pinned unchanged-"
           f"baseline executable {K.EXE_SHA256})" if hw else "not linked")
    lines = [HEALTHY_OK if ok else HEALTHY_FAIL, f"control {ctl['id']}: healthy-witness receipt {ref}",
             "linked (never rerun) only after this control's own restoration was verified"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    open(os.path.join(outdir, "HEALTHY.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "type": "e3", "reference": ref, "checks": checks},
              open(os.path.join(outdir, "HEALTHY.json"), "w"), indent=1)
    print("\n".join(lines))
    return ok, ref


# ---- G.3 (a)-(e): the obligations, from this control's own receipts --------------------------------------------------

def obligations(ctl, outdir):
    os.makedirs(outdir)
    base = os.path.dirname(outdir)
    g = ctl["g3"]
    checks = []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))

    def first(rel):
        return supervise.first_line(os.path.join(base, rel))

    def jl(rel):
        p = os.path.join(base, rel)
        return json.load(open(p)) if os.path.exists(p) else {}
    pr, wr = jl("04-run/PARSED-RECEIPT.json"), jl("04c-witness/PARSED-RECEIPT.json")
    chk(f"(a) the primary is an admissible type-(a) FAIL whose first printed property is {g['predicted']}, recorded as "
        f"printed and not counted as {ctl['named_property']}", first("04-run/CLASSIFICATION.txt") == PRIMARY_OK
        and pr.get("printed_property") == g["predicted"] != ctl["named_property"], pr.get("printed_property"))
    chk(f"(b) the mutant witness is an admissible type-(c) receipt with the exact positive line '{g['witness']['positive']}'",
        first("04c-witness/CLASSIFICATION.txt") == WITNESS_OK and wr.get("positive_line") == g["witness"]["positive"],
        first("04c-witness/CLASSIFICATION.txt"))
    pe, we = pr.get("executable", {}), wr.get("executable", {})
    pin = jl("03-build/PIN.json")
    chk("(b) F.2: the witness ran on the same pinned mutant executable as the primary (path, sha256 and size, before "
        "and after both runs, equal to 03-build/PIN.json)",
        pin and all(x.get("path") == pin["path"] and x.get("sha256") == pin["sha256"] and x.get("size") == pin["size"]
                    for x in (pe.get("before", {}), pe.get("after", {}), we.get("before", {}), we.get("after", {}))),
        pin.get("sha256"))
    chk("(c) none of the witness rejections: every check of the witness receipt holds (CHECKS.txt: no FAIL line)",
        os.path.exists(os.path.join(base, "04c-witness/CHECKS.txt")) and not [
            l for l in open(os.path.join(base, "04c-witness/CHECKS.txt")).read().splitlines() if l.startswith("FAIL")])
    chk("restoration: the worktree restored exactly to C after the mutant receipts", first("05-restore/RESTORE.txt") ==
        "RESTORE: EXACTLY_C")
    chk("(d) the accepted healthy type-(e3) witness linked after the restoration and verified",
        first("06a-healthy-witness/HEALTHY.txt") == HEALTHY_OK)
    chk("the accepted F-1 unchanged-core type-(e1) receipt linked after the restoration and verified",
        first("06b-link/LINK.txt") == "LINK: VERIFIED")
    ok = all(o for _, o, _ in checks)
    lines = [G3_OK if ok else G3_FAIL, f"control {ctl['id']}: AD-1-R4 G.3 (kill criterion for C1 and C25 only)"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    if ok:
        lines.append(f"(e) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): '{g['recording']}'")
    else:
        lines.append("(e) no recording: G.3 is not established")
    open(os.path.join(outdir, "G3.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "holds": ok, "checks": checks,
               "recording_proposed_for_architect_review": g["recording"] if ok else None},
              open(os.path.join(outdir, "G3.json"), "w"), indent=1)
    print("\n".join(lines))
    return ok
