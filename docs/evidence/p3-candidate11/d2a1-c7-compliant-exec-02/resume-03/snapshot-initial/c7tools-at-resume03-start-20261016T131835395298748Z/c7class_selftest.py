#!/usr/bin/env python3
"""Self-test of c7class.py on SYNTHETIC raw outputs built from Table A (never on a real run, never in the Nexus repository).
  c7class.py selftest <new-dir>
"""
import contextlib
import copy
import hashlib
import io
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c7class as K
from c7lib import *  # noqa

RESULTS = []
SAMPLES = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "fixtures", "samples")  # EXEC-02: copied fixtures
EW = "/fake/c7w-healthy/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
EW7 = "/fake/c7w-mutant/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
SHA_W, SHA_W7 = "a" * 64, "b" * 64


def expect(name, cond, detail=""):
    RESULTS.append(bool(cond))
    print("[%s] %s %s" % ("OK" if cond else "FAILED", name, detail), flush=True)


# ---------------------------------------------------------------------------------------------------------------- witness
def step_lines(n, mode):
    """The lines the witness prints for step n. mode 'healthy' or 'mutant'."""
    row = K.CHANGED_23 if (n == 23 and mode == "mutant") else K.TABLE_A[n - 1]
    offered = row["offered"]
    consumed = row["consumed"]
    unused = offered[len(consumed):]
    flag = "   [fault]" if n == K.FAULT_STEP else ""
    note = row["branches"][0] if row["branches"] else "step %d of the frozen plan" % n
    out = ["C7 WITNESS F-PT step %02d/24 %s offered %s consumed %s unused %s faults left %d: %s%s" % (
        n, row["actor"], offered, "[" + ", ".join("(%d, %d)" % c for c in consumed) + "]", unused, row["faults"], note, flag)]
    out[0] = out[0].replace("offered %s" % offered, "offered %s" % json.dumps(offered))
    out[0] = out[0].replace("unused %s" % unused, "unused %s" % json.dumps(unused))
    if mode == "healthy":
        table = "F1: MissingPrepared" if n >= 5 else ""
        cls = ("Conflict" if n >= 5 else "Unattempted")
    else:
        table = "F1: MissingPrepared" if 5 <= n <= 12 else ""
        cls = "Paid" if n >= 23 else ("Conflict" if n >= 5 else "Unattempted")
    ledger = ["Prepared(0)", "Prepared(1)"]
    if mode == "mutant" and n >= 23:
        ledger.append("Fact(0)")
    out += [
        "    branches %s; word after 0x1f (live 3, unapplied 0); phase Armed, latch None" % json.dumps(row["branches"]),
        "    U None; C None; X None",
        "    refusal table after %02d: {%s}" % (n, table),
        "    classes after %02d: F1 %s, F2 Unattempted; debt 0; H 0; A 0" % (n, cls),
        "    conflicted after %02d: F1 %s, F2 false" % (n, "true" if n >= 5 else "false"),
        "    ledger after %02d: [%s]" % (n, ", ".join(ledger)),
    ]
    if mode == "mutant" and n == 13:
        out.append(K.E11_REC)
    if mode == "mutant" and n == 24:
        out.append(K.P6_REC)
    return out


def gen_witness(kind):
    """parts dict for (C) 'C' or (B) 'B'."""
    mode = "healthy" if kind == "C" else "mutant"
    parts = {"blocks": [step_lines(n, mode) for n in range(1, 25)], "kind": kind}
    if kind == "C":
        parts["h1"] = K.H1
    else:
        parts["markers"] = [
            "C7 WITNESS F-PT: 24 steps, every step enabled and consuming its choices as planned; 2 violations", K.M2_OBS, K.M3_OBS, K.M4_OBS]
        parts["summary"] = [K.E11_REC, K.P6_REC]
        parts["panic"] = "thread '%s' (4242) panicked at crates/nexus-governed-control/src/ledger/protocol/model_tests.rs:9705:9:" % TEST_WITNESS
        parts["tail"] = [K.NOTE_LINE, "FAILED", "", "failures:", "", "failures:", "    " + TEST_WITNESS, "",
                         "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 348 filtered out; finished in 0.02s", ""]
    return parts


