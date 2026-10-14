#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission sections 3, 9.A, 9.B, 13 and 14: authenticate every input
(read-only).

Every identity is compared with the value the mission states (or, for the R1 packet's files, with the value the
authenticated R1 catalog and the R1 packet's SHA256SUMS record); any mismatch prints MISMATCH and the exit status is
1. Reads git objects, the remote's ref listing and the pinned baseline executable (hashed and sized, never run).
Writes only ROOT/inputs/ (byte copies of the authenticated inputs, each re-hashed after writing).

    authenticate.py ROOT
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
SRC = "crates/nexus-governed-control/src/ledger/"
BLOBS = {"protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
         "protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
         "protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
# The R1 evidence commit: the R2 evidence commit's required parent (mission 1, 13).
R1 = "f81159046bf0b62666851bc1185992318302d31f"
R1_TREE = "234ca0990434485f59ebda0ceff3086d4e66d797"
R1_PARENT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
R1_PARENT_TREE = "12d5dd2405c59ce9a83c225c0e42b6585d8da107"
R1_REF = "refs/heads/review/p3-c11-d2a1-control-definitions-r1-01"
R1_DIR = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/"
R1_CATALOG_BLOB = "56d4424397be7740cefd8d89b3530aab4b25d3ed"
R1_CATALOG_SHA = "a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479"
R1_DEFS_BLOB = "b947621bb0d56e1e14780853e886c7e832d7f6a3"
R1_DEFS_SHA = "beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0"
R1_FILES = 72
BLOCKED_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-01"
BLOCKED_DIR = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-01/"
BLOCKED_CATALOG_SHA = "a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747"
CONTRACT_COMMIT = "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf"
CONTRACT_PATH = ("docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/"
                 "contract/P3-C11-D2A1-CONTRACT-02-R1.md")
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
R4_PATH = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
R5_PATH = "docs/evidence/p3-candidate11/fr-stutter-r5-01/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
R5_SHA = "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473"
R5_ADOPTION = "docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md"
R5_ADOPTION_SHA = "9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4"
EXE = ("/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/"
       "nexus_governed_control-2faa0b9718d3e6a3")
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_SIZE = 8984464
DEST = "refs/heads/review/p3-c11-d2a1-control-definitions-r2-01"
IDS = ["C1", "C2", "C3a", "C3b", "C3c", "C4", "C5", "C6", "C7", "C8", "C9", "C10", "C11", "C12", "C15", "C16", "C17",
       "C18a", "C18b", "C20", "C23", "C24", "C25", "C26", "C14", "C19", "C21", "C22", "C27", "C28", "A1", "A2", "A3",
       "A5", "A6"]

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
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01 — INPUT AUTHENTICATION (read-only)")
    print("executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission effort: High")
    print()
    print("[1] frozen baseline Candidate C (mission 3, 9.A)")
    chk("type", gtext("cat-file", "-t", C), "commit")
    chk("tree", gtext("rev-parse", C + "^{tree}"), C_TREE)
    chk("parents (single)", gtext("log", "-1", "--format=%P", C), C_PARENT)
    for rel, blob in BLOBS.items():
        chk(f"{rel} blob at C", gtext("rev-parse", f"{C}:{SRC}{rel}"), blob)
        chk(f"{rel} at the R1 evidence commit equals C's", gtext("rev-parse", f"{R1}:{SRC}{rel}"), blob)
    for rel, blob in BLOBS.items():
        data = git("cat-file", "blob", blob)
        chk(f"{rel} content hashes to its blob", subprocess.run(["git", "hash-object", "--stdin"], input=data,
                                                                capture_output=True).stdout.decode().strip(), blob)
        copy_in(root, "C-" + rel.replace("/", "-"), data, sha(data))
    print()
    print("[2] the R1 evidence commit, the required evidence parent (mission 1, 9.B, 13)")
    chk("type", gtext("cat-file", "-t", R1), "commit")
    chk("tree", gtext("rev-parse", R1 + "^{tree}"), R1_TREE)
    chk("single parent", gtext("log", "-1", "--format=%P", R1), R1_PARENT)
    chk("the R1 parent's tree", gtext("rev-parse", R1_PARENT + "^{tree}"), R1_PARENT_TREE)
    chk("Candidate C is an ancestor", anc(C, R1), 0)
    outside = [p for p in gtext("diff", "--name-only", C, R1).splitlines() if not p.startswith("docs/evidence/")]
    chk("paths outside docs/evidence/ differing between C and the R1 evidence commit", outside, [])
    st = gtext("diff", "--name-status", R1_PARENT, R1).splitlines()
    chk(f"the R1 commit adds only its packet ({R1_FILES} files, all added)",
        len(st) == R1_FILES and all(l.startswith("A\t" + R1_DIR) for l in st), True)
    chk("R1 catalog.json blob", gtext("rev-parse", f"{R1}:{R1_DIR}catalog/catalog.json"), R1_CATALOG_BLOB)
    cat = git("show", f"{R1}:{R1_DIR}catalog/catalog.json")
    chk("R1 catalog.json sha256", sha(cat), R1_CATALOG_SHA)
    copy_in(root, "r1/catalog.json", cat, R1_CATALOG_SHA)
    chk("R1 definitions record blob", gtext("rev-parse", f"{R1}:{R1_DIR}ARCHITECT-CONTROL-DEFINITIONS-R1.md"),
        R1_DEFS_BLOB)
    defs = git("show", f"{R1}:{R1_DIR}ARCHITECT-CONTROL-DEFINITIONS-R1.md")
    chk("R1 definitions record sha256", sha(defs), R1_DEFS_SHA)
    copy_in(root, "r1/ARCHITECT-CONTROL-DEFINITIONS-R1.md", defs, R1_DEFS_SHA)
    sums = git("show", f"{R1}:{R1_DIR}SHA256SUMS").decode()
    listed = {l.split("  ", 1)[1]: l.split("  ", 1)[0] for l in sums.splitlines() if l}
    bad = [p for p, d in listed.items() if sha(git("show", f"{R1}:{R1_DIR}{p}") or b"") != d]
    chk(f"R1 packet SHA256SUMS: all {len(listed)} listed files verify at {R1[:8]}", bad, [])
    chk("R1 SHA256SUMS lists every packet file except itself", len(listed), R1_FILES - 1)
    chk("R1 SHA256SUMS lists the R1 catalog with its sha256", listed.get("catalog/catalog.json"), R1_CATALOG_SHA)
    chk("R1 SHA256SUMS lists the R1 definitions record with its sha256",
        listed.get("ARCHITECT-CONTROL-DEFINITIONS-R1.md"), R1_DEFS_SHA)
    rc = json.loads(cat)
    chk("R1 catalog version", rc["version"], "d2a1-controls-v1-r1-candidate")
    chk("R1 catalog status", rc["catalog_status"], "CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING")
    chk("R1 catalog counts", rc["counts"], {"architect_defined": 21, "blocked": 0, "candidate_ready": 35,
                                            "carried_forward": 14, "comparison": 1, "kill_required": 35})
    by = {e["id"]: e for e in rc["controls"]}
    chk("R1 kill-required ids (contract order, A4 excluded)", [e["id"] for e in rc["controls"] if e["id"] != "A4"],
        IDS)
    for cid in IDS:
        data = git("show", f"{R1}:{R1_DIR}catalog/patches/{cid}.patch")
        chk(f"R1 patch {cid}.patch sha256 equals its catalog record", sha(data), by[cid]["patch_sha256"])
        chk(f"R1 patch {cid}.patch equals its catalog patch text", data == by[cid]["patch"].encode(), True)
        chk(f"R1 {cid} listed in R1 SHA256SUMS", listed.get(f"catalog/patches/{cid}.patch"), sha(data))
        copy_in(root, f"r1/patches/{cid}.patch", data, by[cid]["patch_sha256"])
    s6 = git("show", f"{R1}:{R1_DIR}tools/section6.txt")
    chk("R1 tools/section6.txt (mission R1 section 6 as received) listed in R1 SHA256SUMS",
        listed.get("tools/section6.txt"), sha(s6))
    copy_in(root, "r1/section6.txt", s6, listed.get("tools/section6.txt"))
    print()
    print("[3] blocked catalog, contract, AD-1-R4, AD-1-R5 (sources the R1 catalog cites; read by the checkers)")
    b = git("show", f"{R1_PARENT}:{BLOCKED_DIR}catalog/catalog.json")
    chk("blocked catalog.json sha256 (at the R1 parent)", sha(b), BLOCKED_CATALOG_SHA)
    copy_in(root, "blocked/catalog.json", b, BLOCKED_CATALOG_SHA)
    contract = git("show", f"{CONTRACT_COMMIT}:{CONTRACT_PATH}")
    chk(f"contract sha256 (at {CONTRACT_COMMIT[:8]})", sha(contract), CONTRACT_SHA)
    copy_in(root, "CONTRACT-02-R1.md", contract, CONTRACT_SHA)
    r4 = git("show", f"{R1}:{R4_PATH}")
    chk("AD-1-R4 sha256 (at the R1 commit)", sha(r4), R4_SHA)
    copy_in(root, "AD1-R4.md", r4, R4_SHA)
    r5 = git("show", f"{R1}:{R5_PATH}")
    chk("AD-1-R5 sha256 (at the R1 commit)", sha(r5), R5_SHA)
    copy_in(root, "AD1-R5.md", r5, R5_SHA)
    adopt = git("show", f"{R1}:{R5_ADOPTION}")
    chk("AD-1-R5 Owner-adoption record sha256", sha(adopt), R5_ADOPTION_SHA)
    copy_in(root, "R5-OWNER-ADOPTION.md", adopt, R5_ADOPTION_SHA)
    chk("R1 catalog cites these sources' sha256",
        (rc["sources"]["ad1r4"]["sha256"], rc["sources"]["ad1r5"]["sha256"], rc["supersedes_as_candidate"]["sha256"]),
        (R4_SHA, R5_SHA, BLOCKED_CATALOG_SHA))
    print()
    print("[4] pinned unchanged-baseline executable (hashed and sized only; never run)")
    data = open(EXE, "rb").read()
    chk("executable sha256", sha(data), EXE_SHA)
    chk("executable size", len(data), EXE_SIZE)
    print()
    print("[5] remote and local refs (git ls-remote github; read-only)")
    refs = subprocess.run(["git", "-C", REPO, "ls-remote", "github"], capture_output=True, text=True).stdout
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in refs.splitlines() if "\t" in l}
    print(f"    info      remote refs {len(rm)}")
    chk(f"remote {R1_REF} (the R1 review ref)", rm.get(R1_REF), R1)
    chk(f"remote {BLOCKED_REF}", rm.get(BLOCKED_REF), R1_PARENT)
    chk(f"destination {DEST} absent on the remote", DEST in rm, False)
    chk(f"destination {DEST} absent locally", git("rev-parse", "--verify", "-q", DEST), None)
    for ref in ("refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"):
        print(f"    info      remote {ref}: {rm.get(ref)}")
    print()
    print("RESULT: " + ("ALL CHECKS OK" if fail == 0 else "MISMATCH — STOP"))
    return fail


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
