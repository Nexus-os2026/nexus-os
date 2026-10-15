#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01, mission sections 6-9 and 12: the AD-1-R4 G.5 kill criterion and the G.6
offered-versus-consumed Oracle rule, for C4 only (AD-1-R4 F.1 types (a), (c), (e3); F.2; G.5; G.6; K.1; K.2). Nothing
here builds or runs anything: it reads receipt directories and the authenticated input copies and writes its own
records. Every function keeps the raw outcome (as printed) apart from the outer class, and fails closed.

  classify_primary(d, ctl, exe)         G.5(A): the type-(a) primary, full_f8 on the original explorer on the pinned C4
                                        mutant executable. Every check of the unchanged classify_a (as g3.py's
                                        classify_primary carried them for C1 and C25), with G.5(A)'s attribution: the
                                        first printed property is recorded as printed, never relabelled; P4 or P8 is
                                        admissible (mission section 6); any other first property is reported as printed
                                        and stops the mission for an Architect ruling. The primary does not establish
                                        both properties: it is a part of G.5 only. First line of CLASSIFICATION.txt:
                                        PRIMARY_OK or PRIMARY_NOT.
  classify_witness(d, ctl, exe, primary_dir)
                                        G.5(B) and G.6 on the type-(c) mutant witness: the witness test alone with
                                        --exact (no --ignored) on the SAME pinned mutant executable as the primary (F.2).
                                        Required: the witness's own verdict panic (run_c4_witness's verdict panic!), 1
                                        failed, exit 101 by a normal exit; both scenario headers; the C4 WITNESS F-8
                                        header; BOTH exact positive lines at their frozen loci (P4 after step 8 of
                                        scenario P4, P8 after step 12 of scenario P8), each with its violation in that
                                        step's after-state and the locus facts in the printed world; the two separate
                                        lines of other properties (never counted as P4 or P8); and G.6 verified from the
                                        raw output, step by step, in both scenarios (actor, offered stream, consumed
                                        sequence with option counts, unused suffix, branch labels, remaining fault
                                        budget and its [fault] marker, step order; no consumption beyond the offered
                                        stream: no silent Oracle-default zero). Rejected: one property only, NOT
                                        OBSERVED, another step, scenario or locus, a fixture failure, an unrelated panic,
                                        a healthy line, another selection or executable, incomplete output or abnormal
                                        termination, any breach of G.6. First line: WITNESS_OK or WITNESS_NOT.
  link_healthy(ctl, outdir)             G.5(C): the accepted Stage-2 healthy type-(e3) receipt, linked (never rerun)
                                        only after this control's own restoration receipt records RESTORE: EXACTLY_C;
                                        authenticated again here (every file against Stage 2's SHA256SUMS; the catalog's
                                        identities; the pinned unchanged-baseline executable, hashed now; the three exact
                                        healthy lines; no positive marker); G.6 verified on its transcript (each changed
                                        append refused before storage, consuming 0 of its offered choices, the whole
                                        offered stream unused); and compared step by step with this control's mutant
                                        transcript: the same actor sequence and offered streams everywhere, and before
                                        the changed append the same consumption, option counts, unused suffixes, branch
                                        labels and fault budgets (G.6 items 1 and 2). First line HEALTHY_OK or
                                        HEALTHY_FAIL.
  obligations(ctl, outdir)              G.5 (A), (B), (C), (D) and (E)'s identity and restoration part, read from this
                                        control's own receipts; (E)'s Architect review is never claimed; then (F)'s
                                        recording, PROPOSED FOR ARCHITECT REVIEW (never an acceptance). First line G5_OK
                                        or G5_FAIL.

The expectations (ctl["g5"]) are derived by expectations.py from the frozen catalog, AD-1-R4 and Candidate C's source
(the C4Step and C4Scenario constants and run_c4_witness's formats), and cross-checked against the mission's own
statement (common.G5).
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

PRIMARY_OK = ("CLASSIFICATION: G.5(A) PRIMARY — ADMISSIBLE TYPE-(a) FAIL, RECORDED AS PRINTED (A PART ONLY; NOT A KILL "
              "BY ITSELF)")
PRIMARY_NOT = "CLASSIFICATION: G.5(A) PRIMARY — NOT ESTABLISHED"
WITNESS_OK = ("CLASSIFICATION: G.5(B) MUTANT WITNESS — ADMISSIBLE TYPE-(c) RECEIPT: P4 AND P8 AT THEIR FROZEN LOCI; G.6 "
              "HOLDS (A PART ONLY)")
WITNESS_NOT = "CLASSIFICATION: G.5(B) MUTANT WITNESS — NOT ADMISSIBLE"
HEALTHY_OK = "HEALTHY WITNESS LINK: VERIFIED"
HEALTHY_FAIL = "HEALTHY WITNESS LINK: FAILED"
G5_OK = "G.5: (A) (B) (C) (D) HOLD; (E) IDENTITY AND RESTORATION HOLD; (E) ARCHITECT REVIEW PENDING"
G5_FAIL = "G.5: NOT ESTABLISHED"
RULING_REQUIRED = "STOP FOR ARCHITECT RULING (AD-1-R4 G.5(A))"
POSITIVE_MARKERS = ("NAMED PROPERTY", "OBSERVED", "VIOLATION", "WITNESS FIXTURE FAILURE", "OTHER PROPERTIES VIOLATED",
                    "every violation:", "panicked")
HEALTHY_FILES = 9  # the Stage-2 healthy receipt directory: its files as Stage 2's SHA256SUMS lists them

SCEN_HDR = re.compile(r"^C4 WITNESS (\S+) (\w+): (\d+) steps from the initial world; ")
STEP = re.compile(r"^C4 WITNESS (\S+) (\w+) step +(\d+)/(\d+) (\S+) offered (\[[^\]]*\]) consumed (\[[^\]]*\]) unused "
                  r"(\[[^\]]*\]) faults left (\d+): (.*)$")
BRANCHES = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) \(live (\d+), unapplied (\d+)\); phase (\S+), "
                      r"latch (.*)$")
