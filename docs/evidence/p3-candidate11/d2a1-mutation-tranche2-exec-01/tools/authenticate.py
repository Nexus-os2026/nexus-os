#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 1, 3, 4, 5 and 6: authenticate every input (read-only).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 and -CONTINUE-02 (their previous packets and paths).
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 1, 3, 5, 7 and 17) for the ten Tranche-2 controls:

  [1] frozen Candidate C (commit, tree, parent, the three blobs)                          (unchanged from Tranche 1)
  [2] the frozen R2 catalog, its definitions record and packet SHA256SUMS                 (unchanged from Tranche 1)
  [3] the ten Tranche-2 controls: the mission's control matrix (configuration, permitted properties, ceiling, mutated
      file, restored side) against the catalog; each frozen patch (file, sha256, catalog text, SHA256SUMS), applied to
      C's file in a temporary directory outside every repository, gives the catalog's mutant blob and sha256, and so do
      the catalog's own edits; no mutant blob is in the object store
  [4] the contract, AD-1-R4 and its Owner adoption record, AD-1-R5 with its Owner adoption record and the accepted
      F-R proof; the Stage-1 and Stage-2 packets (every SHA256SUMS entry verifies); every unchanged-core receipt this
      tranche links (byte copies under inputs/linked/, each file checked against its packet's SHA256SUMS and against
      the catalog's restored_receipt), and the F-R receipt (INCOMPLETE, never PASS)
  [5] the pinned unchanged-baseline executable (hashed and sized, never run) and its worktree exactly C
  [6] refs, destination and local-branch absence, mission-owned paths absent before creation
  [7] the complete accepted Tranche-1 evidence (ebe42009): commit, tree, parent, confinement; the EXEC-01, RESUME-01
      and CONTINUE-02 packets (every SHA256SUMS entry verifies; EXEC-01 and RESUME-01 byte-identical to their own
      evidence commits); each of the twelve accepted controls' receipt directory and RESULT (historical executor
      labels, unchanged), patch and mutant identity; the Tranche-1 ruling records; the R2 duplicate-policy ruling;
      this mission's original-tools/ equal to the accepted Tranche-1 execution tools and original-publication-tools/
      to the PUBLISH-04 publication tools

Every identity is compared with the value the mission states or the value the frozen catalog records; any mismatch
prints MISMATCH and the exit status is 1. Writes only EV/inputs/ (read-only byte copies of authenticated inputs, re-hashed
after writing) and a temporary patch-check directory outside every repository (removed again).

    authenticate.py            prints the authentication record; exit 0 only if every check holds; this mission's
                               paths must still be absent
    authenticate.py --gate     the same checks again at a gate (this mission's paths reported, not required absent;
                               input copies compared, not rewritten)
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
EITHER = "AD-1-R4 F.1(a): either property the row names"
LINKED_FILES = ["CLASSIFICATION.txt", "CHECKS.txt", "descriptor.json", "PARSED-RECEIPT.json", "RECEIPT.txt",
                "raw-output.txt", "time-v.txt", "scope-record.txt"]
CANDIDATE_RESULT = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"


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


