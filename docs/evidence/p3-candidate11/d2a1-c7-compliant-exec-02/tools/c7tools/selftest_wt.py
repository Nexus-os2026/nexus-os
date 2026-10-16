#!/usr/bin/env python3
"""Self-test of c7wt.py on a THROWAWAY scratch repository (never the Nexus repository):
creation, EXACTLY_W / EXACTLY_W7 verification, patch application, negative cases, authentication R-1..R-8, restoration.
  selftest_wt.py <scratch-dir>      (the directory must not exist)
"""
import io
import contextlib
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c7wt
from c7lib import *  # noqa

ENVG = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t",
            GIT_AUTHOR_DATE="2026-01-01T00:00:00Z", GIT_COMMITTER_DATE="2026-01-01T00:00:00Z", LC_ALL="C")


def sg(repo, *a, check=True):
    p = subprocess.run(["git", "-C", repo, *a], capture_output=True, env=ENVG)
    if check and p.returncode:
        raise RuntimeError(p.stderr.decode())
    return p.stdout.decode().strip()


results = []


def expect(name, cond, detail=""):
    results.append(bool(cond))
    print("[%s] %s %s" % ("OK" if cond else "FAILED", name, detail), flush=True)


def quiet(fn, *a):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = fn(*a)
    return rc, buf.getvalue()