CELLS = re.compile(r"^    U (.*); C (.*); X (.*)$")
G1 = re.compile(r"^    G1: insert issued (true|false), written (true|false), told Committed (true|false), in U "
                r"(true|false), correction in C (true|false), correction in the ledger (true|false)$")
LEDGER = re.compile(r"^    ledger after (\[.*\])$")
VIO = re.compile(r"^    VIOLATION (initial|step|after): (\w+): (.*)$")
FIXTURE = "    WITNESS FIXTURE FAILURE (kills nothing): "
FAULT_MARK = "   [fault]"
G1_KEYS = ("issued", "written", "told", "in_u", "in_c", "corrected")


def body_lines(text, test):
    """The output's lines, with the test's first printed line split off the harness's `test <name> ... ` line."""
    return text.replace(f"test {test} ... ", f"test {test} ... \n", 1).splitlines()


def lit(s):
    try:
        return ast.literal_eval(s)
    except (ValueError, SyntaxError):
        return None


# ---- G.6: the printed transcript, parsed independently of the witness's own checks ---------------------------------

def transcript(lines, config_name):
    """Every scenario the witness printed: {named: {header, initial: [(p, detail)], steps: [record], fixture}} and the
    lines inside the witness block that fit no part of its grammar (`stray`). A step record holds what its five
    printed lines say: n, total, actor, offered, consumed [(choice, options)], unused, faults, note, marker, branches,
    word, phase, latch, cells (U, C, X), g1, ledger, violations [(place, property, detail)], and `shape` problems."""
    out = {"scenarios": {}, "order": [], "stray": [], "witness_lines": 0, "preamble": []}
    cur, rec, expect = None, None, []
    for line in lines:
        # The witness block ends at the panic header (mutant) or at the harness's result; the harness's own lines and
        # the healthy closing lines are not part of the per-step grammar.
        if line.startswith("thread ") or line.startswith("test result:"):
            break
        if line in ("ok", "FAILED", "") or line.startswith("running ") or line.startswith("test ") \
                or line.startswith(f"C4 WITNESS {config_name}: ") \
                or (line.startswith(f"C4 WITNESS {config_name} ") and " steps, each enabled and consuming" in line):
            if expect and rec is not None:
                rec["shape"].append(f"missing {expect}")
            expect, rec = [], None
            continue
        h = SCEN_HDR.match(line)
        s = STEP.match(line)
        if not out["order"] and not (h and h.group(1) == config_name):
            out["preamble"].append(line[:120])  # before the witness's first line: the harness's own lines
            continue
        if h and h.group(1) == config_name:
            if expect and rec is not None:
                rec["shape"].append(f"missing {expect}")
            cur = {"header": line, "named": h.group(2), "total": int(h.group(3)), "initial": [], "steps": [],
                   "fixture": None}
            if h.group(2) in out["scenarios"]:
                out["stray"].append(f"scenario {h.group(2)} printed twice")
            out["scenarios"][h.group(2)] = cur
            out["order"].append(h.group(2))
            rec, expect = None, []
            out["witness_lines"] += 1
            continue
        if s and s.group(1) == config_name and cur is not None and s.group(2) == cur["named"]:
            if expect and rec is not None:
                rec["shape"].append(f"missing {expect}")
            note = s.group(10)
            rec = {"n": int(s.group(3)), "total": int(s.group(4)), "actor": s.group(5), "offered": lit(s.group(6)),
                   "consumed": lit(s.group(7)), "unused": lit(s.group(8)), "faults": int(s.group(9)), "note": note,
                   "marker": note.endswith(FAULT_MARK), "branches": None, "word": None, "phase": None, "latch": None,
                   "cells": None, "g1": None, "ledger": None, "violations": [], "shape": [],
                   "raw": {"step": line}}
            if not (isinstance(rec["offered"], list) and all(isinstance(x, int) for x in rec["offered"])):
                rec["shape"].append(f"offered {s.group(6)} is not a list of choices")
            if not (isinstance(rec["consumed"], list) and all(isinstance(x, tuple) and len(x) == 2 and all(
                    isinstance(y, int) for y in x) for x in rec["consumed"])):
                rec["shape"].append(f"consumed {s.group(7)} is not a list of (choice, options)")
            if not (isinstance(rec["unused"], list) and all(isinstance(x, int) for x in rec["unused"])):
                rec["shape"].append(f"unused {s.group(8)} is not a list of choices")
            cur["steps"].append(rec)
            expect = ["branches", "cells", "g1", "ledger"]
            out["witness_lines"] += 1
            continue
        v = VIO.match(line)
        if v and cur is not None:
            out["witness_lines"] += 1
            if v.group(1) == "initial":
                if cur["steps"]:
                    out["stray"].append(f"an initial-state violation after a step in scenario {cur['named']}: {line}")
                cur["initial"].append((v.group(2), v.group(3)))
            elif rec is None or expect:
                out["stray"].append(f"a step violation outside a complete step record: {line}")
            else:
                rec["violations"].append((v.group(1), v.group(2), v.group(3)))
            continue
        if line.startswith(FIXTURE) and cur is not None:
            out["witness_lines"] += 1
            cur["fixture"] = line[len(FIXTURE):]
            if expect and rec is not None:
                rec["shape"].append(f"missing {expect}")
            rec, expect = None, []
            continue
        if expect and rec is not None:
            want = expect.pop(0)
            m = {"branches": BRANCHES, "cells": CELLS, "g1": G1, "ledger": LEDGER}[want].match(line)
            out["witness_lines"] += 1
            if not m:
                rec["shape"].append(f"line {want!r} expected, got {line[:80]!r}")
                continue
            rec["raw"][want] = line
            if want == "branches":
                rec["branches"] = lit(m.group(1))
                rec["word"], rec["phase"], rec["latch"] = m.group(2), m.group(5), m.group(6)
                if not (isinstance(rec["branches"], list) and all(isinstance(b, str) for b in rec["branches"])):
                    rec["shape"].append(f"branches {m.group(1)[:60]} is not a list of labels")
            elif want == "cells":
                rec["cells"] = {"U": m.group(1), "C": m.group(2), "X": m.group(3)}
            elif want == "g1":
                rec["g1"] = dict(zip(G1_KEYS, (x == "true" for x in m.groups())))
            else:
                rec["ledger"] = m.group(1)
            continue
        out["stray"].append(line[:120])
    if expect and rec is not None:
        rec["shape"].append(f"missing {expect}")
    return out


