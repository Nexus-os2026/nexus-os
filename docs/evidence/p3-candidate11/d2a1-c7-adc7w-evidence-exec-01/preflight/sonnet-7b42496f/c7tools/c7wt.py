#!/usr/bin/env python3
"""C7-W worktree handling: creation records, identity verification (EXACTLY_W / EXACTLY_W7), patch application,
restoration through the frozen inverse patch, and the 7.3A authentication R-1..R-8.

  c7wt.py create  <H|M> <evdir>        add the detached worktree at W-COMMIT, make its real target dir, write the creation record
  c7wt.py verify  <H|M> <W|W7> [out]   verify EXACTLY_W or EXACTLY_W7 (prints and optionally writes the checks)
  c7wt.py apply   <evdir> <out>        M only: git apply --check, then ONE git apply of the frozen patch (never staged), then verify W7
  c7wt.py restore <evdir> <outdir>     M only: R-1..R-8, then git apply -R --check, ONE git apply -R (60 s), then verify EXACTLY_W
  c7wt.py preserve <evdir> <outdir>    7.3A: preserve evidence first (nothing is changed)

Nothing here resets, cleans, checks out, stashes, removes a worktree or edits a file by hand.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa

SPEC = dict(repo=REPO, commit=W_COMMIT, tree=W_TREE, protocol_path=PROTOCOL_PATH, protocol_w_blob=PROTOCOL_C_BLOB,
            protocol_w_sha=PROTOCOL_C_SHA, mutant_blob=MUTANT_BLOB, mutant_sha=MUTANT_SHA, patch_sha=PATCH_SHA,
            patch_commit=K1_COMMITS[1], patch_path=K1_PATCH_PATH, patch_blob=K1_PATCH_BLOB, patch_repo=REPO,
            wt={"H": WT_HEALTHY, "M": WT_MUTANT})
PATCH_FILE = "patch/C7.patch"  # relative to the evidence directory's receipts/00-worktrees
GIT_TIMEOUT = 120


def g(wt, *args, check=False, input=None):
    return git(*args, cwd=wt, check=check, input=input)


def tracked_entries(wt, commit):
    out = g(wt, "ls-tree", "-r", "-z", "--full-tree", commit, check=True).stdout
    entries = []
    for r in out.split(b"\0"):
        if not r:
            continue
        meta, path = r.split(b"\t", 1)
        mode, typ, oid = meta.decode().split()
        entries.append((mode, typ, oid, path))
    return entries


def worktree_entry(path):
    """The `git worktree list --porcelain` block of `path`, as a dict, from the main repository."""
    out = git("worktree", "list", "--porcelain", cwd=SPEC["repo"], check=True).stdout.decode("utf-8", "replace")
    for block in out.strip().split("\n\n"):
        d = {}
        for line in block.splitlines():
            k, _, v = line.partition(" ")
            d[k] = v
        if d.get("worktree") == path:
            return d
    return None


def ownership(path):
    st = os.lstat(path)
    return {"path": path, "realpath": os.path.realpath(path), "uid": st.st_uid, "gid": st.st_gid,
            "mode": "%o" % (st.st_mode & 0o7777), "dev": st.st_dev, "ino": st.st_ino,
            "is_symlink": os.path.islink(path), "is_dir": os.path.isdir(path)}


def scan_tracked(wt, role):
    """Compare every tracked path of the working tree, by content, with the commit's tree.
    Returns (diffs_for_role, diffs_against_w_tree, nentries, notes); for role W7 protocol.rs is expected to be the mutant blob."""
    diffs, diffs_w, notes = [], [], []
    entries = tracked_entries(wt, SPEC["commit"])
    for mode, typ, oid, pb in entries:
        rel = pb.decode("utf-8", "surrogateescape")
        full = os.path.join(wt, os.fsdecode(pb))
        want = oid
        if rel == SPEC["protocol_path"] and role == "W7":
            want = SPEC["mutant_blob"]
        if mode == "160000":
            notes.append("submodule entry %s" % rel)
            continue
        try:
            st = os.lstat(full)
        except OSError as e:
            diffs.append((rel, "missing: %s" % e))
            diffs_w.append((rel, "missing"))
            continue
        if mode == "120000":
            if not os.path.islink(full):
                diffs.append((rel, "not a symlink"))
                diffs_w.append((rel, "not a symlink"))
                continue
            data = os.fsencode(os.readlink(full))
        else:
            if os.path.islink(full) or not os.path.isfile(full):
                diffs.append((rel, "not a regular file"))
                diffs_w.append((rel, "not a regular file"))
                continue
            if (mode == "100755") != bool(st.st_mode & 0o111):
                diffs.append((rel, "mode mismatch (tree %s, worktree %o)" % (mode, st.st_mode & 0o777)))
                diffs_w.append((rel, "mode mismatch"))
                continue
            with open(full, "rb") as f:
                data = f.read()
        bid = git_blob_id(data)
        if bid != oid:
            diffs_w.append((rel, "blob %s != W-TREE %s" % (bid, oid)))
        if bid != want:
            diffs.append((rel, "blob %s != expected %s" % (bid, want)))
    return diffs, diffs_w, len(entries), notes


def verify(wt, role, tag=None):
    """EXACTLY_W (role "W") or EXACTLY_W7 (role "W7") of the worktree `wt`. Returns a list of (id, ok, detail)."""
    out = []

    def add(cid, ok, detail=""):
        out.append((cid, bool(ok), detail))

    add("WT-PATH", os.path.isdir(wt) and not os.path.islink(wt) and os.path.realpath(wt) == wt, wt)
    head = g(wt, "rev-parse", "HEAD").stdout.decode().strip()
    add("WT-HEAD", head == SPEC["commit"], head)
    sym = g(wt, "symbolic-ref", "-q", "HEAD")
    add("WT-DETACHED", sym.returncode != 0, "symbolic-ref rc=%d" % sym.returncode)
    tree = g(wt, "rev-parse", "HEAD^{tree}").stdout.decode().strip()
    add("WT-TREE", tree == SPEC["tree"], tree)
    ent = worktree_entry(wt)
    add("WT-LIST", ent is not None and ent.get("HEAD") == SPEC["commit"] and "detached" in ent
        and "locked" not in ent and "prunable" not in ent and "bare" not in ent, json.dumps(ent))
    cached = g(wt, "diff-index", "--cached", "--name-status", "HEAD")
    add("WT-INDEX", cached.returncode == 0 and cached.stdout.strip() == b"",
        "index equals HEAD" if cached.stdout.strip() == b"" else cached.stdout.decode()[:300])
    unm = g(wt, "ls-files", "-u")
    flags = [l for l in g(wt, "ls-files", "-v").stdout.splitlines() if not l.startswith(b"H ")]
    add("WT-FLAGS", unm.stdout.strip() == b"" and not flags, "unmerged/assume-unchanged/skip-worktree entries: %d" % len(flags))
    diffs, diffs_w, n, notes = scan_tracked(wt, role)
    add("WT-BLOBS", not diffs, "%d tracked paths compared with W-TREE%s; differing: %s"
        % (n, " (protocol.rs against the mutant blob)" if role == "W7" else "", diffs[:5]))
    pr = os.path.join(wt, SPEC["protocol_path"])
    with open(pr, "rb") as f:
        pdata = f.read()
    pid, psha = git_blob_id(pdata), sha256_bytes(pdata)
    if role == "W":
        add("WT-PROTOCOL", pid == SPEC["protocol_w_blob"] and psha == SPEC["protocol_w_sha"], "%s %s" % (pid, psha))
    else:
        add("WT-PROTOCOL", pid == SPEC["mutant_blob"] and psha == SPEC["mutant_sha"], "%s %s" % (pid, psha))
    names = [d[0] for d in diffs_w]
    if role == "W":
        add("WT-DIFF", names == [], "tracked paths whose content differs from W-TREE: %s" % names)
    else:
        add("WT-DIFF", names == [SPEC["protocol_path"]], "tracked paths whose content differs from W-TREE: %s" % names)
    st = g(wt, "status", "--porcelain=v1", "--ignored").stdout.decode().splitlines()
    allowed = {"!! target/"} | ({" M " + SPEC["protocol_path"]} if role == "W7" else set())
    add("WT-STATUS", set(st) <= allowed and (role == "W" or (" M " + SPEC["protocol_path"]) in st),
        "status lines: %s" % st)
    tgt = os.path.join(wt, "target")
    add("WT-TARGET", (not os.path.lexists(tgt)) or (os.path.isdir(tgt) and not os.path.islink(tgt)),
        "target %s" % ("absent" if not os.path.lexists(tgt) else "real directory"))
    obj = g(wt, "cat-file", "-e", SPEC["mutant_blob"])
    add("WT-OBJECT", obj.returncode != 0, "blob %s %s in the object store" % (SPEC["mutant_blob"][:8],
                                                                            "ABSENT" if obj.returncode != 0 else "PRESENT"))
    return out


def report(checks, label):
    ok = all(c[1] for c in checks)
    lines = ["%s %s: %s" % ("[PASS]" if c[1] else "[FAIL]", c[0], c[2]) for c in checks]
    lines.append("%s: %s" % (label, "OK" if ok else "NOT OK"))
    return ok, "\n".join(lines)


def write_new(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "x", encoding="utf-8") as f:
        f.write(text if text.endswith("\n") else text + "\n")


def secure_parent(path):
    p = os.path.dirname(path)
    st = os.lstat(p)
    return os.path.isdir(p) and not os.path.islink(p) and st.st_uid == os.getuid() and not (st.st_mode & 0o002)


def create(role, evdir):
    wt = SPEC["wt"][role]
    if os.path.lexists(wt):
        raise SystemExit("REFUSED: %s exists" % wt)
    if not secure_parent(wt):
        raise SystemExit("REFUSED: parent of %s is not securely owned" % wt)
    if worktree_entry(wt) is not None:
        raise SystemExit("REFUSED: %s is already registered" % wt)
    before = ownership(os.path.dirname(wt))
    t0 = now()
    p = git("worktree", "add", "--detach", wt, SPEC["commit"], cwd=SPEC["repo"], check=False)
    rec = {"mission": MISSION, "role": role, "path": wt, "started": t0, "finished": now(),
           "command": ["git", "-C", SPEC["repo"], "worktree", "add", "--detach", wt, SPEC["commit"]],
           "returncode": p.returncode, "stdout": p.stdout.decode("utf-8", "replace"), "stderr": p.stderr.decode("utf-8", "replace"),
           "parent_ownership": before}
    if p.returncode != 0:
        write_new(os.path.join(evdir, "receipts/00-worktrees/creation-%s-FAILED.json" % role), json.dumps(rec, indent=1))
        raise SystemExit("worktree add failed: %s" % rec["stderr"])
    os.mkdir(os.path.join(wt, "target"))
    rec["ownership"] = ownership(wt)
    rec["target_ownership"] = ownership(os.path.join(wt, "target"))
    rec["worktree_list_entry"] = worktree_entry(wt)
    rec["dot_git"] = open(os.path.join(wt, ".git")).read().strip()
    chk = verify(wt, "W")
    rec["verification_EXACTLY_W"] = [{"id": a, "ok": b, "detail": c} for a, b, c in chk]
    write_new(os.path.join(evdir, "receipts/00-worktrees/creation-%s.json" % role), json.dumps(rec, indent=1))
    ok, text = report(chk, "EXACTLY_W after creation")
    print(text)
    return 0 if ok else 1


def patch_bytes():
    return git("cat-file", "blob", "%s:%s" % (SPEC["patch_commit"], SPEC["patch_path"]), cwd=SPEC["patch_repo"], check=True).stdout


def write_patch(evdir):
    path = os.path.join(evdir, "receipts/00-worktrees", PATCH_FILE)
    data = patch_bytes()
    if sha256_bytes(data) != SPEC["patch_sha"] or git_blob_id(data) != SPEC["patch_blob"]:
        raise SystemExit("PATCH identity mismatch")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "xb") as f:
        f.write(data)
    return path


def apply_patch(evdir, out):
    wt = SPEC["wt"]["M"]
    lines = []
    chk = verify(wt, "W")
    ok, text = report(chk, "EXACTLY_W before the patch")
    lines.append(text)
    if not ok:
        write_new(out, "\n".join(lines))
        print("\n".join(lines))
        return 1
    pth = write_patch(evdir)
    lines.append("patch file %s sha256 %s (catalog value %s)" % (pth, sha256_file(pth), SPEC["patch_sha"]))
    c = g(wt, "apply", "--check", pth)
    lines.append("git apply --check rc=%d %s" % (c.returncode, c.stderr.decode()))
    if c.returncode != 0:
        write_new(out, "\n".join(lines))
        print("\n".join(lines))
        return 1
    a = subprocess.run(["git", "-C", wt, "apply", pth], capture_output=True, timeout=GIT_TIMEOUT,
                       env=dict(os.environ, GIT_OPTIONAL_LOCKS="0", LC_ALL="C"))
    lines.append("git apply (worktree only: no --index, no --cached) rc=%d %s" % (a.returncode, a.stderr.decode()))
    chk = verify(wt, "W7")
    ok2, text2 = report(chk, "EXACTLY_W7 after the patch")
    lines.append(text2)
    write_new(out, "\n".join(lines))
    print("\n".join(lines))
    return 0 if (a.returncode == 0 and ok2) else 1


def auth_r(evdir):
    """7.3A R-1..R-8 for the mutant worktree. Returns a list of (id, ok, detail)."""
    wt = SPEC["wt"]["M"]
    out = []

    def add(cid, ok, detail=""):
        out.append((cid, bool(ok), detail))

    crp = os.path.join(evdir, "receipts/00-worktrees/creation-M.json")
    try:
        cr = json.load(open(crp))
    except Exception as e:  # noqa
        cr = None
        add("R-3", False, "creation record unreadable: %s" % e)
    add("R-1", os.path.realpath(wt) == wt == SPEC["wt"]["M"] and os.path.isdir(wt) and not os.path.islink(wt), wt)
    ent = worktree_entry(wt)
    add("R-2", ent is not None and ent.get("HEAD") == SPEC["commit"] and "detached" in ent, json.dumps(ent))
    if cr is not None:
        st = os.lstat(wt)
        add("R-3", cr.get("mission") == MISSION and cr.get("path") == wt and cr["ownership"]["ino"] == st.st_ino
            and cr["ownership"]["dev"] == st.st_dev and cr["ownership"]["uid"] == st.st_uid, "creation record names this worktree")
    _d, diffs, n, _ = scan_tracked(wt, "W")
    add("R-4", [d[0] for d in diffs] == [SPEC["protocol_path"]], "tracked paths differing from W-TREE: %s" % [d[0] for d in diffs])
    pr = os.path.join(wt, SPEC["protocol_path"])
    try:
        data = open(pr, "rb").read()
        add("R-5", git_blob_id(data) == SPEC["mutant_blob"] and sha256_bytes(data) == SPEC["mutant_sha"],
            "%s %s" % (git_blob_id(data), sha256_bytes(data)))
    except OSError as e:
        add("R-5", False, str(e))
    cached = g(wt, "diff-index", "--cached", "--name-status", "HEAD")
    st = g(wt, "status", "--porcelain=v1", "--ignored").stdout.decode().splitlines()
    add("R-6", cached.returncode == 0 and cached.stdout.strip() == b""
        and set(st) <= {"!! target/", " M " + SPEC["protocol_path"]}, "status: %s" % st)
    pth = os.path.join(evdir, "receipts/00-worktrees", PATCH_FILE)
    try:
        pd = open(pth, "rb").read()
        add("R-7", sha256_bytes(pd) == SPEC["patch_sha"] and pd == patch_bytes(), "patch sha256 %s" % sha256_bytes(pd))
    except OSError as e:
        add("R-7", False, str(e))
        pth = None
    if pth:
        c = g(wt, "apply", "-R", "--check", pth)
        add("R-8", c.returncode == 0, "git apply -R --check rc=%d %s" % (c.returncode, c.stderr.decode()[:200]))
    else:
        add("R-8", False, "no patch file")
    return out


def preserve(evdir, outdir):
    """7.3A step 1: preserve evidence first. Changes nothing in the worktree."""
    wt = SPEC["wt"]["M"]
    os.makedirs(outdir, exist_ok=True)
    f = lambda n: os.path.join(outdir, n)  # noqa
    write_new(f("status-porcelain-ignored.txt"), g(wt, "status", "--porcelain=v1", "--ignored").stdout.decode())
    write_new(f("head.txt"), g(wt, "rev-parse", "HEAD").stdout.decode() + g(wt, "symbolic-ref", "-q", "HEAD").stdout.decode())
    write_new(f("worktree-list-entry.json"), json.dumps(worktree_entry(wt), indent=1))
    write_new(f("diff.txt"), g(wt, "diff", "HEAD").stdout.decode())
    write_new(f("diff-index-cached.txt"), g(wt, "diff-index", "--cached", "--name-status", "HEAD").stdout.decode())
    try:
        d = open(os.path.join(wt, SPEC["protocol_path"]), "rb").read()
        write_new(f("mutant-protocol.identity.txt"), "git blob id %s\nsha256 %s\nbytes %d\n" % (git_blob_id(d), sha256_bytes(d), len(d)))
    except OSError as e:
        write_new(f("mutant-protocol.identity.txt"), "unreadable: %s\n" % e)
    write_new(f("ownership.json"), json.dumps(ownership(wt), indent=1))
    return 0


def restore(evdir, outdir):
    wt = SPEC["wt"]["M"]
    lines = ["RESTORATION (frozen inverse patch) %s" % now()]
    auth = auth_r(evdir)
    ok, text = report(auth, "7.3A authentication R-1..R-8")
    lines.append(text)
    if not ok:
        failing = [c[0] for c in auth if not c[1]]
        lines.append("RESTORATION NOT PERFORMED (failing condition: %s)" % ", ".join(failing))
        write_new(os.path.join(outdir, "restoration.txt"), "\n".join(lines))
        print("\n".join(lines))
        return 2
    pth = os.path.join(evdir, "receipts/00-worktrees", PATCH_FILE)
    c = g(wt, "apply", "-R", "--check", pth)
    lines.append("git apply -R --check rc=%d %s" % (c.returncode, c.stderr.decode()))
    if c.returncode != 0:
        lines.append("RESTORATION NOT PERFORMED (failing condition: R-8)")
        write_new(os.path.join(outdir, "restoration.txt"), "\n".join(lines))
        print("\n".join(lines))
        return 2
    try:
        a = subprocess.run(["git", "-C", wt, "apply", "-R", pth], capture_output=True, timeout=60,
                           env=dict(os.environ, GIT_OPTIONAL_LOCKS="0", LC_ALL="C"))
        lines.append("git apply -R (one attempt, 60 s ceiling) rc=%d %s" % (a.returncode, a.stderr.decode()))
        applied = a.returncode == 0
    except subprocess.TimeoutExpired:
        lines.append("git apply -R TIMED OUT (60 s)")
        applied = False
    chk = verify(wt, "W")
    ok2, text2 = report(chk, "EXACTLY_W after restoration")
    lines.append(text2)
    final = applied and ok2
    lines.append("RESTORATION %s" % ("COMPLETE: EXACTLY_W" if final else "FAILED"))
    write_new(os.path.join(outdir, "restoration.txt"), "\n".join(lines))
    print("\n".join(lines))
    return 0 if final else 3


def main(argv):
    cmd = argv[1]
    if cmd == "create":
        return create(argv[2], argv[3])
    if cmd == "verify":
        wt = SPEC["wt"][argv[2]]
        ok, text = report(verify(wt, argv[3]), "EXACTLY_%s" % argv[3])
        print(text)
        if len(argv) > 4:
            write_new(argv[4], text)
        return 0 if ok else 1
    if cmd == "apply":
        return apply_patch(argv[2], argv[3])
    if cmd == "restore":
        return restore(argv[2], argv[3])
    if cmd == "preserve":
        return preserve(argv[2], argv[3])
    raise SystemExit("usage")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
