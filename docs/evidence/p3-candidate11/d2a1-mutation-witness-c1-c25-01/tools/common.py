"""P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (from TRANCHE3-TAIL-01's common.py, itself from TRANCHE3-CONTINUE-02's,
TRANCHE3-EXEC-01's, TRANCHE2-CONTINUE-02's, Tranche 2's and Tranche 1's): frozen identities and small shared helpers.

WITNESS-C1-C25-01: the mission identity, its evidence root and worktree namespace (mtw; scope prefix p3c11-mtw; every
control a fresh worktree), the order C1 C25 (receipt directories 81-82), the 31 Architect-accepted controls (A1 A2 C20 A5
accepted from the Tranche-3 tail), C7 MUTATION_CONTROL_BLOCKED (unchanged), C4 the one reserved control (NOT_AUTHORIZED),
the two-control matrix, the AD-1-R4 G.3 bindings of C1 and C25 as the mission states them (G3: the predicted first
failure, never counted as the named property; the witness test, its exact positive line and locus, its header and the
line of other properties; the accepted healthy type-(e3) receipt), the Tranche-3 tail evidence (e41c84b1) with its four
accepted controls' identities, the two records of this mission and its publication names (parent e41c84b1). Every
earlier name keeps its value (the earlier missions' history below is unchanged text); the tail's own two records keep
their texts under RULING_T3T and CAMPAIGN_T3T.

TRANCHE3-TAIL-01: the mission identity, its evidence root and worktree namespace (mt3t; scope prefix p3c11-mt3t; every
control a fresh worktree, no preserved one), the order A1 A2 C20 A5 (receipt directories 81-84), the 27 Architect-
accepted controls (C24 C3b C3c C6 accepted from the Tranche-3 continuation), C7 MUTATION_CONTROL_BLOCKED by the
Architect (BLOCKED: never executed, rebuilt, rerun or reinterpreted here), the four-control matrix, the Tranche-3
continuation evidence (c10681b3) with the accepted controls' and C7's blocked evidence identities, the two records of
this mission and its publication names (parent c10681b3). Every earlier name keeps its value (the earlier missions'
history below is unchanged text); the continuation's two own records keep their texts under RULING_T3C and
CAMPAIGN_T3C.

TRANCHE3-CONTINUE-02: the mission identity, its evidence root and worktree namespace (mt3c; scope prefix p3c11-mt3c), the
order C24 C3b C3c C6 C7 A1 A2 C20 A5 (receipt directories 81-89), the 23 Architect-accepted controls (C2 accepted from
Tranche 3), the Tranche-3 stopped evidence (774f457a) with C2's accepted evidence identities, the preserved C24 build
and executable (mission sections 2, 8, 9: never rebuilt; the worktree restored to C), the narrow C24 warning ruling and
the C3b/C3c build rules (mission sections 3-5 as amended by AMENDMENT-01: a clean build, or exactly the one predicted
warning group; permitted, not required), the records of this mission and its publication names (parent 774f457a).
Every earlier name keeps its value (the earlier missions' history below is unchanged text).

TRANCHE3-EXEC-01: the mission identity, its evidence root and worktree namespace (mt3x; scope prefix p3c11-mt3x), the
order C2 C24 C3b C3c C6 C7 A1 A2 C20 A5 (receipt directories 66-75), the 22 Architect-accepted controls, the three
controls reserved for the final witness-bound tranche (C1 C4 C25), this mission's control matrix (mission section 4),
the C7 informational F-3 run (mission section 5), the L1 complete-search rule (mission section 8), the accepted
Tranche-2 continuation evidence (b97205b2) and its five accepted controls' identities, the records of this mission and
its publication names (parent b97205b2). Every earlier name keeps its value (the earlier missions' history below is
unchanged text).

Identities only (no analysis logic). Every value here is the mission's (sections 1-3, 6-15, 19, 20) or the frozen R2
catalog's. Sources are read from git objects of frozen Candidate C, never from a worktree. New here: the Tranche-2
stopped evidence (2e06860f), the five Tranche-2 controls the Architect accepted, the preserved C5 build and executable
(mission sections 2 and 7: never rebuilt), the order C5 A3 A6 C8 C15, this mission's worktree namespace (mt2c) and
publication names.

The module also provides the names that the reused tools import from `common`, with their accepted Tranche-1 values:
supervise.py, classify.py, worktrees.py (EV, MISSION, C, C_TREE, BLOBS, PROTOCOL, REPO, GIT_COMMON_DIR, CRATE, EXE,
EXE_SHA256, EXE_SIZE, BASE_WT, BUILD_ARGV, BUILD_LIMIT, RUN_ENV, MEMORY_MAX, NEED_KIB, LISTED, UNIT_PREFIX, build_env)
and the Stage-2 configs.py and report.py (MODEL, TEST_PREFIX, STAGE1, STAGE1_PACKET, git, blob, source).
"""
import hashlib
import os
import subprocess

MISSION = "P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-witness-c1-c25-01"
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

