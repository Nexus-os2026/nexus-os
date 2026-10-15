#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 9-16: execute ONE authorized control, once.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 6): the restoration status predicate is the
path-aware policy of verify_mutant.status_policy (restored_check). Revised by -CONTINUE-02 (gate/order logic, C28).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 2, 5, 11, 12, 14): gate/order logic and the
restored-evidence mappings only. The order is C23 C10 C16 C26 C3a C5 A3 A6 C8 C15; every other control is refused
(the twelve Tranche-1 controls are Architect-accepted; thirteen are not authorized). C23's prior steps are the frozen
Architect ruling and campaign records and the static pre-execution gate; each later control's prior step is the
previous control's candidate RESULT. link() links, only after the control's own verified restoration, the accepted
exact-final unchanged-core receipt the catalog names (Stage 1 or Stage 2; C8: the Stage-1 composed-quotient F-B PASS,
admissible for F-B's restored side only) or, for C15, the AD-1-R5 F refinement discharge record (never PASS). The
C28-only steps are gone. Every other step, function and receipt (worktree, apply, build, run, classification,
restoration, RESULT) is unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 4, 6-9, 15): gate/order logic beginning at C5 and
the C5-only steps. The order is C5 A3 A6 C8 C15; every other control is refused (seventeen are Architect-accepted;
thirteen are not authorized). C5's prior steps are the frozen Architect ruling records, the campaign record and the
static pre-execution gate; each later control's prior step is the previous control's candidate RESULT. C5 is never
rebuilt and no patch is applied to it (main_c5): its preserved Tranche-2 build is admitted by the dedicated checker
(c5.admit), its preserved worktree authenticated read-only (c5.worktree_check), its preserved executable pinned
(c5.pin), run once (supervise.invoke, unchanged) with the worktree exactly C, classified by the unchanged classify_a
(source EXACTLY_C: the mutant identity is bound through the admitted build record and the pinned executable), the
worktree and the executable reauthenticated after the run, and only then the restored side linked. A3, A6, C8 and C15
run main() unchanged: every other step, function and receipt is Tranche 2's, with the zero-warning, zero-error build
policy of classify_build_success (unchanged; the C5 ruling does not apply to them). link() is unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 2, 4, 5, 7-10, 12): gate/order logic for the order
C2 C24 C3b C3c C6 C7 A1 A2 C20 A5 (every other control is refused: 22 are Architect-accepted; C1, C4 and C25 are
reserved). C2's prior steps are the frozen ruling and campaign records and the static pre-execution gate; each later
control's prior step is the previous control's candidate RESULT. The C5-only steps (main_c5, c5.py) are gone. main()
is the continuation's, with two insertions: after classify_a (unchanged), the tranche-3 rules (t3.rules: the original
explorer, the permitted property, C6/C7 A4 ON, C20/A5 L1 after a complete search) must hold too; and for C7 only, after
an admissible F-PT/P6 candidate and before restoration, the informational run without A4 on F-3 (mission section 5):
the same pinned C7 executable, full_f3, one supervised invocation (supervise.invoke, unchanged), classified by
t3.classify_info, never a kill receipt (an INCOMPLETE there revokes nothing; any infrastructure, identity, OOM, signal,
overshoot or malformed-receipt failure stops the mission). link() keeps every check and gains two: the restored side is
linked only after this control's own restoration receipt exists and records RESTORE: EXACTLY_C (mission section 10);
and its composed-quotient branch names the control's configuration (F-PT for C6/C7) instead of F-B. git, apply_patch,
restored_check, restore and finish are unchanged.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (sections 2-17, AMENDMENT-01): gate/order logic for the order
C24 C3b C3c C6 C7 A1 A2 C20 A5 (every other control is refused: 23 are Architect-accepted; C1, C4 and C25 are reserved);
C24's prior steps are the frozen ruling, build-ruling, amendment and campaign records and the static pre-execution gate;
each later control's prior step is the previous control's candidate RESULT. C24 is never rebuilt and no patch is applied
to it (main_c24, the continuation's main_c5 for C24): its preserved Tranche-3 build is admitted by the dedicated checker
(c24.admit), its preserved worktree authenticated read-only (c24.worktree_check), its preserved executable pinned
(c24.pin), run once (supervise.invoke, unchanged) with the worktree exactly C, classified by the unchanged classify_a
(source EXACTLY_C: the mutant identity is bound through the admitted build record and the pinned executable) and the
tranche-3 rules (t3.rules, unchanged), the worktree and the executable reauthenticated after the run, and only then the
restored side linked (C24_PRESERVED_ARTIFACT_BLOCKED when the preserved worktree or executable is missing or differs).
main() for the other eight is the Tranche-3 one with one insertion: after the unchanged general classifier, for C3b and
C3c only, the dedicated build-admission checker (c3w.admit: a clean build, or exactly the one predicted warning group of
AMENDMENT-01) decides whether the build is admitted and which pin record the run uses; every other control keeps the
general classifier's BUILD: PINNED as its only admission (zero warnings, zero errors). link() links C24's restored side
only after its own reauthentication receipt records REAUTH_OK (every other control: its own RESTORE: EXACTLY_C, as
before). git, apply_patch, restored_check, restore and finish are unchanged.

    control.py <ID>      ID in this mission's order: C24 C3b C3c C6 C7 A1 A2 C20 A5

Steps (each writes its receipt under receipts/<n>-<ID>/; any failure stops the control, restores the source where the
patch was applied, writes RESULT.txt with MUTATION_CONTROL_BLOCKED, and no later control can start):

  gate       the freeze holds; the previous control's RESULT is a candidate (for C23: the frozen ruling and campaign
             records and the static pre-execution gate); this control's receipt directory and worktree do not exist
  01         a new mission-owned disposable worktree exactly at C, authenticated (worktrees.py)
  02         the frozen catalog patch (authenticated copy, sha256 re-checked) applied with `git apply`; then
             verify_mutant.py: only the catalog-named file changed, holding exactly the catalog's mutant blob and sha256;
             nothing staged, nothing untracked, every other tracked file byte-equal to C (no second control, no overlay)
  03         the exact build command in that worktree with a fresh worktree-local target (supervise.build); the build
             must succeed with no error and no warning diagnostic and its one library-test executable is pinned (a
             failure kills nothing and stops the mission; no rebuild)
  04         one supervised invocation of the pinned mutant executable with exactly the catalog's selection on the
             original explorer; classified type (a)
  04a        the tranche-3 rules over that run (t3.rules)
  04b        C7 only: the informational F-3 run without A4 on the same executable (t3.classify_info)
  05         restoration: `git apply -R` of the same patch; the file's blob and sha256 equal C's; verify_tree.py: every
             tracked file byte-equal to C; the path-aware status policy: no tracked change, ignored entries only
             beneath the worktree's own real target directory
  06         (only after a candidate classification and a verified restoration) the restored side is linked

