#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01, mission sections 7 and 16: rehearse render.py and build_packet.py on
SYNTHETIC evidence roots before the freeze. Nothing is built or run; no Nexus executable, Cargo or rustc.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 16 and 19). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 11 and 14): this mission's layout (every control: gate, apply,
build, run, tranche-3 rules, restore, link, RESULT; C7: its informational F-3 run), its three derived statuses and its
previous packets (the continuation's among them).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (sections 16 and 20): this mission's layout (C24: gate,
admission of the preserved build, preserved worktree, pin, run, rules, reauthentication, link, RESULT; C3b and C3c: their
build admission; every other control as before), its four derived statuses and its previous packets (the Tranche-3 one
among them). Scenarios (replacing the list below):
  R1  all nine controls candidates (C3b admitted by the narrow warning rule, C3c clean)
                                                      -> READY_FOR_MUTATION_TRANCHE3_FINAL_ARCHITECT_REVIEW; STAGED
  R2  C24's preserved executable differs                -> C24_PRESERVED_ARTIFACT_BLOCKED; STAGED; nothing later attempted
  R3  no control attempted (the pre-execution gate failed) -> BLOCKED
  R4  C24, C3b candidates; C3c's build admission rejected (another warning) -> MUTATION_CONTROL_BLOCKED; STAGED
  R5  C7's kill run not a candidate while its informational run printed FAIL E11 -> MUTATION_CONTROL_BLOCKED; STAGED
  N1-N7 as below, at this mission's receipt directories (N7: a copy of a Tranche-3 packet file, C2's accepted build
      CHECKS.txt)

    rehearsals.py OUTDIR SCRATCH     both must not exist. SCRATCH (under dev-local/, never published) holds the
                                     synthetic evidence roots and their staged packets; OUTDIR (receipts/04b-rehearsals)
                                     receives REHEARSALS.txt, REHEARSALS.json and SUMMARY.txt (last line `REHEARSALS: ALL
                                     AS EXPECTED` or `REHEARSALS: FAILED`).

