#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 1, 3, 4, 5 and 6: authenticate every input (read-only).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (sections 2, 10 and 20): section [6] states the resume
mission's paths (the previous mission's preserved worktrees present, this mission's absent before creation), and
section [7] authenticates the complete previous evidence packet, the facts of the Architect's C22 ruling, and the
carried-forward receipt copies. Sections [1]-[5] are unchanged.

Every identity is compared with the value the mission states, or with the value the frozen R2 catalog records; any
mismatch prints MISMATCH and the exit status is 1. Reads git objects, the remote's ref listing and the pinned baseline
executable (hashed and sized, never run). Writes only EV/inputs/ (byte copies of authenticated inputs, re-hashed after
writing), the carried-forward receipt copies under EV/receipts/ and EV/documents/ (read-only byte copies of the
previous packet's files, each checked against its SHA256SUMS), and a temporary patch-check directory outside every
repository (removed again).

    authenticate.py            prints the authentication record; exit 0 only if every check holds; this mission's
                               worktree paths must still be absent
    authenticate.py --gate     the same checks again at a gate (this mission's paths reported, not required absent;
                               input and carried-forward copies compared, not rewritten)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

fail = 0
GATE = len(sys.argv) > 1 and sys.argv[1] == "--gate"
TRANCHE = K.ORDER
# Mission-header expectations, stated independently of the catalog and compared with it (the catalog is the authority;
# a disagreement stops the mission).
HEADER = {
    "C22": ("build", "none (build)", "(d) intentional mutant compile failure; restored (e4) healthy build", "T-OWN"),
    "C14": ("unit", "named unit test (not exploration)", "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
            "T-BIND"),
    "C19": ("unit", "named unit test (not exploration)", "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
            "T-CELL"),
    "C21": ("unit", "named unit test (not exploration)", "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
            "W6"),
    "C27": ("unit", "named unit test (not exploration)", "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
            "T-PREC"),
    "C28": ("unit", "named unit test (not exploration)", "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
            "T-RES"),
    "C12": ("explore", "X-1", "(a) original-explorer mutant FAIL; restored (e1)", None),
    "C9": ("explore", "F-B2", "(a) original-explorer mutant FAIL; restored (e1)", None),
    "C11": ("explore", "F-B2", "(a) original-explorer mutant FAIL; restored (e1)", None),
    "C17": ("explore", "F-B2", "(a) original-explorer mutant FAIL; restored (e1)", None),
    "C18a": ("explore", "F-G", "(a) original-explorer mutant FAIL; restored (e1)", None),
    "C18b": ("explore", "F-G", "(a) original-explorer mutant FAIL; restored (e1)", None),
}
STAGE2_TOOLS = ["tools/configs.py", "tools/normalize.py", "tools/report.py", "tools/verify_tree.py",
                "tools/scope-wrap.sh", "vocabulary/VOCABULARY.json"]
LINKED = {"F-B2": ("receipts/61-full-F-B2", "f1d655eb6b85bdafd13313e72c3ffbd49147bed4",
                   "fa48050d7ea0ace7e9382bb25282e5b80f18ae63",
                   "516adc39418a5cc0db8432e1ce7c1051029d05cb30dc133ac9b1147439633aa3",
                   K.TEST_PREFIX + "full_fb2"),
          "F-G": ("receipts/60-full-F-G", "dace6c67fd98904e47b179cbba9b1a421c5971f6",
                  "0b77294ef4eec4da1d911a5cf73f7124e42ebe57",
                  "fabcbf13cfccec2744d03d38042dec7e72e3f3699f941a9a2ce17869d55705d1",
                  K.TEST_PREFIX + "full_fg")}
LINKED_FILES = ["CLASSIFICATION.txt", "CHECKS.txt", "descriptor.json", "PARSED-RECEIPT.json", "RECEIPT.txt",
                "raw-output.txt", "time-v.txt", "scope-record.txt"]


def git(*args):
    r = subprocess.run(["git", "-C", K.REPO, *args], capture_output=True)
    return None if r.returncode != 0 else r.stdout


def gtext(*args):
    out = git(*args)
    return None if out is None else out.decode().strip()


def chk(label, actual, expected):
    global fail
    if actual == expected:
        print(f"    OK        {label}: {actual}")
    else:
        print(f"    MISMATCH  {label}: actual={actual} expected={expected}")
        fail = 1


def copy_in(name, data, expected_sha):
    path = K.inputs(name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):
        chk(f"input copy inputs/{name} already present and equal", K.sha256_file(path), expected_sha)
        return
    with open(path, "wb") as fh:
        fh.write(data)
    os.chmod(path, 0o444)
    chk(f"input copy inputs/{name} sha256", K.sha256_file(path), expected_sha)


def packet_sums(commit, packet):
    sums = git("show", f"{commit}:{packet}/SHA256SUMS").decode()
    return {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}


def patch_result(patch_bytes, rel, base):
    """Apply the documentary patch with `git apply` to a copy of C's file in a temporary directory outside every
    repository (GIT_CEILING_DIRECTORIES stops discovery); return the resulting bytes, or the error."""
    tmp = tempfile.mkdtemp(prefix="mt1x-auth-apply-")
    try:
        os.makedirs(os.path.join(tmp, "w", os.path.dirname(rel)))
        with open(os.path.join(tmp, "w", rel), "wb") as fh:
            fh.write(base)
        with open(os.path.join(tmp, "p.patch"), "wb") as fh:
            fh.write(patch_bytes)
        env = dict(os.environ, GIT_CEILING_DIRECTORIES=tmp)
        r = subprocess.run(["git", "apply", "--whitespace=nowarn", os.path.join(tmp, "p.patch")],
                           cwd=os.path.join(tmp, "w"), capture_output=True, env=env)
        if r.returncode != 0:
            return None, r.stderr.decode()
        files = []
        for d, _, fs in os.walk(os.path.join(tmp, "w")):
            files += [os.path.relpath(os.path.join(d, f), os.path.join(tmp, "w")) for f in fs]
        if files != [rel]:
            return None, f"files after apply: {files}"
        return open(os.path.join(tmp, "w", rel), "rb").read(), ""
    finally:
        shutil.rmtree(tmp)


def main():
    print(f"{K.MISSION} — INPUT AUTHENTICATION (read-only)")
    print("executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission effort: High")
    print()
    print("[1] frozen baseline Candidate C (mission 3)")
    chk("type", gtext("cat-file", "-t", K.C), "commit")
    chk("tree", gtext("rev-parse", K.C + "^{tree}"), K.C_TREE)
    chk("single parent", gtext("log", "-1", "--format=%P", K.C), K.C_PARENT)
    for rel, b in K.BLOBS.items():
        chk(f"{rel} blob at C", gtext("rev-parse", f"{K.C}:{rel}"), b)
        data = git("cat-file", "blob", b)
        chk(f"{rel} content hashes to its blob (in-process)", K.git_blob_id(data), b)
        print(f"    info      {rel} sha256 {K.sha256_bytes(data)}, {len(data)} bytes")
    print()
    print("[2] the frozen R2 catalog and its evidence commit (mission 1, 5)")
    chk("R2 evidence commit type", gtext("cat-file", "-t", K.R2), "commit")
    chk("R2 evidence commit tree", gtext("rev-parse", K.R2 + "^{tree}"), K.R2_TREE)
    chk("R2 evidence commit single parent", gtext("log", "-1", "--format=%P", K.R2), K.R2_PARENT)
    chk("Candidate C is an ancestor of the R2 evidence commit",
        subprocess.run(["git", "-C", K.REPO, "merge-base", "--is-ancestor", K.C, K.R2]).returncode, 0)
    outside = [p for p in gtext("diff", "--name-only", K.C, K.R2).splitlines() if not p.startswith("docs/evidence/")]
    chk("paths outside docs/evidence/ differing between C and the R2 evidence commit", outside, [])
    chk("catalog.json git blob", gtext("rev-parse", f"{K.R2}:{K.CATALOG_PATH}"), K.CATALOG_BLOB)
    cat_bytes = git("show", f"{K.R2}:{K.CATALOG_PATH}")
    chk("catalog.json sha256", K.sha256_bytes(cat_bytes), K.CATALOG_SHA)
    copy_in("catalog.json", cat_bytes, K.CATALOG_SHA)
    chk("definitions record git blob", gtext("rev-parse", f"{K.R2}:{K.DEFS_PATH}"), K.DEFS_BLOB)
    defs = git("show", f"{K.R2}:{K.DEFS_PATH}")
    chk("definitions record sha256", K.sha256_bytes(defs), K.DEFS_SHA)
    copy_in("ARCHITECT-CONTROL-DEFINITIONS-R2.md", defs, K.DEFS_SHA)
    listed = packet_sums(K.R2, K.R2_DIR.rstrip("/"))
    bad = [p for p, d in listed.items() if K.sha256_bytes(git("show", f"{K.R2}:{K.R2_DIR}{p}") or b"") != d]
    chk(f"R2 packet SHA256SUMS: all {len(listed)} listed files verify at {K.R2[:8]}", bad, [])
    chk("R2 SHA256SUMS lists the catalog with its sha256", listed.get("catalog/catalog.json"), K.CATALOG_SHA)
    chk("R2 SHA256SUMS lists the definitions record with its sha256", listed.get("ARCHITECT-CONTROL-DEFINITIONS-R2.md"),
        K.DEFS_SHA)
    cat = json.loads(cat_bytes)
    chk("catalog version recorded in the artifact", cat["version"], K.CATALOG_VERSION)
    chk("catalog counts", cat["counts"], {"architect_defined": 21, "blocked": 0, "candidate_ready": 35,
                                          "carried_forward": 14, "comparison": 1, "kill_required": 35})
    chk("catalog baseline", cat["baseline"], {"blobs": K.BLOBS, "commit": K.C, "tree": K.C_TREE})
    by = {e["id"]: e for e in cat["controls"]}
    chk("catalog: 36 entries (35 kill-required controls and A4)", len(cat["controls"]), 36)
    chk("tranche ids present in the catalog", [c for c in TRANCHE if c in by], TRANCHE)
    chk("non-authorized ids present in the catalog (none is executed)", [c for c in K.NOT_AUTHORIZED if c in by],
        K.NOT_AUTHORIZED)
    chk("tranche + non-authorized + A4 = every catalog entry",
        sorted(TRANCHE + K.NOT_AUTHORIZED + ["A4"]) == sorted(by), True)
    print()
    print("[3] the twelve tranche controls: patch, baseline, mutant, selection (mission 5, 6)")
    for cid in TRANCHE:
        e = by[cid]
        kind, cfg, rtype, named = HEADER[cid]
        print(f"  -- {cid}: {e['description']}")
        chk(f"{cid} kind", e["kind"], kind)
        chk(f"{cid} configuration", e["configuration"], cfg)
        chk(f"{cid} receipt type", e["receipt_type"], rtype)
        if named:
            chk(f"{cid} named test id", e.get("named_test_id"), named)
        else:
            chk(f"{cid} configuration (mission 14)", e["configuration"], K.EXPLORE_CFG[cid])
        chk(f"{cid} mutation source file", e["mutation_source_file"], K.PROTOCOL)
        chk(f"{cid} baseline blob = C's blob of the source file", e["baseline_blob"], K.BLOBS[K.PROTOCOL])
        chk(f"{cid} expected changed blob count", e["expected_changed_blob_count"], 1)
        chk(f"{cid} execution status in the artifact (historical)", e["execution_status"], "NOT_ATTEMPTED")
        pb = git("show", f"{K.R2}:{K.R2_DIR}{e['patch_file'] if e['patch_file'].startswith('catalog/') else 'catalog/' + e['patch_file']}")
        chk(f"{cid} patch file sha256 = catalog patch_sha256", K.sha256_bytes(pb), e["patch_sha256"])
        chk(f"{cid} patch file bytes = catalog patch text", pb == e["patch"].encode(), True)
        chk(f"{cid} patch listed in R2 SHA256SUMS", listed.get("catalog/" + e["patch_file"]), e["patch_sha256"])
        copy_in(f"patches/{cid}.patch", pb, e["patch_sha256"])
        base = git("cat-file", "blob", K.BLOBS[K.PROTOCOL])
        res, err = patch_result(pb, K.PROTOCOL, base)
        chk(f"{cid} documentary patch applies to C's file (git apply, temporary copy outside every repository)",
            err, "")
        if res is not None:
            chk(f"{cid} patched file git blob = catalog mutant blob", K.git_blob_id(res), e["mutant_blob"])
            chk(f"{cid} patched file sha256 = catalog mutant sha256", K.sha256_bytes(res), e["mutant_sha256"])
        # The catalog's edits give the same bytes (anchor found exactly once, replaced once).
        text = base.decode()
        for ed in e["edits"]:
            chk(f"{cid} edit anchor occurs exactly {ed['expected_count']} time(s) at C", text.count(ed["anchor"]),
                ed["expected_count"])
            chk(f"{cid} edit anchor's first line at C", text[:text.find(ed["anchor"])].count("\n") + 1,
                ed["anchor_first_line_at_C"])
            text = text.replace(ed["anchor"], ed["replacement"], 1)
        chk(f"{cid} edits give the mutant blob too", K.git_blob_id(text.encode()), e["mutant_blob"])
        chk(f"{cid} mutant blob absent from the repository object store",
            subprocess.run(["git", "-C", K.REPO, "cat-file", "-e", e["mutant_blob"]], capture_output=True).returncode != 0,
            True)
        if kind != "build":
            chk(f"{cid} requires a mutant executable", e["requires_mutant_executable"], True)
            print(f"    info      {cid} test {e['test']}; ignored at C (catalog) {e['ignored_at_C']}; time limit "
                  f"{e.get('time_limit_seconds')} s; ceiling {e.get('ceiling_states')}; named property "
                  f"{e['named_property']}")
        else:
            chk("C22 mutant executable", e["mutant_executable"], "NOT PRODUCED BY DESIGN")
            chk("C22 mutant test run", e["mutant_test_run"], "NOT PRODUCED BY DESIGN")
            chk("C22 required healthy and mutant command",
                (e["restored_receipt"]["required_future_healthy_command"],
                 e["restored_receipt"]["required_future_mutant_command"]), (" ".join(K.BUILD_ARGV),) * 2)
            chk("C22 expected primary failure", (e["expected_primary_failure"]["diagnostic"],
                                                  e["expected_primary_failure"]["location"]),
                ("error[E0283]", K.UNIT + ":1089"))
    print()
    print("[4] contract, AD-1-R4, AD-1-R5, Stage-1 and Stage-2 evidence (mission 4)")
    contract = git("show", f"{K.CONTRACT_COMMIT}:{K.CONTRACT_PATH}")
    chk(f"base contract sha256 (at {K.CONTRACT_COMMIT[:8]})", K.sha256_bytes(contract or b""), K.CONTRACT_SHA)
    copy_in("CONTRACT-02-R1.md", contract, K.CONTRACT_SHA)
    r4 = git("show", f"{K.C}:{K.AD1R4}")
    chk("AD-1-R4 sha256 (at C)", K.sha256_bytes(r4 or b""), K.AD1R4_SHA)
    copy_in("AD1-R4.md", r4, K.AD1R4_SHA)
    chk("R5 proof evidence commit type", gtext("cat-file", "-t", K.R5_COMMIT), "commit")
    r5 = git("show", f"{K.R5_COMMIT}:{K.AD1R5}")
    chk("AD-1-R5 sha256 (at the R5 proof evidence commit)", K.sha256_bytes(r5 or b""), K.AD1R5_SHA)
    copy_in("AD1-R5.md", r5, K.AD1R5_SHA)
    chk("catalog cites AD-1-R4 and AD-1-R5 sha256", (cat["sources"]["ad1r4"]["sha256"], cat["sources"]["ad1r5"]["sha256"]),
        (K.AD1R4_SHA, K.AD1R5_SHA))
    chk("catalog cites the base contract sha256", cat["sources"]["contract"]["sha256"], K.CONTRACT_SHA)
    for name, commit, packet in (("Stage 1", K.STAGE1, K.STAGE1_PACKET), ("Stage 2", K.STAGE2, K.STAGE2_PACKET)):
        chk(f"{name} evidence commit type", gtext("cat-file", "-t", commit), "commit")
        chk(f"{name}: Candidate C is an ancestor",
            subprocess.run(["git", "-C", K.REPO, "merge-base", "--is-ancestor", K.C, commit]).returncode, 0)
        sums = packet_sums(commit, packet)
        badp = [p for p, d in sums.items() if K.sha256_bytes(git("show", f"{commit}:{packet}/{p}") or b"") != d]
        chk(f"{name} packet SHA256SUMS: all {len(sums)} listed files verify", badp, [])
        if name == "Stage 2":
            s2sums = sums
    for rel in STAGE2_TOOLS:
        data = git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/{rel}")
        chk(f"Stage-2 reused file {rel} listed in its SHA256SUMS", s2sums.get(rel), K.sha256_bytes(data))
        copy_in("stage2/" + rel, data, s2sums.get(rel))
    for cfg, (d, cls_blob, raw_blob, raw_sha, test) in LINKED.items():
        chk(f"linked {cfg} CLASSIFICATION.txt blob", gtext("rev-parse", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/CLASSIFICATION.txt"),
            cls_blob)
        chk(f"linked {cfg} raw-output.txt blob", gtext("rev-parse", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/raw-output.txt"),
            raw_blob)
        for f in LINKED_FILES:
            data = git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/{f}")
            chk(f"linked {cfg} {f} listed in Stage-2 SHA256SUMS", s2sums.get(f"{d}/{f}"), K.sha256_bytes(data or b""))
            copy_in(f"linked/{cfg}/{f}", data, s2sums.get(f"{d}/{f}"))
        raw = git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/raw-output.txt")
        chk(f"linked {cfg} raw output sha256", K.sha256_bytes(raw), raw_sha)
        cls = git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/CLASSIFICATION.txt").decode().splitlines()
        chk(f"linked {cfg} classification first line", cls[0], "CLASSIFICATION: PASS")
        chk(f"linked {cfg} outer class", cls[1],
            "outer class (AD-1-R4 K.2): PASS — admissible unchanged-core original-explorer PASS")
        desc = json.loads(git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/{d}/descriptor.json"))
        chk(f"linked {cfg} argv", desc["argv"], [K.EXE, test, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        chk(f"linked {cfg} executable before/after sha256",
            (desc["executable"]["before"]["sha256"], desc["executable"]["after"]["sha256"]), (K.EXE_SHA256,) * 2)
        chk(f"linked {cfg} source exactly C before/after",
            (desc["source_before"]["verdict"], desc["source_after"]["verdict"]), ("EXACTLY_C",) * 2)
        for cid in [c for c, v in K.EXPLORE_CFG.items() if v == cfg]:
            rr = by[cid]["restored_receipt"]
            chk(f"{cid} catalog restored receipt = this linked {cfg} receipt",
                (rr["commit"], rr["directory"], rr["classification_blob"], rr["raw_output_blob"], rr["raw_output_sha256"],
                 rr["descriptor_test"]), (K.STAGE2, f"{K.STAGE2_PACKET}/{d}", cls_blob, raw_blob, raw_sha, test))
    print()
    print("[5] pinned unchanged-baseline executable and its worktree (hashed and sized; never run here)")
    chk("executable is a regular file, not a symlink", (os.path.isfile(K.EXE), os.path.islink(K.EXE)), (True, False))
    chk("executable sha256", K.sha256_file(K.EXE), K.EXE_SHA256)
    chk("executable size", os.path.getsize(K.EXE), K.EXE_SIZE)
    vt = K.inputs("stage2/tools/verify_tree.py")
    r = subprocess.run([sys.executable, vt, K.BASE_WT], capture_output=True, text=True)
    d = json.loads(r.stdout)
    chk("baseline worktree exactly C (Stage-2 verify_tree.py, byte comparison of every tracked file)", d["verdict"],
        "EXACTLY_C")
    print(f"    info      {d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes; status {d['status_entries']}")
    print()
    print("[6] refs, destination absence and mission-owned paths")
    refs = subprocess.run(["git", "-C", K.REPO, "ls-remote", K.REMOTE], capture_output=True, text=True).stdout
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in refs.splitlines() if "\t" in l}
    print(f"    info      remote refs {len(rm)}")
    chk(f"remote {K.R2_REF}", rm.get(K.R2_REF), K.R2)
    chk("remote Stage-1 review ref", rm.get("refs/heads/review/p3-c11-d2a1-final-baseline-known-runs-01"), K.STAGE1)
    chk("remote Stage-2 review ref", rm.get("refs/heads/review/p3-c11-d2a1-final-baseline-remaining-configs-01"),
        K.STAGE2)
    chk("remote R5 review ref", rm.get("refs/heads/review/p3-c11-d2a1-fr-stutter-r5-01"), K.R5_COMMIT)
    chk(f"destination {K.DEST} absent on the remote", K.DEST in rm, False)
    chk(f"destination {K.DEST} absent locally", git("rev-parse", "--verify", "-q", K.DEST), None)
    chk(f"local branch refs/heads/{K.LOCAL_BRANCH} absent", git("rev-parse", "--verify", "-q",
                                                               "refs/heads/" + K.LOCAL_BRANCH), None)
    for ref in ("refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"):
        print(f"    info      remote {ref}: {rm.get(ref)}")
    chk(f"previous review ref {K.PREV_REF}", rm.get(K.PREV_REF), K.PREV)
    for p in [K.HEALTHY_WT, K.control_wt("C22"), K.PREV_DEV_WT]:
        locked = gtext("-C", p, "rev-parse", "--git-path", "locked") if os.path.isdir(p) else None
        reason = open(locked).read().strip() if locked and os.path.exists(locked) else None
        chk(f"previous mission's preserved worktree present, locked with its reason: {p}",
            reason is not None and reason.startswith(K.PREV_MISSION + " "), True)
    chk(f"previous mission's evidence worktree present: {K.PREV_EVIDENCE_WT}", os.path.isdir(K.PREV_EVIDENCE_WT), True)
    paths = [K.EVIDENCE_WT, K.DEV_WT] + [K.control_wt(c) for c in K.RESUME_ORDER]
    for p in paths:
        if GATE:
            print(f"    info      mission-owned path {p}: present {os.path.lexists(p)}")
        else:
            chk(f"mission-owned path absent before creation: {p}", os.path.lexists(p), False)
    print()
    previous()
    print()
    print("RESULT: " + ("ALL CHECKS HOLD" if not fail else "MISMATCH — STOP"))
    return fail


C22_RULING = {"patch_sha256": "e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6",
              "mutant_blob": "694b744de3b70a3a092c633b934262bd510d41aa",
              "mutant_sha256": "0f13a8e5512439aa6385bbe65c20ceb6a77fe6c819216ef82564f7b8880425e5",
              "diagnostic": "error[E0283]", "location": [K.UNIT, 1089, 5],
              "assertion": "<Ticket as NotClone<_>>::check();"}
# The previous executor's classifications: immutable historical outputs of the defective frozen tool (ruling 1).
HISTORICAL = {"receipts/31-C22/RESULT.txt": "CONTROL RESULT: MUTATION_CONTROL_BLOCKED",
              "receipts/31-C22/03-build/CLASSIFICATION.txt": "CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE",
              "receipts/31-C22/05-restore/RESTORE.txt": "RESTORE: FAILED",
              "receipts/31-C22/02-apply/APPLY.txt": "APPLY: EXACTLY_C_PLUS_PATCH",
              "receipts/20-healthy-build/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/10-baseline-X-1/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/11-baseline-T-BIND/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/12-baseline-T-CELL/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/13-baseline-W6/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/14-baseline-T-PREC/CLASSIFICATION.txt": "CLASSIFICATION: PASS",
              "receipts/15-baseline-T-RES/CLASSIFICATION.txt": "CLASSIFICATION: PASS"}


def previous():
    print("[7] the previous execution evidence, the C22 ruling's facts and the carried-forward copies (resume 1, 2)")
    chk("previous evidence commit type", gtext("cat-file", "-t", K.PREV), "commit")
    chk("previous evidence commit tree", gtext("rev-parse", K.PREV + "^{tree}"), K.PREV_TREE)
    chk("previous evidence commit single parent (the R2 evidence commit)", gtext("log", "-1", "--format=%P", K.PREV),
        K.PREV_PARENT)
    ns = gtext("diff", "--name-status", "--no-renames", K.PREV_PARENT, K.PREV).splitlines()
    chk("every path the previous evidence commit changes is added under its packet",
        bool(ns) and all(l.startswith("A\t" + K.PREV_PACKET + "/") for l in ns), True)
    sums = packet_sums(K.PREV, K.PREV_PACKET)
    files = sorted(f[len(K.PREV_PACKET) + 1:] for f in
                   gtext("ls-tree", "-r", "--name-only", K.PREV, K.PREV_PACKET + "/").splitlines())
    chk("previous SHA256SUMS lists every packet file but itself", sorted(sums) == [f for f in files if f != "SHA256SUMS"],
        True)
    data = {}
    proc = subprocess.Popen(["git", "-C", K.REPO, "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    for rel in files:
        proc.stdin.write(f"{K.PREV}:{K.PREV_PACKET}/{rel}\n".encode())
        proc.stdin.flush()
        size = int(proc.stdout.readline().split()[2])
        data[rel] = proc.stdout.read(size)
        proc.stdout.read(1)
    proc.stdin.close()
    proc.wait()
    bad = [p for p, d in sums.items() if K.sha256_bytes(data[p]) != d]
    chk(f"previous packet SHA256SUMS: all {len(sums)} listed files verify at {K.PREV[:8]}", bad, [])
    print(f"    info      previous packet: {len(files)} files, {sum(len(v) for v in data.values())} bytes; SHA256SUMS "
          f"sha256 {K.sha256_bytes(data['SHA256SUMS'])}")
    for rel, first in HISTORICAL.items():
        chk(f"previous {rel} first line (immutable history)", data[rel].decode().splitlines()[0], first)
    ap = json.loads(data["receipts/31-C22/02-apply/APPLY.json"])
    mf = ap["verify_mutant"]["mutant_file"]
    chk("C22 frozen patch sha256 (ruling)", (ap["patch_sha256"], ap["catalog_patch_sha256"]),
        (C22_RULING["patch_sha256"],) * 2)
    chk("C22 mutant protocol.rs blob and sha256 (ruling)", (mf["path"], mf["git_blob"], mf["sha256"]),
        (K.PROTOCOL, C22_RULING["mutant_blob"], C22_RULING["mutant_sha256"]))
    bd = json.loads(data["receipts/31-C22/03-build/descriptor.json"])
    pr = json.loads(data["receipts/31-C22/03-build/PARSED-RECEIPT.json"])
    chk("C22 build: the exact command", bd["argv"], K.BUILD_ARGV)
    for when in ("source_before", "source_after"):
        m = bd[when]["mutant_file"]
        other = [x for x in bd[when]["unexpected_status"] if not x.startswith("!! target/")]
        chk(f"C22 {when}: HEAD/tree C, detached, nothing staged, no other tracked difference, mutant bytes exact; "
            f"every rejected status entry an ignored file under the worktree-local target/",
            (bd[when]["head"], bd[when]["tree"], bd[when]["detached"], bd[when]["index_differs_from_head"],
             bd[when]["other_content_mismatches"], bd[when]["missing"], bd[when]["mode_mismatches"], m["git_blob"],
             m["sha256"], other),
            (K.C, K.C_TREE, True, [], [], [], [], C22_RULING["mutant_blob"], C22_RULING["mutant_sha256"], []))
    print(f"    info      C22 source_after: {len(bd['source_after']['unexpected_status'])} ignored entries under target/ "
          f"listed one by one (the defect); source_before: {len(bd['source_before']['unexpected_status'])}")
    chk("C22 diagnostics: exactly one coded error, E0283, and cargo's one summary",
        [d.split(":")[0] for d in pr["diagnostics"]], [C22_RULING["diagnostic"], "error"])
    chk("C22 primary location (ruling)", pr["primary_location"], C22_RULING["location"])
    stderr = data["receipts/31-C22/03-build/build.stderr"].decode()
    chk("C22 rendered assertion line at 1089 (ruling)", f"1089 |     {C22_RULING['assertion']}" in stderr.splitlines(),
        True)
    chk("C22 build process: exit 101 by a normal exit; no signal, OOM or timeout; completed record",
        (pr["process"]["exit_status"], pr["process"]["signal"], pr["supervisor"]["oom_kill"], pr["supervisor"]["oom"],
         pr["supervisor"]["timed_out"], pr["supervisor"]["completed"], pr["supervisor"]["swap_peak_bytes"]),
        (101, None, 0, 0, False, True, "0"))
    chk("C22: no compiler crash, no mutant executable (by design)",
        ("internal compiler error" in stderr, pr["mutant_executable"], [a for a in pr["deps_artifacts"]
                                                                       if a["test_executable_name"]]),
        (False, "NOT PRODUCED BY DESIGN", []))
    rs = json.loads(data["receipts/31-C22/05-restore/RESTORE.json"])
    chk("C22 restoration: protocol.rs back to C's blob and sha256; verify_tree.py EXACTLY_C",
        (rs["check"]["rc"], rs["reverse"]["rc"], rs["before"]["git_blob"], rs["after"]["git_blob"], rs["after"]["sha256"],
         rs["verify_tree"]["verdict"]),
        (0, 0, C22_RULING["mutant_blob"], K.BLOBS[K.PROTOCOL],
         "a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296", "EXACTLY_C"))
    hd = json.loads(data["receipts/20-healthy-build/descriptor.json"])
    chk("healthy build (e4): the same command; source exactly C before and after",
        (hd["argv"], hd["source_before"]["verdict"], hd["source_after"]["verdict"]), (K.BUILD_ARGV, "EXACTLY_C", "EXACTLY_C"))
    # The carried-forward copies: byte copies of the previous packet's files at the same relative path under EV.
    carried = [f for f in files if any(f == c or f.startswith(c + "/") for c in K.CARRIED)]
    chk("every carried-forward path is in the previous packet", all(any(f == c or f.startswith(c + "/") for f in files)
                                                                     for c in K.CARRIED), True)
    pairs = [(f, f) for f in carried] + [(src, dst) for dst, src in K.CARRIED_DOCS.items()]
    wrong = []
    for src, dst in pairs:
        path = os.path.join(K.EV, dst)
        if not GATE and not os.path.exists(path):
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as fh:
                fh.write(data[src])
            os.chmod(path, 0o444)
        if not os.path.isfile(path) or os.path.islink(path) or K.sha256_file(path) != sums[src]:
            wrong.append(dst)
    chk(f"carried-forward copies under EV ({len(pairs)} files) equal the previous packet's SHA256SUMS", wrong, [])
    for c in K.CARRIED:
        d = os.path.join(K.EV, c)
        if os.path.isdir(d):
            extra = [os.path.relpath(os.path.join(a, f), K.EV) for a, _, fs in os.walk(d) for f in fs
                     if os.path.relpath(os.path.join(a, f), K.EV) not in carried]
            chk(f"carried-forward {c}: no file that is not the previous packet's", extra, [])


if __name__ == "__main__":
    sys.exit(main())
