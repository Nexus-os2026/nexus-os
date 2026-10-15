#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 13: create the publication worktree. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 17): the evidence worktree exactly at the required parent (the
accepted Tranche-1 evidence commit), on a new local branch, at a new path. No other worktree is touched. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (section 20): the required parent is the Tranche-2 stopped evidence
commit. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (section 15): the required parent is the accepted
continuation evidence commit. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 20): the required parent
is the Tranche-3 stopped evidence commit. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 (section 11): the
required parent is the Tranche-3 continuation evidence commit (c10681b3).

    worktree.py      refuses unless the path and the branch are absent and no hook or filter would run on checkout;
                     runs `git worktree add -b <branch> <path> <parent>` once; prints every check. Exit 0 only if
                     every check holds.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main():
    hp = K.git("config", "--get", "core.hooksPath", check=False).decode().strip()
    hooks_dir = hp or f"{K.REPO}/.git/hooks"
    hooks_dir = hooks_dir if os.path.isabs(hooks_dir) else os.path.join(K.REPO, hooks_dir)
    hooks = sorted(n for n in os.listdir(hooks_dir) if not n.endswith(".sample")) if os.path.isdir(hooks_dir) else []
    check("no active git hook (the effective hooks directory holds only *.sample files): no post-checkout hook can run",
          not hooks, f"hooksPath {hp!r}; effective {hooks_dir}; non-sample entries {hooks}")
    filters = K.git("config", "--get-regexp", "^filter\\.", check=False).decode().strip()
    attrs = K.git("ls-tree", K.PARENT, ".gitattributes").decode().strip()
    check("no configured filter and no .gitattributes at the parent: checkout runs no smudge filter",
          not filters and not attrs, f"filters {filters!r}; .gitattributes {attrs!r}")
    check("the worktree path is absent", not os.path.lexists(K.EVIDENCE_WT), K.EVIDENCE_WT)
    absent = subprocess.run(["git", "-C", K.REPO, "show-ref", "--verify", "-q", "refs/heads/" + K.LOCAL_BRANCH])
    check("the local evidence branch is absent", absent.returncode == 1, K.LOCAL_BRANCH)
    check("the parent is the Tranche-3 continuation evidence commit, with its recorded tree",
          K.git("rev-parse", K.PARENT + "^{tree}").decode().strip() == K.PARENT_TREE, K.PARENT)
    if fails:
        print("WORKTREE: REFUSED: " + "; ".join(fails))
        return 1
    cmd = ["git", "-C", K.REPO, "worktree", "add", "-b", K.LOCAL_BRANCH, K.EVIDENCE_WT, K.PARENT]
    print("command: " + " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(f"exit {r.returncode}; stdout {r.stdout.strip()!r}; stderr {r.stderr.strip()!r}")
    check("git worktree add succeeded", r.returncode == 0)
    head = K.git("rev-parse", "HEAD", repo=K.EVIDENCE_WT).decode().strip()
    branch = K.git("symbolic-ref", "HEAD", repo=K.EVIDENCE_WT).decode().strip()
    status = K.git("status", "--porcelain", "-uall", repo=K.EVIDENCE_WT).decode()
    check("the worktree HEAD is exactly the parent", head == K.PARENT, head)
    check("the worktree is on the new local evidence branch", branch == "refs/heads/" + K.LOCAL_BRANCH, branch)
    check("the worktree is clean", status == "", status[:200])
    check("the packet path is absent at the parent", not os.path.lexists(os.path.join(K.EVIDENCE_WT, K.PACKET)))
    check("the destination ref is still absent locally", subprocess.run(
        ["git", "-C", K.REPO, "show-ref", "--verify", "-q", K.DEST]).returncode == 1)
    print("WORKTREE: " + ("CREATED" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
