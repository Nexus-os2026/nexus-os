#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once. Revised by -RESUME-01 and
-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 7): recorded once, before C23. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (section 16): recorded once, before C5; it also freezes the Tranche-2
base tooling (original-tools/), the C5 build evidence (inputs/c5/), the C5 expected executable, the exact C5 warning
set (tools/c5.py), all five relevant frozen patches, the order, commands, classifiers, ceilings, restoration and proof
links and the packet tooling, and records the identities of the previous packet and the preserved C5 artifacts.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (section 11): recorded once, before C2. It freezes every tool
(tools/, including t3.py: the C3b alternative-property rule, C6/C7 A4 ON, the L1 complete-search rule and the C7
informational classifier), the base tooling (original-tools/: the accepted continuation's execution tooling), every
authenticated input (the catalog, the ten frozen patches, the linked unchanged-core receipts, the contract, AD-1-R4/R5,
the adoption record and proof), the expectations (the ten controls, their order, selections, permitted properties,
ceilings, limits, A4 and L1 rules, C7's informational run, restoration and link mappings) and this mission's two
records; it records the identities of Candidate C, the catalog, every previous evidence commit (Tranche 1, Tranche 2,
the continuation), the linked evidence and the pinned baseline executable. No C5 artifact is involved here.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 16): recorded once, before C24's test invocation. It
also freezes the C24 admission and pin tooling (tools/c24.py) with the preserved C24 build evidence (inputs/c24/), the
C3b/C3c build rule of AMENDMENT-01 (tools/c3w.py), and this mission's four records (the C2 acceptance, the build
rulings, AMENDMENT-01, the campaign status); it records the identities of the Tranche-3 stopped evidence (774f457a) and
the preserved C24 artifacts (executable sha256 and size, dep-info, worktree HEAD).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (section 5, step 15): recorded once, before A1's build. It freezes
every tool (tools/; c24.py and c3w.py are not carried), the base tooling (original-tools/: the Tranche-3 continuation's
execution tooling), every authenticated input (the catalog, the four frozen patches, the linked Stage-1 F-1 and F-2
receipts, the contract, AD-1-R4/R5, the adoption record and proof), the expectations (the four controls, their order,
selections, permitted properties, ceilings, limits, the L1 complete-search rule for C20 and A5, restoration and link
mappings) and this mission's two records (the continuation acceptance ruling with C7's block; the 27-of-35 campaign
status); it records the identities of the Tranche-3 continuation evidence (c10681b3) besides every earlier one. No
preserved artifact is involved here (C24's preserved-artifact identity is no longer recorded: C24 is accepted).

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (section 3, step 8): recorded once, before C1's build. It
freezes every tool (tools/, g3.py added: the G.3 primary and witness classifiers, the healthy-witness link and the G.3
obligations), the base tooling (original-tools/: the Tranche-3 tail's execution tooling), every authenticated input (the
catalog, the two frozen patches, the linked Stage-1 F-1 receipt, the two accepted Stage-2 healthy witness receipts, the
contract, AD-1-R4/R5, the adoption record and proof), the expectations (the two controls, their order, selections, the
G.3 blocks: predicted first failures, witness tests, positive lines and loci, schedules, panics, healthy receipts;
ceilings, limits, restoration and link mappings) and this mission's two records (the tail acceptance ruling; the
31-of-35 campaign status); it records the identities of the Tranche-3 tail evidence (e41c84b1) besides every earlier one.

Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01 (section 5, steps 11-12): recorded once, before C4's build. It
freezes every tool (tools/, g5.py added: the G.5(A) primary and G.5(B)/G.6 witness classifiers, the G.6 transcript
parser, the healthy-witness link with G.6 and the healthy-versus-mutant comparison, the G.5 obligations; g3.py not
carried), the base tooling (original-tools/: the C1/C25 mission's execution tooling), every authenticated input (the
catalog, the C4 patch, the linked Stage-2 F-8 receipt and the accepted Stage-2 C4 healthy witness receipt, the contract,
AD-1-R4/R5, the adoption record and proof), the expectations (C4: its selection, F-8 ceiling and limits, the G.5 block:
admissible first properties, the witness test, both scenarios' frozen steps and changed appends, positive lines and
loci, panics, healthy lines; restoration and link mappings) and this mission's two records (the C1/C25 acceptance
ruling; the 33-of-35 campaign status); it records the identities of the C1/C25 witness evidence (74b3f485) besides every
earlier one.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists or a pre-freeze receipt does not
                            record success

Frozen files: every tool (tools/), the base tools (original-tools/), every authenticated input (inputs/), the
expectations (receipts/02-EXPECTATIONS.json) and this mission's two records (the Architect ruling record and the
campaign status). supervise.py and control.py re-check every one of them before every supervised step; the gates check
them again.

