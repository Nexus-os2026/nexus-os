#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01 receipt classifier: process-aware, role-specific and fail-closed.

  classify.py <descriptor.json> [<pins.json>]     prints the classification and exits 0 only if ADMISSIBLE

A descriptor is written by supervise.py for a real invocation, or by classifier-cases.py for a synthetic case.
Pins (expected identities) come from the second argument, or from the descriptor's "pins" field (synthetic cases).
Neither printed output nor exit status alone decides: every check below must agree (AD-1-R3 K.2 as supplemented
by AD1R3-DISPOSITION-01). Receipt types, each with only its own obligations:

  c4-healthy-witness   the C4 witness on the pinned UNCHANGED-baseline executable: PASS, no violation, both
                       healthy refusals with zero storage choices consumed, every Oracle record as frozen
  c4-mutant-witness    the same witness on the pinned C4-MUTANT executable: the witness's own verdict panic with
                       P4 and P8 each OBSERVED at its frozen locus, every Oracle record as frozen, exit 101
  named-unit-test-pass a restored named unit test on the pinned baseline executable: its own PASS; no explorer
                       report is required (NOT PRODUCED BY DESIGN)
  exploration-pass     an unchanged-core exploration PASS: explorer report, counts, property lines, branches
  build                a build: exit 0, finished, source identity of its role, every produced test executable
                       recorded by sha256 (the C22 mutant-build exemption never extends to a healthy build)
"""
import ast, json, os, re, sys

TEST = "ledger::protocol::model_tests::c4_witness_p4_and_p8_on_f8"
MODEL_PATH = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
PROTOCOL_PATH = "crates/nexus-governed-control/src/ledger/protocol.rs"
MEMORY_MAX = "17179869184"
STEP = re.compile(r"^C4 WITNESS F-8 (P\d) step +(\d+)/(\d+) (\S+) offered (\[[^\]]*\]) consumed (\[[^\]]*\]) "
                  r"unused (\[[^\]]*\]) faults left (\d+): (.*)$")
BRANCHES = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) ")
VIOLATION = re.compile(r"^    VIOLATION (step|after|initial): (\w+): (.*)$")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; "
                    r"(\d+) filtered out; finished in [0-9.]+s$", re.M)


def load_schedule(path=None):
    path = path or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "frozen", "c4-schedule.json")
    return json.load(open(path))


def pairs(text):
    return [[int(a), int(b)] for a, b in re.findall(r"\((\d+), (\d+)\)", text)]


def ints(text):
    return [int(v) for v in re.findall(r"\d+", text)]


def healthy_line(name, sc):
    n = len(sc["steps"]) + 1
    return (f"C4 WITNESS F-8 {name}: {n} steps, each enabled and consuming its choices as frozen; no violation; step {n} "
            f"shows the healthy core's '{sc['ack']['healthy']['branch']}', consuming 0 of its {len(sc['ack']['offered'])} "
            f"offered choices; {sc['keeps']}")


def named_line(name, sc, seen="OBSERVED"):
    n = len(sc["steps"]) + 1
    return (f"NAMED PROPERTY {name}: {seen} on the world after step {n} of scenario {name} "
            f"({sc['ack']['changed']['branch']}; {sc['locus']})")


class Verdict:
    def __init__(self):
        self.reasons, self.facts = [], []

    def need(self, ok, reason):
        if not ok:
            self.reasons.append(reason)
        return ok

    def fact(self, text):
        self.facts.append(text)


def check_steps(v, out, sched, role):
    """Every replayed step's Oracle record, read from the raw output, against the frozen schedule."""
    lines = out.splitlines()
    seen = {}
    for i, line in enumerate(lines):
        m = STEP.match(line)
        if not m:
            continue
        sc, n, total, actor, offered, consumed, unused, faults, _ = m.groups()
        b = BRANCHES.match(lines[i + 1]) if i + 1 < len(lines) else None
        branches = ast.literal_eval(b.group(1)) if b else None
        block = []
        for nxt in lines[i + 1:]:
            if STEP.match(nxt) or not nxt.startswith("    "):
                break
            vm = VIOLATION.match(nxt)
            if vm:
                block.append((vm.group(1), vm.group(2)))
        seen.setdefault(sc, []).append(dict(n=int(n), total=int(total), actor=actor, offered=ints(offered),
                                            consumed=pairs(consumed), unused=ints(unused), faults=int(faults),
                                            branches=branches, violations=block))
    for name, scd in sched["scenarios"].items():
        rows = seen.get(name, [])
        plan = [sched["steps"][s] for s in scd["steps"]]
        total = len(plan) + 1
        if not v.need(len(rows) == total, f"scenario {name}: {len(rows)} step records, not {total}"):
            continue
        for k, (row, want) in enumerate(zip(rows, plan + [None]), start=1):
            where = f"scenario {name} step {k}"
            v.need(row["n"] == k and row["total"] == total, f"{where}: numbered {row['n']}/{row['total']}")
            if not v.need(len(row["consumed"]) <= len(row["offered"]), f"{where}: over-consumption: consumed "
                          f"{len(row['consumed'])} choices beyond the {len(row['offered'])} offered"):
                continue
            v.need([c for c, _ in row["consumed"]] == row["offered"][:len(row["consumed"])],
                   f"{where}: a consumed choice is not the offered one")
            v.need(row["unused"] == row["offered"][len(row["consumed"]):], f"{where}: unused suffix misreported")
            if want is not None:
                v.need(row["actor"] == want["actor"], f"{where}: actor {row['actor']}, not {want['actor']}")
                v.need(row["offered"] == want["offered"], f"{where}: offered {row['offered']}")
                v.need(row["consumed"] == want["consumed"], f"{where}: consumed {row['consumed']}, not {want['consumed']}")
                v.need(row["unused"] == [], f"{where}: unexpected unused Oracle suffix {row['unused']}")
                v.need(row["faults"] == want["faults"], f"{where}: faults left {row['faults']}")
                v.need(row["branches"] == want["branches"], f"{where}: branches {row['branches']}")
            else:
                ack = scd["ack"]
                end = ack["healthy"] if role == "healthy" else ack["changed"]
                v.need(row["actor"] == ack["actor"], f"{where}: actor {row['actor']}")
                v.need(row["offered"] == ack["offered"], f"{where}: offered {row['offered']}")
                v.need(row["consumed"] == end["consumed"], f"{where}: consumed {row['consumed']}, the {role} plan is "
                       f"{end['consumed']}")
                want_unused = ack["offered"] if role == "healthy" else []
                v.need(row["unused"] == want_unused, f"{where}: unused {row['unused']}, the {role} plan is {want_unused}")
                v.need(row["faults"] == end["faults"], f"{where}: faults left {row['faults']}")
                v.need(row["branches"] == [end["branch"]], f"{where}: branches {row['branches']}")
                if role == "mutant":
                    v.need(("after", name) in row["violations"], f"{where}: no '{name}' among the world's violations "
                           f"after the changed append (the frozen locus)")
                v.fact(f"{where}: offered {row['offered']} consumed {row['consumed']} unused {row['unused']} "
                       f"faults left {row['faults']} branches {row['branches']} violations {row['violations']}")
            if role == "healthy":
                v.need(row["violations"] == [], f"{where}: violations {row['violations']}")