def g6_check(tr, w, role):
    """(problems, facts): G.6 on one printed transcript, `role` 'mutant' or 'healthy'. Each scenario must be printed
    once, in the frozen order, with its exact header, and every step numbered 1..total with exactly the frozen actor
    and offered stream; every step's consumption must be a prefix of its offered stream (never longer: a choice
    beyond it would be the Oracle's silent default zero) with the frozen choices AND option counts; the printed unused
    suffix must be exactly the rest of the offered stream and the frozen one; the branch labels and the remaining
    fault budget the frozen ones, the [fault] marker present exactly when the budget fell. Before the changed append
    every step is the same on both cores (the frozen schedule); at the changed append the role's planned end only."""
    problems, facts = [], []
    if tr["stray"]:
        problems.append(f"lines outside the witness's grammar: {tr['stray'][:3]}")
    if tr["order"] != [s["named"] for s in w["scenarios"]]:
        problems.append(f"scenarios printed {tr['order']}, not {[s['named'] for s in w['scenarios']]}")
    for sc in w["scenarios"]:
        name = sc["named"]
        s = tr["scenarios"].get(name)
        if s is None:
            problems.append(f"scenario {name}: not printed")
            continue
        if s["header"] != sc["scenario_header"]:
            problems.append(f"scenario {name}: header {s['header'][:90]!r} is not the frozen one")
        if s["fixture"] is not None:
            problems.append(f"scenario {name}: WITNESS FIXTURE FAILURE {s['fixture'][:120]}")
        plan = [dict(st, n=i + 1) for i, st in enumerate(sc["steps"])]
        end = sc["changed_end"][role]
        plan.append({"n": sc["total"], "actor": sc["changed_actor"], "offered": sc["offered"],
                     "consumed": end["consumed"], "unused": end["unused"], "branches": [end["branch"]],
                     "faults": end["faults"]})
        got = s["steps"]
        if [r["n"] for r in got] != list(range(1, sc["total"] + 1)) or any(r["total"] != sc["total"] for r in got) \
                or s["total"] != sc["total"]:
            problems.append(f"scenario {name}: steps numbered {[(r['n'], r['total']) for r in got]}, not 1..{sc['total']}"
                            f" of {sc['total']} (the actor-step order)")
            continue
        budget = w["initial_faults"]
        for r, p in zip(got, plan):
            at = f"scenario {name} step {r['n']}/{r['total']}"
            if r["shape"]:
                problems.append(f"{at}: printed record incomplete: {r['shape'][:2]}")
                continue
            off, con, unu = r["offered"], [tuple(x) for x in r["consumed"]], r["unused"]
            if r["actor"] != p["actor"]:
                problems.append(f"{at}: actor {r['actor']}, not {p['actor']}")
            if off != p["offered"]:
                problems.append(f"{at}: offered {off}, not the frozen {p['offered']}")
            if len(con) > len(off):
                problems.append(f"{at}: consumed {len(con)} choices, beyond the {len(off)} offered (a silent Oracle "
                                f"default zero)")
            elif [c for c, _ in con] != off[:len(con)]:
                problems.append(f"{at}: consumed choices {[c for c, _ in con]} are not the offered prefix {off}")
            if any(not (0 <= c < o) for c, o in con):
                problems.append(f"{at}: a consumed choice not below its option count {con}")
            if con != [tuple(x) for x in p["consumed"]]:
                problems.append(f"{at}: consumed {con}, not the frozen {[tuple(x) for x in p['consumed']]} (choices and "
                                f"option counts)")
            if unu != off[len(con):]:
                problems.append(f"{at}: printed unused suffix {unu} is not the rest of the offered stream "
                                f"{off[len(con):]}")
            if unu != p["unused"]:
                problems.append(f"{at}: unused suffix {unu}, not the frozen {p['unused']}")
            if r["branches"] != p["branches"]:
                problems.append(f"{at}: branches {r['branches']}, not the frozen {p['branches']}")
            if r["faults"] != p["faults"]:
                problems.append(f"{at}: {r['faults']} faults left, not the frozen {p['faults']}")
            if r["marker"] != (r["faults"] < budget) or r["faults"] > budget:
                problems.append(f"{at}: [fault] marker {r['marker']} with the budget {budget} -> {r['faults']}")
            budget = r["faults"]
            facts.append(f"{at} {r['actor']}: offered {off} consumed {con} unused {unu}; branches {r['branches']}; "
                         f"faults left {r['faults']}{' [fault]' if r['marker'] else ''}; violations "
                         f"{[(pl, pr) for pl, pr, _ in r['violations']]}")
    return problems, facts


