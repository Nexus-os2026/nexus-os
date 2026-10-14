#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission section 6: the nonignored suite's inventory and classification.

  check_suite.py      reads receipts/20-list-all, 21-list-ignored, 30-suite; writes, in receipts/30-suite,
                      INVENTORY.json, INVENTORY.txt, DIAGNOSTICS-51.txt, CHECKS.txt and CLASSIFICATION.txt

Mechanical only: every name comes from the pinned executable's own listing, the suite's own output, model_tests.rs at
C (a git object), AD-1-R4 at C (a git object) and the sealed gate-36 receipt (sha256-verified). Its first line of
CLASSIFICATION.txt is `CLASSIFICATION: PASS` only if every check holds; supervise.py starts no full run otherwise.
"""
import ast
import hashlib
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import report  # noqa: E402

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
PIN = json.load(open(os.path.join(EV, "receipts/10-build/PIN.json")))
OUT = os.path.join(EV, "receipts/30-suite")
P = "ledger::protocol::model_tests::"
AD1R4 = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
GATE36 = ("/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage5/"
          "36-negatives-and-fail-closed.txt")
GATE36_SHA = "2c517f358002e4e99449b554ec8242952640f177269ae89e9430984a4257f1e6"
C4_SCHEDULE = ("a464a36621523f539d2554a6b176d678c7f885ea",
               "docs/evidence/p3-candidate11/c4-witness-01/frozen/c4-schedule.json",
               "d616492ff708a1a836f114424a2cce8ed6c0771d22d3f59e984322b7fa3f536b")
DEFAULTS = {"x1_two_facts_two_writers_one_admission": "X-1", "x2_one_fact_two_gating_callers": "X-2",
            "x3_one_conflicting_fact_with_env": "X-3", "x4_one_conflicting_fact_one_acknowledgement": "X-4",
            "x5_one_clean_batch_of_two": "X-5"}
# AD-1-R4 D, "Diagnostic tests kept", by group; the 24 of gate 36 are read from its command line.
GROUPS = [
    ("admission-quotient differentials", [f"quotient_x{i}_matches_the_original" for i in range(1, 6)]),
    ("admission-quotient negatives, observer, refusals", [
        "quotient_negative_e2_admission_views_twice", "quotient_negative_e9_admission_ignores_the_latch",
        "quotient_negative_p12_admission_ignores_recovery", "quotient_negative_p1_admission_ignores_unapplied",
        "quotient_observer_reports_the_admission_properties", "quotient_refuses_an_admission_that_keeps_bits",
        "quotient_refuses_an_admission_that_keeps_its_counter", "quotient_refuses_an_admission_that_writes_the_table",
        "an_admission_that_keeps_its_counter_fails_l1_in_the_original"]),
    ("symmetry differentials", [
        "gc_symmetry_x2_matches_the_original", "gc_symmetry_sym1_matches_the_original",
        "gc_symmetry_sym_pt_matches_the_original", "gc_symmetry_sym_b_matches_the_original",
        "gc_symmetry_sym_bi_matches_the_original"]),
    ("configuration exercise", [
        "gc_symmetry_sym_pt_takes_the_a4_steps", "gc_symmetry_sym_b_takes_the_batch_steps",
        "gc_symmetry_sym_bi_takes_the_batch_inserts"]),
    ("negatives and fail-closed (gate 36)", None),
    ("symmetry supporting", [
        "gc_symmetry_finds_the_original_l1_cycle", "gc_symmetry_initial_states", "gc_symmetry_l1_on_finite_graphs",
        "gc_symmetry_properties_follow_the_exchange", "gc_symmetry_renames_consistently"]),
]
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
WITNESSES = {"c1_witness_p1_at_a_successful_admission": [C1_HEALTHY], "c25_witness_p7_on_the_resulting_world": [C25_HEALTHY],
             "c4_witness_p4_and_p8_on_f8": C4_HEALTHY}
START = re.compile(r"^test (\S+)( - should panic)? \.\.\. ?(.*)$")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; "
                    r"(\d+) filtered out; finished in ([0-9.]+)s$")
# rustc 1.94 prints "thread '<name>' (<thread id>) panicked at"; v1 required the older form and so matched none.
PANIC = re.compile(r"^thread '([^']*)'(?: \(\d+\))? panicked at ")
STEP = re.compile(r"^C4 WITNESS F-8 (P\d) step +(\d+)/(\d+) (\S+) offered (\[[^\]]*\]) consumed (\[[^\]]*\]) "
                  r"unused (\[[^\]]*\]) faults left (\d+): (.*)$")
BRANCHES_LINE = re.compile(r"^    branches (\[.*\]); word after (0x[0-9a-f]+) ")
VIOLATION = re.compile(r"^    VIOLATION (step|after|initial): (\w+): (.*)$")

checks = []


def check(label, ok, detail=""):
    checks.append((label, bool(ok), detail))
    return ok


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git_blob(commit, path):
    return subprocess.run(["git", "-C", REPO, "cat-file", "blob", f"{commit}:{path}"], capture_output=True,
                          check=True).stdout


def listing(d):
    raw = open(os.path.join(EV, d, "raw-output.txt")).read()
    names = [l[:-len(": test")] for l in raw.splitlines() if l.endswith(": test")]
    summary = re.search(r"^(\d+) tests, (\d+) benchmarks$", raw, re.M)
    desc = json.load(open(os.path.join(EV, d, "descriptor.json")))
    return names, (int(summary.group(1)), int(summary.group(2))) if summary else None, desc


def process_ok(label, desc, limit):
    proc, sup, exe = desc["process"], desc["supervisor"], desc["executable"]
    check(f"{label}: executable is the pinned one before and after",
          exe["sha256_before"] == exe["sha256_after"] == PIN["sha256"] and exe["path"] == PIN["executable"],
          f"{exe['sha256_before']} / {exe['sha256_after']}")
    check(f"{label}: source exactly C before and after", desc["source_before"]["verdict"] == "EXACTLY_C"
          and desc["source_after"]["verdict"] == "EXACTLY_C",
          f"{desc['source_before']['verdict']} / {desc['source_after']['verdict']}")
    check(f"{label}: normal exit 0, no signal", proc["exit_status"] == 0 and proc["signal"] is None,
          f"exit {proc['exit_status']}, signal {proc['signal']}, supervisor rc {proc['supervisor_rc']}")
    check(f"{label}: supervision complete; 16 GiB, zero swap; no OOM, no timeout",
          sup["completed"] and sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and sup["oom_kill"] == 0
          and sup["swap_peak_bytes"] == "0" and not sup["timed_out"] and sup["time_limit_seconds"] == limit
          and sup["wall_seconds"] is not None and sup["wall_seconds"] < limit,  # v1: 0.0 s was read as missing
          f"memory.peak {sup['memory_peak_bytes']} B, swap.peak {sup['swap_peak_bytes']} B, oom_kill {sup['oom_kill']}, "
          f"wall {sup['wall_seconds']} s of {limit} s")


def model_tests_index():
    """{test name: its attributes and body} for every #[test] of model_tests.rs at C."""
    text = report.model_text()
    out = {}
    for m in re.finditer(r"#\[test\]", text):
        rest = text[m.start():]
        fn = re.search(r"\bfn (\w+)\s*\(", rest)
        end = rest.find("\n}\n", fn.end())
        out[fn.group(1)] = rest[:end + 3]
    return out


