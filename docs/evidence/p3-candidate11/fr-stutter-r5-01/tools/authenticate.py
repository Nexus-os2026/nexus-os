#!/usr/bin/env python3
"""Authenticates every input of P3-C11-DR01-D2A1-FR-STUTTER-R5-01 (mission
section 3). Reads git objects and the authenticated contract copy only.
Stops (exit 1) on any mismatch.

  authenticate.py [--remote]     --remote also reads GitHub refs
"""
import hashlib
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
S1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
S1_PARENT = "0622a4d042b1dfd3160f06fc5d333e131e2a60c8"
S2 = "89326b859792a442c1853c3d35d30a48301c009e"
EXE = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
AD1R4_PATH = "docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
AD1R4_SHA = "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693"
CONTRACT = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r4-consolidation-01/inputs/CONTRACT-02-R1.md"
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
P1 = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/"
P2 = "docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/"
BLOBS = {
    "crates/nexus-governed-control/src/ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
    "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
    "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114",
}
STAGE2_REF = "refs/heads/review/p3-c11-d2a1-final-baseline-remaining-configs-01"
DEST_REF = "refs/heads/review/p3-c11-d2a1-fr-stutter-r5-01"

fails = 0


def git(*a, ok=False):
    r = subprocess.run(["git", "-C", REPO, *a], capture_output=True)
    if r.returncode and not ok:
        raise SystemExit(f"git {' '.join(a)} failed: {r.stderr.decode()}")
    return r.stdout


def check(ok, what):
    global fails
    print(f"{'OK  ' if ok else 'FAIL'}  {what}")
    if not ok:
        fails += 1


def sha(b):
    return hashlib.sha256(b).hexdigest()


def show(commit, path):
    return git("show", f"{commit}:{path}")


def sums(commit, prefix, expected_entries):
    text = show(commit, prefix + "SHA256SUMS").decode()
    bad, n = [], 0
    for line in text.splitlines():
        h, name = line.split(None, 1)
        name = name.lstrip("*")
        n += 1
        if sha(show(commit, prefix + name)) != h:
            bad.append(name)
    check(n == expected_entries and not bad,
          f"{prefix}SHA256SUMS at {commit[:8]}: {n} entries (expected {expected_entries}), every committed blob matches ({len(bad)} mismatches); "
          f"SHA256SUMS sha256 {sha(text.encode())}")