Each synthetic root holds byte copies of this mission's tools (tools/common.py naming that root as EV: the only
difference), original-tools/, inputs/, documents/, development/, the expectations and the supervision self-test, and
synthetic control directories in the layout of the real receipts, built from receipts/03-t3-controls' synthetic runs,
rules and links (a synthetic build receipt with an empty build.stdout, the sampler's constant record, a restoration and a
RESULT; C7: its synthetic informational F-3 run). Scenarios:
  R1  all ten controls candidates                     -> READY_FOR_MUTATION_TRANCHE3_ARCHITECT_REVIEW; packet STAGED
  R2  C2 candidate, C24's build NOT OK (a warning)     -> MUTATION_CONTROL_BLOCKED; packet STAGED; C3b..A5 not attempted
  R3  no control attempted (the pre-execution gate failed) -> BLOCKED
  R4  C7's kill run not a candidate while its informational run printed FAIL E11 (the informational result is never
      counted as the kill)                           -> MUTATION_CONTROL_BLOCKED, C7 stopped; packet STAGED
  N1  a receipt byte-equal to a previous-packet file at a path outside the enumerated records -> UNAUTHORIZED_DUPLICATE
  N2  an enumerated sampler path holding another previous-packet file's bytes            -> LITERAL_MISMATCH
  N3  an empty file at a path outside the enumerated zero-length artifacts                -> ZERO_LENGTH_UNLISTED
  N4  an empty build.stdout whose descriptor does not record it                           -> ZERO_DESCRIPTOR
  N5  a whitespace error in an authored document                                          -> whitespace check fails
  N6  a staged copy of a Candidate C source file (no mutant source is ever written here)    -> source-copy check fails
  N7  a staged copy of a continuation packet file (A3's accepted build CHECKS.txt)         -> UNAUTHORIZED_DUPLICATE
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

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
    t3c = os.path.join(K.EV, "receipts/03-t3-controls")
    order = {"R1": K.ORDER, "R2": K.ORDER[:1], "R3": [], "R4": K.ORDER[:3], "R5": K.ORDER[:5]}[scenario]

    def write(d, rel, text):
        os.makedirs(os.path.dirname(os.path.join(d, rel)), exist_ok=True)
        open(os.path.join(d, rel), "w").write(text)

    def run_and_rules(d, cid, pick):
        src = os.path.join(t3c, "A", sorted(x for x in os.listdir(os.path.join(t3c, "A"))
                                            if x.split("-")[1] == cid and pick in x)[0])
        shutil.copytree(os.path.join(src, "04-run"), os.path.join(d, "04-run"))
        shutil.copytree(os.path.join(src, "04a-rules"), os.path.join(d, "04a-rules"))
        open(os.path.join(d, "04-run/cgroup-samples.txt"), "w").write(SAMPLER)
        open(os.path.join(d, "04-run/RECEIPT.txt"), "w").write(f"SUPERVISED MUTANT RUN — control {cid}: {SYN}\n")
        return [f"run (type (a)): {open(os.path.join(d, '04-run/CLASSIFICATION.txt')).readline().strip()}",
                f"tranche-3 rules: {open(os.path.join(d, '04a-rules/RULES.txt')).readline().strip()}"]
    for cid in order:
        ctl = exp["controls"][cid]
        d = os.path.join(ev, ctl["dir"])
        os.makedirs(d)
        write(d, "00-GATE.txt", "GATE: OPEN\n" + SYN + "\n")
        if cid == "C24":
            # TRANCHE3-CONTINUE-02: C24's own chain (no apply, build or restore).
            art = scenario == "R2"
            write(d, "01-admission/ADMISSION.txt", "C24 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW "
                  "C24-SPECIFIC WARNING RULING)\n" + SYN + "\n")
            write(d, "02-worktree/AUTHENTICATE-BEFORE.txt", "C24 PRESERVED WORKTREE: AUTHENTICATED (EXACTLY C; LOCKED; NOT "
                  "MODIFIED BY THIS MISSION)\n" + SYN + "\n")
            write(d, "03-pin/PIN.txt", ("C24_PRESERVED_ARTIFACT_BLOCKED" if art else "C24 PRESERVED EXECUTABLE: PINNED")
                  + "\n" + SYN + "\n")
            facts = ["build admission: C24 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED (NARROW "
                     "C24-SPECIFIC WARNING RULING)"]
            if art:
                facts.append("STOP: C24_PRESERVED_ARTIFACT_BLOCKED (synthetic). No later control is executed.")
                res = "CONTROL RESULT: C24_PRESERVED_ARTIFACT_BLOCKED"
            else:
                json.dump({"path": "/synthetic", "sha256": "0" * 64, "size": 1, "synthetic": SYN},
                          open(os.path.join(d, "03-pin/PIN.json"), "w"))
                facts += run_and_rules(d, cid, "P-first-permitted-property")
                write(d, "05-reauth/REAUTH.txt", "C24 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; "
                      "EXECUTABLE AND TARGET UNCHANGED)\n" + SYN + "\n")
                shutil.copytree(os.path.join(t3c, "B", "C24-real-inputs", "06-link"), os.path.join(d, "06-link"))
                res = CAND_RESULT
            write(d, "RESULT.txt", "\n".join([res, f"control {cid}: {ctl['description']}",
                                              f"receipt type: {ctl['receipt_type']}"] + facts) + "\n")
            json.dump({"control": cid, "status": res, "facts": facts}, open(os.path.join(d, "CONTROL-RESULT.json"), "w"),
                      indent=1)
            continue
        os.makedirs(os.path.join(d, "03-build"))
        write(d, "02-apply/APPLY.txt", "APPLY: EXACTLY_C_PLUS_PATCH\n" + SYN + "\n")
        narrow = cid == "C3b"  # R1 and R4: C3b admitted by the narrow warning rule
        reject = scenario == "R4" and cid == "C3c"
        warn = narrow or reject
        stderr = (f"{SYN}\n" + ("warning: variant `Future` is never constructed\n" if narrow else "")
                  + ("warning: unused variable: `set`\n" if reject else "")
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
        facts = ["apply: APPLY: EXACTLY_C_PLUS_PATCH", f"build: {'BUILD: NOT OK' if warn else 'BUILD: PINNED'}"]
        pin_rel = "03-build/PIN.json"
        if cid in ("C3b", "C3c"):
            first = (f"{cid} BUILD ADMISSION: REJECTED — STOP" if reject else
                     f"{cid} BUILD ADMISSION: ADMISSIBLE — EXACT NARROW WARNING BUILD (ARCHITECT RULING, AMENDMENT-01)"
                     if narrow else
                     f"{cid} BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)")
            write(d, "03a-admission/ADMISSION.txt", first + "\n" + SYN + "\n")
            facts.append(f"build admission (C3b/C3c rule, AMENDMENT-01): {first}")
            pin_rel = "03a-admission/PIN.json" if narrow else pin_rel
        not_cand = scenario == "R5" and cid == "C7"
        if not reject:
            pick = "N-C7-kill-run-with-A4-OFF" if not_cand else "P-first-permitted-property"
            json.dump({"id": "03-build", "path": "/synthetic", "sha256": "0" * 64, "size": 1},
                      open(os.path.join(d, pin_rel), "w"))
            facts += run_and_rules(d, cid, pick)
            if cid == "C7":
                isrc = os.path.join(t3c, "E", sorted(x for x in os.listdir(os.path.join(t3c, "E")) if "P-FAIL-E11" in x)[0])
                shutil.copytree(os.path.join(isrc, "04b-info-F3"), os.path.join(d, "04b-info-F3"))
                open(os.path.join(d, "04b-info-F3/cgroup-samples.txt"), "w").write(SAMPLER)
                open(os.path.join(d, "04b-info-F3/RECEIPT.txt"), "w").write(
                    f"SUPERVISED MUTANT RUN — control C7: full_f3 informational ({SYN})\n")
            if not not_cand:
                shutil.copytree(os.path.join(t3c, "B", f"{cid}-real-inputs", "06-link"), os.path.join(d, "06-link"))
        write(d, "05-restore/RESTORE.txt", f"RESTORE: EXACTLY_C\ncontrol {cid}: {SYN}\n")
        res = BLOCKED_RESULT if (reject or not_cand) else CAND_RESULT
        if reject:
            facts.append("STOP: the mutant build is not admissible under its build rule (kills nothing; no rebuild). No "
                         "later control is executed.")
        if not_cand:
            facts.append("STOP: the mutant run is not admissible kill evidence of the named property or test. No later "
                         "control is executed.")
        write(d, "RESULT.txt", "\n".join([res, f"control {cid}: {ctl['description']}",
                                          f"receipt type: {ctl['receipt_type']}"] + facts) + "\n")
        json.dump({"control": cid, "status": res, "facts": facts}, open(os.path.join(d, "CONTROL-RESULT.json"), "w"),
                  indent=1)
    return ev


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
    for sc, want, want_packet in (("R1", "READY_FOR_MUTATION_TRANCHE3_FINAL_ARCHITECT_REVIEW", True),
                                  ("R2", "C24_PRESERVED_ARTIFACT_BLOCKED", True), ("R3", "BLOCKED", True),
                                  ("R4", "MUTATION_CONTROL_BLOCKED", True), ("R5", "MUTATION_CONTROL_BLOCKED", True)):
        ev = make_root(os.path.join(scratch, sc), exp, sc)
        os.makedirs(os.path.join(scratch, sc, "render"))
        rc, o = run_tool(ev, [os.path.join(ev, "tools/render.py"), os.path.join(scratch, sc, "render")])
        open(os.path.join(out, f"{sc}-render.console.txt"), "w").write(o)
        st = [l for l in o.splitlines() if l.startswith("STATUS: ")]
        record(f"{sc} render.py status", want, st[0] if st else o[-200:], rc == 0 and st and st[0].startswith(
            f"STATUS: {want};"))
        summ = open(os.path.join(scratch, sc, "render/TRANCHE-SUMMARY.md")).read()
        if sc == "R2":
            record("R2 summary: C24 stopped (preserved artifact); C3b..A5 not attempted; 0 candidates",
                   "stopped C24; not attempted C3b C3c C6 C7 A1 A2 C20 A5; 0 of 9",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- Authorized but not", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C24." in summ
                   and "- Authorized but not attempted: C3b, C3c, C6, C7, A1, A2, C20, A5." in summ and "**0 of 9**" in summ)
        if sc == "R4":
            record("R4 summary: C3c stopped at its build admission; C3b admitted by the narrow rule; 2 candidates",
                   "stopped C3c; 2 of 9; the admission lines present",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- New executor", "- build admission"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C3c." in summ
                   and "**2 of 9**" in summ and "C3c BUILD ADMISSION: REJECTED — STOP" in summ
                   and "C3b BUILD ADMISSION: ADMISSIBLE — EXACT NARROW WARNING BUILD" in summ)
        if sc == "R5":
            record("R5 summary: C7 stopped although its informational run printed FAIL E11 (never counted as the kill); "
                   "4 candidates", "stopped C7; 4 of 9; the informational line present",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- New executor", "- C7 informational"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C7." in summ
                   and "**4 of 9**" in summ and "- C7 informational run without A4 on F-3 (not the kill receipt): "
                   "INFORMATIONAL OUTCOME: FAIL E11" in summ)
        if sc == "R1":
            record("R1 summary: L1 complete-search lines for C20 and A5; C7's informational line; C24's admission and "
                   "reauthentication; C3b narrow and C3c clean admissions",
                   "2 L1 lines; 1 informational line; the C24, C3b and C3c admission lines",
                   f"{summ.count('- L1 complete search (AD-1-R4 Q): yes')} L1; "
                   f"{summ.count('- C7 informational run without A4 on F-3')} informational",
                   summ.count("- L1 complete search (AD-1-R4 Q): yes") == 2
                   and summ.count("- C7 informational run without A4 on F-3") == 1
                   and "C24 BUILD ADMISSION: ARCHITECT-ADMISSIBLE BUILD AUTHENTICATED" in summ
                   and "- restoration: C24 REAUTHENTICATION AFTER THE RUN: HOLDS" in summ
                   and "C3b BUILD ADMISSION: ADMISSIBLE — EXACT NARROW WARNING BUILD" in summ
                   and "C3c BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD" in summ)
        bad_words = [w for w in ("D2A-1 ACCEPTED", "ALL MUTATIONS COMPLETE", "PHASE THREE COMPLETE", "ACCEPTED COMPONENT",
                                 "INTEGRATED") if w in summ or w in open(os.path.join(scratch, sc, "render/INDEX.md")).read()]
        record(f"{sc} no prohibited wording in INDEX.md or TRANCHE-SUMMARY.md", "none", bad_words, not bad_words)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/build_packet.py"), os.path.join(scratch, sc, "packet")])
        open(os.path.join(out, f"{sc}-packet.console.txt"), "w").write(o)
        last = o.rstrip().splitlines()[-1]
        record(f"{sc} build_packet.py", "PACKET: STAGED — ALL CHECKS HOLD", last,
               (last == "PACKET: STAGED — ALL CHECKS HOLD") == want_packet and rc == (0 if want_packet else 1))
        if sc == "R1":
            m = open(os.path.join(scratch, sc, "packet/documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv")).read().splitlines()
            ctl_samplers = [l for l in m if l.startswith("CGROUP_CONSTANT\treceipts/") and not l.split("\t")[1].startswith(
                "receipts/01-")]
            record("R1 map: the eight build.stdout and the sampler records mapped, bound to their descriptors (C24: its "
                   "run only; C7: its informational run's sampler too)",
                   "8 ZERO_LENGTH build.stdout rows; 18 control CGROUP_CONSTANT rows (8 builds, 9 runs, C7's "
                   "informational run)",
                   f"{sum('/03-build/build.stdout' in l for l in m)} build.stdout, {len(ctl_samplers)} control samplers",
                   sum("/03-build/build.stdout" in l for l in m) == 8 and len(ctl_samplers) == 18
                   and sum(1 for l in ctl_samplers if "/04b-info-F3/" in l) == 1)
    prior = K.git("show", f"{K.T1}:{K.CONTINUE02_PACKET}/receipts/38-C9/04-run/CHECKS.txt")
    prior2 = K.git("show", f"{K.T1}:{K.EXEC01_PACKET}/receipts/01-supervision-selftest/1/out.txt")
    prior3 = K.git("show", f"{K.T3X}:{K.T3X_PACKET}/receipts/66-C2/03-build/CHECKS.txt")
    negs = [("N1", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/81-C24/04-run/EXTRA.txt"),
                                                               "wb").write(prior)),
            ("N2", "LITERAL_MISMATCH", lambda ev: open(os.path.join(ev, "receipts/85-C7/04b-info-F3/cgroup-samples.txt"),
                                                         "wb").write(prior2)),
            ("N3", "ZERO_LENGTH_UNLISTED", lambda ev: open(os.path.join(ev, "receipts/84-C6/04-run/EMPTY.txt"), "w")
             .write("")),
            ("N4", "ZERO_DESCRIPTOR", lambda ev: json.dump(dict(json.load(open(os.path.join(
                ev, "receipts/89-A5/03-build/descriptor.json"))), stdout_sha256="0" * 64), open(os.path.join(
                ev, "receipts/89-A5/03-build/descriptor.json"), "w"))),
            ("N5", "every file this mission authored is whitespace-clean", lambda ev: open(os.path.join(
                ev, "documents/NOTE.md"), "w").write("a synthetic note with a trailing space \n")),
            ("N6", "no source copy", lambda ev: open(os.path.join(ev, "receipts/88-C20/04-run/copy.txt"), "wb").write(
                K.blob(K.C, K.UNIT))),
            ("N7", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/82-C3b/03-build/COPY.txt"),
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
