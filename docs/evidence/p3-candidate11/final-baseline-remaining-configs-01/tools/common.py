"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01: frozen identities and small shared helpers.

Identities only (no analysis logic): the two static vocabulary checks share these constants and nothing else.
Sources are always read from git objects of frozen Candidate C, never from the worktree.
"""
import hashlib
import subprocess

MISSION = "P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-remaining-configs-01"
REPO = "/home/nexus/NEXUS/nexus-os"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"

C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
CRATE_SRC = "crates/nexus-governed-control/src"
PROTOCOL = CRATE_SRC + "/ledger/protocol.rs"
MODEL = CRATE_SRC + "/ledger/protocol/model_tests.rs"
UNIT = CRATE_SRC + "/ledger/protocol/unit_tests.rs"
BLOBS = {PROTOCOL: "a2df2841b99cad5d452008a6d4150e9369d07e66",
         MODEL: "28085ce232827301104b5d757af042ec933976f8",
         UNIT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}

EXE = WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
EXE_SHA256 = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
EXE_SIZE = 8984464

STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
STAGE1_TREE = "646c4fe9fa96bfee7b4f731a38f20ebfff14239e"
STAGE1_PARENT = "0622a4d042b1dfd3160f06fc5d333e131e2a60c8"
STAGE1_REF = "refs/heads/review/p3-c11-d2a1-final-baseline-known-runs-01"
STAGE1_PACKET = "docs/evidence/p3-candidate11/final-baseline-known-runs-01"

PACKET = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01"
DEST_REF = "refs/heads/review/p3-c11-d2a1-final-baseline-remaining-configs-01"
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"

AD1R4 = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
AD1R4_SHA256 = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
CT1 = "docs/evidence/p3-candidate11/current-tree-183/manifest.json"
CT1_BLOB = "5d31cb45123403626f06c521dd2d53103676250e"
C4_SCHEDULE = ("a464a36621523f539d2554a6b176d678c7f885ea",
               "docs/evidence/p3-candidate11/c4-witness-01/frozen/c4-schedule.json",
               "d616492ff708a1a836f114424a2cce8ed6c0771d22d3f59e984322b7fa3f536b")

TEST_PREFIX = "ledger::protocol::model_tests::"


def git(*args, repo=REPO, check=True):
    return subprocess.run(["git", "-C", repo, *args], capture_output=True, check=check).stdout


def blob(commit, path):
    return git("cat-file", "blob", f"{commit}:{path}")


def source(path):
    """A source file of frozen C, as text, from the git object (never the worktree)."""
    return blob(C, path).decode("utf-8")


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()