def main(remote):
    print(f"AUTHENTICATION — P3-C11-DR01-D2A1-FR-STUTTER-R5-01")
    # Candidate C
    check(git("cat-file", "-t", C).decode().strip() == "commit", f"C {C} is a commit")
    check(git("rev-parse", f"{C}^{{tree}}").decode().strip() == C_TREE, f"C tree {C_TREE}")
    parents = git("rev-list", "--parents", "-n1", C).decode().split()[1:]
    check(parents == [C_PARENT], f"C has the single parent {C_PARENT} ({parents})")
    for path, blob in BLOBS.items():
        got = git("rev-parse", f"{C}:{path}").decode().strip()
        check(got == blob, f"{path.split('/')[-1]} blob at C {got}")
    # the 49 Rust inputs, from Stage-1's dep-info receipt
    dep = show(S1, P1 + "receipts/11-pin/DEP-INFO.txt").decode()
    listed = re.findall(r"^\s+OK\s+(\S+)\s+blob at C ([0-9a-f]{40})\s+on disk ([0-9a-f]{40})", dep, re.M)
    tree = {}
    for line in git("ls-tree", "-r", C, "crates/nexus-governed-control/src").decode().splitlines():
        meta, p = line.split("\t", 1)
        if p.endswith(".rs"):
            tree[p] = meta.split()[2]
    check(len(listed) == 49 and "inputs: 49" in dep and "RESULT: every input is C’s blob" in dep,
          f"Stage-1 DEP-INFO lists 49 inputs, each C's blob ({len(listed)})")
    check({p: b for p, b, d in listed} == tree and all(b == d for p, b, d in listed),
          f"the 49 dep-info inputs are exactly the {len(tree)} .rs files of crates/nexus-governed-control/src at C, blob for blob")
    check(EXE in dep or EXE in show(S1, P1 + "receipts/11-pin/PIN-RECEIPT.txt").decode(),
          f"pinned executable identity {EXE[:16]}… recorded by Stage-1 (reference only; not run)")
    # AD-1-R4
    ad = show(S2, AD1R4_PATH)
    check(sha(ad) == AD1R4_SHA, f"AD-1-R4 sha256 {sha(ad)}")
    check(git("rev-parse", f"{S2}:{AD1R4_PATH}").decode().strip() == "35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1", "AD-1-R4 blob 35d5074c…")
    row15 = "| 15 | `full_fr` (4256) | `explore`, F-R | REQUIRED_FINAL_RUN | C15 | |"
    check(row15 in ad.decode(), "AD-1-R4 E.3 row 15 reads: " + row15)
    # Stage-1 and Stage-2
    check(git("rev-list", "--parents", "-n1", S1).decode().split()[1:] == [S1_PARENT], f"Stage-1 {S1[:8]} single parent {S1_PARENT[:8]}")
    check(git("rev-list", "--parents", "-n1", S2).decode().split()[1:] == [S1], f"Stage-2 {S2[:8]} single parent {S1[:8]}")
    check(git("merge-base", "--is-ancestor", C, S1, ok=True) == b"" and
          subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", C, S1]).returncode == 0, "C is an ancestor of Stage-1")
    nonev = git("diff", "--name-only", C, S2, "--", ".", ":!docs/evidence").decode().split()
    check(nonev == [], f"from C to Stage-2 nothing outside docs/evidence changed ({len(nonev)} paths)")
    sums(S1, P1, 179)
    sums(S2, P2, 268)
    # receipts
    f2 = show(S1, P1 + "receipts/41-full-F-2/raw-output.txt")
    check(sha(f2) == "985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5", "F-2 raw output sha256 985a3976…")
    f2t = f2.decode()
    check("  outcome: PASS" in f2t and "  states 12602670 (expanded 12602670), transitions 54214081 (with a fault 3558228), states with no enabled step 3206" in f2t
          and "  D1: HOLDS" in f2t and "  L1: HOLDS: 50655853 non-fault edges, no cycle; 3206 end states, each terminal (D1)" in f2t
          and "test result: ok. 1 passed; 0 failed" in f2t, "F-2: PASS, 12,602,670 states, D1 HOLDS, L1 HOLDS, harness ok")
    props = re.findall(r"^  (P\d+|D1|L1|E\d+|KEY): (.*)$", f2t, re.M)
    holds = [p for p, v in props if v.startswith("HOLDS")]
    check(len(props) == 25 and len(holds) == 24 and dict(props)["E10"].startswith("not applicable"),
          f"F-2: 25 property lines, 24 HOLDS, E10 not applicable ({len(props)}, {len(holds)})")
    cls = show(S1, P1 + "receipts/41-full-F-2/CLASSIFICATION.txt").decode()
    check(cls.startswith("CLASSIFICATION: PASS") and "34 of 34" in cls, "F-2 classification: PASS — admissible unchanged-core original-explorer PASS (34 of 34)")
    rec = show(S1, P1 + "receipts/41-full-F-2/RECEIPT.txt").decode()
    check(EXE in rec and "EXACTLY_C" in rec, "F-2 receipt: pinned executable and the exact C worktree")
    fr = show(S2, P2 + "receipts/69-full-F-R/raw-output.txt")
    check(sha(fr) == "78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd", "F-R raw output sha256 78e44c33…")
    frt = fr.decode()
    check("  outcome: INCOMPLETE" in frt and "  states 30000001 (expanded 26539923)" in frt
          and "INCOMPLETE: the state cap of 30000000 was reached. No property is established by this run." in frt,
          "F-R: INCOMPLETE, 30,000,001 discovered, 26,539,923 expanded; no property established")
    frc = show(S2, P2 + "receipts/69-full-F-R/CLASSIFICATION.txt").decode()
    check(frc.splitlines()[0] == "CLASSIFICATION: NOT PASS" and frc.splitlines()[1].startswith("outer class (AD-1-R4 K.2): INCOMPLETE (the state cap of 30000000 was reached."),
          f"F-R classification: {frc.splitlines()[0]}; {frc.splitlines()[1][:60]}…")
    check(EXE in show(S2, P2 + "receipts/69-full-F-R/RECEIPT.txt").decode(), "F-R receipt: the same pinned executable")
    su = show(S1, P1 + "receipts/30-suite/raw-output.txt")
    check(sha(su) == "d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3", "suite raw output sha256 d6f58742…")
    sut = su.decode()
    check("test ledger::protocol::model_tests::e8_holds_for_stale_worker_operations ... ok" in sut and
          "test result: ok. 313 passed; 0 failed; 35 ignored; 0 measured; 0 filtered out" in sut,
          "exact-final nonignored suite: 313 passed, 0 failed; e8_holds_for_stale_worker_operations ... ok")
    check(show(S1, P1 + "receipts/30-suite/CLASSIFICATION.txt").decode().startswith("CLASSIFICATION: PASS"), "suite classification: PASS")
    check(EXE in show(S1, P1 + "receipts/30-suite/RECEIPT.txt").decode(), "suite receipt: the pinned executable")
    voc = show(S2, P2 + "vocabulary/VOCABULARY.json")
    check(sha(voc) == "db8734e0bd2743f22a27fb4001e8deb0f6381b393aecc33c781d710ee57fa932" and voc.count(b'"label"') >= 193,
          "193-row vocabulary sha256 db8734e0…")
    # contract (C15's row, F-R's row)
    ct = open(CONTRACT, "rb").read()
    check(sha(ct) == CONTRACT_SHA, f"contract P3-C11-D2A1-CONTRACT-02-R1 sha256 {sha(ct)}")
    lines = ct.decode().split("\n")
    check(lines[712] == "| C15 | a cell operation ignores the occupancy number | E8 | F-R |", f"contract line 713: {lines[712]}")
    check(lines[850] == "| F-R | as F-2 | | | | | REPLAY on, three times |", f"contract line 851: {lines[850]}")
    check(lines[638].startswith("| REPLAY | a fixed number of times"), "contract line 639: the REPLAY actor's program")
    if remote:
        refs = git("ls-remote", "github", "refs/heads/review/*").decode()
        m = re.search(r"^([0-9a-f]{40})\t" + re.escape(STAGE2_REF) + "$", refs, re.M)
        check(m is not None and m.group(1) == S2, f"GitHub {STAGE2_REF} = {S2[:12]}")
        check(DEST_REF not in refs, f"GitHub {DEST_REF} absent")
    print(f"AUTHENTICATION: {'ALL INPUTS AUTHENTICATED' if fails == 0 else f'{fails} MISMATCH(ES) — STOP'}")
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main("--remote" in sys.argv))