# Mission-owned worktrees (created by this mission only; absent before; mission sections 4 and 7): one fresh worktree per
# control, in this mission's own namespace (mt3t), and the evidence worktree. TRANCHE3-TAIL-01: no preserved worktree is
# used (C5_WT and C24_WT keep their values: read only, for the accepted controls' worktree checks).
EVIDENCE_WT = NEXUS + "/nexus-os-p3-c11-d2a1-mutation-witness-c1-c25-01-evidence"
C5_WT = NEXUS + "/nexus-os-p3-c11-mt2x-C5"
C24_WT = NEXUS + "/nexus-os-p3-c11-mt3x-C24"


def control_wt(cid):
    return NEXUS + "/nexus-os-p3-c11-mtw-" + cid


# This mission (sections 1, 2 and 6): the exact order (a stop at one control stops every later one), the 27 Architect-
# accepted controls (never executed again), C7 (MUTATION_CONTROL_BLOCKED by the Architect: expressly excluded, never
# executed, rebuilt or reinterpreted), and every control not authorized.
ORDER = ["C1", "C25"]
FIRST_DIR = 81  # receipt directories 81-C1, 82-C25 (between the stop record 80 and the final gate 90)
ACCEPTED_T1 = ["C22", "C14", "C19", "C21", "C27", "C28", "C12", "C9", "C11", "C17", "C18a", "C18b"]
ACCEPTED_T2 = ["C23", "C10", "C16", "C26", "C3a"]
ACCEPTED_T2C = ["C5", "A3", "A6", "C8", "C15"]
ACCEPTED_T3 = ["C2"]
ACCEPTED_T3C = ["C24", "C3b", "C3c", "C6"]
ACCEPTED_T3T = ["A1", "A2", "C20", "A5"]  # WITNESS-C1-C25-01 section 1: accepted on e41c84b1 (31 of 35)
ACCEPTED = ACCEPTED_T1 + ACCEPTED_T2 + ACCEPTED_T2C + ACCEPTED_T3 + ACCEPTED_T3C + ACCEPTED_T3T
BLOCKED = ["C7"]  # MUTATION_CONTROL_BLOCKED (Architect ruling on c10681b3): E11 observed, P6 unmet; excluded here
NOT_AUTHORIZED = ["C4"]  # WITNESS-C1-C25-01 sections 1 and 10: C4 stays reserved; never executed here
KILL_REQUIRED = 35
# The mission's control matrix (section 4), stated independently of the catalog and compared with it (the catalog is
# the authority; a disagreement stops the mission): configuration, permitted printed properties (C3b: P13 or P12),
# state ceiling, mutated file, restored-side link (S1/S2: the accepted exact-final Stage-1/Stage-2 receipt; C6 and C7:
# the Stage-1 F-PT composed-quotient PASS, restored side only). TRANCHE3-TAIL-01 (mission section 6): the four controls.
# WITNESS-C1-C25-01 (mission sections 4 and 5): the two controls, both F-1, 60,000,000 states, protocol.rs, Stage-1 F-1.
MATRIX = {
    "C1": ("F-1", ["P1"], 60000000, PROTOCOL, "S1:45-full-F-1"),
    "C25": ("F-1", ["P7"], 60000000, PROTOCOL, "S1:45-full-F-1"),
}
# WITNESS-C1-C25-01 (mission sections 4 and 5; AD-1-R4 F.1, F.2, G.3, G.4), stated independently of the catalog, AD-1-R4
# and the source, and compared with them (a disagreement stops the mission). For each control: the predicted first
# failure of the type-(a) primary (recorded as printed; never counted as the named property; another first property
# stops the mission for an Architect ruling); the named property; the type-(c) mutant witness (test, steps in its F-1
# header, the exact positive line, the property that must appear in the line of other properties); the accepted healthy
# type-(e3) receipt (Stage 2) with its exact healthy line; the frozen patch and the mutant blob; the G.3(e) recording,
# proposed for Architect review only. The witness panic is run_witness's own panic! (model_tests.rs, unchanged by both
# patches, which change protocol.rs only).
G3 = {
    "C1": {"predicted": "E3", "named": "P1", "witness_test": "c1_witness_p1_at_a_successful_admission",
           "witness_const": "C1_WITNESS", "steps": 11,
           "positive": "NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)",
           "healthy_line": "C1 WITNESS F-1: 11 steps, each enabled and taking its choices; no violation; step 11 shows "
                           "the healthy core's 'admission: refused'",
           "healthy_dir": "receipts/50-witness-C1",
           "patch_sha256": "5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252",
           "mutant_blob": "7d95dd425fcdf17b12dda2fdce461f97ba7b9a66",
           "recording": "C1: P1 (first failure E3; witness)"},
    "C25": {"predicted": "E4", "named": "P7", "witness_test": "c25_witness_p7_on_the_resulting_world",
            "witness_const": "C25_WITNESS", "steps": 1,
            "positive": "NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))",
            "healthy_line": "C25 WITNESS F-1: 1 steps, each enabled and taking its choices; no violation; step 1 shows "
                            "the healthy core's 'append_gating: Refused Uncertain Storage'",
            "healthy_dir": "receipts/51-witness-C25",
            "patch_sha256": "29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732",
            "mutant_blob": "0456a0c1c1fe40f71593edc6ba5938a3b0c29900",
            "recording": "C25: P7 (first failure E4; witness)",
            "preserved_lines": ["self.failed();", "self.rolled(io.rollback());", "self.keep = true;"]},
}
WITNESS_LIMIT = 2700  # mission section 6: explorer and witness, 2700 s maximum wall time
# Controls whose kill run is on F-PT (A4 ON by F-PT's definition; the original explorer; the quotient is the restored
# side only), and the controls whose required property is L1 (evaluated only after a COMPLETE search; mission 8).
A4_ON = ["C6", "C7"]
L1_CONTROLS = ["C20", "A5"]
# The C7 informational run (mission section 5): the same pinned C7 mutant executable, before restoration, on F-3 with
# A4 OFF, the original explorer, 30,000,000 states, 2700 s; informational, never the C7 kill receipt.
C7_INFO = {"control": "C7", "configuration": "F-3", "test_name": "full_f3", "const": "F3", "ceiling_states": 30000000,
           "limit_seconds": 2700, "dir": "04b-info-F3"}