def check_common(v, d, pins, kind):
    sup, proc = d.get("supervisor", {}), d.get("process", {})
    v.need(sup.get("memory_max") == MEMORY_MAX, f"supervisor memory.max {sup.get('memory_max')}, not 16 GiB")
    v.need(sup.get("swap_max") == "0", f"supervisor memory.swap.max {sup.get('swap_max')}, not 0")
    v.need(sup.get("oom", 1) == 0 and sup.get("oom_kill", 1) == 0, f"memory events oom {sup.get('oom')} "
           f"oom_kill {sup.get('oom_kill')}")
    v.need(sup.get("completed") is True, "the scope's completion record is missing (killed from outside?)")
    v.need(sup.get("timed_out") is False, "the time limit was reached")
    wall, limit = sup.get("wall_seconds"), sup.get("time_limit_seconds")
    v.need(isinstance(wall, (int, float)) and isinstance(limit, (int, float)) and wall < limit,
           f"wall {wall} s not within the {limit} s limit")
    v.need(sup.get("scope_result") in ("success", "gone"), f"scope result {sup.get('scope_result')}")
    v.need(proc.get("signal") is None, f"terminated by signal {proc.get('signal')}")
    v.need(proc.get("exit_status") == proc.get("supervisor_rc"), "the supervisor's status differs from the command's")
    v.need(d.get("head") == pins["source_commit"], f"source HEAD {d.get('head')}, not {pins['source_commit']}")
    v.need(d.get("tree_head") == pins["source_tree"], f"HEAD tree {d.get('tree_head')}")
    src = d.get("source", {})
    v.need(src.get(MODEL_PATH) == pins["model_tests_blob"], f"model_tests.rs as built {src.get(MODEL_PATH)}")


