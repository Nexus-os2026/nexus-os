#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: the classifier on SYNTHETIC descriptors only. No Nexus code is built or run.

Each case is a descriptor written here from the frozen schedule, with a deliberate defect or none, and the outer
result the classifier must give. Exit 0 only if every case is classified as expected.
"""
import copy, json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib.util
spec = importlib.util.spec_from_file_location("classify", os.path.join(os.path.dirname(os.path.abspath(__file__)), "classify.py"))
C = importlib.util.module_from_spec(spec); spec.loader.exec_module(C)

SCHED = C.load_schedule()
PINS = {"source_commit": "a" * 40, "source_tree": "b" * 40, "model_tests_blob": "c" * 40,
        "baseline_protocol_blob": "a2df2841b99cad5d452008a6d4150e9369d07e66", "mutant_protocol_blob": "d" * 40,
        "baseline_executable_sha256": "1" * 64, "mutant_executable_sha256": "2" * 64}
EXE = "/synthetic/target/release/deps/nexus_governed_control-0000000000000000"
DETAIL = {"P4": "U is occupied together with C or X", "P8": "G1's attempt is written, uncorrected and not in U"}


def fmt_pairs(p):
    return "[" + ", ".join(f"({a}, {b})" for a, b in p) + "]"


def fmt_list(p):
    return "[" + ", ".join(str(x) for x in p) + "]"


def fmt_strs(p):
    return "[" + ", ".join(json.dumps(s) for s in p) + "]"


def witness_output(role, tweak=None):
    """Text in the witness's printed form. tweak(scenario, n, row) may alter a step record."""
    out = ["", "running 1 test"]
    first = True
    lines_v = []
    for name, sc in SCHED["scenarios"].items():
        plan = [SCHED["steps"][s] for s in sc["steps"]]
        total = len(plan) + 1
        ack = sc["ack"]
        header = (f"C4 WITNESS F-8 {name}: {total} steps from the initial world; step {total}, Ack(0)'s append, is offered "
                  f"{fmt_list(ack['offered'])}: ...")
        out.append((f"test {C.TEST} ... " if first else "") + header)
        first = False
        for n in range(1, total + 1):
            if n < total:
                st = plan[n - 1]
                row = dict(actor=st["actor"], offered=st["offered"], consumed=st["consumed"], faults=st["faults"],
                           branches=st["branches"], violations=[])
            else:
                end = ack["healthy"] if role == "healthy" else ack["changed"]
                row = dict(actor=ack["actor"], offered=ack["offered"], consumed=end["consumed"], faults=end["faults"],
                           branches=[end["branch"]], violations=[] if role == "healthy" else [("after", name)])
            if tweak:
                row = tweak(name, n, row) or row
            unused = row["offered"][len(row["consumed"]):] if len(row["consumed"]) <= len(row["offered"]) else []
            out.append(f"C4 WITNESS F-8 {name} step {n:>2}/{total} {row['actor']} offered {fmt_list(row['offered'])} "
                       f"consumed {fmt_pairs(row['consumed'])} unused {fmt_list(row.get('unused', unused))} faults left "
                       f"{row['faults']}: {row['actor']}: synthetic note")
            out.append(f"    branches {fmt_strs(row['branches'])}; word after 0x4 (live 0, unapplied 0); phase Healthy, latch None")
            out.append("    U None; C None; X None")
            out.append("    G1: insert issued false, written false, told Committed false, in U false, correction in C false, "
                       "correction in the ledger false")
            out.append("    ledger after [Prepared(0), Holder(0), Prepared(1)]")
            for place, prop in row["violations"]:
                out.append(f"    VIOLATION {place}: {prop}: {DETAIL.get(prop, 'synthetic')}")
                lines_v.append(f"{name} step {n} {place}: {prop}: {DETAIL.get(prop, 'synthetic')}")
    return out, lines_v


