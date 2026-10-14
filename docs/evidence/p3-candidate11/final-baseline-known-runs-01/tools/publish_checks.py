#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission section 16: read-only checks before and after publication.

  publish_checks.py pre  <evidence-commit>    confinement, lineage, C unchanged, hooks, workflow triggers, rulesets,
                                               remote state and destination absence
  publish_checks.py post <evidence-commit>    remote ref, commit/tree/parent, changed paths, key blobs, manifest

Each check prints one line; exit 0 only if every check holds. Nothing is written to the repository or the remote.
"""
import hashlib
import json
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
REMOTE = "github"
SLUG = "Nexus-os2026/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
PARENT = "0622a4d042b1dfd3160f06fc5d333e131e2a60c8"
PARENT_TREE = "a512aa45a2daf385081b55e16353f6263b95665d"
PACKET = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/"
BRANCH = "review/p3-c11-d2a1-final-baseline-known-runs-01"
DEST = "refs/heads/" + BRANCH
PROTECTED = ["refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"]
fails = []


def git(*args, ok=(0,)):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True)
    if r.returncode not in ok:
        raise SystemExit(f"git {' '.join(args)}: {r.stderr.decode()}")
    return r.stdout


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def tree_map(c):
    out = {}
    for e in git("ls-tree", "-r", "-z", c).split(b"\0"):
        if e:
            meta, path = e.split(b"\t", 1)
            out[path.decode()] = tuple(meta.decode().split())
    return out


def glob_re(pattern):
    """GitHub Actions branch filter glob: '*' any run of non-'/' characters, '**' any run, '?' one character."""
    out, i = "", 0
    while i < len(pattern):
        if pattern.startswith("**", i):
            out += ".*"
            i += 2
        elif pattern[i] == "*":
            out += "[^/]*"
            i += 1
        elif pattern[i] == "?":
            out += "."
            i += 1
        elif pattern[i] in "[]":
            out += pattern[i]
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile("^" + out + "$")


def push_matches(on, branch):
    """Would a push of `branch` trigger this `on:` block? Fail closed on anything not understood."""
    if on is None:
        return False, "no trigger"
    if isinstance(on, str):
        on = {on: None}
    if isinstance(on, list):
        on = {k: None for k in on}
    if "push" not in on:
        return False, "no push trigger"
    push = on["push"] or {}
    if "branches" in push:
        pats = push["branches"]
        neg = [p[1:] for p in pats if p.startswith("!")]
        pos = [p for p in pats if not p.startswith("!")]
        m = any(glob_re(p).match(branch) for p in pos) and not any(glob_re(p).match(branch) for p in neg)
        return m, f"branches {pats}"
    if "branches-ignore" in push:
        pats = push["branches-ignore"]
        return not any(glob_re(p).match(branch) for p in pats), f"branches-ignore {pats}"
    if "tags" in push or "tags-ignore" in push:
        return False, f"tags only {push}"
    return True, f"push with no branch filter {push}"


def gh(path):
    r = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github+json", path], capture_output=True, text=True)
    return r.returncode, r.stdout


def pre(e):
    import yaml
    info = git("cat-file", "commit", e).decode()
    head = info.partition("\n\n")[0].splitlines()
    tree = [l[5:] for l in head if l.startswith("tree ")][0]
    parents = [l[7:] for l in head if l.startswith("parent ")]
    check("evidence commit has the single parent 0622a4d0", parents == [PARENT], parents)
    check("the parent's tree is unchanged", git("rev-parse", PARENT + "^{tree}").decode().strip() == PARENT_TREE)
    te, tp, tc = tree_map(e), tree_map(PARENT), tree_map(C)
    changed = sorted(p for p in set(te) | set(tp) if te.get(p) != tp.get(p))
    check("every changed path is added under the packet (no modification, no deletion)",
          changed and all(p.startswith(PACKET) and p not in tp for p in changed), f"{len(changed)} paths")
    outside = sorted(p for p in set(te) | set(tc) if te.get(p) != tc.get(p) and not p.startswith("docs/evidence/"))
    check("no path outside docs/evidence/ differs from Candidate C", not outside, outside[:5])
    check("Candidate C is unchanged", git("rev-parse", C + "^{tree}").decode().strip() == C_TREE)
    modes = sorted({te[p][0] for p in changed})
    check("packet files are regular, non-executable blobs", modes == ["100644"], modes)
    sizes = sum(int(git("cat-file", "-s", te[p][2]).decode()) for p in changed)
    check("packet size at most 6 MiB", sizes <= 6 * 1024 * 1024, f"{sizes} bytes")
    nontext = [p for p in changed if b"\0" in git("cat-file", "blob", te[p][2])]
    check("packet is text only (no NUL byte in any file)", not nontext, nontext)
    hooks = subprocess.run(["bash", "-c", f"ls -A {REPO}/.git/hooks | grep -v '\\.sample$' || true"],
                           capture_output=True, text=True).stdout.split()
    hp = git("config", "--get", "core.hooksPath", ok=(0, 1)).decode().strip()
    check("no active git hook (only *.sample files)", not hooks, f"hooksPath {hp}; non-sample entries {hooks}")
    wf = [p for p in te if p.startswith(".github/workflows/") and p.endswith((".yml", ".yaml"))]
    trig = []
    for p in sorted(wf):
        d = yaml.safe_load(git("cat-file", "blob", te[p][2]))
        on = d.get("on", d.get(True))
        m, why = push_matches(on, BRANCH)
        trig.append((p, m, why))
        print(f"     workflow {p}: push of {BRANCH} triggers it: {m} ({why})")
    check(f"no workflow in the evidence commit's tree has a push trigger matching {BRANCH}", not any(m for _, m, _ in trig))
    check("glob semantics self-test", glob_re("implement/**").match("implement/a/b") and not
          glob_re("implement/**").match(BRANCH) and glob_re("review/**").match(BRANCH) and not glob_re("main").match(BRANCH))
    rc, out = gh(f"repos/{SLUG}/actions/workflows?per_page=100")
    active = json.loads(out)["workflows"] if rc == 0 else None
    check("remote workflow list read", active is not None, f"rc {rc}")
    if active:
        for w in active:
            print(f"     remote workflow {w['path']}: state {w['state']}; in the evidence commit's tree: {w['path'] in te}")
        extra = [w["path"] for w in active if w["state"] == "active" and w["path"] not in te
                 and w["path"].startswith(".github/workflows/")]
        check("active remote workflows absent from the pushed tree cannot run on its push (GitHub reads the pushed "
              "commit's workflow files): recorded", True, extra)
    rc, out = gh(f"repos/{SLUG}/rulesets")
    rules = json.loads(out) if rc == 0 else []
    for r in rules:
        rc2, o2 = gh(f"repos/{SLUG}/rulesets/{r['id']}")
        d = json.loads(o2) if rc2 == 0 else {}
        cond = d.get("conditions", {}).get("ref_name", {})
        inc = cond.get("include", [])
        exc = cond.get("exclude", [])
        hit = any(p == "~ALL" or glob_re(p.replace("refs/heads/", "")).match(BRANCH) for p in inc) and not any(
            glob_re(p.replace("refs/heads/", "")).match(BRANCH) for p in exc)
        print(f"     ruleset {r['id']} {r.get('name')!r}: enforcement {d.get('enforcement')}, target {d.get('target')}, "
              f"include {inc}, exclude {exc}; applies to {BRANCH}: {hit}")
    remote = git("ls-remote", REMOTE).decode().splitlines()
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in remote}
    check("destination ref absent on the remote", DEST not in rm)
    check("destination ref absent locally", git("show-ref", "--verify", "-q", DEST, ok=(0, 1)) == b"" and
          subprocess.run(["git", "-C", REPO, "show-ref", "--verify", "-q", DEST]).returncode == 1)
    for p in PROTECTED:
        print(f"     remote {p}: {rm.get(p)}")
    digest = hashlib.sha256("\n".join(sorted(remote)).encode()).hexdigest()
    print(f"     remote refs: {len(remote)}; sha256 of the sorted ls-remote listing {digest}")
    print(f"     remote URL: {git('remote', 'get-url', REMOTE).decode().strip()}; push config: "
          f"{git('config', '--get-regexp', '^remote\\.' + REMOTE + '\\.push', ok=(0, 1)).decode().strip() or 'none'}")


def post(e):
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in git("ls-remote", REMOTE).decode().splitlines()}
    check("remote destination ref points at the evidence commit", rm.get(DEST) == e, rm.get(DEST))
    rc, out = gh(f"repos/{SLUG}/commits/{e}")
    d = json.loads(out) if rc == 0 else {}
    check("GitHub commit object: sha", d.get("sha") == e)
    check("GitHub commit object: tree", d.get("commit", {}).get("tree", {}).get("sha") ==
          git("rev-parse", e + "^{tree}").decode().strip(), d.get("commit", {}).get("tree", {}).get("sha"))
    check("GitHub commit object: single parent 0622a4d0", [p["sha"] for p in d.get("parents", [])] == [PARENT])
    rc, out = gh(f"repos/{SLUG}/compare/{PARENT}...{e}")
    cmp = json.loads(out) if rc == 0 else {}
    files = [f["filename"] for f in cmp.get("files", [])]
    local = sorted(p for p in tree_map(e) if p.startswith(PACKET))
    check("GitHub compare: ahead by 1, behind by 0", (cmp.get("ahead_by"), cmp.get("behind_by")) == (1, 0),
          (cmp.get("ahead_by"), cmp.get("behind_by")))
    check("GitHub compare: every changed file is added under the packet",
          files and all(f.startswith(PACKET) for f in files) and all(f["status"] == "added" for f in cmp.get("files", [])),
          f"{len(files)} files (the compare API lists at most 300)")
    check("GitHub compare: same file set as the local packet (when within the API's listing limit)",
          len(local) > 300 or sorted(files) == local, f"remote {len(files)}, local {len(local)}")
    for rel in ("SHA256SUMS", "INDEX.md", "receipts/10-build/PIN.json", "receipts/30-suite/CLASSIFICATION.txt",
                "receipts/41-full-F-2/CLASSIFICATION.txt", "receipts/42-full-F-B/CLASSIFICATION.txt",
                "receipts/43-full-F-3/CLASSIFICATION.txt", "receipts/44-full-F-PT/CLASSIFICATION.txt",
                "receipts/45-full-F-1/CLASSIFICATION.txt", "receipts/50-A4/A4-COMPARISON.txt"):
        local_blob = git("rev-parse", f"{e}:{PACKET}{rel}", ok=(0, 128)).decode().strip()
        rc, out = gh(f"repos/{SLUG}/contents/{PACKET}{rel}?ref={e}")
        remote_blob = json.loads(out).get("sha") if rc == 0 else None
        check(f"remote blob of {rel} equals local", remote_blob == local_blob and local_blob, remote_blob)


if __name__ == "__main__":
    {"pre": pre, "post": post}[sys.argv[1]](sys.argv[2])
    print("RESULT: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    sys.exit(0 if not fails else 1)
