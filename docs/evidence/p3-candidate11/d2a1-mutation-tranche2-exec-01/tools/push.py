#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 15: exactly one normal, non-forced, explicit-refspec
push of the evidence commit to the authorized review ref. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01
(section 17) for this commit and ref. Its console is publication/41-PUSH.txt (local only).

    push.py <evidence-commit>

Refuses unless finalize.py committed this commit, the pre-push checks (publication/40-PRE-PUSH.txt) held on it, and the
destination ref is absent on the remote immediately before the push. Runs `git push github <commit>:<ref>` once (no
force option of any kind), then reads the remote ref back.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402


def main(e):
    fin = open(os.path.join(K.EV, "publication/30-FINALIZE.txt")).read().rstrip().splitlines()
    pre = open(os.path.join(K.EV, "publication/40-PRE-PUSH.txt")).read().rstrip().splitlines()
    if fin[-1:] != [f"FINALIZE: COMMITTED {e}"]:
        raise SystemExit(f"REFUSED: finalize.py did not commit {e}: {fin[-1:]}")
    if pre[-1:] != ["RESULT: ALL CHECKS HOLD"]:
        raise SystemExit(f"REFUSED: the pre-push checks did not hold: {pre[-1:]}")
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in K.git("ls-remote", K.REMOTE).decode().splitlines()}
    print(f"destination {K.DEST} on {K.REMOTE} immediately before the push: {rm.get(K.DEST, 'ABSENT')}")
    if K.DEST in rm:
        print("PUSH: REFUSED: the destination exists")
        return 1
    cmd = ["git", "-C", K.REPO, "push", K.REMOTE, f"{e}:{K.DEST}"]
    print("command: " + " ".join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    print(f"exit {r.returncode}")
    print("stdout: " + r.stdout.strip())
    print("stderr: " + r.stderr.strip())
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in K.git("ls-remote", K.REMOTE).decode().splitlines()}
    print(f"destination {K.DEST} on {K.REMOTE} after the push: {rm.get(K.DEST, 'ABSENT')}")
    ok = r.returncode == 0 and rm.get(K.DEST) == e
    print("PUSH: " + ("PUSHED ONCE; REMOTE REF READ BACK EQUAL" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
