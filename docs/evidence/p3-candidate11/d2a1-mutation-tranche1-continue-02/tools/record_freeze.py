#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: record the freeze, once, before the first baseline
invocation. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 10): the freeze is recorded once before
C14, and it also covers the EXEC-01 tools the repair starts from, the C22 Architect-ruling record and every
carried-forward receipt copy; the pre-freeze receipts are this mission's.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 14): recorded once before the C28 invocation; it
covers the RESUME-01 tools the mission starts from (original-tools/), every tool (the dedicated C28 admission/pin tool
and its controls among them), the inputs (catalog, the twelve patches including the seven relevant ones), the
carried-forward expectations and baselines, both Architect ruling records; and it records, as identities, the two
previous mission-execution packets (commit, tree, SHA256SUMS sha256), the C28 prior build evidence (every file by
path, blob and sha256), the C28 expected executable identity, the carried-forward baseline receipt identities, the
execution order, commands, environments, classifiers, supervision, restoration policy and packet tooling.

    record_freeze.py        writes receipts/05-FREEZE.json (sha256 of every frozen file) and receipts/05-FREEZE-RECORD.txt
                            (first line `FREEZE: RECORDED`); refuses if either exists

Frozen: every tool (tools/*.py, tools/scope-wrap.sh), the EXEC-01 tools (original-tools/), the reused vocabulary, every
authenticated input copy (catalog, definitions record, the twelve patches, contract, AD-1-R4, AD-1-R5, the Stage-2 tool
copies, the linked Stage-2 receipts), the carried-forward expectations (receipts/02-EXPECTATIONS.json: commands,
environments, selections, named properties, ceilings, limits, control order, restoration and link mappings), the
carried-forward baseline, healthy-build and C22 receipts, the C22 Architect-ruling record and the catalog-freeze ruling
record. supervise.py and control.py re-check every one of them before every supervised step.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

POLICY = {
    "control_order": K.CONTINUE_ORDER,
    "architect_accepted_not_executed": K.CONTINUE_ACCEPTED,
    "c28": "the existing RESUME-01 build, admitted read-only by tools/c28_admit.py under the narrow C28 ruling (no "
           "rebuild, no Cargo, no rustc); the preserved executable pinned read-only; one supervised T-RES invocation; "
           "the restored worktree re-authenticated read-only; the carried-forward T-RES (e2) linked",
    "later_builds": "C12 C9 C11 C17 C18a C18b: one fresh worktree, the frozen patch, one build each with the general "
                    "classifier unchanged (no error, no warning diagnostic; the C28 exception does not transfer)",
    "not_authorized": K.NOT_AUTHORIZED,
    "supervision": "systemd user scope MemoryMax=16G MemorySwapMax=0, RuntimeMaxSec=limit+10 (backstop), timeout -s KILL "
                   "<limit>, /usr/bin/time -v, nice -n 10; MemAvailable >= 18 GiB before each step; no conflicting test "
                   "executable, build or p3c11-* scope",
    "limits_seconds": {"baseline tests": 180, "builds": K.BUILD_LIMIT, "unit-test controls": 180, "C12 (X-1)": 180,
                       "F-B2 and F-G controls": 2700},
    "build_command": " ".join(K.BUILD_ARGV),
    "build_environment": K.build_env("<worktree>/target"),
    "run_environment": K.RUN_ENV,
    "restoration": "git apply -R of the same frozen patch in the disposable worktree; changed file byte-equal to C; "
                   "verify_tree.py EXACTLY_C; the path-aware status policy (verify_mutant.status_policy): no tracked "
                   "change, ignored entries only beneath the worktree's own real target directory; C28: no patch is "
                   "applied or reversed, the restored worktree is re-authenticated read-only by the same checks",
    "classifiers": "classify.py unchanged (RESUME-01 = EXEC-01): classify_build_success (pin), classify_a, classify_b; "
                   "C28's run: classify_b with source EXACTLY_C (the mutant blob withheld from the run-source check "
                   "only)",
    "packet_tooling": "render.py, build_packet.py, publish_checks.py, final_gate.py (frozen with every other tool)",
    "source_verification": "verify_mutant.py: HEAD/tree C, detached, index = HEAD, every tracked byte and mode compared, "
                           "the named file exactly the catalog mutant; the path-aware status policy (resume section 7)",
    "retries": "none: one build per control (none for C28), one run per control, no rebuild, no rerun; C22 C14 C19 "
               "C21 C27 and the baselines are not rerun",
}


def identities():
    """The identities the freeze records (continue mission section 14), each re-checked by the pre-execution gate."""
    import c28_admit
    out = {}
    for name, commit, packet in (("EXEC-01", K.PREV, K.PREV_PACKET), ("RESUME-01", K.RESUME, K.RESUME_PACKET)):
        b = K.git("rev-parse", f"{commit}:{packet}/SHA256SUMS").decode().strip()
        out[f"{name} packet"] = {"commit": commit, "tree": K.git("rev-parse", commit + "^{tree}").decode().strip(),
                                 "packet": packet, "SHA256SUMS_blob": b,
                                 "SHA256SUMS_sha256": K.sha256_bytes(K.git("cat-file", "blob", b))}
    ev = c28_admit.evidence()
    out["C28 prior build evidence"] = [{k: a[k] for k in ("path", "blob", "sha256")} for a in ev["auth"]]
    out["C28 expected executable"] = {k: c28_admit.RULING[k] for k in ("executable", "exe_sha256", "exe_size",
                                                                       "mutant_blob", "mutant_sha256", "patch_sha256",
                                                                       "stderr_sha256")}
    out["carried-forward baselines"] = {}
    for c in K.CARRIED:
        for rel in (["receipts/02-EXPECTATIONS.json"] if c.endswith(".json") else
                    [c + "/CLASSIFICATION.txt", c + "/descriptor.json", c + "/raw-output.txt"]):
            b = K.git("rev-parse", f"{K.PREV}:{K.PREV_PACKET}/{rel}").decode().strip()
            out["carried-forward baselines"][rel] = {"blob": b, "sha256": K.sha256_bytes(K.git("cat-file", "blob", b))}
    out["candidate_c"] = {"commit": K.C, "tree": K.C_TREE, "blobs": K.BLOBS}
    out["catalog"] = {"path": K.CATALOG_PATH, "blob": K.CATALOG_BLOB, "sha256": K.CATALOG_SHA, "commit": K.R2}
    return out


PRE_FREEZE = {"receipts/00-authentication.txt": "RESULT: ALL CHECKS HOLD",
              "receipts/01-supervision-selftest/SELFTEST.txt": "SELF-TEST RESULT: ALL OK",
              "receipts/03-c28-admission-controls/SUMMARY.txt": "C28 ADMISSION CONTROLS: ALL AS EXPECTED",
              "receipts/04-tool-diff/TOOL-DIFF-CHECK.txt": "TOOL DIFF: ALL CHECKS HOLD",
              "receipts/04b-render-rehearsal/SUMMARY.txt": "RENDER REHEARSAL: ALL AS EXPECTED"}


def main():
    fj = os.path.join(K.EV, "receipts/05-FREEZE.json")
    ft = os.path.join(K.EV, "receipts/05-FREEZE-RECORD.txt")
    if os.path.exists(fj) or os.path.exists(ft):
        raise SystemExit("REFUSED: the freeze is already recorded")
    files = {}
    for sub in ("tools", "original-tools", "inputs", "vocabulary") + tuple(c for c in K.CARRIED if c.count("/") == 1
                                                                         and os.path.isdir(os.path.join(K.EV, c))):
        for d, dirs, fs in os.walk(os.path.join(K.EV, sub)):
            dirs[:] = sorted(x for x in dirs if x != "__pycache__")
            for f in sorted(fs):
                if not f.endswith(".pyc"):
                    p = os.path.join(d, f)
                    files[os.path.relpath(p, K.EV)] = K.sha256_file(p)
    for rel in ("receipts/02-EXPECTATIONS.json", K.RULING, K.C28_RULING) + tuple(K.CARRIED_DOCS):
        files[rel] = K.sha256_file(os.path.join(K.EV, rel))
    prior = {rel: K.sha256_file(os.path.join(K.EV, rel)) for rel in PRE_FREEZE}
    for rel in prior:
        last = open(os.path.join(K.EV, rel)).read().rstrip().splitlines()[-1]
        if last != PRE_FREEZE[rel]:
            raise SystemExit(f"REFUSED: {rel} does not record success")
    rec = {"mission": K.MISSION, "recorded": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "files": files,
           "pre_freeze_receipts": prior, "policy": POLICY, "identities": identities(),
           "statement": "No semantic tool repair, patch repair, classifier repair, alternate control encoding or "
                        "automatic retry after this point. A frozen-tool failure stops the mission and preserves its raw "
                        "evidence."}
    json.dump(rec, open(fj, "w"), indent=1)
    lines = ["FREEZE: RECORDED", f"recorded: {rec['recorded']}", f"frozen files: {len(files)}",
             f"05-FREEZE.json sha256 {K.sha256_file(fj)}"]
    lines += [f"  {h}  {p}" for p, h in sorted(files.items())]
    lines += ["pre-freeze receipts (success recorded):"] + [f"  {h}  {p}" for p, h in prior.items()]
    lines += ["policy:"] + [f"  {k}: {v}" for k, v in POLICY.items()]
    lines += ["identities:"] + [f"  {k}: {json.dumps(v)}" for k, v in rec["identities"].items()]
    lines.append(rec["statement"])
    open(ft, "w").write("\n".join(lines) + "\n")
    print("\n".join(lines[:4]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
