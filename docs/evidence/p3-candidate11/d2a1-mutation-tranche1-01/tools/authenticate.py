#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01, mission sections 3, 4, 7 and 8: authenticate every input (read-only).

Every identity is compared with the mission's stated value; any mismatch prints MISMATCH and the exit status is 1.
Reads git objects, sealed local evidence outside the repository and the pinned baseline executable (hashed, never
run). Writes only ROOT/inputs/ (byte copies of the authenticated inputs, each re-hashed after writing).

    authenticate.py ROOT
"""
import hashlib
import os
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
AUD = "/home/nexus/nexus-audits/claude-architect"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
SRC = "crates/nexus-governed-control/src/ledger/"
BLOBS = {"protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
         "protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
         "protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
MANIFEST = "docs/evidence/p3-candidate11/current-tree-183/manifest.json"
MANIFEST_BLOB = "5d31cb45123403626f06c521dd2d53103676250e"
MANIFEST_SHA = "20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279"
PARENT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"   # the evidence commit's required parent; AD-1-R5's evidence commit
PARENT_TREE = "ecc6914ade5378068eceaa08e88fa45a0bec35b6"
PARENT_PARENT = "89326b859792a442c1853c3d35d30a48301c009e"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"
R3C = "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf"
AD1R2C = "8693e3a742ee4d3917cbfdb809b377bcd919d9f4"
B = "a464a36621523f539d2554a6b176d678c7f885ea"
CONTRACT_PATH = ("docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/"
                 "contract/P3-C11-D2A1-CONTRACT-02-R1.md")
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
CONTRACT_LOCAL = AUD + "/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/P3-C11-D2A1-CONTRACT-02-R1.md"
R4_PATH = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
R4_BLOB = "35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1"
R4_ADOPTION = "docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md"
R4_ADOPTION_BLOB = "54c956bc1324dd3a8fd97439faa815f0865b404d"
R5_PATH = "docs/evidence/p3-candidate11/fr-stutter-r5-01/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
R5_SHA = "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473"
R5_BLOB = "ae3cabdb45627f9b3179a098c50d27e50c55101f"
PROOF_PATH = "docs/evidence/p3-candidate11/fr-stutter-r5-01/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"
PROOF_SHA = "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b"
DEFS_LOCAL = AUD + "/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py"
DEFS_SHA = "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"
DEFS_README = AUD + "/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/README.txt"
PATCHES = {  # documentary patches the adopted definitions cite (AD-1-R4 G.2)
    "C1-mutation.patch": (AD1R2C, "docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/"
                          "p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C1-mutation.patch",
                          "5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252"),
    "C25-narrow-mutation.patch": (AD1R2C, "docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/"
                                  "p3-c11-dr01-d2a1-ad1r1-witness-audit-01/patches/C25-narrow-mutation.patch",
                                  "29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732"),
    "C4-test1-removed.patch": (B, "docs/evidence/p3-candidate11/c4-witness-01/patches/C4-test1-removed.patch",
                               "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d"),
}
EXE = ("/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/"
       "nexus_governed_control-2faa0b9718d3e6a3")
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_SIZE = 8984464
BASE_WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"
DEST = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-01"
REVIEW_REFS = {"refs/heads/review/p3-c11-d2a1-fr-stutter-r5-01": PARENT,
               "refs/heads/review/p3-c11-d2a1-final-baseline-known-runs-01": STAGE1,
               "refs/heads/review/p3-c11-d2a1-final-baseline-remaining-configs-01": STAGE2}

fail = 0


def git(*args):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True)
    if r.returncode != 0:
        return None
    return r.stdout


def gtext(*args):
    out = git(*args)
    return None if out is None else out.decode().strip()


def sha(b):
    return hashlib.sha256(b).hexdigest()


def chk(label, actual, expected):
    global fail
    if actual == expected:
        print(f"    OK        {label}: {actual}")
    else:
        print(f"    MISMATCH  {label}: actual={actual} expected={expected}")
        fail = 1


def copy_in(root, name, data, expected_sha):
    path = os.path.join(root, "inputs", name)
    with open(path, "wb") as fh:
        fh.write(data)
    chk(f"input copy inputs/{name} sha256", sha(open(path, "rb").read()), expected_sha)


def main(root):
    os.makedirs(os.path.join(root, "inputs"), exist_ok=True)
    print("P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01 — INPUT AUTHENTICATION (read-only)")
    print("executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission effort: High")
    print()
    print("[1] frozen Candidate C (mission 3)")
    chk("type", gtext("cat-file", "-t", C), "commit")
    chk("tree", gtext("rev-parse", C + "^{tree}"), C_TREE)
    chk("parents", gtext("log", "-1", "--format=%P", C), C_PARENT)
    for rel, blob in BLOBS.items():
        chk(f"{rel} blob", gtext("rev-parse", f"{C}:{SRC}{rel}"), blob)
    chk("current-tree manifest blob at C", gtext("rev-parse", f"{C}:{MANIFEST}"), MANIFEST_BLOB)
    man = git("show", f"{C}:{MANIFEST}")
    chk("current-tree manifest sha256", sha(man), MANIFEST_SHA)
    import json
    chk("current-tree manifest version", json.loads(man).get("version"), "ct-1")
    chk("manifest at the evidence parent equals C's", gtext("rev-parse", f"{PARENT}:{MANIFEST}"), MANIFEST_BLOB)
    for rel, blob in BLOBS.items():
        chk(f"{rel} at the evidence parent equals C's", gtext("rev-parse", f"{PARENT}:{SRC}{rel}"), blob)
    for rel, blob in BLOBS.items():
        copy_in(root, "C-" + rel.replace("/", "-"), git("cat-file", "blob", blob), sha(git("cat-file", "blob", blob)))
    print()
    print("[2] evidence lineage (mission 4, 18)")
    chk("evidence parent type", gtext("cat-file", "-t", PARENT), "commit")
    chk("evidence parent tree", gtext("rev-parse", PARENT + "^{tree}"), PARENT_TREE)
    chk("evidence parent's single parent", gtext("log", "-1", "--format=%P", PARENT), PARENT_PARENT)
    chk("Stage-1 evidence commit type", gtext("cat-file", "-t", STAGE1), "commit")
    chk("Stage-2 evidence commit type", gtext("cat-file", "-t", STAGE2), "commit")
    chk("Stage-1 is an ancestor of the evidence parent",
        subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", STAGE1, PARENT]).returncode, 0)
    chk("Stage-2 is an ancestor of the evidence parent",
        subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", STAGE2, PARENT]).returncode, 0)
    chk("Candidate C is an ancestor of the evidence parent",
        subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", C, PARENT]).returncode, 0)
    outside = [p for p in gtext("diff", "--name-only", C, PARENT).splitlines() if not p.startswith("docs/evidence/")]
    chk("paths outside docs/evidence/ differing between C and the evidence parent", outside, [])
    print()
    print("[3] contract, AD-1-R4, AD-1-R5, the F-R proof (mission 1, 7)")
    contract = git("show", f"{R3C}:{CONTRACT_PATH}")
    chk(f"contract sha256 (git, at {R3C[:8]})", sha(contract), CONTRACT_SHA)
    chk("contract sha256 (sealed local copy)", sha(open(CONTRACT_LOCAL, "rb").read()), CONTRACT_SHA)
    copy_in(root, "CONTRACT-02-R1.md", contract, CONTRACT_SHA)
    r4 = git("show", f"{PARENT}:{R4_PATH}")
    chk("AD-1-R4 blob", gtext("rev-parse", f"{PARENT}:{R4_PATH}"), R4_BLOB)
    chk("AD-1-R4 sha256", sha(r4), R4_SHA)
    chk("AD-1-R4 Owner-adoption record blob", gtext("rev-parse", f"{PARENT}:{R4_ADOPTION}"), R4_ADOPTION_BLOB)
    copy_in(root, "AD1-R4.md", r4, R4_SHA)
    r5 = git("show", f"{PARENT}:{R5_PATH}")
    chk("AD-1-R5 blob (at its evidence commit c1e46fc4)", gtext("rev-parse", f"{PARENT}:{R5_PATH}"), R5_BLOB)
    chk("AD-1-R5 sha256", sha(r5), R5_SHA)
    copy_in(root, "AD1-R5.md", r5, R5_SHA)
    proof = git("show", f"{PARENT}:{PROOF_PATH}")
    chk("accepted supporting F-R proof sha256", sha(proof), PROOF_SHA)
    print(f"    info      F-R proof path {PROOF_PATH}; blob {gtext('rev-parse', PARENT + ':' + PROOF_PATH)}")
    print()
    print("[4] historical sealed controls definition file (mission 7)")
    defs = open(DEFS_LOCAL, "rb").read()
    chk("controls_defs.py sha256", sha(defs), DEFS_SHA)
    seal = open(AUD + "/p3-c11-dr01-d2a1-campaign-01/SHA256SUMS").read()
    chk("controls_defs.py listed in CAMPAIGN-01's seal",
        seal.count(f"{DEFS_SHA}  tools/stage5-prepared-NOT-EXECUTED/controls_defs.py"), 1)
    r = subprocess.run(["sha256sum", "--quiet", "-c", "SHA256SUMS"], cwd=AUD + "/p3-c11-dr01-d2a1-campaign-01",
                       capture_output=True, text=True)
    chk("CAMPAIGN-01 seal verifies (sha256sum -c)", r.returncode, 0)
    copy_in(root, "controls_defs.py.txt", defs, DEFS_SHA)
    readme = open(DEFS_README, "rb").read()
    print(f"    info      the sealed README beside it (sha256 {sha(readme)}), verbatim:")
    for line in readme.decode().splitlines():
        print(f"              | {line}")
    copy_in(root, "controls_defs.README.txt", readme, sha(readme))
    print()
    print("[5] documentary patches cited by the adopted definitions (AD-1-R4 G.2)")
    for name, (commit, path, digest) in PATCHES.items():
        data = git("show", f"{commit}:{path}")
        chk(f"{name} sha256 (at {commit[:8]})", sha(data) if data is not None else None, digest)
        copy_in(root, name, data, digest)
    print()
    print("[6] pinned unchanged-baseline executable (hashed and sized, never run) and its worktree")
    data = open(EXE, "rb").read()
    chk("executable sha256", sha(data), EXE_SHA)
    chk("executable size", len(data), EXE_SIZE)
    chk("baseline worktree HEAD", subprocess.run(["git", "-C", BASE_WT, "rev-parse", "HEAD"], capture_output=True,
                                                 text=True).stdout.strip(), C)
    chk("baseline worktree porcelain entries",
        len(subprocess.run(["git", "-C", BASE_WT, "status", "--porcelain"], capture_output=True,
                           text=True).stdout.splitlines()), 0)
    print()
    print("[7] remote state (git ls-remote github; read-only)")
    refs = subprocess.run(["git", "-C", REPO, "ls-remote", "github"], capture_output=True, text=True).stdout
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in refs.splitlines() if "\t" in l}
    print(f"    info      remote refs {len(rm)}; sha256 of the sorted listing "
          f"{sha(chr(10).join(sorted(refs.splitlines())).encode())}")
    for ref, want in REVIEW_REFS.items():
        chk(f"remote {ref}", rm.get(ref), want)
    chk(f"destination {DEST} present on the remote", DEST in rm, False)
    for ref in ("refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"):
        print(f"    info      remote {ref}: {rm.get(ref)}")
    print()
    print("RESULT: " + ("ALL CHECKS OK" if fail == 0 else "MISMATCH — STOP"))
    return fail


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