No mutant source is committed, staged or written to any object store. Nothing is retried. The worktree and its target
are kept for review.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c24  # noqa: E402
import c3w  # noqa: E402
import classify  # noqa: E402
import common as K  # noqa: E402
import supervise  # noqa: E402
import t3  # noqa: E402
import verify_mutant  # noqa: E402
import worktrees  # noqa: E402

RESULT_OK = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
RESULT_BLOCKED = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED"
RESULT_ARTIFACT_BLOCKED = "CONTROL RESULT: C24_PRESERVED_ARTIFACT_BLOCKED"
REAUTH_OK = "C24 REAUTHENTICATION AFTER THE RUN: HOLDS (WORKTREE EXACTLY C; EXECUTABLE AND TARGET UNCHANGED)"
APPLY_OK = "APPLY: EXACTLY_C_PLUS_PATCH"
RESTORE_OK = "RESTORE: EXACTLY_C"
LINK_OK = "LINK: VERIFIED"
FLAGS = ["--exact", "--ignored", "--nocapture", "--test-threads=1"]
# AD-1-R5 D: never written, anywhere, about F-R.
PROHIBITED = ["F-R PASS", "F-R ORIGINAL GRAPH PASSED", "F-R ORIGINAL GRAPH EXPLORED"]


def git(wt, *args):
    r = subprocess.run(["git", "-C", wt, *args], capture_output=True, text=True)
    return {"argv": ["git", "-C", wt, *args], "rc": r.returncode, "stdout": r.stdout, "stderr": r.stderr}