Recorded identities (each re-checked by the pre-execution gate): Candidate C; the catalog; the accepted Tranche-1
evidence commit and its three packets' manifests; the Tranche-2 stopped evidence commit and the accepted continuation
evidence commit with their packet manifests; the Stage-1, Stage-2 and R5 evidence; every linked receipt; the pinned
baseline executable; each control's patch, mutant identity, test, configuration, permitted properties, ceiling, A4 and
L1 rules, C7's informational run and restored side; the commands, environments, resource policy, restoration rule and
packet/publication tooling.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

POLICY = {
    "control_order": K.ORDER,
    "architect_accepted_not_executed": K.ACCEPTED,
    "blocked_not_executed": K.BLOCKED,
    "not_authorized": K.NOT_AUTHORIZED,
    "controls": "C4 alone, once. A fresh mission-owned worktree at C (mwc4 namespace); the frozen catalog patch (exact "
                "mutant blob and sha256, only protocol.rs changed: Test 1's sixteen lines removed); one build, "
                "classified by the unchanged general classifier (BUILD: PINNED only: zero warnings, zero errors; no "
                "admission exception); the type-(a) primary: full_f8 on the original explorer on the pinned mutant "
                "executable (F-8, 30,000,000 states, 2700 s), classified by g5.classify_primary (AD-1-R4 G.5(A): the "
                "first property recorded as printed, never relabelled; P4 or P8 admissible; any other first property "
                "stops for an Architect ruling; PASS, INCOMPLETE or a FAIL beyond the ceiling kills nothing); then the "
                "type-(c) mutant witness on the SAME pinned executable, the witness test alone with --exact, classified "
                "by g5.classify_witness (G.5(B): both exact positive lines at their frozen loci, the header, the two "
                "separate lines of other properties, its own verdict panic, 1 failed, exit 101; G.6 verified from the "
                "raw output at every step of both scenarios); restoration by git apply -R, verified byte for byte, and "
                "the post-restoration authentication (ownership, lock, the pinned executable unchanged); then, only "
                "after RESTORE: EXACTLY_C, the accepted healthy type-(e3) witness (Stage 2, with G.6 on it and against "
                "the mutant) and the accepted F-8 type-(e1) receipt (Stage 2) linked, never rerun; then the G.5 "
                "obligations record ((E)'s Architect review never claimed; its (F) recording proposed for Architect "
                "review only). C7 (MUTATION_CONTROL_BLOCKED) and every accepted control: never executed "
                "(control.main refuses each by name; no expectation exists for any of them)",
    "build_rules": {"C4": "zero warnings, zero errors (the unchanged general classifier; no exception)"},
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"builds": K.BUILD_LIMIT, "every run (primary and witness)": 2700},
    "ceilings_states": {c: K.MATRIX[c][2] for c in K.ORDER},
    "permitted_properties": {c: K.MATRIX[c][1] for c in K.ORDER},
    "g5": {c: {"admissible_first_properties": K.G5[c]["permitted_primary"], "named_properties": K.G5[c]["named"],
               "witness_test": K.TEST_PREFIX + K.G5[c]["witness_test"],
               "positive_lines": {n: s["positive"] for n, s in K.G5[c]["scenarios"].items()},
               "changed_steps": {n: {"step": s["steps"], "offered": s["offered"], "mutant_consumed": s["mutant_consumed"],
                                     "mutant_unused": s["mutant_unused"], "healthy_consumed": s["healthy_consumed"],
                                     "healthy_unused": s["healthy_unused"]} for n, s in K.G5[c]["scenarios"].items()},
               "healthy_receipt": K.STAGE2_PACKET + "/" + K.G5[c]["healthy_dir"], "recording_template_proposed":
                   K.G5[c]["recording_template"]} for c in K.ORDER},
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "git apply -R of the same frozen patch in the disposable worktree; the changed file byte-equal to C; "
                   "verify_tree.py EXACTLY_C (all tracked blobs and modes); the path-aware status policy "
                   "(verify_mutant.status_policy): no tracked change, ignored entries only beneath the worktree's own "
                   "real target directory",
    "restored_side": "linked only after the control's own verified restoration (g5.link_healthy and link() refuse "
                     "otherwise), never rerun: Stage-2 52-witness-C4 (type (e3)) and Stage-2 64-full-F-8 (type (e1))",
    "classifiers": "classify.py and t3.py byte-identical to the accepted tooling's (classify_build_success: pin; zero "
                   "warnings, zero errors; no exception); g5.py (added; g3.py not carried): classify_primary (every "
                   "check g3's primary carried; G.5(A)'s attribution), classify_witness (type (c): G.5(B), F.2; G.6 "
                   "from the raw output), link_healthy (type (e3): G.5(C); G.6 on it and against the mutant), "
                   "obligations (G.5 (A)-(E); (F) proposed); no control-specific admission checker",
    "results": "KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING; MUTATION_CONTROL_BLOCKED; INCOMPLETE — ARCHITECT "
               "REVIEW REQUIRED (classify_a: INCOMPLETE; kills nothing); STOPPED BEFORE EXECUTION (gate refused)",
    "packet_tooling": "render.py, build_packet.py, publish_checks.py, worktree.py, finalize.py, push.py, final_gate.py "
                      "(frozen with every other tool)",
    "retries": "none: one build per control, one primary and one witness run per control, no rebuild, no rerun, no "
               "cap increase, no alternate patch, no quotient or symmetry reduction for a mutant, no instrumentation; "
               "the accepted controls, C7, the accepted healthy witness and the unchanged-core configurations are "
               "never run",
}
PRE_FREEZE = {"receipts/00-authentication.txt": "RESULT: ALL CHECKS HOLD",
              "receipts/01-supervision-selftest/SELFTEST.txt": "SELF-TEST RESULT: ALL OK",
              "receipts/02-EXPECTATIONS.console.txt": "EXPECTATIONS: DERIVED — ALL CROSS-CHECKS HOLD",
              "receipts/03-t3-controls/SUMMARY.txt": "T3 CONTROLS: ALL AS EXPECTED",
              "receipts/04-tool-diff/TOOL-DIFF-CHECK.txt": "TOOL DIFF: ALL CHECKS HOLD",
              "receipts/04b-rehearsals/SUMMARY.txt": "REHEARSALS: ALL AS EXPECTED"}


