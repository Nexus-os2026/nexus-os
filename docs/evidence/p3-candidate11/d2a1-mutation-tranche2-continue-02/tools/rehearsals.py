#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01, mission sections 7 and 16: rehearse render.py and build_packet.py on
SYNTHETIC evidence roots before the freeze. Nothing is built or run; no Nexus executable, Cargo or rustc.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 16 and 19): this mission's layout (C5: admission,
preserved worktree, pin, run, reauthentication, link; A3, A6, C8, C15 as before), its four derived statuses and its
previous packets (the Tranche-2 packet among them).

    rehearsals.py OUTDIR SCRATCH     both must not exist. SCRATCH (under dev-local/, never published) holds the
                                     synthetic evidence roots and their staged packets; OUTDIR (receipts/04b-rehearsals)
                                     receives REHEARSALS.txt, REHEARSALS.json and SUMMARY.txt (last line `REHEARSALS: ALL
                                     AS EXPECTED` or `REHEARSALS: FAILED`).

Each synthetic root holds byte copies of this mission's tools (tools/common.py naming that root as EV: the only
difference), original-tools/, inputs/, documents/, development/, the expectations and the supervision self-test, and
synthetic control directories in the layout of the real receipts, built from receipts/03-c2-controls' synthetic runs
and links (A3-C15: a synthetic build receipt with an empty build.stdout, the sampler's constant record, a restoration and
a RESULT; C5: synthetic admission, worktree, pin and reauthentication records, no build or restoration). Scenarios:
  R1  all five controls candidates                  -> READY_FOR_MUTATION_TRANCHE2_FINAL_ARCHITECT_REVIEW; packet STAGED
  R2  C5 candidate, A3's build NOT OK (a warning)    -> MUTATION_CONTROL_BLOCKED; packet STAGED; A6 C8 C15 not attempted
  R3  no control attempted (the pre-execution gate failed) -> BLOCKED
  R4  C5's preserved executable differs              -> C5_PRESERVED_ARTIFACT_BLOCKED; packet STAGED
  N1  a receipt byte-equal to a previous-packet file at a path outside the enumerated records -> UNAUTHORIZED_DUPLICATE
  N2  an enumerated sampler path holding another previous-packet file's bytes            -> LITERAL_MISMATCH
  N3  an empty file at a path outside the enumerated zero-length artifacts                -> ZERO_LENGTH_UNLISTED
  N4  an empty build.stdout whose descriptor does not record it                           -> ZERO_DESCRIPTOR
  N5  a whitespace error in an authored document                                          -> whitespace check fails
  N6  a staged copy of a Candidate C source file (no mutant source is ever written here)    -> source-copy check fails
  N7  a staged copy of a Tranche-2 packet file (C5's historical build CHECKS.txt)          -> UNAUTHORIZED_DUPLICATE
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c5  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402

results = []
CAND_RESULT = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
BLOCKED_RESULT = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED"
SAMPLER = "sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n"
SYN = "SYNTHETIC (rehearsals.py): not the record of any build or run"


def record(case, expected, got, ok, detail=""):
    results.append({"case": case, "expected": expected, "got": got, "as_expected": bool(ok), "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def make_root(root, exp, scenario):
    ev = os.path.join(root, "ev")
    os.makedirs(ev)
    for sub in ("tools", "original-tools", "inputs", "documents", "development"):
        shutil.copytree(os.path.join(K.EV, sub), os.path.join(ev, sub), symlinks=True,
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    c = os.path.join(ev, "tools/common.py")
    os.chmod(c, 0o644)
    s = open(c).read()
    assert s.count(f'EV = "{K.EV}"') == 1
    open(c, "w").write(s.replace(f'EV = "{K.EV}"', f'EV = "{ev}"'))
    os.makedirs(os.path.join(ev, "receipts"))
    for r in ("02-EXPECTATIONS.json", "01-supervision-selftest", "00-authentication.txt"):
        src = os.path.join(K.EV, "receipts", r)
        (shutil.copytree if os.path.isdir(src) else shutil.copyfile)(src, os.path.join(ev, "receipts", r))
    st = os.path.join(ev, "receipts/01-supervision-selftest/SELFTEST.txt")
    if os.path.exists(st):  # the copy names the synthetic root, as the real self-test names the real one
        text = open(st).read()
        open(st, "w").write(text.replace(K.EV + "/", ev + "/"))
    gate = "PRE-EXECUTION GATE: ALL CHECKS HOLD" if scenario != "R3" else "PRE-EXECUTION GATE: FAILED: synthetic"
    open(os.path.join(ev, "receipts/06-PRE-EXECUTION-GATE.txt"), "w").write(gate + "\n" + SYN + "\n")
    t2 = os.path.join(K.EV, "receipts/03-c2-controls")
    order = {"R1": K.ORDER, "R2": K.ORDER[:2], "R3": [], "R4": K.ORDER[:1]}[scenario]
    for cid in order:
        ctl = exp["controls"][cid]
        d = os.path.join(ev, ctl["dir"])
        if cid == "C5":
            c5_root(d, ctl, t2, scenario)
            continue
        os.makedirs(os.path.join(d, "03-build"))
        open(os.path.join(d, "00-GATE.txt"), "w").write("GATE: OPEN\n" + SYN + "\n")
        os.makedirs(os.path.join(d, "02-apply"))
        open(os.path.join(d, "02-apply/APPLY.txt"), "w").write("APPLY: EXACTLY_C_PLUS_PATCH\n" + SYN + "\n")
        warn = scenario == "R2" and cid == "A3"
        stderr = (f"{SYN}\n" + ("warning: unused variable: `nonce`\n" if warn else "")
                  + "    Finished `release` profile [optimized] target(s) in 1.00s\n")
        open(os.path.join(d, "03-build/build.stderr"), "w").write(stderr)
        open(os.path.join(d, "03-build/build.stdout"), "w").write("")
        open(os.path.join(d, "03-build/cgroup-samples.txt"), "w").write(SAMPLER)
        open(os.path.join(d, "03-build/RECEIPT.txt"), "w").write(f"SUPERVISED MUTANT BUILD — control {cid} ({SYN})\n")
        json.dump({"id": "03-build", "type": "build", "control": cid, "worktree": ctl["worktree"], "argv": K.BUILD_ARGV,
                   "supervisor": {"memory_max": K.MEMORY_MAX}, "stdout_path": "build.stdout",
                   "stdout_sha256": K.sha256_bytes(b""), "stderr_path": "build.stderr",
                   "stderr_sha256": K.sha256_bytes(stderr.encode()), "process": {"exit_status": 0, "signal": None},
                   "synthetic": SYN}, open(os.path.join(d, "03-build/descriptor.json"), "w"), indent=1)
        open(os.path.join(d, "03-build/CLASSIFICATION.txt"), "w").write(
            ("BUILD: NOT OK\nouter class (AD-1-R4 K.2): BUILD NOT OK (no error or warning diagnostic): kills nothing; the "
             "mission stops\n" if warn else "BUILD: PINNED\nouter class (AD-1-R4 K.2): mutant build succeeded; one "
             "library-test executable pinned\n"))
        if not warn:
            src = os.path.join(t2, "A", sorted(x for x in os.listdir(os.path.join(t2, "A"))
                                               if x.split("-")[1] == cid and "P-first" in x)[0])
            shutil.copytree(src, os.path.join(d, "04-run"))
            open(os.path.join(d, "04-run/cgroup-samples.txt"), "w").write(SAMPLER)
            open(os.path.join(d, "04-run/RECEIPT.txt"), "w").write(f"SUPERVISED MUTANT RUN — control {cid}: {SYN}\n")
            shutil.copytree(os.path.join(t2, "B", f"{cid}-real-inputs"), os.path.join(d, "06-link"))
            json.dump({"id": "03-build", "path": "/synthetic", "sha256": "0" * 64, "size": 1},
                      open(os.path.join(d, "03-build/PIN.json"), "w"))
        os.makedirs(os.path.join(d, "05-restore"))
        open(os.path.join(d, "05-restore/RESTORE.txt"), "w").write(f"RESTORE: EXACTLY_C\ncontrol {cid}: {SYN}\n")
        res = BLOCKED_RESULT if warn else CAND_RESULT
        facts = [f"apply: APPLY: EXACTLY_C_PLUS_PATCH", f"build: {'BUILD: NOT OK' if warn else 'BUILD: PINNED'}"] + (
            ["STOP: the mutant build did not succeed (kills nothing; no rebuild). No later control is executed."]
            if warn else [])
        open(os.path.join(d, "RESULT.txt"), "w").write("\n".join([res, f"control {cid}: {ctl['description']}",
                                                                  f"receipt type: {ctl['receipt_type']}"] + facts) + "\n")
        json.dump({"control": cid, "status": res, "facts": facts}, open(os.path.join(d, "CONTROL-RESULT.json"), "w"),
                  indent=1)
    return ev


def c5_root(d, ctl, t2, scenario):
    """C5's synthetic receipts in the layout of control.main_c5 (no apply, build or restoration)."""
    os.makedirs(d)
    open(os.path.join(d, "00-GATE.txt"), "w").write("GATE: OPEN\n" + SYN + "\n")
    for sub, name, first in (("01-admission", "ADMISSION", c5.ADMIT_OK), ("02-worktree", "AUTHENTICATE-BEFORE", c5.WT_OK),
                             ("03-pin", "PIN", c5.PIN_OK if scenario != "R4" else c5.PIN_FAIL)):
        os.makedirs(os.path.join(d, sub))
        open(os.path.join(d, sub, name + ".txt"), "w").write(first + "\n" + SYN + "\n")
        json.dump({"synthetic": SYN}, open(os.path.join(d, sub, name + ".json"), "w"))
    if scenario == "R4":
        res, facts = "CONTROL RESULT: C5_PRESERVED_ARTIFACT_BLOCKED", [f"preserved executable: {c5.PIN_FAIL}",
                                                                       "STOP: C5_PRESERVED_ARTIFACT_BLOCKED"]
    else:
        json.dump({"path": "/synthetic", "sha256": "0" * 64, "size": 1}, open(os.path.join(d, "03-pin/PIN.json"), "w"))
        src = os.path.join(t2, "A", sorted(x for x in os.listdir(os.path.join(t2, "A"))
                                           if x.split("-")[1] == "C5" and "P-as-main_c5" in x)[0])
        shutil.copytree(src, os.path.join(d, "04-run"))
        open(os.path.join(d, "04-run/cgroup-samples.txt"), "w").write(SAMPLER)
        open(os.path.join(d, "04-run/RECEIPT.txt"), "w").write(f"SUPERVISED MUTANT RUN — control C5: {SYN}\n")
        os.makedirs(os.path.join(d, "05-reauth"))
        open(os.path.join(d, "05-reauth/REAUTH.txt"), "w").write(control.REAUTH_OK + "\n" + SYN + "\n")
        shutil.copytree(os.path.join(t2, "B", "C5-real-inputs"), os.path.join(d, "06-link"))
        res, facts = CAND_RESULT, [f"build admission: {c5.ADMIT_OK}", f"preserved executable: {c5.PIN_OK}"]
    open(os.path.join(d, "RESULT.txt"), "w").write("\n".join([res, f"control C5: {ctl['description']}",
                                                              f"receipt type: {ctl['receipt_type']}"] + facts) + "\n")
    json.dump({"control": "C5", "status": res, "facts": facts}, open(os.path.join(d, "CONTROL-RESULT.json"), "w"),
              indent=1)


def run_tool(ev, args):
    r = subprocess.run([sys.executable, "-B", *args], capture_output=True, text=True,
                       env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    return r.returncode, r.stdout + r.stderr


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    print(f"REHEARSALS — {K.MISSION} (synthetic evidence roots under {scratch}; nothing is built or run)")
    for sc, want, want_packet in (("R1", "READY_FOR_MUTATION_TRANCHE2_FINAL_ARCHITECT_REVIEW", True),
                                  ("R2", "MUTATION_CONTROL_BLOCKED", True), ("R3", "BLOCKED", True),
                                  ("R4", "C5_PRESERVED_ARTIFACT_BLOCKED", True)):
        ev = make_root(os.path.join(scratch, sc), exp, sc)
        os.makedirs(os.path.join(scratch, sc, "render"))
        rc, o = run_tool(ev, [os.path.join(ev, "tools/render.py"), os.path.join(scratch, sc, "render")])
        open(os.path.join(out, f"{sc}-render.console.txt"), "w").write(o)
        st = [l for l in o.splitlines() if l.startswith("STATUS: ")]
        record(f"{sc} render.py status", want, st[0] if st else o[-200:], rc == 0 and st and st[0].startswith(
            f"STATUS: {want};"))
        summ = open(os.path.join(scratch, sc, "render/TRANCHE-SUMMARY.md")).read()
        if sc == "R2":
            record("R2 summary: A3 stopped; A6, C8, C15 not attempted; 1 candidate",
                   "stopped A3; not attempted A6 C8 C15; 1 of 5",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- Authorized but not", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): A3." in summ
                   and "- Authorized but not attempted: A6, C8, C15." in summ and "**1 of 5**" in summ)
        bad_words = [w for w in ("F-R PASS", "F-R ORIGINAL GRAPH PASSED", "F-R ORIGINAL GRAPH EXPLORED", "D2A-1 ACCEPTED",
                                 "ALL MUTATIONS COMPLETE", "PHASE THREE COMPLETE") if w in summ or w in open(
            os.path.join(scratch, sc, "render/INDEX.md")).read()]
        record(f"{sc} no prohibited wording in INDEX.md or TRANCHE-SUMMARY.md", "none", bad_words, not bad_words)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/build_packet.py"), os.path.join(scratch, sc, "packet")])
        open(os.path.join(out, f"{sc}-packet.console.txt"), "w").write(o)
        last = o.rstrip().splitlines()[-1]
        record(f"{sc} build_packet.py", "PACKET: STAGED — ALL CHECKS HOLD", last,
               (last == "PACKET: STAGED — ALL CHECKS HOLD") == want_packet and rc == (0 if want_packet else 1))
        if sc == "R1":
            m = open(os.path.join(scratch, sc, "packet/documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv")).read().splitlines()
            record("R1 map: the four build.stdout and the sampler records mapped, bound to their descriptors (C5: "
                   "no build of its own; its run's sampler)",
                   "4 ZERO_LENGTH build.stdout rows; 8 control CGROUP_CONSTANT rows for A3-C15 and 1 for C5's run",
                   f"{sum('/03-build/build.stdout' in l for l in m)} build.stdout, "
                   f"{sum(1 for l in m if l.startswith('CGROUP_CONSTANT') and not l.split(chr(9))[1].startswith(
                       'receipts/01-'))} control samplers",
                   sum("/03-build/build.stdout" in l for l in m) == 4
                   and sum(1 for l in m if l.startswith("CGROUP_CONSTANT\t") and l.split("\t")[1].startswith(
                       ("receipts/62-", "receipts/63-", "receipts/64-", "receipts/65-"))) == 8
                   and sum(1 for l in m if l.startswith("CGROUP_CONSTANT\treceipts/61-C5/04-run/")) == 1)
    prior = K.git("show", f"{K.T1}:{K.CONTINUE02_PACKET}/receipts/38-C9/04-run/CHECKS.txt")
    prior2 = K.git("show", f"{K.T1}:{K.EXEC01_PACKET}/receipts/01-supervision-selftest/1/out.txt")
    prior3 = K.git("show", f"{K.T2}:{K.C5_BUILD_DIR}/CHECKS.txt")
    negs = [("N1", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/62-A3/04-run/EXTRA.txt"),
                                                               "wb").write(prior)),
            ("N2", "LITERAL_MISMATCH", lambda ev: open(os.path.join(ev, "receipts/63-A6/04-run/cgroup-samples.txt"),
                                                         "wb").write(prior2)),
            ("N3", "ZERO_LENGTH_UNLISTED", lambda ev: open(os.path.join(ev, "receipts/64-C8/04-run/EMPTY.txt"), "w")
             .write("")),
            ("N4", "ZERO_DESCRIPTOR", lambda ev: json.dump(dict(json.load(open(os.path.join(
                ev, "receipts/65-C15/03-build/descriptor.json"))), stdout_sha256="0" * 64), open(os.path.join(
                ev, "receipts/65-C15/03-build/descriptor.json"), "w"))),
            ("N5", "every file this mission authored is whitespace-clean", lambda ev: open(os.path.join(
                ev, "documents/NOTE.md"), "w").write("a synthetic note with a trailing space \n")),
            ("N6", "no source copy", lambda ev: open(os.path.join(ev, "receipts/64-C8/04-run/copy.txt"), "wb").write(
                K.blob(K.C, K.UNIT))),
            ("N7", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/61-C5/01-admission/COPY.txt"),
                                                               "wb").write(prior3))]
    for n, want, act in negs:
        ev = make_root(os.path.join(scratch, n), exp, "R1")
        act(ev)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/build_packet.py"), os.path.join(scratch, n, "packet")])
        open(os.path.join(out, f"{n}-packet.console.txt"), "w").write(o)
        last = o.rstrip().splitlines()[-1]
        record(f"{n} build_packet.py rejects ({want})", f"PACKET: FAILED, naming {want}", last[:160],
               rc == 1 and last.startswith("PACKET: FAILED") and want in o)
    bad = [r for r in results if not r["as_expected"]]
    open(os.path.join(out, "REHEARSALS.txt"), "w").write(
        "\n".join(f"{'AS EXPECTED' if r['as_expected'] else 'UNEXPECTED '} {r['case']}: expected {r['expected']}; got "
                  f"{r['got']}" for r in results) + "\n")
    json.dump(results, open(os.path.join(out, "REHEARSALS.json"), "w"), indent=1)
    verdict = "REHEARSALS: " + ("ALL AS EXPECTED" if not bad else "FAILED")
    open(os.path.join(out, "SUMMARY.txt"), "w").write(f"{len(results)} rehearsal checks; {len(bad)} unexpected\n"
                                                      + verdict + "\n")
    print(verdict)
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
