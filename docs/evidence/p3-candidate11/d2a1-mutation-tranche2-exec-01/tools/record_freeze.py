#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once. Revised by -RESUME-01 and
-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 7): recorded once, before C23.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists or a pre-freeze receipt does not
                            record success

Frozen files: every tool (tools/), the accepted Tranche-1 tools (original-tools/) and publication tools
(original-publication-tools/), every authenticated input (inputs/: the catalog, the definitions record, the ten
patches, the contract, AD-1-R4, AD-1-R5, its Owner adoption record, the accepted F-R proof, every linked unchanged-core
receipt), the expectations (receipts/02-EXPECTATIONS.json: patch identities, expected mutant blobs, control order,
test and configuration selections, named-property alternatives, A6's key fields, ceilings, limits, restoration and link
mappings) and the two records of this mission (the Architect ruling, the campaign status). supervise.py and control.py
re-check every one of them before every supervised step; the gates check them again.

Recorded identities (each re-checked by the pre-execution gate): Candidate C; the catalog; the accepted Tranche-1
evidence commit and its three packets' manifests; the Stage-1, Stage-2 and R5 evidence; every linked receipt; the
pinned baseline executable; each control's patch, mutant identity, test, configuration, permitted properties, ceiling
and restored side; the commands, environments, resource policy, restoration rule and packet/publication tooling.
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
    "controls": "each once, in order: a fresh mission-owned worktree at C; the frozen catalog patch (exact mutant blob "
                "and sha256, only the catalog's file changed); one build, with no error and no warning diagnostic (no "
                "C28-style exception); one original-explorer run of the catalog's test on the pinned mutant executable; "
                "restoration by git apply -R, verified byte for byte; then the restored side linked; a stop on the "
                "first unsuccessful control",
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"builds": K.BUILD_LIMIT, "every Tranche-2 run": 2700},
    "ceilings_states": {c: K.MATRIX[c][2] for c in K.ORDER},
    "permitted_properties": {c: K.MATRIX[c][1] for c in K.ORDER},
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "git apply -R of the same frozen patch in the disposable worktree; the changed file byte-equal to C; "
                   "verify_tree.py EXACTLY_C; the path-aware status policy (verify_mutant.status_policy): no tracked "
                   "change, ignored entries only beneath the worktree's own real target directory",
    "restored_side": "linked only after the control's own verified restoration, never rerun: the accepted exact-final "
                     "Stage-1/Stage-2 unchanged-core receipt the catalog names; C8: the Stage-1 composed-quotient F-B "
                     "PASS (restored side only); C15: the AD-1-R5 F refinement discharge record (never PASS)",
    "classifiers": "classify.py: classify_build_success (pin) and classify_a; classify_a's only change is catalog-"
                   "required (C3a/A6 permitted alternatives; A6 key fields)",
    "packet_tooling": "render.py, build_packet.py, publish_checks.py, worktree.py, finalize.py, push.py, final_gate.py "
                      "(frozen with every other tool)",
    "retries": "none: one build per control, one run per control, no rebuild, no rerun, no cap increase, no alternate "
               "patch; the accepted controls and the unchanged-core configurations are never rerun",
}
PRE_FREEZE = {"receipts/00-authentication.txt": "RESULT: ALL CHECKS HOLD",
              "receipts/01-supervision-selftest/SELFTEST.txt": "SELF-TEST RESULT: ALL OK",
              "receipts/02-EXPECTATIONS.console.txt": "EXPECTATIONS: DERIVED — ALL CROSS-CHECKS HOLD",
              "receipts/03-t2-controls/SUMMARY.txt": "T2 CONTROLS: ALL AS EXPECTED",
              "receipts/04-tool-diff/TOOL-DIFF-CHECK.txt": "TOOL DIFF: ALL CHECKS HOLD",
              "receipts/04b-rehearsals/SUMMARY.txt": "REHEARSALS: ALL AS EXPECTED"}


def identities():
    """The identities the freeze records (mission section 7), each re-checked by the pre-execution gate."""
    out = {"candidate_c": {"commit": K.C, "tree": K.git("rev-parse", K.C + "^{tree}").decode().strip(), "blobs": {
        p: K.git("rev-parse", f"{K.C}:{p}").decode().strip() for p in K.BLOBS}},
        "catalog": {"path": K.CATALOG_PATH, "blob": K.git("rev-parse", f"{K.R2}:{K.CATALOG_PATH}").decode().strip(),
                    "sha256": K.sha256_bytes(K.blob(K.R2, K.CATALOG_PATH)), "commit": K.R2},
        "tranche1": {"commit": K.T1, "tree": K.git("rev-parse", K.T1 + "^{tree}").decode().strip()}}
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
    for sub in ("tools", "original-tools", "original-publication-tools", "inputs"):
        for d, dirs, fs in os.walk(os.path.join(K.EV, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                if f.endswith(".pyc"):
                    raise SystemExit(f"REFUSED: bytecode under {sub}/ ({f}); run with python3 -B")
                p = os.path.join(d, f)
                files[os.path.relpath(p, K.EV)] = K.sha256_file(p)
    for rel in ("receipts/02-EXPECTATIONS.json", K.RULING, K.CAMPAIGN):
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