def identities():
    """The identities the freeze records (mission section 7), each re-checked by the pre-execution gate."""
    out = {"candidate_c": {"commit": K.C, "tree": K.git("rev-parse", K.C + "^{tree}").decode().strip(), "blobs": {
        p: K.git("rev-parse", f"{K.C}:{p}").decode().strip() for p in K.BLOBS}},
        "catalog": {"path": K.CATALOG_PATH, "blob": K.git("rev-parse", f"{K.R2}:{K.CATALOG_PATH}").decode().strip(),
                    "sha256": K.sha256_bytes(K.blob(K.R2, K.CATALOG_PATH)), "commit": K.R2},
        "tranche1": {"commit": K.T1, "tree": K.git("rev-parse", K.T1 + "^{tree}").decode().strip()},
        "tranche2": {"commit": K.T2, "tree": K.git("rev-parse", K.T2 + "^{tree}").decode().strip(),
                     "SHA256SUMS_blob": K.git("rev-parse", f"{K.T2}:{K.T2_PACKET}/SHA256SUMS").decode().strip()},
        "tranche2_continuation": {"commit": K.T2C, "tree": K.git("rev-parse", K.T2C + "^{tree}").decode().strip(),
                                  "SHA256SUMS_blob": K.git("rev-parse", f"{K.T2C}:{K.T2C_PACKET}/SHA256SUMS")
                                  .decode().strip()},
        "tranche3": {"commit": K.T3X, "tree": K.git("rev-parse", K.T3X + "^{tree}").decode().strip(),
                     "SHA256SUMS_blob": K.git("rev-parse", f"{K.T3X}:{K.T3X_PACKET}/SHA256SUMS").decode().strip()},
        "tranche3_continuation": {"commit": K.T3C, "tree": K.git("rev-parse", K.T3C + "^{tree}").decode().strip(),
                                  "SHA256SUMS_blob": K.git("rev-parse", f"{K.T3C}:{K.T3C_PACKET}/SHA256SUMS")
                                  .decode().strip()},
        "tranche3_tail": {"commit": K.T3T, "tree": K.git("rev-parse", K.T3T + "^{tree}").decode().strip(),
                          "SHA256SUMS_blob": K.git("rev-parse", f"{K.T3T}:{K.T3T_PACKET}/SHA256SUMS").decode().strip()},
        "witness_c1_c25": {"commit": K.W1, "tree": K.git("rev-parse", K.W1 + "^{tree}").decode().strip(),
                           "SHA256SUMS_blob": K.git("rev-parse", f"{K.W1}:{K.W1_PACKET}/SHA256SUMS").decode().strip()}}
    for name, packet in K.T1_PACKETS:
        b = K.git("rev-parse", f"{K.T1}:{packet}/SHA256SUMS").decode().strip()
        out["tranche1"][name] = {"packet": packet, "SHA256SUMS_blob": b,
                                 "SHA256SUMS_sha256": K.sha256_bytes(K.git("cat-file", "blob", b))}
    for name, commit in (("stage1", K.STAGE1), ("stage2", K.STAGE2), ("r5", K.R5_COMMIT)):
        out[name] = {"commit": commit, "tree": K.git("rev-parse", commit + "^{tree}").decode().strip()}
    out["fr_discharge"] = {"proof": K.sha256_bytes(K.blob(K.R5_COMMIT, K.FR_PROOF)),
                           "ad1r5": K.sha256_bytes(K.blob(K.R5_COMMIT, K.AD1R5)),
                           "adoption": K.sha256_bytes(K.blob(K.AD1R5_ADOPTION_COMMIT, K.AD1R5_ADOPTION))}
    exp = json.load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    out["controls"] = {}
    for cid in K.ORDER:
        c = exp["controls"][cid]
        L = c["linked"]
        out["controls"][cid] = {
            "dir": c["dir"], "worktree": c["worktree"], "patch_sha256": c["patch"]["sha256"],
            "patch_file_sha256": K.sha256_file(K.inputs("patches", cid + ".patch")), "source_file": c["source_file"],
            "mutant_blob": c["mutant_blob"], "mutant_sha256": c["mutant_sha256"], "test": c["test"],
            "configuration": c["configuration"], "argv_tail": c["argv_tail"],
            "permitted": c.get("permitted_properties", [c["named_property"]]), "ceiling_states": c["ceiling_states"],
            "limit_seconds": c["limit_seconds"], "verdict_assertion": c["verdict_assertion"],
            "a4": c.get("a4"), "l1_rule": bool(c.get("l1_rule")), "informational": c.get("informational"),
            "linked": {"type": L["type"], "commit": L["commit"], "directory": L["directory"],
                       "classification_blob": L["classification_blob"], "raw_output_sha256": L["raw_output_sha256"]}}
    out["baseline_executable"] = {"path": K.EXE, "sha256": K.sha256_file(K.EXE), "size": os.path.getsize(K.EXE)}
    for cid in K.ORDER:
        out["controls"][cid]["build_rule"] = exp["controls"][cid]["build_rule"]
        g = exp["controls"][cid]["g5"]
        out["controls"][cid]["g5"] = {
            "permitted_primary": g["permitted_primary"], "named": g["named"], "witness": g["witness"],
            "healthy": dict(g["healthy"], files={f: K.sha256_file(os.path.join(K.EV, g["healthy"]["input_dir"], f))
                                                 for f in sorted(os.listdir(os.path.join(K.EV, g["healthy"]["input_dir"])))})}
    return out


