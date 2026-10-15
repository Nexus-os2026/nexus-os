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
    "not_authorized": K.NOT_AUTHORIZED,
    "controls": "each once, in order; a stop on the first unsuccessful control. C24: no worktree created, no patch, no "
                "build: the preserved Tranche-3 build admitted by c24.admit (exactly the one Corrected warning group; "
                "the historical BUILD: NOT OK unchanged), the preserved worktree authenticated read-only (exactly C, "
                "locked with its owner's reason), the preserved executable pinned read-only (sha256 8a81956b..., "
                "8,987,680 bytes, the target unmodified since the build), one original-explorer run of full_f1 "
                "(classify_a with source EXACTLY_C; t3.rules), the worktree and executable reauthenticated, then the "
                "restored side linked. Every other control: a fresh mission-owned worktree at C; the frozen catalog patch "
                "(exact mutant blob and sha256, only the catalog's file changed); one build, classified by the unchanged "
                "general classifier; C3b/C3c: then c3w.admit (a clean build, or exactly the one predicted warning group: "
                "Future for C3b, Set for C3c; permitted, not required; AMENDMENT-01); C6 C7 A1 A2 C20 A5: BUILD: PINNED "
                "only (zero warnings, zero errors); one original-explorer run of the catalog's test on the pinned mutant "
                "executable, classified by the unchanged classify_a, then the tranche-3 rules (t3.rules); C7 only: after "
                "an admissible F-PT/P6 candidate and before restoration, one informational run without A4 on F-3 "
                "(full_f3, same executable; t3.classify_info; never the kill receipt); restoration by git apply -R, "
                "verified byte for byte; then the restored side linked",
    "build_rules": {"C24": "preserved build; narrow C24 warning ruling (mission section 2); no rebuild",
                    "C3b": "a clean build, or exactly `warning: variant `Future` is never constructed` at "
                           "protocol.rs:635:5 (AMENDMENT-01)",
                    "C3c": "a clean build, or exactly `warning: variant `Set` is never constructed` at protocol.rs:636:5 "
                           "(AMENDMENT-01)",
                    "C6 C7 A1 A2 C20 A5": "zero warnings, zero errors (the unchanged general classifier; no exception)"},
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"builds": K.BUILD_LIMIT, "every run": 2700},
    "ceilings_states": {c: K.MATRIX[c][2] for c in K.ORDER},
    "permitted_properties": {c: K.MATRIX[c][1] for c in K.ORDER},
    "a4_on": K.A4_ON,
    "l1_complete_search": K.L1_CONTROLS,
    "c7_informational": K.C7_INFO,
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "git apply -R of the same frozen patch in the disposable worktree; the changed file byte-equal to C; "
                   "verify_tree.py EXACTLY_C (all tracked blobs and modes); the path-aware status policy "
                   "(verify_mutant.status_policy): no tracked change, ignored entries only beneath the worktree's own "
                   "real target directory. C24: nothing applied; the preserved worktree and executable reauthenticated",
    "restored_side": "linked only after the control's own verified restoration (C24: its reauthentication) (link() "
                     "refuses otherwise), never rerun: Stage-1 45-full-F-1 (C24 A1 A2), Stage-2 66-full-F-6 (C3b C3c), "
                     "Stage-1 44-full-F-PT composed-quotient PASS (C6 C7; restored side only), Stage-1 41-full-F-2 "
                     "(C20 A5)",
    "classifiers": "classify.py byte-identical to the accepted tooling's: classify_build_success (pin; zero warnings, "
                   "zero errors; no exception) and classify_a for every kill run; c24.admit (C24 only) and c3w.admit (C3b "
                   "and C3c only) as the dedicated control-specific admission checkers; t3.rules (adds requirements "
                   "only); t3.classify_info for C7's informational run (never a kill label)",
    "packet_tooling": "render.py, build_packet.py, publish_checks.py, worktree.py, finalize.py, push.py, final_gate.py "
                      "(frozen with every other tool)",
    "retries": "none: C24 never rebuilt; one build per other control, one kill run per control (C7: plus its one "
               "informational run), no rebuild, no rerun, no cap increase, no alternate patch, no quotient for a mutant; "
               "the accepted controls and the unchanged-core configurations are never rerun",
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
                     "SHA256SUMS_blob": K.git("rev-parse", f"{K.T3X}:{K.T3X_PACKET}/SHA256SUMS").decode().strip()}}
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
    out["c24_preserved"] = {"executable": {"path": K.C24_EXE, "sha256": K.sha256_file(K.C24_EXE),
                                           "size": os.path.getsize(K.C24_EXE)},
                            "dep_info_sha256": K.sha256_file(K.C24_EXE + ".d"),
                            "worktree_head": K.git("rev-parse", "HEAD", repo=K.C24_WT).decode().strip(),
                            "build_stderr_sha256": K.sha256_file(K.inputs("c24", "03-build", "build.stderr"))}
    for cid in K.ORDER:
        out["controls"][cid]["build_rule"] = exp["controls"][cid]["build_rule"]
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