# A6 (mission section 11; catalog R2): the printed key fields must name recovery_required.
A6_KEY_TEXT = "the test's table (classes, debt counter, H, A, recovery_required)"

# Build policy (mission sections 8 and 9): identical for every mutant build, and identical to Tranche 1's.
BUILD_ARGV = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run", "-j", "2"]
BUILD_LIMIT = 900
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
MEMORY_MAX = "17179869184"
NEED_KIB = 18 * 1024 * 1024
UNIT_PREFIX = "p3c11-mtw"
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

# The Tranche-2 stopped evidence (mission sections 1-3): immutable; read from git objects, never copied or changed.
T2 = "2e06860f4b404d1a79a0d290116239b5545b931f"
T2_TREE = "4c929aa877816cc0cb5c42e32ddbdd75b380bacd"
T2_PARENT = T1
T2_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01"
T2_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01"
# Where each Tranche-2 control's receipts are (the five accepted ones; C5's build, stopped).
T2_RECEIPTS = {"C23": "receipts/51-C23", "C10": "receipts/52-C10", "C16": "receipts/53-C16", "C26": "receipts/54-C26",
               "C3a": "receipts/55-C3a", "C5": "receipts/56-C5"}

# The five Tranche-2 controls the Architect accepted (mission section 1), with the exact evidence identities of their
# Tranche-2 receipts (git blobs at T2; sha256 of the run's raw output and of the pinned mutant executable; the printed
# property and the discovered count). Authenticated against the git objects; never rebuilt, rerun or regenerated.
T2_ACCEPTED_EVIDENCE = {
    "C23": {"result_blob": "6e4bbaae5e1ce8b63157b9a3408e85b2e20c8cc4", "run_classification_blob":
            "d6f1dfadce972bf50d5cb8c02fe584b7a34453e0", "raw_output_sha256":
            "eccccfb71cab27d3c9c880c64d07dd2e0db41c50f2abfcc6417638853ce209bd", "executable_sha256":
            "3549e576ab943bad1cb9ee0a2d96d073924b5225dc0b24abc399b5d6db2b98b9", "link_blob":
            "90b40e304ad92764538c41835badb665ac701a04", "property": "E1", "states": 671, "configuration": "F-2"},
    "C10": {"result_blob": "c535a404e357e7dec2b306e3ffed262395abc860", "run_classification_blob":
            "cd9cc8d439572f422535fe5758337ea0ffbe3c71", "raw_output_sha256":
            "36c16109f0c1ad0fa54d7db0eb534b8f473c29c656505ea969ecf2a9cda1fd13", "executable_sha256":
            "5c8ef9f07b0ef37b430161c9da088bcbe7253ea17a6863ce2677d5f28645e867", "link_blob":
            "e9f40ccd3ed0fbdd01b6cb0298824bada87fcf01", "property": "E4", "states": 42, "configuration": "F-1"},
    "C16": {"result_blob": "743cf3867a06af943057dc9ebf3e769d6065a33a", "run_classification_blob":
            "93a23d98c83f084db57bfa5e154dc934b679d2bc", "raw_output_sha256":
            "ee469a45a015b4ad23914c2ebb23c83a281e47a0136f37a4fcd11b801e1242ba", "executable_sha256":
            "d53f4f0c7c96364c55c654b98ec6966066e68e30d590da964eb5488a9069ce8c", "link_blob":
            "cb1f93f40ca4fbaee1704d7d1c46f190ed934f65", "property": "E3", "states": 1513, "configuration": "F-D"},
    "C26": {"result_blob": "f4505012b964e1f1da47efce8c2293bb85036749", "run_classification_blob":
            "2bb850c436b57352457c9cd405f5a48c06680bee", "raw_output_sha256":
            "0313d7154ad42a4f69f3d8bb2e6b919df5b8e94ab15a45e9bc5998ba15f83faa", "executable_sha256":
            "336c30483abf4a0b0672caefdd87e8c7fc504094b36d1129cce4fb90b0663f97", "link_blob":
            "355d44f80eded7f2f6dfa2b521204da690236dc1", "property": "E9", "states": 3478, "configuration": "F-4"},
    "C3a": {"result_blob": "d6a4c0645d564bb0be25bd71fed8f80b2e80155a", "run_classification_blob":
            "158d224057bd5bbc552c1e48501c1eebec66fdf3", "raw_output_sha256":
            "b17f69a34ae04752b2d123493d6b8ec8d0b393fd5ca4f8b3de4d52f2d2339000", "executable_sha256":
            "c71e66debb2f3d42527668f48818c74f5ca48f03aceda67c9b8285b6f8e5a0a6", "link_blob":
            "4654bb57a2746abd0ef13229eedb86129c4a20db", "property": "P13", "states": 746152, "configuration": "F-7"},
}