# ---- G.5(A): the primary ------------------------------------------------------------------------------------------

def classify_primary(d, ctl, exe, source_verdict="EXACTLY_C_PLUS_PATCH"):
    classify.refuse_twice(d)
    ck = classify.Checks()
    desc, text = classify.load(d)
    g = ctl["g5"]
    PERM, PRED, NAMED = g["permitted_primary"], g["predicted"], g["named"]
    if not ck("the run started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return classify.write(d, PRIMARY_NOT, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    cfg, cap, limit = ctl["configuration"], ctl["ceiling_states"], ctl["limit_seconds"]
    ck("tool invariant: the admissible first properties are exactly C4's named P4 and P8 (G.5(A), mission section 6); "
       "the prediction P4 is one of them and is not a receipt", PERM == NAMED == ["P4", "P8"] and PRED == "P4"
       and all(p in report.model_constants()[0] for p in PERM), f"{PERM} / {NAMED} / {PRED}")
    classify.check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    classify.check_selection(ck, desc, exe, ctl["test"], ctl["argv_tail"], ctl["ignored"])
    ck(f"the catalog's own test {ctl['test']} on the original explorer (full_exploration; never a quotient or symmetry "
       "test)", desc.get("test") == ctl["test"] and ctl["runner"] == "full_exploration"
       and "gc_symmetry" not in ctl["test"] and ctl["test"].endswith("::full_f8") and cfg == "F-8", desc.get("test"))
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
        ck(f"caps printed: {cap:,} states, {limit} s (F-8's ceiling)",
           rep.get("cap") == cap and rep.get("time_cap_s") == limit, f"{rep.get('cap')}, {rep.get('time_cap_s')}")
        ck("raw outcome FAIL; no INCOMPLETE line", rep.get("outcome") == "FAIL" and rep["incomplete"] is None,
           f"{rep.get('outcome')}; {rep['incomplete']}")
        ck("G.5(A): the first printed property, recorded as printed and never relabelled, is P4 or P8 (mission section "
           "6); any other first property is reported as printed and stops for an Architect ruling", prop in PERM,
           rep["fail"])
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
    other_first = bool(rep and rep.get("outcome") == "FAIL" and prop not in [None] + PERM
                       and rep.get("states", 0) <= cap)
    if good:
        outer = (f"FAIL — admissible type-(a) original-explorer mutant FAIL; first failure {prop} recorded as printed "
                 f"(AD-1-R4 G.5(A) primary of C4); the primary does not establish P4 and P8: both are attributed only "
                 "by the type-(c) witness (G.5(B))")
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
        outer = (f"FAIL of {prop} as the first printed property, neither P4 nor P8: reported as printed, not "
                 f"reinterpreted; {RULING_REQUIRED}; kills nothing")
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): kills nothing"
    extra = []
    trace_n = None
    if rep:
        m = re.fullmatch(r"  shortest trace \((\d+) steps\):", rep.get("trace_steps") or "")
        trace_n = int(m.group(1)) if m else None
        extra.append(f"raw outcome (as printed): outcome {rep.get('outcome')}; FAIL {rep.get('fail')}; "
                     f"{rep.get('trace_steps', '').strip() if rep.get('trace_steps') else 'no trace'}; harness {result}")
        if "states" in rep:
            extra.append(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}) of the {cap:,} ceiling; "
                         f"transitions {rep.get('transitions')}; elapsed {rep.get('elapsed_s')} s")
    extra.append(f"G.5(A): first printed property {prop} (recorded as printed; never relabelled); admissible first "
                 f"properties {' or '.join(PERM)}; the AD-1-R4 prediction {PRED} is not a receipt; neither P4 nor P8 is "
                 "inferred from the other; the attribution of both comes only from G.5(B)")
    extra.append("kill label: none (the primary is a part of G.5 only)")
    extra.append(classify.resources_line(sup, limit))
    classify.write(d, PRIMARY_OK if good else PRIMARY_NOT, outer, ck,
                   {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (a), the G.5(A) primary", "test": ctl["test"],
                    "configuration": cfg, "named_properties": NAMED, "admissible_first_properties": PERM,
                    "predicted_first_failure_not_a_receipt": PRED, "printed_property": prop,
                    "other_first_property": other_first, "ceiling_states": cap, "time_limit_seconds": limit,
                    "shortest_trace_steps": trace_n,
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


# ---- G.5(B) and G.6: the mutant witness ---------------------------------------------------------------------------

def classify_witness(d, ctl, exe, primary_dir, source_verdict="EXACTLY_C_PLUS_PATCH"):
    classify.refuse_twice(d)
    ck = classify.Checks()
    desc, text = classify.load(d)
    w = ctl["g5"]["witness"]
    test = w["test"]
    if not ck("the witness started (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", "")):
        return classify.write(d, WITNESS_NOT, "NOT ADMISSIBLE (not started)", ck, {"control": ctl["id"]}) or 1
    proc, sup = desc["process"], desc["supervisor"]
    limit = w["limit_seconds"]
    classify.check_identity(ck, desc, exe, source_verdict, ctl["mutant_blob"])
    pdesc = json.load(open(os.path.join(primary_dir, "descriptor.json")))
    pin = json.load(open(os.path.join(os.path.dirname(primary_dir), "03-build/PIN.json")))
    pid = [pdesc["executable"]["before"], pdesc["executable"]["after"]]
    ck("F.2 role: the SAME pinned C4 mutant executable as this control's type-(a) primary (03-build/PIN.json and the "
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
    msg, n_vio, others = [], None, {}
    hdr = re.compile(rf"^{re.escape(w['header_prefix'])}(\d+) violations$")
    sc = {s["named"]: s for s in w["scenarios"]}
    names = [s["named"] for s in w["scenarios"]]
    if len(heads) == 1:
        h = heads[0][1]
        ck(f"the panic is the witness's own verdict panic (run_c4_witness's verdict panic!, {pa['file']}:{pa['line']}:"
           f"{pa['col']}), not the fixture panic ({w['fixture_panic']['line']}), the healthy assertion or an unrelated "
           "panic", (h.group(2), int(h.group(3)), int(h.group(4))) == (pa["file"], pa["line"], pa["col"]),
           f"{h.group(2)}:{h.group(3)}:{h.group(4)}")
        msg = classify.panic_message(lines, heads[0][0])
        m = hdr.match(msg[0]) if msg else None
        n_vio = int(m.group(1)) if m else None
        ck(f"the C4 WITNESS F-8 header: '{w['header_prefix']}<n> violations', n at least 2 (G.5(B))",
           m is not None and n_vio >= 2, msg[:1])
        for i, name in enumerate(names):
            s = sc[name]
            ck(f"the exact positive line of {name} at its frozen locus, line {2 + 2 * i} of the verdict: "
               f"'{s['positive']}' (G.5(B))", msg[1 + 2 * i:2 + 2 * i] == [s["positive"]], msg[1 + 2 * i:2 + 2 * i])
            o = None
            if len(msg) > 2 + 2 * i and msg[2 + 2 * i].startswith(s["others_prefix"]):
                o = lit(msg[2 + 2 * i][len(s["others_prefix"]):])
            others[name] = o
            ck(f"the separate line of other properties of scenario {name}, each reported separately and not counted as "
               f"{name}: a list of property names without {name} (it never substitutes for {name})",
               isinstance(o, list) and name not in o and all(isinstance(x, str) for x in o), msg[2 + 2 * i:3 + 2 * i])
        k = 1 + 2 * len(names)
        vio = msg[k + 1:]
        ck("'every violation:' then exactly n violation lines (the header's count), each indented",
           msg[k:k + 1] == ["every violation:"] and n_vio is not None and len(vio) == n_vio
           and all(v.startswith("  ") and not v.startswith("   ") for v in vio), f"{len(vio)} lines; n {n_vio}")
        for name in names:
            s = sc[name]
            locus = f"  {name} step {s['total']} after: {name}: "
            ck(f"{name}'s violation is listed at its frozen locus ('{locus.strip()} …': the world after step "
               f"{s['total']} of scenario {name})", sum(v.startswith(locus) for v in vio) >= 1,
               [v[:70] for v in vio if f": {name}: " in v])
    ck("both exact positive lines occur exactly once in the whole output, each as a whole line; exactly two NAMED "
       "PROPERTY lines; no 'NOT OBSERVED'", all(lines.count(sc[n]["positive"]) == 1 and text.count(sc[n]["positive"])
                                                 == 1 for n in names)
       and sum("NAMED PROPERTY" in l for l in lines) == len(names) and "NOT OBSERVED" not in text)
    ck("no healthy line in a mutant-witness receipt (F.2; G.5(B)): no healthy scenario line, no 'scenarios healthy', no "
       "'no violation', no healthy refusal assertion", all(sc[n]["healthy_line"] not in text for n in names)
       and w["healthy_all"] not in text and "no violation" not in text and "WITNESS: no violation" not in text
       and "shows the healthy core's" not in text)
    ck("no WITNESS FIXTURE FAILURE; no quotient or symmetry refusal; no explorer report (NOT PRODUCED BY DESIGN for "
       "type (c))", "WITNESS FIXTURE FAILURE" not in text and "inapplicable" not in text and "EXPLORATION " not in text)
    body = body_lines(text, test)
    tr = transcript(body, w["config_name"])
    probs, facts = g6_check(tr, w, "mutant")
    ck("G.6 from the raw output, both scenarios, every step: the frozen actor order; the offered stream; the consumed "
       "choices, each equal to the offered prefix with its option count; no consumption beyond the offered stream (no "
       "silent Oracle-default zero); the printed unused suffix; the branch labels; the remaining fault budget and its "
       "[fault] marker; at each changed append exactly the frozen storage-choice stream with no unused suffix",
       not probs, probs[:4])
    loci = {}
    for name in names:
        s = sc[name]
        st = (tr["scenarios"].get(name) or {}).get("steps") or []
        last = st[-1] if len(st) == s["total"] else None
        loci[name] = last
        ck(f"scenario {name} step {s['total']}'s record prints {name} in the world after it (VIOLATION after: {name})",
           last is not None and any(pl == "after" and pr == name for pl, pr, _ in last["violations"]),
           last and [(pl, pr) for pl, pr, _ in last["violations"]])
        if name == "P4":
            ck("P4's locus in the printed world after step 8: U holds the acknowledgement while X holds an explanation "
               "(F1's, as the witness's own predicate requires; compared with the healthy receipt's X after the link)",
               last is not None and (last["cells"] or {}).get("U", "").startswith("Some(Acknowledgement {")
               and (last["cells"] or {}).get("X", "").startswith("Some((Explanation { original: "),
               last and {k: v[:40] for k, v in (last["cells"] or {}).items()})
        if name == "P8":
            ck("P8's locus in the printed world after step 12: G1's insert issued, its record written, not told "
               "Committed, not in U, no correction in C or in the ledger", last is not None and last["g1"] == {
                   "issued": True, "written": True, "told": False, "in_u": False, "in_c": False, "corrected": False},
               last and last["g1"])
        others_printed = sorted({pr for r in st for pl, pr, _ in r["violations"] if pr != name}
                                | {pr for pr, _ in (tr["scenarios"].get(name) or {}).get("initial", []) if pr != name})
        ck(f"scenario {name}'s line of other properties lists exactly the other properties printed in that scenario",
           others.get(name) == others_printed, f"{others.get(name)} / {others_printed}")
    vio_lines = sum(len(r["violations"]) for s in tr["scenarios"].values() for r in s["steps"])
    initial = sum(len(s["initial"]) for s in tr["scenarios"].values())
    ck("the header's count equals the printed per-step violation lines plus the initial states' ones",
       n_vio is not None and vio_lines + initial == n_vio, f"{vio_lines} printed + {initial} initial; header {n_vio}")
    good = ck.ok()
    seen = {n: (lines.count(sc[n]["positive"]) == 1) for n in names}
    if good:
        outer = ("FAIL — admissible type-(c) named-property mutant-witness receipt (G.5(B)): both "
                 + " and ".join(f"'{sc[n]['positive']}'" for n in names)
                 + f"; {w['header_prefix']}{n_vio} violations; G.6 verified from the raw output in both scenarios")
    elif proc["signal"] is not None or sup["timed_out"] or sup["oom_kill"]:
        outer = "INCOMPLETE (killed from outside or abnormal termination): establishes nothing"
    elif "WITNESS FIXTURE FAILURE" in text:
        outer = "NOT ADMISSIBLE (a WITNESS FIXTURE FAILURE: establishes nothing; G.5(B))"
    elif "NOT OBSERVED" in text:
        outer = "NOT ADMISSIBLE (a named property NOT OBSERVED at its locus: establishes nothing; G.5(B))"
    elif proc["exit_status"] == 0 or w["healthy_all"] in text:
        outer = "NOT ADMISSIBLE (a healthy result in a mutant-witness receipt: establishes nothing; F.2, G.5(B))"
    elif sum(seen.values()) == 1:
        outer = ("NOT ADMISSIBLE (only " + [n for n in names if seen[n]][0] + " shown: one property is insufficient; "
                 "G.5(B), K.2 case 9)")
    elif probs:
        outer = "NOT ADMISSIBLE (G.6 breached: " + "; ".join(probs[:2]) + "; K.2 case 14)"
    else:
        outer = "NOT ADMISSIBLE (" + "; ".join(ck.failed()) + "): establishes nothing"
    extra = [f"raw outcome (as printed): harness {result}; panic message {msg[:1 + 2 * len(names)]}"]
    extra += [f"positive line {n}: {sc[n]['positive']!r} — {'present at its frozen locus' if good else 'NOT ESTABLISHED'}"
              for n in names]
    extra += [f"executable: the pinned C4 mutant executable of the primary, {exe['path']} sha256 {exe['sha256']} size "
              f"{exe['size']}",
              "explorer report, counts, property lines and branch list: NOT PRODUCED BY DESIGN (F.1 (c))",
              "kill label: none (the witness is a part of G.5 only)", classify.resources_line(sup, limit)]
    classify.write(d, WITNESS_OK if good else WITNESS_NOT, outer, ck,
                   {"control": ctl["id"], "receipt_type": "AD-1-R4 F.1 (c), the G.5(B) mutant witness", "test": test,
                    "named_properties": names, "positive_lines": {n: sc[n]["positive"] for n in names},
                    "observed": seen, "header_violations": n_vio, "other_properties": others, "panic_message": msg,
                    "g6_problems": probs, "transcript": tr,
                    "harness_result": result, "process": proc, "supervisor": sup, "executable": desc["executable"],
                    "primary_executable": pdesc["executable"], "selection": desc["selection"],
                    "raw_output_sha256": desc["raw_output_sha256"],
                    "not_produced_by_design": "the explorer report, counts, property lines and branch list (F.1 (c))"},
                   extra + [f"G.6: {f}" for f in facts])
    return 0 if good else 1


# ---- G.5(C): the accepted healthy witness, linked; G.6 on it and against the mutant ---------------------------------

def link_healthy(ctl, outdir):
    os.makedirs(outdir)
    hw, w = ctl["g5"]["healthy"], ctl["g5"]["witness"]
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
    if names != want_names:
        return _write_healthy(outdir, ctl, checks, None)
    # A byte-changed copy has already failed above; every later check still runs on it, so that each defect is reported.
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
    for s in w["scenarios"]:
        chk(f"the exact healthy line of scenario {s['named']} (G.5(C)), exactly once, as a whole line: "
            f"'{s['healthy_line']}'", body.count(s["healthy_line"]) == 1 and raw.count(s["healthy_line"]) == 1)
    chk(f"the exact closing healthy line, exactly once: '{w['healthy_all']}'",
        body.count(w["healthy_all"]) == 1 and raw.count(w["healthy_all"]) == 1)
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {K.LISTED - 1} filtered out; finished in "
    res = re.findall(r"^test result: .*$", raw, re.M)
    chk("harness: 'ok', one test, 1 passed, 0 failed", len(res) == 1 and res[0].startswith(want)
        and re.search(r"\nok\n\ntest result: ok\.", raw) is not None, res)
    marks = [l for l in body if any(m in l for m in POSITIVE_MARKERS)]
    chk("no positive marker, violation, fixture failure or panic in the healthy receipt (F.2)", not marks, marks[:2])
    tr = transcript(body, w["config_name"])
    probs, facts = g6_check(tr, w, "healthy")
    chk("G.6 on the healthy transcript, both scenarios, every step: the frozen actor order, offered streams, consumption "
        "with option counts, unused suffixes, branches and fault budgets; at each changed append the healthy refusal "
        "before storage, consuming 0 of its offered choices, the whole offered stream unused", not probs, probs[:4])
    chk("no violation anywhere on the healthy core (initial states and every step)",
        not any(s["initial"] or any(r["violations"] for r in s["steps"]) for s in tr["scenarios"].values()))
    hp4 = (tr["scenarios"].get("P4") or {}).get("steps") or []
    hp8 = (tr["scenarios"].get("P8") or {}).get("steps") or []
    chk("scenario P4, step 8 on the healthy core: U stays empty and X holds an explanation (F1's, by the accepted "
        "receipt's own kept-predicate)", len(hp4) == 8 and (hp4[-1]["cells"] or {}).get("U") == "None"
        and (hp4[-1]["cells"] or {}).get("X", "").startswith("Some((Explanation { original: "),
        hp4 and {k: v[:40] for k, v in (hp4[-1]["cells"] or {}).items()})
    chk("scenario P8, step 12 on the healthy core: G1's written attempt stays in U (issued, written, not told Committed, "
        "in U)", len(hp8) == 12 and (hp8[-1]["g1"] or {}).get("issued") is True and hp8[-1]["g1"]["written"] is True
        and hp8[-1]["g1"]["told"] is False and hp8[-1]["g1"]["in_u"] is True, hp8 and hp8[-1]["g1"])
    wpar = os.path.join(os.path.dirname(outdir), "04c-witness", "PARSED-RECEIPT.json")
    mtr = json.load(open(wpar)).get("transcript") if os.path.exists(wpar) else None
    cmp_probs = []
    if not mtr:
        cmp_probs.append("this control's mutant transcript is missing")
    else:
        for s in w["scenarios"]:
            n = s["named"]
            hs, ms = (tr["scenarios"].get(n) or {}).get("steps") or [], (mtr["scenarios"].get(n) or {}).get("steps") or []
            if len(hs) != s["total"] or len(ms) != s["total"]:
                cmp_probs.append(f"scenario {n}: {len(hs)} healthy and {len(ms)} mutant steps, not {s['total']}")
                continue
            for i, (a, b) in enumerate(zip(hs, ms)):
                if (a["actor"], a["offered"]) != (b["actor"], b["offered"]):
                    cmp_probs.append(f"scenario {n} step {i + 1}: actor/offered {a['actor']} {a['offered']} healthy, "
                                     f"{b['actor']} {b['offered']} mutant")
                if i + 1 < s["total"]:
                    ka = ([tuple(x) for x in a["consumed"]], a["unused"], a["branches"], a["faults"], a["marker"])
                    kb = ([tuple(x) for x in b["consumed"]], b["unused"], b["branches"], b["faults"], b["marker"])
                    if ka != kb:
                        cmp_probs.append(f"scenario {n} step {i + 1} (before the changed append): healthy {ka} mutant {kb}")
            if n == "P4" and len(hs) == len(ms) == s["total"] and hs[-1]["cells"] and ms[-1]["cells"] \
                    and hs[-1]["cells"]["X"] != ms[-1]["cells"]["X"]:
                cmp_probs.append("scenario P4 step 8: the mutant's X differs from the healthy core's (F1's explanation)")
    chk("G.6 items 1 and 2: healthy and mutant transcripts receive the same actor sequence and the same offered stream "
        "at every step; before the changed append they agree on consumption, option counts, unused suffix, branch "
        "labels, remaining faults and [fault] markers; at P4 step 8 the mutant's X is the healthy core's (F1's "
        "explanation)", not cmp_probs, cmp_probs[:3])
    return _write_healthy(outdir, ctl, checks, hw, facts)


def _write_healthy(outdir, ctl, checks, hw, facts=()):
    ok = bool(checks) and all(o for _, o, _ in checks)
    ref = (f"{hw['directory']} at {hw['commit']} (accepted healthy-witness type-(e3) PASS on the pinned unchanged-"
           f"baseline executable {K.EXE_SHA256})" if hw else "not linked")
    lines = [HEALTHY_OK if ok else HEALTHY_FAIL, f"control {ctl['id']}: healthy-witness receipt {ref}",
             "linked (never rerun) only after this control's own restoration was verified"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    lines += [f"G.6 healthy: {f}" for f in facts]
    open(os.path.join(outdir, "HEALTHY.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "type": "e3", "reference": ref, "checks": checks, "g6_facts": list(facts)},
              open(os.path.join(outdir, "HEALTHY.json"), "w"), indent=1)
    print("\n".join(lines))
    return ok, ref


# ---- G.5 (A)-(E): the obligations, from this control's own receipts; (F) proposed ----------------------------------

def obligations(ctl, outdir):
    os.makedirs(outdir)
    base = os.path.dirname(outdir)
    g = ctl["g5"]
    checks = []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))

    def first(rel):
        return supervise.first_line(os.path.join(base, rel))

    def jl(rel):
        p = os.path.join(base, rel)
        return json.load(open(p)) if os.path.exists(p) else {}
    pr, wr = jl("04-run/PARSED-RECEIPT.json"), jl("04c-witness/PARSED-RECEIPT.json")
    printed = pr.get("printed_property")
    chk("(A) the primary is an admissible type-(a) FAIL of full_f8 on the original explorer within F-8's 30,000,000 "
        "ceiling, its first printed property P4 or P8, recorded as printed (never relabelled)",
        first("04-run/CLASSIFICATION.txt") == PRIMARY_OK and printed in g["permitted_primary"]
        and (pr.get("report") or {}).get("states", 1 << 62) <= ctl["ceiling_states"], printed)
    chk("(B) the mutant witness is an admissible type-(c) receipt showing BOTH exact positive lines at their frozen loci "
        "(one alone is insufficient)", first("04c-witness/CLASSIFICATION.txt") == WITNESS_OK
        and wr.get("positive_lines") == {s["named"]: s["positive"] for s in g["witness"]["scenarios"]}
        and wr.get("observed") == {"P4": True, "P8": True}, first("04c-witness/CLASSIFICATION.txt"))
    chk("(B) G.6 holds on the mutant transcript (verified from the raw output: no G.6 problem)",
        wr.get("g6_problems") == [], (wr.get("g6_problems") or [])[:2])
    pe, we = pr.get("executable", {}), wr.get("executable", {})
    pin = jl("03-build/PIN.json")
    chk("(B) F.2: the witness ran on the same pinned C4 mutant executable as the primary (path, sha256 and size, "
        "before and after both runs, equal to 03-build/PIN.json)",
        pin and all(x.get("path") == pin["path"] and x.get("sha256") == pin["sha256"] and x.get("size") == pin["size"]
                    for x in (pe.get("before", {}), pe.get("after", {}), we.get("before", {}), we.get("after", {}))),
        pin.get("sha256"))
    chk("(B) none of the witness rejections: every check of the witness receipt holds (CHECKS.txt: no FAIL line)",
        os.path.exists(os.path.join(base, "04c-witness/CHECKS.txt")) and not [
            l for l in open(os.path.join(base, "04c-witness/CHECKS.txt")).read().splitlines() if l.startswith("FAIL")])
    chk("(E) restoration: the worktree restored exactly to C after the mutant receipts (RESTORE: EXACTLY_C)",
        first("05-restore/RESTORE.txt") == "RESTORE: EXACTLY_C")
    chk("(C) the accepted healthy type-(e3) witness linked after the restoration and verified, G.6 on it and against the "
        "mutant included", first("06a-healthy-witness/HEALTHY.txt") == HEALTHY_OK)
    chk("(D) the accepted F-8 unchanged-core type-(e1) receipt linked after the restoration and verified",
        first("06b-link/LINK.txt") == "LINK: VERIFIED")
    ap = jl("02-apply/APPLY.json")
    rs = jl("05-restore/RESTORE.json")
    chk("(E) identity (AD-1-R4 I items 1-7): Candidate C and its tree; C4, the frozen patch and its sha256; the mutant "
        "blob and sha256 as built; the pinned executable's sha256 and size; the exact invocations; the raw receipts' "
        "sha256; the restored blob and the clean worktree",
        ap.get("patch_sha256") == ap.get("catalog_patch_sha256") == ctl["patch"]["sha256"]
        and (ap.get("verify_mutant") or {}).get("verdict") == "EXACTLY_C_PLUS_PATCH"
        and ((ap.get("verify_mutant") or {}).get("mutant_file") or {}).get("git_blob") == ctl["mutant_blob"]
        and ((ap.get("verify_mutant") or {}).get("mutant_file") or {}).get("sha256") == ctl["mutant_sha256"]
        and bool(pin.get("sha256")) and bool(pr.get("raw_output_sha256")) and bool(wr.get("raw_output_sha256"))
        and (rs.get("after") or {}).get("git_blob") == ctl["baseline_blob"]
        and (rs.get("before") or {}).get("git_blob") == ctl["mutant_blob"], "")
    ok = all(o for _, o, _ in checks)
    rec = g["recording_template"].format(printed=printed) if ok else None
    lines = [G5_OK if ok else G5_FAIL, f"control {ctl['id']}: AD-1-R4 G.5 (kill criterion for C4 only; P4 AND P8)"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    lines.append("(E) the Sole Architect's independent review: PENDING (never claimed by the executor)")
    if ok:
        lines.append(f"(F) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance): '{rec}'")
    else:
        lines.append("(F) no recording: G.5 is not established")
    open(os.path.join(outdir, "G5.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "holds_A_to_D_and_E_identity": ok, "checks": checks,
               "E_architect_review": "PENDING", "first_printed_property": printed,
               "recording_proposed_for_architect_review": rec},
              open(os.path.join(outdir, "G5.json"), "w"), indent=1)
    print("\n".join(lines))
    return ok