def gate(cid, exp):
    if cid not in K.ORDER:
        return [f"{cid} is not in this mission's order (the 23 accepted controls are never executed again; C1, C4 and "
                f"C25 are reserved for the final witness-bound tranche)"], []
    i = K.ORDER.index(cid)
    need = []
    problems, lines = [], []
    if i == 0:
        need += [(r, K.RECORD_FIRST[r]) for r in K.RECORDS]
        need += [("receipts/06-PRE-EXECUTION-GATE.txt", "PRE-EXECUTION GATE: ALL CHECKS HOLD")]
        fr = json.load(open(supervise.FREEZE)) if os.path.exists(supervise.FREEZE) else {"files": {}}
        frozen = [r for r in K.RECORDS + ["receipts/02-EXPECTATIONS.json"] if r not in fr["files"]]
        lines.append(f"the Architect ruling, build-ruling and amendment records, the campaign status record and the "
                     f"expectations are in the freeze: {'yes' if not frozen else 'NO: ' + str(frozen)}")
        if frozen:
            problems.append(f"not frozen: {frozen}")
    else:
        need += [(exp["controls"][K.ORDER[i - 1]]["dir"] + "/RESULT.txt", RESULT_OK)]
    for p, want in need:
        got = supervise.first_line(os.path.join(K.EV, p))
        lines.append(f"prior step: {p}: {got!r} (required {want!r})")
        if got != want:
            problems.append(f"{p} is not {want!r}")
    later = [c for c in K.ORDER[i + 1:] if os.path.exists(os.path.join(K.EV, exp["controls"][c]["dir"]))]
    if later:
        problems.append(f"a later control's receipts exist: {later}")
    rec = []
    fz = supervise.freeze_check(rec)
    lines += rec
    if fz:
        problems.append(fz)
    if cid == "C24":
        if not os.path.isdir(exp["controls"][cid]["worktree"]):
            problems.append("the preserved C24 worktree is absent")
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
    """Mission section 5: the restored side, linked (never rerun) only after this control's own restoration was
    verified. type e1: the accepted exact-final unchanged-core receipt the catalog names, authenticated again here
    (every file against its packet's SHA256SUMS; the catalog's classification and raw-output identities; Candidate C;
    the execution policy; the pinned baseline executable, hashed now). C8: the Stage-1 composed-quotient F-B PASS,
    admissible for F-B's restored side only (AD-1-R4 F); the mutant itself ran full_fb on the original explorer.
    type fr-discharge (C15, AD-1-R5 F): the refinement discharge record, citable only now: Candidate C, the accepted
    exact-final F-2 receipt, the accepted proof, AD-1-R5 and its Owner adoption; the F-R receipt stays INCOMPLETE. The
    record is not a run, has no outcome line and is never PASS. build_desc is unused (kept for the call's shape)."""
    os.makedirs(outdir)
    L = ctl["linked"]
    checks = []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))
    exe_now = K.sha256_file(K.EXE)

    def receipt(d, commit, packet, directory, n_files):
        """The receipt files under d equal the accepted packet's SHA256SUMS; returns (descriptor, CLASSIFICATION lines,
        raw text, executable sha256 before and after: both descriptor formats, Stage 1's and Stage 2's)."""
        sums = K.blob(commit, packet + "/SHA256SUMS").decode()
        listed = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}
        rel = directory[len(packet) + 1:]
        names = sorted(os.listdir(d))
        bad = [f for f in names if listed.get(f"{rel}/{f}") != K.sha256_file(os.path.join(d, f))]
        chk(f"the linked files ({len(names)}) equal the accepted evidence: {directory} in {commit[:8]}'s SHA256SUMS",
            not bad and len(names) == n_files, bad)
        h = json.load(open(os.path.join(d, "descriptor.json")))
        ex = h["executable"]
        ids = ((ex["before"]["sha256"], ex["after"]["sha256"]) if "before" in ex else (ex.get("sha256_before"),
                                                                                       ex.get("sha256_after")))
        return (h, open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines(),
                open(os.path.join(d, "raw-output.txt"), encoding="utf-8").read(), ids)
    d = os.path.join(K.EV, L["input_dir"])
    commit = L["commit"]
    packet = K.STAGE1_PACKET if commit == K.STAGE1 else K.STAGE2_PACKET
    h, cl, raw, ids = receipt(d, commit, packet, L["directory"], 8)
    chk("its classification blob and raw output identity are the catalog's",
        K.git_blob_id(open(os.path.join(d, "CLASSIFICATION.txt"), "rb").read()) == L["classification_blob"]
        and K.git_blob_id(open(os.path.join(d, "raw-output.txt"), "rb").read()) == L["raw_output_blob"]
        and K.sha256_file(os.path.join(d, "raw-output.txt")) == L["raw_output_sha256"], "")
    kind = "composed-quotient" if L.get("quotient") else "original-explorer"
    chk(f"accepted exact-final unchanged-core {kind} PASS (classification, as the catalog records it)",
        cl[:2] == ["CLASSIFICATION: PASS", L["outer_class"]] and L["outer_class"].endswith(f"unchanged-core {kind} PASS"),
        cl[:2])
    test = h.get("test") or h["argv"][1]
    if L["type"] == "e1" and not L.get("quotient"):
        cfg = L["configuration"]
        chk(f"exact configuration: the same test and flags as this control's run ({cfg})",
            h["argv"] == [K.EXE, *ctl["argv_tail"]] and test == ctl["test"] == L["descriptor_test"], h["argv"])
    elif L["type"] == "e1":
        bounds = [l[len("  bounds: "):] for l in raw.splitlines() if l.startswith("  bounds: ")]
        cfg = ctl["configuration"]
        chk(f"{cfg}'s restored side (AD-1-R4 F): the composed-quotient test with the same flags, the same {cfg} "
            f"configuration (bounds) as this control's original-explorer {ctl['test'].split('::')[-1]} run",
            h["argv"] == [K.EXE, L["descriptor_test"], *FLAGS] and test == L["descriptor_test"]
            and ctl["argv_tail"] == [ctl["test"], *FLAGS] and ctl["test"] != test and bounds == [ctl["bounds"]]
            and [l for l in raw.splitlines() if "EXPLORATION " in l][:1] == [
                f"test {test} ... EXPLORATION {cfg} (gating-caller symmetry quotient composed with the admission "
                "observer quotient: experimental, not the contract's explorer)"],
            f"{h['argv'][1:2]}; mutant run {ctl['test']}")
    else:
        chk("the discharge's basis: the accepted exact-final F-2 receipt, full_f2 with the same flags on the pinned "
            "baseline executable", h["argv"] == [K.EXE, L["descriptor_test"], *FLAGS] and test == K.TEST_PREFIX +
            "full_f2", h["argv"][1:2])
    chk("exact Candidate C: the source exactly C before and after",
        h["source_before"]["verdict"] == "EXACTLY_C" == h["source_after"]["verdict"]
        and h["source_before"]["head"] == K.C and h["source_before"]["tree"] == K.C_TREE, "")
    chk("execution policy: env -i PATH/LANG/RUST_BACKTRACE; 16 GiB; zero swap; 2700 s",
        h["env"] == dict(K.RUN_ENV) and h["supervisor"]["memory_max"] == K.MEMORY_MAX
        and h["supervisor"]["swap_max"] == "0" and h["supervisor"]["time_limit_seconds"] == ctl["limit_seconds"], "")
    chk("artifact identity at this control's restoration: the linked receipt's executable sha256 equals the pinned "
        "baseline executable, hashed now", ids[0] == ids[1] == K.EXE_SHA256 == exe_now, exe_now)
    extra = []
    if L["type"] == "e1":
        ref = (f"{L['directory']} at {commit} (accepted exact-final {L['configuration']} unchanged-core {kind} PASS, "
               f"type (e1))")
    else:
        chk("the F-2 receipt is the PASS the proof rests on: outcome PASS; 12,602,670 states, all expanded; D1 and L1 "
            "HOLD", "\n  outcome: PASS\n" in raw and "\n  states 12602670 (expanded 12602670), transitions 54214081 " in raw
            and "\n  D1: HOLDS\n" in raw and "\n  L1: HOLDS: 50655853 non-fault edges, no cycle; 3206 end states" in raw, "")
        proof = open(K.inputs("FR-STUTTERING-REFINEMENT-PROOF.md"), "rb").read()
        r5 = open(K.inputs("AD1-R5.md"), "rb").read()
        adoption = open(K.inputs("AD1-R5-OWNER-ADOPTION.md"), "rb").read()
        pt, at, rt = proof.decode(), adoption.decode(), r5.decode()
        chk("the accepted F-R proof: sha256 and blob as the catalog and the adoption record give them",
            K.sha256_bytes(proof) == L["proof_sha256"] == K.FR_PROOF_SHA and K.git_blob_id(proof) == K.FR_PROOF_BLOB,
            K.sha256_bytes(proof))
        chk("the proof is bound to the exact frozen baseline: Candidate C's commit and tree, the three blobs, and this "
            "F-2 receipt's raw output", all(x in pt for x in (K.C, K.C_TREE, *K.BLOBS.values(), L["raw_output_sha256"])),
            "")
        chk("AD-1-R5: sha256 as the catalog gives it; its section F (C15) and the recording", K.sha256_bytes(r5) ==
            L["ad1r5_sha256"] == K.AD1R5_SHA and "## F. C15" in rt and f"> {K.FR_RECORDING}" in rt, K.sha256_bytes(r5))
        chk("the Owner adoption of AD-1-R5: sha256 as the catalog gives it; adopted in full; the recording and the "
            "proof's sha256", K.sha256_bytes(adoption) == L["adoption_sha256"] == K.AD1R5_ADOPTION_SHA
            and "On 2026-10-14 the Owner approved and adopted AD-1-R5 in full." in at and f"> {K.FR_RECORDING}" in at
            and K.FR_PROOF_SHA in at, K.sha256_bytes(adoption))
        fd = os.path.join(K.EV, "inputs/linked/S2-69-full-F-R")
        fh, fcl, fraw, fids = receipt(fd, K.STAGE2, K.STAGE2_PACKET, L["fr_receipt"], 8)
        chk("F-R's own receipt stays INCOMPLETE (30,000,001 discovered, 26,539,923 expanded, at the cap): never PASS",
            fcl[0] == "CLASSIFICATION: NOT PASS" and fcl[1].startswith("outer class (AD-1-R4 K.2): INCOMPLETE (the state "
                                                                       "cap of 30000000 was reached")
            and "\n  outcome: INCOMPLETE\n" in fraw and "\n  states 30000001 (expanded 26539923)," in fraw
            and fh["argv"] == [K.EXE, ctl["test"], *FLAGS], fcl[:1])
        ref = (f"the AD-1-R5 F refinement discharge record (not a run; no outcome line; never PASS): Candidate C "
               f"{K.C}; the accepted exact-final F-2 receipt {L['directory']} at {commit}; the accepted proof "
               f"{K.FR_PROOF} (sha256 {K.FR_PROOF_SHA}); AD-1-R5 (sha256 {K.AD1R5_SHA}) and its Owner adoption "
               f"{K.AD1R5_ADOPTION} (sha256 {K.AD1R5_ADOPTION_SHA})")
        extra = [f"recording (AD-1-R5 D): {K.FR_RECORDING}",
                 f"beside it: the F-R receipt {L['fr_receipt']} at {K.STAGE2} stays INCOMPLETE (30,000,001 discovered, "
                 "26,539,923 expanded, at the 30,000,000-state cap); the F-2 receipt above is the run the discharge "
                 "rests on; the branch accounting is FR-BRANCH-DISPOSITION.tsv of the R5 packet"]
    # TRANCHE3-EXEC-01 (mission section 10): no restored-side receipt is linked before this control's own restoration
    # holds: its restoration receipt must exist beside this link and record RESTORE: EXACTLY_C.
    # TRANCHE3-CONTINUE-02 (mission sections 9 and 17): C24 (no patch applied here) links only after its own
    # reauthentication receipt records REAUTH_OK.
    own = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else "05-restore/RESTORE.txt")
    want_own = REAUTH_OK if ctl["id"] == "C24" else RESTORE_OK
    chk(f"this control's own {'reauthentication' if ctl['id'] == 'C24' else 'restoration'} receipt exists and records "
        f"{want_own} (no link before {'reauthentication' if ctl['id'] == 'C24' else 'restoration'})",
        supervise.first_line(own) == want_own, f"{os.path.relpath(own, K.EV)}: {supervise.first_line(own)!r}")
    ok = all(o for _, o, _ in checks)
    # TRANCHE2-CONTINUE-02 (restored-side mappings): the link names, by path and sha256, the control's own restoration
    # receipt (C5: its reauthentication receipt) it follows, so that each link record is bound to its own control.
    rest = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else "05-restore/RESTORE.txt")
    rest_line = (f"after this control's own {'reauthentication' if ctl['id'] == 'C24' else 'restoration'} receipt "
                 f"{os.path.relpath(rest, K.EV)} (sha256 {K.sha256_file(rest)}; first line "
                 f"{supervise.first_line(rest)!r})" if os.path.exists(rest) else
                 f"no restoration receipt beside {os.path.relpath(outdir, K.EV)}: a tooling control of {K.MISSION}")
    lines = [LINK_OK if ok else "LINK: FAILED", f"control {ctl['id']}: restored-side receipt {ref}",
             "linked only after this control's own restoration was verified (receipt linking does not substitute for "
             "restoration evidence)", rest_line] + extra
    lines += [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + dd) if dd else ''}" for l, o, dd in checks]
    text = "\n".join(lines) + "\n"
    record = {"control": ctl["id"], "type": L["type"], "reference": ref, "checks": checks, "follows": rest_line}
    if L["type"] == "fr-discharge":
        record["recording"] = K.FR_RECORDING
        bad = [w for w in PROHIBITED if w in text or w in json.dumps(record, ensure_ascii=False)]
        if bad:
            lines[0] = "LINK: FAILED"
            text = "\n".join(lines + [f"FAIL prohibited F-R wording present: {bad}"]) + "\n"
            ok = False
    open(os.path.join(outdir, "LINK.txt"), "w").write(text)
    json.dump(record, open(os.path.join(outdir, "LINK.json"), "w"), indent=1)
    print(text, end="")
    return ok, ref


