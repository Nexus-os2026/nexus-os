#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 9-16: execute ONE authorized control, once.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 6): the gate starts after the Architect-accepted C22
record (C22 is never executed again), and the restoration status predicate is the path-aware policy of
verify_mutant.status_policy (restored_check). Every other step is unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (sections 5, 7-10 and 13): gate/order logic only. C22, C14,
C19, C21 and C27 are Architect-accepted and refused; the order is C28 C12 C9 C11 C17 C18a C18b; C12's prior step is
C28's candidate RESULT. C28 (main_c28) starts from its existing, Architect-admissible build: no worktree is created, no
patch is applied, nothing is built; 01 the read-only admission (c28_admit.py), 02 the read-only pin, 04 one supervised
invocation of the pinned preserved executable on the restored worktree, classified by the unchanged classify_b (source
EXACTLY_C, the restored state; the mutant identity is carried by the admission and the pin), 05 the restored worktree
re-authenticated read-only by the repaired verifier, 06 the carried-forward T-RES (e2) link. Every other step and every
control from C12 on is unchanged.

    control.py <ID>      ID in the continue order: C28 C12 C9 C11 C17 C18a C18b (C22 C14 C19 C21 C27 are refused)

Steps (each writes its receipt under receipts/<n>-<ID>/; any failure stops the control, restores the source where the
patch was applied, writes RESULT.txt with MUTATION_CONTROL_BLOCKED, and no later control can start):

  gate       the freeze holds; the previous control's RESULT is a candidate (for C14: the frozen Architect ruling
             record accepting C22, the carried-forward baseline receipts and healthy build are PASS, and the static
             pre-execution gate holds); this control's receipt directory and worktree do not exist
  01         a new mission-owned disposable worktree exactly at C, authenticated (worktrees.py)
  02         the frozen catalog patch (authenticated copy, sha256 re-checked) applied with `git apply`; then
             verify_mutant.py: only the catalog-named file changed, holding exactly the catalog's mutant blob and sha256;
             nothing staged, nothing untracked, every other tracked file byte-equal to C (no second control, no overlay)
  03         the exact build command in that worktree with a fresh worktree-local target (supervise.build); C22: type
             (d) classification; every other control: the build must succeed and its one library-test executable is
             pinned (a failure kills nothing and stops the mission; no rebuild)
  04         (not C22) one supervised invocation of the pinned mutant executable with exactly the catalog's selection;
             classified type (a) or (b)
  05         restoration: `git apply -R` of the same patch; the file's blob and sha256 equal C's; verify_tree.py: every
             tracked file byte-equal to C; the path-aware status policy: no tracked change, ignored entries only
             beneath the worktree's own real target directory
  06         (only after a candidate classification and a verified restoration) the restored-side receipt is linked:
             C22 the healthy build (e4); C14/C19/C21/C27/C28 their standalone (e2); C12 the standalone X-1 (e1);
             C9/C11/C17 the accepted Stage-2 F-B2 (e1); C18a/C18b the accepted Stage-2 F-G (e1)

No mutant source is committed, staged or written to any object store. Nothing is retried. The worktree and its target
are kept for review.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c28_admit  # noqa: E402
import classify  # noqa: E402
import common as K  # noqa: E402
import supervise  # noqa: E402
import verify_mutant  # noqa: E402
import worktrees  # noqa: E402

RESULT_OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
RESULT_BLOCKED = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED"
APPLY_OK = "APPLY: EXACTLY_C_PLUS_PATCH"
RESTORE_OK = "RESTORE: EXACTLY_C"
LINK_OK = "LINK: VERIFIED"


def git(wt, *args):
    r = subprocess.run(["git", "-C", wt, *args], capture_output=True, text=True)
    return {"argv": ["git", "-C", wt, *args], "rc": r.returncode, "stdout": r.stdout, "stderr": r.stderr}


