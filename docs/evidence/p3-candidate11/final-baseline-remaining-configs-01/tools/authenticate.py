#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 2: authenticate every identity before any analysis or
invocation. Read-only: it writes nothing to the repository, the worktree or the remote.

  authenticate.py            prints one line per check and a verdict; exit 0 only if every check holds

Checks: frozen C (commit, tree, single parent, the three blobs, ct-1, AD-1-R4); the mission worktree (detached at C,
byte-equal to C, only the mission-owned target/); the pinned executable (path, regular file, sha256, size); the
Stage-1 evidence commit (tree, parent, ancestry from C, docs-only lineage, the GitHub review ref) and every entry of
its SHA256SUMS against the committed blobs; the Stage-1 pin record; the absence of this mission's destination ref and
packet path.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa: E402,F401,F403

lines, fails = [], []


def check(label, ok, detail=""):
    lines.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main():
    rev = lambda x: git("rev-parse", x).decode().strip()
    check("C is a commit", git("cat-file", "-t", C).decode().strip() == "commit")
    check("C's tree", rev(C + "^{tree}") == C_TREE, rev(C + "^{tree}"))
    parents = git("rev-list", "--parents", "-n1", C).decode().split()[1:]
    check("C has exactly one parent, the frozen one", parents == [C_PARENT], parents)
    for path, want in BLOBS.items():
        check(f"C:{path} blob", rev(f"{C}:{path}") == want, rev(f"{C}:{path}"))
    check("ct-1 blob at C", rev(f"{C}:{CT1}") == CT1_BLOB, rev(f"{C}:{CT1}"))
    check("AD-1-R4 sha256 at C", sha256_bytes(blob(C, AD1R4)) == AD1R4_SHA256, sha256_bytes(blob(C, AD1R4)))

    # The mission worktree.
    r = subprocess.run([sys.executable, os.path.join(EV, "tools/verify_tree.py"), WT], capture_output=True, text=True)
    d = json.loads(r.stdout)
    check("worktree exactly C (verify_tree.py: HEAD, tree, detached, index, status, every tracked byte and mode)",
          r.returncode == 0 and d["verdict"] == "EXACTLY_C",
          f"{d['verdict']}; {d['tracked_blobs_compared']} blobs, {d['bytes_compared']} bytes; status {d['status_entries']}")
    wt_head = git("rev-parse", "HEAD", repo=WT).decode().strip()
    check("worktree HEAD is C", wt_head == C, wt_head)

    # The pinned executable.
    real = os.path.realpath(EXE)
    check("executable path is a regular file, not a symlink", os.path.isfile(EXE) and not os.path.islink(EXE)
          and real == EXE, real)
    if os.path.isfile(EXE):
        st = os.stat(EXE)
        dig = sha256_file(EXE)
        check("executable sha256 equals the pin", dig == EXE_SHA256, dig)
        check("executable size equals the pin", st.st_size == EXE_SIZE, st.st_size)
        lines.append(f"     executable mtime_ns {st.st_mtime_ns}, inode {st.st_ino}, mode {oct(st.st_mode)}")

    # Stage-1 evidence.
    check("Stage-1 evidence commit tree", rev(STAGE1 + "^{tree}") == STAGE1_TREE, rev(STAGE1 + "^{tree}"))
    sp = git("rev-list", "--parents", "-n1", STAGE1).decode().split()[1:]
    check("Stage-1 evidence commit single parent", sp == [STAGE1_PARENT], sp)
    anc = subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", C, STAGE1]).returncode == 0
    check("C is an ancestor of the Stage-1 evidence commit", anc)
    changed = git("diff", "--name-only", C, STAGE1).decode().split()
    check("C..Stage-1 changes only docs/evidence paths (no source)", changed and all(p.startswith("docs/evidence/")
          for p in changed), f"{len(changed)} paths")
    remote = git("ls-remote", REMOTE, STAGE1_REF).decode().split()
    check("GitHub Stage-1 review ref equals the Stage-1 evidence commit", remote[:1] == [STAGE1], remote)
    sums = blob(STAGE1, STAGE1_PACKET + "/SHA256SUMS").decode()
    listed = {}
    for l in sums.splitlines():
        digest, rel = l.split("  ", 1)
        listed[rel] = digest
    bad = [rel for rel, dg in listed.items() if sha256_bytes(blob(STAGE1, f"{STAGE1_PACKET}/{rel}")) != dg]
    files = [p[len(STAGE1_PACKET) + 1:] for p in
             git("ls-tree", "-r", "--name-only", STAGE1, STAGE1_PACKET + "/").decode().split()]
    unlisted = sorted(set(files) - set(listed) - {"SHA256SUMS"})
    missing = sorted(set(listed) - set(files))
    check("Stage-1 SHA256SUMS: every entry equals its committed blob", not bad, f"{len(listed)} entries; bad {bad}")
    check("Stage-1 SHA256SUMS: self-excluding, every packet file listed, none missing",
          "SHA256SUMS" not in listed and not unlisted and not missing, f"unlisted {unlisted}; missing {missing}")
    check("Stage-1 SHA256SUMS sha256", True, sha256_bytes(sums.encode()))
    pin = json.loads(blob(STAGE1, STAGE1_PACKET + "/receipts/10-build/PIN.json"))
    check("Stage-1 PIN.json equals the pin (path, sha256, size, commit, tree)",
          (pin["executable"], pin["sha256"], pin["size"], pin["commit"], pin["tree"]) ==
          (EXE, EXE_SHA256, EXE_SIZE, C, C_TREE), pin["sha256"])

    # Destination.
    dest = git("ls-remote", REMOTE, DEST_REF).decode().strip()
    check("destination review ref absent on GitHub", dest == "", dest)
    have = subprocess.run(["git", "-C", REPO, "cat-file", "-e", f"{STAGE1}:{PACKET}"], capture_output=True).returncode
    check("packet path absent at the Stage-1 evidence commit", have != 0)
    local = subprocess.run(["git", "-C", REPO, "show-ref", "--verify", DEST_REF], capture_output=True).returncode
    check("destination ref absent locally", local != 0)

    lines.append(f"AUTHENTICATION: {'ALL OK' if not fails else 'FAILED: ' + '; '.join(fails)}")
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
