#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once. Revised by -RESUME-01 and
-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 7): recorded once, before C23. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (section 16): recorded once, before C5; it also freezes the Tranche-2
base tooling (original-tools/), the C5 build evidence (inputs/c5/), the C5 expected executable, the exact C5 warning
set (tools/c5.py), all five relevant frozen patches, the order, commands, classifiers, ceilings, restoration and proof
links and the packet tooling, and records the identities of the previous packet and the preserved C5 artifacts.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists or a pre-freeze receipt does not
                            record success

Frozen files: every tool (tools/), the Tranche-2 base tools (original-tools/), every authenticated input (inputs/: the
catalog, the definitions record, the five patches, the contract, AD-1-R4, AD-1-R5, its Owner adoption record, the
accepted F-R proof, every linked unchanged-core receipt, C5's preserved Tranche-2 receipts), the expectations
(receipts/02-EXPECTATIONS.json: patch identities, expected mutant blobs, control order, test and configuration
selections, named-property alternatives, A6's key fields, ceilings, limits, restoration and link mappings, C5's
preserved build and executable) and the three records of this mission (the two Architect ruling records, the campaign
status). supervise.py and control.py
re-check every one of them before every supervised step; the gates check them again.

Recorded identities (each re-checked by the pre-execution gate): Candidate C; the catalog; the accepted Tranche-1
evidence commit and its three packets' manifests; the Tranche-2 stopped evidence commit and its packet manifest; the
Stage-1, Stage-2 and R5 evidence; every linked receipt; the pinned baseline executable; the preserved C5 executable
(sha256, size), its dep-info and the C5 worktree's HEAD; each control's patch, mutant identity, test, configuration,
permitted properties, ceiling and restored side; the commands, environments, resource policy, restoration rule and
packet/publication tooling.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c5  # noqa: E402
import common as K  # noqa: E402

POLICY = {
    "control_order": K.ORDER,
    "architect_accepted_not_executed": K.ACCEPTED,
    "not_authorized": K.NOT_AUTHORIZED,
    "controls": "each once, in order; a stop on the first unsuccessful control. C5: no patch, no build, no rebuild: the "
                "preserved Tranche-2 build admitted by tools/c5.py (exactly the five permitted warning groups, C5 only), "
                "the preserved worktree authenticated read-only, the preserved executable pinned read-only, one "
                "original-explorer full_f5 run, the worktree and executable reauthenticated, then the restored side "
                "linked. A3, A6, C8, C15: a fresh mission-owned worktree at C; the frozen catalog patch (exact mutant "
                "blob and sha256, only the catalog's file changed); one build, with no error and no warning diagnostic "
                "(the C5 ruling does not apply); one original-explorer run of the catalog's test on the pinned mutant "
                "executable; restoration by git apply -R, verified byte for byte; then the restored side linked",
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"builds": K.BUILD_LIMIT, "every run": 2700},
    "ceilings_states": {c: K.MATRIX[c][2] for c in K.ORDER},
    "permitted_properties": {c: K.MATRIX[c][1] for c in K.ORDER},
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "A3/A6/C8/C15: git apply -R of the same frozen patch in the disposable worktree; the changed file "
                   "byte-equal to C; verify_tree.py EXACTLY_C; the path-aware status policy (verify_mutant.status_policy): "
                   "no tracked change, ignored entries only beneath the worktree's own real target directory. C5: no "
                   "patch applied here; the preserved worktree (restored in Tranche 2) reauthenticated read-only after "
                   "the run",
    "restored_side": "linked only after the control's own verified restoration, never rerun: the accepted exact-final "
                     "Stage-1/Stage-2 unchanged-core receipt the catalog names; C8: the Stage-1 composed-quotient F-B "
                     "PASS (restored side only); C15: the AD-1-R5 F refinement discharge record (never PASS)",
    "classifiers": "classify.py byte-identical to Tranche 2's: classify_build_success (pin; zero warnings, zero errors; "
                   "no exception) for A3, A6, C8, C15 and classify_a for every run (C5's with source EXACTLY_C); the "
                   "dedicated C5 build-admission checker c5.admit, C5 only",
    "packet_tooling": "render.py, build_packet.py, publish_checks.py, worktree.py, finalize.py, push.py, final_gate.py "
                      "(frozen with every other tool)",
    "c5_permitted_warnings": [w["header"] + f" at {w['file']}:{w['line']}:{w['col']}" for w in c5.WARNINGS],
    "c5_expected_executable": {"path": K.C5_EXE, "sha256": K.C5_EXE_SHA256, "size": K.C5_EXE_SIZE},
    "retries": "none: one build per control (none for C5), one run per control, no rebuild, no rerun, no cap increase, "
               "no alternate patch, no quotient for a mutant; the accepted controls and the unchanged-core "
               "configurations are never rerun",
}
PRE_FREEZE = {"receipts/00-authentication.txt": "RESULT: ALL CHECKS HOLD",
              "receipts/01-supervision-selftest/SELFTEST.txt": "SELF-TEST RESULT: ALL OK",
              "receipts/02-EXPECTATIONS.console.txt": "EXPECTATIONS: DERIVED — ALL CROSS-CHECKS HOLD",
              "receipts/03-c2-controls/SUMMARY.txt": "C2 CONTROLS: ALL AS EXPECTED",
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
                     "SHA256SUMS_blob": K.git("rev-parse", f"{K.T2}:{K.T2_PACKET}/SHA256SUMS").decode().strip()}}
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
            "key_fields_sha256": K.sha256_bytes(c["key_fields"].encode()) if c.get("key_fields") else None,
            "linked": {"type": L["type"], "commit": L["commit"], "directory": L["directory"],
                       "classification_blob": L["classification_blob"], "raw_output_sha256": L["raw_output_sha256"]}}
    out["baseline_executable"] = {"path": K.EXE, "sha256": K.sha256_file(K.EXE), "size": os.path.getsize(K.EXE)}
    dep = K.C5_EXE + ".d"
    out["c5_preserved"] = {"executable": {"path": K.C5_EXE, "sha256": K.sha256_file(K.C5_EXE) if os.path.isfile(K.C5_EXE)
                                          else None, "size": os.path.getsize(K.C5_EXE) if os.path.isfile(K.C5_EXE)
                                          else None},
                           "dep_info_sha256": K.sha256_file(dep) if os.path.isfile(dep) else None,
                           "worktree_head": K.git("rev-parse", "HEAD", repo=K.C5_WT, check=False).decode().strip(),
                           "build_receipt_files": {f: K.sha256_file(K.inputs("c5", "03-build", f)) for f in
                                                   sorted(os.listdir(K.inputs("c5", "03-build")))}}
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
    for rel in ("receipts/02-EXPECTATIONS.json", K.RULING, K.C5_RULING, K.CAMPAIGN):
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
