#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 13: controls of the witness and full-run classifiers on
synthetic and historical inputs. Nothing here runs the pinned executable.

  checker_controls.py WORKDIR OUT.txt

Historical inputs: the preserved Stage-1 outputs (git objects of the Stage-1 evidence commit): the three witnesses'
segments of the unfiltered suite, re-framed as a standalone run, and F-2's full report, re-labelled as a remaining
configuration (header, bounds, test name). Synthetic descriptors in this mission's launcher format carry the process
and resource records. Each positive control must classify PASS; each negative control must classify NOT PASS; the
meta-controls require the harness itself to judge a wrong expectation FAILED.
"""
import copy
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402
import configs  # noqa: E402
import normalize  # noqa: E402

TOOLS = os.path.dirname(os.path.abspath(__file__))
S1 = common.STAGE1_PACKET
P = common.TEST_PREFIX
EXE_ID = dict(path=common.EXE, sha256=common.EXE_SHA256, size=common.EXE_SIZE, mtime_ns=0, inode=0, symlink=False)
TREE = dict(verdict="EXACTLY_C")


def descriptor(kind, test, argv_tail, raw, limit):
    return dict(
        id="synthetic", type=kind, test=test, worktree=common.WT, argv=[common.EXE, *argv_tail],
        env={"PATH": "/usr/bin:/bin"}, cwd="", prior=None, source_before=TREE, source_after=TREE,
        executable=dict(path=common.EXE, pinned_sha256=common.EXE_SHA256, pinned_size=common.EXE_SIZE,
                        before=dict(EXE_ID), after=dict(EXE_ID)),
        selection=dict(listing=dict(argv=[], exit=0, stdout="", listed=[test + ": test"]),
                       ignored_listing=dict(argv=[], exit=0, stdout="", listed=[]),
                       executable_after_listing=dict(EXE_ID)),
        process=dict(exit_status=0, signal=None, supervisor_rc=0, wrapper_recorded_status="0"),
        supervisor=dict(memory_max="17179869184", swap_max="0", memory_high="max", memory_peak_bytes="1000000",
                        swap_peak_bytes="0", oom=0, oom_kill=0, max_events=0, completed=True, timed_out=False,
                        wall_seconds=10.0, time_limit_seconds=limit, backstop_runtime_max_seconds=limit + 10,
                        scope_result="gone", scope_show={}, launcher_elapsed_seconds=10.1,
                        max_rss_kib_largest_process=1000),
        output_path="raw-output.txt", raw_output_sha256=common.sha256_bytes(raw.encode()))


def suite_witness_raw(test):
    text = common.blob(common.STAGE1, f"{S1}/receipts/30-suite/raw-output.txt").decode()
    seg, why = normalize.suite_segment(text, test.split("::")[-1])
    if seg is None:
        raise SystemExit(why)
    return f"\nrunning 1 test\ntest {test} ... {seg}\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; " \
           f"347 filtered out; finished in 0.00s\n\n"


def full_raw(cfg, req):
    f2 = common.blob(common.STAGE1, f"{S1}/receipts/41-full-F-2/raw-output.txt").decode()
    d = configs.derive()["required"]["F-2"]
    out = f2.replace(P + "full_f2", req["test"]).replace("EXPLORATION F-2\n", f"EXPLORATION {cfg}\n")
    out = out.replace("  bounds: " + d["bounds"], "  bounds: " + req["bounds"])
    out = out.replace("  actors: " + d["actors"], "  actors: " + req["actors"])
    return out


def run_case(work, name, tool, key, raw, desc):
    d = os.path.join(work, name)
    if os.path.exists(d):
        shutil.rmtree(d)
    os.makedirs(d)
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    desc = copy.deepcopy(desc)
    desc["raw_output_sha256"] = common.sha256_bytes(raw.encode())
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)
    r = subprocess.run([sys.executable, os.path.join(TOOLS, tool), key, d], capture_output=True, text=True,
                       timeout=600)
    cls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines() if os.path.exists(
        os.path.join(d, "CLASSIFICATION.txt")) else ["(no classification)"]
    failed = [l for l in cls if l.startswith("FAILED CHECK")]
    return r.returncode, cls[0], cls[1] if len(cls) > 1 else "", failed, r.stderr[-300:]


def main(work, out):
    os.makedirs(work, exist_ok=True)
    rep, results = [f"CLASSIFIER CONTROLS (synthetic and historical inputs; work directory {work})", ""], []

    def case(cid, what, tool, key, raw, desc, want_pass, expect_check=None):
        rc, first, outer, failed, err = run_case(work, cid, tool, key, raw, desc)
        got_pass = first == "CLASSIFICATION: PASS" and rc == 0
        named = expect_check is None or any(expect_check in f for f in failed)
        ok = (got_pass == want_pass) and (want_pass or named)
        results.append((cid, ok, got_pass, want_pass))
        rep.extend([f"{cid} — {what} ({tool} {key})", f"    {first}; {outer}",
                    *[f"    {f[:200]}" for f in failed[:4]],
                    *([f"    stderr: {err}"] if err.strip() else []),
                    f"    required: {'PASS' if want_pass else 'NOT PASS'}" +
                    (f" naming '{expect_check}'" if expect_check and not want_pass else "") +
                    f" -> CONTROL {'OK' if ok else 'FAILED'}", ""])

    # Witnesses.
    for key, test in (("c1", "c1_witness_p1_at_a_successful_admission"),
                      ("c25", "c25_witness_p7_on_the_resulting_world"), ("c4", "c4_witness_p4_and_p8_on_f8")):
        t = P + test
        raw = suite_witness_raw(t)
        desc = descriptor("witness", t, [t, "--exact", "--nocapture", "--test-threads=1"], raw, 180)
        case(f"W-{key}-PC", "Stage-1 suite segment re-framed as a standalone run", "check_witness.py", key, raw, desc,
             True)
    t = P + "c1_witness_p1_at_a_successful_admission"
    raw = suite_witness_raw(t)
    desc = descriptor("witness", t, [t, "--exact", "--nocapture", "--test-threads=1"], raw, 180)
    healthy = ("C1 WITNESS F-1: 11 steps, each enabled and taking its choices; no violation; step 11 shows the healthy "
               "core's 'admission: refused'")
    neg = [
        ("W-NC1", "a positive marker line", raw.replace(healthy, "NAMED PROPERTY P1: OBSERVED at step 11 (admission: "
                                                                 "admitted)\n" + healthy), desc, "positive marker"),
        ("W-NC2", "the healthy line altered", raw.replace(healthy, healthy.replace("refused", "admitted")), desc,
         "healthy line"),
        ("W-NC3", "the healthy line twice", raw.replace(healthy, healthy + "\n" + healthy), desc, "healthy line"),
        ("W-NC4", "exit 101", raw, dict(desc, process=dict(desc["process"], exit_status=101, supervisor_rc=101)),
         "normal exit"),
        ("W-NC5", "killed by signal 9 at the time limit", raw,
         dict(desc, process=dict(desc["process"], exit_status=None, signal=9),
              supervisor=dict(desc["supervisor"], timed_out=True, wall_seconds=180.0)), "normal exit"),
        ("W-NC6", "the test is ignored (listed with --ignored)", raw,
         dict(desc, selection=dict(desc["selection"], ignored_listing=dict(argv=[], exit=0, stdout="",
                                                                           listed=[t + ": test"]))), "not an ignored"),
        ("W-NC7", "the executable differs after the run", raw,
         dict(desc, executable=dict(desc["executable"], after=dict(EXE_ID, sha256="0" * 64))), "pinned"),
        ("W-NC8", "a violation line", raw.replace(healthy, "    VIOLATION step: P1: x\n" + healthy), desc,
         "positive marker"),
        ("W-NC9", "an OOM kill recorded", raw, dict(desc, supervisor=dict(desc["supervisor"], oom_kill=1)),
         "launching command"),
        ("W-NC10", "--ignored in argv", raw, dict(desc, argv=[common.EXE, t, "--exact", "--ignored", "--nocapture",
                                                              "--test-threads=1"]), "argv"),
    ]
    for cid, what, r2, d2, chk in neg:
        case(cid, what, "check_witness.py", "c1", r2, d2, False, chk)
    t4 = P + "c4_witness_p4_and_p8_on_f8"
    raw4 = suite_witness_raw(t4)
    desc4 = descriptor("witness", t4, [t4, "--exact", "--nocapture", "--test-threads=1"], raw4, 180)
    old = "step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [] unused [0, 0, 0, 1, 0, 0] faults left 2"
    if raw4.count(old) != 1:
        raise SystemExit("C4 control snippet not found once")
    case("W-NC11", "C4: the changed step consumes a choice (G.6)", "check_witness.py", "c4",
         raw4.replace(old, "step  8/8 Ack(0) offered [0, 0, 0, 1, 0, 0] consumed [(0, 4)] unused [0, 0, 1, 0, 0] "
                           "faults left 2"), desc4, False, "G.6")
    # Full runs.
    der = configs.derive()["required"]
    for cfg, key in (("F-4", "f4"), ("F-G", "fg")):
        req = der[cfg]
        raw = full_raw(cfg, req)
        desc = descriptor("full", req["test"], [req["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                          raw, 2700)
        case(f"F-{key}-PC", f"Stage-1 F-2 report re-labelled as {cfg}", "check_full.py", key, raw, desc, True)
    req = der["F-4"]
    base = full_raw("F-4", req)
    bdesc = descriptor("full", req["test"], [req["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                       base, 2700)
    # F-W: E10 must HOLD; the re-labelled F-2 prints E10 not applicable -> NOT PASS; with E10 HOLDS -> PASS.
    reqw = der["F-W"]
    rw = full_raw("F-W", reqw)
    dw = descriptor("full", reqw["test"], [reqw["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                    rw, 2700)
    case("F-fw-NC", "F-W with E10 printed 'not applicable'", "check_full.py", "fw", rw, dw, False, "E10")
    case("F-fw-PC", "F-W with E10 HOLDS (25 HOLD)", "check_full.py", "fw",
         rw.replace("  E10: not applicable here (worker verbs do not unwind)", "  E10: HOLDS"), dw, True)
    # F-D: L1 not claimed and DROP applicable.
    reqd = der["F-D"]
    rd = full_raw("F-D", reqd)
    l1 = next(l for l in rd.splitlines() if l.startswith("  L1: "))
    rdd = rd.replace(l1, "  L1: not claimed for this configuration")
    dd = descriptor("full", reqd["test"], [reqd["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                    rdd, 2700)
    case("F-fd-PC", "F-D with L1 not claimed (23 HOLD)", "check_full.py", "fd", rdd, dd, True)
    case("F-fd-NC", "F-D with L1 HOLDS (wrong applicability)", "check_full.py", "fd", rd, dd, False, "L1")
    st = next(l for l in base.splitlines() if l.startswith("  states "))
    fneg = [
        ("F-NC1", "INCOMPLETE line (state cap)", base.replace("  outcome: PASS", "  outcome: INCOMPLETE").replace(
            st, st + "\n  INCOMPLETE: the state cap of 30000000 was reached. No property is established by this run."),
         bdesc, "outcome PASS"),
        ("F-NC2", "discovered states above the cap", base.replace(st, st.replace("states 12602670 (expanded 12602670)",
                                                                                 "states 30000001 (expanded 30000001)")),
         bdesc, "<= 30,000,000"),
        ("F-NC3", "expanded below discovered", base.replace(st, st.replace("(expanded 12602670)", "(expanded 12602669)")),
         bdesc, "expanded"),
        ("F-NC4", "a property line not HOLDS", base.replace("  P7: HOLDS", "  P7: FAILS"), bdesc, "property line"),
        ("F-NC5", "bounds not the configuration's", base.replace("cell constraint true", "cell constraint false"), bdesc,
         "bounds"),
        ("F-NC6", "actors not the configuration's", base.replace("  actors: [Fw(0), Fw(1),", "  actors: [Fw(0),"),
         bdesc, "actors"),
        ("F-NC7", "a raw label outside the vocabulary",
         base.replace("    [taken    ] append_cell C: Refused(Latched) (not in the listed set)",
                      "    [taken    ] append_cell C: Refused(Bogus) (not in the listed set)\n"
                      "    [taken    ] append_cell C: Refused(Latched) (not in the listed set)"), bdesc, "branch list"),
        ("F-NC8", "a NOT APPLICABLE operation taken",
         base.replace("    [NOT TAKEN] snapshot: Taken", "    [taken    ] snapshot: Taken"), bdesc, "branch list"),
        ("F-NC9", "exit 101", base, dict(bdesc, process=dict(bdesc["process"], exit_status=101, supervisor_rc=101)),
         "normal exit"),
        ("F-NC10", "timed out", base, dict(bdesc, process=dict(bdesc["process"], exit_status=None, signal=9),
                                           supervisor=dict(bdesc["supervisor"], timed_out=True, wall_seconds=2700.0)),
         "normal exit"),
        ("F-NC11", "a quotient refusal line", base.replace("  outcome: PASS", "  outcome: PASS\nquotient inapplicable: x"),
         bdesc, "refusal"),
        ("F-NC12", "two tests listed", base, dict(bdesc, selection=dict(bdesc["selection"], listing=dict(
            argv=[], exit=0, stdout="", listed=[req["test"] + ": test", P + "full_f2: test"]))), "listing"),
        ("F-NC13", "a quotient explorer header", base.replace(
            "EXPLORATION F-4\n", "EXPLORATION F-4 (admission observer quotient: experimental, not the contract's "
                                 "explorer)\n"), bdesc, "header"),
        ("F-NC14", "source not C after the run", base, dict(bdesc, source_after=dict(verdict="NOT_C")), "source"),
        ("F-NC15", "a FAIL with its states beyond the cap is BUDGET EXCEEDED, never PASS",
         base.replace("  outcome: PASS", "  outcome: FAIL").replace(st, st.replace(
             "states 12602670 (expanded 12602670)", "states 30000002 (expanded 30000001)") + "\n  FAIL: P7: x"),
         bdesc, "outcome PASS"),
    ]
    for cid, what, r2, d2, chk in fneg:
        case(cid, what, "check_full.py", "f4", r2, d2, False, chk)
    # The F-NC15 outer class must be BUDGET EXCEEDED.
    cls15 = open(os.path.join(work, "F-NC15", "CLASSIFICATION.txt")).read()
    ok15 = "INCOMPLETE — BUDGET EXCEEDED" in cls15
    results.append(("F-NC15-class", ok15, None, None))
    rep += [f"F-NC15-class — the outer class reads 'INCOMPLETE — BUDGET EXCEEDED': {'OK' if ok15 else 'FAILED'}", ""]
    # Meta-controls: the harness judges a wrong expectation FAILED.
    m1 = (results[0][2] == (not results[0][3]))  # W-c1-PC judged against NOT PASS
    m2 = any(cid == "F-NC4" and gp == (not wp) for cid, ok, gp, wp in results if gp is not None)
    results += [("MC1", not m1, None, None), ("MC2", not m2, None, None)]
    rep += [f"MC1 — meta-control: W-c1-PC judged as if it had to be NOT PASS: the harness "
            f"{'would wrongly accept' if m1 else 'rejects it'} -> CONTROL {'OK' if not m1 else 'FAILED'}",
            f"MC2 — meta-control: F-NC4 judged as if it had to PASS: the harness "
            f"{'would wrongly accept' if m2 else 'rejects it'} -> CONTROL {'OK' if not m2 else 'FAILED'}", ""]
    allok = all(ok for _, ok, _, _ in results)
    rep.append(f"CLASSIFIER CONTROLS: {sum(ok for _, ok, _, _ in results)} of {len(results)} behave as required — "
               f"{'ALL OK' if allok else 'FAILED: ' + ', '.join(c for c, ok, _, _ in results if not ok)}")
    open(out, "w").write("\n".join(rep) + "\n")
    print("\n".join(rep))
    return 0 if allok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
