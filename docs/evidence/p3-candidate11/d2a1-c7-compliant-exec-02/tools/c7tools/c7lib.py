"""C7-W compliant execution: shared helpers and frozen identities (mission P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02).
Adapted from the EXEC-01 helper of the same name; every change is listed in ../TOOLS-PROVENANCE.txt."""
import hashlib
import json
import os
import subprocess
import sys
import time

REPO = "/home/nexus/NEXUS/nexus-os"
REMOTE = "github"
MISSION = "P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02"

C_BASE = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
W_COMMIT = "6c424c3aec579401543ae010e3f8729e17c4ed98"
W_TREE = "5cc7ae59a6a5ef38b5630e76849a1f5b468ac4a4"
W_BLOB = "c18a6235a9eb32314931c51dd7bb574df77fbe28"
W_BLOB_SHA = "6762d91f3e4de61902efb8f32eccf38fcf4166f5c036ba7a2832f05944b0be25"
W_BLOB_SIZE = 329569
W_PATCH_SHA = "c8509c025542c81fdc44e296a0da256e04ba533a971dfbe1b379ee6f9f76b0a9"
W_REF = "refs/heads/review/p3-c11-d2a1-c7-witness-source-w1-01"

K1_BLOB = "1918374263740d2f98640da00fc466737945975e"
K1_SHA = "d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83"
K1_PATH = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json"
K1_COMMITS = ["c10681b3", "452b3cb5"]
K1_PATCH_PATH = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/patches/C7.patch"
K1_PATCH_BLOB = "df0dd5846f9003da1c46642dac6673882085f661"

K2_COMMIT = "16e74908e922160993a726e71e45374f1e5ff8a1"
K2_TREE = "dbfdceca15feb58e0be6d4e143a1fa9a4f33e66f"
K2_BLOB = "cb3a208e1d79a7e94e2ae0001e208a2b6436bc46"
K2_SHA = "989490ec9272a528fe0185ad0dc9f41e3aed1e414d2eb4a88346197dc60b3f3b"
K2_PATH = "docs/evidence/p3-candidate11/d2a1-c7-adc7w-final-freeze-01/AD-C7-W-FROZEN-REVIEW.md"
K2_REF = "refs/heads/review/p3-c11-d2a1-c7-adc7w-final-freeze-01"

K3_SHA = "c5a9f3d1ade663ca4ef57143c5ae5a1392296d9a05bf226888ba99df2ca44af7"
K3_LINES = 36
K3_BYTES = 1736

# K4: the NEW Owner approval record (pinned by the Sole Architect, ruling P3-C11-DR01-D2A1-C7-K4-AUTHENTICATION-01)
K4_COMMIT = "2148c5a524b6979dde2eb63b6d1442e39dcfb960"
K4_TREE = "55b5118646fe3a311fc0364868ef05f9760a1012"
K4_PARENT = "643e54baa1f551cba48e9f96df2ac44dac9201a4"
K4_BLOB = "cfcbf9982add53dee8f0b4d9c6b634474b4e072b"
K4_SHA = "4b74b90a73b7f93d804859afb8bf3d18d0d457603984995f5c1d3ae8a39392e2"
K4_PATH = "docs/evidence/p3-candidate11/c7-owner-identity-reauthorization-2026-10-16/OWNER-DECISION-2026-10-16.md"
K4_REF = "refs/heads/review/p3-c11-d2a1-c7-owner-identity-approval-01"
K4_CONVEYING_MISSION = "P3-C11-DR01-D2A1-C7-OWNER-IDENTITY-COMPLETION-02"

# Historical identities: preserved, never relied on as authority, never retroactively validated
HIST_K4_COMMIT = "4c29b2f0035ecaa86bca8d49238331485a1140e6"
HIST_K4_BLOB = "f9ce488c5db8fba8e3f0277334446b11d423c4de"
HIST_K4_SHA = "800f767d4df66f6a8c1c30306395042ff62a22bc3a354349d813ffc100066a54"
HIST_K4_PATH = "docs/evidence/p3-candidate11/d2a1-c7-adc7w-owner-adoption-01/OWNER-ADOPTION-2026-10-15.md"
HIST_K4_REF = "refs/heads/review/p3-c11-d2a1-c7-adc7w-owner-adoption-01"
HIST_EXEC01_COMMIT = "643e54baa1f551cba48e9f96df2ac44dac9201a4"
HIST_EXEC01_REF = "refs/heads/review/p3-c11-d2a1-c7-adc7w-evidence-exec-01"
HIST_EXEC01_EVDIR = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01"
HIST_WT_HEALTHY = "/home/nexus/NEXUS/nexus-os-p3-c11-c7w-healthy"
HIST_WT_MUTANT = "/home/nexus/NEXUS/nexus-os-p3-c11-c7w-mutant"