def main():
    base = os.path.abspath(sys.argv[1])
    os.makedirs(base)
    repo = os.path.join(base, "repo")
    os.makedirs(repo)
    sg(repo, "init", "-q")
    os.makedirs(os.path.join(repo, os.path.dirname(PROTOCOL_PATH)))
    with open(os.path.join(repo, PROTOCOL_PATH), "wb") as f:
        f.write(blob_at(C_BASE, PROTOCOL_PATH))
    open(os.path.join(repo, ".gitignore"), "w").write("target/\n")
    open(os.path.join(repo, "other.txt"), "w").write("other\n")
    open(os.path.join(repo, "run.sh"), "w").write("#!/bin/sh\n")
    os.chmod(os.path.join(repo, "run.sh"), 0o755)
    os.symlink("other.txt", os.path.join(repo, "link"))
    sg(repo, "add", "-A")
    sg(repo, "commit", "-q", "-m", "scratch W")
    commit, tree = sg(repo, "rev-parse", "HEAD"), sg(repo, "rev-parse", "HEAD^{tree}")
    c7wt.SPEC.update(repo=repo, commit=commit, tree=tree, wt={"H": os.path.join(base, "wt-healthy"), "M": os.path.join(base, "wt-mutant")})
    ev = os.path.join(base, "ev")
    os.makedirs(ev)
    print("scratch repo", repo, "commit", commit)
    expect("scratch protocol.rs is the C-BASE blob", sg(repo, "rev-parse", "HEAD:" + PROTOCOL_PATH) == PROTOCOL_C_BLOB)
    expect("mutant blob absent from the scratch object store", subprocess.run(["git", "-C", repo, "cat-file", "-e", MUTANT_BLOB]).returncode != 0)
    M, H = c7wt.SPEC["wt"]["M"], c7wt.SPEC["wt"]["H"]
    rc, _ = quiet(c7wt.create, "H", ev)
    expect("create H", rc == 0)
    rc, _ = quiet(c7wt.create, "M", ev)
    expect("create M", rc == 0)
    try:
        c7wt.create("M", ev)
        expect("create M twice is refused", False)
    except SystemExit as e:
        expect("create M twice is refused", "REFUSED" in str(e))
    ok = lambda role, wt=M: all(c[1] for c in c7wt.verify(wt, role))  # noqa
    expect("M verifies as EXACTLY_W", ok("W"))
    expect("M does not verify as EXACTLY_W7 before the patch", not ok("W7"))
    rc, _ = quiet(c7wt.apply_patch, ev, os.path.join(ev, "apply.txt"))
    expect("apply the frozen patch", rc == 0)
    aj = json.load(open(os.path.join(ev, "receipts/00-worktrees/apply-C7.json")))
    expect("apply receipt (EXEC-02): UTC times, patch identity, mutant identity, ok", aj["ok"] and aj["patch_sha256"] == PATCH_SHA and aj["mutant_protocol"]["git_blob"] == MUTANT_BLOB
           and aj["started_utc"] <= aj["finished_utc"])
    expect("M verifies as EXACTLY_W7", ok("W7"))
    expect("M does not verify as EXACTLY_W after the patch", not ok("W"))
    expect("mutant blob never written to the scratch object store", subprocess.run(["git", "-C", repo, "cat-file", "-e", MUTANT_BLOB]).returncode != 0)
    expect("H still EXACTLY_W", ok("W", H))
    auth = c7wt.auth_r(ev)
    expect("R-1..R-8 all hold in the mutated state", all(c[1] for c in auth) and len(auth) == 8, str([c[0] for c in auth]))
    rc, _ = quiet(c7wt.preserve, ev, os.path.join(ev, "preserved"))
    expect("preserve writes evidence", rc == 0 and os.path.exists(os.path.join(ev, "preserved/mutant-protocol.identity.txt")))
    # negatives
    pr = os.path.join(M, PROTOCOL_PATH)
    good = open(pr, "rb").read()
    open(pr, "ab").write(b" ")
    expect("hand edit -> not EXACTLY_W7", not ok("W7"))
    a2 = c7wt.auth_r(ev)
    expect("hand edit -> R-5 fails (an appended byte leaves the reverse patch applicable, so R-8 still holds)",
           [c[0] for c in a2 if not c[1]] == ["R-5"], str([c[0] for c in a2 if not c[1]]))
    rc, out = quiet(c7wt.restore, ev, os.path.join(ev, "r-refused"))
    expect("hand edit -> RESTORATION NOT PERFORMED", rc == 2 and "RESTORATION NOT PERFORMED" in out)
    open(pr, "wb").write(good)
    expect("bytes put back -> EXACTLY_W7 again", ok("W7"))
    open(os.path.join(M, "stray.txt"), "w").write("x")
    expect("stray untracked file -> not EXACTLY_W7", not ok("W7"))
    os.remove(os.path.join(M, "stray.txt"))
    open(os.path.join(M, "other.txt"), "a").write("x")
    expect("second tracked path changed -> R-4 fails", not [c for c in c7wt.auth_r(ev) if c[0] == "R-4"][0][1])
    open(os.path.join(M, "other.txt"), "w").write("other\n")
    expect("restored -> EXACTLY_W7", ok("W7"))
    os.rename(os.path.join(ev, "receipts/00-worktrees/creation-M.json"), os.path.join(ev, "creation-M.saved"))
    expect("no creation record -> R-3 fails", not [c for c in c7wt.auth_r(ev) if c[0] == "R-3"][0][1])
    os.rename(os.path.join(ev, "creation-M.saved"), os.path.join(ev, "receipts/00-worktrees/creation-M.json"))
    # the restoration itself
    rc, out = quiet(c7wt.restore, ev, os.path.join(ev, "r-ok"))
    expect("restoration completes: EXACTLY_W", rc == 0 and "RESTORATION COMPLETE: EXACTLY_W" in out)
    expect("M verifies as EXACTLY_W after restoration", ok("W"))
    expect("mutant blob still absent from the scratch object store", subprocess.run(["git", "-C", repo, "cat-file", "-e", MUTANT_BLOB]).returncode != 0)
    open(os.path.join(M, "target/x"), "w").write("x")
    expect("files under the ignored target do not matter", ok("W"))
    # staging is forbidden: it would write the mutant blob to the object store, which the verification detects.
    os.remove(os.path.join(ev, "receipts/00-worktrees", c7wt.PATCH_FILE))  # scratch only: the tool never overwrites it
    os.remove(os.path.join(ev, "receipts/00-worktrees/apply-C7.json"))  # scratch only (EXEC-02: apply now also writes this receipt)
    rc, _ = quiet(c7wt.apply_patch, ev, os.path.join(ev, "apply2.txt"))
    expect("second apply (scratch only)", rc == 0 and ok("W7"))
    sg(M, "add", PROTOCOL_PATH)
    expect("staged change -> not EXACTLY_W7", not ok("W7"))
    expect("staged change -> WT-INDEX and WT-OBJECT both fail", sorted(c[0] for c in c7wt.verify(M, "W7") if not c[1]) == ["WT-DIFF", "WT-INDEX", "WT-OBJECT", "WT-STATUS"] or
           {"WT-INDEX", "WT-OBJECT"} <= {c[0] for c in c7wt.verify(M, "W7") if not c[1]}, str([c[0] for c in c7wt.verify(M, "W7") if not c[1]]))
    expect("staged change -> R-6 fails", not [c for c in c7wt.auth_r(ev) if c[0] == "R-6"][0][1])
    print("\nSELF-TEST (c7wt) %s: %d/%d" % ("ALL OK" if all(results) else "FAILED", sum(results), len(results)))
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