def check_exe(v, d, pinned):
    exe = d.get("executable", {})
    v.need(exe.get("sha256_before") == pinned, f"executable before {exe.get('sha256_before')}, pinned {pinned}")
    v.need(exe.get("sha256_after") == pinned, f"executable after {exe.get('sha256_after')}, pinned {pinned}")
    v.need(d.get("argv", [])[1:] == [d.get("test"), "--exact", "--nocapture", "--test-threads=1"],
           f"argv {d.get('argv', [])[1:]}")
    v.need(d.get("argv", [None])[0] == exe.get("path"), "the executable run is not the pinned path")


def selection(v, out, test, outcome):
    v.need(out.count("\nrunning 1 test\n") == 1 and "running 0 tests" not in out, "not exactly one test ran")
    v.need(len(re.findall(rf"^test {re.escape(test)} \.\.\. ", out, re.M)) == 1, f"no single line for {test}")
    other = [l for l in re.findall(r"^test (\S+) \.\.\. ", out, re.M) if l != test]
    v.need(not other, f"other tests ran: {other}")
    r = RESULT.findall(out)
    v.need(len(r) == 1, f"{len(r)} harness result lines")
    if r:
        status, passed, failed, ignored, measured, _ = r[0]
        want = ("ok", "1", "0") if outcome == "pass" else ("FAILED", "0", "1")
        v.need((status, passed, failed) == want and ignored == "0" and measured == "0",
               f"harness result {r[0][:5]}, expected {want}")