# The preserved C5 build (mission sections 2, 6 and 7): admitted under the narrow C5-specific warning ruling; never
# rebuilt; its executable pinned read-only where cargo wrote it.
C5_BUILD_DIR = T2_PACKET + "/receipts/56-C5/03-build"
C5_PATCH_SHA256 = "a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8"
C5_MUTANT_BLOB = "e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495"
C5_MUTANT_SHA256 = "31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd"
C5_EXE = C5_WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
C5_EXE_SHA256 = "90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9"
C5_EXE_SIZE = 8986720
C5_STDERR_SHA256 = "4b7bdea581849cc8a980ab637228c2e19afd7a8f12704e8a44a2dc922663e8dc"
C5_LOCK_REASON = ("P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 control C5 disposable mutant worktree: mission-owned; "
                  "preserve until Architect review")

# The accepted Tranche-2 continuation evidence (mission sections 1 and 3): immutable; read from git objects, never
# copied or changed; its five controls Architect-accepted (never rebuilt, rerun or regenerated).
T2C = "b97205b2b9ea058ee864b6649dc5002d28b1edeb"
T2C_TREE = "6db2f923278f9f6ea7bacd97bd29444d4e8d6816"
T2C_PARENT = T2
T2C_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02"
T2C_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02"
T2C_RECEIPTS = {"C5": "receipts/61-C5", "A3": "receipts/62-A3", "A6": "receipts/63-A6", "C8": "receipts/64-C8",
                "C15": "receipts/65-C15"}
T2C_ACCEPTED_EVIDENCE = {
    "C5": {"result_blob": "cc1ccc68b5cfc300a8f19204e32e9e421c8675ec", "run_classification_blob":
           "30564b81a2d71ee3e89cd3a34c3b748ba1dfd7c6", "raw_output_sha256":
           "62ca176023a7f5066a474483fda926cd32e0f263c3c67835d9d5f6d1329ced19", "executable_sha256":
           "90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9", "link_blob":
           "0df4b80247545642ec0e723f38d9441a356dd5ac", "property": "P13", "states": 418422, "configuration": "F-5",
           "pin": "03-pin/PIN.json"},
    "A3": {"result_blob": "8fe853c28225ffa6f2cba3fb85d1fdf448e8fd44", "run_classification_blob":
           "72851e74fac3d3d84d256200315585c03337aaa6", "raw_output_sha256":
           "17132fdad65419ef1f26eca196f38c13cdb5322e337636d17c58423d24e9e1b9", "executable_sha256":
           "4805365da56eb6534708df651442299570561b3937838366b819267f420c4b64", "link_blob":
           "96a67228e9b659064937eb7c967caba0fcdeb57e", "property": "P12", "states": 112880, "configuration": "F-5",
           "pin": "03-build/PIN.json"},
    "A6": {"result_blob": "0b676921013432ad4149ccbf48e537b128955107", "run_classification_blob":
           "9ca455757e8fbe4b8cc6208f8aa765c65a630adb", "raw_output_sha256":
           "52ea67c27c5187cc81ae1e11b333f19b9d1081c0846375177cf7dafebc56db2d", "executable_sha256":
           "53d72467fe1d5cd1c318cf413ef0550c2cf05ce0d62954816cf25499b9e0b01e", "link_blob":
           "56c25fc66b3c2406cccb5c355d398c5673d3275d", "property": "P12", "states": 3537396, "configuration": "F-5",
           "pin": "03-build/PIN.json"},
    "C8": {"result_blob": "f0bc22fbedbe4c86afa59dbff793ed7dd713e99d", "run_classification_blob":
           "f2ff96fbc6b76dd2b168893ce7850f66e7dca71e", "raw_output_sha256":
           "b42d0eb5775ba8fb6c1279dcf390468c7fffcbc47e7f4e888fc2cd23e3dd90cc", "executable_sha256":
           "60cc14b41266ec603e6a8400bac791cb7c93e03acd73a01c3ddbc72f0c3f2109", "link_blob":
           "bb3625dbac4da15285fd388eb1b30145300c62ff", "property": "E5", "states": 547, "configuration": "F-B",
           "pin": "03-build/PIN.json"},
    "C15": {"result_blob": "712e4ca3a4d02d9c41547c201f0c8ce8c681c54d", "run_classification_blob":
            "40386480443f4b4f3cc2cd116b7fc5d68218b227", "raw_output_sha256":
            "75c2c6d76f3868e75b85e1a4770ab78ebb691fab87981187bcd070bea471a5ad", "executable_sha256":
            "89809695e0b87c8be259927a622bf2f2326a0323bde357aaa2406facf1c7b9f3", "link_blob":
            "77e46b103ff14bf8fdcda94df18661756ceb7aa3", "property": "E8", "states": 83988, "configuration": "F-R",
            "pin": "03-build/PIN.json"},
}
# The worktrees of the 27 accepted controls (read-only; never touched by this mission; TRANCHE3-CONTINUE-02: C2's added;
# TRANCHE3-TAIL-01: C24's preserved mt3x worktree and C3b's, C3c's and C6's mt3c worktrees added) and C7's (blocked;
# read-only; never touched).
ACCEPTED_WT = dict([(c, NEXUS + "/nexus-os-p3-c11-mt1x-" + c) for c in ACCEPTED_T1]
                   + [(c, NEXUS + "/nexus-os-p3-c11-mt2x-" + c) for c in ACCEPTED_T2 + ["C5"]]
                   + [(c, NEXUS + "/nexus-os-p3-c11-mt2c-" + c) for c in ["A3", "A6", "C8", "C15"]]
                   + [(c, NEXUS + "/nexus-os-p3-c11-mt3x-" + c) for c in ACCEPTED_T3 + ["C24"]]
                   + [(c, NEXUS + "/nexus-os-p3-c11-mt3c-" + c) for c in ["C3b", "C3c", "C6"]]
                   + [(c, NEXUS + "/nexus-os-p3-c11-mt3t-" + c) for c in ACCEPTED_T3T])