def packet_files(commit, packet):
    """Every file of a packet at a commit: {packet-relative path: bytes} (one cat-file --batch process)."""
    files = sorted(f[len(packet) + 1:] for f in gtext("ls-tree", "-r", "--name-only", commit, packet + "/").splitlines())
    data = {}
    proc = subprocess.Popen(["git", "-C", K.REPO, "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    for rel in files:
        proc.stdin.write(f"{commit}:{packet}/{rel}\n".encode())
        proc.stdin.flush()
        size = int(proc.stdout.readline().split()[2])
        data[rel] = proc.stdout.read(size)
        proc.stdout.read(1)
    proc.stdin.close()
    proc.wait()
    return data


def section1_2():
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
    print("[2] the frozen R2 catalog and its evidence commit (mission 3)")
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
    chk("catalog.json unchanged at the Tranche-1 evidence commit", gtext("rev-parse", f"{K.T1}:{K.CATALOG_PATH}"),
        K.CATALOG_BLOB)
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
    r2_now = [p for p in gtext("ls-tree", "-r", "--name-only", K.T1, K.R2_DIR).splitlines()]
    chk("the R2 packet is byte-identical at the Tranche-1 evidence commit (every path's blob)",
        [p for p in r2_now if gtext("rev-parse", f"{K.T1}:{p}") != gtext("rev-parse", f"{K.R2}:{p}")], [])
    cat = json.loads(cat_bytes)
    chk("catalog version recorded in the artifact", cat["version"], K.CATALOG_VERSION)
    chk("catalog counts", cat["counts"], {"architect_defined": 21, "blocked": 0, "candidate_ready": 35,
                                          "carried_forward": 14, "comparison": 1, "kill_required": 35})
    chk("catalog baseline", cat["baseline"], {"blobs": K.BLOBS, "commit": K.C, "tree": K.C_TREE})
    return cat, listed


def section3(cat, listed):
    by = {e["id"]: e for e in cat["controls"]}
    print("[3] the ten Tranche-2 controls: matrix, patch, baseline, mutant (mission 2, 3, 4)")
    chk("catalog: 36 entries (35 kill-required controls and A4)", len(cat["controls"]), 36)
    chk("Tranche-2 ids present in the catalog, in the mission's order", [c for c in TRANCHE if c in by], TRANCHE)
    chk("Tranche 2 + Tranche-1 accepted + not authorized + A4 = every catalog entry, each once",
        sorted(TRANCHE + K.ACCEPTED + K.NOT_AUTHORIZED + ["A4"]) == sorted(by) and len(set(TRANCHE + K.ACCEPTED +
                                                                                           K.NOT_AUTHORIZED)) == 35, True)
    for cid in TRANCHE:
        e = by[cid]
        cfg, allowed, cap, src, link = K.MATRIX[cid]
        print(f"  -- {cid}: {e['description']}")
        chk(f"{cid} kind", e["kind"], "explore")
        chk(f"{cid} configuration (mission 4)", e["configuration"], cfg)
        chk(f"{cid} named property / permitted properties (mission 4)", e["named_property"], allowed)
        chk(f"{cid} ceiling (mission 4, 10)", e["ceiling_states"], cap)
        chk(f"{cid} time limit", e["time_limit_seconds"], 2700)
        chk(f"{cid} explorer", e["explorer"], "original (`explore`)")
        chk(f"{cid} receipt type", e["receipt_type"], "(a) original-explorer mutant FAIL; restored (e1)")
        chk(f"{cid} special attribution rules", e["special_attribution_rules"] == [] if cid not in ("C3a", "A6", "C15")
            else (e["special_attribution_rules"] == [EITHER] if cid != "C15" else
                  e["special_attribution_rules"][0].startswith("AD-1-R5 F: original explorer, E8, 30,000,000")), True)
        chk(f"{cid} mutation source file", e["mutation_source_file"], src)
        chk(f"{cid} baseline blob = C's blob of the source file", e["baseline_blob"], K.BLOBS[src])
        chk(f"{cid} expected changed blob count", e["expected_changed_blob_count"], 1)
        chk(f"{cid} execution status in the artifact (historical)", e["execution_status"], "NOT_ATTEMPTED")
        chk(f"{cid} requires a mutant executable", e["requires_mutant_executable"], True)
        pb = git("show", f"{K.R2}:{K.R2_DIR}catalog/{e['patch_file']}")
        chk(f"{cid} patch file sha256 = catalog patch_sha256", K.sha256_bytes(pb), e["patch_sha256"])
        chk(f"{cid} patch file bytes = catalog patch text", pb == e["patch"].encode(), True)
        chk(f"{cid} patch listed in R2 SHA256SUMS", listed.get("catalog/" + e["patch_file"]), e["patch_sha256"])
        copy_in(f"patches/{cid}.patch", pb, e["patch_sha256"])
        base = git("cat-file", "blob", K.BLOBS[src])
        res, err = patch_result(pb, src, base)
        chk(f"{cid} frozen patch applies to C's file (git apply, temporary copy outside every repository)", err, "")
        if res is not None:
            chk(f"{cid} patched file git blob = catalog mutant blob", K.git_blob_id(res), e["mutant_blob"])
            chk(f"{cid} patched file sha256 = catalog mutant sha256", K.sha256_bytes(res), e["mutant_sha256"])
        text = base.decode()
        for ed in e["edits"]:
            chk(f"{cid} edit anchor occurs exactly {ed['expected_count']} time(s) at C", text.count(ed["anchor"]),
                ed["expected_count"])
            text = text.replace(ed["anchor"], ed["replacement"], 1)
        chk(f"{cid} the catalog's edits give the mutant blob too", K.git_blob_id(text.encode()), e["mutant_blob"])
        chk(f"{cid} mutant blob absent from the repository object store",
            subprocess.run(["git", "-C", K.REPO, "cat-file", "-e", e["mutant_blob"]], capture_output=True).returncode != 0,
            True)
        print(f"    info      {cid} test {e['test']}; ignored at C {e['ignored_at_C']}; restored side "
              f"{e['restoration']['restored_side'][:110]}")
    return by


def stage_receipt(name, commit, packet, sums, d, rr, quotient=False):
    """One accepted unchanged-core receipt: every file listed in its packet's SHA256SUMS, copied under inputs/linked/,
    and equal to the catalog's restored_receipt fields."""
    tag = f"{'S1' if commit == K.STAGE1 else 'S2'}-{d}"
    for f in LINKED_FILES:
        data = git("show", f"{commit}:{packet}/receipts/{d}/{f}")
        chk(f"{name} {d}/{f} listed in its packet's SHA256SUMS", sums.get(f"receipts/{d}/{f}"),
            K.sha256_bytes(data or b""))
        copy_in(f"linked/{tag}/{f}", data, sums.get(f"receipts/{d}/{f}"))
    if rr is None:
        return tag
    chk(f"{name} {d}: CLASSIFICATION.txt blob = catalog", gtext("rev-parse", f"{commit}:{packet}/receipts/{d}/"
                                                                             "CLASSIFICATION.txt"), rr["classification_blob"])
    chk(f"{name} {d}: raw-output.txt blob = catalog", gtext("rev-parse", f"{commit}:{packet}/receipts/{d}/raw-output.txt"),
        rr["raw_output_blob"])
    chk(f"{name} {d}: raw-output.txt sha256 = catalog", K.sha256_bytes(git("show", f"{commit}:{packet}/receipts/{d}/"
                                                                                  "raw-output.txt")),
        rr["raw_output_sha256"])
    cls = git("show", f"{commit}:{packet}/receipts/{d}/CLASSIFICATION.txt").decode().splitlines()
    chk(f"{name} {d}: classification first two lines", cls[:2], ["CLASSIFICATION: PASS", rr["outer_class"]])
    chk(f"{name} {d}: outer class is the {'composed-quotient' if quotient else 'original-explorer'} PASS", rr["outer_class"],
        "outer class (AD-1-R4 K.2): PASS — admissible unchanged-core " + ("composed-quotient PASS" if quotient else
                                                                          "original-explorer PASS"))
    desc = json.loads(git("show", f"{commit}:{packet}/receipts/{d}/descriptor.json"))
    chk(f"{name} {d}: argv", desc["argv"], [K.EXE, rr["descriptor_test"], "--exact", "--ignored", "--nocapture",
                                            "--test-threads=1"])
    ex = desc["executable"]
    before = ex["before"]["sha256"] if "before" in ex else ex.get("sha256_before")
    after = ex["after"]["sha256"] if "after" in ex else ex.get("sha256_after")
    chk(f"{name} {d}: executable before/after sha256 = the pinned baseline executable", (before, after),
        (K.EXE_SHA256,) * 2)
    chk(f"{name} {d}: source exactly C before/after (head, tree)",
        (desc["source_before"]["verdict"], desc["source_after"]["verdict"], desc["source_before"]["head"],
         desc["source_before"]["tree"]), ("EXACTLY_C", "EXACTLY_C", K.C, K.C_TREE))
    return tag


def section4(by):
    print("[4] contract, AD-1-R4/R5, Owner adoptions, the F-R proof, Stage-1 and Stage-2 evidence (mission 3, 5)")
    contract = git("show", f"{K.CONTRACT_COMMIT}:{K.CONTRACT_PATH}")
    chk(f"base contract sha256 (at {K.CONTRACT_COMMIT[:8]})", K.sha256_bytes(contract or b""), K.CONTRACT_SHA)
    copy_in("CONTRACT-02-R1.md", contract, K.CONTRACT_SHA)
    r4 = git("show", f"{K.C}:{K.AD1R4}")
    chk("AD-1-R4 sha256 (at C)", K.sha256_bytes(r4 or b""), K.AD1R4_SHA)
    copy_in("AD1-R4.md", r4, K.AD1R4_SHA)
    a4 = git("show", f"{K.T1}:{K.AD1R4_ADOPTION}")
    print(f"    info      AD-1-R4 Owner adoption record {K.AD1R4_ADOPTION}: sha256 {K.sha256_bytes(a4 or b'')}")
    chk("R5 proof evidence commit type", gtext("cat-file", "-t", K.R5_COMMIT), "commit")
    chk("R5 evidence commit: Candidate C is an ancestor",
        subprocess.run(["git", "-C", K.REPO, "merge-base", "--is-ancestor", K.C, K.R5_COMMIT]).returncode, 0)
    r5 = git("show", f"{K.R5_COMMIT}:{K.AD1R5}")
    chk("AD-1-R5 sha256 (at the R5 proof evidence commit)", K.sha256_bytes(r5 or b""), K.AD1R5_SHA)
    copy_in("AD1-R5.md", r5, K.AD1R5_SHA)
    proof = git("show", f"{K.R5_COMMIT}:{K.FR_PROOF}")
    chk("F-R stuttering-refinement proof blob (at the R5 evidence commit)", gtext("rev-parse", f"{K.R5_COMMIT}:{K.FR_PROOF}"),
        K.FR_PROOF_BLOB)
    chk("F-R stuttering-refinement proof sha256", K.sha256_bytes(proof or b""), K.FR_PROOF_SHA)
    chk("the proof and AD-1-R5 unchanged at the Tranche-1 evidence commit",
        (gtext("rev-parse", f"{K.T1}:{K.FR_PROOF}"), K.sha256_bytes(git("show", f"{K.T1}:{K.AD1R5}") or b"")),
        (K.FR_PROOF_BLOB, K.AD1R5_SHA))
    copy_in("FR-STUTTERING-REFINEMENT-PROOF.md", proof, K.FR_PROOF_SHA)
    r5sums = packet_sums(K.R5_COMMIT, K.R5_PACKET)
    badr5 = [p for p, d in r5sums.items() if K.sha256_bytes(git("show", f"{K.R5_COMMIT}:{K.R5_PACKET}/{p}") or b"") != d]
    chk(f"R5 packet SHA256SUMS: all {len(r5sums)} listed files verify", badr5, [])
    chk("R5 SHA256SUMS lists the proof and AD-1-R5 with their sha256",
        (r5sums.get("FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"), r5sums.get(
            "P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md")), (K.FR_PROOF_SHA, K.AD1R5_SHA))
    adopt = git("show", f"{K.AD1R5_ADOPTION_COMMIT}:{K.AD1R5_ADOPTION}")
    chk("AD-1-R5 Owner adoption record sha256 (= the catalog's adoption_record_sha256)", K.sha256_bytes(adopt or b""),
        K.AD1R5_ADOPTION_SHA)
    chk("AD-1-R5 Owner adoption record unchanged at the Tranche-1 evidence commit",
        K.sha256_bytes(git("show", f"{K.T1}:{K.AD1R5_ADOPTION}") or b""), K.AD1R5_ADOPTION_SHA)
    at = (adopt or b"").decode()
    chk("the adoption record adopts AD-1-R5 in full and records the F-R recording words and the proof sha256",
        ("On 2026-10-14 the Owner approved and adopted AD-1-R5 in full." in at, f"> {K.FR_RECORDING}" in at,
         K.FR_PROOF_SHA in at, K.AD1R5_SHA in at), (True, True, True, True))
    copy_in("AD1-R5-OWNER-ADOPTION.md", adopt, K.AD1R5_ADOPTION_SHA)
    chk("catalog cites AD-1-R4, AD-1-R5 and the adoption record", (cat_src(by)["ad1r4"]["sha256"],
                                                                  cat_src(by)["ad1r5"]["sha256"],
                                                                  cat_src(by)["ad1r5"]["adoption_record_sha256"]),
        (K.AD1R4_SHA, K.AD1R5_SHA, K.AD1R5_ADOPTION_SHA))
    chk("catalog cites the base contract sha256", cat_src(by)["contract"]["sha256"], K.CONTRACT_SHA)
    sums = {}
    for name, commit, packet, ref in (("Stage 1", K.STAGE1, K.STAGE1_PACKET, K.STAGE1_REF),
                                      ("Stage 2", K.STAGE2, K.STAGE2_PACKET, K.STAGE2_REF)):
        chk(f"{name} evidence commit type", gtext("cat-file", "-t", commit), "commit")
        chk(f"{name}: Candidate C is an ancestor",
            subprocess.run(["git", "-C", K.REPO, "merge-base", "--is-ancestor", K.C, commit]).returncode, 0)
        s = packet_sums(commit, packet)
        badp = [p for p, d in s.items() if K.sha256_bytes(git("show", f"{commit}:{packet}/{p}") or b"") != d]
        chk(f"{name} packet SHA256SUMS: all {len(s)} listed files verify", badp, [])
        same = [p for p in gtext("ls-tree", "-r", "--name-only", K.T1, packet + "/").splitlines()
                if gtext("rev-parse", f"{K.T1}:{p}") != gtext("rev-parse", f"{commit}:{p}")]
        chk(f"{name} packet byte-identical at the Tranche-1 evidence commit", same, [])
        sums[commit] = s
    print("  -- the unchanged-core receipts this tranche links (catalog restored_receipt; mission 5)")
    done = set()
    for cid in TRANCHE:
        e = by[cid]
        rr = e["restored_receipt"]
        cfg, _, _, _, link = K.MATRIX[cid]
        if link == "FR-DISCHARGE":
            f2 = rr["f2_receipt"]
            chk(f"{cid} restored side: the AD-1-R5 F discharge record (catalog type)", rr["type"],
                "AD-1-R5 F refinement discharge record (not a run, never PASS)")
            chk(f"{cid} catalog: the record's AD-1-R5 and proof sha256", (rr["ad1r5_sha256"], rr["proof_sha256"]),
                (K.AD1R5_SHA, K.FR_PROOF_SHA))
            chk(f"{cid} catalog: the record's F-2 receipt", (f2["commit"], f2["directory"]),
                (K.STAGE1, K.STAGE1_PACKET + "/receipts/41-full-F-2"))
            if "41-full-F-2" not in done:
                stage_receipt(f"{cid} F-2 (discharge basis)", K.STAGE1, K.STAGE1_PACKET, sums[K.STAGE1], "41-full-F-2",
                              dict(f2))
                done.add("41-full-F-2")
            stage_receipt(f"{cid} F-R receipt (INCOMPLETE, never PASS)", K.STAGE2, K.STAGE2_PACKET, sums[K.STAGE2],
                          "69-full-F-R", None)
            cls = git("show", f"{K.STAGE2}:{K.STAGE2_PACKET}/receipts/69-full-F-R/CLASSIFICATION.txt").decode().splitlines()
            chk(f"{cid} the F-R receipt stays INCOMPLETE (first line NOT PASS; outer class INCOMPLETE at the cap)",
                (cls[0], cls[1].startswith("outer class (AD-1-R4 K.2): INCOMPLETE (the state cap of 30000000 was "
                                           "reached")), ("CLASSIFICATION: NOT PASS", True))
            continue
        stage, d = link.split(":")
        commit, packet = (K.STAGE1, K.STAGE1_PACKET) if stage == "S1" else (K.STAGE2, K.STAGE2_PACKET)
        chk(f"{cid} catalog restored receipt (mission 5: {link})", (rr["commit"], rr["directory"]),
            (commit, f"{packet}/receipts/{d}"))
        if d not in done:
            stage_receipt(f"{cid} ({cfg})", commit, packet, sums[commit], d, rr, quotient=(cid == "C8"))
            done.add(d)
        else:
            print(f"    info      {cid}: {d} authenticated above (shared restored side)")
    print(f"    info      linked receipts authenticated and copied: {sorted(done)}")


def cat_src(by):
    return json.loads(open(K.inputs("catalog.json"), "rb").read())["sources"]


def section5():
    print("[5] pinned unchanged-baseline executable and its worktree (hashed and sized; never run here)")
    chk("executable is a regular file, not a symlink", (os.path.isfile(K.EXE), os.path.islink(K.EXE)), (True, False))
    chk("executable sha256", K.sha256_file(K.EXE), K.EXE_SHA256)
    chk("executable size", os.path.getsize(K.EXE), K.EXE_SIZE)
    vt = os.path.join(K.EV, "tools/verify_tree.py")
    r = subprocess.run([sys.executable, vt, K.BASE_WT], capture_output=True, text=True)
    d = json.loads(r.stdout)
    chk("baseline worktree exactly C (verify_tree.py, byte comparison of every tracked file)", d["verdict"], "EXACTLY_C")
    print(f"    info      {d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes; status {d['status_entries']}")


def section6():
    print("[6] refs, destination absence and mission-owned paths (mission 17)")
    refs = subprocess.run(["git", "-C", K.REPO, "ls-remote", K.REMOTE], capture_output=True, text=True).stdout
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in refs.splitlines() if "\t" in l}
    print(f"    info      remote refs {len(rm)}")
    chk(f"remote {K.R2_REF}", rm.get(K.R2_REF), K.R2)
    chk("remote Stage-1 review ref", rm.get(K.STAGE1_REF), K.STAGE1)
    chk("remote Stage-2 review ref", rm.get(K.STAGE2_REF), K.STAGE2)
    chk("remote R5 review ref", rm.get(K.R5_REF), K.R5_COMMIT)
    chk(f"remote {K.EXEC01_REF}", rm.get(K.EXEC01_REF), K.EXEC01)
    chk(f"remote {K.RESUME01_REF}", rm.get(K.RESUME01_REF), K.RESUME01)
    chk(f"remote {K.T1_REF} (the accepted Tranche-1 evidence)", rm.get(K.T1_REF), K.T1)
    chk(f"destination {K.DEST} absent on the remote", K.DEST in rm, False)
    chk(f"destination {K.DEST} absent locally", git("rev-parse", "--verify", "-q", K.DEST), None)
    if GATE:
        print(f"    info      local branch refs/heads/{K.LOCAL_BRANCH}: "
              f"{gtext('rev-parse', '--verify', '-q', 'refs/heads/' + K.LOCAL_BRANCH)}")
    else:
        chk(f"local branch refs/heads/{K.LOCAL_BRANCH} absent", git("rev-parse", "--verify", "-q",
                                                                   "refs/heads/" + K.LOCAL_BRANCH), None)
    for ref in ("refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"):
        print(f"    info      remote {ref}: {rm.get(ref)}")
    for c in K.ACCEPTED:
        p = K.NEXUS + "/nexus-os-p3-c11-mt1x-" + c
        print(f"    info      Tranche-1 worktree of {c}: {p} present {os.path.isdir(p)} (not touched by this mission)")
    paths = [K.EVIDENCE_WT] + [K.control_wt(c) for c in TRANCHE]
    for p in paths:
        if GATE:
            print(f"    info      mission-owned path {p}: present {os.path.lexists(p)}")
        else:
            chk(f"mission-owned path absent before creation: {p}", os.path.lexists(p), False)
    chk("no worktree for any control outside Tranche 2 in this mission's namespace",
        [c for c in K.NOT_AUTHORIZED + K.ACCEPTED if os.path.lexists(K.control_wt(c))], [])


def section7(by):
    print("[7] the accepted Tranche-1 evidence (mission 1, 6, 7): commit, packets, the twelve accepted controls, tools")
    chk("Tranche-1 evidence commit type", gtext("cat-file", "-t", K.T1), "commit")
    chk("Tranche-1 evidence commit tree", gtext("rev-parse", K.T1 + "^{tree}"), K.T1_TREE)
    chk("Tranche-1 evidence commit single parent (the RESUME-01 evidence commit)", gtext("log", "-1", "--format=%P", K.T1),
        K.T1_PARENT)
    ns = gtext("diff", "--name-status", "--no-renames", K.T1_PARENT, K.T1).splitlines()
    chk("every path the Tranche-1 evidence commit changes is added under the CONTINUE-02 packet",
        bool(ns) and all(l.startswith("A\t" + K.CONTINUE02_PACKET + "/") for l in ns), True)
    outside = [p for p in gtext("diff", "--name-only", K.C, K.T1).splitlines() if not p.startswith("docs/evidence/")]
    chk("paths outside docs/evidence/ differing between C and the Tranche-1 evidence commit", outside, [])
    data = {}
    for name, packet in K.T1_PACKETS:
        files = packet_files(K.T1, packet)
        sums = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in files["SHA256SUMS"].decode().splitlines() if l}
        chk(f"{name} packet: SHA256SUMS lists every packet file but itself", sorted(sums) ==
            sorted(f for f in files if f != "SHA256SUMS"), True)
        chk(f"{name} packet: all {len(sums)} listed files verify at {K.T1[:8]}",
            [p for p, d in sums.items() if K.sha256_bytes(files[p]) != d], [])
        print(f"    info      {name} packet: {len(files)} files, {sum(len(v) for v in files.values())} bytes; SHA256SUMS "
              f"sha256 {K.sha256_bytes(files['SHA256SUMS'])}")
        own = {"EXEC-01": K.EXEC01, "RESUME-01": K.RESUME01}.get(name)
        if own:
            diff = [p for p in files if gtext("rev-parse", f"{own}:{packet}/{p}") != gtext("rev-parse",
                                                                                        f"{K.T1}:{packet}/{p}")]
            chk(f"{name} packet byte-identical at its own evidence commit {own[:8]} and at {K.T1[:8]}", diff, [])
        data[name] = files
    names = {K.EXEC01_PACKET: "EXEC-01", K.RESUME01_PACKET: "RESUME-01", K.CONTINUE02_PACKET: "CONTINUE-02"}
    for cid in K.ACCEPTED:
        packet, d = K.T1_RECEIPTS[cid]
        f = data[names[packet]]
        e = by[cid]
        first = f[f"{d}/RESULT.txt"].decode().splitlines()[0]
        want = "CONTROL RESULT: MUTATION_CONTROL_BLOCKED" if cid == "C22" else CANDIDATE_RESULT
        chk(f"{cid}: {names[packet]} {d}/RESULT.txt first line (historical executor label, unchanged)", first, want)
        # C28's apply receipt is RESUME-01's (CONTINUE-02 admitted that build without re-applying the patch).
        af = data["RESUME-01"] if cid == "C28" else f
        ap = json.loads(af[f"{d}/02-apply/APPLY.json"]) if f"{d}/02-apply/APPLY.json" in af else None
        chk(f"{cid}: frozen patch, mutant blob and sha256 are the catalog's (its apply receipt)",
            (ap["patch_sha256"], ap["verify_mutant"]["mutant_file"]["git_blob"],
             ap["verify_mutant"]["mutant_file"]["sha256"]) if ap else None,
            (e["patch_sha256"], e["mutant_blob"], e["mutant_sha256"]))
    c2 = data["CONTINUE-02"]
    for rel, first in (("ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md",
                        "C22 C14 C19 C21 C27 — MUTATION CONTROL KILL ACCEPTED; C28 BUILD ARCHITECT-ADMISSIBLE (NARROW); "
                        "C28 KILL PENDING"),
                       ("ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md",
                        "C28 BUILD — ARCHITECT-ADMISSIBLE (NARROW C28-SPECIFIC RULING; NO GENERAL WARNING EXCEPTION)"),
                       ("publication-recovery/publish-04/documents/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md",
                        "DUPLICATE POLICY R2 — 21 EXACT NONEMPTY DUPLICATES AND 8 EXACT ZERO-LENGTH ARTIFACTS PERMITTED "
                        "FOR PUBLICATION (NO SIZE THRESHOLD; NO WILDCARD; CANDIDATE ACCEPTANCE PENDING)")):
        chk(f"CONTINUE-02 {rel} first line", c2[rel].decode().splitlines()[0], first)
    chk("RESUME-01 ARCHITECT-RULING-C22-ACCEPTANCE.md first line",
        data["RESUME-01"]["ARCHITECT-RULING-C22-ACCEPTANCE.md"].decode().splitlines()[0],
        "C22 — MUTATION CONTROL KILL ACCEPTED")
    chk("CONTINUE-02 INDEX.md: the derived Tranche-1 status", [l for l in c2["INDEX.md"].decode().splitlines()
                                                              if l.startswith("Derived status: ")][0].split(". ")[0],
        "Derived status: **READY_FOR_MUTATION_TRANCHE1_FINAL_ARCHITECT_REVIEW**")
    # The accepted Tranche-1 execution tools (this mission's original-tools/) and publication tools.
    c2tools = {f[len("tools/"):]: K.sha256_bytes(v) for f, v in c2.items() if f.startswith("tools/")}
    r1tools = {f[len("tools/"):]: K.sha256_bytes(v) for f, v in data["RESUME-01"].items() if f.startswith("tools/")}
    accepted = dict({t: h for t, h in r1tools.items() if t not in c2tools}, **c2tools)
    orig = {f: K.sha256_file(os.path.join(K.EV, "original-tools", f)) for f in
            sorted(os.listdir(os.path.join(K.EV, "original-tools")))}
    chk(f"original-tools/ ({len(orig)}) = the accepted Tranche-1 execution tools: CONTINUE-02's tools/ and the RESUME-01 "
        "tools it ran unchanged (names and sha256)", orig, accepted)
    cf = c2["CARRIED-FORWARD.md"].decode()
    chk("CONTINUE-02 CARRIED-FORWARD.md names every RESUME-01 tool it ran unchanged, with its sha256",
        all(f"| `{t}` |" in cf and h in cf for t, h in r1tools.items() if t not in c2tools), True)
    p4 = "publication-recovery/publish-04/tools/"
    pub = {f[len(p4):]: K.sha256_bytes(v) for f, v in c2.items() if f.startswith(p4)}
    opub = {f: K.sha256_file(os.path.join(K.EV, "original-publication-tools", f)) for f in
            sorted(os.listdir(os.path.join(K.EV, "original-publication-tools")))}
    chk(f"original-publication-tools/ ({len(opub)}) = the PUBLISH-04 publication tools (names and sha256)", opub, pub)


def main():
    print(f"{K.MISSION} — INPUT AUTHENTICATION (read-only)")
    print("executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission effort: High")
    print()
    cat, listed = section1_2()
    print()
    by = section3(cat, listed)
    print()
    section4(by)
    print()
    section5()
    print()
    section6()
    print()
    section7(by)
    print()
    print("RESULT: " + ("ALL CHECKS HOLD" if not fail else "MISMATCH — STOP"))
    return fail


if __name__ == "__main__":
    sys.exit(main())
