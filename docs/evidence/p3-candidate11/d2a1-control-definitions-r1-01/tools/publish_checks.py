#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission section 14: read-only checks before and after publication.
Adapted from MUTATION-TRANCHE1-01 tools/publish_checks.py (the same checks), with this mission's parent 59df2103, its
one allowed path, the destination ref, the 4 MiB limit, and its key blobs (the catalog, the definitions record, all
35 patches, read back with their sha256).

  publish_checks.py pre     <evidence-commit>   confinement, lineage, C unchanged, nothing mutant staged, hooks,
                                                 workflow triggers, rulesets, remote state and destination absence
  publish_checks.py post    <evidence-commit>   remote ref, commit/tree/parent, changed paths, key blobs
  publish_checks.py actions <evidence-commit>   GitHub Actions readback without event filtering

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
PARENT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
PARENT_TREE = "12d5dd2405c59ce9a83c225c0e42b6585d8da107"
EV = "docs/evidence/p3-candidate11/"
ALLOWED = (EV + "d2a1-control-definitions-r1-01/",)
PACKET = EV + "d2a1-control-definitions-r1-01/"
BRANCH = "review/p3-c11-d2a1-control-definitions-r1-01"
DEST = "refs/heads/" + BRANCH
PROTECTED = ["refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"]
LIMIT = 4 * 1024 * 1024
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
    parents = [l[7:] for l in head if l.startswith("parent ")]
    check("evidence commit has the single parent 59df2103", parents == [PARENT], parents)
    check("the parent's tree is unchanged", git("rev-parse", PARENT + "^{tree}").decode().strip() == PARENT_TREE)
    te, tp, tc = tree_map(e), tree_map(PARENT), tree_map(C)
    changed = sorted(p for p in set(te) | set(tp) if te.get(p) != tp.get(p))
    check("every changed path is added under an allowed path (no modification, no deletion)",
          changed and all(p.startswith(ALLOWED) and p not in tp for p in changed), f"{len(changed)} paths")
    check("the changed paths are this mission's packet only", all(p.startswith(PACKET) for p in changed))
    outside = sorted(p for p in set(te) | set(tc) if te.get(p) != tc.get(p) and not p.startswith("docs/evidence/"))
    check("no path outside docs/evidence/ differs from Candidate C", not outside, outside[:5])
    check("Candidate C is unchanged", git("rev-parse", C + "^{tree}").decode().strip() == C_TREE)
    check("the current-tree manifest is unchanged", te.get(EV + "current-tree-183/manifest.json") ==
          tp.get(EV + "current-tree-183/manifest.json") == tc.get(EV + "current-tree-183/manifest.json"))
    modes = sorted({te[p][0] for p in changed})
    check("added files are regular, non-executable blobs", modes == ["100644"], modes)
    sizes = sum(int(git("cat-file", "-s", te[p][2]).decode()) for p in changed)
    check("added bytes at most 4 MiB", sizes <= LIMIT, f"{sizes} bytes")
    nontext = [p for p in changed if b"\0" in git("cat-file", "blob", te[p][2])]
    check("text only (no NUL byte in any added file)", not nontext, nontext)
    rs = [p for p in changed if p.endswith(".rs")]
    check("no Rust source added", not rs, rs)
    crate_blobs = {v[2] for p, v in tc.items() if p.startswith("crates/")}
    copies = [p for p in changed if te[p][2] in crate_blobs]
    check("no added file equals a crate source blob of C", not copies, copies)
    cat = json.loads(git("cat-file", "blob", te[PACKET + "catalog/catalog.json"][2]))
    mutants = {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}
    staged_mut = [p for p in changed if te[p][2] in mutants]
    check("no mutant source staged (no added blob equals any catalog mutant blob)", not staged_mut, staged_mut)
    elf = [p for p in changed if git("cat-file", "blob", te[p][2])[:4] == b"\x7fELF"]
    check("no executable binary staged", not elf, elf)
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
    check("rulesets read", rc == 0, f"{len(rules)} rulesets")
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
    check("destination ref absent locally",
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
    check("GitHub commit object: single parent 59df2103", [p["sha"] for p in d.get("parents", [])] == [PARENT])
    rc, out = gh(f"repos/{SLUG}/compare/{PARENT}...{e}")
    cmp = json.loads(out) if rc == 0 else {}
    files = [f["filename"] for f in cmp.get("files", [])]
    local = sorted(p for p in tree_map(e) if p.startswith(PACKET))
    check("GitHub compare: ahead by 1, behind by 0", (cmp.get("ahead_by"), cmp.get("behind_by")) == (1, 0),
          (cmp.get("ahead_by"), cmp.get("behind_by")))
    check("GitHub compare: every changed file is added under an allowed path",
          files and all(f.startswith(ALLOWED) for f in files) and all(f["status"] == "added" for f in cmp.get("files", [])),
          f"{len(files)} files (the compare API lists at most 300)")
    check("GitHub compare: same file set as the local commit (when within the API's listing limit)",
          len(local) > 300 or sorted(files) == local, f"remote {len(files)}, local {len(local)}")
    import base64
    cat = json.loads(git("cat-file", "blob", git("rev-parse", f"{e}:{PACKET}catalog/catalog.json").decode().strip()))
    key = [PACKET + k for k in ("SHA256SUMS", "INDEX.md", "ARCHITECT-CONTROL-DEFINITIONS-R1.md",
                                "C22-RESTORED-BUILD-RULING.md", "catalog/catalog.json", "catalog/CATALOG.md",
                                "catalog/PATCH-IDENTITIES.md", "receipts/20-checker-a.txt", "receipts/21-checker-b.txt",
                                "receipts/30-syntax-check.txt", "receipts/40-negative-controls.txt",
                                "receipts/60-final-gate.txt")]
    key += [PACKET + "catalog/" + c["patch_file"] for c in cat["controls"] if c.get("patch_file")]
    check("35 patch paths read back", len(key) == 12 + 35, len(key))
    for path in key:
        local_blob = git("rev-parse", f"{e}:{path}", ok=(0, 128)).decode().strip()
        local_sha = hashlib.sha256(git("cat-file", "blob", local_blob)).hexdigest() if local_blob else None
        rc, out = gh(f"repos/{SLUG}/contents/{path}?ref={e}")
        d = json.loads(out) if rc == 0 else {}
        remote_blob = d.get("sha")
        if d.get("encoding") == "base64" and d.get("content"):
            remote_sha = hashlib.sha256(base64.b64decode(d["content"])).hexdigest()
        else:
            r2 = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github.raw",
                                 f"repos/{SLUG}/contents/{path}?ref={e}"], capture_output=True)
            remote_sha = hashlib.sha256(r2.stdout).hexdigest() if r2.returncode == 0 else None
        want = None
        for c in cat["controls"]:
            if c.get("patch_file") and path.endswith("catalog/" + c["patch_file"]):
                want = c["patch_sha256"]
        check(f"remote {path.replace(PACKET, '')}: blob {remote_blob} and sha256 {remote_sha} equal local"
              + (" and the catalogued patch sha256" if want else ""),
              remote_blob == local_blob and local_blob and remote_sha == local_sha and (want is None or want == local_sha))


def actions(e):
    """GitHub Actions readback without event filtering: every run for the evidence commit and for the branch."""
    for q in (f"head_sha={e}", f"branch={BRANCH}"):
        rc, out = gh(f"repos/{SLUG}/actions/runs?{q}&per_page=100")
        d = json.loads(out) if rc == 0 else {}
        runs = d.get("workflow_runs", [])
        print(f"     actions runs ({q}, no event filter): total_count {d.get('total_count')}")
        for r in runs:
            print(f"       run {r['id']} {r['name']!r} event {r['event']} status {r['status']} conclusion "
                  f"{r['conclusion']} head {r['head_sha']}")
        check(f"Actions readback ({q}) read", rc == 0, f"rc {rc}")
        check(f"no workflow run for {q}", d.get("total_count") == 0, d.get("total_count"))


if __name__ == "__main__":
    {"pre": pre, "post": post, "actions": actions}[sys.argv[1]](sys.argv[2])
    print("RESULT: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    sys.exit(0 if not fails else 1)