BLOCKED_WT = {"C7": NEXUS + "/nexus-os-p3-c11-mt3c-C7"}

# The Architect ruling records and the campaign status record of this mission (written from mission sections 1, 2, 5).
# The two TRANCHE2-CONTINUE-02 records keep their names (RULING_T2C, C5_RULING): they are authenticated at T2C.
RULING_T2C = "documents/ARCHITECT-RULING-TRANCHE2-PARTIAL-ACCEPTANCE.md"
C5_RULING = "documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md"
C5_RULING_FIRST = "C5 BUILD — ARCHITECT-ADMISSIBLE (NARROW C5-SPECIFIC WARNING RULING; NO GENERAL WARNING EXCEPTION)"
# TRANCHE3-CONTINUE-02: the two TRANCHE3-EXEC-01 records keep their texts under the names RULING_T3X and CAMPAIGN_T3X
# (authenticated at T3X); RULING and CAMPAIGN name this mission's own records.
RULING_T3X = "documents/ARCHITECT-RULING-TRANCHE2-CONTINUATION-ACCEPTANCE.md"
RULING_T3X_FIRST = ("TRANCHE 2 CONTINUATION — C5 A3 A6 C8 C15 — MUTATION CONTROL KILL ACCEPTED (22 OF 35); "
                    "TRANCHE 3 AUTHORIZED: C2 C24 C3b C3c C6 C7 A1 A2 C20 A5")
CAMPAIGN_T3X = "documents/CAMPAIGN-STATUS-BEFORE-TRANCHE3.md"
CAMPAIGN_T3X_FIRST = "CAMPAIGN STATUS BEFORE TRANCHE 3 — 22 OF 35 KILL-REQUIRED CONTROLS ARCHITECT-ACCEPTED"
# TRANCHE3-TAIL-01: the continuation's own two records keep their texts under RULING_T3C and CAMPAIGN_T3C (authenticated
# at T3C, with W_RULING and AMENDMENT); RULING and CAMPAIGN name this mission's own records.
RULING_T3C = "documents/ARCHITECT-RULING-TRANCHE3-C2-ACCEPTANCE.md"
RULING_T3C_FIRST = ("TRANCHE 3 — C2 — MUTATION CONTROL KILL ACCEPTED (23 OF 35); TRANCHE 3 CONTINUATION AUTHORIZED: "
                    "C24 C3b C3c C6 C7 A1 A2 C20 A5")
# WITNESS-C1-C25-01: the tail's own two records keep their texts under RULING_T3T and CAMPAIGN_T3T (authenticated at
# T3T); RULING and CAMPAIGN name this mission's own records.
RULING_T3T = "documents/ARCHITECT-RULING-TRANCHE3-CONTINUATION-ACCEPTANCE.md"
RULING_T3T_FIRST = ("TRANCHE 3 CONTINUATION — C24 C3b C3c C6 — MUTATION CONTROL KILL ACCEPTED (27 OF 35); C7 "
                    "MUTATION_CONTROL_BLOCKED; TRANCHE 3 TAIL AUTHORIZED: A1 A2 C20 A5")
RULING = "documents/ARCHITECT-RULING-TRANCHE3-TAIL-ACCEPTANCE.md"
RULING_FIRST = ("TRANCHE 3 TAIL — A1 A2 C20 A5 — MUTATION CONTROL KILL ACCEPTED (31 OF 35); C7 "
                "MUTATION_CONTROL_BLOCKED; C4 RESERVED; WITNESS-BOUND C1 C25 AUTHORIZED (C1 THEN C25)")
W_RULING = "documents/ARCHITECT-RULING-C24-C3B-C3C-BUILD-ADMISSIBILITY.md"
W_RULING_FIRST = ("C24 BUILD ARCHITECT-ADMISSIBLE (NARROW C24-SPECIFIC WARNING RULING); C3b AND C3c CONDITIONAL BUILD "
                  "RULES; NO GENERAL WARNING EXCEPTION")