def segments(lines, listed):
    """[(name, should_panic, [lines])] in output order; a start line names a listed test."""
    segs, cur = [], None
    for l in lines:
        m = START.match(l)
        if m and m.group(1) in listed:
            cur = [m.group(1), bool(m.group(2)), [m.group(3)]]
            segs.append(cur)
        elif RESULT.match(l) or l == "failures:":
            cur = None
        elif cur is not None:
            cur[2].append(l)
    return segs


def result_of(seg_lines):
    body = [l for l in seg_lines if l.strip() != ""]
    if not body:
        return "MISSING", []
    last = body[-1]
    if last == "ok" or last == "FAILED" or last == "ignored" or last.startswith("ignored, "):
        return last.split(",")[0], body[:-1]
    return "UNPARSED: " + last[:120], body


def c4_steps(out_lines, sched):
    """G.6, healthy role: every step's offered and consumed choices against the frozen schedule (as C4-WITNESS-01's
    classify.py check_steps does), read from the suite's own output."""
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


def main():
    if os.path.exists(os.path.join(OUT, "CLASSIFICATION.txt")):
        raise SystemExit("REFUSED: already classified")
    props, branches = report.model_constants()
    all_names, all_sum, all_desc = listing("receipts/20-list-all")
    ign_names, ign_sum, ign_desc = listing("receipts/21-list-ignored")
    for label, d in (("listing (all)", all_desc), ("listing (ignored)", ign_desc)):
        process_ok(label, d, 60)
    check("listing (all): names unique, count equals its summary", len(set(all_names)) == len(all_names)
          and all_sum == (len(all_names), 0), f"{len(all_names)} names; summary {all_sum}")
    check("listing (ignored): names unique, count equals its summary, a subset of all", len(set(ign_names)) == len(ign_names)
          and ign_sum == (len(ign_names), 0) and set(ign_names) <= set(all_names), f"{len(ign_names)}; summary {ign_sum}")
    listed, ignored = set(all_names), set(ign_names)
    nonignored = sorted(listed - ignored)
    # AD-1-R4 E.3: the 34 ignored tests of model_tests.rs (at 42dd1bd1), recomputed here from the source at C.
    idx = model_tests_index()
    src_ignored = sorted(P + n for n, t in idx.items() if re.search(r"#\[ignore\b", t.split("fn " + n)[0]))
    e3 = re.findall(r"^\| \d+ \| `(\w+)` \(\d+\) \|", git_blob(C, AD1R4).decode(), re.M)
    check("ignored tests of model_tests.rs at C equal AD-1-R4 E.3's 34", sorted(P + n for n in e3) == src_ignored
          and len(e3) == 34, f"E.3 {len(e3)}; source {len(src_ignored)}")
    check("the executable's ignored model tests equal the source's", sorted(n for n in ignored if n.startswith(P))
          == src_ignored, f"{len([n for n in ignored if n.startswith(P)])} listed")
    other_ignored = sorted(n for n in ignored if not n.startswith(P))
    check("ignored outside model_tests: recorded", True, f"{other_ignored}")

    # ---- the suite ------------------------------------------------------------------------------------------
    desc = json.load(open(os.path.join(OUT, "descriptor.json")))
    process_ok("suite", desc, 180)
    check("suite: no name filter; --nocapture --test-threads=1; pinned executable",
          desc["argv"] == [PIN["executable"], "--nocapture", "--test-threads=1"], f"{desc['argv']}")
    raw = open(os.path.join(OUT, "raw-output.txt"), "rb").read()
    check("suite: raw output sha256 equals the launcher's record", sha256(raw) == desc["raw_output_sha256"], sha256(raw))
    text = raw.decode("utf-8", errors="replace")
    lines = text.splitlines()
    running = re.findall(r"^running (\d+) tests?$", text, re.M)
    results = [RESULT.match(l) for l in lines if RESULT.match(l)]
    check("suite: one 'running N tests' line, N = every listed test", running == [str(len(all_names))], f"{running}")
    check("suite: one harness result line", len(results) == 1, f"{[r.group(0) for r in results]}")
    r = results[0].groups() if results else None
    check("suite: test result ok; passed = nonignored; 0 failed; ignored = listed ignored; 0 filtered",
          r is not None and r[0] == "ok" and int(r[1]) == len(nonignored) and r[2] == "0" and int(r[3]) == len(ignored)
          and r[4] == "0" and r[5] == "0", f"{results[0].group(0) if results else None}")
    segs = segments(lines, listed)
    names = [s[0] for s in segs]
    dup = sorted({n for n in names if names.count(n) > 1})
    outcome = {}
    bodies = {}
    for name, sp, seg in segs:
        res, body = result_of(seg)
        outcome[name] = res
        bodies[name] = body
    executed = sorted(n for n, o in outcome.items() if o != "ignored")
    check("suite: every listed test appears exactly once", not dup and sorted(names) == sorted(all_names),
          f"duplicates {dup}; missing {sorted(listed - set(names))}")
    check("suite: every ignored test reported 'ignored' (none executed)", all(outcome.get(n) == "ignored" for n in ignored),
          f"{sorted((n, outcome.get(n)) for n in ignored if outcome.get(n) != 'ignored')}")
    check("suite: executed set equals the nonignored set", executed == nonignored,
          f"listed-not-executed {sorted(set(nonignored) - set(executed))}; executed-not-listed-nonignored "
          f"{sorted(set(executed) - set(nonignored))}")
    bad = sorted((n, o) for n, o in outcome.items() if n not in ignored and o != "ok")
    check("suite: every executed test ok (0 failed, none unparsed)", not bad, f"{bad}")
    check("suite: no 'failures:' section", "\nfailures:\n" not in text, "")

    # Refusals, fixture failures and panics, attributed to the test that printed them.
    refusal_hits, panic_hits = [], []
    cur = None
    for l in lines:
        m = START.match(l)
        if m and m.group(1) in listed:
            cur = m.group(1)
        for kind in ("quotient inapplicable", "symmetry inapplicable"):
            if kind in l:
                refusal_hits.append((cur, kind, l.strip()[:200]))
        pm = PANIC.match(l)
        if pm:
            panic_hits.append((pm.group(1), cur))
    allowed_refusal = lambda n: n and n.startswith(P) and any(
        k in idx.get(n[len(P):], "") for k in ("inapplicable", "should_panic", "catch_unwind"))
    stray = [h for h in refusal_hits if not allowed_refusal(h[0])]
    check("suite: every quotient/symmetry refusal is inside a test written to provoke it (should_panic, catch_unwind "
          "or an expected refusal in its own text)", not stray, f"{len(refusal_hits)} refusal lines; stray {stray}")
    check("suite: no WITNESS FIXTURE FAILURE anywhere", "WITNESS FIXTURE FAILURE" not in text, "")
    unexpected_panics = []
    for thread, seg in panic_hits:
        t = thread if thread in listed else seg
        src = idx.get(t[len(P):], "") if t and t.startswith(P) else None
        if src is not None and not any(k in src for k in ("should_panic", "catch_unwind", "inapplicable")):
            unexpected_panics.append((thread, seg))
    check("suite: every panic message is attributed to a listed test", all((t if t in listed else s) in listed
                                                                           for t, s in panic_hits),
          f"{[(t, s) for t, s in panic_hits if (t if t in listed else s) not in listed]}")
    check("suite: every panic in a model test is in a should_panic or catch_unwind test", not unexpected_panics,
          f"{len(panic_hits)} panic messages; unexpected {unexpected_panics}")
    check("suite: panic messages from other modules: recorded (the tests passed)", True,
          f"{sorted({(t, s) for t, s in panic_hits if not ((t if t in listed else s) or '').startswith(P)})}")

    # Five default explorations.
    defaults = {}
    for test, cfg in DEFAULTS.items():
        n = P + test
        rep = report.parse_report("\n".join(bodies.get(n, [])))
        ok = rep is not None and rep["name"] == cfg and rep["header"] == f"EXPLORATION {cfg}" and outcome.get(n) == "ok"
        res = report.check_pass(rep, props, branches) if rep else []
        failed = [l for l, good, _ in res if not good]
        check(f"default exploration {cfg} ({test}): ok, PASS, 24 HOLD, E10 not applicable, complete, branches complete",
              ok and not failed and rep["cap"] == 250000 and rep["time_cap_s"] == 180,
              f"states {rep and rep.get('states')}, expanded {rep and rep.get('expanded')}, cap {rep and rep.get('cap')}; "
              f"failed {failed}")
        if rep:
            defaults[cfg] = {k: rep.get(k) for k in ("states", "expanded", "transitions", "fault_edges", "no_step",
                                                    "elapsed_s", "tables_mib", "cap", "time_cap_s", "actors", "bounds")}
            defaults[cfg]["properties"] = rep["properties"]
            defaults[cfg]["taken"] = [b for m, b in rep["branches"] if m == "taken    "]
            defaults[cfg]["extra_branches"] = rep["extra_branches"]

    # The 51 diagnostic tests of AD-1-R4 D (N.8).
    g36 = open(GATE36, "rb").read()
    check("gate 36 receipt sha256", sha256(g36) == GATE36_SHA, sha256(g36))
    cmd = re.search(r"^command: (.*)$", g36.decode(), re.M).group(1)
    gate36 = [a[len(P):] for a in cmd.split() if a.startswith(P)]
    check("gate 36 names 24 tests", len(gate36) == 24 and len(set(gate36)) == 24, f"{len(gate36)}")
    ad = git_blob(C, AD1R4).decode()
    dsec = ad[ad.index("**Diagnostic tests kept.**"):ad.index("## E. Which configurations")]
    diag, rows = [], []
    for group, members in GROUPS:
        members = gate36 if members is None else members
        for m in members:
            diag.append(m)
            rows.append((group, m))
    named_in_d = all(("`%s`" % m) in dsec for g, m in rows if g not in ("negatives and fail-closed (gate 36)",
                                                                        "admission-quotient differentials"))
    check("every explicitly named diagnostic appears in AD-1-R4 D's text", named_in_d, "")
    check("51 distinct diagnostic tests", len(diag) == 51 and len(set(diag)) == 51, f"{len(diag)}")
    diag_rows = []
    for group, m in rows:
        n = P + m
        diag_rows.append({"group": group, "test": n, "listed": n in listed, "ignored": n in ignored,
                          "occurrences": names.count(n), "result": outcome.get(n)})
    missing = [d["test"] for d in diag_rows if not d["listed"] or d["ignored"] or d["occurrences"] != 1]
    failed = [d["test"] for d in diag_rows if d["result"] != "ok"]
    check("51 diagnostics: each listed once, nonignored, executed once", not missing, f"{missing}")
    check("51 diagnostics: each ok", not failed, f"{failed}")

    # Healthy witnesses.
    sched_bytes = git_blob(C4_SCHEDULE[0], C4_SCHEDULE[1])
    check("C4 frozen schedule (evidence commit B) sha256", sha256(sched_bytes) == C4_SCHEDULE[2], sha256(sched_bytes))
    sched = json.loads(sched_bytes)
    witness = {}
    for test, healthy in WITNESSES.items():
        n = P + test
        body = bodies.get(n, [])
        present = [h for h in healthy if body.count(h) == 1]
        markers = [l for l in body if "OBSERVED" in l or "VIOLATION" in l or "WITNESS FIXTURE FAILURE" in l
                   or l.startswith("WITNESS:") or "panicked" in l]
        check(f"healthy witness {test}: ok; the exact healthy line(s) of G.3(d)/G.5(C), once each; no positive marker, "
              f"violation, fixture failure or panic", outcome.get(n) == "ok" and present == healthy and not markers,
              f"result {outcome.get(n)}; healthy lines {len(present)}/{len(healthy)}; markers {markers[:3]}")
        witness[test] = {"result": outcome.get(n), "healthy_lines_found": present, "markers": markers}
    c4 = P + "c4_witness_p4_and_p8_on_f8"
    probs, facts, counts = c4_steps(bodies.get(c4, []), sched)
    check("healthy C4 witness: G.6 offered/consumed for every step, against the frozen schedule", not probs,
          f"step records {counts}; problems {probs}")
    witness["c4_witness_p4_and_p8_on_f8"]["g6_facts"] = facts

    # ---- outputs -------------------------------------------------------------------------------------------
    inv = {"executable": PIN, "listed": all_names, "ignored": sorted(ignored), "nonignored": nonignored,
           "outcomes": outcome, "executed": executed, "duplicates": dup,
           "harness_result": results[0].group(0) if results else None, "ignored_outside_model_tests": other_ignored,
           "refusal_lines": refusal_hits, "panic_messages": panic_hits, "default_explorations": defaults,
           "diagnostics_51": diag_rows, "witnesses": witness}
    json.dump(inv, open(os.path.join(OUT, "INVENTORY.json"), "w"), indent=1)
    with open(os.path.join(OUT, "INVENTORY.txt"), "w") as f:
        f.write(f"TEST INVENTORY — pinned executable {PIN['executable']} sha256 {PIN['sha256']}\n")
        f.write(f"listed {len(all_names)}; ignored {len(ignored)}; nonignored {len(nonignored)}; executed "
                f"{len(executed)}; harness: {inv['harness_result']}\n")
        f.write("columns: listed-ignored? | result in the suite | test\n")
        for n in all_names:
            f.write(f"{'IGNORED   ' if n in ignored else 'nonignored'} | {outcome.get(n, 'ABSENT'):8} | {n}\n")
        f.write(f"difference between listed nonignored and executed: "
                f"{sorted(set(nonignored) ^ set(executed)) or 'none'}\n")
    with open(os.path.join(OUT, "DIAGNOSTICS-51.txt"), "w") as f:
        f.write("THE 51 DIAGNOSTIC TESTS OF AD-1-R4 D (N.8) ON THE PINNED EXECUTABLE (from the suite's own output)\n")
        for d in diag_rows:
            f.write(f"{d['group']:52} | listed {d['listed']} | ignored {d['ignored']} | occurrences {d['occurrences']} | "
                    f"{d['result']:6} | {d['test']}\n")
        f.write(f"total {len(diag_rows)}; ok {sum(d['result'] == 'ok' for d in diag_rows)}\n")
    good = all(ok for _, ok, _ in checks)
    with open(os.path.join(OUT, "CHECKS.txt"), "w") as f:
        for label, ok, detail in checks:
            f.write(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}\n")
    with open(os.path.join(OUT, "CLASSIFICATION.txt"), "w") as f:
        f.write(f"CLASSIFICATION: {'PASS' if good else 'NOT PASS'}\n")
        f.write(f"raw outcome: {inv['harness_result']}; exit {desc['process']['exit_status']}, signal "
                f"{desc['process']['signal']}\n")
        f.write(f"checks: {sum(ok for _, ok, _ in checks)} of {len(checks)} hold\n")
        for label, ok, detail in checks:
            if not ok:
                f.write(f"FAILED CHECK: {label}: {detail}\n")
    print(open(os.path.join(OUT, "CHECKS.txt")).read())
    print(open(os.path.join(OUT, "CLASSIFICATION.txt")).read())
    return 0 if good else 1


if __name__ == "__main__":
    sys.exit(main())