PATCH_SHA = "b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4"
PROTOCOL_PATH = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL_TESTS_PATH = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UNIT_TESTS_PATH = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
PROTOCOL_C_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
PROTOCOL_C_SHA = "a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296"
MUTANT_BLOB = "0d4ab9929c0e15d910735a52b72a5f3855bef58a"
MUTANT_SHA = "6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa"
MODEL_TESTS_C_BLOB = "28085ce232827301104b5d757af042ec933976f8"
MODEL_TESTS_C_SIZE = 310612
MODEL_TESTS_C_SHA = "2e93834f795f50842d5b124612895c4a150946130d2c789dac816ac36f6b6972"
UNIT_TESTS_C_BLOB = "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"

E_C_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
E_C_SIZE = 8984464
HIST_C7_SHA = "b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5"

PROTECTED_MAIN = "6e15dee613ea35823d1adbf6d8917ed59eeda583"
PROTECTED_REBUILD = "f727f5c39fab8d5c729a55eb28ad576d3d56ce47"
DEST_REF = "refs/heads/review/p3-c11-d2a1-c7-compliant-exec-02"
DEST_PARENT = "2148c5a524b6979dde2eb63b6d1442e39dcfb960"
DEST_PACKET = "docs/evidence/p3-candidate11/d2a1-c7-compliant-exec-02"

WT_HEALTHY = "/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-healthy"
WT_MUTANT = "/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-mutant"
EV_DIR = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-compliant-exec-02"

# Mission section 5: before each expensive operation, free space minus its projected growth must stay >= 16 GiB.
DISK_FLOOR_BYTES = 16 * 2**30
DISK_PATH = "/home/nexus/NEXUS"
PROJECTED_BUILD_BYTES = 3 * 2**30  # measured historically ~0.9 GiB per build; 3 GiB is the conservative projection
PROJECTED_INVOKE_BYTES = 512 * 2**20

BUILD_INPUTS = [
    "rust-toolchain.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/nexus-governed-control/Cargo.toml",
    ".cargo/config.toml",
    "crates/nexus-governed-control/src/lib.rs",
    "crates/nexus-governed-control/src/ledger.rs",
    PROTOCOL_PATH,
    UNIT_TESTS_PATH,
]

TEST_WITNESS = "ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt"
TEST_FULL_FPT = "ledger::protocol::model_tests::full_fpt"
TEST_FULL_F3 = "ledger::protocol::model_tests::full_f3"


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def utcns():
    """UTC timestamp with nanoseconds, for the sequence log."""
    t = time.time_ns()
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t // 10**9)) + ".%09dZ" % (t % 10**9)


def disk_free(path=None):
    st = os.statvfs(path or DISK_PATH)
    return st.f_bavail * st.f_frsize


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def git_blob_id(data):
    """Git blob id computed in-process; nothing is written to the object store."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def git(*args, cwd=REPO, check=True, input=None):
    env = dict(os.environ)
    env["GIT_OPTIONAL_LOCKS"] = "0"
    env["LC_ALL"] = "C"
    p = subprocess.run(["git", "-C", cwd, *args], capture_output=True, input=input, env=env)
    if check and p.returncode != 0:
        raise RuntimeError("git %s failed rc=%d: %s" % (" ".join(args), p.returncode, p.stderr.decode("utf-8", "replace")))
    return p


def git_out(*args, cwd=REPO):
    return git(*args, cwd=cwd).stdout.decode("utf-8", "replace").strip()


def blob_at(commit, path, cwd=REPO):
    return git("cat-file", "blob", "%s:%s" % (commit, path), cwd=cwd).stdout


def blob_id_at(commit, path, cwd=REPO):
    return git_out("rev-parse", "%s:%s" % (commit, path), cwd=cwd)


class Rec:
    """Gate record: every check is printed and retained; any FAIL means STOP."""

    def __init__(self, title):
        self.lines = []
        self.fails = []
        self.log("# %s" % title)
        self.log("# mission %s" % MISSION)
        self.log("# started %s" % now())

    def log(self, s=""):
        print(s, flush=True)
        self.lines.append(s)

    def check(self, cid, desc, ok, detail=""):
        status = "PASS" if ok else "FAIL"
        line = "[%s] %s: %s" % (status, cid, desc)
        if detail != "":
            line += " -- %s" % detail
        self.log(line)
        if not ok:
            self.fails.append(cid)
        return ok

    def info(self, cid, text):
        self.log("[INFO] %s: %s" % (cid, text))

    def write(self, path):
        with open(path, "x", encoding="utf-8") as f:
            f.write("\n".join(self.lines) + "\n")


EC_EVID = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
EC_RECEIPTS = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts"
EC_LIST_ALL_SHA = "fe5126990e77f9715ca46be82e7cdbc375eb0f3c3b4f9e5a63ef36ae570435a5"
EC_LIST_IGN_SHA = "4b13105f6ff6ebba618100e093d1fde445da6b5ed396006272b2761ea5a1ae1e"