def assemble(parts):
    replay = [l for b in parts["blocks"] for l in b]
    if parts["kind"] == "C":
        if parts.get("h1"):
            replay.append(parts["h1"])
        lines = ["", "running 1 test", "test %s ... %s" % (TEST_WITNESS, replay[0])] + replay[1:]
        lines += parts.get("pre_ok", []) + ["ok", "", "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 348 filtered out; finished in 0.01s", ""]
        return "\n".join(lines) + "\n"
    lines = ["", "running 1 test", "test %s ... %s" % (TEST_WITNESS, replay[0])] + replay[1:] + [""]
    lines += [parts["panic"]] + parts["markers"] + ["every violation:"] + parts["summary"] + parts["tail"]
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------------------------------------------- explorer
KEY_FIELDS = "state key fields: word (low 16 bits, live, unapplied; recomposed exactly); core: phase, session, latch cause"
BRANCHES = ["    [taken    ] append_facts: Committed (inserted)", "    [NOT TAKEN] append_facts: Committed (already present)",
            "    [taken    ] explain: Explained", "    [taken    ] extra thing (not in the listed set)"]


FROZEN_LINES = {"F-PT": K.frozen_config_lines("A")[0][1:], "F-3": K.frozen_config_lines("F3")[0][1:]}  # EXEC-02: the exact frozen bounds/actors lines


def instance(name, a4, outcome, prop=None, incomplete=None, steps=4, states=1234, extra_outcome=False, bounds=None):
    L = ["EXPLORATION %s" % name, "  outcome: %s" % outcome]
    if extra_outcome:
        L.append("  outcome: PASS")
    fb, fa = FROZEN_LINES.get(name, FROZEN_LINES["F-PT"])
    L += [bounds or fb,
          fa,
          "  states %d (expanded %d), transitions 5000 (with a fault 100), states with no enabled step 3" % (states, states),
          "  elapsed 41.5 s; explorer tables 12 MiB"]
    if incomplete:
        L.append("  INCOMPLETE: %s. No property is established by this run." % incomplete)
    if prop:
        L.append("  FAIL: %s: a table entry left in a step that did not see its explanation in the ledger" % prop)
        L.append("  shortest trace (%d steps):" % steps)
        L += ["    %3d. Fw(0): arm -> Armed%s" % (i + 1, "   [fault]" if i == 2 else "") for i in range(steps)]
        L.append("    end: phase Armed, latch None, U None, C None, X None, table Table { class: [Paid, Unattempted], debt: 0, h: 0, a: 0 }, qualified frontier 0, ledger [Prepared(0)]")
    else:
        for p in K.PROPERTIES:
            v = "NOT ESTABLISHED (incomplete)" if incomplete else (
                "HOLDS: 77 non-fault edges, no cycle; 3 end states, each terminal (D1)" if p == "L1" else
                "not applicable here (worker verbs do not unwind)" if p == "E10" else "HOLDS")
            L.append("  %s: %s" % (p, v))
    L.append("  branches:")
    L += BRANCHES
    L.append("  " + KEY_FIELDS)
    return L


def gen_explorer(kind, outcome, prop=None, incomplete=None, copies=None, extra_outcome=False, exit_override=None, panic_loc="4133:5", left=None, bounds=None):
    test = TEST_FULL_FPT if kind == "A" else TEST_FULL_F3
    name, a4 = ("F-PT", "true") if kind == "A" else ("F-3", "false")
    inst = instance(name, a4, outcome, prop, incomplete, extra_outcome=extra_outcome, bounds=bounds)
    lines = ["", "running 1 test", "test %s ... %s" % (test, inst[0])] + inst[1:]
    if outcome == "PASS":
        lines += ["", "ok", "", "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 348 filtered out; finished in 41.60s", ""]
        return "\n".join(lines) + "\n", 0
    ncopies = 1 if copies is None else copies
    lines += ["", ""]
    lines.append("thread '%s' (777) panicked at crates/nexus-governed-control/src/ledger/protocol/model_tests.rs:%s:" % (test, panic_loc))
    lines.append("assertion `left == right` failed: failed: %s" % ('{"%s"}' % prop if prop else "{}"))
    for _ in range(ncopies):
        lines += inst
    lines += ["", left or ("  left: Fail" if outcome == "FAIL" else "  left: Incomplete"), " right: Pass", K.NOTE_LINE, "FAILED", "", "failures:", "", "failures:", "    " + test, "",
              "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 348 filtered out; finished in 41.60s", ""]
    return "\n".join(lines) + "\n", 101