AMENDMENT = "documents/ARCHITECT-AMENDMENT-01-C3B-C3C-BUILD-RULE.md"
AMENDMENT_FIRST = ("ARCHITECT AMENDMENT-01 — C3b AND C3c: A CLEAN BUILD OR EXACTLY THE ONE PREDICTED WARNING GROUP; THE "
                   "WARNING IS PERMITTED, NOT REQUIRED")
CAMPAIGN_T3C = "documents/CAMPAIGN-STATUS-BEFORE-TRANCHE3-CONTINUATION.md"
CAMPAIGN_T3C_FIRST = ("CAMPAIGN STATUS BEFORE THE TRANCHE 3 CONTINUATION — 23 OF 35 KILL-REQUIRED CONTROLS "
                      "ARCHITECT-ACCEPTED")
CAMPAIGN_T3T = "documents/CAMPAIGN-STATUS-BEFORE-TRANCHE3-TAIL.md"
CAMPAIGN_T3T_FIRST = "CAMPAIGN STATUS BEFORE THE TRANCHE 3 TAIL — 27 OF 35 KILL-REQUIRED CONTROLS ARCHITECT-ACCEPTED"
CAMPAIGN = "documents/CAMPAIGN-STATUS-BEFORE-WITNESS-C1-C25.md"
CAMPAIGN_FIRST = ("CAMPAIGN STATUS BEFORE THE C1/C25 WITNESS MISSION — 31 OF 35 KILL-REQUIRED CONTROLS "
                  "ARCHITECT-ACCEPTED")
RECORDS = [RULING, CAMPAIGN]
RECORD_FIRST = {RULING: RULING_FIRST, CAMPAIGN: CAMPAIGN_FIRST}

# The Tranche-3 stopped evidence (mission sections 1, 2, 8, 20): immutable; read from git objects, never copied or
# changed. C2 Architect-accepted (never rebuilt, rerun or regenerated); C24 stopped at its build (historical BUILD: NOT
# OK, unchanged), its build now admitted under the narrow C24 ruling.
T3X = "774f457a4fc27f6b26e132a0caf71b683b9af8cd"
T3X_TREE = "f74660b98288c53e285bcb4d3b98fb9f55962b57"
T3X_PARENT = T2C
T3X_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche3-exec-01"
T3X_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01"
T3X_RECEIPTS = {"C2": "receipts/66-C2", "C24": "receipts/67-C24"}
T3X_ACCEPTED_EVIDENCE = {
    "C2": {"result_blob": "57c789536982a03c357492bd7aec619e27ac7684", "run_classification_blob":
           "a0f3a04d1daf7e1fc25e17f5366c327602f6e674", "raw_output_sha256":
           "758a6b98779fb27bd44ef1e99783ceb1982b2532e135d0d7de50e48963a1fb31", "executable_sha256":
           "8460515fe149a78637bcb1d797d9cb5c2ed5b89eb2361cd74e02e96cedaed0ed", "link_blob":
           "4f3e63c5849ce856c188f47517af7a98cc825608", "property": "P4", "states": 361534, "configuration": "F-1",
           "pin": "03-build/PIN.json"},
}

# The preserved C24 build (mission sections 2, 8 and 9): admitted under the narrow C24-specific warning ruling; never
# rebuilt; its executable pinned read-only where cargo wrote it; its worktree restored to C in Tranche 3 and locked with
# that mission's reason (read only here).
C24_BUILD_DIR = T3X_PACKET + "/receipts/67-C24/03-build"
C24_APPLY_DIR = T3X_PACKET + "/receipts/67-C24/02-apply"
C24_PATCH_SHA256 = "39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44"
C24_MUTANT_BLOB = "3e1bfbe746851b6c63811343cde9dd3b83ad0fa9"
C24_MUTANT_SHA256 = "d4732d83f924d824752d62d59a28e12cc97a456669f7cbea0868d7e6e9742268"
C24_EXE = C24_WT + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
C24_EXE_SHA256 = "8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d"
C24_EXE_SIZE = 8987680
C24_STDERR_SHA256 = "1d2a24899f268f358f7446ae876e860e41a4bae7963dee45987fb2e7f1f7112e"
C24_LOCK_REASON = ("P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 control C24 disposable mutant worktree: mission-owned; "
                   "preserve until Architect review")
# The C3b and C3c build rule (mission sections 3-5 as amended by AMENDMENT-01): a clean build (BUILD: PINNED under the
# unchanged general classifier), or exactly this one warning group (the variant named, in AckRefusal, protocol.rs line
# 630 the enum, line 635/636 the variant). Permitted, not required; nothing transfers to any other control.
C3W = {"C3b": {"variant": "Future", "line": 635, "col": 5,
               "patch_sha256": "614e53fd75518991c5e6518bc0fe61cf546cf092b955e3eda0e71cf25d5f6dd4",
               "mutant_blob": "e6aff1884a4b9b7f880eee8784c371648fb2c073",
               "mutant_sha256": "84dc2a424ce80ce47b71871f111c4aaa37d8b776dc4851f16513d86312247273"},
       "C3c": {"variant": "Set", "line": 636, "col": 5,
               "patch_sha256": "1e60d98d6d42c0aab581a5bde34162f9767949a495fad801b73d9c740d9e2e85",
               "mutant_blob": "5cc9c09ac96062c5d348e86ace8cb0ac2225e83d",
               "mutant_sha256": "e72f8f619fa1c9b4a8379351138b354fa83fe470b1c56450b8b22f28c61710c2"}}
