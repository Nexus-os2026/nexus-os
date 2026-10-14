#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 15: exactly one normal, non-forced, explicit-refspec
push of the evidence commit to the authorized review ref. Its console is receipts/41-PUSH.txt (local only).

    push.py <evidence-commit>

Refuses unless finalize.py committed this commit, the pre-push checks (receipts/40-PRE-PUSH.txt) held on it, and the
destination ref is absent on the remote immediately before the push. Runs `git push github <commit>:<ref>` once (no
force option of any kind), then reads the remote ref back.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402


def main(e):
    Q.no_bytecode()
    fin = open(os.path.join(Q.P, "receipts/30-FINALIZE.txt")).read().splitlines()
    pre = open(os.path.join(Q.P, "receipts/40-PRE-PUSH.txt")).read().splitlines()
    if fin[-2:] != [f"FINALIZE: COMMITTED {e}", "exit 0"]:
        raise SystemExit(f"REFUSED: finalize.py did not commit {e}: {fin[-2:]}")
    if pre[-2:] != ["RESULT: ALL CHECKS HOLD", "exit 0"] or f"pre {e}" not in pre[0]:
        raise SystemExit(f"REFUSED: the pre-push checks did not hold on {e}: {pre[:1]} {pre[-2:]}")
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in Q.git("ls-remote", Q.REMOTE).decode().splitlines()}
    print(f"destination {Q.DEST} on {Q.REMOTE} immediately before the push: {rm.get(Q.DEST, 'ABSENT')}")
    if Q.DEST in rm:
        print("PUSH: REFUSED: the destination exists")
        return 1
    cmd = ["git", "-C", Q.REPO, "push", Q.REMOTE, f"{e}:{Q.DEST}"]
    print("command: " + " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(f"exit {r.returncode}")
    print("stdout: " + r.stdout.strip())
    print("stderr: " + r.stderr.strip())
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in Q.git("ls-remote", Q.REMOTE).decode().splitlines()}
    print(f"destination {Q.DEST} on {Q.REMOTE} after the push: {rm.get(Q.DEST, 'ABSENT')}")
    ok = r.returncode == 0 and rm.get(Q.DEST) == e
    print("PUSH: " + ("PUSHED ONCE; REMOTE REF READ BACK EQUAL" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
