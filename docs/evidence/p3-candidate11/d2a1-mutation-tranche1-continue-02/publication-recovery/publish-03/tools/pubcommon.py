"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03: identities and small helpers of the publication-recovery tools.

Identities only (no analysis logic). Every value is the mission's (sections 3, 4, 6, 12, 14) or a frozen identity of the
CONTINUE-02 evidence root, which every tool here treats as read-only. Nothing here executes a build, a test or an
executable.
"""
import hashlib
import os
import stat
import subprocess

MISSION = "P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03"
SRC_MISSION = "P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02"
SRC = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-continue-02"
P = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-publish-03"
REPO = "/home/nexus/NEXUS/nexus-os"
NEXUS = "/home/nexus/NEXUS"

# Authenticated repository inputs (mission section 4).
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
R2 = "9f22f890d8532df395f22cd8db8db2552bfdd14c"
R2_TREE = "435f242243907eb7fd3e745669926193f11c5dcf"
R2_PACKET = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01"
CATALOG_PATH = R2_PACKET + "/catalog/catalog.json"
CATALOG_BLOB = "1918374263740d2f98640da00fc466737945975e"
CATALOG_SHA = "d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83"
EXEC01 = "19eda8cc7f559054933fed4a93bdb0a18cc725d4"
EXEC01_TREE = "1b313ec5c23b5bf76db90b5fa89758d9cd880a75"
EXEC01_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01"
EXEC01_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-exec-01"
RESUME01 = "3695a817a6190781dc66769a72a487b442d3e6da"
RESUME01_TREE = "a5fce69d5f60b0b86f3982aa655d5a33f4ef9ea4"
RESUME01_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01"
RESUME01_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-resume-01"
R2_REF = "refs/heads/review/p3-c11-d2a1-control-definitions-r2-01"
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"
STAGE2_PACKET = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01"
PARENT = RESUME01
PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02"
BRANCH = "review/p3-c11-d2a1-mutation-tranche1-continue-02"
DEST = "refs/heads/" + BRANCH
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"
WORKTREE = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-tranche1-publish-03"
OLD_WORKTREE = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-tranche1-continue-02-evidence"
LIMIT = 32 * 1024 * 1024

# The seven current candidates (the CONTINUE-02 order) and the previously Architect-accepted controls.
CANDIDATES = ["C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
ACCEPTED = ["C22", "C14", "C19", "C21", "C27"]
CANDIDATE_RESULT = "CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"

def git(*args, repo=REPO, check=True):
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, check=check).stdout


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def git_blob_id(data):
    """The git blob id of bytes, computed in-process (never written to any object store)."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def kind(path):
    """File type of one entry, by lstat and content (no executable is run): symlink, dir, or regular:{elf,text,binary}."""
    st = os.lstat(path)
    if stat.S_ISLNK(st.st_mode):
        return "symlink"
    if stat.S_ISDIR(st.st_mode):
        return "dir"
    if not stat.S_ISREG(st.st_mode):
        return "other"
    with open(path, "rb") as f:
        b = f.read()
    if b[:4] == b"\x7fELF":
        return "regular:elf"
    if b"\0" in b:
        return "regular:binary"
    try:
        b.decode("utf-8")
    except UnicodeDecodeError:
        return "regular:binary"
    return "regular:text"
