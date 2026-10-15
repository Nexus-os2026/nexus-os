"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (from CONTINUE-02's common.py, itself from RESUME-01's and EXEC-01's):
frozen identities and small shared helpers.

Identities only (no analysis logic). Every value here is the mission's (sections 1-5, 8, 9, 16, 17) or the frozen R2
catalog's. Sources are read from git objects of frozen Candidate C, never from a worktree.

The module also provides the names that the reused tools import from `common`, with their accepted Tranche-1 values:
supervise.py, classify.py, worktrees.py (EV, MISSION, C, C_TREE, BLOBS, PROTOCOL, REPO, GIT_COMMON_DIR, CRATE, EXE,
EXE_SHA256, EXE_SIZE, BASE_WT, BUILD_ARGV, BUILD_LIMIT, RUN_ENV, MEMORY_MAX, NEED_KIB, LISTED, UNIT_PREFIX, build_env)
and the Stage-2 configs.py and report.py (MODEL, TEST_PREFIX, STAGE1, STAGE1_PACKET, git, blob, source).
"""
import hashlib
import os
import subprocess

MISSION = "P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche2-exec-01"
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

# Frozen catalog (mission section 3) and its evidence commit / review ref.
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

# Governing contract and evidence (catalog sources; mission section 5).
CONTRACT_COMMIT = "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf"
CONTRACT_PATH = ("docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/"
                 "contract/P3-C11-D2A1-CONTRACT-02-R1.md")
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
AD1R4 = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
AD1R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
AD1R4_ADOPTION = "docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md"
R5_COMMIT = "c1e46fc404a74ed76b848b709a7a246b59f3347f"
R5_PACKET = "docs/evidence/p3-candidate11/fr-stutter-r5-01"
AD1R5 = R5_PACKET + "/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md"
AD1R5_SHA = "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473"
FR_PROOF = R5_PACKET + "/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md"
FR_PROOF_BLOB = "c631a061dd7206b9bc62822711be0eca2a21c223"
FR_PROOF_SHA = "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b"
AD1R5_ADOPTION = "docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md"
AD1R5_ADOPTION_SHA = "9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4"
AD1R5_ADOPTION_COMMIT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
FR_RECORDING = "F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
STAGE1_PACKET = "docs/evidence/p3-candidate11/final-baseline-known-runs-01"
STAGE1_REF = "refs/heads/review/p3-c11-d2a1-final-baseline-known-runs-01"
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"
STAGE2_PACKET = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01"
STAGE2_REF = "refs/heads/review/p3-c11-d2a1-final-baseline-remaining-configs-01"
R5_REF = "refs/heads/review/p3-c11-d2a1-fr-stutter-r5-01"

# Accepted pinned unchanged-baseline executable (the linked receipts' executable) and its worktree (Stage 1's, exactly C).
BASE_WT = NEXUS + "/nexus-os-p3-c11-final-baseline-known-runs-01"
EXE = BASE_WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
EXE_SHA256 = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_SIZE = 8984464

# Mission-owned worktrees (created by this mission only; absent before).
EVIDENCE_WT = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-tranche2-exec-01-evidence"


def control_wt(cid):
    return NEXUS + "/nexus-os-p3-c11-mt2x-" + cid


# Tranche 2 (mission sections 2, 4, 15): the exact execution order, and every control not authorized.
ORDER = ["C23", "C10", "C16", "C26", "C3a", "C5", "A3", "A6", "C8", "C15"]
ACCEPTED = ["C22", "C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
NOT_AUTHORIZED = ["C1", "C2", "C3b", "C3c", "C4", "C6", "C7", "C20", "C24", "C25", "A1", "A2", "A5"]
KILL_REQUIRED = 35
# The mission's control matrix (section 4), stated independently of the catalog and compared with it (the catalog is
# the authority; a disagreement stops the mission): configuration, permitted printed properties, state ceiling, mutated
# file, restored-side link.
MATRIX = {
    "C23": ("F-2", ["E1"], 30000000, PROTOCOL, "S1:41-full-F-2"),
    "C10": ("F-1", ["E4"], 60000000, PROTOCOL, "S1:45-full-F-1"),
    "C16": ("F-D", ["E3"], 30000000, PROTOCOL, "S2:68-full-F-D"),
    "C26": ("F-4", ["E9"], 30000000, PROTOCOL, "S2:62-full-F-4"),
    "C3a": ("F-7", ["P13", "P15"], 30000000, PROTOCOL, "S2:67-full-F-7"),
    "C5": ("F-5", ["P13"], 30000000, PROTOCOL, "S2:65-full-F-5"),
    "A3": ("F-5", ["P12"], 30000000, MODEL, "S2:65-full-F-5"),
    "A6": ("F-5", ["P12", "P15"], 30000000, MODEL, "S2:65-full-F-5"),
    "C8": ("F-B", ["E5"], 30000000, PROTOCOL, "S1:42-full-F-B"),
    "C15": ("F-R", ["E8"], 30000000, PROTOCOL, "FR-DISCHARGE"),
}
# A6 (mission section 4; catalog R2): the printed key fields must name recovery_required.
A6_KEY_TEXT = "the test's table (classes, debt counter, H, A, recovery_required)"

# Build policy (mission sections 8 and 9): identical for every mutant build, and identical to Tranche 1's.
BUILD_ARGV = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run", "-j", "2"]
BUILD_LIMIT = 900
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
MEMORY_MAX = "17179869184"
NEED_KIB = 18 * 1024 * 1024
UNIT_PREFIX = "p3c11-mt2x"
LISTED = 348  # tests the pinned baseline executable lists at C (Stage-1 receipts/20-list-all)


def build_env(target):
    return [("HOME", "/home/nexus"), ("PATH", "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin"),
            ("CARGO_TARGET_DIR", target), ("CARGO_TERM_VERBOSE", "true"), ("CARGO_NET_OFFLINE", "true")]


# The accepted Tranche-1 evidence (mission sections 1 and 6): immutable; read from git objects, never copied.
T1 = "ebe42009ba1bd262cf31ae2f401e02cec101afa7"
T1_TREE = "7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400"
T1_PARENT = "3695a817a6190781dc66769a72a487b442d3e6da"
T1_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02"
EXEC01 = "19eda8cc7f559054933fed4a93bdb0a18cc725d4"
EXEC01_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01"
EXEC01_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-exec-01"
RESUME01 = "3695a817a6190781dc66769a72a487b442d3e6da"
RESUME01_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01"
RESUME01_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche1-resume-01"
CONTINUE02_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-continue-02"
T1_PACKETS = [("EXEC-01", EXEC01_PACKET), ("RESUME-01", RESUME01_PACKET), ("CONTINUE-02", CONTINUE02_PACKET)]
# Where each accepted control's receipts are (packet, receipt directory).
T1_RECEIPTS = {"C22": (EXEC01_PACKET, "receipts/31-C22"), "C14": (RESUME01_PACKET, "receipts/32-C14"),
               "C19": (RESUME01_PACKET, "receipts/33-C19"), "C21": (RESUME01_PACKET, "receipts/34-C21"),
               "C27": (RESUME01_PACKET, "receipts/35-C27"), "C28": (CONTINUE02_PACKET, "receipts/36-C28"),
               "C12": (CONTINUE02_PACKET, "receipts/37-C12"), "C9": (CONTINUE02_PACKET, "receipts/38-C9"),
               "C11": (CONTINUE02_PACKET, "receipts/39-C11"), "C17": (CONTINUE02_PACKET, "receipts/40-C17"),
               "C18a": (CONTINUE02_PACKET, "receipts/41-C18a"), "C18b": (CONTINUE02_PACKET, "receipts/42-C18b")}

# The Architect ruling record and the campaign status record of this mission (written from mission section 1).
RULING = "documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md"
RULING_FIRST = ("TRANCHE 1 — C22 C14 C19 C21 C27 C28 C12 C9 C11 C17 C18a C18b — MUTATION CONTROL KILL ACCEPTED "
                "(12 OF 35); A4 SATISFIED — ARCHITECT ACCEPTED")
CAMPAIGN = "documents/CAMPAIGN-STATUS-BEFORE-TRANCHE2.md"
CAMPAIGN_FIRST = "CAMPAIGN STATUS BEFORE TRANCHE 2 — 12 OF 35 KILL-REQUIRED CONTROLS ARCHITECT-ACCEPTED"

# Publication (mission section 17).
PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/"
BRANCH = "review/p3-c11-d2a1-mutation-tranche2-exec-01"
DEST = "refs/heads/" + BRANCH
LOCAL_BRANCH = "evidence/p3-c11-d2a1-mutation-tranche2-exec-01"
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"
PACKET_LIMIT = 48 * 1024 * 1024

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
