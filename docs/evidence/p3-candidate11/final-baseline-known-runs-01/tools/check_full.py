#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission sections 7 to 12 and 14: parse and classify one full run.

  check_full.py f2|fb|f3|fpt|f1      reads receipts/4n-full-<CFG>/; writes there PARSED-RECEIPT.json, CHECKS.txt and
                                     CLASSIFICATION.txt (first line `CLASSIFICATION: PASS` only if every check holds)

Keeps two facts apart (AD-1-R4 K.2): the raw outcome, exactly as printed, and the outer admissibility class. The
expected values below are the mission's (sections 8 to 12) and AD-1-R4 N.9/N.10; they were written before the run.
"""
import hashlib
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import report  # noqa: E402

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
PIN = json.load(open(os.path.join(EV, "receipts/10-build/PIN.json")))
P = "ledger::protocol::model_tests::"
LISTED = 348
COMMON = ("gating conflict false acknowledgement actors 0 attempts each 0 malformed snapshots false cell constraint "
          "false worker verbs unwind false DROP false REPLAY 0 A4 {a4}; ticket limit 8, refusal limit 4; settlement and "
          "the worker on; caps {cap} states, 2700s")
F3_ACTORS = "[Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env]"
FB_ACTORS = "[Fw(0), Se, Wk, Gc(0), Gc(1), Adm, Env]"
F12_ACTORS = "[Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm]"
RUNS = {
    "f2": dict(dir="receipts/41-full-F-2", test=P + "full_f2", cfg="F-2", explorer="original",
               header="EXPLORATION F-2", cap=30_000_000, states=12_602_670, actors=F12_ACTORS,
               bounds="facts 2 conflicting [false, true] ENV false batch false faults 2 admissions 2 gating callers 2 "
                      + COMMON.format(a4="false", cap=30000000)),
    "fb": dict(dir="receipts/42-full-F-B", test=P + "gc_symmetry_fb", cfg="F-B", explorer="composed",
               header="EXPLORATION F-B " + report.COMPOSED, cap=30_000_000, keys=3_775_373, kf=156_463,
               derived=22_182_849, actors=FB_ACTORS,
               bounds="facts 2 conflicting [true, true] ENV true batch true faults 2 admissions 2 gating callers 2 "
                      + COMMON.format(a4="false", cap=30000000)),
    "f3": dict(dir="receipts/43-full-F-3", test=P + "gc_symmetry_f3", cfg="F-3", explorer="composed",
               header="EXPLORATION F-3 " + report.COMPOSED, cap=30_000_000, keys=20_431_851, kf=880_778,
               derived=119_948_772, actors=F3_ACTORS,
               bounds="facts 2 conflicting [true, true] ENV true batch false faults 2 admissions 2 gating callers 2 "
                      + COMMON.format(a4="false", cap=30000000)),
    "fpt": dict(dir="receipts/44-full-F-PT", test=P + "gc_symmetry_fpt", cfg="F-PT", explorer="composed",
                header="EXPLORATION F-PT " + report.COMPOSED, cap=30_000_000, keys=29_751_473, kf=1_698_028,
                derived=173_414_754, actors=F3_ACTORS, margin=248_527,
                bounds="facts 2 conflicting [true, true] ENV true batch false faults 2 admissions 2 gating callers 2 "
                       + COMMON.format(a4="true", cap=30000000)),
    "f1": dict(dir="receipts/45-full-F-1", test=P + "full_f1", cfg="F-1", explorer="original",
               header="EXPLORATION F-1", cap=60_000_000, states=46_524_336, actors=F12_ACTORS,
               bounds="facts 2 conflicting [false, false] ENV false batch false faults 3 admissions 2 gating callers 2 "
                      + COMMON.format(a4="false", cap=60000000)),
}
N_ADMISSIONS = 2


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main(key):
    run = RUNS[key]
    d = os.path.join(EV, run["dir"])
    if os.path.exists(os.path.join(d, "CLASSIFICATION.txt")):
        raise SystemExit("REFUSED: already classified")
    checks = []
    check = lambda label, ok, detail="": checks.append((label, bool(ok), str(detail)))
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    started = "not_started" not in desc
    check("the run started (no pre-flight refusal)", started, desc.get("not_started", ""))
    if not started:
        return write(d, key, run, checks, desc, None, None, "NOT ADMISSIBLE (not started: %s)" % desc["not_started"])
    proc, sup, exe = desc["process"], desc["supervisor"], desc["executable"]
    raw = open(os.path.join(d, "raw-output.txt"), "rb").read()
    text = raw.decode("utf-8", errors="replace")
    props, branches = report.model_constants()

    # K.2 (1): identity.
    check("executable is the pinned one before, at selection and after", exe["path"] == PIN["executable"]
          and exe["sha256_before"] == exe["sha256_after"] == desc["selection"]["sha256_after"] == PIN["sha256"],
          f"{exe['sha256_before']} / {desc['selection']['sha256_after']} / {exe['sha256_after']}")
    check("source exactly C before and after", desc["source_before"]["verdict"] == "EXACTLY_C"
          and desc["source_after"]["verdict"] == "EXACTLY_C",
          f"{desc['source_before']['verdict']} / {desc['source_after']['verdict']}")
    check("raw output sha256 equals the launcher's record", sha256(raw) == desc["raw_output_sha256"], sha256(raw))
    # K.2 (2): selection.
    check("argv: exactly the one test, --exact --ignored --nocapture --test-threads=1, pinned executable",
          desc["argv"] == [PIN["executable"], run["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
          desc["argv"])
    check("pre-run listing selects exactly that one test", desc["selection"]["listed"] == [run["test"] + ": test"]
          and desc["selection"]["exit"] == 0, desc["selection"]["listed"])
    starts = re.findall(r"^test (\S+) \.\.\. ", text, re.M)
    check("the harness ran exactly that one test", re.findall(r"^running (\d+) tests?$", text, re.M) == ["1"]
          and starts == [run["test"]], f"running {re.findall(r'^running (.*)$', text, re.M)}; started {starts}")
    # K.2 (4): harness result.
    result = re.findall(r"^test result: .*$", text, re.M)
    want = (f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {LISTED - 1} filtered out; finished in ")
    check("harness: 'ok' and 'test result: ok. 1 passed; 0 failed; ... 347 filtered out'",
          len(result) == 1 and result[0].startswith(want) and re.search(r"\n\nok\n\ntest result: ok\.", text),
          result)
    # K.2 (5), (6): process and launching command.
    check("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
          f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check("launching command: 16 GiB, zero swap, enforced; no OOM, no kill, no timeout; complete record",
          sup["completed"] and sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and sup["oom_kill"] == 0
          and sup["swap_peak_bytes"] == "0" and not sup["timed_out"] and sup["time_limit_seconds"] == 2700
          and sup["wall_seconds"] is not None and sup["wall_seconds"] < 2700,
          f"memory.peak {sup['memory_peak_bytes']} B, swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, oom_kill "
          f"{sup['oom_kill']}, wall {sup['wall_seconds']} s of 2700 s, scope {sup['scope_result']}")
    # Refusals and panics.
    refusals = [l for l in text.splitlines() if "quotient inapplicable" in l or "symmetry inapplicable" in l]
    check("no quotient or symmetry refusal", not refusals, refusals[:3])
    check("no panic", "panicked at" not in text and "WITNESS FIXTURE FAILURE" not in text, "")
    # K.2 (3), (7): the report.
    rep = report.parse_report(text)
    check("one exploration report", rep is not None and text.count("EXPLORATION ") == 1, "")
    if rep is None:
        return write(d, key, run, checks, desc, None, text, classify(checks, None, run))
    check("header names the configuration and explorer", rep["header"] == run["header"], rep["header"])
    check("printed bounds equal the configuration", rep.get("bounds") == run["bounds"], rep.get("bounds"))
    check("printed actors equal the configuration", rep.get("actors") == run["actors"], rep.get("actors"))
    for label, ok, detail in report.check_pass(rep, props, branches):
        check(label, ok, detail)
    check("cap printed equals the authorised ceiling", rep.get("cap") == run["cap"] and rep.get("time_cap_s") == 2700,
          f"{rep.get('cap')}, {rep.get('time_cap_s')} s")
    count_ok = True
    if run["explorer"] == "original":
        check("original explorer: no quotient or symmetry line", "quotient_line" not in rep and "symmetry_line" not in rep)
        count_ok = rep.get("states") == run["states"] and rep.get("expanded") == run["states"]
        check(f"COUNT: states exactly {run['states']:,} and expanded equal", count_ok,
              f"states {rep.get('states')}, expanded {rep.get('expanded')}")
    else:
        check("composed quotient: the admission line and the symmetry line are printed",
              "quotient_line" in rep and "symmetry_line" in rep, "")
        check("admission observer evaluated at every expanded key", rep.get("evaluations") == rep.get("expanded"),
              f"{rep.get('evaluations')} evaluations, {rep.get('expanded')} expanded")
        count_ok = (rep.get("states") == run["keys"] and rep.get("expanded") == run["keys"]
                    and rep.get("self_mirrored") == run["kf"])
        check(f"COUNT: quotient keys exactly {run['keys']:,}, 0 unexpanded, {run['kf']:,} self-mirrored", count_ok,
              f"keys {rep.get('states')}, expanded {rep.get('expanded')}, self-mirrored {rep.get('self_mirrored')}")
        if "states" in rep and "self_mirrored" in rep:
            derived = (N_ADMISSIONS + 1) * (2 * rep["states"] - rep["self_mirrored"])
            check(f"DERIVED original states (N+1)(2K-Kf) = {run['derived']:,}", derived == run["derived"], derived)
            rep["derived_original_states"] = derived
        if "margin" in run:
            check(f"reduced-key margin below 30,000,000 = {run['margin']:,}",
                  run["cap"] - rep.get("states", 0) == run["margin"], run["cap"] - rep.get("states", 0))
    return write(d, key, run, checks, desc, rep, text, classify(checks, rep, run, count_ok))


def classify(checks, rep, run, count_ok=True):
    """AD-1-R4 K.1/K.2 outer class, fail-closed."""
    failed = [l for l, ok, _ in checks if not ok]
    if not failed:
        return ("PASS — admissible unchanged-core " +
                ("original-explorer PASS" if run["explorer"] == "original" else "composed-quotient PASS"))
    if any("refusal" in l for l in failed):
        return "INCOMPLETE — VERIFICATION INVALID (quotient or symmetry refusal)"
    if rep is not None and rep.get("outcome") == "FAIL":
        if rep.get("states", 0) > run["cap"]:
            return "INCOMPLETE — BUDGET EXCEEDED (FAIL printed beyond the authorised cardinality)"
        return f"FAIL (raw; property {rep.get('fail')}); admissibility per the failed checks"
    if rep is not None and rep.get("outcome") == "INCOMPLETE":
        return f"INCOMPLETE ({rep.get('incomplete')})"
    if any(l.startswith("normal exit") or l.startswith("launching command") for l in failed):
        return "INCOMPLETE (killed from outside or abnormal termination; never PASS)"
    if not count_ok and all(l.startswith("COUNT") or l.startswith("DERIVED") or l.startswith("reduced-key")
                            for l in failed):
        return "COUNT DIVERGENCE (otherwise a complete PASS; not silently accepted)"
    return "NOT ADMISSIBLE (" + "; ".join(failed) + ")"


def write(d, key, run, checks, desc, rep, text, outer):
    good = all(ok for _, ok, _ in checks)
    keep = ("header", "outcome", "bounds", "actors", "quotient_line", "evaluations", "admitted", "symmetry_line",
            "self_mirrored", "states", "expanded", "transitions", "fault_edges", "no_step", "elapsed_s", "tables_mib",
            "cap", "time_cap_s", "incomplete", "fail", "trace_steps", "properties", "branches", "extra_branches",
            "derived_original_states")
    parsed = {"key": key, "configuration": run["cfg"], "test": run["test"], "explorer": run["explorer"],
              "expected": {k: v for k, v in run.items() if k not in ("dir",)},
              "report": {k: rep.get(k) for k in keep} if rep else None,
              "harness_result": (re.findall(r"^test result: .*$", text, re.M) if text else None),
              "process": desc.get("process"), "supervisor": desc.get("supervisor"),
              "executable": desc.get("executable"), "selection": desc.get("selection"),
              "raw_output_sha256": desc.get("raw_output_sha256"), "outer_classification": outer,
              "original_graph_explored": run["explorer"] == "original"}
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
                    f"{'FAIL ' + rep['fail'] if rep.get('fail') else ''}{'INCOMPLETE ' + rep['incomplete'] if rep.get('incomplete') else ''}"
                    f"; harness {parsed['harness_result']}\n")
        if run["explorer"] == "composed":
            f.write("The original graph was not explored. The derived original-state count is conditional on the "
                    "accepted proof (B(1)) and N.\n")
        f.write(f"checks: {sum(ok for _, ok, _ in checks)} of {len(checks)} hold\n")
        for label, ok, detail in checks:
            if not ok:
                f.write(f"FAILED CHECK: {label}: {detail}\n")
    print(open(os.path.join(d, "CHECKS.txt")).read())
    print(open(os.path.join(d, "CLASSIFICATION.txt")).read())
    return 0 if good else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