C3W_ENUM = ("AckRefusal", 630, 17)

# The Tranche-3 continuation evidence (mission sections 2, 3 and 9): immutable; read from git objects, never copied or
# changed. C24, C3b, C3c and C6 Architect-accepted (never rebuilt, rerun or regenerated); C7 MUTATION_CONTROL_BLOCKED
# (Architect ruling: the authentic F-PT, A4-ON original explorer returned E11; the frozen requirement is P6; no
# attribution exception): its receipts are cited as blocked, never as a kill candidate.
T3C = "c10681b3c65dd377b5a13d08140427e8b34441d0"
T3C_TREE = "7071c0ce01c9ed9a01f28ff7792d648516466a48"
T3C_PARENT = T3X
T3C_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche3-continue-02"
T3C_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche3-continue-02"
T3C_RECEIPTS = {"C24": "receipts/81-C24", "C3b": "receipts/82-C3b", "C3c": "receipts/83-C3c", "C6": "receipts/84-C6",
                "C7": "receipts/85-C7"}
T3C_ACCEPTED_EVIDENCE = {
    "C24": {"result_blob": "1886f614db4d4fcaa3cf2d6f5d1c29f81509a50b", "run_classification_blob":
            "55dd2e6ac7ac6ad56352f2888dec7419d62c9eb9", "raw_output_sha256":
            "9b48791e313d3a0f33ae0d77c37edf7dd1fb18a74efd4aa11f7c15b93a2af3df", "executable_sha256":
            "8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d", "link_blob":
            "65cea3435409a291c3ea2f07e659a5d3f3717ef7", "rules_blob": "bdd3d121a0902765dca50ad0aaf893c1808ba5f7",
            "property": "P8", "states": 185114, "expanded": 87111, "configuration": "F-1", "pin": "03-pin/PIN.json",
            "restore": "05-reauth/REAUTH.txt"},
    "C3b": {"result_blob": "550d4fe8a75054ffcb4dacf44b02920c9eb9cf18", "run_classification_blob":
            "777c32bc174d22d03bb36978a5ee0438a2a2a876", "raw_output_sha256":
            "9c08f2f01dad5348b2f21a968f1c4a29aad54f32fe27d6b7ab87e2b185998b9b", "executable_sha256":
            "8aeecde3c905a0f7069e28eec7f187db0cf2d9637e93293841bff92d8d31281b", "link_blob":
            "277ba52a0675aeba3c4925a1cfe452bd0b3c62bd", "rules_blob": "48c9fc24a1408f4ea9bbed4a5906e3fb43087663",
            "property": "P13", "states": 67871, "expanded": 44338, "configuration": "F-6", "pin": "03-build/PIN.json",
            "restore": "05-restore/RESTORE.txt"},
    "C3c": {"result_blob": "3378c4f38dc8d0cea10f432904efae43a36b9286", "run_classification_blob":
            "e7a1107a1c283f3206eabb4dc947f8159de61ff1", "raw_output_sha256":
            "e746174a0ef2371ab9630191faa3e213cbcfad6fa2fd576844fb5b77e133197c", "executable_sha256":
            "e8cfbe4c98412d4ae78fbe3c38d9c6e93d2af16d445631971618822646c38e32", "link_blob":
            "ed1af73eef6315518e2a30d7667830067ed1465b", "rules_blob": "3aa7e1505e51b7daf5e5b6b44c479d76c578b358",
            "property": "P12", "states": 541827, "expanded": 381761, "configuration": "F-6", "pin": "03-build/PIN.json",
            "restore": "05-restore/RESTORE.txt"},
    "C6": {"result_blob": "f82d4bc60180a18317638bd076d52df28d48b849", "run_classification_blob":
           "4de8c877a34e156e969cee95ed9dcb025d2736f5", "raw_output_sha256":
           "920e109f65a60b316c70e21dabe30942bc95db5e225b00af1470df2bcf41fd01", "executable_sha256":
           "5c88a5c7b0c2c9e9ab644c97bd93a4e97c0570e85807a5be7467466bdcdd832d", "link_blob":
           "9d936b126a51a9a4464d3addf80cc2245f902e55", "rules_blob": "4fe96c63ff59a47a7d94c9f1282fc26c71aaa769",
           "property": "P6", "states": 2652070, "expanded": 1807301, "configuration": "F-PT", "pin": "03-build/PIN.json",
           "restore": "05-restore/RESTORE.txt"},
}
# C7 (mission sections 1, 2, 9): MUTATION_CONTROL_BLOCKED, E11 observed, P6 unmet. Its receipts are authenticated as
# they stand; nothing here makes it a kill candidate.
T3C_BLOCKED_EVIDENCE = {
    "C7": {"result_blob": "da50b23a8f9c482bf7649ee1d093560d65585cd4", "run_classification_blob":
           "020517d6e2e08f18c13f669828c2c4de55b6d119", "raw_output_sha256":
           "db3bf1643cb5f1d367004400f5caa9d975ebeb2bf45aca2e8b64dae98691154d", "executable_sha256":
           "b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5", "rules_blob":
           "50e8369a8b97e211fd58306f6f3e95094fdf97ed", "property": "E11", "required_property": "P6", "states": 1071165,
           "expanded": 686654, "configuration": "F-PT", "pin": "03-build/PIN.json"},
}