def finish(outdir, ctl, status, facts):
    lines = [status, f"control {ctl['id']}: {ctl['description']}", f"receipt type: {ctl['receipt_type']}"] + facts
    open(os.path.join(outdir, "RESULT.txt"), "w").write("\n".join(lines) + "\n")
    json.dump({"control": ctl["id"], "status": status, "facts": facts}, open(os.path.join(outdir, "CONTROL-RESULT.json"),
                                                                             "w"), indent=1)
    print("\n".join(lines))
    return 0 if status == RESULT_OK else 1


def main_c24():
    """Mission sections 8, 9 and 17: C24 from its preserved build, once. No patch, no build, no rebuild."""
    exp = supervise.expectations()
    ctl = exp["controls"]["C24"]
    outdir = os.path.join(K.EV, ctl["dir"])
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {ctl['dir']} exists (one attempt per control; nothing is retried)")
    os.makedirs(outdir)
    problems, lines = gate("C24", exp)
    open(os.path.join(outdir, "00-GATE.txt"), "w").write(
        ("GATE: OPEN" if not problems else "GATE: CLOSED — " + "; ".join(problems)) + "\n" + "\n".join(lines) + "\n")
    if problems:
        return finish(outdir, ctl, RESULT_BLOCKED, [f"not started: {'; '.join(problems)}"])
    facts = []
    wt = ctl["worktree"]
    bdir = os.path.join(K.EV, ctl["preserved"]["build_receipt"]["input_dir"])
    rec, ok = c24.admit(bdir, ctl)
    c24.write(os.path.join(outdir, "01-admission"), "ADMISSION", c24.ADMIT_OK, c24.ADMIT_FAIL,
              f"control C24: the preserved Tranche-3 build ({ctl['preserved']['build_receipt']['directory']} at "
              f"{K.T3X}), admitted under the Architect's narrow C24-specific warning ruling by the dedicated checker; not "
              f"rebuilt; the historical result BUILD: NOT OK is unchanged", rec, ok)
    facts.append(f"build admission: {supervise.first_line(os.path.join(outdir, '01-admission/ADMISSION.txt'))}")
    if not ok:
        facts.append("STOP: the preserved build is not admissible under the C24 ruling (kills nothing; no rebuild). No "
                     "later control is executed.")
        return finish(outdir, ctl, RESULT_BLOCKED, facts)
    rec, ok = c24.worktree_check(ctl, "before the run", restored_check)
    c24.write(os.path.join(outdir, "02-worktree"), "AUTHENTICATE-BEFORE", c24.WT_OK, c24.WT_FAIL,
              f"control C24: the preserved worktree {wt}, read-only, before the run (not created, patched or restored by "
              f"this mission)", rec, ok)
    facts.append(f"preserved worktree before the run: "
                 f"{supervise.first_line(os.path.join(outdir, '02-worktree/AUTHENTICATE-BEFORE.txt'))}")
    prec, pok = c24.pin(ctl, bdir)
    c24.write(os.path.join(outdir, "03-pin"), "PIN", c24.PIN_OK, c24.PIN_FAIL,
              f"control C24: the preserved executable {ctl['preserved']['executable']['path']}, pinned read-only", prec,
              pok)
    facts.append(f"preserved executable: {supervise.first_line(os.path.join(outdir, '03-pin/PIN.txt'))}")
    if not (ok and pok):
        facts.append("STOP: C24_PRESERVED_ARTIFACT_BLOCKED (the preserved worktree or executable is missing or differs; "
                     "nothing is rebuilt). No later control is executed.")
        return finish(outdir, ctl, RESULT_ARTIFACT_BLOCKED, facts)
    exe = {"path": prec["path"], "sha256": prec["sha256"], "size": prec["size"]}
    facts.append(f"pinned mutant executable: {exe['path']} sha256 {exe['sha256']} size {exe['size']}")

    def source_ok(rec2, when):
        os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # read-only: the preserved worktree's index is never refreshed
        try:
            d = restored_check(wt, when)
        finally:
            del os.environ["GIT_OPTIONAL_LOCKS"]
        rec2.append(d["line"])
        return d, d["verdict"] == "EXACTLY_C"
    rdir = os.path.join(outdir, "04-run")
    supervise.invoke(rdir, f"SUPERVISED MUTANT RUN — control C24: {ctl['test']} on the preserved, pinned C24 mutant "
                           "executable (type (a)); the worktree exactly C (restored in Tranche 3)",
                     exe["path"], exe["sha256"], exe["size"], os.path.join(wt, K.CRATE), wt, ctl["test"], ctl["ignored"],
                     ctl["argv_tail"], ctl["limit_seconds"], source_ok, "mutant-explore",
                     [(ctl["dir"] + "/01-admission/ADMISSION.txt", c24.ADMIT_OK),
                      (ctl["dir"] + "/02-worktree/AUTHENTICATE-BEFORE.txt", c24.WT_OK),
                      (ctl["dir"] + "/03-pin/PIN.txt", c24.PIN_OK)],
                     {"control": "C24", "configuration": ctl["configuration"], "mutant_blob": ctl["mutant_blob"],
                      "patch_sha256": ctl["patch"]["sha256"],
                      "executable_origin": f"the preserved Tranche-3 C24 build {ctl['preserved']['build_receipt']['directory']}"
                                           f" at {K.T3X}, admitted under the narrow C24-specific warning ruling; not rebuilt",
                      "source_state": "EXACTLY_C (the worktree was restored in Tranche 3; the mutant identity is bound "
                                      "through the admitted build record and the pinned executable)"})
    classify.classify_a(rdir, dict(ctl, mutant_blob=None), exe, source_verdict=ctl["preserved"]["run_source_verdict"])
    cls = open(os.path.join(rdir, "CLASSIFICATION.txt")).read().splitlines()
    candidate = cls[0] == classify.CANDIDATE
    facts.append(f"run (type (a)): {cls[0]}; {cls[1]}")
    rules_ok = t3.rules(os.path.join(outdir, "04a-rules"), rdir, ctl)
    facts.append(f"tranche-3 rules: {supervise.first_line(os.path.join(outdir, '04a-rules/RULES.txt'))}")
    candidate = candidate and rules_ok
    stop = None if candidate else "the mutant run is not admissible kill evidence of the named property or test"
    wrec, wok = c24.worktree_check(ctl, "after the run", restored_check)
    prec2, pok2 = c24.pin(ctl, bdir)
    rrec = {"worktree": wrec, "executable": prec2,
            "checks": [("worktree: " + l, o, d) for l, o, d in wrec["checks"]]
            + [("executable and target: " + l, o, d) for l, o, d in prec2["checks"]]}
    c24.write(os.path.join(outdir, "05-reauth"), "REAUTH", REAUTH_OK, "C24 REAUTHENTICATION AFTER THE RUN: FAILED",
              "control C24: the preserved worktree and executable after the run (read-only; no patch was applied, so "
              "nothing is restored)", rrec, wok and pok2)
    facts.append(f"after the run: {supervise.first_line(os.path.join(outdir, '05-reauth/REAUTH.txt'))}")
    if not (wok and pok2):
        stop = (stop + "; " if stop else "") + "the reauthentication after the run failed"
    if candidate and wok and pok2:
        lok, ref = link(ctl, exp, os.path.join(outdir, "06-link"), None)
        facts.append(f"linked restored-side receipt: {supervise.first_line(os.path.join(outdir, '06-link/LINK.txt'))}; "
                     f"{ref}")
        if not lok:
            stop = "the restored-side receipt could not be linked"
    if stop:
        facts.append(f"STOP: {stop}. No later control is executed.")
    return finish(outdir, ctl, RESULT_OK if not stop and candidate else RESULT_BLOCKED, facts)


