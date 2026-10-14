#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 11 and 15: read-only checks before and after
publication. Adapted from CONTINUE-02 tools/publish_checks.py (the same checks, the same parent 3695a817, packet path
and review ref), plus: both read-only roots unchanged; no copy of any Candidate C file outside docs/evidence/ staged;
every remote tree entry read back (not only the compare API's first 300 files); and a preflight mode that runs, before
any commit exists, every check that does not need one.

  publish_checks.py preflight                   hooks, workflow triggers (the parent's tree, which the commit's tree
                                                 equals outside the packet), rulesets, branch protection, remote state,
                                                 destination absence
  publish_checks.py pre     <evidence-commit>   confinement, lineage, C and the previous packets unchanged, nothing
                                                 mutant, executable, target or source staged, text only, both read-only
                                                 roots unchanged, plus every preflight check on the commit's tree
  publish_checks.py post    <evidence-commit>   remote ref, commit/tree/parent, the full remote tree, key files,
                                                 SHA256SUMS, both maps and the Architect records
  publish_checks.py actions <evidence-commit>   GitHub Actions readback without event filtering

Each check prints one line; exit 0 only if every check holds. Nothing is written to the repository or the remote.
"""
import base64
import hashlib
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402

REPO, REMOTE, SLUG = Q.REPO, Q.REMOTE, Q.SLUG
C, C_TREE, PARENT, PARENT_TREE, EXEC01, R2 = Q.C, Q.C_TREE, Q.PARENT, Q.RESUME01_TREE, Q.EXEC01, Q.R2
PREV_PACKET, RESUME_PACKET, R2_PACKET = Q.EXEC01_PACKET + "/", Q.RESUME01_PACKET + "/", Q.R2_PACKET + "/"
PREV_BRANCH, RESUME_BRANCH = Q.EXEC01_REF[len("refs/heads/"):], Q.RESUME01_REF[len("refs/heads/"):]
CATALOG, CATALOG_BLOB = Q.CATALOG_PATH, Q.CATALOG_BLOB
EV = "docs/evidence/p3-candidate11/"
PACKET = Q.PACKET + "/"
ALLOWED = (PACKET,)
BRANCH, DEST = Q.BRANCH, Q.DEST
PROTECTED = ["refs/heads/main", "refs/heads/rebuild/phase0-trust-boundary"]
LIMIT = Q.LIMIT
EMPTY = Q.EMPTY_BLOB
BEFORE = os.path.join(Q.P, "receipts/00-ROOT-MANIFESTS-BEFORE")
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


def roots_unchanged():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import manifest as M
    for name in ("continue-02", "publish-03"):
        before = M.load(os.path.join(BEFORE, name + ".tsv"))
        now = {r[0]: r for r in M.rows(Q.ROOTS[name])}
        changed = sorted(p for p in before if p in now and now[p] != before[p])
        missing = sorted(p for p in before if p not in now)
        added = sorted(p for p in now if p not in before)
        check(f"the read-only {name} root equals its before-manifest (every entry, field for field; nothing added or "
              f"removed)", not (changed or missing or added),
              f"{len(now)} entries; changed {changed[:3]}; missing {missing[:3]}; added {added[:3]}")


def environment(te, label):
    import yaml
    hp = git("config", "--get", "core.hooksPath", ok=(0, 1)).decode().strip()
    hooks_dir = hp or f"{REPO}/.git/hooks"
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
    check(f"no workflow in {label} has a push trigger matching {BRANCH}", not any(m for _, m, _ in trig),
          f"{len(wf)} workflows")
    check("glob semantics self-test", glob_re("implement/**").match("implement/a/b") and not
          glob_re("implement/**").match(BRANCH) and glob_re("review/**").match(BRANCH) and not glob_re("main").match(BRANCH))
    rc, out = gh(f"repos/{SLUG}/actions/workflows?per_page=100")
    active = json.loads(out)["workflows"] if rc == 0 else None
    check("remote workflow list read", active is not None, f"rc {rc}")
    if active:
        for w in active:
            print(f"     remote workflow {w['path']}: state {w['state']}; in {label}: {w['path'] in te}")
        extra = [w["path"] for w in active if w["state"] == "active" and w["path"] not in te
                 and w["path"].startswith(".github/workflows/")]
        check(f"active remote workflows absent from {label} cannot run on its push (GitHub reads the pushed commit's "
              "workflow files): recorded", True, extra)
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
    check("the R2 review ref still points at the R2 evidence commit", rm.get(Q.R2_REF) == R2)
    check("the EXEC-01 review ref still points at the EXEC-01 evidence commit", rm.get(Q.EXEC01_REF) == EXEC01)
    check("the RESUME-01 review ref still points at the parent", rm.get(Q.RESUME01_REF) == PARENT)
    for p in PROTECTED:
        print(f"     remote {p}: {rm.get(p)}")
    digest = hashlib.sha256("\n".join(sorted(remote)).encode()).hexdigest()
    print(f"     remote refs: {len(remote)}; sha256 of the sorted ls-remote listing {digest}")
    print(f"     remote URL: {git('remote', 'get-url', REMOTE).decode().strip()}; push config: "
          f"{git('config', '--get-regexp', '^remote\\.' + REMOTE + '\\.push', ok=(0, 1)).decode().strip() or 'none'}")


def preflight(_=None):
    check("the parent's tree is unchanged", git("rev-parse", PARENT + "^{tree}").decode().strip() == PARENT_TREE)
    check("Candidate C is unchanged", git("rev-parse", C + "^{tree}").decode().strip() == C_TREE)
    tp = tree_map(PARENT)
    check("the packet path is absent at the parent", not [p for p in tp if p.startswith(PACKET)])
    environment(tp, "the parent's tree")
    roots_unchanged()


def pre(e):
    info = git("cat-file", "commit", e).decode()
    head = info.partition("\n\n")[0].splitlines()
    parents = [l[7:] for l in head if l.startswith("parent ")]
    check("evidence commit has the single parent 3695a817 (the RESUME-01 evidence commit)", parents == [PARENT], parents)
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
    check("the EXEC-01 packet is unchanged (every path has the same mode and blob)",
          len(prev) == Q.EXEC01_COUNT and all(te.get(p) == tp.get(p) for p in prev), f"{len(prev)} paths")
    res = [p for p in set(te) | set(tp) if p.startswith(RESUME_PACKET)]
    check("the RESUME-01 packet is unchanged (every path has the same mode and blob)",
          len(res) == Q.RESUME01_COUNT and all(te.get(p) == tp.get(p) for p in res), f"{len(res)} paths")
    outside = sorted(p for p in set(te) | set(tc) if te.get(p) != tc.get(p) and not p.startswith("docs/evidence/"))
    check("no path outside docs/evidence/ differs from Candidate C", not outside, outside[:5])
    check("Candidate C is unchanged", git("rev-parse", C + "^{tree}").decode().strip() == C_TREE)
    check("the current-tree manifest is unchanged", te.get(EV + "current-tree-183/manifest.json") ==
          tp.get(EV + "current-tree-183/manifest.json") == tc.get(EV + "current-tree-183/manifest.json"))
    modes = sorted({te[p][0] for p in changed})
    check("added files are regular, non-executable blobs (no symlink, no submodule)", modes == ["100644"], modes)
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
    source_blobs = {v[2] for p, v in tc.items() if not p.startswith("docs/evidence/")} - {EMPTY}
    copies = [p for p in changed if te[p][2] in source_blobs]
    check("no source copy: no added file equals any blob of Candidate C outside docs/evidence/ (the empty blob excluded)",
          not copies, copies)
    cat = json.loads(git("cat-file", "blob", te[CATALOG][2]))
    mut = {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}
    staged_mut = [p for p in changed if te[p][2] in mut]
    check("no mutant source staged (no added blob equals any catalog mutant blob)", not staged_mut, staged_mut)
    present = [b for b in sorted(mut) if subprocess.run(["git", "-C", REPO, "cat-file", "-e", b],
                                                         capture_output=True).returncode == 0]
    check("no catalog mutant blob exists in the object store", not present, present[:3])
    elf = [p for p in changed if git("cat-file", "blob", te[p][2])[:4] == b"\x7fELF"]
    check("no executable binary staged", not elf, elf)
    targets = [p for p in changed if "/target/" in p or p.endswith((".rlib", ".rmeta", ".so", ".o", ".pyc"))]
    check("no target directory, build artifact or bytecode staged", not targets, targets[:3])
    environment(te, "the evidence commit's tree")
    roots_unchanged()


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
    check("the RESUME-01, EXEC-01 and R2 review refs are unchanged", (rm.get(Q.RESUME01_REF), rm.get(Q.EXEC01_REF),
                                                                     rm.get(Q.R2_REF)) == (PARENT, EXEC01, R2))
    for p in PROTECTED:
        print(f"     remote {p}: {rm.get(p)}")
    rc, out = gh(f"repos/{SLUG}/commits/{e}")
    d = json.loads(out) if rc == 0 else {}
    tree = git("rev-parse", e + "^{tree}").decode().strip()
    check("GitHub commit object: sha", d.get("sha") == e)
    check("GitHub commit object: tree", d.get("commit", {}).get("tree", {}).get("sha") == tree,
          d.get("commit", {}).get("tree", {}).get("sha"))
    check("GitHub commit object: single parent 3695a817", [p["sha"] for p in d.get("parents", [])] == [PARENT])
    rc, out = gh(f"repos/{SLUG}/compare/{PARENT}...{e}")
    cmp = json.loads(out) if rc == 0 else {}
    files = [f["filename"] for f in cmp.get("files", [])]
    local_tree = tree_map(e)
    local = sorted(p for p in local_tree if p.startswith(PACKET))
    check("GitHub compare: ahead by 1, behind by 0", (cmp.get("ahead_by"), cmp.get("behind_by")) == (1, 0),
          (cmp.get("ahead_by"), cmp.get("behind_by")))
    check("GitHub compare: every listed file is added under the allowed path",
          files and all(f.startswith(ALLOWED) for f in files) and all(f["status"] == "added" for f in cmp.get("files", [])),
          f"{len(files)} files listed (the compare API lists at most 300; {len(local)} local)")
    rc, out = gh(f"repos/{SLUG}/git/trees/{tree}?recursive=1")
    rt = json.loads(out) if rc == 0 else {}
    remote_tree = {x["path"]: (x["mode"], x["type"], x["sha"]) for x in rt.get("tree", []) if x["type"] == "blob"}
    check("GitHub tree (recursive) read in full (not truncated)", rc == 0 and rt.get("truncated") is False,
          f"{len(remote_tree)} blobs")
    check("every remote tree entry equals the local evidence commit's (path, mode, blob): all changed paths read back",
          remote_tree == {p: v for p, v in local_tree.items()},
          f"{len(remote_tree)} remote; {len(local_tree)} local; differing "
          f"{sorted(p for p in set(remote_tree) | set(local_tree) if remote_tree.get(p) != local_tree.get(p))[:3]}")
    key = ["SHA256SUMS", "INDEX.md", "INDEX-PUBLICATION-RECOVERY.md", "TRANCHE-SUMMARY.md", "CARRIED-FORWARD.md",
           "ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md", "ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md",
           "publication/10-build-packet.txt", "receipts/80-STOP-RECORD.md", "receipts/90-final-gate.txt",
           "receipts/90-final-gate-attempt-1-CRASHED.txt", "receipts/05-FREEZE.json", "receipts/06-PRE-EXECUTION-GATE.txt",
           "tools/build_packet.py",
           "publication-recovery/publish-03/documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md",
           "publication-recovery/publish-03/documents/DUPLICATE-CONTENT-MAP.tsv",
           "publication-recovery/publish-03/receipts/80-STOP-RECORD.md",
           "publication-recovery/publish-03/receipts/04-REPAIR-CONTROLS/CONTROLS.json",
           "publication-recovery/publish-03/tools/build_packet.py",
           "publication-recovery/publish-04/documents/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md",
           "publication-recovery/publish-04/documents/NONEMPTY-DUPLICATE-MAP.tsv",
           "publication-recovery/publish-04/documents/NONEMPTY-DUPLICATE-MAP.md",
           "publication-recovery/publish-04/documents/ZERO-LENGTH-ARTIFACT-MAP.tsv",
           "publication-recovery/publish-04/documents/ZERO-LENGTH-ARTIFACT-MAP.md",
           "publication-recovery/publish-04/tools/build_packet.py",
           "publication-recovery/publish-04/receipts/04-REPAIR-CONTROLS/CONTROLS.json",
           "publication-recovery/publish-04/receipts/10-FREEZE-RECORD.txt",
           "publication-recovery/publish-04/receipts/20-FINAL-GATE.txt"]
    for p in sorted(local_tree):
        if not p.startswith(PACKET + "receipts/"):
            continue
        rel = p[len(PACKET):]
        if re.match(r"receipts/\d\d-C[0-9a-z]+/(RESULT\.txt|CONTROL-RESULT\.json|02-apply/APPLY\.txt|"
                    r"03-build/CLASSIFICATION\.txt|03-build/PIN\.json|04-run/CLASSIFICATION\.txt|04-run/raw-output\.txt|"
                    r"05-restore/RESTORE\.txt|06-link/LINK\.txt|06-link/LINK\.json|01-admission/ADMISSION\.txt|"
                    r"02-pin/PIN\.txt|02-pin/PIN\.json)$", rel):
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
    packet = sorted(p[len(PACKET):] for p in local_tree if p.startswith(PACKET))
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
    Q.no_bytecode()
    {"preflight": preflight, "pre": pre, "post": post, "actions": actions}[sys.argv[1]](
        sys.argv[2] if len(sys.argv) > 2 else None)
    print("RESULT: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    sys.exit(0 if not fails else 1)
