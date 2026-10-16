#!/usr/bin/env python3
"""EXEC-02 self-test of c7bwc.py WITHOUT any cargo build: the strict libtest listing parser (real libtest fixtures and E-C's
listings read from git objects) and the new 13.3 order checks (synthetic receipts in a scratch directory).
The EXEC-01 c7bwc selftest builds a throwaway crate with cargo; this mission runs no cargo outside its two builds, so that
part is omitted and recorded as omitted.
  st_bwc_nocargo.py <new-scratch-dir>
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c7bwc as B
from c7lib import *  # noqa

res = []


def expect(name, cond, det=""):
    res.append(bool(cond))
    print("[%s] %s %s" % ("OK" if cond else "FAILED", name, det), flush=True)


def main(base):
    os.makedirs(base)
    fx = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "fixtures", "listfix")
    rd = lambda n: open(os.path.join(fx, n), "rb").read()  # noqa
    p = B.parse_listing(rd("f1.txt"))
    expect("parser: real libtest --list (3 tests)", p["count"] == 3 and not p["problems"] and p["tests"][0] == "m::ignored_one", str(p["problems"]))
    p = B.parse_listing(rd("f2.txt"))
    expect("parser: real libtest --list --ignored (1 test, singular trailer)", p["count"] == 1 and not p["problems"])
    p = B.parse_listing(rd("f6.txt"))
    expect("parser: real libtest empty selection", p["count"] == 0 and p["tests"] == [] and not p["problems"])
    expect("render_listing reproduces real output byte for byte", all(B.render_listing(B.parse_listing(rd(n))["tests"]) == rd(n) for n in ("f1.txt", "f2.txt", "f3.txt", "f6.txt")))
    ec = B.load_ec(B.default_cfg())
    pa = B.parse_listing(ec["all_raw"])
    expect("parser: E-C 20-list-all (348 tests, sorted)", pa["count"] == 348 and not pa["problems"] and pa["tests"] == sorted(pa["tests"]), str(pa["problems"]))
    expect("render_listing reproduces E-C 20-list-all byte for byte", B.render_listing(pa["tests"]) == ec["all_raw"])
    pi = B.parse_listing(ec["ign_raw"])
    expect("parser: E-C 21-list-ignored (35 tests)", pi["count"] == 35 and not pi["problems"])
    expect("E-C listings hash to the frozen values", ec["all_sha"] == EC_LIST_ALL_SHA and ec["ign_sha"] == EC_LIST_IGN_SHA)
    expect("E-C toolchain lines read from the E-C build record", len(ec["rustc_lines"]) >= 5 and len(ec["cargo_lines"]) >= 5, "%d/%d" % (len(ec["rustc_lines"]), len(ec["cargo_lines"])))
    bad = [b"", b"1 test, 0 benchmarks", b"a::b: test\n1 test, 0 benchmarks\n", b"a::b: test\n\n2 tests, 0 benchmarks\n", b"a::b: test\n\n1 tests, 0 benchmarks\n",
           b"a::b: test\na::b: test\n\n2 tests, 0 benchmarks\n", b"a::b: bench\n\n1 test, 0 benchmarks\n", b"junk\n0 tests, 0 benchmarks\n", b"a: test\n\n1 test, 1 benchmark\n"]
    expect("parser rejects 9 malformed listings", all(B.parse_listing(b)["problems"] for b in bad))
    # order checks on synthetic receipts
    def mk(ev, shift=None):
        t = [0]

        def ts():
            t[0] += 1
            return "2026-10-16T13:00:%02d.000000000Z" % t[0]

        def desc(path):
            os.makedirs(path, exist_ok=True)
            a, b = ts(), ts()
            json.dump({"supervisor": {"started_utc": a, "finished_utc": b}}, open(os.path.join(path, "descriptor.json"), "w"))
        r = os.path.join(ev, "receipts")
        desc(os.path.join(r, "10-build-E-W"))
        for sp in B.LIST_SPECS:
            desc(os.path.join(r, "20-list-E-W", sp[0]))
        os.makedirs(os.path.join(r, "25-stage3-gate"))
        a, b = ts(), ts()
        json.dump({"started_utc": a, "finished_utc": b, "admissible": True, "mutant_worktree_existed": False}, open(os.path.join(r, "25-stage3-gate/STAGE3-E-W.json"), "w"))
        os.makedirs(os.path.join(r, "00-worktrees"))
        for n in ("creation-M.json", "apply-C7.json"):
            a, b = ts(), ts()
            json.dump({"started_utc": a, "finished_utc": b}, open(os.path.join(r, "00-worktrees", n), "w"))
        desc(os.path.join(r, "11-build-E-W7"))
        for sp in B.LIST_SPECS:
            desc(os.path.join(r, "21-list-E-W7", sp[0]))
        if shift:
            shift(r)
    ev = os.path.join(base, "good")
    mk(ev)
    oc = B.order_checks(ev)
    expect("order checks: the compliant 13.3 order passes", all(c[2] for c in oc) and len(oc) == 3, str([(c[0], c[2], c[3]) for c in oc]))

    def old_order(r):
        # EXEC-01's order: E-W7 built before the E-W listings ran
        j = json.load(open(os.path.join(r, "11-build-E-W7/descriptor.json")))
        j["supervisor"]["started_utc"], j["supervisor"]["finished_utc"] = "2026-10-16T13:00:02.500000000Z", "2026-10-16T13:00:02.600000000Z"
        json.dump(j, open(os.path.join(r, "11-build-E-W7/descriptor.json"), "w"))
    ev2 = os.path.join(base, "exec01-order")
    mk(ev2, old_order)
    oc = B.order_checks(ev2)
    expect("order checks: the EXEC-01 order (E-W7 built before the E-W listings) FAILS ORD-2", [c[0] for c in oc if not c[2]] == ["ORD-2"], str([(c[0], c[2]) for c in oc]))

    def gate_late(r):
        j = json.load(open(os.path.join(r, "25-stage3-gate/STAGE3-E-W.json")))
        j["mutant_worktree_existed"] = True
        json.dump(j, open(os.path.join(r, "25-stage3-gate/STAGE3-E-W.json"), "w"))
    ev3 = os.path.join(base, "gate-late")
    mk(ev3, gate_late)
    oc = B.order_checks(ev3)
    expect("order checks: a Stage-3 gate taken after the mutant worktree existed FAILS ORD-3", [c[0] for c in oc if not c[2]] == ["ORD-3"], str([(c[0], c[2]) for c in oc]))
    ev4 = os.path.join(base, "missing")
    mk(ev4, lambda r: os.remove(os.path.join(r, "00-worktrees/apply-C7.json")))
    oc = B.order_checks(ev4)
    expect("order checks: a missing receipt FAILS ORD-0", [c[0] for c in oc if not c[2]] == ["ORD-0"], str(oc))
    # PROC-8: disk floor
    d = {"process": {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"},
         "supervisor": {"completed": True, "timed_out": False, "scope_result": "gone", "oom": 0, "oom_kill": 0, "swap_peak_bytes": "0", "swap_events": {},
                        "memory_max": "17179869184", "swap_max": "0", "memory_peak_bytes": "1000", "wall_seconds": 1.0, "time_limit_seconds": 2700,
                        "disk_floor_breach": False, "disk": {"min_free_bytes": DISK_FLOOR_BYTES + 1}}}
    expect("PROC-1..8 pass on a clean synthetic descriptor", all(c[2] for c in B.proc_checks(d, "x", 2700)))
    d["supervisor"]["disk"]["min_free_bytes"] = DISK_FLOOR_BYTES - 1
    expect("PROC-8 fails when the sampled minimum free space fell below 16 GiB", [c[0] for c in B.proc_checks(d, "x", 2700) if not c[2]] == ["PROC-8"])
    d["supervisor"]["disk"]["min_free_bytes"] = DISK_FLOOR_BYTES + 1
    d["supervisor"]["disk_floor_breach"] = True
    expect("PROC-8 fails on a recorded floor breach", [c[0] for c in B.proc_checks(d, "x", 2700) if not c[2]] == ["PROC-8"])
    print("\nSELF-TEST (c7bwc, no cargo) %s: %d/%d" % ("ALL OK" if all(res) else "FAILED", sum(res), len(res)))
    return 0 if all(res) else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