def main(cid):
    if cid in K.ACCEPTED or cid not in K.ORDER:
        raise SystemExit(f"REFUSED: {cid} is not executed by {K.MISSION} ({' '.join(K.ACCEPTED)} are "
                         f"Architect-accepted; only {' '.join(K.ORDER)} run)")
    if cid == "C24":
        return main_c24()
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
            # TRANCHE3-CONTINUE-02 (mission sections 3-5, AMENDMENT-01): C3b and C3c only: the dedicated admission
            # checker decides (a clean build, or exactly the one predicted warning group) and names the pin record.
            admitted, pin_file = first == "BUILD: PINNED", os.path.join(bdir, "PIN.json")
            bprior = [(ctl["dir"] + "/03-build/CLASSIFICATION.txt", "BUILD: PINNED")]
            if cid in c3w.CONTROLS:
                admitted, pin_file, afirst = c3w.admit(bdir, ctl, os.path.join(outdir, "03a-admission"))
                facts.append(f"build admission (C3b/C3c rule, AMENDMENT-01): {afirst}")
                bprior = [(ctl["dir"] + "/03a-admission/ADMISSION.txt", afirst)]
            if not admitted:
                stop = ("the mutant build is not admissible under its build rule (kills nothing; no rebuild)"
                        if cid in c3w.CONTROLS else "the mutant build did not succeed (kills nothing; no rebuild)")
            else:
                pin = json.load(open(pin_file))
                facts.append(f"pinned mutant executable: {pin['path']} sha256 {pin['sha256']} size {pin['size']}")
                exe = {"path": pin["path"], "sha256": pin["sha256"], "size": pin["size"]}
                rdir = os.path.join(outdir, "04-run")
                title = (f"SUPERVISED MUTANT RUN — control {cid}: {ctl['test']} on the pinned mutant executable "
                         f"(type ({'a' if ctl['kind'] == 'explore' else 'b'}))")
                supervise.invoke(rdir, title, pin["path"], pin["sha256"], pin["size"], os.path.join(wt, K.CRATE), wt,
                                 ctl["test"], ctl["ignored"], ctl["argv_tail"], ctl["limit_seconds"], source_ok,
                                 "mutant-" + ctl["kind"], bprior,
                                 {"control": cid, "configuration": ctl.get("configuration", "named unit test"),
                                  "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"]})
                (classify.classify_a if ctl["kind"] == "explore" else classify.classify_b)(rdir, ctl, exe)
                cls = open(os.path.join(rdir, "CLASSIFICATION.txt")).read().splitlines()
                candidate = cls[0] == classify.CANDIDATE
                facts.append(f"run (type ({'a' if ctl['kind'] == 'explore' else 'b'})): {cls[0]}; {cls[1]}")
                # TRANCHE3-EXEC-01: the tranche-3 rules (t3.rules) must hold too; they only add requirements.
                rules_ok = t3.rules(os.path.join(outdir, "04a-rules"), rdir, ctl)
                facts.append(f"tranche-3 rules: {supervise.first_line(os.path.join(outdir, '04a-rules/RULES.txt'))}")
                candidate = candidate and rules_ok
                if not candidate:
                    stop = "the mutant run is not admissible kill evidence of the named property or test"
                # TRANCHE3-EXEC-01 (mission section 5): C7 only, after an admissible F-PT/P6 candidate and before
                # restoration: the informational run without A4 on F-3, on the same pinned executable; never a kill.
                if candidate and ctl.get("informational"):
                    info = ctl["informational"]
                    idir = os.path.join(outdir, info["dir"])
                    supervise.invoke(idir, f"SUPERVISED MUTANT RUN — control {cid}: {info['test']} on the same pinned "
                                           "mutant executable, WITHOUT A4 on F-3 (informational, mission section 5; not "
                                           "the kill receipt)", pin["path"], pin["sha256"], pin["size"],
                                     os.path.join(wt, K.CRATE), wt, info["test"], info["ignored"], info["argv_tail"],
                                     info["limit_seconds"], source_ok, "mutant-informational",
                                     [(ctl["dir"] + "/04a-rules/RULES.txt", t3.RULES_OK)],
                                     {"control": cid, "configuration": info["configuration"],
                                      "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"],
                                      "informational": True, "required_to_kill": False})
                    iok, ifirst = t3.classify_info(idir, info, exe, "EXACTLY_C_PLUS_PATCH", ctl["mutant_blob"])
                    facts.append(f"informational run without A4 on F-3 (not the kill receipt): {ifirst}")
                    if not iok:
                        stop = ("the C7 informational run is not admissible (infrastructure, identity, OOM, signal, "
                                "overshoot or malformed receipt): the mission stops")
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
