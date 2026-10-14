#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 3, 4, 5 and 14: authenticate every input (read-only).

Every identity is compared with the value the mission states (or, for the blocked packet's files, with the value the
authenticated blocked catalog records); any mismatch prints MISMATCH and the exit status is 1. Reads git objects,
sealed local evidence outside the repository, the remote's ref listing and the pinned baseline executable (hashed
and sized, never run). Writes only ROOT/inputs/ (byte copies of the authenticated inputs, each re-hashed after
writing).

    authenticate.py ROOT
"""
import hashlib
import json
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
# The evidence commit's required parent (mission 14): the blocked catalog's evidence commit.
PARENT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
PARENT_TREE = "12d5dd2405c59ce9a83c225c0e42b6585d8da107"
PARENT_PARENT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"
BLOCKED_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-01"
BLOCKED_DIR = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-01/"
BLOCKED_CATALOG_SHA = "a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747"
CARRIED = ["C1", "C2", "C4", "C9", "C10", "C12", "C14", "C19", "C21", "C22", "C24", "C25", "C27", "C28"]
R5_ADOPTION = "docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md"
R5_ADOPTION_BLOB = "1492e185cb475bd32d95079a39b49078c0e23ddc"
R5_ADOPTION_SHA = "9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4"
R4_ADOPTION = "docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md"
R4_ADOPTION_BLOB = "54c956bc1324dd3a8fd97439faa815f0865b404d"
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
R5_PATH = "docs/evidence/p3-candidate11/fr-stutter-r5-01/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
R5_SHA = "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473"
R5_BLOB = "ae3cabdb45627f9b3179a098c50d27e50c55101f"
PROOF_PATH = "docs/evidence/p3-candidate11/fr-stutter-r5-01/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"
PROOF_SHA = "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b"
DEFS_LOCAL = AUD + "/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py"
DEFS_SHA = "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"
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
DEST = "refs/heads/review/p3-c11-d2a1-control-definitions-r1-01"
REVIEW_REFS = {BLOCKED_REF: PARENT,
               "refs/heads/review/p3-c11-d2a1-fr-stutter-r5-01": PARENT_PARENT,
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
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as fh:
        fh.write(data)
    chk(f"input copy inputs/{name} sha256", sha(open(path, "rb").read()), expected_sha)


def anc(a, b):
    return subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", a, b]).returncode


def main(root):
    os.makedirs(os.path.join(root, "inputs"), exist_ok=True)
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01 — INPUT AUTHENTICATION (read-only)")
    print("executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission effort: High")
    print()
    print("[1] frozen baseline Candidate C (mission 3)")
    chk("type", gtext("cat-file", "-t", C), "commit")
    chk("tree", gtext("rev-parse", C + "^{tree}"), C_TREE)
    chk("parents (single)", gtext("log", "-1", "--format=%P", C), C_PARENT)
    for rel, blob in BLOBS.items():
        chk(f"{rel} blob at C", gtext("rev-parse", f"{C}:{SRC}{rel}"), blob)
        chk(f"{rel} at the evidence parent equals C's", gtext("rev-parse", f"{PARENT}:{SRC}{rel}"), blob)
    for rel, blob in BLOBS.items():
        data = git("cat-file", "blob", blob)
        copy_in(root, "C-" + rel.replace("/", "-"), data, sha(data))
    print()
    print("[2] the blocked catalog's evidence commit, the required evidence parent (mission 1, 14)")
    chk("type", gtext("cat-file", "-t", PARENT), "commit")
    chk("tree", gtext("rev-parse", PARENT + "^{tree}"), PARENT_TREE)
    chk("single parent", gtext("log", "-1", "--format=%P", PARENT), PARENT_PARENT)
    chk("Candidate C is an ancestor", anc(C, PARENT), 0)
    chk("Stage-1 evidence commit is an ancestor", anc(STAGE1, PARENT), 0)
    chk("Stage-2 evidence commit is an ancestor", anc(STAGE2, PARENT), 0)
    outside = [p for p in gtext("diff", "--name-only", C, PARENT).splitlines() if not p.startswith("docs/evidence/")]
    chk("paths outside docs/evidence/ differing between C and the evidence parent", outside, [])
    added = gtext("diff", "--name-only", PARENT_PARENT, PARENT).splitlines()
    chk("the blocked commit adds only its packet and the R5 adoption record",
        all(p.startswith(BLOCKED_DIR) or p == R5_ADOPTION for p in added) and len(added) == 45, True)
    cat = git("show", f"{PARENT}:{BLOCKED_DIR}catalog/catalog.json")
    chk("blocked catalog.json sha256", sha(cat), BLOCKED_CATALOG_SHA)
    copy_in(root, "blocked/catalog.json", cat, BLOCKED_CATALOG_SHA)
    bc = json.loads(cat)
    chk("blocked catalog version", bc["version"], "d2a1-controls-v1")
    chk("blocked catalog status", bc["catalog_status"], "CONTROL_CATALOG_BLOCKED")
    chk("blocked catalog READY set equals the mission's 14 (mission 5)", sorted(bc["ready"]), sorted(CARRIED))
    chk("blocked catalog BLOCKED count", len(bc["blocked"]), 21)
    sums = git("show", f"{PARENT}:{BLOCKED_DIR}SHA256SUMS").decode()
    listed = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}
    bad = [p for p, d in listed.items() if sha(git("show", f"{PARENT}:{BLOCKED_DIR}{p}") or b"") != d]
    chk(f"blocked packet SHA256SUMS: all {len(listed)} listed files verify at {PARENT[:8]}", bad, [])
    by = {e["id"]: e for e in bc["controls"]}
    for cid in CARRIED:
        data = git("show", f"{PARENT}:{BLOCKED_DIR}catalog/patches/{cid}.patch")
        chk(f"blocked patch {cid}.patch sha256 equals its catalog record", sha(data), by[cid]["patch_sha256"])
        chk(f"blocked patch {cid}.patch equals its catalog patch text", data == by[cid]["patch"].encode(), True)
        chk(f"blocked {cid} listed in SHA256SUMS", listed.get(f"catalog/patches/{cid}.patch"), sha(data))
        copy_in(root, f"blocked/patches/{cid}.patch", data, by[cid]["patch_sha256"])
    print()
    print("[3] contract, AD-1-R4, AD-1-R5 and its Owner-adoption record, the F-R proof (mission 4)")
    contract = git("show", f"{R3C}:{CONTRACT_PATH}")
    chk(f"contract sha256 (git, at {R3C[:8]})", sha(contract), CONTRACT_SHA)
    chk("contract sha256 (sealed local copy)", sha(open(CONTRACT_LOCAL, "rb").read()), CONTRACT_SHA)
    copy_in(root, "CONTRACT-02-R1.md", contract, CONTRACT_SHA)
    r4 = git("show", f"{PARENT}:{R4_PATH}")
    chk("AD-1-R4 blob (at the evidence parent)", gtext("rev-parse", f"{PARENT}:{R4_PATH}"), R4_BLOB)
    chk("AD-1-R4 sha256", sha(r4), R4_SHA)
    chk("AD-1-R4 Owner-adoption record blob", gtext("rev-parse", f"{PARENT}:{R4_ADOPTION}"), R4_ADOPTION_BLOB)
    copy_in(root, "AD1-R4.md", r4, R4_SHA)
    r5 = git("show", f"{PARENT}:{R5_PATH}")
    chk("AD-1-R5 blob (at the evidence parent)", gtext("rev-parse", f"{PARENT}:{R5_PATH}"), R5_BLOB)
    chk("AD-1-R5 sha256", sha(r5), R5_SHA)
    copy_in(root, "AD1-R5.md", r5, R5_SHA)
    adopt = git("show", f"{PARENT}:{R5_ADOPTION}")
    chk("R5 Owner-adoption record blob", gtext("rev-parse", f"{PARENT}:{R5_ADOPTION}"), R5_ADOPTION_BLOB)
    chk("R5 Owner-adoption record sha256", sha(adopt), R5_ADOPTION_SHA)
    t = adopt.decode()
    chk("R5 adoption record names AD-1-R5's sha256, AD-1-R4's sha256 and the F-R proof's sha256",
        R5_SHA in t and R4_SHA in t and PROOF_SHA in t, True)
    chk("R5 adoption record: 'approved and adopted AD-1-R5 in full'", "approved and adopted AD-1-R5 in full" in t,
        True)
    copy_in(root, "R5-OWNER-ADOPTION.md", adopt, R5_ADOPTION_SHA)
    proof = git("show", f"{PARENT}:{PROOF_PATH}")
    chk("accepted supporting F-R proof sha256", sha(proof), PROOF_SHA)
    print()
    print("[4] the historical partial controls file (mission 4)")
    defs = open(DEFS_LOCAL, "rb").read()
    chk("controls_defs.py sha256", sha(defs), DEFS_SHA)
    seal = open(AUD + "/p3-c11-dr01-d2a1-campaign-01/SHA256SUMS").read()
    chk("controls_defs.py listed in CAMPAIGN-01's seal",
        seal.count(f"{DEFS_SHA}  tools/stage5-prepared-NOT-EXECUTED/controls_defs.py"), 1)
    r = subprocess.run(["sha256sum", "--quiet", "-c", "SHA256SUMS"], cwd=AUD + "/p3-c11-dr01-d2a1-campaign-01",
                       capture_output=True, text=True)
    chk("CAMPAIGN-01 seal verifies (sha256sum -c)", r.returncode, 0)
    copy_in(root, "controls_defs.py.txt", defs, DEFS_SHA)
    print()
    print("[5] documentary patches cited by AD-1-R4 G.2 (C1, C25, C4)")
    for name, (commit, path, digest) in PATCHES.items():
        data = git("show", f"{commit}:{path}")
        chk(f"{name} sha256 (at {commit[:8]})", sha(data) if data is not None else None, digest)
        copy_in(root, name, data, digest)
    for cid, name in (("C1", "C1-mutation.patch"), ("C25", "C25-narrow-mutation.patch"),
                      ("C4", "C4-test1-removed.patch")):
        chk(f"blocked {cid}.patch is the documentary {name}", by[cid]["patch_sha256"], PATCHES[name][2])
    print()
    print("[6] pinned unchanged-baseline executable (hashed and sized only; never run)")
    data = open(EXE, "rb").read()
    chk("executable sha256", sha(data), EXE_SHA)
    chk("executable size", len(data), EXE_SIZE)
    print()
    print("[7] remote and local refs (git ls-remote github; read-only)")
    refs = subprocess.run(["git", "-C", REPO, "ls-remote", "github"], capture_output=True, text=True).stdout
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in refs.splitlines() if "\t" in l}
    print(f"    info      remote refs {len(rm)}")
    for ref, want in REVIEW_REFS.items():
        chk(f"remote {ref}", rm.get(ref), want)
    chk(f"destination {DEST} absent on the remote", DEST in rm, False)
    chk(f"destination {DEST} absent locally", git("rev-parse", "--verify", "-q", DEST), None)
    for ref in ("refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"):
        print(f"    info      remote {ref}: {rm.get(ref)}")
    print()
    print("RESULT: " + ("ALL CHECKS OK" if fail == 0 else "MISMATCH — STOP"))
    return fail


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