# ---------------------------------------------------------------------------------------------------------------- receipts
def good_desc(kind, raw_path, text, code):
    role_exe, role_sha, wt = (EW, SHA_W, WT_HEALTHY) if kind == "C" else (EW7, SHA_W7, WT_MUTANT)
    test = TEST_WITNESS if kind in ("C", "B") else (TEST_FULL_FPT if kind == "A" else TEST_FULL_F3)
    args = [test, "--exact", "--nocapture", "--test-threads=1"] if kind in ("C", "B") else [test, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
    ok = [{"id": "WT-X", "ok": True, "detail": "x"}]
    return {
        "id": "r", "type": "invocation", "label": kind, "mission": MISSION, "worktree": wt, "cwd": os.path.join(wt, "crates/nexus-governed-control"),
        "argv": [role_exe] + args, "env": {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "RUST_BACKTRACE": "0"}, "limit_seconds": 2700,
        "executable": {"path": role_exe, "sha256_before": role_sha, "sha256_after": role_sha, "size": 1000},
        "process": {"exit_status": code, "signal": None, "supervisor_rc": code, "wrapper_recorded_status": str(code)},
        "supervisor": {"memory_max": "17179869184", "swap_max": "0", "memory_high": "max", "memory_peak_bytes": "2000000000", "swap_peak_bytes": "0",
                       "oom": 0, "oom_kill": 0, "swap_events": {"high": 0, "max": 0, "fail": 0}, "completed": True, "timed_out": False, "wall_seconds": 12.5,
                       "time_limit_seconds": 2700, "scope_result": "success", "scope_show": "", "launcher_elapsed_seconds": 12.6, "max_rss_kib_largest_process": 1000,
                       "disk_floor_breach": False, "disk": {"min_free_bytes": DISK_FLOOR_BYTES + 2**30, "floor_bytes": DISK_FLOOR_BYTES},
                       "started_utc": "2026-10-16T13:00:00.000000000Z", "finished_utc": "2026-10-16T13:00:12.500000000Z"},
        "output_path": "raw-output.txt", "raw_output_sha256": hashlib.sha256(text.encode()).hexdigest(),
        "source_verification_before": ok, "source_verification_after": ok, "finished_at": "2026-10-15T23:59:59+01:00",
    }


def make(base, name, kind, text, code, edit=None):
    d = os.path.join(base, name)
    os.makedirs(d)
    open(os.path.join(d, "raw-output.txt"), "w", encoding="utf-8", newline="").write(text)
    desc = good_desc(kind, "raw-output.txt", text, code)
    if edit:
        edit(desc)
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)
    return d


def run(kind, d, pins):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = K.classify(kind, d, pins)
    cj = json.load(open(os.path.join(d, "CLASSIFICATION.json")))
    return rc, cj


