#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 13: create the new publication worktree, exactly at the
required parent (the RESUME-01 evidence commit), on a new local publication branch, at a new path. The old failed
evidence worktree and every other worktree are not touched.

    worktree.py      refuses unless the path and the branch are absent and no hook or filter would run on checkout;
                     runs `git worktree add -b <branch> <path> <parent>` once; prints every check. Exit 0 only if
                     every check holds.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402

fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main():
    Q.no_bytecode()
    hp = Q.git("config", "--get", "core.hooksPath", check=False).decode().strip()
    hooks_dir = hp or f"{Q.REPO}/.git/hooks"
    hooks = sorted(n for n in os.listdir(hooks_dir) if not n.endswith(".sample")) if os.path.isdir(hooks_dir) else []
    check("no active git hook (effective hooks directory holds only *.sample files): no post-checkout hook can run",
          not hooks, f"hooksPath {hp!r}; non-sample entries {hooks}")
    filters = Q.git("config", "--get-regexp", "^filter\\.", check=False).decode().strip()
    attrs = Q.git("ls-tree", Q.PARENT, ".gitattributes").decode().strip()
    check("no configured filter and no .gitattributes at the parent: checkout runs no smudge filter",
          not filters and not attrs, f"filters {filters!r}; .gitattributes {attrs!r}")
    check("the worktree path is absent", not os.path.lexists(Q.WORKTREE), Q.WORKTREE)
    absent = subprocess.run(["git", "-C", Q.REPO, "show-ref", "--verify", "-q", "refs/heads/" + Q.LOCAL_BRANCH])
    check("the local publication branch is absent", absent.returncode == 1, Q.LOCAL_BRANCH)
    check("the parent is the RESUME-01 evidence commit, with its recorded tree",
          Q.git("rev-parse", Q.PARENT + "^{tree}").decode().strip() == Q.RESUME01_TREE, Q.PARENT)
    if fails:
        print("WORKTREE: REFUSED: " + "; ".join(fails))
        return 1
    cmd = ["git", "-C", Q.REPO, "worktree", "add", "-b", Q.LOCAL_BRANCH, Q.WORKTREE, Q.PARENT]
    print("command: " + " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(f"exit {r.returncode}; stdout {r.stdout.strip()!r}; stderr {r.stderr.strip()!r}")
    check("git worktree add succeeded", r.returncode == 0)
    head = Q.git("rev-parse", "HEAD", repo=Q.WORKTREE).decode().strip()
    branch = Q.git("symbolic-ref", "HEAD", repo=Q.WORKTREE).decode().strip()
    status = Q.git("status", "--porcelain", "-uall", repo=Q.WORKTREE).decode()
    check("the worktree HEAD is exactly the parent", head == Q.PARENT, head)
    check("the worktree is on the new local publication branch", branch == "refs/heads/" + Q.LOCAL_BRANCH, branch)
    check("the worktree is clean", status == "", status[:200])
    check("the packet path is absent at the parent", not os.path.lexists(os.path.join(Q.WORKTREE, Q.PACKET)))
    check("the destination ref is still absent locally", subprocess.run(
        ["git", "-C", Q.REPO, "show-ref", "--verify", "-q", Q.DEST]).returncode == 1)
    print("WORKTREE: " + ("CREATED" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
