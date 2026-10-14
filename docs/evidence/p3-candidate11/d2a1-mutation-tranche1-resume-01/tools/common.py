"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (from EXEC-01's common.py): frozen identities and small shared helpers.

Identities only (no analysis logic). Every value here is the mission's (sections 3, 4, 9, 19, 20) or the frozen R2
catalog's. Sources are read from git objects of frozen Candidate C, never from a worktree.

The module also provides the names that the reused Stage-2 tools (configs.py, normalize.py, report.py, copied byte for
byte from evidence commit 89326b85) import from `common`: EV, MODEL, TEST_PREFIX, STAGE1, STAGE1_PACKET, git, blob,
source.
"""
import hashlib
import os
import subprocess

MISSION = "P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-resume-01"
REPO = "/home/nexus/NEXUS/nexus-os"
GIT_COMMON_DIR = REPO + "/.git"
NEXUS = "/home/nexus/NEXUS"

# Frozen engineering baseline (mission section 3).
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
CRATE = "crates/nexus-governed-control"
CRATE_SRC = CRATE + "/src"
PROTOCOL = CRATE_SRC + "/ledger/protocol.rs"
MODEL = CRATE_SRC + "/ledger/protocol/model_tests.rs"
UNIT = CRATE_SRC + "/ledger/protocol/unit_tests.rs"
BLOBS = {PROTOCOL: "a2df2841b99cad5d452008a6d4150e9369d07e66",
         MODEL: "28085ce232827301104b5d757af042ec933976f8",
         UNIT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}

# Frozen catalog (mission section 1) and its evidence commit / review ref.
R2 = "9f22f890d8532df395f22cd8db8db2552bfdd14c"
R2_TREE = "435f242243907eb7fd3e745669926193f11c5dcf"
R2_PARENT = "f81159046bf0b62666851bc1185992318302d31f"
R2_REF = "refs/heads/review/p3-c11-d2a1-control-definitions-r2-01"
R2_DIR = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/"
CATALOG_PATH = R2_DIR + "catalog/catalog.json"
CATALOG_BLOB = "1918374263740d2f98640da00fc466737945975e"
CATALOG_SHA = "d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83"
CATALOG_VERSION = "d2a1-controls-v1-r2-candidate"
DEFS_PATH = R2_DIR + "ARCHITECT-CONTROL-DEFINITIONS-R2.md"
DEFS_BLOB = "c79b75b65b85e1fad53592e92ab2d8507d6c8799"
DEFS_SHA = "1da1ae92f313496dade2bfea5c508af1e17bf2e39b9bd11cca3e448c35f0cb26"

# Governing contract and evidence (mission section 4).
CONTRACT_COMMIT = "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf"
CONTRACT_PATH = ("docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/"
                 "contract/P3-C11-D2A1-CONTRACT-02-R1.md")
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
AD1R4 = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
AD1R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
R5_COMMIT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"
AD1R5 = "docs/evidence/p3-candidate11/fr-stutter-r5-01/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
AD1R5_SHA = "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
STAGE1_PACKET = "docs/evidence/p3-candidate11/final-baseline-known-runs-01"
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"
STAGE2_PACKET = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01"

# Accepted pinned unchanged-baseline executable (mission section 4) and its worktree (Stage 1's, exactly C).
BASE_WT = NEXUS + "/nexus-os-p3-c11-final-baseline-known-runs-01"
EXE = BASE_WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
EXE_SHA256 = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_SIZE = 8984464

# Mission-owned worktrees (created by this mission only; absent before).
HEALTHY_WT = NEXUS + "/nexus-os-p3-c11-mt1x-healthy"
EVIDENCE_WT = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-tranche1-resume-01-evidence"
DEV_WT = NEXUS + "/nexus-os-p3-c11-mt1r-dev-verifier"


def control_wt(cid):
    return NEXUS + "/nexus-os-p3-c11-mt1x-" + cid


# Tranche (mission sections 2 and 9): execution order.
ORDER = ["C22", "C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
NOT_AUTHORIZED = ["C1", "C2", "C3a", "C3b", "C3c", "C4", "C5", "C6", "C7", "C8", "C10", "C15", "C16", "C20", "C23",
                  "C24", "C25", "C26", "A1", "A2", "A3", "A5", "A6"]
# Resume mission sections 1, 4 and 11: C22 is Architect-accepted and never executed again; the eleven remaining
# controls run in this order. ORDER (above) is unchanged: it fixes every control's receipt directory and worktree.
ACCEPTED = ["C22"]
RESUME_ORDER = ["C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
C22_RULING = "documents/ARCHITECT-RULING-C22-ACCEPTANCE.md"
C22_RULING_FIRST = "C22 — MUTATION CONTROL KILL ACCEPTED"
NAMED = {"C14": "T-BIND", "C19": "T-CELL", "C21": "W6", "C27": "T-PREC", "C28": "T-RES"}
EXPLORE_CFG = {"C12": "X-1", "C9": "F-B2", "C11": "F-B2", "C17": "F-B2", "C18a": "F-G", "C18b": "F-G"}

# Build policy (mission sections 8, 11, 12): identical for the healthy build and every mutant build.
BUILD_ARGV = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run", "-j", "2"]
BUILD_LIMIT = 900
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
MEMORY_MAX = "17179869184"
NEED_KIB = 18 * 1024 * 1024
UNIT_PREFIX = "p3c11-mt1x"
LISTED = 348  # tests the pinned baseline executable lists at C (Stage-1 receipts/20-list-all)


def build_env(target):
    return [("HOME", "/home/nexus"), ("PATH", "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin"),
            ("CARGO_TARGET_DIR", target), ("CARGO_TERM_VERBOSE", "true"), ("CARGO_NET_OFFLINE", "true")]


# The previous execution evidence (resume mission section 2): immutable history; its accepted receipts are carried
# forward (authenticated byte copies under EV, never republished) and referenced by commit, path, blob and sha256.
PREV_MISSION = "P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01"
PREV = "19eda8cc7f559054933fed4a93bdb0a18cc725d4"
PREV_TREE = "1b313ec5c23b5bf76db90b5fa89758d9cd880a75"
PREV_PARENT = R2
PREV_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-exec-01"
PREV_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01"
PREV_EVIDENCE_WT = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-tranche1-exec-01-evidence"
PREV_DEV_WT = NEXUS + "/nexus-os-p3-c11-mt1x-dev-rehearsal"
CARRIED = ["receipts/02-EXPECTATIONS.json", "receipts/10-baseline-X-1", "receipts/11-baseline-T-BIND",
           "receipts/12-baseline-T-CELL", "receipts/13-baseline-W6", "receipts/14-baseline-T-PREC",
           "receipts/15-baseline-T-RES", "receipts/20-healthy-build", "receipts/31-C22"]
CARRIED_DOCS = {"documents/ARCHITECT-RULING-CATALOG-FREEZE.md": "ARCHITECT-RULING-CATALOG-FREEZE.md"}

# Publication (resume mission section 20).
PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/"
BRANCH = "review/p3-c11-d2a1-mutation-tranche1-resume-01"
DEST = "refs/heads/" + BRANCH
LOCAL_BRANCH = "evidence/p3-c11-d2a1-mutation-tranche1-resume-01"
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"
PACKET_LIMIT = 32 * 1024 * 1024

TEST_PREFIX = "ledger::protocol::model_tests::"
UNIT_PREFIX_TEST = "ledger::protocol::unit_tests::"


def git(*args, repo=REPO, check=True):
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, check=check).stdout


def blob(commit, path):
    return git("cat-file", "blob", f"{commit}:{path}")


def source(path):
    """A source file of frozen C, as text, from the git object (never a worktree)."""
    return blob(C, path).decode("utf-8")


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


def inputs(*parts):
    return os.path.join(EV, "inputs", *parts)