def main(base):
    base = os.path.abspath(base)
    os.makedirs(base)
    pins = os.path.join(base, "pins")
    os.makedirs(pins)
    json.dump({"executable": {"path": EW, "sha256": SHA_W, "size": 1000}}, open(os.path.join(pins, "E-W.json"), "w"))
    json.dump({"executable": {"path": EW7, "sha256": SHA_W7, "size": 1000}}, open(os.path.join(pins, "E-W7.json"), "w"))
    counter = [0]

    def case(name, kind, text, code, want_verdict=None, want_fail=None, want_ok=None, edit=None):
        counter[0] += 1
        d = make(base, "%03d-%s" % (counter[0], kind), kind, text, code, edit)
        rc, cj = run(kind, d, pins)
        v = cj["verdict"]
        ok = True
        if want_verdict is not None:
            ok &= v == want_verdict
        if want_ok is True:
            ok &= rc == 0
        if want_ok is False:
            ok &= rc != 0 and not v.startswith(("PASS_ADM", "ADMISSIBLE", "FAIL_E11_ADM", "COMPLETED"))
        if want_fail:
            ok &= want_fail in cj["failed_checks"]
        expect(name, ok, "verdict=%s fails=%s%s" % (v, cj["failed_checks"][:6], "" if want_fail is None else " (want %s)" % want_fail))
        return cj

    def wit(kind, edit_parts=None, code=None):
        p = gen_witness(kind)
        if edit_parts:
            edit_parts(p)
        return assemble(p), (code if code is not None else (0 if kind == "C" else 101))

    # ---- positives
    case("(C) healthy synthetic output is PASS — ADMISSIBLE", "C", *wit("C"), want_verdict="PASS_ADMISSIBLE", want_ok=True)
    case("(B) mutant synthetic output is ADMISSIBLE with exactly two canonical records", "B", *wit("B"), want_verdict="ADMISSIBLE_TWO_CANONICAL", want_ok=True)

    # ---- (C) negatives
    def c_violation(p):
        p["blocks"][12].append(K.E11_REC)
    case("(C) with a VIOLATION line is a STOP (C7-13)", "C", *wit("C", c_violation, 0), want_ok=False, want_fail="C7-13")
    case("(C) exit status 101 is a STOP (C7-12)", "C", *wit("C", None, 101), want_ok=False, want_fail="C7-12")
    case("(C) without H1 is not admissible", "C", *wit("C", lambda p: p.pop("h1")), want_ok=False, want_fail="RP-1")
    case("(C) with H1 twice is not admissible", "C", *wit("C", lambda p: p["blocks"][23].append(K.H1)), want_ok=False)

    def c_changed(p):
        p["blocks"][22] = step_lines(23, "mutant")
    case("(C) whose step 23 took the changed end is not (C)", "C", *wit("C", c_changed), want_ok=False, want_fail="CL8-C")

    def c_offered(p):
        p["blocks"][7][0] = p["blocks"][7][0].replace("offered [0, 0, 0, 0]", "offered [0, 0, 0, 1]")
    case("(C) with a changed offered stream at step 8 breaks G.9", "C", *wit("C", c_offered), want_ok=False, want_fail="G9-08")

    def c_nofault(p):
        p["blocks"][14][0] = p["blocks"][14][0].replace("   [fault]", "")
    case("(C) with the fault flag missing at step 15 breaks G.9", "C", *wit("C", c_nofault), want_ok=False, want_fail="G9-15")

    def c_lost(p):
        p["blocks"][23] = step_lines(24, "mutant")
    case("(C) whose step 24 shows the lost state fails the kept predicate / pairing", "C", *wit("C", c_lost), want_ok=False)

    def c_table(p):
        p["blocks"][12][3] = "    refusal table after 13: {}"
    case("(C) whose refusal-table entry is missing after 13 fails the kept predicate", "C", *wit("C", c_table), want_ok=False, want_fail="CL9-C")

    def c_fix(p):
        p["blocks"][3].append("    WITNESS FIXTURE FAILURE (kills nothing): row 04 x")
    case("(C) with a fixture-failure line is not admissible", "C", *wit("C", c_fix), want_ok=False)
    case("(C) with an extra line in REPLAY is not admissible (strict schema)", "C", *wit("C", lambda p: p["blocks"][5].append("something unexpected")), want_ok=False, want_fail="RP-6")
    case("(C) with 23 step blocks is not admissible", "C", *wit("C", lambda p: p["blocks"].pop(10)), want_ok=False, want_fail="RP-2")

    def c_tail(p):
        p["pre_ok"] = ["note: stray"]
    case("(C) with a stray line before `ok` is not admissible", "C", *wit("C", c_tail), want_ok=False)
    case("(C) swapped onto the mutant executable is not admissible (C7-14)", "C", *wit("C"), want_ok=False, want_fail="ID-2",
         edit=lambda d: d["executable"].update(path=EW7, sha256_before=SHA_W7, sha256_after=SHA_W7))
    case("(C) run under the wrong argv is not admissible", "C", *wit("C"), want_ok=False, want_fail="ID-4",
         edit=lambda d: d["argv"].append("--ignored"))
    case("(C) with an oom kill is not admissible", "C", *wit("C"), want_ok=False, want_fail="CL1-3", edit=lambda d: d["supervisor"].update(oom_kill=1))
    case("(C) with nonzero swap is not admissible", "C", *wit("C"), want_ok=False, want_fail="CL1-4", edit=lambda d: d["supervisor"].update(swap_peak_bytes="4096"))
    case("(C) with a supervisor timeout is not admissible", "C", *wit("C"), want_ok=False, want_fail="CL1-2", edit=lambda d: d["supervisor"].update(timed_out=True))
    case("(C) with a signal is not admissible", "C", *wit("C"), want_ok=False, want_fail="CL1-1", edit=lambda d: d["process"].update(signal=9))
    case("(C) on the E-C executable hash is not admissible (C7-4)", "C", *wit("C"), want_ok=False, want_fail="ID-2",
         edit=lambda d: d["executable"].update(sha256_before=E_C_SHA, sha256_after=E_C_SHA))
    case("(C) with a source-verification failure after the run is not admissible", "C", *wit("C"), want_ok=False, want_fail="ID-8",
         edit=lambda d: d["source_verification_after"].append({"id": "WT-BLOBS", "ok": False, "detail": "x"}))
    text, code = wit("C")
    case("(C) with CRLF line endings is not admissible", "C", text.replace("\n", "\r\n"), code, want_ok=False, want_fail="RAW-1")
    case("(C) without a trailing newline is not admissible", "C", text.rstrip("\n"), code, want_ok=False, want_fail="RAW-1")

    # ---- (B) negatives
    def b_third(p):
        p["blocks"][23].append("    VIOLATION after 24: P7: a third record")
        p["summary"].append("    VIOLATION after 24: P7: a third record")
        p["markers"][0] = p["markers"][0].replace("2 violations", "3 violations")
    case("(B) with a third record is not admissible (C7-8)", "B", *wit("B", b_third), want_ok=False, want_fail="C7-5")

    def b_dup(p):
        p["blocks"][12].append(K.E11_REC)
        p["summary"].insert(1, K.E11_REC)
        p["markers"][0] = p["markers"][0].replace("2 violations", "3 violations")
    case("(B) with a second E11 record is not admissible", "B", *wit("B", b_dup), want_ok=False, want_fail="C7-8")

    def b_wrongblock(p):
        p["blocks"][12].remove(K.E11_REC)
        p["blocks"][13].append(K.E11_REC)
    case("(B) with E11 in step 14's block is not admissible (locus block)", "B", *wit("B", b_wrongblock), want_ok=False)

    def b_n(p):
        p["markers"][0] = p["markers"][0].replace("2 violations", "3 violations")
    case("(B) whose M1 says n=3 is not admissible (C7-9)", "B", *wit("B", b_n), want_ok=False, want_fail="C7-9b")

    def b_nomark(p):
        p["markers"].pop(2)
    case("(B) without M3 is not admissible", "B", *wit("B", b_nomark), want_ok=False, want_fail="C7-10a")

    def b_m2(p):
        p["markers"][1] = "NAMED PROPERTY P6: NOT OBSERVED on the world after step 24 (P6 record at that locus false; step 23 took the changed end false; lost state shown false)"
    case("(B) whose M2 disagrees with the raw state is not admissible", "B", *wit("B", b_m2), want_ok=False, want_fail="C7-10b")

    def b_m4(p):
        p["markers"][3] = 'OTHER PROPERTIES VIOLATED, each reported separately and not counted as P6: ["E11", "E4"]'
    case("(B) whose M4 lists another property is not admissible", "B", *wit("B", b_m4), want_ok=False, want_fail="C7-10d")

    def b_loc(p):
        p["panic"] = p["panic"].replace("9705:9", "9638:13")
    case("(B) panicking at the fixture-failure location is an unrelated panic (C7-11)", "B", *wit("B", b_loc), want_ok=False, want_fail="C7-11")

    def b_tailv(p):
        p["tail"].insert(0, K.E11_REC)
    case("(B) with a VIOLATION line in the TAIL is not admissible", "B", *wit("B", b_tailv), want_ok=False)

    def b_swap(p):
        p["summary"] = [K.P6_REC, K.E11_REC]
    case("(B) whose SUMMARY is not an ordered copy is not admissible", "B", *wit("B", b_swap), want_ok=False, want_fail="C7-9a")

    def b_noe11(p):
        p["blocks"][12].remove(K.E11_REC)
        p["summary"] = [K.P6_REC]
        p["markers"][0] = p["markers"][0].replace("2 violations", "1 violations")
        p["markers"][2] = "SEPARATE PROPERTY E11: NOT OBSERVED at the transition of step 13 (E11 record at that locus false)"
    case("(B) with P6 but without E11 is not admissible (C7-6)", "B", *wit("B", b_noe11), want_ok=False, want_fail="C7-6")

    def b_nop6(p):
        p["blocks"][23].remove(K.P6_REC)
        p["summary"] = [K.E11_REC]
        p["markers"][0] = p["markers"][0].replace("2 violations", "1 violations")
    case("(B) with E11 but without P6 is not admissible (C7-7)", "B", *wit("B", b_nop6), want_ok=False, want_fail="C7-7")

    def b_healthy23(p):
        p["blocks"][22] = step_lines(23, "healthy")
    case("(B) whose step 23 took the healthy end is not admissible (CL-8)", "B", *wit("B", b_healthy23), want_ok=False, want_fail="CL8-B")
    case("(B) with exit status 0 is not admissible", "B", *wit("B", None, 0), want_ok=False, want_fail="C7-17b")

    def b_h1(p):
        p["blocks"][23].append(K.H1)
    case("(B) containing H1 is not admissible", "B", *wit("B", b_h1), want_ok=False)

    def b_filtered(p):
        p["tail"][-2] = p["tail"][-2].replace("348 filtered", "347 filtered")
    case("(B) with `347 filtered out` is not admissible (C7-17)", "B", *wit("B", b_filtered), want_ok=False, want_fail="REG-4")

    def b_extra(p):
        p["blocks"][5].append("    VIOLATION step 06 in-step: P1: forged")
    case("(B) with an extra record inside a step block is not admissible", "B", *wit("B", b_extra), want_ok=False)

    def b_swapexe(p):
        pass
    case("(B) run on the healthy executable E-W is not admissible (C7-14)", "B", *wit("B", b_swapexe), want_ok=False, want_fail="ID-2",
         edit=lambda d: d["executable"].update(path=EW, sha256_before=SHA_W, sha256_after=SHA_W))
    case("(B) after hand-edit of the raw file (descriptor hash mismatch) is not admissible", "B", *wit("B"), want_ok=False, want_fail="ID-7",
         edit=lambda d: d.update(raw_output_sha256="0" * 64))
    case("(B) with the historical C7 executable hash is not admissible (C7-4)", "B", *wit("B"), want_ok=False, want_fail="ID-2",
         edit=lambda d: d["executable"].update(sha256_before=HIST_C7_SHA, sha256_after=HIST_C7_SHA))

    # ---- (A) explorer
    t, c = gen_explorer("A", "FAIL", prop="E11")
    case("(A) FAIL E11 with two byte-equal instances is ADMISSIBLE (C7-1)", "A", t, c, want_verdict="FAIL_E11_ADMISSIBLE", want_ok=True)
    t, c = gen_explorer("A", "FAIL", prop="P6")
    case("(A) printing P6 is a STOP for a ruling (C7-2); E11 is never relabelled", "A", t, c, want_verdict="STOP_C7-2_P6", want_ok=False)
    t, c = gen_explorer("A", "FAIL", prop="E4")
    case("(A) printing another property is a STOP (C7-3)", "A", t, c, want_verdict="STOP_C7-3", want_ok=False)
    t, c = gen_explorer("A", "PASS")
    case("(A) PASS is a STOP (C7-3)", "A", t, c, want_verdict="STOP_C7-3", want_ok=False)
    t, c = gen_explorer("A", "INCOMPLETE", incomplete="the state cap of 30000000 was reached")
    case("(A) INCOMPLETE is a STOP (C7-3)", "A", t, c, want_verdict="STOP_C7-3", want_ok=False)
    t, c = gen_explorer("A", "FAIL", prop="E11", copies=0)
    case("(A) FAIL with one instance only is not admissible", "A", t, c, want_ok=False)
    t, c = gen_explorer("A", "FAIL", prop="E11", copies=2)
    case("(A) FAIL with three instances is not admissible", "A", t, c, want_ok=False)
    t, c = gen_explorer("A", "FAIL", prop="E11", panic_loc="4140:5")
    case("(A) with the panic at another location is not admissible", "A", t, c, want_ok=False)
    t, c = gen_explorer("A", "FAIL", prop="E11")
    case("(A) with exit status 0 and a FAIL report is not admissible", "A", t, 0, want_ok=False)
    case("(A) on the E-W executable is not admissible", "A", t, c, want_ok=False, want_fail="ID-2",
         edit=lambda d: d["executable"].update(path=EW, sha256_before=SHA_W, sha256_after=SHA_W))
    case("(A) with a supervisor timeout is not admissible", "A", t, c, want_ok=False, want_fail="CL1-2", edit=lambda d: d["supervisor"].update(timed_out=True))
    t2 = t.replace("348 filtered out", "347 filtered out")
    case("(A) with `347 filtered out` is not admissible", "A", t2, c, want_ok=False)
    # EXEC-02 additions: the exact frozen configuration lines and the disk floor
    fb = FROZEN_LINES["F-PT"][0]
    t4, c4 = gen_explorer("A", "FAIL", prop="E11", bounds=fb.replace(" faults 2 ", " faults 3 "))
    case("(A) with a bounds line differing from the frozen F-PT bounds is not admissible (EX-2b)", "A", t4, c4, want_ok=False, want_fail="EX-2b")
    t5, c5 = gen_explorer("A", "FAIL", prop="E11", bounds=FROZEN_LINES["F-3"][0])
    case("(A) carrying F-3's bounds (A4 false) is not admissible (EX-2 / EX-2b)", "A", t5, c5, want_ok=False, want_fail="EX-2b")
    case("(A) with a disk-floor breach during the run is not admissible (CL1-8)", "A", t, c, want_ok=False, want_fail="CL1-8",
         edit=lambda d: d["supervisor"].update(disk_floor_breach=True))
    case("(C) whose sampled minimum free space fell below 16 GiB is not admissible (CL1-8)", "C", *wit("C"), want_ok=False, want_fail="CL1-8",
         edit=lambda d: d["supervisor"]["disk"].update(min_free_bytes=DISK_FLOOR_BYTES - 1))
    tq = t.replace("test %s ... EXPLORATION F-PT" % TEST_FULL_FPT, "test %s ... EXPLORATION F-PT (gating-caller symmetry quotient)" % TEST_FULL_FPT)
    case("(A) whose header names a quotient/symmetry explorer is not admissible (EX-3)", "A", tq, c, want_ok=False)
    t3 = t.replace("states 1234 (expanded 1234)", "states 31000000 (expanded 1234)")
    case("(A) with states beyond the ceiling is not admissible (C7-3)", "A", t3.replace("states 31000000", "states 31000000"), c, want_ok=False)

    # ---- F-3 (informational)
    t, c = gen_explorer("F3", "FAIL", prop="E11")
    cj = case("F-3 FAIL E11 is informational (as expected)", "F3", t, c, want_ok=True)
    expect("F-3 FAIL E11 text", cj["f3_text"] == "INFORMATIONAL F-3: FAIL E11 (as expected)", cj.get("f3_text", ""))
    t, c = gen_explorer("F3", "FAIL", prop="P6")
    cj = case("F-3 FAIL P6 is recorded verbatim and flagged (never a kill)", "F3", t, c, want_ok=True)
    expect("F-3 FAIL P6 flagged", "FLAGGED" in cj["f3_text"] and "never counted as P6" in cj["f3_text"], cj.get("f3_text", ""))
    t, c = gen_explorer("F3", "PASS")
    cj = case("F-3 PASS is recorded", "F3", t, c, want_ok=True)
    expect("F-3 PASS text", cj["f3_text"] == "INFORMATIONAL F-3: PASS", cj.get("f3_text", ""))
    t, c = gen_explorer("F3", "INCOMPLETE", incomplete="the time cap of 2700s was reached")
    cj = case("F-3 INCOMPLETE stays INCOMPLETE (F3-3)", "F3", t, c, want_ok=True)
    expect("F-3 INCOMPLETE text", "INCOMPLETE" in cj["f3_text"] and "F3-3" in cj["f3_text"], cj.get("f3_text", ""))
    t, c = gen_explorer("F3", "PASS")
    case("F-3 PASS report with exit 101 is NOT COMPLETED (F3-4)", "F3", t, 101, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11")
    case("F-3 FAIL report with exit 0 is NOT COMPLETED (F3-4)", "F3", t, 0, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11", copies=0)
    case("F-3 FAIL with one instance is NOT COMPLETED (F3-4)", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11", copies=2)
    case("F-3 FAIL with three instances is NOT COMPLETED (F3-4)", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11", panic_loc="1:1")
    case("F-3 with the panic at another location is NOT COMPLETED", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11", extra_outcome=True)
    case("F-3 with two outcome lines in an instance is NOT COMPLETED", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False)
    t, c = gen_explorer("F3", "FAIL", prop="E11")
    t_bad = t.replace("  left: Fail", "  left: Incomplete")
    case("F-3 with a mismatching `left:` line is NOT COMPLETED", "F3", t_bad, c, want_verdict="NOT_COMPLETED", want_ok=False)
    case("F-3 killed by a signal is NOT COMPLETED (supervisor facts)", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False, want_fail="CL1-1",
         edit=lambda d: d["process"].update(signal=9, exit_status=137, supervisor_rc=137, wrapper_recorded_status="137"))
    case("F-3 with an oom kill is NOT COMPLETED", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False, want_fail="CL1-3", edit=lambda d: d["supervisor"].update(oom=1, oom_kill=1))
    case("F-3 with a timeout is NOT COMPLETED", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False, want_fail="CL1-2", edit=lambda d: d["supervisor"].update(timed_out=True))
    t, c = gen_explorer("F3", "FAIL", prop="E11", bounds=FROZEN_LINES["F-PT"][0])
    case("F-3 carrying F-PT's bounds (A4 true) is NOT COMPLETED (EX-2b)", "F3", t, c, want_verdict="NOT_COMPLETED", want_ok=False, want_fail="EX-2b")
    t, c = gen_explorer("F3", "FAIL", prop="E11")
    case("F-3 whose instance is for F-PT is NOT COMPLETED", "F3", t.replace("EXPLORATION F-3", "EXPLORATION F-PT"), c, want_verdict="NOT_COMPLETED", want_ok=False)

    # ---- real sample: the C9 `full_fb2` FAIL E4 output exercises the explorer parser on a REAL report
    sample = open(os.path.join(SAMPLES, "C9-fullfpt-raw.txt"), encoding="utf-8").read()
    lines = sample.split("\n")[:-1]
    nb = [l for l in lines if l != ""]
    pfx = "test ledger::protocol::model_tests::full_fb2 ... "
    body = [nb[1][len(pfx):]] + nb[2:]
    end = [i for i, l in enumerate(body) if l.startswith("  state key fields: ")][0]
    info, probs = K.parse_instance(body[:end + 1])
    expect("real C9 sample: instance parses with no problems", probs == [], str(probs))
    expect("real C9 sample: outcome FAIL, property E4, 2-step trace", info["outcome"] == "FAIL" and info["fail"][0] == "E4" and info.get("trace_steps") == 2, str((info.get("outcome"), info.get("fail"))))
    counter[0] += 1
    d = make(base, "%03d-real" % counter[0], "F3", sample, 101)
    # name/test/filtered counts differ from the frozen target, so it must not classify as a completed F-3
    rc, cj = run("F3", d, pins)
    expect("real C9 sample is NOT accepted as the F-3 target (other test, 347 filtered)", rc != 0 and cj["verdict"] == "NOT_COMPLETED", cj["verdict"])

    print("\nSELF-TEST (c7class) %s: %d/%d" % ("ALL OK" if all(RESULTS) else "FAILED", sum(RESULTS), len(RESULTS)))
    return 0 if all(RESULTS) else 1
