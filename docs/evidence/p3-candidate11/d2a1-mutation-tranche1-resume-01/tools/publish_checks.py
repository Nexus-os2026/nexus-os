#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 20: read-only checks before and after publication.
Adapted from CONTROL-DEFINITIONS-R2-01 tools/publish_checks.py (the same checks), with this mission's parent 9f22f890
(the R2 evidence commit), its one allowed path, the destination ref, the 32 MiB limit, and its key receipts read back.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 20): parent 19eda8cc (the EXEC-01 evidence commit),
the resume packet path and review ref, the previous packet checked unchanged, and the resume receipts read back.

  publish_checks.py pre     <evidence-commit>   confinement, lineage, C and the R2 packet unchanged, nothing mutant or
                                                 executable staged, text only, hooks, workflow triggers, rulesets,
                                                 remote state and destination absence
  publish_checks.py post    <evidence-commit>   remote ref, commit/tree/parent, changed paths, key receipts, manifest
  publish_checks.py actions <evidence-commit>   GitHub Actions readback without event filtering

Each check prints one line; exit 0 only if every check holds. Nothing is written to the repository or the remote.
"""
import base64
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
PARENT = "19eda8cc7f559054933fed4a93bdb0a18cc725d4"
PARENT_TREE = "1b313ec5c23b5bf76db90b5fa89758d9cd880a75"
R2 = "9f22f890d8532df395f22cd8db8db2552bfdd14c"
PREV_PACKET = "docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/"
PREV_BRANCH = "review/p3-c11-d2a1-mutation-tranche1-exec-01"
R2_PACKET = "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/"
CATALOG = R2_PACKET + "catalog/catalog.json"
CATALOG_BLOB = "1918374263740d2f98640da00fc466737945975e"
EV = "docs/evidence/p3-candidate11/"
PACKET = EV + "d2a1-mutation-tranche1-resume-01/"
ALLOWED = (PACKET,)
BRANCH = "review/p3-c11-d2a1-mutation-tranche1-resume-01"
DEST = "refs/heads/" + BRANCH
PROTECTED = ["refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"]
LIMIT = 32 * 1024 * 1024
EMPTY = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
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


def mutant_blobs(te):
    cat = json.loads(git("cat-file", "blob", te[CATALOG][2]))
    return {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}


def pre(e):
    import yaml
    info = git("cat-file", "commit", e).decode()
    head = info.partition("\n\n")[0].splitlines()
    parents = [l[7:] for l in head if l.startswith("parent ")]
    check("evidence commit has the single parent 19eda8cc (the EXEC-01 evidence commit)", parents == [PARENT], parents)
    check("the parent's tree is unchanged", git("rev-parse", PARENT + "^{tree}").decode().strip() == PARENT_TREE)
    te, tp, tc = tree_map(e), tree_map(PARENT), tree_map(C)
    changed = sorted(p for p in set(te) | set(tp) if te.get(p) != tp.get(p))
    check("every changed path is added under the allowed path (no modification, no deletion)",
          changed and all(p.startswith(ALLOWED) and p not in tp for p in changed), f"{len(changed)} paths")
    r2 = [p for p in set(te) | set(tp) if p.startswith(R2_PACKET)]
    check("the R2 packet is unchanged (every R2 packet path has the same mode and blob)",
          len(r2) == 72 and all(te.get(p) == tp.get(p) for p in r2), f"{len(r2)} paths")
    check("the frozen R2 catalog is unchanged", te.get(CATALOG, ("", "", ""))[2] == CATALOG_BLOB, te.get(CATALOG))
    prev = [p for p in set(te) | set(tp) if p.startswith(PREV_PACKET)]
    check("the previous (EXEC-01) packet is unchanged (every path has the same mode and blob)",
          len(prev) == 963 and all(te.get(p) == tp.get(p) for p in prev), f"{len(prev)} paths")
    outside = sorted(p for p in set(te) | set(tc) if te.get(p) != tc.get(p) and not p.startswith("docs/evidence/"))
    check("no path outside docs/evidence/ differs from Candidate C", not outside, outside[:5])
    check("Candidate C is unchanged", git("rev-parse", C + "^{tree}").decode().strip() == C_TREE)
    check("the current-tree manifest is unchanged", te.get(EV + "current-tree-183/manifest.json") ==
          tp.get(EV + "current-tree-183/manifest.json") == tc.get(EV + "current-tree-183/manifest.json"))
    modes = sorted({te[p][0] for p in changed})
    check("added files are regular, non-executable blobs", modes == ["100644"], modes)
    sizes = sum(int(git("cat-file", "-s", te[p][2]).decode()) for p in changed)
    check("added bytes at most 32 MiB", sizes <= LIMIT, f"{sizes} bytes")
    nontext = []
    for p in changed:
        b = git("cat-file", "blob", te[p][2])
        try:
            b.decode("utf-8")
        except UnicodeDecodeError:
            nontext.append(p)
        if b"\0" in b:
            nontext.append(p)
    check("text only (UTF-8, no NUL byte in any added file)", not nontext, nontext)
    check("no Rust source added", not [p for p in changed if p.endswith(".rs")])
    crate_blobs = {v[2] for p, v in tc.items() if p.startswith("crates/")} - {EMPTY}
    copies = [p for p in changed if te[p][2] in crate_blobs]
    check("no added file equals a crate source blob of C (the empty blob excluded)", not copies, copies)
    staged_mut = [p for p in changed if te[p][2] in mutant_blobs(te)]
    check("no mutant source staged (no added blob equals any catalog mutant blob)", not staged_mut, staged_mut)
    elf = [p for p in changed if git("cat-file", "blob", te[p][2])[:4] == b"\x7fELF"]
    check("no executable binary staged", not elf, elf)
    targets = [p for p in changed if "/target/" in p or p.endswith((".rlib", ".rmeta", ".so", ".o"))]
    check("no target directory or build artifact staged", not targets, targets[:3])
    hp = git("config", "--get", "core.hooksPath", ok=(0, 1)).decode().strip()
    hooks_dir = hp or f"{REPO}/.git/hooks"
    import os
    hooks = sorted(n for n in os.listdir(hooks_dir) if not n.endswith(".sample")) if os.path.isdir(hooks_dir) else []
    check("no active git hook: the effective hooks directory (core.hooksPath, or .git/hooks) holds only *.sample files",
          not hooks, f"hooksPath {hp!r}; effective {hooks_dir}; non-sample entries {hooks}")
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
    rc, out = gh(f"repos/{SLUG}/branches/{BRANCH.replace('/', '%2F')}/protection")
    print(f"     branch protection read for {BRANCH}: rc {rc} ({'none' if rc != 0 else out[:120]})")
    remote = git("ls-remote", REMOTE).decode().splitlines()
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in remote}
    check("destination ref absent on the remote", DEST not in rm)
    check("destination ref absent locally",
          subprocess.run(["git", "-C", REPO, "show-ref", "--verify", "-q", DEST]).returncode == 1)
    check("the R2 review ref still points at the R2 evidence commit",
          rm.get("refs/heads/review/p3-c11-d2a1-control-definitions-r2-01") == R2)
    check("the previous (EXEC-01) review ref still points at the parent", rm.get("refs/heads/" + PREV_BRANCH) == PARENT)
    for p in PROTECTED:
        print(f"     remote {p}: {rm.get(p)}")
    digest = hashlib.sha256("\n".join(sorted(remote)).encode()).hexdigest()
    print(f"     remote refs: {len(remote)}; sha256 of the sorted ls-remote listing {digest}")
    print(f"     remote URL: {git('remote', 'get-url', REMOTE).decode().strip()}; push config: "
          f"{git('config', '--get-regexp', '^remote\\.' + REMOTE + '\\.push', ok=(0, 1)).decode().strip() or 'none'}")


def remote_blob(path, ref):
    rc, out = gh(f"repos/{SLUG}/contents/{path}?ref={ref}")
    d = json.loads(out) if rc == 0 else {}
    blob = d.get("sha")
    if d.get("encoding") == "base64" and d.get("content"):
        sha = hashlib.sha256(base64.b64decode(d["content"])).hexdigest()
    else:
        r2 = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github.raw", f"repos/{SLUG}/contents/{path}?ref={ref}"],
                            capture_output=True)
        sha = hashlib.sha256(r2.stdout).hexdigest() if r2.returncode == 0 else None
    return blob, sha


def post(e):
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in git("ls-remote", REMOTE).decode().splitlines()}
    check("remote destination ref points at the evidence commit", rm.get(DEST) == e, rm.get(DEST))
    rc, out = gh(f"repos/{SLUG}/commits/{e}")
    d = json.loads(out) if rc == 0 else {}
    check("GitHub commit object: sha", d.get("sha") == e)
    check("GitHub commit object: tree", d.get("commit", {}).get("tree", {}).get("sha") ==
          git("rev-parse", e + "^{tree}").decode().strip(), d.get("commit", {}).get("tree", {}).get("sha"))
    check("GitHub commit object: single parent 19eda8cc", [p["sha"] for p in d.get("parents", [])] == [PARENT])
    rc, out = gh(f"repos/{SLUG}/compare/{PARENT}...{e}")
    cmp = json.loads(out) if rc == 0 else {}
    files = [f["filename"] for f in cmp.get("files", [])]
    local = sorted(p for p in tree_map(e) if p.startswith(PACKET))
    check("GitHub compare: ahead by 1, behind by 0", (cmp.get("ahead_by"), cmp.get("behind_by")) == (1, 0),
          (cmp.get("ahead_by"), cmp.get("behind_by")))
    check("GitHub compare: every listed file is added under the allowed path",
          files and all(f.startswith(ALLOWED) for f in files) and all(f["status"] == "added" for f in cmp.get("files", [])),
          f"{len(files)} files listed (the compare API lists at most 300; {len(local)} local)")
    check("GitHub compare: the listed files are local packet files", set(files) <= set(local))
    key = ["SHA256SUMS", "INDEX.md", "TRANCHE-SUMMARY.md", "CARRIED-FORWARD.md", "ARCHITECT-RULING-C22-ACCEPTANCE.md",
           "receipts/00-authentication.txt", "receipts/03-verifier-controls/SUMMARY.txt",
           "receipts/04-tool-repair/TOOL-REPAIR.diff", "receipts/04-tool-repair/TOOL-DIFF-CHECK.txt",
           "receipts/05-FREEZE.json", "receipts/05-FREEZE-RECORD.txt", "receipts/06-PRE-EXECUTION-GATE.txt",
           "receipts/90-final-gate.txt"]
    tm = tree_map(e)
    for p in sorted(tm):
        if not p.startswith(PACKET + "receipts/"):
            continue
        rel = p[len(PACKET):]
        if re.match(r"receipts/\d\d-C[0-9a-z]+/(RESULT\.txt|02-apply/APPLY\.txt|03-build/CLASSIFICATION\.txt|"
                         r"03-build/build\.stderr|03-build/PIN\.json|04-run/CLASSIFICATION\.txt|04-run/raw-output\.txt|"
                         r"05-restore/RESTORE\.txt|06-link/LINK\.txt)$", rel):
            key.append(rel)
    print(f"     reading back {len(key)} key files from GitHub at the evidence commit")
    for rel in key:
        path = PACKET + rel
        local_blob = git("rev-parse", f"{e}:{path}", ok=(0, 128)).decode().strip()
        local_sha = hashlib.sha256(git("cat-file", "blob", local_blob)).hexdigest() if local_blob else None
        rb, rs = remote_blob(path, e)
        check(f"remote {rel}: blob {rb} and sha256 equal local", rb == local_blob and local_blob and rs == local_sha,
              rs)
    sums = git("cat-file", "blob", git("rev-parse", f"{e}:{PACKET}SHA256SUMS").decode().strip()).decode()
    listed = [l.split("  ", 1) for l in sums.splitlines() if l]
    bad = [p for h, p in listed if hashlib.sha256(git("cat-file", "blob", git("rev-parse", f"{e}:{PACKET}{p}")
                                                      .decode().strip())).hexdigest() != h]
    packet = sorted(p[len(PACKET):] for p in tm if p.startswith(PACKET))
    check("packet manifest: SHA256SUMS lists every packet file but itself, and every listed sha256 verifies at the "
          "evidence commit", not bad and sorted(p for _, p in listed) == [p for p in packet if p != "SHA256SUMS"],
          f"{len(listed)} listed")


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