def main():
    fj = os.path.join(K.EV, "receipts/05-FREEZE.json")
    ft = os.path.join(K.EV, "receipts/05-FREEZE-RECORD.txt")
    if os.path.exists(fj) or os.path.exists(ft):
        raise SystemExit("REFUSED: the freeze is already recorded")
    prior = {}
    for rel, want in PRE_FREEZE.items():
        p = os.path.join(K.EV, rel)
        last = open(p).read().rstrip().splitlines()[-1] if os.path.exists(p) else None
        if last != want:
            raise SystemExit(f"REFUSED: {rel} does not record success ({last!r})")
        prior[rel] = K.sha256_file(p)
    files = {}
    for sub in ("tools", "original-tools", "inputs"):
        for d, dirs, fs in os.walk(os.path.join(K.EV, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                if f.endswith(".pyc"):
                    raise SystemExit(f"REFUSED: bytecode under {sub}/ ({f}); run with python3 -B")
                p = os.path.join(d, f)
                files[os.path.relpath(p, K.EV)] = K.sha256_file(p)
    for rel in ["receipts/02-EXPECTATIONS.json"] + K.RECORDS:
        files[rel] = K.sha256_file(os.path.join(K.EV, rel))
    rec = {"mission": K.MISSION, "recorded": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "files": files,
           "pre_freeze_receipts": prior, "policy": POLICY, "identities": identities(),
           "statement": "After this point: no patch change, no checker or classifier change, no semantic tool repair, "
                        "no automatic retry. A frozen-tool failure stops the mission and preserves its raw evidence."}
    json.dump(rec, open(fj, "w"), indent=1)
    lines = ["FREEZE: RECORDED", f"recorded: {rec['recorded']}", f"frozen files: {len(files)}",
             f"05-FREEZE.json sha256 {K.sha256_file(fj)}"]
    lines += [f"  {h}  {p}" for p, h in sorted(files.items())]
    lines += ["pre-freeze receipts (success recorded):"] + [f"  {h}  {p}" for p, h in prior.items()]
    lines += ["policy:"] + [f"  {k}: {json.dumps(v, ensure_ascii=False)}" for k, v in POLICY.items()]
    lines += ["identities:"] + [f"  {k}: {json.dumps(v)}" for k, v in rec["identities"].items()]
    lines.append(rec["statement"])
    open(ft, "w").write("\n".join(lines) + "\n")
    print("\n".join(lines[:4]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