def gate(cid, exp):
    if cid not in K.CONTINUE_ORDER:
        return [f"{cid} is not in the continue order (C22 C14 C19 C21 C27 are Architect-accepted and never executed "
                f"again)"], []
    i = K.CONTINUE_ORDER.index(cid)
    need = []
    problems, lines = [], []
    if i == 0:
        # The carried-forward receipts and the ruling records are frozen byte copies (freeze_check below).
        carried = [b for b in exp["baselines"] if b["dir"] in K.CARRIED]
        need += [(b["dir"] + "/CLASSIFICATION.txt", "CLASSIFICATION: PASS") for b in carried]
        need += [(K.RULING, K.RULING_FIRST), (K.C28_RULING, K.C28_RULING_FIRST)]
        need += [("receipts/06-PRE-EXECUTION-GATE.txt", "PRE-EXECUTION GATE: ALL CHECKS HOLD")]
        fr = json.load(open(supervise.FREEZE)) if os.path.exists(supervise.FREEZE) else {"files": {}}
        frozen = [r for r in [K.RULING, K.C28_RULING] + [b["dir"] + "/descriptor.json" for b in carried]
                  if r not in fr["files"]]
        lines.append(f"the Architect ruling records and the carried-forward baselines {[b['key'] for b in carried]} "
                     f"are in the freeze: {'yes' if not frozen else 'NO: ' + str(frozen)}")
        if frozen:
            problems.append(f"not frozen: {frozen}")
    else:
        need += [(exp["controls"][K.CONTINUE_ORDER[i - 1]]["dir"] + "/RESULT.txt", RESULT_OK)]
    for p, want in need:
        got = supervise.first_line(os.path.join(K.EV, p))
        lines.append(f"prior step: {p}: {got!r} (required {want!r})")
        if got != want:
            problems.append(f"{p} is not {want!r}")
    later = [c for c in K.CONTINUE_ORDER[i + 1:] if os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    if later:
        problems.append(f"a later control's receipts exist: {later}")
    rec = []
    fz = supervise.freeze_check(rec)
    lines += rec
    if fz:
        problems.append(fz)
    if cid == "C28":
        if not os.path.isdir(exp["controls"][cid]["worktree"]):
            problems.append("the preserved C28 worktree is absent")
    elif os.path.lexists(exp["controls"][cid]["worktree"]):
        problems.append("the control's worktree path exists")
    return problems, lines


def apply_patch(wt, ctl, outdir):
    os.makedirs(outdir)
    patch = os.path.join(K.EV, ctl["patch"]["input"])
    rec = {"patch": patch, "patch_sha256": K.sha256_file(patch), "catalog_patch_sha256": ctl["patch"]["sha256"],
           "catalog_patch_file": ctl["patch"]["catalog_file"]}
    ok = rec["patch_sha256"] == ctl["patch"]["sha256"]
    rec["check"] = git(wt, "apply", "--check", "--whitespace=nowarn", patch) if ok else None
    ok = ok and rec["check"]["rc"] == 0
    rec["apply"] = git(wt, "apply", "--whitespace=nowarn", patch) if ok else None
    applied = bool(rec["apply"]) and rec["apply"]["rc"] == 0
    r = subprocess.run([sys.executable, supervise.VERIFY_MUTANT, wt, ctl["source_file"], ctl["mutant_blob"],
                        ctl["mutant_sha256"]], capture_output=True, text=True)
    rec["verify_mutant"] = json.loads(r.stdout)
    rec["numstat"] = git(wt, "diff", "--numstat")["stdout"]
    rec["changed_files"] = git(wt, "diff", "--name-only")["stdout"].split()
    ok = (applied and rec["verify_mutant"]["verdict"] == "EXACTLY_C_PLUS_PATCH"
          and rec["changed_files"] == [ctl["source_file"]])
    m = rec["verify_mutant"]["mutant_file"] or {}
    lines = [APPLY_OK if ok else "APPLY: FAILED",
             f"control {ctl['id']}: {ctl['description']}",
             f"patch: {ctl['patch']['input']} (the frozen R2 packet's {ctl['patch']['catalog_file']}), sha256 "
             f"{rec['patch_sha256']} (catalog {ctl['patch']['sha256']})",
             f"git apply --check: rc {rec['check']['rc'] if rec['check'] else 'not run'}; git apply: rc "
             f"{rec['apply']['rc'] if rec['apply'] else 'not run'} {(rec['apply'] or {}).get('stderr', '').strip()}",
             f"changed files (git diff --name-only): {rec['changed_files']}; numstat {rec['numstat'].strip()!r}",
             f"verify_mutant.py: {rec['verify_mutant']['verdict']}; {m.get('path')} git blob {m.get('git_blob')} "
             f"(expected {ctl['mutant_blob']}), sha256 {m.get('sha256')} (expected {ctl['mutant_sha256']}); status "
             f"{rec['verify_mutant']['status_entries']}; other mismatches {rec['verify_mutant']['other_content_mismatches']}"
             f"; staged {rec['verify_mutant']['index_differs_from_head']}; {rec['verify_mutant']['tracked_blobs_compared']}"
             f" tracked blobs compared",
             "the mutant blob is computed in-process and never written to any object store; nothing is staged or "
             "committed"]
    json.dump(rec, open(os.path.join(outdir, "APPLY.json"), "w"), indent=1)
    open(os.path.join(outdir, "APPLY.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return ok, applied


def restored_check(wt, when):
    """Read-only: is the worktree exactly C (resume mission section 7, restored worktree)? verify_tree.py (Stage 1's,
    unchanged: HEAD, tree, detached, index, every tracked byte and mode) and the path-aware status policy (no tracked
    change; ignored entries only beneath the worktree's own real target directory)."""
    rv = []
    v = supervise.tree_check(rv, wt, when)
    pol = verify_mutant.status_policy(wt, None)
    ok = v["verdict"] == "EXACTLY_C" and pol["ok"]
    return {"verdict": "EXACTLY_C" if ok else "NOT_C", "verify_tree": v, "status_policy": pol, "line": rv[0]}


def restore(wt, ctl, outdir, applied):
    os.makedirs(outdir)
    patch = os.path.join(K.EV, ctl["patch"]["input"])
    rec = {"applied_before_restoration": applied}
    path = os.path.join(wt, ctl["source_file"])
    before = open(path, "rb").read()
    rec["before"] = {"git_blob": K.git_blob_id(before), "sha256": K.sha256_bytes(before)}
    if applied:
        rec["check"] = git(wt, "apply", "-R", "--check", "--whitespace=nowarn", patch)
        rec["reverse"] = git(wt, "apply", "-R", "--whitespace=nowarn", patch) if rec["check"]["rc"] == 0 else None
    after = open(path, "rb").read()
    rec["after"] = {"git_blob": K.git_blob_id(after), "sha256": K.sha256_bytes(after)}
    chk = restored_check(wt, "after restoration")
    v, pol = chk["verify_tree"], chk["status_policy"]
    rv = [chk["line"]]
    rec["verify_tree"] = v
    rec["status_policy"] = pol
    ok = (rec["after"]["git_blob"] == ctl["baseline_blob"] and rec["after"]["sha256"] == ctl["baseline_sha256"]
          and v["verdict"] == "EXACTLY_C" and pol["ok"])
    lines = [RESTORE_OK if ok else "RESTORE: FAILED",
             f"control {ctl['id']}: method: reverse application of the same frozen patch (git apply -R) in the "
             f"disposable worktree {wt}" if applied else f"control {ctl['id']}: the patch was not applied; state verified",
             f"{ctl['source_file']}: before restoration git blob {rec['before']['git_blob']} sha256 {rec['before']['sha256']}",
             f"{ctl['source_file']}: after restoration git blob {rec['after']['git_blob']} sha256 {rec['after']['sha256']} "
             f"(C: {ctl['baseline_blob']} / {ctl['baseline_sha256']})",
             rv[0], f"path-aware status policy (--ignored --untracked-files=all --no-renames -z): "
             f"{'HOLDS' if pol['ok'] else 'REJECTED'}; {pol['records']} records, {pol['target_entries']} ignored entries "
             f"beneath the worktree-local real target; shown {pol['shown']}; rejected {pol['rejected'][:5]}",
             "every changed file is byte-equal to C; the worktree is clean but for its own target directory"
             if ok else "RESTORATION NOT VERIFIED"]
    json.dump(rec, open(os.path.join(outdir, "RESTORE.json"), "w"), indent=1)
    open(os.path.join(outdir, "RESTORE.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return ok


def link(ctl, exp, outdir, build_desc):
    os.makedirs(outdir)
    L = ctl["linked"]
    checks = []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))
    exe_now = K.sha256_file(K.EXE)
    if L["type"] == "e4":
        d = os.path.join(K.EV, L["dir"])
        chk("the healthy build (e4) is classified PASS", supervise.first_line(os.path.join(d, "CLASSIFICATION.txt")) ==
            "CLASSIFICATION: PASS")
        h = json.load(open(os.path.join(d, "descriptor.json")))
        norm = lambda e, t: {k: ("<worktree>/target" if v == t else v) for k, v in e.items()}
        chk("identical command", h["argv"] == build_desc["argv"] == K.BUILD_ARGV, h["argv"])
        chk("identical environment (the worktree-local target is the only per-worktree value)",
            norm(h["env"], h["target"]) == norm(build_desc["env"], build_desc["target"]), norm(h["env"], h["target"]))
        chk("identical target policy (a fresh worktree-local target in each worktree)",
            h["target"] == os.path.join(h["worktree"], "target") and build_desc["target"] ==
            os.path.join(build_desc["worktree"], "target"), f"{h['target']}; {build_desc['target']}")
        chk("identical toolchain (rustc -vV, cargo -vV, active toolchain)",
            all(h["toolchain"][k] == build_desc["toolchain"][k] for k in ("rustc_vV", "cargo_vV", "active_toolchain")),
            h["toolchain"]["rustc"])
        chk("identical build inputs (rust-toolchain.toml, Cargo.lock, Cargo configuration, manifests)",
            h["toolchain"]["build_inputs"] == build_desc["toolchain"]["build_inputs"], "")
        chk("identical supervision and resource limits", all(h["supervisor"][k] == build_desc["supervisor"][k] for k in
                                                             ("memory_max", "swap_max", "time_limit_seconds")),
            f"{h['supervisor']['memory_max']} / {h['supervisor']['swap_max']} / {h['supervisor']['time_limit_seconds']}")
        chk("the intended difference is the source identity only: healthy EXACTLY_C, mutant C plus the C22 patch",
            h["source_before"]["verdict"] == "EXACTLY_C" and build_desc["source_before"]["verdict"] ==
            "EXACTLY_C_PLUS_PATCH", "")
        ref = f"{L['dir']} (type (e4) healthy-build success)"
    elif L.get("stage2"):
        cfg = L["configuration"]
        d = os.path.join(K.EV, L["input_dir"])
        sums = K.blob(K.STAGE2, K.STAGE2_PACKET + "/SHA256SUMS").decode()
        listed = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}
        rel = L["directory"][len(K.STAGE2_PACKET) + 1:]
        bad = [f for f in os.listdir(d) if listed.get(f"{rel}/{f}") != K.sha256_file(os.path.join(d, f))]
        chk(f"the linked receipt's files equal the accepted Stage-2 evidence ({K.STAGE2[:8]} SHA256SUMS)", not bad, bad)
        chk("its classification blob and raw output identity are the catalog's",
            K.git_blob_id(open(os.path.join(d, "CLASSIFICATION.txt"), "rb").read()) == L["classification_blob"]
            and K.git_blob_id(open(os.path.join(d, "raw-output.txt"), "rb").read()) == L["raw_output_blob"]
            and K.sha256_file(os.path.join(d, "raw-output.txt")) == L["raw_output_sha256"], "")
        cl = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
        chk("accepted unchanged-core PASS of the original explorer", cl[:2] == [
            "CLASSIFICATION: PASS", "outer class (AD-1-R4 K.2): PASS — admissible unchanged-core original-explorer PASS"],
            cl[:2])
        h = json.load(open(os.path.join(d, "descriptor.json")))
        chk(f"exact configuration: the same test and flags as this control's run ({cfg})",
            h["argv"] == [K.EXE, *ctl["argv_tail"]] and h["test"] == ctl["test"], h["argv"])
        chk("exact Candidate C: the source exactly C before and after",
            h["source_before"]["verdict"] == "EXACTLY_C" == h["source_after"]["verdict"]
            and h["source_before"]["head"] == K.C and h["source_before"]["tree"] == K.C_TREE, "")
        chk("execution policy: env -i PATH/LANG/RUST_BACKTRACE; 16 GiB; zero swap; 2700 s",
            h["env"] == dict(K.RUN_ENV) and h["supervisor"]["memory_max"] == K.MEMORY_MAX
            and h["supervisor"]["swap_max"] == "0" and h["supervisor"]["time_limit_seconds"] == ctl["limit_seconds"], "")
        chk("artifact identity at this control's restoration: the linked receipt's executable sha256 equals the pinned "
            "baseline executable, hashed now", h["executable"]["before"]["sha256"] == h["executable"]["after"]["sha256"]
            == K.EXE_SHA256 == exe_now, exe_now)
        ref = f"{L['directory']} at {K.STAGE2} (accepted exact-final {cfg} unchanged-core PASS, type (e1))"
    else:
        d = os.path.join(K.EV, L["dir"])
        cl = supervise.first_line(os.path.join(d, "CLASSIFICATION.txt"))
        chk(f"the standalone baseline ({L['type']}) of this mission is classified PASS", cl == "CLASSIFICATION: PASS", cl)
        h = json.load(open(os.path.join(d, "descriptor.json")))
        chk("the same test and flags as this control's run, on the pinned baseline executable",
            h["argv"] == [K.EXE, *ctl["argv_tail"]] and h["test"] == ctl["test"], h["argv"])
        chk("exact Candidate C before and after", h["source_before"]["verdict"] == "EXACTLY_C" ==
            h["source_after"]["verdict"], "")
        chk("artifact identity at this control's restoration: the baseline receipt's executable sha256 equals the "
            "pinned baseline executable, hashed now", h["executable"]["before"]["sha256"] ==
            h["executable"]["after"]["sha256"] == K.EXE_SHA256 == exe_now, exe_now)
        ref = f"{L['dir']} (type ({L['type']}), standalone, this mission)"
    ok = all(o for _, o, _ in checks)
    lines = [LINK_OK if ok else "LINK: FAILED", f"control {ctl['id']}: restored-side receipt {ref}",
             "linked only after this control's own restoration was verified (receipt linking does not substitute for "
             "restoration evidence)"]
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    open(os.path.join(outdir, "LINK.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"reference": ref, "checks": checks}, open(os.path.join(outdir, "LINK.json"), "w"), indent=1)
    print("\n".join(lines))
    return ok, ref


def reauthenticate_restoration(wt, ctl, outdir, exe):
    """C28 (continue mission sections 9 and 13): no patch is applied or reversed by this mission; the restored worktree
    (restored by RESUME-01's `git apply -R`, authenticated in 01-admission) is re-authenticated read-only: the changed
    file's blob and sha256 are C's, verify_tree.py EXACTLY_C, the path-aware restored policy, and the pinned
    executable is unchanged."""
    os.makedirs(outdir)
    path = os.path.join(wt, ctl["source_file"])
    data = open(path, "rb").read()
    rec = {"applied_by_this_mission": False, "file": {"git_blob": K.git_blob_id(data), "sha256": K.sha256_bytes(data)}}
    chk = restored_check(wt, "after the C28 run")
    v, pol = chk["verify_tree"], chk["status_policy"]
    rec["verify_tree"] = v
    rec["status_policy"] = pol
    now = supervise.exe_identity(exe["path"])
    rec["executable_now"] = now
    ok = (rec["file"]["git_blob"] == ctl["baseline_blob"] and rec["file"]["sha256"] == ctl["baseline_sha256"]
          and v["verdict"] == "EXACTLY_C" and pol["ok"] and supervise.exe_ok(now, exe["sha256"], exe["size"]))
    lines = [RESTORE_OK if ok else "RESTORE: FAILED",
             f"control {ctl['id']}: no patch is applied or reversed by this mission; the worktree {wt} was restored by "
             f"RESUME-01 (git apply -R of the frozen patch; authenticated in 01-admission A21) and is re-authenticated "
             f"here read-only (GIT_OPTIONAL_LOCKS=0)",
             f"{ctl['source_file']}: git blob {rec['file']['git_blob']} sha256 {rec['file']['sha256']} (C: "
             f"{ctl['baseline_blob']} / {ctl['baseline_sha256']})",
             chk["line"], f"path-aware status policy (--ignored --untracked-files=all --no-renames -z): "
             f"{'HOLDS' if pol['ok'] else 'REJECTED'}; {pol['records']} records, {pol['target_entries']} ignored entries "
             f"beneath the worktree-local real target; shown {pol['shown']}; rejected {pol['rejected'][:5]}",
             f"pinned executable now: sha256 {now['sha256']} size {now['size']} (pinned {exe['sha256']} / {exe['size']})",
             "every tracked file is byte-equal to C; the worktree is clean but for its own target directory"
             if ok else "RESTORATION NOT VERIFIED"]
    json.dump(rec, open(os.path.join(outdir, "RESTORE.json"), "w"), indent=1)
    open(os.path.join(outdir, "RESTORE.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return ok


def main_c28():
    """C28 from its existing, Architect-admissible build (continue mission sections 2, 8 and 9): no rebuild."""
    os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # the preserved worktree is RESUME-01's: read, never written
    exp = supervise.expectations()
    ctl = exp["controls"]["C28"]
    outdir = os.path.join(K.EV, ctl["dir"])
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {ctl['dir']} exists (one attempt per control; nothing is retried)")
    os.makedirs(outdir)
    problems, lines = gate("C28", exp)
    open(os.path.join(outdir, "00-GATE.txt"), "w").write(
        ("GATE: OPEN" if not problems else "GATE: CLOSED — " + "; ".join(problems)) + "\n" + "\n".join(lines) + "\n")
    if problems:
        return finish(outdir, ctl, RESULT_BLOCKED, [f"not started: {'; '.join(problems)}"])
    wt = ctl["worktree"]
    facts = ["build: NOT REPEATED (the existing RESUME-01 C28 build; no Cargo, no rustc; 03-build absent by design)"]
    status, stop, candidate, exe = RESULT_BLOCKED, None, False, None
    c28_admit.admit_receipt(os.path.join(outdir, "01-admission"))
    first = supervise.first_line(os.path.join(outdir, "01-admission/ADMISSION.txt"))
    facts.append(f"existing-build admission (narrow C28 ruling): {first}")
    if first != c28_admit.ADMITTED:
        stop = "the existing C28 build is not admissible under the narrow ruling (no rebuild)"
    else:
        c28_admit.pin_receipt(os.path.join(outdir, "02-pin"), os.path.join(outdir, "01-admission/ADMISSION.txt"))
        pfirst = supervise.first_line(os.path.join(outdir, "02-pin/PIN.txt"))
        facts.append(f"pin: {pfirst}")
        if pfirst != c28_admit.PINNED:
            status = "CONTROL RESULT: C28_PRESERVED_ARTIFACT_BLOCKED"
            stop = "C28_PRESERVED_ARTIFACT_BLOCKED: the preserved executable is missing or differs (not rebuilt)"
        else:
            pin = json.load(open(os.path.join(outdir, "02-pin/PIN.json")))
            exe = {"path": pin["path"], "sha256": pin["sha256"], "size": pin["size"]}
            facts.append(f"pinned preserved executable: {pin['path']} sha256 {pin['sha256']} size {pin['size']}")

            def source_ok(rec, when):
                d = restored_check(wt, when)
                rec.append(d["line"] + f"; path-aware restored status policy "
                           f"{'HOLDS' if d['status_policy']['ok'] else 'REJECTED'} "
                           f"({d['status_policy']['target_entries']} ignored entries beneath the real target)")
                return d, d["verdict"] == "EXACTLY_C"
            rdir = os.path.join(outdir, "04-run")
            supervise.invoke(rdir, f"SUPERVISED MUTANT RUN — control C28: {ctl['test']} on the pinned preserved C28 "
                                   f"executable (existing build, not rebuilt; type (b)); the worktree is the restored one",
                             pin["path"], pin["sha256"], pin["size"], os.path.join(wt, K.CRATE), wt, ctl["test"],
                             ctl["ignored"], ctl["argv_tail"], ctl["limit_seconds"], source_ok, "mutant-unit",
                             [(ctl["dir"] + "/02-pin/PIN.txt", c28_admit.PINNED)],
                             {"control": "C28", "configuration": "named unit test", "mutant_blob": ctl["mutant_blob"],
                              "patch_sha256": ctl["patch"]["sha256"], "existing_build": pin["build_receipt"],
                              "worktree_source_state": "EXACTLY_C (restored; the executable was built from C plus the "
                                                       "frozen C28 patch: 01-admission A3)"})
            # The unchanged classifier; the run's source check is the restored state (the mutant identity is the
            # admission's and the pin's), so the mutant blob is withheld from check_identity only.
            classify.classify_b(rdir, dict(ctl, mutant_blob=None), exe, source_verdict="EXACTLY_C")
            cls = open(os.path.join(rdir, "CLASSIFICATION.txt")).read().splitlines()
            candidate = cls[0] == classify.CANDIDATE
            facts.append(f"run (type (b)): {cls[0]}; {cls[1]}")
            if not candidate:
                stop = "the C28 run is not admissible kill evidence of the named test"
    rok = reauthenticate_restoration(wt, ctl, os.path.join(outdir, "05-restore"),
                                     exe or {"path": c28_admit.RULING["executable"], "sha256": c28_admit.RULING[
                                         "exe_sha256"], "size": c28_admit.RULING["exe_size"]})
    facts.append(f"restoration (re-authenticated, read-only): {supervise.first_line(os.path.join(outdir, '05-restore/RESTORE.txt'))}")
    if not rok:
        stop = (stop + "; " if stop else "") + "restoration not re-authenticated"
    if candidate and rok:
        lok, ref = link(ctl, exp, os.path.join(outdir, "06-link"), None)
        facts.append(f"linked restored-side receipt: {supervise.first_line(os.path.join(outdir, '06-link/LINK.txt'))}; "
                     f"{ref}")
        if not lok:
            stop = "the restored-side receipt could not be linked"
    if stop:
        facts.append(f"STOP: {stop}. No later control is executed.")
    return finish(outdir, ctl, RESULT_OK if not stop and candidate and rok else status, facts)


def finish(outdir, ctl, status, facts):
    lines = [status, f"control {ctl['id']}: {ctl['description']}", f"receipt type: {ctl['receipt_type']}"] + facts
    open(os.path.join(outdir, "RESULT.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "status": status, "facts": facts}, open(os.path.join(outdir, "CONTROL-RESULT.json"),
                                                                             "w"), indent=1)
    print("\n".join(lines))
    return 0 if status == RESULT_OK else 1


def main(cid):
    if cid in K.CONTINUE_ACCEPTED or cid not in K.CONTINUE_ORDER:
        raise SystemExit(f"REFUSED: {cid} is not executed by {K.MISSION} ({' '.join(K.CONTINUE_ACCEPTED)} are "
                         f"Architect-accepted; only {' '.join(K.CONTINUE_ORDER)} run)")
    if cid == "C28":
        return main_c28()
    exp = supervise.expectations()
    ctl = exp["controls"][cid]
    outdir = os.path.join(K.EV, ctl["dir"])
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {ctl['dir']} exists (one attempt per control; nothing is retried)")
    os.makedirs(outdir)
    problems, lines = gate(cid, exp)
    open(os.path.join(outdir, "00-GATE.txt"), "w").write(
        ("GATE: OPEN" if not problems else "GATE: CLOSED — " + "; ".join(problems)) + "\n" + "\n".join(lines) + "\n")
    if problems:
        return finish(outdir, ctl, RESULT_BLOCKED, [f"not started: {'; '.join(problems)}"])
    wt = ctl["worktree"]
    facts = []
    r, ok = worktrees.create(wt, f"control {cid} disposable mutant worktree")
    worktrees.write(os.path.join(outdir, "01-worktree"), "CREATE", f"CREATE the {cid} worktree {wt} at C", r, ok)
    if not ok:
        return finish(outdir, ctl, RESULT_BLOCKED, ["worktree creation failed"])
    a, ok = worktrees.authenticate(wt, f"control {cid} disposable mutant worktree")
    worktrees.write(os.path.join(outdir, "01-worktree"), "AUTHENTICATE", f"AUTHENTICATE the {cid} worktree {wt}", a, ok)
    if not ok:
        return finish(outdir, ctl, RESULT_BLOCKED, ["worktree authentication failed; no patch applied"])
    ok, applied = apply_patch(wt, ctl, os.path.join(outdir, "02-apply"))
    facts.append(f"apply: {supervise.first_line(os.path.join(outdir, '02-apply/APPLY.txt'))}")
    candidate, stop = False, None
    if not ok:
        stop = "the patch did not produce exactly the catalog's mutant"
    else:
        def source_ok(rec, when):
            d = supervise.mutant_check(rec, wt, ctl, when)
            return d, d["verdict"] == "EXACTLY_C_PLUS_PATCH"
        bdir = os.path.join(outdir, "03-build")
        supervise.build(bdir, f"SUPERVISED MUTANT BUILD — control {cid} ({ctl['description']}); the exact command",
                        wt, source_ok, [(ctl["dir"] + "/02-apply/APPLY.txt", APPLY_OK)],
                        {"control": cid, "role": "mutant build", "patch_sha256": ctl["patch"]["sha256"],
                         "mutant_blob": ctl["mutant_blob"], "mutant_sha256": ctl["mutant_sha256"]},
                        manifest=(ctl["kind"] == "build"))
        if ctl["kind"] == "build":
            classify.classify_d(bdir, ctl)
            first = supervise.first_line(os.path.join(bdir, "CLASSIFICATION.txt"))
            candidate = first == classify.CANDIDATE
            facts.append(f"build (type (d)): {first}; {open(os.path.join(bdir, 'CLASSIFICATION.txt')).read().splitlines()[1]}")
            facts.append("mutant executable: NOT PRODUCED BY DESIGN; mutant test run: NOT PRODUCED BY DESIGN")
            if not candidate:
                stop = "the C22 build is not the exact E0283 diagnostic"
        else:
            classify.classify_build_success(bdir, "pin", "EXACTLY_C_PLUS_PATCH", ctl["mutant_blob"],
                                            expect_target=os.path.join(wt, "target"))
            first = supervise.first_line(os.path.join(bdir, "CLASSIFICATION.txt"))
            facts.append(f"build: {first}")
            if first != "BUILD: PINNED":
                stop = "the mutant build did not succeed (kills nothing; no rebuild)"
            else:
                pin = json.load(open(os.path.join(bdir, "PIN.json")))
                facts.append(f"pinned mutant executable: {pin['path']} sha256 {pin['sha256']} size {pin['size']}")
                exe = {"path": pin["path"], "sha256": pin["sha256"], "size": pin["size"]}
                rdir = os.path.join(outdir, "04-run")
                title = (f"SUPERVISED MUTANT RUN — control {cid}: {ctl['test']} on the pinned mutant executable "
                         f"(type ({'a' if ctl['kind'] == 'explore' else 'b'}))")
                supervise.invoke(rdir, title, pin["path"], pin["sha256"], pin["size"], os.path.join(wt, K.CRATE), wt,
                                 ctl["test"], ctl["ignored"], ctl["argv_tail"], ctl["limit_seconds"], source_ok,
                                 "mutant-" + ctl["kind"], [(ctl["dir"] + "/03-build/CLASSIFICATION.txt", "BUILD: PINNED")],
                                 {"control": cid, "configuration": ctl.get("configuration", "named unit test"),
                                  "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"]})
                (classify.classify_a if ctl["kind"] == "explore" else classify.classify_b)(rdir, ctl, exe)
                cls = open(os.path.join(rdir, "CLASSIFICATION.txt")).read().splitlines()
                candidate = cls[0] == classify.CANDIDATE
                facts.append(f"run (type ({'a' if ctl['kind'] == 'explore' else 'b'})): {cls[0]}; {cls[1]}")
                if not candidate:
                    stop = "the mutant run is not admissible kill evidence of the named property or test"
    rok = restore(wt, ctl, os.path.join(outdir, "05-restore"), applied)
    facts.append(f"restoration: {supervise.first_line(os.path.join(outdir, '05-restore/RESTORE.txt'))}")
    if not rok:
        stop = (stop + "; " if stop else "") + "restoration failed"
    if candidate and rok:
        bdesc = json.load(open(os.path.join(outdir, "03-build/descriptor.json")))
        lok, ref = link(ctl, exp, os.path.join(outdir, "06-link"), bdesc)
        facts.append(f"linked restored-side receipt: {supervise.first_line(os.path.join(outdir, '06-link/LINK.txt'))}; "
                     f"{ref}")
        if not lok:
            stop = "the restored-side receipt could not be linked"
    if stop:
        facts.append(f"STOP: {stop}. No later control is executed.")
    return finish(outdir, ctl, RESULT_OK if not stop and candidate and rok else RESULT_BLOCKED, facts)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
