#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: positive and negative controls of check_full.py, run before any full
run. Nothing Nexus is built or run: each control is a synthetic receipt directory, in a scratch directory outside the
packet, made from a HISTORICAL raw output (the experimental F-B PASS at 2d83b74b, PT-FB-EXPERIMENT-01, and F-2 PASS
at 244c5b23, CAMPAIGN-01) with the current pin and a synthetic, consistent descriptor. Historical outputs are used
only as text fixtures; they are not evidence of this mission.

  checker_controls.py <scratch-dir>
"""
import copy
import hashlib
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_full  # noqa: E402

AUD = "/home/nexus/nexus-audits/claude-architect/"
FB_HIST = AUD + "p3-c11-dr01-d2a1-pt-fb-experiment-01/receipts/stage4/gc-symmetry-full-F-B.txt"


def historical_output(path, test):
    t = open(path).read()
    a = t.index("\nrunning 1 test\n")
    b = t.index("\n", t.index("\ntest result: ", a) + 1)
    out = t[a:b + 1]
    # The historical executable had 345 or more tests; this pinned one lists 348.
    return re.sub(r"(\d+) filtered out", "347 filtered out", out)


def campaign_f2():
    for root, _, files in os.walk(AUD + "p3-c11-dr01-d2a1-campaign-01/receipts"):
        for f in files:
            p = os.path.join(root, f)
            if f.endswith(".txt"):
                t = open(p, errors="replace").read()
                if "test ledger::protocol::model_tests::full_f2 ... EXPLORATION F-2" in t and "\nrunning 1 test\n" in t:
                    return p
    raise SystemExit("no historical F-2 output")


def descriptor(raw, test):
    pin = check_full.PIN
    ok_tree = {"verdict": "EXACTLY_C"}
    return {"argv": [pin["executable"], test, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
            "executable": {"path": pin["executable"], "sha256_before": pin["sha256"], "sha256_after": pin["sha256"]},
            "selection": {"listed": [test + ": test"], "exit": 0, "sha256_after": pin["sha256"]},
            "source_before": ok_tree, "source_after": ok_tree,
            "process": {"exit_status": 0, "signal": None, "supervisor_rc": 0},
            "supervisor": {"completed": True, "memory_max": "17179869184", "swap_max": "0", "oom_kill": 0, "oom": 0,
                           "swap_peak_bytes": "0", "timed_out": False, "time_limit_seconds": 2700, "wall_seconds": 60.0,
                           "memory_peak_bytes": "1", "scope_result": "gone"},
            "raw_output_sha256": hashlib.sha256(raw.encode()).hexdigest()}


def run_case(scratch, key, raw, mutate_desc=None):
    d = os.path.join(scratch, key + "-" + str(len(os.listdir(scratch))))
    os.makedirs(d)
    desc = descriptor(raw, check_full.RUNS[key]["test"])
    if mutate_desc:
        mutate_desc(desc)
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"))
    runs = copy.deepcopy(check_full.RUNS)
    saved_ev, saved_runs = check_full.EV, check_full.RUNS
    check_full.EV, check_full.RUNS = scratch, runs
    runs[key]["dir"] = os.path.basename(d)
    try:
        sys.stdout = open(os.devnull, "w")
        rc = check_full.main(key)
    finally:
        sys.stdout = sys.__stdout__
        check_full.EV, check_full.RUNS = saved_ev, saved_runs
    return rc, open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()[1]


def main(scratch):
    if os.path.exists(scratch):
        shutil.rmtree(scratch)
    os.makedirs(scratch)
    fb = historical_output(FB_HIST, check_full.RUNS["fb"]["test"])
    f2 = historical_output(campaign_f2(), check_full.RUNS["f2"]["test"])
    cases = [
        ("positive: F-B historical PASS text", "fb", fb, None, 0, "PASS — admissible unchanged-core composed-quotient"),
        ("positive: F-2 historical PASS text", "f2", f2, None, 0, "PASS — admissible unchanged-core original-explorer"),
        ("negative: F-B key count off by one", "fb", fb.replace("states 3775373 (expanded 3775373)",
                                                                  "states 3775374 (expanded 3775374)"), None, 1, "COUNT"),
        ("negative: F-2 state count off by one", "f2", f2.replace("states 12602670 (expanded 12602670)",
                                                                    "states 12602671 (expanded 12602671)"), None, 1, "COUNT"),
        ("negative: a symmetry refusal line", "fb", fb.replace("\nok\n", "\nsymmetry inapplicable: injected\nok\n"), None, 1,
         "INCOMPLETE — VERIFICATION INVALID"),
        ("negative: PASS printed, then killed by signal 9", "fb", fb, lambda d: d["process"].update(exit_status=None, signal=9),
         1, "INCOMPLETE (killed"),
        ("negative: timeout", "fb", fb, lambda d: d["supervisor"].update(timed_out=True, wall_seconds=2700.0), 1,
         "INCOMPLETE (killed"),
        ("negative: OOM kill recorded", "fb", fb, lambda d: d["supervisor"].update(oom_kill=1), 1, "INCOMPLETE (killed"),
        ("negative: executable sha256 changed after", "fb", fb,
         lambda d: d["executable"].update(sha256_after="0" * 64), 1, "NOT ADMISSIBLE"),
        ("negative: E10 line altered", "fb", fb.replace("E10: not applicable here (worker verbs do not unwind)",
                                                        "E10: HOLDS"), None, 1, "NOT ADMISSIBLE"),
        ("negative: a branch line removed", "fb", re.sub(r"\n    \[[^\]]*\] lookup: Absent\n", "\n", fb, count=1), None, 1,
         "NOT ADMISSIBLE"),
        ("negative: two tests selected", "fb", fb, lambda d: d["selection"].update(listed=["a: test", "b: test"]), 1,
         "NOT ADMISSIBLE"),
        ("negative: wrong bounds (cap raised)", "fb", fb.replace("caps 30000000 states", "caps 30000001 states"), None, 1,
         "NOT ADMISSIBLE"),
    ]
    good = True
    for label, key, raw, mut, want_rc, want_class in cases:
        rc, cls = run_case(scratch, key, raw, mut)
        ok = rc == want_rc and want_class in cls
        good &= ok
        print(f"{'OK  ' if ok else 'FAIL'} {label}: rc {rc}; {cls}")
    print(f"CHECKER CONTROLS: {'ALL AS EXPECTED' if good else 'UNEXPECTED RESULT'}")
    return 0 if good else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
