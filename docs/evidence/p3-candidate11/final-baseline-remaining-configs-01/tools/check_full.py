#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission sections 9, 10 and 12: parse and classify one remaining
configuration's run.

  check_full.py fg|fb2|f4|fw|f8|f5|f6|f7|fd|fr [RECEIPT-DIR]
      reads RECEIPT-DIR (default: the mission's receipt directory for that key); writes there PARSED-RECEIPT.json,
      CHECKS.txt, CLASSIFICATION.txt (first line `CLASSIFICATION: PASS` only if every check holds) and the run's
      BRANCH-ACCOUNTING-<CFG>.json/.md over the frozen vocabulary

Keeps two facts apart (AD-1-R4 K.2): the raw outcome, exactly as printed, and the outer admissibility class. Every
expected value is source-derived (configs.py: bounds, actors, cap, property applicability, NOT APPLICABLE
operations) or the mission's (section 9). No expected state count exists or is asserted. The report is parsed by
Stage 1's report.py parse_report, unchanged.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402
import configs  # noqa: E402
import normalize  # noqa: E402
import report  # noqa: E402

LISTED = 348
CAP = 30_000_000
LIMIT = 2700
KEYS = {"fg": ("F-G", "receipts/60-full-F-G"), "fb2": ("F-B2", "receipts/61-full-F-B2"),
        "f4": ("F-4", "receipts/62-full-F-4"), "fw": ("F-W", "receipts/63-full-F-W"),
        "f8": ("F-8", "receipts/64-full-F-8"), "f5": ("F-5", "receipts/65-full-F-5"),
        "f6": ("F-6", "receipts/66-full-F-6"), "f7": ("F-7", "receipts/67-full-F-7"),
        "fd": ("F-D", "receipts/68-full-F-D"), "fr": ("F-R", "receipts/69-full-F-R")}
# Mission section 9, property applicability, stated independently of configs.py and compared with it.
MISSION_HOLDS = {"F-W": 25, "F-D": 23}
L1_HOLDS = re.compile(r"^HOLDS: (\d+) non-fault edges, no cycle; (\d+) end states, each terminal \(D1\)$")


def main(key, d=None):
    cfg, rdir = KEYS[key]
    d = d or os.path.join(common.EV, rdir)
    if os.path.exists(os.path.join(d, "CLASSIFICATION.txt")):
        raise SystemExit("REFUSED: already classified")
    checks = []
    check = lambda label, ok, detail="": checks.append((label, bool(ok), str(detail)))
    derived = configs.derive()
    check("configuration expectations derive from the source at C (every anchor holds)", not derived["problems"],
          derived["problems"])
    req = derived["required"][cfg]
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    started = "not_started" not in desc
    check("the run started (no pre-flight refusal)", started, desc.get("not_started", ""))
    if not started:
        return write(d, cfg, req, checks, desc, None, None, None, "NOT ADMISSIBLE (not started: %s)" % desc["not_started"])
    proc, sup, exe = desc["process"], desc["supervisor"], desc["executable"]
    raw = open(os.path.join(d, "raw-output.txt"), "rb").read()
    text = raw.decode("utf-8", errors="replace")

    # K.2 (1): identity.
    ids = [exe["before"], desc["selection"]["executable_after_listing"], exe["after"]]
    check("executable is the pinned one (path, sha256, size) before, at selection and after",
          exe["path"] == common.EXE and all(i["sha256"] == common.EXE_SHA256 and i["size"] == common.EXE_SIZE
                                             and not i["symlink"] for i in ids),
          " / ".join(f"{i['sha256'][:16]}… {i['size']}" for i in ids))
    check("source exactly C before and after", desc["source_before"]["verdict"] == "EXACTLY_C"
          and desc["source_after"]["verdict"] == "EXACTLY_C",
          f"{desc['source_before']['verdict']} / {desc['source_after']['verdict']}")
    check("raw output sha256 equals the launcher's record", common.sha256_bytes(raw) == desc["raw_output_sha256"],
          common.sha256_bytes(raw))
    # K.2 (2): selection.
    check("argv: the pinned executable, exactly the one test, --exact --ignored --nocapture --test-threads=1",
          desc["argv"] == [common.EXE, req["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
          desc["argv"])
    check("the test runs this configuration with the original explorer (full_exploration, source-derived)",
          req["runner"] == "full_exploration" and req["explorer"] == "original" and req["ignored"],
          f"{req['runner']} ({req['explorer']}), ignored {req['ignored']}")
    sel = desc["selection"]["listing"]
    check("pre-run listing (--exact --ignored --list) selects exactly that one test",
          sel["exit"] == 0 and sel["listed"] == [req["test"] + ": test"], sel["listed"])
    starts = re.findall(r"^test (\S+) \.\.\. ", text, re.M)
    check("the harness ran exactly that one test", re.findall(r"^running (\d+) tests?$", text, re.M) == ["1"]
          and starts == [req["test"]], f"running {re.findall(r'^running (.*)$', text, re.M)}; started {starts}")
    # K.2 (4): harness result.
    result = re.findall(r"^test result: .*$", text, re.M)
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {LISTED - 1} filtered out; finished in "
    check(f"harness: 'ok' then '{want}…'", len(result) == 1 and result[0].startswith(want)
          and re.search(r"\n\nok\n\ntest result: ok\.", text), result)
    # K.2 (5)-(7): process and launching command.
    check("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
          f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check("launching command: 16 GiB, zero swap, enforced; no OOM, no kill, no timeout; complete record; within 45 min",
          sup["completed"] and sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and sup["oom_kill"] == 0
          and sup["swap_peak_bytes"] == "0" and not sup["timed_out"] and sup["time_limit_seconds"] == LIMIT
          and sup["wall_seconds"] is not None and sup["wall_seconds"] < LIMIT,
          f"memory.peak {sup['memory_peak_bytes']} B, swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, oom_kill "
          f"{sup['oom_kill']}, wall {sup['wall_seconds']} s of {LIMIT} s, scope {sup['scope_result']}")
    refusals = [l for l in text.splitlines() if "inapplicable" in l]
    check("no quotient or symmetry refusal", not refusals, refusals[:3])
    check("no panic, no fixture failure", "panicked" not in text and "WITNESS FIXTURE FAILURE" not in text)

    # The report.
    rep = report.parse_report(text)
    check("exactly one exploration report", rep is not None and text.count("EXPLORATION ") == 1, "")
    if rep is None:
        return write(d, cfg, req, checks, desc, None, text, None, classify(checks, None))
    check("header: 'EXPLORATION <cfg>' exactly (the original explorer prints no quotient or symmetry note)",
          rep["header"] == f"EXPLORATION {cfg}", rep["header"])
    check("original explorer: no quotient line, no symmetry line", "quotient_line" not in rep
          and "symmetry_line" not in rep)
    check("outcome PASS", rep.get("outcome") == "PASS", rep.get("outcome"))
    check("no INCOMPLETE line, no FAIL line", rep["incomplete"] is None and rep["fail"] is None,
          f"incomplete {rep['incomplete']}, fail {rep['fail']}")
    check("printed bounds equal the source-derived configuration", rep.get("bounds") == req["bounds"], rep.get("bounds"))
    check("printed actors equal actors() of the source-derived configuration", rep.get("actors") == req["actors"],
          rep.get("actors"))
    check(f"cap printed equals the authorised ceiling ({CAP:,} states, {LIMIT} s) and the configuration's",
          rep.get("cap") == CAP == req["config"]["cap"] and rep.get("time_cap_s") == LIMIT == req["config"]["time"],
          f"{rep.get('cap')}, {rep.get('time_cap_s')} s")
    have = ("states", "expanded", "transitions", "fault_edges", "no_step")
    counts_ok = all(k in rep for k in have)
    check("counts printed", counts_ok, "")
    if counts_ok:
        check(f"discovered states <= {CAP:,} (K.1, inclusive)", rep["states"] <= CAP, f"{rep['states']:,}")
        check("expanded states equal discovered states (0 unexpanded)", rep["expanded"] == rep["states"],
              f"states {rep['states']}, expanded {rep['expanded']}")
    # Properties (Q), by source-derived applicability, compared with the mission's statement.
    expect = req["properties"]
    names = [k for k, _ in rep["properties"]]
    check("one line for each of the 25 PROPERTIES, in order", names == configs.PROPERTIES, names)
    pv = dict(rep["properties"])
    bad = []
    for p, form in expect.items():
        got = pv.get(p)
        if form == "L1-HOLDS":
            m = L1_HOLDS.match(got or "")
            if not m:
                bad.append((p, got))
            elif counts_ok and (int(m.group(1)) != rep["transitions"] - rep["fault_edges"]
                                or int(m.group(2)) != rep["no_step"]):
                bad.append((p, f"{got} (inconsistent with the counts)"))
        elif got != form:
            bad.append((p, got))
    check("every property line has its source-derived applicable form", not bad, bad)
    holds = sum(1 for p, v in rep["properties"] if v == "HOLDS" or (p == "L1" and L1_HOLDS.match(v)))
    n_expect = sum(f in ("HOLDS", "L1-HOLDS") for f in expect.values())
    check(f"{n_expect} lines HOLD (mission section 9: {MISSION_HOLDS.get(cfg, 24)})",
          holds == n_expect == MISSION_HOLDS.get(cfg, 24), f"{holds} HOLD")
    check("KEY HOLDS", pv.get("KEY") == "HOLDS", pv.get("KEY"))
    check("D1 HOLDS", pv.get("D1") == "HOLDS", pv.get("D1"))
    check("L1: " + ("HOLDS" if cfg != "F-D" else "'not claimed for this configuration'"),
          (L1_HOLDS.match(pv.get("L1", "")) is not None) if cfg != "F-D" else
          pv.get("L1") == "not claimed for this configuration", pv.get("L1"))
    check("E10: " + ("HOLDS" if cfg == "F-W" else "'not applicable here (worker verbs do not unwind)'"),
          pv.get("E10") == ("HOLDS" if cfg == "F-W" else "not applicable here (worker verbs do not unwind)"),
          pv.get("E10"))
    check("state key fields line present", rep["key_fields"] is not None, "")
    # Branch accounting over the frozen vocabulary (mission section 10).
    vocab, vsha = normalize.load_vocab()
    table, problems = normalize.normalize(text, cfg, vocab, derived,
                                          f"this mission's raw output {os.path.join(d, 'raw-output.txt')}, sha256 "
                                          f"{common.sha256_bytes(raw)}")
    if table is not None:
        table["vocabulary_sha256"] = vsha
        normalize.write(table, d, f"BRANCH-ACCOUNTING-{cfg}")
    check("branch list complete and every raw label resolved to the frozen vocabulary; no NOT APPLICABLE operation "
          "taken (normalizer: NORMALIZED)", table is not None and not problems, problems)
    return write(d, cfg, req, checks, desc, rep, text, table, classify(checks, rep))


def classify(checks, rep):
    """AD-1-R4 K.1/K.2 outer class, fail-closed."""
    failed = [l for l, ok, _ in checks if not ok]
    if not failed:
        return "PASS — admissible unchanged-core original-explorer PASS"
    if any("refusal" in l for l in failed):
        return "INCOMPLETE — VERIFICATION INVALID (quotient or symmetry refusal)"
    if rep is not None and rep.get("outcome") == "FAIL":
        if rep.get("states", 0) > CAP:
            return "INCOMPLETE — BUDGET EXCEEDED (FAIL printed beyond the authorised cardinality)"
        return f"FAIL (raw; property {rep.get('fail')}); admissibility per the failed checks"
    if rep is not None and rep.get("outcome") == "INCOMPLETE":
        return f"INCOMPLETE ({rep.get('incomplete')})"
    if any(l.startswith("normal exit") or l.startswith("launching command") for l in failed):
        return "INCOMPLETE (killed from outside or abnormal termination; never PASS)"
    return "NOT ADMISSIBLE (" + "; ".join(failed) + ")"


def write(d, cfg, req, checks, desc, rep, text, table, outer):
    good = all(ok for _, ok, _ in checks)
    keep = ("header", "outcome", "bounds", "actors", "states", "expanded", "transitions", "fault_edges", "no_step",
            "elapsed_s", "tables_mib", "cap", "time_cap_s", "incomplete", "fail", "trace_steps", "properties",
            "branches", "extra_branches")
    parsed = {"configuration": cfg, "test": req["test"], "explorer": "original", "receipt_type": "AD-1-R4 F.1 (e1)",
              "expected": {k: req[k] for k in ("bounds", "actors", "properties", "na")},
              "report": {k: rep.get(k) for k in keep} if rep else None,
              "harness_result": (re.findall(r"^test result: .*$", text, re.M) if text else None),
              "process": desc.get("process"), "supervisor": desc.get("supervisor"),
              "executable": desc.get("executable"), "selection": desc.get("selection"),
              "raw_output_sha256": desc.get("raw_output_sha256"), "outer_classification": outer,
              "branch_accounting": ({k: table[k] for k in ("verdict", "totals", "taken_outside_static", "operations")}
                                    if table else None),
              "original_graph_explored": True}
    if rep and rep.get("fault_edges") is not None:
        parsed["report"]["nonfault_edges"] = rep["transitions"] - rep["fault_edges"]
    json.dump(parsed, open(os.path.join(d, "PARSED-RECEIPT.json"), "w"), indent=1)
    with open(os.path.join(d, "CHECKS.txt"), "w") as f:
        for label, ok, detail in checks:
            f.write(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}\n")
    with open(os.path.join(d, "CLASSIFICATION.txt"), "w") as f:
        f.write(f"CLASSIFICATION: {'PASS' if good else 'NOT PASS'}\n")
        f.write(f"outer class (AD-1-R4 K.2): {outer}\n")
        if rep:
            f.write(f"raw outcome (as printed): outcome {rep.get('outcome')}; "
                    f"{'FAIL ' + rep['fail'] if rep.get('fail') else ''}"
                    f"{'INCOMPLETE ' + rep['incomplete'] if rep.get('incomplete') else ''}"
                    f"; harness {parsed['harness_result']}\n")
            if all(k in rep for k in ("states", "expanded", "transitions", "fault_edges", "no_step")):
                f.write(f"measured: states {rep['states']:,} (expanded {rep['expanded']:,}); transitions "
                        f"{rep['transitions']:,} (with a fault {rep['fault_edges']:,}; non-fault "
                        f"{rep['transitions'] - rep['fault_edges']:,}); no enabled step {rep['no_step']:,}; elapsed "
                        f"{rep.get('elapsed_s')} s; explorer tables {rep.get('tables_mib')} MiB\n")
        sup = desc.get("supervisor") or {}
        if sup:
            f.write(f"resources: wall {sup.get('wall_seconds')} s of {LIMIT} s; scope memory.peak "
                    f"{sup.get('memory_peak_bytes')} B; swap peak {sup.get('swap_peak_bytes')} B; oom_kill "
                    f"{sup.get('oom_kill')}; largest process max RSS {sup.get('max_rss_kib_largest_process')} KiB\n")
        if table:
            f.write(f"branch accounting: {table['verdict']}; TAKEN {table['totals']['TAKEN']}, NOT TAKEN "
                    f"{table['totals']['NOT TAKEN']}, NOT APPLICABLE {table['totals']['NOT APPLICABLE']}; taken "
                    f"outside the static BRANCHES {table['taken_outside_static']}\n")
        f.write(f"checks: {sum(ok for _, ok, _ in checks)} of {len(checks)} hold\n")
        for label, ok, detail in checks:
            if not ok:
                f.write(f"FAILED CHECK: {label}: {detail}\n")
    print(open(os.path.join(d, "CHECKS.txt")).read())
    print(open(os.path.join(d, "CLASSIFICATION.txt")).read())
    return 0 if good else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
