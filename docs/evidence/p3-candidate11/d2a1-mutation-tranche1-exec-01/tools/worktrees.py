"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission sections 8 and 10: create and authenticate one mission-owned
worktree exactly at Candidate C.

create(path, role)        `git worktree add --detach <path> C` (the path must not exist), then `git worktree lock` with
                          a reason naming this mission and the role (ownership is recorded in the repository's own
                          worktree administration, and the worktree is protected from pruning until review)
authenticate(path, role)  actual filesystem identity (realpath, not a symlink, owner uid, device and inode); worktree
                          ownership (the .git file and the administrative directory point at each other, the worktree
                          list shows it detached at C and locked with this mission's reason); HEAD and tree; a clean
                          status (nothing modified, untracked or ignored); every tracked file byte-equal to C
                          (verify_tree.py); and the baseline file's blob, computed in-process

Both return (record dict, ok). Nothing here deletes, resets or cleans anything.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

VERIFY = os.path.join(K.EV, "tools/verify_tree.py")


def run(args, cwd=None):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    return {"argv": args, "rc": r.returncode, "stdout": r.stdout, "stderr": r.stderr}


def reason(role):
    return f"{K.MISSION} {role}: mission-owned; preserve until Architect review"


def create(path, role):
    rec = {"path": path, "role": role, "existed_before": os.path.lexists(path)}
    if rec["existed_before"]:
        rec["refused"] = "the path exists"
        return rec, False
    rec["add"] = run(["git", "-C", K.REPO, "worktree", "add", "--detach", path, K.C])
    rec["lock"] = run(["git", "-C", K.REPO, "worktree", "lock", "--reason", reason(role), path])
    return rec, rec["add"]["rc"] == 0 and rec["lock"]["rc"] == 0


def worktree_entry(path):
    out = run(["git", "-C", K.REPO, "worktree", "list", "--porcelain"])["stdout"]
    for block in out.strip().split("\n\n"):
        lines = block.splitlines()
        if lines and lines[0] == "worktree " + path:
            return lines
    return None


def authenticate(path, role, baseline_file=K.PROTOCOL):
    rec, checks = {"path": path, "role": role}, []

    def chk(label, ok, detail=""):
        checks.append((label, bool(ok), str(detail)))

    st = os.lstat(path)
    rec["filesystem"] = {"realpath": os.path.realpath(path), "is_symlink": os.path.islink(path), "uid": st.st_uid,
                         "gid": st.st_gid, "dev": st.st_dev, "ino": st.st_ino, "mode": oct(st.st_mode)}
    chk("filesystem identity: the path is its own realpath, a directory, not a symlink",
        os.path.realpath(path) == path and os.path.isdir(path) and not os.path.islink(path), rec["filesystem"])
    chk("owned by this user", st.st_uid == os.getuid(), st.st_uid)
    dotgit = os.path.join(path, ".git")
    gitfile = open(dotgit).read().strip() if os.path.isfile(dotgit) else None
    admin = gitfile[len("gitdir: "):] if gitfile and gitfile.startswith("gitdir: ") else None
    rec["dotgit"] = gitfile
    chk("the worktree's .git file points into the repository's worktree administration",
        admin is not None and os.path.dirname(admin) == os.path.join(K.GIT_COMMON_DIR, "worktrees"), gitfile)
    back = open(os.path.join(admin, "gitdir")).read().strip() if admin and os.path.exists(os.path.join(admin, "gitdir")) \
        else None
    locked = open(os.path.join(admin, "locked")).read().strip() if admin and os.path.exists(os.path.join(admin, "locked")) \
        else None
    rec["admin"] = {"dir": admin, "gitdir": back, "locked": locked}
    chk("the administrative directory points back at this worktree", back == dotgit, back)
    chk("locked with this mission's reason (mission ownership)", locked == reason(role), locked)
    entry = worktree_entry(path)
    rec["worktree_list_entry"] = entry
    chk("git worktree list: this path, HEAD C, detached, locked",
        entry is not None and f"HEAD {K.C}" in entry and "detached" in entry
        and any(l.startswith("locked") for l in entry), entry)
    top = run(["git", "-C", path, "rev-parse", "--show-toplevel"])["stdout"].strip()
    common_dir = run(["git", "-C", path, "rev-parse", "--path-format=absolute", "--git-common-dir"])["stdout"].strip()
    chk("toplevel is the path; the common git directory is the repository's", top == path
        and os.path.realpath(common_dir) == os.path.realpath(K.GIT_COMMON_DIR), f"{top}; {common_dir}")
    head = run(["git", "-C", path, "rev-parse", "HEAD", "HEAD^{tree}"])["stdout"].split()
    sym = run(["git", "-C", path, "symbolic-ref", "-q", "HEAD"])["rc"]
    chk("HEAD is Candidate C and its tree, detached", head == [K.C, K.C_TREE] and sym != 0, head)
    status = run(["git", "-C", path, "status", "--porcelain", "--ignored", "--untracked-files=all"])["stdout"]
    rec["status"] = status
    chk("clean: nothing modified, staged, untracked or ignored", status == "", status[:200])
    v = json.loads(run([sys.executable, VERIFY, path])["stdout"])
    rec["verify_tree"] = v
    chk("every tracked file byte-equal to C (verify_tree.py)", v["verdict"] == "EXACTLY_C",
        f"{v['verdict']}; {v['tracked_blobs_compared']} blobs, {v['bytes_compared']} bytes")
    data = open(os.path.join(path, baseline_file), "rb").read()
    rec["baseline_file"] = {"path": baseline_file, "git_blob": K.git_blob_id(data), "sha256": K.sha256_bytes(data)}
    chk(f"baseline file {baseline_file} blob (in-process) is C's", K.git_blob_id(data) == K.BLOBS[baseline_file],
        K.git_blob_id(data))
    rec["checks"] = checks
    return rec, all(ok for _, ok, _ in checks)


def write(outdir, name, title, rec, ok):
    os.makedirs(outdir, exist_ok=True)
    json.dump(rec, open(os.path.join(outdir, name + ".json"), "w"), indent=1, default=str)
    lines = [title, f"result: {'OK' if ok else 'FAILED'}"]
    for label, good, detail in rec.get("checks", []):
        lines.append(f"{'OK  ' if good else 'FAIL'} {label}: {detail}")
    for k in ("add", "lock"):
        if k in rec:
            lines.append(f"{k}: {' '.join(rec[k]['argv'])} -> rc {rec[k]['rc']}; {rec[k]['stdout'].strip()} "
                         f"{rec[k]['stderr'].strip()}")
    open(os.path.join(outdir, name + ".txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
