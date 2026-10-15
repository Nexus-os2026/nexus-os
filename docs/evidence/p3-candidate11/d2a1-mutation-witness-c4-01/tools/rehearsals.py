#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01, mission sections 7 and 16: rehearse render.py and build_packet.py on
SYNTHETIC evidence roots before the freeze. Nothing is built or run; no Nexus executable, Cargo or rustc.
Revised by every later mission for its own layout (TRANCHE2-CONTINUE-02, TRANCHE3-EXEC-01, TRANCHE3-CONTINUE-02,
TRANCHE3-TAIL-01, WITNESS-C1-C25-01); their scenario lists are replaced here.

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01 (section 5, step 10): this mission's layout (C4: gate, worktree,
apply, build, the type-(a) primary (04-run), the type-(c) mutant witness (04c-witness) only after an admissible primary,
restore, the post-restoration authentication (05b-restore-auth), the type-(e3) healthy link (06a-healthy-witness), the
type-(e1) link (06b-link), the G.5 record (07-g5), RESULT), its four derived statuses (mission section 15) and its
previous packets (the C1/C25 packet among them). record, pick_dir, run_tool, synthetic_freeze and supervise_first are
unchanged. Scenarios:
  R1  C4 candidate (first property P4)                 -> KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; STAGED
  R2  C4's primary printed E3 (neither P4 nor P8: Architect ruling) -> MUTATION_CONTROL_BLOCKED; STAGED; no witness
  R3  C4 not attempted (the pre-execution gate failed)  -> STOPPED BEFORE EXECUTION
  R4  C4's mutant witness shows P4 only (P8 missing)    -> MUTATION_CONTROL_BLOCKED; STAGED
  R5  C4's primary INCOMPLETE at the state cap          -> INCOMPLETE — ARCHITECT REVIEW REQUIRED; STAGED; no witness
  R6  C4's mutant witness breaches G.6 (a choice consumed beyond the offered stream) -> MUTATION_CONTROL_BLOCKED; STAGED
  N1-N7 the packet policy negatives at C4's receipt directory (N2: the mutant witness's sampler path; N7: a copy of a
      C1/C25 packet file, C1's accepted build CHECKS.txt)
  M1-M6 control.main itself, end to end, in synthetic roots (the real gate, g5.classify_primary, g5.classify_witness,
      g5.link_healthy, link, g5.obligations, finish and RESULT labels; the worktree, apply, build, run, restore and
      restore-auth steps replaced by fakes that create, apply, build and run nothing): M1 C4 candidate (P4 first); M2 the
      stopped-control path (the primary E3: MUTATION_CONTROL_BLOCKED; no witness, no link); M3 the witness killed from
      outside: INCOMPLETE — ARCHITECT REVIEW REQUIRED; M4 the primary INCOMPLETE at the state cap: INCOMPLETE, no
      witness; M5 the witness breaching G.6: MUTATION_CONTROL_BLOCKED; M6 C4 candidate (P8 first). In every M root
      control.main refuses C7, C1 and C25 and writes nothing for them. Then render.py's status, and for M1 and M2 the
      final gate (final_gate.py in the synthetic root): FAILED with exactly the G6 pinned-executable check failing (the
      synthetic pin names no file), every other check holding.

    rehearsals.py OUTDIR SCRATCH     both must not exist. SCRATCH (under dev-local/, never published) holds the
                                     synthetic evidence roots and their staged packets; OUTDIR (receipts/04b-rehearsals)
                                     receives REHEARSALS.txt, REHEARSALS.json and SUMMARY.txt (last line `REHEARSALS: ALL
                                     AS EXPECTED` or `REHEARSALS: FAILED`).
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
PRIMARY_OK_PICK, WITNESS_OK_PICK = "P-first-failure-P4", "P-P4-and-P8"
G6_PICK = "N-G.6-implicit-Oracle-default"


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
    order = {"R1": K.ORDER, "R2": K.ORDER, "R3": [], "R4": K.ORDER, "R5": K.ORDER, "R6": K.ORDER, "M": []}[scenario]
    # (control, step, pick): the synthetic receipt this scenario's stopping step takes; every other step is a positive one.
    stop_pick = {"R2": ("C4", "primary", "N-wrong-first-property-E3"), "R4": ("C4", "witness", "N-P8-missing"),
                 "R5": ("C4", "primary", "N-incomplete-search-INCOMPLETE-at-the-state-cap"),
                 "R6": ("C4", "witness", G6_PICK)}.get(scenario)

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
        facts.append(f"primary (type (a), G.5(A)): {p}")
        if not (here and here[1] == "primary"):
            w = run_copy(d, cid, "W", here[2] if here else WITNESS_OK_PICK, "04c-witness", "the mutant witness")
            facts.append(f"mutant witness (type (c), G.5(B), G.6): {w}")
        write(d, "05-restore/RESTORE.txt", f"RESTORE: EXACTLY_C\ncontrol {cid}: {SYN}\n")
        write(d, "05b-restore-auth/RESTORE-AUTH.txt", f"RESTORE AUTH: OK\ncontrol {cid}: {SYN}\n")
        res = CAND_RESULT
        if not here:
            shutil.copytree(os.path.join(t3c, "H", f"{cid}-real-inputs", "06a-healthy-witness"),
                            os.path.join(d, "06a-healthy-witness"))
            shutil.copytree(os.path.join(t3c, "B", f"{cid}-real-inputs", "06b-link"), os.path.join(d, "06b-link"))
            shutil.copytree(os.path.join(t3c, "G", f"{cid}-all-obligations-first-P4", "07-g5"), os.path.join(d, "07-g5"))
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


