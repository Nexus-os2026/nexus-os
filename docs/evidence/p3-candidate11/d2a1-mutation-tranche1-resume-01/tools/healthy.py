#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 8: the exact-command healthy build of Candidate C (the
type-(e4) receipt C22 links). Run once; nothing is retried.

    healthy.py      receipts/19-healthy-worktree (creation and authentication of a fresh mission-owned worktree at C)
                    receipts/20-healthy-build    (the supervised build, its target manifest and its (e4) classification)

The build is `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` in the new worktree with a
fresh worktree-local target, CARGO_NET_OFFLINE=true, the repository's toolchain and Cargo configuration unchanged, no
feature, cfg or RUSTFLAGS, 16 GiB, zero swap, 15 minutes. The produced executable is recorded and never run. The
worktree and target are kept for review.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import supervise  # noqa: E402
import worktrees  # noqa: E402

WT_DIR = os.path.join(K.EV, "receipts/19-healthy-worktree")
BUILD_DIR = os.path.join(K.EV, "receipts/20-healthy-build")
PRIOR = [("receipts/15-baseline-T-RES/CLASSIFICATION.txt", "CLASSIFICATION: PASS")]


def main():
    if os.path.exists(WT_DIR) or os.path.exists(BUILD_DIR):
        raise SystemExit("REFUSED: the healthy-build receipts exist (one build; nothing is retried)")
    rec = []
    problem = supervise.freeze_check(rec)
    for p, want in PRIOR:
        got = supervise.first_line(os.path.join(K.EV, p))
        if got != want:
            problem = (problem or "") + f" prior step {p} is {got!r}, not {want!r}"
    if problem:
        os.makedirs(WT_DIR)
        open(os.path.join(WT_DIR, "REFUSED.txt"), "w").write("\n".join(rec) + f"\nREFUSED: {problem}\n")
        print("REFUSED:", problem)
        return 9
    r, ok = worktrees.create(K.HEALTHY_WT, "healthy-build")
    worktrees.write(WT_DIR, "CREATE", f"CREATE the healthy-build worktree {K.HEALTHY_WT} at C", r, ok)
    if not ok:
        return 4
    a, ok = worktrees.authenticate(K.HEALTHY_WT, "healthy-build")
    worktrees.write(WT_DIR, "AUTHENTICATE", f"AUTHENTICATE the healthy-build worktree {K.HEALTHY_WT}", a, ok)
    if not ok:
        return 4

    def source_ok(rec, when):
        d = supervise.tree_check(rec, K.HEALTHY_WT, when)
        return d, d["verdict"] == "EXACTLY_C"
    supervise.build(BUILD_DIR, "SUPERVISED HEALTHY BUILD — Candidate C, the exact command (mission section 8; type (e4))",
                    K.HEALTHY_WT, source_ok, PRIOR,
                    {"role": "healthy build (e4)", "worktree_receipts": "receipts/19-healthy-worktree"}, manifest=True)
    return classify.classify_build_success(BUILD_DIR, "e4", "EXACTLY_C",
                                           expect_target=os.path.join(K.HEALTHY_WT, "target"))


if __name__ == "__main__":
    sys.exit(main())
