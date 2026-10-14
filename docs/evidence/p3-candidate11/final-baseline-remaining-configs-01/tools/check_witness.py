#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission sections 7 and 12: classify one standalone healthy witness.

  check_witness.py c1|c25|c4 [RECEIPT-DIR]     reads RECEIPT-DIR (default: the mission's receipt directory); writes
                                               CHECKS.txt, PARSED-RECEIPT.json and CLASSIFICATION.txt there (first
                                               line `CLASSIFICATION: PASS` only if every check holds)

AD-1-R4 type (e3): F.1 (e3), F.2 and K.2. The raw outcome and the outer class are kept apart. The expected healthy
lines are AD-1-R4's G.3(d) and G.5(C) and are asserted to occur verbatim in AD-1-R4 at C. For C4, G.6 is checked a
second time, from the raw output, against the frozen schedule of evidence commit a464a366 (the step-record logic is
Stage 1's check_suite.py c4_steps, unchanged in behaviour).
"""
import ast
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

LISTED = 348
P = common.TEST_PREFIX
C1_HEALTHY = ("C1 WITNESS F-1: 11 steps, each enabled and taking its choices; no violation; step 11 shows the healthy "
              "core's 'admission: refused'")
C25_HEALTHY = ("C25 WITNESS F-1: 1 steps, each enabled and taking its choices; no violation; step 1 shows the healthy "
               "core's 'append_gating: Refused Uncertain Storage'")
C4_HEALTHY = [
    "C4 WITNESS F-8 P4: 8 steps, each enabled and consuming its choices as frozen; no violation; step 8 shows the healthy "
    "core's 'append_acknowledgement: Refused(NotNow(CellsBusy))', consuming 0 of its 6 offered choices; U stays empty "
    "and X holds F1's explanation",
    "C4 WITNESS F-8 P8: 12 steps, each enabled and consuming its choices as frozen; no violation; step 12 shows the "
    "healthy core's 'append_acknowledgement: Refused(NotNow(NotHealthy))', consuming 0 of its 4 offered choices; G1's "
    "written attempt stays in U",
    "C4 WITNESS F-8: 2 scenarios healthy; no violation"]
W = {"c1": dict(test=P + "c1_witness_p1_at_a_successful_admission", dir="receipts/50-witness-C1", healthy=[C1_HEALTHY],
                control="C1", clause="G.3(d)"),
     "c25": dict(test=P + "c25_witness_p7_on_the_resulting_world", dir="receipts/51-witness-C25",
                 healthy=[C25_HEALTHY], control="C25", clause="G.3(d)"),
     "c4": dict(test=P + "c4_witness_p4_and_p8_on_f8", dir="receipts/52-witness-C4", healthy=C4_HEALTHY, control="C4",
                clause="G.5(C)")}
STEP = re.compile(r"^C4 WITNESS F-8 (P\d) step +(\d+)/(\d+) (\S+) offered (\[[^\]]*\]) consumed (\[[^\]]*\]) "
                  r"unused (\[[^\]]*\]) faults left (\d+): (.*)$")
BRANCHES_LINE = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) ")
VIOLATION = re.compile(r"^    VIOLATION (step|after|initial): (\w+): (.*)$")
PANIC = re.compile(r"^thread '([^']*)'(?: \(\d+\))? panicked at ")


def c4_steps(out_lines, sched):
    """G.6, healthy role: every step's offered and consumed choices against the frozen schedule (Stage 1's
    check_suite.py c4_steps, unchanged in behaviour)."""
    problems, facts, seen = [], [], {}
    for i, line in enumerate(out_lines):
        m = STEP.match(line)
        if not m:
            continue
        sc, n, total, actor, offered, consumed, unused, faults, _ = m.groups()
        b = BRANCHES_LINE.match(out_lines[i + 1]) if i + 1 < len(out_lines) else None
        block = []
        for nxt in out_lines[i + 1:]:
            if STEP.match(nxt) or not nxt.startswith("    "):
                break
            vm = VIOLATION.match(nxt)
            if vm:
                block.append((vm.group(1), vm.group(2)))
        seen.setdefault(sc, []).append(dict(
            n=int(n), total=int(total), actor=actor, offered=[int(v) for v in re.findall(r"\d+", offered)],
            consumed=[[int(a), int(c)] for a, c in re.findall(r"\((\d+), (\d+)\)", consumed)],
            unused=[int(v) for v in re.findall(r"\d+", unused)], faults=int(faults),
            branches=ast.literal_eval(b.group(1)) if b else None, violations=block))
    for name, scd in sched["scenarios"].items():
        rows = seen.get(name, [])
        plan = [sched["steps"][s] for s in scd["steps"]]
        total = len(plan) + 1
        if len(rows) != total:
            problems.append(f"scenario {name}: {len(rows)} step records, not {total}")
            continue
        for k, (row, want) in enumerate(zip(rows, plan + [None]), start=1):
            where = f"scenario {name} step {k}"
            need = lambda ok, why: ok or problems.append(f"{where}: {why}")
            need(row["n"] == k and row["total"] == total, f"numbered {row['n']}/{row['total']}")
            if not need(len(row["consumed"]) <= len(row["offered"]), "over-consumption"):
                continue
            need([c for c, _ in row["consumed"]] == row["offered"][:len(row["consumed"])], "a consumed choice is not offered")
            need(row["unused"] == row["offered"][len(row["consumed"]):], "unused suffix misreported")
            need(row["violations"] == [], f"violations {row['violations']}")
            if want is not None:
                need(row["actor"] == want["actor"], f"actor {row['actor']}")
                need(row["offered"] == want["offered"], f"offered {row['offered']}")
                need(row["consumed"] == want["consumed"], f"consumed {row['consumed']}")
                need(row["unused"] == [], f"unused {row['unused']}")
                need(row["faults"] == want["faults"], f"faults left {row['faults']}")
                need(row["branches"] == want["branches"], f"branches {row['branches']}")
            else:
                ack, end = scd["ack"], scd["ack"]["healthy"]
                need(row["actor"] == ack["actor"], f"actor {row['actor']}")
                need(row["offered"] == ack["offered"], f"offered {row['offered']}")
                need(row["consumed"] == end["consumed"], f"consumed {row['consumed']}")
                need(row["unused"] == ack["offered"], f"unused {row['unused']}")
                need(row["faults"] == end["faults"], f"faults left {row['faults']}")
                need(row["branches"] == [end["branch"]], f"branches {row['branches']}")
                facts.append(f"{where}: offered {row['offered']} consumed {row['consumed']} unused {row['unused']} "
                             f"faults left {row['faults']} branches {row['branches']}")
    return problems, facts, {k: len(v) for k, v in seen.items()}


def main(key, d=None):
    w = W[key]
    d = d or os.path.join(common.EV, w["dir"])
    if os.path.exists(os.path.join(d, "CLASSIFICATION.txt")):
        raise SystemExit("REFUSED: already classified")
    checks = []
    check = lambda label, ok, detail="": checks.append((label, bool(ok), str(detail)))
    ad1r4 = common.blob(common.C, common.AD1R4).decode()
    flat = re.sub(r"\s+", " ", ad1r4)
    for h in w["healthy"]:
        check(f"expected healthy line occurs verbatim in AD-1-R4 {w['clause']} at C", h in flat, h[:60] + "…")
    desc = json.load(open(os.path.join(d, "descriptor.json")))
    started = "not_started" not in desc
    check("the witness started (no pre-flight refusal)", started, desc.get("not_started", ""))
    if not started:
        return write(d, key, w, checks, desc, None, "NOT ADMISSIBLE (not started: %s)" % desc["not_started"], None)
    proc, sup, exe = desc["process"], desc["supervisor"], desc["executable"]
    raw = open(os.path.join(d, "raw-output.txt"), "rb").read()
    text = raw.decode("utf-8", errors="replace")
    lines = text.splitlines()
    # The test's first printed line shares the harness's `test <name> ... ` line; split it off once.
    body = text.replace(f"test {w['test']} ... ", f"test {w['test']} ... \n", 1).splitlines()
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
    # F.2: selection.
    check("argv: the pinned executable, exactly the one test, --exact --nocapture --test-threads=1, no --ignored",
          desc["argv"] == [common.EXE, w["test"], "--exact", "--nocapture", "--test-threads=1"], desc["argv"])
    sel = desc["selection"]
    check("selection listing (--exact --list) shows exactly that one test",
          sel["listing"]["exit"] == 0 and sel["listing"]["listed"] == [w["test"] + ": test"], sel["listing"]["listed"])
    check("the test is not an ignored test (--exact --ignored --list shows none)",
          sel["ignored_listing"]["exit"] == 0 and sel["ignored_listing"]["listed"] == [], sel["ignored_listing"]["listed"])
    starts = re.findall(r"^test (\S+) \.\.\. ", text, re.M)
    check("the harness ran exactly that one test", re.findall(r"^running (\d+) tests?$", text, re.M) == ["1"]
          and starts == [w["test"]], f"running {re.findall(r'^running (.*)$', text, re.M)}; started {starts}")
    # K.2 (4): harness result.
    result = re.findall(r"^test result: .*$", text, re.M)
    want = f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {LISTED - 1} filtered out; finished in "
    check(f"harness: 'ok' then '{want}…'", len(result) == 1 and result[0].startswith(want)
          and re.search(r"\nok\n\ntest result: ok\.", text), result)
    # K.2 (5)-(7): process and launching command.
    check("normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
          f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check("launching command: 16 GiB, zero swap, enforced; no OOM, no kill, no timeout; complete record; within 180 s",
          sup["completed"] and sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and sup["oom_kill"] == 0
          and sup["swap_peak_bytes"] == "0" and not sup["timed_out"] and sup["time_limit_seconds"] == 180
          and sup["wall_seconds"] is not None and sup["wall_seconds"] < 180,
          f"memory.peak {sup['memory_peak_bytes']} B, swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, oom_kill "
          f"{sup['oom_kill']}, wall {sup['wall_seconds']} s of 180 s, scope {sup['scope_result']}")
    # The body: exact healthy lines, no positive marker, violation, fixture failure or panic.
    counts = {h[:48] + "…": body.count(h) for h in w["healthy"]}
    check(f"the exact healthy line(s) of {w['clause']}, each exactly once as a whole line, nowhere else",
          all(body.count(h) == 1 and text.count(h) == 1 for h in w["healthy"]), counts)
    markers = [l for l in body if "OBSERVED" in l or "VIOLATION" in l or "WITNESS FIXTURE FAILURE" in l
               or l.startswith("WITNESS:") or "NAMED PROPERTY" in l or PANIC.match(l) or "panicked" in l]
    check("no positive marker (NAMED PROPERTY / OBSERVED), violation, fixture failure or panic", not markers,
          markers[:3])
    check("no other test's output, no 'FAILED' or 'failures:' section", "FAILED" not in text and "failures:" not in text)
    g6 = None
    if key == "c4":
        sched_bytes = common.blob(common.C4_SCHEDULE[0], common.C4_SCHEDULE[1])
        check("C4 frozen schedule (evidence commit a464a366) sha256", common.sha256_bytes(sched_bytes) ==
              common.C4_SCHEDULE[2], common.sha256_bytes(sched_bytes))
        sched = json.loads(sched_bytes)
        probs, facts, counts = c4_steps(body, sched)
        check("G.6 (healthy role): every step's offered and consumed choices, unused suffix, branches and faults "
              "against the frozen schedule; the changed step consumes nothing and leaves its whole offered suffix",
              not probs and counts == {"P4": 8, "P8": 12}, f"step records {counts}; problems {probs}")
        g6 = dict(facts=facts, step_records=counts, problems=probs)
    outer = "PASS — admissible healthy-witness type-(e3) PASS" if all(ok for _, ok, _ in checks) else None
    if outer is None:
        failed = [l for l, ok, _ in checks if not ok]
        if any(l.startswith("normal exit") or l.startswith("launching command") for l in failed):
            outer = "NOT ADMISSIBLE (abnormal termination or resource evidence; never PASS)"
        else:
            outer = "NOT ADMISSIBLE (" + "; ".join(failed) + ")"
    return write(d, key, w, checks, desc, text, outer, g6)


def write(d, key, w, checks, desc, text, outer, g6):
    good = all(ok for _, ok, _ in checks)
    parsed = {"witness": key, "control": w["control"], "test": w["test"], "receipt_type": "AD-1-R4 F.1 (e3)",
              "expected_healthy_lines": w["healthy"], "outer_classification": outer,
              "raw_outcome": (re.findall(r"^test result: .*$", text, re.M) if text else None),
              "process": desc.get("process"), "supervisor": desc.get("supervisor"),
              "executable": desc.get("executable"), "selection": desc.get("selection"),
              "raw_output_sha256": desc.get("raw_output_sha256"), "g6": g6,
              "not_produced_by_design": "the explorer report, counts, property lines and branch list (F.1 (e3))"}
    json.dump(parsed, open(os.path.join(d, "PARSED-RECEIPT.json"), "w"), indent=1)
    with open(os.path.join(d, "CHECKS.txt"), "w") as f:
        for label, ok, detail in checks:
            f.write(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}\n")
        if g6:
            for fact in g6["facts"]:
                f.write(f"     G.6 {fact}\n")
    with open(os.path.join(d, "CLASSIFICATION.txt"), "w") as f:
        f.write(f"CLASSIFICATION: {'PASS' if good else 'NOT PASS'}\n")
        f.write(f"outer class (AD-1-R4 K.2): {outer}\n")
        if text is not None:
            f.write(f"raw outcome (as printed): harness {parsed['raw_outcome']}\n")
        f.write(f"checks: {sum(ok for _, ok, _ in checks)} of {len(checks)} hold\n")
        for label, ok, detail in checks:
            if not ok:
                f.write(f"FAILED CHECK: {label}: {detail}\n")
    print(open(os.path.join(d, "CHECKS.txt")).read())
    print(open(os.path.join(d, "CLASSIFICATION.txt")).read())
    return 0 if good else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