DRIVE_PICKS = {"M1": {}, "M2": {("C4", "primary"): "N-wrong-first-property-E3"},
               "M3": {("C4", "witness"): "N-killed-from-outside-timeout"},
               "M4": {("C4", "primary"): "N-incomplete-search-INCOMPLETE-at-the-state-cap"},
               "M5": {("C4", "witness"): G6_PICK},
               "M6": {("C4", "primary"): "P-first-failure-P8"}}
DRIVE_WANT = {"M1": ([CAND_RESULT], "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"),
              "M2": ([BLOCKED_RESULT], "MUTATION_CONTROL_BLOCKED"),
              "M3": ([INCOMPLETE_RESULT], "INCOMPLETE — ARCHITECT REVIEW REQUIRED"),
              "M4": ([INCOMPLETE_RESULT], "INCOMPLETE — ARCHITECT REVIEW REQUIRED"),
              "M5": ([BLOCKED_RESULT], "MUTATION_CONTROL_BLOCKED"),
              "M6": ([CAND_RESULT], "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING")}


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
    """--drive (run as this root's own copy of rehearsals.py, so that every module is the root's): control.main for C4,
    with the worktree, apply, build, run, restore and restore-auth steps replaced by fakes. g5's classifiers,
    link_healthy, link, obligations, finish and the gate run unchanged on t3_controls' synthetic runs and the
    authenticated inputs."""
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
        json.dump({"fake": fake, "patch_sha256": ctl["patch"]["sha256"], "catalog_patch_sha256": ctl["patch"]["sha256"],
                   "verify_mutant": {"verdict": "EXACTLY_C_PLUS_PATCH", "mutant_file": {
                       "git_blob": ctl["mutant_blob"], "sha256": ctl["mutant_sha256"]}}},
                  open(os.path.join(outdir, "APPLY.json"), "w"), indent=1)
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
        json.dump({"fake": fake, "applied_before_restoration": True,
                   "before": {"git_blob": ctl["mutant_blob"], "sha256": ctl["mutant_sha256"]},
                   "after": {"git_blob": ctl["baseline_blob"], "sha256": ctl["baseline_sha256"]}},
                  open(os.path.join(outdir, "RESTORE.json"), "w"), indent=1)
        return True

    def restore_auth(wt, ctl, outdir, pin):
        os.makedirs(outdir)
        open(os.path.join(outdir, "RESTORE-AUTH.txt"), "w").write("RESTORE AUTH: OK\n" + fake)
        return True
    control.apply_patch, control.restore, control.restore_auth = apply_patch, restore, restore_auth
    supervise.build, supervise.invoke = build, invoke
    classify.classify_build_success = classify_build_success
    for x in ("C7", "C1", "C25"):
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
    P4, P8 = [s["positive"] for s in exp["controls"]["C4"]["g5"]["witness"]["scenarios"]]
    for sc, want, want_packet in (("R1", "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING", True),
                                  ("R2", "MUTATION_CONTROL_BLOCKED", True), ("R3", "STOPPED BEFORE EXECUTION", True),
                                  ("R4", "MUTATION_CONTROL_BLOCKED", True),
                                  ("R5", "INCOMPLETE — ARCHITECT REVIEW REQUIRED", True),
                                  ("R6", "MUTATION_CONTROL_BLOCKED", True)):
        ev = make_root(os.path.join(scratch, sc), exp, sc)
        os.makedirs(os.path.join(scratch, sc, "render"))
        rc, o = run_tool(ev, [os.path.join(ev, "tools/render.py"), os.path.join(scratch, sc, "render")])
        open(os.path.join(out, f"{sc}-render.console.txt"), "w").write(o)
        st = [l for l in o.splitlines() if l.startswith("STATUS: ")]
        record(f"{sc} render.py status", want, st[0] if st else o[-200:], rc == 0 and st and st[0].startswith(
            f"STATUS: {want};"))
        summ = open(os.path.join(scratch, sc, "render/TRANCHE-SUMMARY.md")).read()
        if sc == "R1":
            record("R1 summary: C4 candidate; 33 accepted + 1 pending; the primary's first property as printed (P4, "
                   "never relabelled); both exact positive lines; the G.6 changed-append record; (E) pending and (F) "
                   "proposed, never made; no admission, informational, reauthentication or L1 line",
                   "1 of 1; the campaign lines; P4; both positive lines; G.6; one proposal",
                   f"candidates {'**1 of 1**' in summ}; proposals {summ.count('PROPOSED FOR ARCHITECT REVIEW')}",
                   "**1 of 1**" in summ and "33 accepted + 1 pending Architect review" in summ and "**33 of 35**" in summ
                   and "- primary first printed property (recorded as printed; never relabelled; it alone establishes "
                       "neither P4 nor P8): P4" in summ
                   and f"`{P4}`" in summ and f"`{P8}`" in summ
                   and summ.count("(F) recording, PROPOSED FOR ARCHITECT REVIEW (not an acceptance)") == 1
                   and "(E) the Sole Architect's independent review: PENDING" in summ
                   and "- G.6 at each changed append (from the raw output): scenario P4 step 8/8 Ack(0): offered "
                       "[0, 0, 0, 1, 0, 0], consumed [(0, 4), (0, 4), (0, 4), (1, 3), (0, 2), (0, 3)], unused []" in summ
                   and "scenario P8 step 12/12 Ack(0): offered [0, 0, 0, 0], consumed [(0, 4), (0, 4), (0, 4), (0, 3)], "
                       "unused []" in summ
                   and "- healthy witness (type (e3), linked): HEALTHY WITNESS LINK: VERIFIED" in summ
                   and "post-restoration authentication: RESTORE AUTH: OK" in summ
                   and "- build admission" not in summ and "informational run" not in summ and "REAUTH" not in summ
                   and "L1 complete search" not in summ and "C7" in summ)
        if sc == "R2":
            record("R2 summary: C4 stopped (the primary printed E3: Architect ruling); no witness; 0 candidates",
                   "stopped C4; 0 of 1; E3; ruling; witness not run",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C4." in summ
                   and "**0 of 1**" in summ and "it alone establishes neither P4 nor P8): E3" in summ
                   and "STOP FOR ARCHITECT RULING" in summ and "- mutant witness classification: not run" in summ)
        if sc in ("R4", "R6"):
            record(f"{sc} summary: C4 stopped at its mutant witness ({'only P4 shown' if sc == 'R4' else 'G.6 breached'});"
                   " 0 candidates", "stopped C4; 0 of 1",
                   [l for l in summ.splitlines() if l.startswith(("- Stopped", "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C4." in summ
                   and "**0 of 1**" in summ and ("only P4 shown" if sc == "R4" else "G.6 breached") in summ)
        if sc == "R5":
            record("R5 summary: C4 stopped INCOMPLETE at the state cap (kills nothing; no witness run); 0 candidates",
                   "stopped C4; 0 of 1; INCOMPLETE", [l for l in summ.splitlines() if l.startswith(("- Stopped",
                                                                                                  "- New executor"))],
                   "- Stopped (first unsuccessful control; nothing later was attempted): C4." in summ
                   and "**0 of 1**" in summ and "INCOMPLETE — ARCHITECT REVIEW REQUIRED" in summ
                   and "- mutant witness classification: not run" in summ)
        idx = open(os.path.join(scratch, sc, "render/INDEX.md")).read()
        bad_words = [w for w in ("D2A-1 ACCEPTED", "ALL MUTATIONS COMPLETE", "PHASE THREE ACCEPTED", "PHASE THREE COMPLETE",
                                 "INTEGRATED", "34/35", "34 OF 35", "34 of 35", "35/35", "35 of 35", "C4 ACCEPTED",
                                 "C4: ACCEPTED", "ACCEPTED MUTATION CONTROL KILL") if w in summ or w in idx]
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
            record("R1 map: the build.stdout and the three sampler records (build, primary, witness) mapped, bound to "
                   "their descriptors", "1 ZERO_LENGTH build.stdout row; 3 control CGROUP_CONSTANT rows (1 of them "
                   "04c-witness)",
                   f"{sum(l.split(chr(9))[1].endswith('/03-build/build.stdout') for l in m[1:])} build.stdout, "
                   f"{len(ctl_samplers)} control samplers, "
                   f"{sum(l.split(chr(9))[1].endswith('/04c-witness/cgroup-samples.txt') for l in ctl_samplers)} witness "
                   "(by each row's path column; a row's previous-packet column may name earlier witness samplers)",
                   sum(l.split("\t")[1].endswith("/03-build/build.stdout") for l in m[1:]) == 1 and len(ctl_samplers) == 3
                   and sum(l.split("\t")[1] == "receipts/81-C4/04c-witness/cgroup-samples.txt" for l in ctl_samplers) == 1
                   and sorted(l.split("\t")[1] for l in ctl_samplers) == [
                       f"receipts/81-C4/{s}/cgroup-samples.txt" for s in ("03-build", "04-run", "04c-witness")])
    prior = K.git("show", f"{K.T1}:{K.CONTINUE02_PACKET}/receipts/38-C9/04-run/CHECKS.txt")
    prior2 = K.git("show", f"{K.T1}:{K.EXEC01_PACKET}/receipts/01-supervision-selftest/1/out.txt")
    prior3 = K.git("show", f"{K.W1}:{K.W1_PACKET}/receipts/81-C1/03-build/CHECKS.txt")
    negs = [("N1", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/81-C4/04-run/EXTRA.txt"),
                                                               "wb").write(prior)),
            ("N2", "LITERAL_MISMATCH", lambda ev: open(os.path.join(ev, "receipts/81-C4/04c-witness/cgroup-samples.txt"),
                                                         "wb").write(prior2)),
            ("N3", "ZERO_LENGTH_UNLISTED", lambda ev: open(os.path.join(ev, "receipts/81-C4/04-run/EMPTY.txt"), "w")
             .write("")),
            ("N4", "ZERO_DESCRIPTOR", lambda ev: json.dump(dict(json.load(open(os.path.join(
                ev, "receipts/81-C4/03-build/descriptor.json"))), stdout_sha256="0" * 64), open(os.path.join(
                ev, "receipts/81-C4/03-build/descriptor.json"), "w"))),
            ("N5", "every file this mission authored is whitespace-clean", lambda ev: open(os.path.join(
                ev, "documents/NOTE.md"), "w").write("a synthetic note with a trailing space \n")),
            ("N6", "no source copy", lambda ev: open(os.path.join(ev, "receipts/81-C4/04c-witness/copy.txt"), "wb").write(
                K.blob(K.C, K.UNIT))),
            ("N7", "UNAUTHORIZED_DUPLICATE", lambda ev: open(os.path.join(ev, "receipts/81-C4/03-build/COPY.txt"),
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
    for sc in ("M1", "M2", "M3", "M4", "M5", "M6"):
        ev = make_root(os.path.join(scratch, sc), exp, "M")
        synthetic_freeze(ev)
        rc, o = run_tool(ev, [os.path.join(ev, "tools/rehearsals.py"), "--drive", sc, t3c])
        open(os.path.join(out, f"{sc}-control-main.console.txt"), "w").write(o)
        want_results, want_status = DRIVE_WANT[sc]
        got = [supervise_first(os.path.join(ev, exp["controls"][c]["dir"], "RESULT.txt")) for c in K.ORDER]
        got = [g for g in got if g is not None]
        record(f"{sc} control.main end to end: the RESULT label; C7 (blocked), C1 and C25 (accepted) refused by name, "
               "nothing written for them", want_results, got, rc == 0 and got == want_results
               and "C7 REFUSAL: REFUSED: C7 is MUTATION_CONTROL_BLOCKED" in o
               and "C1 REFUSAL: REFUSED: C1 is not executed" in o and "C25 REFUSAL: REFUSED: C25 is not executed" in o
               and not [x for x in os.listdir(os.path.join(ev, "receipts")) if x.endswith(("-C7", "-C1", "-C25"))])
        c4 = os.path.join(ev, exp["controls"]["C4"]["dir"])
        if sc in ("M2", "M4"):
            record(f"{sc} the stopped path: C4 stopped at its primary (no witness run, no link, no G.5 record); restored "
                   "and authenticated", "no 04c-witness, 06a, 06b or 07-g5; RESTORE: EXACTLY_C; RESTORE AUTH: OK",
                   [x for x in sorted(os.listdir(c4))],
                   not [x for x in ("04c-witness", "06a-healthy-witness", "06b-link", "07-g5")
                        if os.path.exists(os.path.join(c4, x))]
                   and supervise_first(os.path.join(c4, "05-restore/RESTORE.txt")) == "RESTORE: EXACTLY_C"
                   and supervise_first(os.path.join(c4, "05b-restore-auth/RESTORE-AUTH.txt")) == "RESTORE AUTH: OK")
        if sc in ("M3", "M5"):
            record(f"{sc} the witness not admissible ({'killed from outside' if sc == 'M3' else 'G.6 breached'}): "
                   "restored and authenticated; nothing linked", "RESTORE: EXACTLY_C; no 06a, 06b or 07-g5",
                   [x for x in sorted(os.listdir(c4))],
                   supervise_first(os.path.join(c4, "05-restore/RESTORE.txt")) == "RESTORE: EXACTLY_C"
                   and not [x for x in ("06a-healthy-witness", "06b-link", "07-g5") if os.path.exists(os.path.join(c4, x))])
        if sc in ("M1", "M6"):
            import g5  # noqa: E402
            want_g = [g5.PRIMARY_OK, g5.WITNESS_OK, "RESTORE: EXACTLY_C", "RESTORE AUTH: OK", g5.HEALTHY_OK,
                      "LINK: VERIFIED", g5.G5_OK]
            g = [supervise_first(os.path.join(c4, f)) for f in (
                "04-run/CLASSIFICATION.txt", "04c-witness/CLASSIFICATION.txt", "05-restore/RESTORE.txt",
                "05b-restore-auth/RESTORE-AUTH.txt", "06a-healthy-witness/HEALTHY.txt", "06b-link/LINK.txt",
                "07-g5/G5.txt")]
            printed = "P4" if sc == "M1" else "P8"
            g5txt = open(os.path.join(c4, "07-g5/G5.txt")).read() if os.path.exists(os.path.join(c4, "07-g5/G5.txt")) else ""
            record(f"{sc} every G.5 step run by control.main itself: primary, witness, restore, restore-auth, healthy "
                   f"link, unchanged-core link, G.5 record; (F) proposed with the first property {printed}",
                   "each exactly its success label; the recording names " + printed, [x[:40] for x in g if x],
                   g == want_g and f"'C4: P4, P8 (first failure {printed}; witness)'" in g5txt)
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
            record(f"{sc} final-gate rehearsal (final_gate.py in the synthetic root): FAILED with exactly the G6 check "
                   "of the synthetic pinned executable failing (it names no file); every other check holds",
                   "1 failing check, 'G6 C4: pinned mutant executable preserved'",
                   f"{len(fails)} failing: {[f[:60] for f in fails]}; last {o.rstrip().splitlines()[-1][:80]!r}",
                   rc == 1 and len(fails) == 1
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