# The Tranche-3 tail evidence (WITNESS-C1-C25-01 sections 1 and 2): immutable; read from git objects, never copied or
# changed. A1, A2, C20 and A5 Architect-accepted (never rebuilt, rerun or regenerated).
T3T = "e41c84b1c41dded6cc9816f11f6603f5d80587a9"
T3T_TREE = "10b408a1432edeed5720024bd6b8807852ecb464"
T3T_PARENT = T3C
T3T_REF = "refs/heads/review/p3-c11-d2a1-mutation-tranche3-tail-01"
T3T_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche3-tail-01"
T3T_RECEIPTS = {"A1": "receipts/81-A1", "A2": "receipts/82-A2", "C20": "receipts/83-C20", "A5": "receipts/84-A5"}
T3T_ACCEPTED_EVIDENCE = {
    "A1": {"result_blob": "36ebc7018c529f132ac9e5e4c4255d699bcc870b", "run_classification_blob":
           "67bc4098e4e6487b013ad79edd2d231b0e1286bb", "raw_output_sha256":
           "2a1c9413592503f43f03c0e42a1e9b53cf3b45810e1db72cf53172f422ab2244", "executable_sha256":
           "d58ccd31c2c42acbb065d3ef08a00f42a8966c04bd0f000f938f3b9b2bdf6b12", "link_blob":
           "bfe421be31dbde67d0300ef6fcc90cce61e53c0e", "rules_blob": "780559d2ebb0090a65ae8eea701704ab8573bb3e",
           "property": "P1", "states": 1962, "expanded": 430, "configuration": "F-1", "pin": "03-build/PIN.json",
           "restore": "05-restore/RESTORE.txt"},
    "A2": {"result_blob": "8761e5125e3eef33bb1b616818b661564ff2e3ef", "run_classification_blob":
           "3d073b0d3b9f8d7982e231d2594d59b671877fd3", "raw_output_sha256":
           "d1edba58123e3301465ed89916ca80769a1d0b3ade516cd7503ed63585275e93", "executable_sha256":
           "257cb481e0de203654f7007d60ebe537e84b71dfd8c98b02b90288365249662d", "link_blob":
           "858475a06a6236f3557b8e3527a5861e10b877fb", "rules_blob": "1fdd1d3a704524852a5ee4c07034cc6eeace853e",
           "property": "P1", "states": 1453, "expanded": 277, "configuration": "F-1", "pin": "03-build/PIN.json",
           "restore": "05-restore/RESTORE.txt"},
    "C20": {"result_blob": "6ceebd19d13ffd6b190640f53d656c57ed20c6cd", "run_classification_blob":
            "fd0015ee598bf755bb20c56053a06c857381ebd2", "raw_output_sha256":
            "8d80f6092fca5e55f26e55d64a22637112caeda8fd3a7ef8bc5e8e3d80aa31a5", "executable_sha256":
            "5433a13ba47468ff457cf665ae1383e64fdd7de26086b4a95b9ffd7c4b121a02", "link_blob":
            "83cf05b8aeeb47d37bee2b57353eeeb859079e3d", "rules_blob": "f0c9fd9b0d110a5706dcd2c2c8a5b0ba8f8e43e9",
            "property": "L1", "states": 12590754, "expanded": 12590754, "configuration": "F-2",
            "pin": "03-build/PIN.json", "restore": "05-restore/RESTORE.txt"},
    "A5": {"result_blob": "04478cd57ce6191432eabfee05ff21fa7decd3b5", "run_classification_blob":
           "171d8d469829e7ff3e88869d321b5cd9e2622bd4", "raw_output_sha256":
           "ba743f7dc3c70ee50016f201a2e3cb19cc6227e41d921c14338b27f68672e3c6", "executable_sha256":
           "22de28a2f3ff3a94d480f5247523ca2139600988139e9c4769d79be874041a85", "link_blob":
           "40ac1980516c49b4530f888b2725a6ed198814d2", "rules_blob": "7212f1e420cf6174c4798fcf88c710ed4333f7d7",
           "property": "L1", "states": 12684666, "expanded": 12684666, "configuration": "F-2",
           "pin": "03-build/PIN.json", "restore": "05-restore/RESTORE.txt"},
}

# Publication (WITNESS-C1-C25-01 section 9): the evidence commit's parent is the Tranche-3 tail evidence commit.
PARENT = T3T
PARENT_TREE = T3T_TREE
PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-witness-c1-c25-01/"
BRANCH = "review/p3-c11-d2a1-mutation-witness-c1-c25-01"
DEST = "refs/heads/" + BRANCH
LOCAL_BRANCH = "evidence/p3-c11-d2a1-mutation-witness-c1-c25-01"
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"
PACKET_LIMIT = 64 * 1024 * 1024  # TRANCHE3-EXEC-01 section 14: at most 64 MiB (unchanged)

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
