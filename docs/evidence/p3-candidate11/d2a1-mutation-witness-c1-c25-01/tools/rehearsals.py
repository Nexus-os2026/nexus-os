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
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (section 5, steps 9 and 13): this mission's layout (every control:
gate, worktree, apply, build, run, tranche-3 rules, restore, link, RESULT; no admission, preserved build or informational
run), its four derived statuses (mission section 12) and its previous packets (the continuation's among them).
Scenarios (replacing both lists):
  R1  all four controls candidates                     -> KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; STAGED
  R2  the first control stopped: A1 printed P4, not P1  -> MUTATION_CONTROL_BLOCKED; STAGED; A2 C20 A5 not attempted
  R3  no control attempted (the pre-execution gate failed) -> STOPPED BEFORE EXECUTION
  R4  A1, A2 candidates; C20 INCOMPLETE at the state cap -> INCOMPLETE — ARCHITECT REVIEW REQUIRED; STAGED
  R5  A1, A2, C20 candidates; A5 FAIL L1 from a search prefix (expanded < discovered) -> MUTATION_CONTROL_BLOCKED
  N1-N7 the packet policy negatives at this mission's receipt directories (N7: a copy of a continuation packet file,
      C3b's accepted build CHECKS.txt)
  M1-M5 control.main itself, end to end, in synthetic roots (the real gate, classify_a, t3.rules, link, finish and RESULT
      labels; the worktree, apply, build, run and restore steps replaced by fakes that create, apply, build and run
      nothing; each control once, in order, stopping at the first unsuccessful one, exactly as the execution loop does):
      M1 all four candidates; M2 the stopped-first-control path (A1 FAIL P4: MUTATION_CONTROL_BLOCKED; A2's gate then
      closed; nothing later attempted); M3 A1 killed from outside: INCOMPLETE — ARCHITECT REVIEW REQUIRED; M4 C20
      INCOMPLETE at the state cap; M5 A5 L1 from a search prefix: MUTATION_CONTROL_BLOCKED. In every M root control.main
      refuses C7 and writes nothing. Then render.py's status, and for M1 and M2 the final gate (final_gate.py, unchanged
      code, in the synthetic root): FAILED with exactly the G6 pinned-executable checks failing (the synthetic pins name
      no file), every other check holding.

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (section 3, steps 6 and 7): this mission's layout (every control:
gate, worktree, apply, build, the type-(a) primary (04-run), the type-(c) mutant witness (04c-witness) only after an
admissible primary, restore, the type-(e3) healthy link (06a-healthy-witness), the type-(e1) link (06b-link), the G.3
record (07-g3), RESULT), its four derived statuses (mission section 10) and its previous packets (the tail's among them).
Scenarios (replacing every list above):
  R1  C1 and C25 candidates                            -> KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; STAGED
  R2  the first control stopped: C1's primary printed P1, not E3 (Architect ruling required)
                                                      -> MUTATION_CONTROL_BLOCKED; STAGED; C25 not attempted
  R3  no control attempted (the pre-execution gate failed) -> STOPPED BEFORE EXECUTION
  R4  C1 candidate; C25's mutant witness NOT OBSERVED   -> MUTATION_CONTROL_BLOCKED; STAGED
  R5  C1's primary INCOMPLETE at the state cap          -> INCOMPLETE — ARCHITECT REVIEW REQUIRED; STAGED
  N1-N7 the packet policy negatives at this mission's receipt directories (N2: the mutant witness's sampler path; N7: a
      copy of a tail packet file, A1's accepted build CHECKS.txt)
  M1-M5 control.main itself, end to end, in synthetic roots (the real gate, g3.classify_primary, g3.classify_witness,
      g3.link_healthy, link, g3.obligations, finish and RESULT labels; the worktree, apply, build, run and restore steps
      replaced by fakes that create, apply, build and run nothing; each control once, in order, stopping at the first
      unsuccessful one, exactly as the execution loop does): M1 both candidates; M2 the stopped-first-control path (C1's
      primary P1: MUTATION_CONTROL_BLOCKED; C25's gate then closed; nothing later attempted; no witness run); M3 C1's
      witness killed from outside: INCOMPLETE — ARCHITECT REVIEW REQUIRED; M4 C1's primary INCOMPLETE at the state cap:
      INCOMPLETE, no witness run; M5 C1 candidate, C25's witness an unrelated panic: MUTATION_CONTROL_BLOCKED. In every M
      root control.main refuses C7 and C4 and writes nothing for them. Then render.py's status, and for M1 and M2 the
      final gate (final_gate.py in the synthetic root): FAILED with exactly the G6 pinned-executable checks failing (the
      synthetic pins name no file), every other check holding.

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
INCOMPLETE_RESULT = "CONTROL RESULT: INCOMPLETE — ARCHITECT REVIEW REQUIRED"
SAMPLER = "sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n"
SYN = "SYNTHETIC (rehearsals.py): not the record of any build or run"
PRIMARY_OK_PICK, WITNESS_OK_PICK = "P-first-failure", "P-positive"


def record(case, expected, got, ok, detail=""):
    results.append({"case": case, "expected": expected, "got": got, "as_expected": bool(ok), "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def pick_dir(t3c, sec, cid, pick):
    return os.path.join(t3c, sec, sorted(x for x in os.listdir(os.path.join(t3c, sec))
                                         if x.split("-")[0] == cid and pick in x)[0])


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
    order = {"R1": K.ORDER, "R2": K.ORDER[:1], "R3": [], "R4": K.ORDER, "R5": K.ORDER[:1], "M": []}[scenario]
    # (control, step): the synthetic receipt this scenario's stopping control takes; every other step is a positive one.
    stop_pick = {"R2": ("C1", "primary", "N-wrong-first-property-P1"), "R4": ("C25", "witness", "N-NOT-OBSERVED"),
                 "R5": ("C1", "primary", "N-incomplete-search-INCOMPLETE-at-the-state-cap")}.get(scenario)

    def write(d, rel, text):
        os.makedirs(os.path.dirname(os.path.join(d, rel)), exist_ok=True)
        open(os.path.join(d, rel), "w").write(text)

    def run_copy(d, cid, sec, pick, step, title):
        src = pick_dir(t3c, sec, cid, pick)
        shutil.copytree(os.path.join(src, step), os.path.join(d, step))
        open(os.path.join(d, step, "cgroup-samples.txt"), "w").write(SAMPLER)
        open(os.path.join(d, step, "RECEIPT.txt"), "w").write(f"SUPERVISED MUTANT RUN — control {cid}: {title}: {SYN}\n")
        return open(os.path.join(d, step, "CLASSIFICATION.txt")).readline().strip()
    for cid in order:
        ctl = exp["controls"][cid]
        d = os.path.join(ev, ctl["dir"])
        os.makedirs(d)
        write(d, "00-GATE.txt", "GATE: OPEN\n" + SYN + "\n")
        os.makedirs(os.path.join(d, "03-build"))
        write(d, "02-apply/APPLY.txt", "APPLY: EXACTLY_C_PLUS_PATCH\n" + SYN + "\n")
        stderr = f"{SYN}\n    Finished `release` profile [optimized] target(s) in 1.00s\n"
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
            "BUILD: PINNED\nouter class (AD-1-R4 K.2): mutant build succeeded; one library-test executable pinned\n")
        shutil.copyfile(os.path.join(pick_dir(t3c, "W", cid, WITNESS_OK_PICK), "03-build/PIN.json"),
                        os.path.join(d, "03-build/PIN.json"))
        facts = ["apply: APPLY: EXACTLY_C_PLUS_PATCH", "build: BUILD: PINNED"]
        here = stop_pick if stop_pick is not None and stop_pick[0] == cid else None
        p = run_copy(d, cid, "A", here[2] if here and here[1] == "primary" else PRIMARY_OK_PICK, "04-run",
                     "the primary")
        facts.append(f"primary (type (a), G.3(a)): {p}")
        if not (here and here[1] == "primary"):
            w = run_copy(d, cid, "W", here[2] if here else WITNESS_OK_PICK, "04c-witness", "the mutant witness")
            facts.append(f"mutant witness (type (c), G.3(b)): {w}")
        write(d, "05-restore/RESTORE.txt", f"RESTORE: EXACTLY_C\ncontrol {cid}: {SYN}\n")
        res = CAND_RESULT
        if not here:
            shutil.copytree(os.path.join(t3c, "H", f"{cid}-real-inputs", "06a-healthy-witness"),
                            os.path.join(d, "06a-healthy-witness"))
            shutil.copytree(os.path.join(t3c, "B", f"{cid}-real-inputs", "06b-link"), os.path.join(d, "06b-link"))
            shutil.copytree(os.path.join(t3c, "G", f"{cid}-all-obligations", "07-g3"), os.path.join(d, "07-g3"))
        else:
            res = INCOMPLETE_RESULT if scenario == "R5" else BLOCKED_RESULT
            facts.append("STOP: a synthetic stop. No later control is executed.")
        write(d, "RESULT.txt", "\n".join([res, f"control {cid}: {ctl['description']}",
                                          f"receipt type: {ctl['receipt_type']}"] + facts) + "\n")
        json.dump({"control": cid, "status": res, "facts": facts}, open(os.path.join(d, "CONTROL-RESULT.json"), "w"),
                  indent=1)
    return ev


def run_tool(ev, args):
    r = subprocess.run([sys.executable, "-B", *args], capture_output=True, text=True,
                       env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    return r.returncode, r.stdout + r.stderr


DRIVE_PICKS = {"M1": {}, "M2": {("C1", "primary"): "N-wrong-first-property-P1"},
               "M3": {("C1", "witness"): "N-killed-from-outside-timeout"},
               "M4": {("C1", "primary"): "N-incomplete-search-INCOMPLETE-at-the-state-cap"},
               "M5": {("C25", "witness"): "N-unrelated-panic"}}
DRIVE_WANT = {"M1": ([CAND_RESULT] * 2, "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"),
              "M2": ([BLOCKED_RESULT], "MUTATION_CONTROL_BLOCKED"),
              "M3": ([INCOMPLETE_RESULT], "INCOMPLETE — ARCHITECT REVIEW REQUIRED"),
              "M4": ([INCOMPLETE_RESULT], "INCOMPLETE — ARCHITECT REVIEW REQUIRED"),
              "M5": ([CAND_RESULT, BLOCKED_RESULT], "MUTATION_CONTROL_BLOCKED")}


def synthetic_freeze(ev):
    """A freeze record of the synthetic root's own files (what record_freeze.py freezes), so that the unchanged
    supervise.freeze_check, control.gate and final_gate.py check the root exactly as they check the real one."""
    files = {}
    for sub in ("tools", "original-tools", "inputs"):
        for d, dirs, fs in os.walk(os.path.join(ev, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                files[os.path.relpath(os.path.join(d, f), ev)] = K.sha256_file(os.path.join(d, f))
    for rel in ["receipts/02-EXPECTATIONS.json"] + K.RECORDS:
        files[rel] = K.sha256_file(os.path.join(ev, rel))
    json.dump({"mission": K.MISSION, "synthetic": SYN, "files": files}, open(os.path.join(ev, "receipts/05-FREEZE.json"),
                                                                              "w"), indent=1)


def drive(scenario, t3c):
    """--drive (run as this root's own copy of rehearsals.py, so that every module is the root's): control.main for each
    control in order, stopping at the first non-zero result, with the worktree, apply, build, run and restore steps
    replaced by fakes. g3's classifiers, link_healthy, link, obligations, finish and the gate run unchanged on
    t3_controls' synthetic runs and the authenticated inputs."""
    import classify  # noqa: E402
    import control  # noqa: E402
    import supervise  # noqa: E402
    import worktrees  # noqa: E402
    fake = "SYNTHETIC (rehearsals.py --drive): a fake step; nothing was created, applied, built or run\n"
    picks = DRIVE_PICKS[scenario]

    def exe_for(cid):  # t3_controls.py's synthetic executable identity (its runs are pinned to it)
        sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (t3_controls.py; never built, never run)".encode())
        return {"path": f"/synthetic/nexus-os-p3-c11-mtw-{cid}/target/release/deps/nexus_governed_control-"
                        "0123456789abcdef", "sha256": sha, "size": 4373}
    worktrees.create = lambda path, role: ({"path": path, "fake": fake}, True)
    worktrees.authenticate = lambda path, role: ({"path": path, "fake": fake, "checks": []}, True)

    def apply_patch(wt, ctl, outdir):
        os.makedirs(outdir)
        open(os.path.join(outdir, "APPLY.txt"), "w").write(control.APPLY_OK + "\n" + fake)
        return True, True

    def build(bdir, title, wt, source_ok, prior, extra, manifest, gate=True):
        for p, want in prior:
            assert supervise.first_line(os.path.join(K.EV, p)) == want, (p, want)
        os.makedirs(bdir)
        json.dump({"fake": fake, "control": extra["control"]}, open(os.path.join(bdir, "descriptor.json"), "w"))

    def classify_build_success(bdir, kind, sv, mb, expect_target=None):
        cid = json.load(open(os.path.join(bdir, "descriptor.json")))["control"]
        open(os.path.join(bdir, "CLASSIFICATION.txt"), "w").write("BUILD: PINNED\nouter class: " + fake)
        json.dump(exe_for(cid), open(os.path.join(bdir, "PIN.json"), "w"))
        return 0

    def invoke(outdir, title, exe, sha, size, cwd, wt, test, ignored, argv_tail, limit, source_ok, kind, prior, extra,
               gate=True, env=None):
        for p, want in prior:
            assert supervise.first_line(os.path.join(K.EV, p)) == want, (p, want)
        cid = extra["control"]
        assert (exe, sha, size) == tuple(exe_for(cid).values()), (exe, sha, size)
        role = "witness" if kind == "mutant-witness" else "primary"
        pick = picks.get((cid, role), WITNESS_OK_PICK if role == "witness" else PRIMARY_OK_PICK)
        src = os.path.join(pick_dir(t3c, "W" if role == "witness" else "A", cid, pick),
                           "04c-witness" if role == "witness" else "04-run")
        os.makedirs(outdir)
        for f in ("raw-output.txt", "descriptor.json"):
            shutil.copyfile(os.path.join(src, f), os.path.join(outdir, f))
        print(f"FAKE INVOKE {cid} {kind}: {test} (prior {[p for p, _ in prior]}); synthetic run "
              f"{os.path.basename(os.path.dirname(src))}")
        return 0

    def restore(wt, ctl, outdir, applied):
        os.makedirs(outdir)
        open(os.path.join(outdir, "RESTORE.txt"), "w").write(control.RESTORE_OK + "\n" + fake)
        return True
    control.apply_patch, control.restore = apply_patch, restore
    supervise.build, supervise.invoke = build, invoke
    classify.classify_build_success = classify_build_success
    for x in ("C7", "C4"):
        try:
            control.main(x)
            print(f"{x} REFUSAL: NOT REFUSED")
        except SystemExit as e:
            print(f"{x} REFUSAL: {e}")
    exp = supervise.expectations()
    for cid in K.ORDER:
        rc = control.main(cid)
        print(f"== {cid}: rc {rc}; {supervise.first_line(os.path.join(K.EV, exp['controls'][cid]['dir'], 'RESULT.txt'))}")
        if rc:
            nxt = K.ORDER[K.ORDER.index(cid) + 1:]
            if nxt:
                problems, _ = control.gate(nxt[0], exp)
                print(f"GATE {nxt[0]} AFTER THE STOP: {'CLOSED' if problems else 'OPEN'}: {'; '.join(problems)[:200]}")
            open(os.path.join(K.EV, "receipts/80-STOP-RECORD.md"), "w").write(
                f"# Stop record — synthetic ({scenario})\n\n{SYN}\n")
            break
    return 0


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    print(f"REHEARSALS — {K.MISSION} (synthetic evidence roots under {scratch}; nothing is built or run)")
    W1, W25 = exp["controls"]["C1"]["g3"]["witness"]["positive"], exp["controls"]["C25"]["g3"]["witness"]["positive"]
    for sc, want, want_packet in (("R1", "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING", True),
                                  ("R2", "MUTATION_CONTROL_BLOCKED", True), ("R3", "STOPPED BEFORE EXECUTION", True),
                                  ("R4", "MUTATION_CONTROL_BLOCKED", True),
                                  ("R5", "INCOMPLETE — ARCHITECT REVIEW REQUIRED", True)):
        ev = make_root(os.path.join(scratch, sc), exp, sc)
        os.makedirs(os.path.join(scratch, sc, "render"))
        rc, o = run_tool(ev, [os.path.join(ev, "tools/render.py"), os.path.join(scratch, sc, "render")])
        open(os.path.join(out, f"{sc}-render.console.txt"), "w").write(o)
        st = [l for l in o.splitlines() if l.startswith("STATUS: ")]
        record(f"{sc} render.py status", want, st[0] if st else o[-200:], rc == 0 and st and st[0].startswith(
            f"STATUS: {want};"))
        summ = open(os.path.join(scratch, sc, "render/TRANCHE-SUMMARY.md")).read()
        if sc == "R1":
            record("R1 summary: 2 candidates; 31 accepted + 2 pending; each primary's first property as printed and "
                   "each witness's exact positive line; the (e) recordings proposed, never made; no admission, "
                   "informational, reauthentication or L1 line",
                   "2 of 2; the campaign lines; E3, E4; both positive lines; 2 proposals",
                   f"candidates {'**2 of 2**' in summ}; proposals {summ.count('PROPOSED FOR ARCHITECT REVIEW')}",
                   "**2 of 2**" in summ and "31 accepted + 2 pending Architect review" in summ and "**31 of 35**" in summ
                   and "- primary first printed property (recorded as printed; not counted as P1): E3" in summ
                   and "- primary first printed property (recorded as printed; not counted as P7): E4" in summ
                   and f"`{W1}`" in summ and f"`{W25}`" in summ
                   and summ.count("(e) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance)") == 2
                   and "- healthy witness (type (e3), linked): HEALTHY WITNESS LINK: VERIFIED" in summ
                   and "- build admission" not in summ and "informational run" not in summ and "REAUTH" not in summ
                   and "L1 complete search" not in summ and "C4" in summ and "C7" in summ)
        if sc == "R2":
            record("R2 summary: the first control stopped (C1's primary printed P1: Architect ruling required); C25 not "
                   "attempted; no witness for C1; 0 candidates", "stopped C1; not attempted C25; 0 of 2; witness not run",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- Authorized but not", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C1." in summ
                   and "- Authorized but not attempted: C25." in summ and "**0 of 2**" in summ
                   and "- primary first printed property (recorded as printed; not counted as P1): P1" in summ
                   and "ARCHITECT RULING REQUIRED" in summ and "- mutant witness classification: not run" in summ)
        if sc == "R4":
            record("R4 summary: C25 stopped (its witness NOT OBSERVED); 1 candidate", "stopped C25; 1 of 2; NOT OBSERVED",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C25." in summ
                   and "**1 of 2**" in summ and "NOT OBSERVED" in summ)
        if sc == "R5":
            record("R5 summary: C1 stopped INCOMPLETE at the state cap (kills nothing; no witness run); 0 candidates",
                   "stopped C1; 0 of 2; INCOMPLETE", [l for l in summ.splitlines() if l.startswith(("- Stopped",
                                                                                                  "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C1." in summ
                   and "**0 of 2**" in summ and "INCOMPLETE — ARCHITECT REVIEW REQUIRED" in summ
                   and "- mutant witness classification: not run" in summ)
        idx = open(os.path.join(scratch, sc, "render/INDEX.md")).read()
        bad_words = [w for w in ("D2A-1 ACCEPTED", "ALL MUTATIONS COMPLETE", "PHASE THREE ACCEPTED", "PHASE THREE COMPLETE",
                                 "INTEGRATED", "33/35", "33 OF 35", "33 of 35", "C1 ACCEPTED", "C25 ACCEPTED",
                                 "ACCEPTED MUTATION CONTROL KILL") if w in summ or w in idx]
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
            record("R1 map: the two build.stdout and the six sampler records (two builds, two primaries, two witnesses) "
                   "mapped, bound to their descriptors", "2 ZERO_LENGTH build.stdout rows; 6 control CGROUP_CONSTANT rows "
                   "(2 of them 04c-witness)",
                   f"{sum('/03-build/build.stdout' in l for l in m)} build.stdout, {len(ctl_samplers)} control samplers, "
                   f"{sum('/04c-witness/' in l for l in ctl_samplers)} witness",
                   sum("/03-build/build.stdout" in l for l in m) == 2 and len(ctl_samplers) == 6
                   and sum("/04c-witness/cgroup-samples.txt" in l for l in ctl_samplers) == 2)
    prior = K.git("show", f"{K.T1}:{K.CONTINUE02_PACKET}/receipts/38-C9/04-run/CHECKS.txt")
    prior2 = K.git("show", f"{K.T1}:{K.EXEC01_PACKET}/receipts/01-supervision-selftest/1/out.txt")
    prior3 = K.git("show", f"{K.T3T}:{K.T3T_PACKET}/receipts/81-A1/03-build/CHECKS.txt")
    negs = [("N1", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/81-C1/04-run/EXTRA.txt"),
                                                               "wb").write(prior)),
            ("N2", "LITERAL_MISMATCH", lambda ev: open(os.path.join(ev, "receipts/82-C25/04c-witness/cgroup-samples.txt"),
                                                         "wb").write(prior2)),
            ("N3", "ZERO_LENGTH_UNLISTED", lambda ev: open(os.path.join(ev, "receipts/82-C25/04-run/EMPTY.txt"), "w")
             .write("")),
            ("N4", "ZERO_DESCRIPTOR", lambda ev: json.dump(dict(json.load(open(os.path.join(
                ev, "receipts/82-C25/03-build/descriptor.json"))), stdout_sha256="0" * 64), open(os.path.join(
                ev, "receipts/82-C25/03-build/descriptor.json"), "w"))),
            ("N5", "every file this mission authored is whitespace-clean", lambda ev: open(os.path.join(
                ev, "documents/NOTE.md"), "w").write("a synthetic note with a trailing space \n")),
            ("N6", "no source copy", lambda ev: open(os.path.join(ev, "receipts/81-C1/04c-witness/copy.txt"), "wb").write(
                K.blob(K.C, K.UNIT))),
            ("N7", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/82-C25/03-build/COPY.txt"),
                                                               "wb").write(prior3))]
    for n, want, act in negs:
        ev = make_root(os.path.join(scratch, n), exp, "R1")
        act(ev)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/build_packet.py"), os.path.join(scratch, n, "packet")])
        open(os.path.join(out, f"{n}-packet.console.txt"), "w").write(o)
        last = o.rstrip().splitlines()[-1]
        record(f"{n} build_packet.py rejects ({want})", f"PACKET: FAILED, naming {want}", last[:160],
               rc == 1 and last.startswith("PACKET: FAILED") and want in o)
    t3c = os.path.join(K.EV, "receipts/03-t3-controls")
    for sc in ("M1", "M2", "M3", "M4", "M5"):
        ev = make_root(os.path.join(scratch, sc), exp, "M")
        synthetic_freeze(ev)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/rehearsals.py"), "--drive", sc, t3c])
        open(os.path.join(out, f"{sc}-control-main.console.txt"), "w").write(o)
        want_results, want_status = DRIVE_WANT[sc]
        got = [supervise_first(os.path.join(ev, exp["controls"][c]["dir"], "RESULT.txt")) for c in K.ORDER]
        got = [g for g in got if g is not None]
        record(f"{sc} control.main end to end: RESULT labels in order, stopping at the first unsuccessful control; C7 "
               "and C4 refused by name, nothing written for them", want_results, got, rc == 0 and got == want_results
               and "C7 REFUSAL: REFUSED: C7 is MUTATION_CONTROL_BLOCKED" in o
               and "C4 REFUSAL: REFUSED: C4 is reserved" in o
               and not [x for x in os.listdir(os.path.join(ev, "receipts")) if x.endswith(("-C7", "-C4"))])
        c1 = os.path.join(ev, exp["controls"]["C1"]["dir"])
        if sc in ("M2", "M4"):
            record(f"{sc} the stopped-first-control path: C1 stopped at its primary (no witness run, no link), C25's gate "
                   "then closed, no later control's receipt directory",
                   "GATE C25 AFTER THE STOP: CLOSED; only 81-C1 exists; no 04c-witness, 06a, 06b or 07",
                   [l for l in o.splitlines() if l.startswith(("GATE C25", "== "))],
                   "GATE C25 AFTER THE STOP: CLOSED: receipts/81-C1/RESULT.txt is not" in o
                   and sorted(x for x in os.listdir(os.path.join(ev, "receipts")) if x[:2] in ("81", "82")) == ["81-C1"]
                   and not [x for x in ("04c-witness", "06a-healthy-witness", "06b-link", "07-g3")
                            if os.path.exists(os.path.join(c1, x))])
        if sc == "M3":
            record("M3 C1's witness killed from outside: INCOMPLETE; restored; nothing linked; C25's gate closed",
                   "04c-witness INCOMPLETE; RESTORE: EXACTLY_C; no 06a, 06b or 07; GATE C25 CLOSED",
                   [l for l in o.splitlines() if l.startswith(("GATE C25", "== "))],
                   supervise_first(os.path.join(c1, "05-restore/RESTORE.txt")) == "RESTORE: EXACTLY_C"
                   and not [x for x in ("06a-healthy-witness", "06b-link", "07-g3") if os.path.exists(os.path.join(c1, x))]
                   and "GATE C25 AFTER THE STOP: CLOSED" in o)
        if sc == "M1":
            import g3  # noqa: E402
            want_g = [g3.PRIMARY_OK, g3.WITNESS_OK, g3.HEALTHY_OK, "LINK: VERIFIED", g3.G3_OK] * len(K.ORDER)
            g = [supervise_first(os.path.join(ev, exp["controls"][c]["dir"], f)) for c in K.ORDER
                 for f in ("04-run/CLASSIFICATION.txt", "04c-witness/CLASSIFICATION.txt", "06a-healthy-witness/HEALTHY.txt",
                           "06b-link/LINK.txt", "07-g3/G3.txt")]
            record("M1 every G.3 step of both controls, run by control.main itself: primary, witness, healthy link, "
                   "unchanged-core link, G.3 record", "each exactly its success label", [x[:40] for x in g if x],
                   g == want_g)
        os.makedirs(os.path.join(scratch, sc, "render"))
        rc, o = run_tool(ev, [os.path.join(ev, "tools/render.py"), os.path.join(scratch, sc, "render")])
        open(os.path.join(out, f"{sc}-render.console.txt"), "w").write(o)
        st = [l for l in o.splitlines() if l.startswith("STATUS: ")]
        record(f"{sc} render.py status over control.main's own receipts", want_status, st[0] if st else o[-200:],
               rc == 0 and st and st[0].startswith(f"STATUS: {want_status};"))
        if sc in ("M1", "M2"):
            rc, o = run_tool(ev, [os.path.join(ev, "tools/final_gate.py")])
            open(os.path.join(out, f"{sc}-final-gate.console.txt"), "w").write(o)
            fails = [l for l in o.splitlines() if l.startswith("FAIL ")]
            pinned = len(want_results)  # every attempted control's (fake) build pinned a synthetic executable
            record(f"{sc} final-gate rehearsal (final_gate.py in the synthetic root): FAILED with exactly the G6 checks "
                   "of the synthetic pinned executables failing (they name no file); every other check holds",
                   f"{pinned} failing check(s), each 'G6 <control>: pinned mutant executable preserved'",
                   f"{len(fails)} failing: {[f[:60] for f in fails]}; last {o.rstrip().splitlines()[-1][:80]!r}",
                   rc == 1 and len(fails) == pinned
                   and all(f.startswith("FAIL G6 ") and "pinned mutant executable preserved" in f for f in fails)
                   and o.rstrip().splitlines()[-1].startswith("FINAL GATE: FAILED"))
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


def supervise_first(path):
    return open(path).readline().rstrip("\n") if os.path.exists(path) else None


if __name__ == "__main__":
    if sys.argv[1] == "--drive":
        sys.exit(drive(sys.argv[2], sys.argv[3]))
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))