def healthy_text(tweak=None, tail=True):
    out, _ = witness_output("healthy", tweak)
    for name, sc in SCHED["scenarios"].items():
        out.append(C.healthy_line(name, sc))
    out.append("C4 WITNESS F-8: 2 scenarios healthy; no violation")
    if tail:
        out += ["ok", "", f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s", ""]
    return "\n".join(out) + "\n"


def mutant_text(tweak=None, seen=("OBSERVED", "OBSERVED"), panic_first=None):
    out, lines_v = witness_output("mutant", tweak)
    out.append(f"thread '{C.TEST}' (4242) panicked at {C.MODEL_PATH}:9020:5:")
    out.append(panic_first or f"C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; "
                              f"{len(lines_v)} violations")
    for (name, sc), s in zip(SCHED["scenarios"].items(), seen):
        out.append(C.named_line(name, sc, s))
        out.append(f"OTHER PROPERTIES VIOLATED in scenario {name}, each reported separately and not counted as {name}: []")
    out.append("every violation:")
    out += ["  " + l for l in lines_v]
    out += ["note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace", "FAILED", "", "failures:", "",
            "failures:", f"    {C.TEST}", "",
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 347 filtered out; finished in 0.00s", ""]
    return "\n".join(out) + "\n"


def base(kind, role, out, exe_sha, exit_status, test=C.TEST):
    return {
        "id": "synthetic", "type": kind, "role": role, "test": test, "pins": PINS,
        "argv": [EXE, test, "--exact", "--nocapture", "--test-threads=1"],
        "executable": {"path": EXE, "sha256_before": exe_sha, "sha256_after": exe_sha},
        "head": PINS["source_commit"], "tree_head": PINS["source_tree"],
        "porcelain": [] if role == "baseline" else [f" M {C.PROTOCOL_PATH}"],
        "source": {C.MODEL_PATH: PINS["model_tests_blob"],
                   C.PROTOCOL_PATH: PINS["baseline_protocol_blob"] if role == "baseline" else PINS["mutant_protocol_blob"]},
        "process": {"exit_status": exit_status, "signal": None, "supervisor_rc": exit_status},
        "supervisor": {"memory_max": C.MEMORY_MAX, "swap_max": "0", "oom": 0, "oom_kill": 0, "completed": True,
                       "timed_out": False, "wall_seconds": 0.05, "time_limit_seconds": 180, "scope_result": "gone"},
        "output": out,
    }


def healthy(**kw):
    return base("c4-healthy-witness", "baseline", healthy_text(**kw), PINS["baseline_executable_sha256"], 0)


def mutant(**kw):
    return base("c4-mutant-witness", "mutant", mutant_text(**kw), PINS["mutant_executable_sha256"], 101)


def setp(d, path, value):
    cur = d
    for k in path[:-1]:
        cur = cur[k]
    cur[path[-1]] = value
    return d


cases = []


def case(label, d, admissible):
    cases.append((label, d, admissible))


case("healthy witness on the pinned baseline executable", healthy(), True)
case("healthy witness on the mutant executable", setp(setp(healthy(), ["executable", "sha256_before"], PINS["mutant_executable_sha256"]),
                                                     ["executable", "sha256_after"], PINS["mutant_executable_sha256"]), False)
d = healthy(); d["role"] = "mutant"; d["porcelain"] = [f" M {C.PROTOCOL_PATH}"]; d["source"][C.PROTOCOL_PATH] = PINS["mutant_protocol_blob"]
case("healthy witness built from the mutant source", d, False)
case("mutant witness on the pinned mutant executable, P4 and P8 observed", mutant(), True)
case("mutant witness on the baseline executable (wrong executable)", setp(setp(mutant(), ["executable", "sha256_before"],
     PINS["baseline_executable_sha256"]), ["executable", "sha256_after"], PINS["baseline_executable_sha256"]), False)
case("mutant witness: executable changed during the run", setp(mutant(), ["executable", "sha256_after"], "3" * 64), False)
unit = base("named-unit-test-pass", "baseline",
            "\nrunning 1 test\ntest ledger::protocol::unit_tests::t_bind ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; "
            "0 measured; 346 filtered out; finished in 0.00s\n\n", PINS["baseline_executable_sha256"], 0,
            test="ledger::protocol::unit_tests::t_bind")
case("restored named-unit-test PASS without explorer output", unit, True)
build = base("build", "baseline", "", "", 0)
build.update({"stderr": "   Compiling nexus-governed-control v0.1.0 (...)\n    Finished `release` profile [optimized] target(s) in 3m 01s\n"
              "  Executable unittests src/lib.rs (target/release/deps/nexus_governed_control-0000000000000000)\n",
              "produced_executables": [EXE], "recorded_executables": [{"path": EXE, "sha256": "1" * 64, "size": 1}], "jobs": 2,
              "argv": ["cargo", "test"], "test": None})
case("healthy C22 build success without a test-run report", build, True)
b2 = copy.deepcopy(build); b2["recorded_executables"] = []
case("healthy build whose produced executable is not recorded (no C22 exemption for healthy outputs)", b2, False)
b3 = copy.deepcopy(build); b3["jobs"] = 8
case("build with more than two compiler jobs", b3, False)
k = healthy(); k["process"].update({"exit_status": None, "signal": 9, "supervisor_rc": None}); k["supervisor"]["completed"] = False
case("PASS text followed by a kill (signal 9, no completion record)", k, False)
k = healthy(); k["supervisor"].update({"oom_kill": 1, "oom": 1})
case("PASS text with an out-of-memory kill event", k, False)
k = healthy(); k["process"].update({"exit_status": 137, "supervisor_rc": 137}); k["supervisor"]["timed_out"] = True
case("PASS text followed by the time limit's kill", k, False)
case("unrelated panic, exit 101", mutant(panic_first="called `Option::unwrap()` on a `None` value"), False)
k = base("c4-mutant-witness", "mutant", "\nrunning 1 test\ntest " + C.TEST + " ... error: process didn't exit successfully\n",
         PINS["mutant_executable_sha256"], 1)
case("nonzero exit with no completed failure of the named test", k, False)
case("mutant: only P4 observed", mutant(seen=("OBSERVED", "NOT OBSERVED")), False)
case("mutant: only P8 observed", mutant(seen=("NOT OBSERVED", "OBSERVED")), False)
case("mutant: NOT OBSERVED for both", mutant(seen=("NOT OBSERVED", "NOT OBSERVED")), False)
case("mutant: P8 verdict line present but no P8 violation at step 12 (wrong locus)",
     mutant(tweak=lambda s, n, r: dict(r, violations=[]) if (s, n) == ("P8", 12) else r), False)
case("healthy: unexpected unused Oracle suffix at P4 step 2",
     healthy(tweak=lambda s, n, r: dict(r, consumed=r["consumed"][:3]) if (s, n) == ("P4", 2) else r), False)
case("healthy: changed append consumed its storage choices (not the healthy refusal)",
     healthy(tweak=lambda s, n, r: dict(r, consumed=SCHED["scenarios"]["P8"]["ack"]["changed"]["consumed"]) if (s, n) == ("P8", 12) else r), False)
case("mutant: over-consumption beyond the offered stream at P8 step 11",
     mutant(tweak=lambda s, n, r: dict(r, consumed=r["consumed"] + [[0, 3]], unused=[]) if (s, n) == ("P8", 11) else r), False)
case("mutant: changed append left an unused suffix",
     mutant(tweak=lambda s, n, r: dict(r, consumed=r["consumed"][:2]) if (s, n) == ("P4", 8) else r), False)
k = healthy(); k["output"] = k["output"].replace("C4 WITNESS F-8: 2 scenarios healthy", "    WITNESS FIXTURE FAILURE (kills nothing): x\nC4 WITNESS F-8: 2 scenarios healthy")
case("healthy text with a witness fixture failure", k, False)
k = healthy(); k["output"] = k["output"].replace("running 1 test", "running 2 tests")
case("selection: two tests ran", k, False)
k = healthy(); k["argv"] = k["argv"][:2] + k["argv"][3:]
case("selection: no --exact", k, False)
k = mutant(); k["process"].update({"signal": 6})
case("mutant: exit by a signal", k, False)
k = healthy(); k["supervisor"]["swap_max"] = "max"
case("supervision: swap not disabled", k, False)
k = healthy(); k["supervisor"]["memory_max"] = "max"
case("supervision: no memory cap", k, False)
k = healthy(); k["output"] = healthy_text(tail=False)
case("healthy: PASS text without the harness's completion lines", k, False)
k = healthy(); k["head"] = "e" * 40
case("healthy: another source commit", k, False)

failures = 0
for n, (label, d, want) in enumerate(cases, start=1):
    outer, v = C.classify(d, d["pins"], SCHED)
    got = not v.reasons
    ok = got == want
    failures += not ok
    print(f"{'OK  ' if ok else 'FAIL'} case {n:>2}: {label}: {outer}"
          f"{'' if got else ' — ' + '; '.join(v.reasons[:3]) + (' …' if len(v.reasons) > 3 else '')}")
print(f"\nCLASSIFIER CASES: {len(cases) - failures}/{len(cases)} as expected")
sys.exit(1 if failures else 0)
