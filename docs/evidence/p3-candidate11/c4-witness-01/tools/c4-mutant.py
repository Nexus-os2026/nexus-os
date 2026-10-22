#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: the one disposable C4 mutant worktree.

  c4-mutant.py apply   <A-commit> <receipt>   create the worktree detached at A, verify it, apply the frozen patch
  c4-mutant.py restore <A-commit> <receipt>   reverse-apply the frozen patch and verify every file against A by blob

The worktree is mission-owned and disposable; its applied source and its executables are never committed. Each
step refuses on any identity mismatch. Nothing is retried.
"""
import hashlib, os, subprocess, sys

REPO = "/home/nexus/NEXUS/nexus-os"
MWT = "/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01-mutant"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01"
PATCH = os.path.join(EV, "patches/C4-test1-removed.patch")
PATCH_SHA = "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
BASE_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
MUTANT_BLOB = "f3793c2ff61f8adbf43fe17d6b5237b49bc507b0"
MUTANT_SHA = "4037de6e33a901f933f118dde5a395b70b1e5cd7e58e54dcf0078d4a40f335b2"


def git(cwd, *args):
    r = subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"FAILED git {' '.join(args)}: {r.returncode}\n{r.stdout}{r.stderr}")
    return r.stdout


def sha256(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    mode, a, receipt = sys.argv[1], sys.argv[2], sys.argv[3]
    if os.path.exists(receipt):
        raise SystemExit(f"REFUSED: {receipt} exists")
    out = []
    note = lambda t: (out.append(t), print(t))
    a_tree = git(REPO, "rev-parse", f"{a}^{{tree}}").strip()
    a_model = git(REPO, "rev-parse", f"{a}:{MODEL}").strip()
    assert git(REPO, "rev-parse", f"{a}:{PROTOCOL}").strip() == BASE_BLOB, "A's protocol.rs is not the baseline blob"
    assert sha256(PATCH) == PATCH_SHA, "the frozen patch changed"
    note(f"C4 MUTANT {mode.upper()} — A {a} tree {a_tree}; A's model_tests.rs {a_model}; patch {PATCH} sha256 {PATCH_SHA}")
    if mode == "apply":
        if os.path.exists(MWT):
            raise SystemExit(f"REFUSED: {MWT} exists")
        note(git(REPO, "worktree", "add", "--detach", MWT, a).strip() or f"worktree added: {MWT}")
        head, tree = git(MWT, "rev-parse", "HEAD", "HEAD^{tree}").split()
        por = git(MWT, "status", "--porcelain")
        pb, mb = git(MWT, "hash-object", PROTOCOL, MODEL).split()
        note(f"  before: HEAD {head} tree {tree} porcelain lines {len(por.splitlines())} protocol.rs {pb} model_tests.rs {mb}")
        assert (head, tree, por, pb, mb) == (a, a_tree, "", BASE_BLOB, a_model), "the new worktree is not A"
        note("  " + (git(MWT, "apply", "--check", "-v", PATCH) or "git apply --check: ok").strip().replace("\n", "; "))
        git(MWT, "apply", PATCH)
        pb2, mb2 = git(MWT, "hash-object", PROTOCOL, MODEL).split()
        por2 = git(MWT, "status", "--porcelain").splitlines()
        stat = git(MWT, "diff", "--numstat").strip()
        note(f"  after: protocol.rs blob {pb2} sha256 {sha256(os.path.join(MWT, PROTOCOL))}; model_tests.rs {mb2}; "
             f"porcelain {por2}; numstat {stat!r}")
        assert pb2 == MUTANT_BLOB and sha256(os.path.join(MWT, PROTOCOL)) == MUTANT_SHA, "the mutant blob differs"
        assert mb2 == a_model and por2 == [f" M {PROTOCOL}"] and stat == f"0\t16\t{PROTOCOL}", "more than the patch changed"
        assert git(MWT, "rev-parse", "HEAD").strip() == a
        note("  RESULT: the mutant source is A plus exactly the frozen C4 patch")
    elif mode == "restore":
        git(MWT, "apply", "-R", PATCH)
        head, tree = git(MWT, "rev-parse", "HEAD", "HEAD^{tree}").split()
        por = git(MWT, "status", "--porcelain")
        pb, mb = git(MWT, "hash-object", PROTOCOL, MODEL).split()
        diff = git(MWT, "diff", "--stat", a).strip()
        note(f"  restored: HEAD {head} tree {tree} porcelain lines {len(por.splitlines())} protocol.rs {pb} "
             f"model_tests.rs {mb}; diff against A: {diff!r}")
        assert (head, tree, por, pb, mb, diff) == (a, a_tree, "", BASE_BLOB, a_model, ""), "restoration is not A"
        note("  RESULT: every file equals A by blob; the worktree is clean at A (kept; its target/ holds the mutant "
             "executable, never committed)")
    else:
        raise SystemExit("usage")
    open(receipt, "w").write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