def classify(d, pins, sched):
    v = Verdict()
    kind = d.get("type")
    out = d.get("output", "")
    if kind not in ("c4-healthy-witness", "c4-mutant-witness", "named-unit-test-pass", "exploration-pass", "build"):
        v.need(False, f"unknown receipt type {kind}")
        return "NOT ADMISSIBLE", v
    check_common(v, d, pins, kind)
    src, proc = d.get("source", {}), d.get("process", {})
    mutant_role = d.get("role") == "mutant"
    if mutant_role:
        v.need(src.get(PROTOCOL_PATH) == pins["mutant_protocol_blob"], f"protocol.rs as built {src.get(PROTOCOL_PATH)}, "
               f"not the frozen C4 mutant blob")
        v.need(d.get("porcelain") == [f" M {PROTOCOL_PATH}"], f"mutant worktree porcelain {d.get('porcelain')}")
    else:
        v.need(src.get(PROTOCOL_PATH) == pins["baseline_protocol_blob"], f"protocol.rs as built {src.get(PROTOCOL_PATH)}")
        v.need(d.get("porcelain") == [], f"baseline worktree not clean: {d.get('porcelain')}")
    if kind == "build":
        v.need(proc.get("exit_status") == 0, f"build exit {proc.get('exit_status')}")
        v.need(re.search(r"Finished `release` profile \[optimized\]", d.get("stderr", "")) is not None,
               "no 'Finished `release` profile' line")
        produced, recorded = d.get("produced_executables", []), d.get("recorded_executables", [])
        v.need(len(produced) == 1, f"{len(produced)} test executables produced for the crate")
        v.need(sorted(p["path"] for p in recorded) == sorted(produced) and all(re.fullmatch(r"[0-9a-f]{64}", p.get("sha256", ""))
               for p in recorded), "a produced test executable is not recorded by sha256 (no exemption for a build "
               "that produced one)")
        v.need(d.get("jobs") == 2, f"compiler jobs {d.get('jobs')}, not 2")
        return ("BUILD OK" if not v.reasons else "NOT ADMISSIBLE"), v
    if kind == "c4-healthy-witness":
        v.need(not mutant_role, "a healthy witness must not run on a mutant source")
        check_exe(v, d, pins["baseline_executable_sha256"])
        v.need(d.get("test") == TEST, f"test {d.get('test')}")
        v.need(proc.get("exit_status") == 0, f"exit {proc.get('exit_status')}, not 0")
        selection(v, out, TEST, "pass")
        v.need("panicked" not in out, "a panic is in the output")
        v.need("WITNESS FIXTURE FAILURE" not in out, "a witness fixture failure")
        v.need(re.search(r"^    VIOLATION ", out, re.M) is None, "a violation is in the output")
        for name, sc in sched["scenarios"].items():
            v.need(out.count(healthy_line(name, sc) + "\n") == 1, f"the healthy line of scenario {name} is missing")
        v.need(out.count("C4 WITNESS F-8: 2 scenarios healthy; no violation\n") == 1, "the final healthy line is missing")
        check_steps(v, out, sched, "healthy")
        return ("PASS" if not v.reasons else "NOT ADMISSIBLE"), v
    if kind == "c4-mutant-witness":
        v.need(mutant_role, "a mutant witness must run on the C4 mutant source")
        check_exe(v, d, pins["mutant_executable_sha256"])
        v.need(d.get("test") == TEST, f"test {d.get('test')}")
        v.need(proc.get("exit_status") == 101, f"exit {proc.get('exit_status')}, not 101")
        selection(v, out, TEST, "fail")
        panics = re.findall(r"^thread '([^']*)' \(\d+\) panicked at ([^:]+):(\d+):(\d+):\n(.*)$", out, re.M)
        v.need(len(panics) == 1 and out.count("panicked at") == 1, f"{out.count('panicked at')} panics")
        if panics:
            thread, file, _, _, first = panics[0]
            v.need(thread == TEST and file == MODEL_PATH, f"panic in thread {thread} at {file}")
            v.need(re.fullmatch(r"C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; "
                                r"\d+ violations", first) is not None, f"the panic is not the witness's verdict: {first!r}")
        v.need("WITNESS FIXTURE FAILURE" not in out, "a witness fixture failure")
        v.need("NOT OBSERVED" not in out, "a named property NOT OBSERVED")
        for name, sc in sched["scenarios"].items():
            v.need(out.count(named_line(name, sc) + "\n") == 1, f"NAMED PROPERTY {name}: OBSERVED at its frozen locus "
                   f"is missing")
        check_steps(v, out, sched, "mutant")
        return ("POSITIVE: P4 AND P8 OBSERVED" if not v.reasons else "NOT ADMISSIBLE"), v
    if kind == "named-unit-test-pass":
        check_exe(v, d, pins["baseline_executable_sha256"])
        test = d.get("test", "")
        v.need(test.startswith("ledger::protocol::unit_tests::"), f"not a named unit test: {test}")
        v.need(proc.get("exit_status") == 0, f"exit {proc.get('exit_status')}")
        selection(v, out, test, "pass")
        v.need(re.search(rf"^test {re.escape(test)} \.\.\. ok$", out, re.M) is not None, "the test's own 'ok' is missing")
        v.need("panicked" not in out, "a panic is in the output")
        v.fact("explorer report, state counts, property lines and branch coverage: NOT PRODUCED BY DESIGN (not required)")
        return ("PASS" if not v.reasons else "NOT ADMISSIBLE"), v
    if kind == "exploration-pass":
        check_exe(v, d, pins["baseline_executable_sha256"])
        test = d.get("test", "")
        v.need(proc.get("exit_status") == 0, f"exit {proc.get('exit_status')}")
        selection(v, out, test, "pass")
        v.need(re.search(r"^  outcome: PASS$", out, re.M) is not None, "no 'outcome: PASS'")
        m = re.search(r"^  states (\d+) \(expanded (\d+)\)", out, re.M)
        v.need(m is not None and int(m.group(1)) <= d.get("cap", 0), "no state count within the ceiling")
        for prop in d.get("property_lines", []):
            v.need(prop in out, f"property line missing: {prop}")
        v.need(re.search(r"^  branches ", out, re.M) is not None, "no branch coverage")
        return ("PASS" if not v.reasons else "NOT ADMISSIBLE"), v


def main():
    d = json.load(open(sys.argv[1]))
    if "output_path" in d and "output" not in d:
        d["output"] = open(os.path.join(os.path.dirname(sys.argv[1]), d["output_path"]), errors="replace").read()
    if "stderr_path" in d and "stderr" not in d:
        d["stderr"] = open(os.path.join(os.path.dirname(sys.argv[1]), d["stderr_path"]), errors="replace").read()
    pins = json.load(open(sys.argv[2])) if len(sys.argv) > 2 else d["pins"]
    outer, v = classify(d, pins, load_schedule())
    print(f"receipt {d.get('id')} type {d.get('type')} role {d.get('role')}: {outer}")
    for f in v.facts:
        print(f"  fact: {f}")
    for r in v.reasons:
        print(f"  reason: {r}")
    sys.exit(0 if not v.reasons else 1)


if __name__ == "__main__":
    main()
