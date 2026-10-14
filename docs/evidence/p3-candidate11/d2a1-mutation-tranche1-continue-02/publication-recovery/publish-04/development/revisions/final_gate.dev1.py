#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 10, 11 and 14: the one static final publication gate,
after the freeze. Read-only but for its own outputs: receipts/21-ROOT-MANIFESTS-AFTER/{continue-02,publish-03}.tsv (the
after-manifests of both read-only roots). Its console is receipts/20-FINAL-GATE.txt.

    final_gate.py                   the gate (once, after the freeze)
    final_gate.py --rehearse DIR    the same checks on a scratch rehearsal (DIR/packet, DIR/receipts/ holding its
                                    freeze and receiving its after-manifests); the real worktree must still be empty

Checks: the freeze (every frozen file of this root and every staged packet file unchanged; nothing added but the
freeze and the gate's own outputs); no Cargo, rustc, test, explorer or other executable command in any tool this
mission ran (static inventory of every process call), and none running now; both read-only roots unchanged (after ==
before, byte for byte); exactly 21 mapped nonempty duplicates and 8 mapped zero-length artifacts, no unmapped
duplicate and no unlisted empty file; every repair control as expected; the builder diff verdict; the previous
failures and stop records present; the complete C28, C12, C9, C11, C17, C18a, C18b evidence present, results not
relabelled; no source, executable, target or credential; packet confinement; Candidate C, the catalog and the previous
commits unchanged; the destination absent; SHA256SUMS complete and valid; the size with the post-gate records; the
whitespace classification (every file this mission authored clean; every retained raw finding listed).
"""
import ast
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble as A  # noqa: E402
import build_packet as B  # noqa: E402
import manifest as M  # noqa: E402
import pub4common as Q  # noqa: E402

DEST = os.path.join(Q.WORKTREE, Q.PACKET)
BEFORE = os.path.join(Q.P, "receipts/00-ROOT-MANIFESTS-BEFORE")
P4D = A.P4_DIR
# This mission's packet files that are raw tool output, unified diffs or preserved development failures (byte-exact;
# their whitespace findings are listed, never edited). Every other file under publish-04/, and the root index, is
# authored by this mission and must be whitespace-clean.
RAW_P4 = {f"{P4D}/receipts/03-BUILDER-DIFF/FROZEN-TO-PUBLISH-03.diff",
          f"{P4D}/receipts/03-BUILDER-DIFF/PUBLISH-03-TO-PUBLISH-04.diff",
          f"{P4D}/receipts/03-BUILDER-DIFF/FROZEN-TO-PUBLISH-04.diff",
          f"{P4D}/receipts/04-REPAIR-CONTROLS/P4-builder-run.txt",
          f"{P4D}/receipts/07-ASSEMBLE.console.txt",
          f"{P4D}/development/build-packet-dev1-FAILED-official-maps-not-yet-written.txt",
          f"{P4D}/development/repair-controls-dev1-FAILED-N24b-incomplete-expectation.txt",
          f"{P4D}/development/repair_controls.dev1-to-dev2.diff",
          f"{P4D}/development/authenticate.dev1-to-dev2.diff",
          f"{P4D}/development/superseded-1-authentication-trailing-space/01-AUTHENTICATION.txt"}
ALLOWED_PROGRAMS = {"git", "gh", "ps"}  # ps: this gate's own read-only process listing
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def whitespace(data):
    """git's default whitespace errors: blank-at-eol (space, tab or CR before the newline), space-before-tab in the
    indent, blank-at-eof. Returns [(line, kind)]."""
    out = []
    lines = data.split(b"\n")
    for n, l in enumerate(lines[:-1] if data.endswith(b"\n") else lines, 1):
        if l[-1:] in (b" ", b"\t", b"\r"):
            out.append((n, "trailing whitespace"))
        indent = l[:len(l) - len(l.lstrip(b" \t"))]
        if b" \t" in indent:
            out.append((n, "space before tab in indent"))
    body = lines[:-1] if data.endswith(b"\n") else lines
    blank = 0
    while blank < len(body) and not body[len(body) - 1 - blank].strip():
        blank += 1
    if data.endswith(b"\n") and blank:
        out.append((len(body) - blank + 1, "blank line at end of file"))
    return out


def authored(rel):
    return rel == "INDEX-PUBLICATION-RECOVERY.md" or (rel.startswith(P4D + "/") and rel not in RAW_P4)


def process_calls(path):
    """Every process launch in a tool, as (line, program) — the literal argv[0], 'python' for sys.executable, or
    UNKNOWN. A call is subprocess.{run,Popen,call,check_call,check_output} or os.{system,popen,exec*,spawn*}."""
    src = open(path).read()
    tree = ast.parse(src)
    out = []
    lists = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.Assign) and isinstance(n.value, ast.List):
            for t in n.targets:
                if isinstance(t, ast.Name):
                    lists.setdefault(t.id, []).append(n.value)

    def head(node):
        if isinstance(node, ast.Name) and node.id in lists:
            hs = {head(v) for v in lists[node.id]}
            return hs.pop() if len(hs) == 1 else "UNKNOWN"
        if isinstance(node, ast.List) and node.elts:
            e = node.elts[0]
            if isinstance(e, ast.Constant) and isinstance(e.value, str):
                return e.value
            if isinstance(e, ast.Attribute) and isinstance(e.value, ast.Name) and (e.value.id, e.attr) == ("sys",
                                                                                                           "executable"):
                return "python"
        return "UNKNOWN"
    for n in ast.walk(tree):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name):
            mod, fn = n.func.value.id, n.func.attr
            if (mod == "subprocess" and fn in ("run", "Popen", "call", "check_call", "check_output")) or (
                    mod == "os" and (fn in ("system", "popen") or fn.startswith(("exec", "spawn")))):
                out.append((n.lineno, head(n.args[0]) if n.args and mod == "subprocess" else "UNKNOWN"))
    return out


def main(rehearse=None):
    Q.no_bytecode()
    if rehearse and not rehearse.startswith(Q.P + "/scratch/"):
        raise SystemExit("REFUSED: a rehearsal lives under this mission's scratch/")
    rdir = os.path.join(rehearse, "receipts") if rehearse else os.path.join(Q.P, "receipts")
    dest = os.path.join(rehearse, "packet") if rehearse else DEST
    after = os.path.join(rdir, "21-ROOT-MANIFESTS-AFTER")
    fz_path = os.path.join(rdir, "10-FREEZE.json")
    fz = json.load(open(fz_path))
    rec = open(os.path.join(rdir, "10-FREEZE-RECORD.txt")).read()
    print(f"FINAL PUBLICATION GATE — {Q.MISSION}" + (f" — REHEARSAL in {rehearse}" if rehearse else ""))

    print("[freeze]")
    check("10-FREEZE-RECORD.txt records this freeze record's sha256 (FREEZE: RECORDED)",
          rec.startswith("FREEZE: RECORDED") and f"sha256 {Q.sha256_file(fz_path)}" in rec, Q.sha256_file(fz_path))
    now = {f: Q.sha256_file(os.path.join(Q.P, f)) for f in A.root_files(Q.P)}
    changed = sorted(f for f, h in fz["root_files"].items() if now.get(f) != h)
    gate_own = {"receipts/10-FREEZE.json", "receipts/10-FREEZE-RECORD.txt", "receipts/20-FINAL-GATE.txt"}
    added = sorted(f for f in now if f not in fz["root_files"] and f not in gate_own
                   and not f.startswith("receipts/21-ROOT-MANIFESTS-AFTER/"))
    check("every frozen file of this mission's root is unchanged (tools, documents, receipts, development; both maps, "
          "the ruling record, the builder, the controls and the publication tooling among them)", not changed,
          f"{len(fz['root_files'])} frozen; changed {changed}")
    check("nothing added to this mission's root since the freeze but the freeze record and the gate's own outputs",
          not added, added)
    packet = {os.path.relpath(f, dest): Q.sha256_file(f) for f in B.files_under(dest)}
    check("the staged packet equals the frozen packet exactly (every path and sha256; nothing added or removed)",
          packet == fz["packet_files"], f"{len(packet)} files; frozen {len(fz['packet_files'])}")
    check("frozen maps unchanged", (Q.sha256_file(B.MAP), Q.sha256_file(B.ZERO_MAP)) == (fz["maps"]["nonempty"],
                                                                                       fz["maps"]["zero_length"]))

    print("[no engineering execution]")
    tools = sorted(os.path.join(Q.P, "tools", f) for f in os.listdir(os.path.join(Q.P, "tools")) if f.endswith(".py"))
    ran = tools + [os.path.join(Q.SRC, "tools/render.py"), os.path.join(Q.SRC, "tools/common.py")]
    inventory = {os.path.relpath(p, Q.AUDITS): process_calls(p) for p in ran}
    for p, calls in inventory.items():
        print(f"     {p}: {sorted({c for _, c in calls}) or 'no process launch'}")
    bad = {p: c for p, c in inventory.items() if [x for x in c if x[1] not in ALLOWED_PROGRAMS | {"python"}]}
    check("every process any tool of this mission launches (and the CONTINUE-02 render.py and common.py the builder "
          "runs and imports) is git, gh, ps (this gate's process listing), or this Python interpreter running this "
          "mission's own tools: no Cargo, rustc, test, explorer or other executable", not bad, bad)
    py = [(p, l) for p in ran for l, c in process_calls(p) if c == "python"]
    print(f"     interpreter launches: {py}")
    ps = subprocess.run(["ps", "-eo", "pid,comm,args"], capture_output=True, text=True).stdout.splitlines()
    busy = [l.strip()[:120] for l in ps[1:] if re.search(r"\b(cargo|rustc)\b|nexus_governed_control", l)
            and "ps -eo" not in l]
    check("no cargo, rustc or governed-control test process is running now", not busy, busy[:3])
    log = open(os.path.join(Q.P, "development/DEVELOPMENT-LOG.txt")).read()
    check("the development log records that nothing was built, compiled, tested, explored, mutated or executed",
          "nothing is built, compiled, tested, explored, mutated or executed" in log)

    print("[read-only roots]")
    os.makedirs(after, exist_ok=False)
    for name in ("continue-02", "publish-03"):
        tsv = os.path.join(after, name + ".tsv")
        rows = M.rows(Q.ROOTS[name])
        with open(tsv, "w") as f:
            f.write("\t".join(M.HEADER) + "\n")
            for r in rows:
                f.write("\t".join(r) + "\n")
        before = os.path.join(BEFORE, name + ".tsv")
        b, a = M.load(before), M.load(tsv)
        check(f"the {name} root after packet construction equals its before-manifest byte for byte (every entry: path, "
              f"type, size, sha256, mode, mtime_ns, symlink target; nothing added, nothing removed)",
              Q.sha256_file(tsv) == Q.sha256_file(before) and a == b,
              f"{len(a)} entries; sha256 {Q.sha256_file(tsv)}")
    check("the CONTINUE-02 root still equals PUBLISH-03's source-evidence manifest",
          Q.sha256_file(os.path.join(after, "continue-02.tsv")) == Q.P3_SRC_MANIFEST_SHA)

    print("[maps, policy and controls]")
    nrows = open(B.MAP).read().splitlines()[1:]
    zrows = open(B.ZERO_MAP).read().splitlines()[1:]
    cls = {}
    for l in nrows:
        cls[l.split("\t")[0]] = cls.get(l.split("\t")[0], 0) + 1
    check("the nonempty map holds exactly 21 rows (7 LINK_RECEIPT, 10 CGROUP_CONSTANT, 4 SELFTEST_CONSTANT)",
          len(nrows) == 21 and cls == {"LINK_RECEIPT": 7, "CGROUP_CONSTANT": 10, "SELFTEST_CONSTANT": 4}, cls)
    check("the zero-length map holds exactly 8 rows (2 self-test outputs, 6 mutant-build stdouts)",
          len(zrows) == 8 and sum(l.split("\t")[0].endswith("/03-build/build.stdout") for l in zrows) == 6)
    for name, src in (("NONEMPTY-DUPLICATE-MAP.tsv", B.MAP), ("ZERO-LENGTH-ARTIFACT-MAP.tsv", B.ZERO_MAP),
                      ("ARCHITECT-RULING-DUPLICATE-POLICY-R2.md", os.path.join(Q.P, "documents",
                                                                              "ARCHITECT-RULING-DUPLICATE-POLICY-R2.md"))):
        check(f"the packet's {name} is byte-equal to this mission's", packet.get(f"{P4D}/documents/{name}") ==
              Q.sha256_file(src))
    allf = sorted(packet)
    acc, accz, problems = B.dup_policy(dest, allf)
    check("exactly 21 mapped nonempty duplicates, each verified; no unmapped duplicate", len(acc) == 21 and not [
        p for p in problems if p[0] in ("UNAUTHORIZED_DUPLICATE", "DUPLICATE_COUNT")], f"{len(acc)} accepted")
    check("exactly 8 mapped zero-length artifacts, each verified; no unlisted empty file", len(accz) == 8 and not [
        p for p in problems if p[0].startswith(("ZERO_LENGTH_", "ZERO_MAPPED"))] and sum(
        1 for f in allf if os.path.getsize(os.path.join(dest, f)) == 0) == 8, f"{len(accz)} accepted")
    check("the R2 policy reports no problem over the whole packet", not problems, problems[:10])
    ctl = json.load(open(os.path.join(Q.P, "receipts/04-REPAIR-CONTROLS/CONTROLS.json")))
    mission = [c for c in ctl if c["as_expected"] is not None]
    ids = {c["control"] for c in mission}
    need = [f"P{n}" for n in range(1, 5)] + [f"N{n}" for n in range(1, 27)]
    missing = [n for n in need if not [i for i in ids if re.fullmatch(re.escape(n) + r"[a-z]?", i)]]
    check("every required repair control (P1-P4, N1-N26) ran, and every control is as expected",
          not missing and all(c["as_expected"] for c in mission), f"{len(mission)} controls; missing {missing}")
    con = open(os.path.join(Q.P, "receipts/04-REPAIR-CONTROLS.console.txt")).read()
    check("the controls ran the frozen builder and the frozen maps",
          f"builder {os.path.join(Q.P, 'tools/build_packet.py')} sha256 {fz['root_files']['tools/build_packet.py']}" in con
          and f"sha256 {fz['maps']['nonempty']}" in con and f"sha256 {fz['maps']['zero_length']}" in con
          and con.rstrip().endswith("REPAIR CONTROLS: ALL AS EXPECTED\nexit 0"))
    bd = open(os.path.join(Q.P, "receipts/03-BUILDER-DIFF.console.txt")).read()
    check("builder diff: ONLY THE DUPLICATE-CONTENT AND ZERO-LENGTH POLICY CHANGED (frozen builder)",
          "BUILDER DIFF: ONLY THE DUPLICATE-CONTENT AND ZERO-LENGTH POLICY CHANGED" in bd
          and f"sha256 {fz['root_files']['tools/build_packet.py']}" in bd)

    print("[preserved failures and candidate evidence]")
    preserved = [("CONTINUE-02", Q.SRC, r, r) for r in (
        "receipts/90-final-gate-attempt-1-CRASHED.txt", "receipts/90-final-gate.txt", "tools/build_packet.py",
        "receipts/80-STOP-RECORD.md", "development/DEVELOPMENT-LOG.txt", "receipts/05-FREEZE.json",
        "receipts/06-PRE-EXECUTION-GATE.txt")]
    preserved.append(("CONTINUE-02", Q.SRC, "publication/10-build-packet.txt", "publication/10-build-packet.txt"))
    preserved += [("PUBLISH-03", Q.P3, r, f"{A.P3_DIR}/{r}") for r in (
        "tools/build_packet.py", "receipts/03-BUILDER-DIFF/BUILDER.diff", "receipts/04-REPAIR-CONTROLS/CONTROLS.json",
        "receipts/04-REPAIR-CONTROLS/P3-builder-run.txt", "receipts/04-REPAIR-CONTROLS.console.txt",
        "receipts/07-DUPLICATE-INVENTORY.tsv", "receipts/07-DUPLICATE-INVENTORY.txt", "receipts/80-STOP-RECORD.md",
        "development/DEVELOPMENT-LOG.txt", "receipts/00-SOURCE-MANIFEST-BEFORE.tsv",
        "receipts/05-SOURCE-MANIFEST-AFTER.tsv", "receipts/90-ROOT-SHA256SUMS.txt",
        "documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md", "documents/DUPLICATE-CONTENT-MAP.tsv")]
    missing = [f"{who} {rel}" for who, root, src, rel in preserved
               if packet.get(rel) != Q.sha256_file(os.path.join(root, src))]
    check("both publication failures and every stop record are present, byte-equal to their roots (CONTINUE-02: the "
          "crashed and the re-run final gate, the frozen builder, its duplicate failure, the stop record, the "
          "development log; PUBLISH-03: the repaired builder and diff, the controls with the failed P3, the inventory, "
          "the stop record, the development log, the manifests, the root sums, the R1 record and map)", not missing,
          f"{len(preserved)} records; missing or different {missing}")
    exp = json.load(open(os.path.join(Q.SRC, "receipts/02-EXPECTATIONS.json")))
    for cid in Q.CANDIDATES:
        d = exp["controls"][cid]["dir"]
        files = [os.path.relpath(os.path.join(a, f), Q.SRC) for a, _, fs in os.walk(os.path.join(Q.SRC, d)) for f in fs]
        files.append(d + ".console.txt")
        miss = [f for f in files if packet.get(f) != Q.sha256_file(os.path.join(Q.SRC, f))]
        first = open(os.path.join(dest, d, "RESULT.txt")).read().splitlines()[0]
        check(f"{cid}: complete evidence ({d}/, its console) present byte-equal; its result still "
              f"{Q.CANDIDATE_RESULT!r}", not miss and first == Q.CANDIDATE_RESULT, f"{len(files)} files; missing {miss}")

    print("[packet contents]")
    pairs = [(s, r) for s, r in A.sources(post=True) if r in packet]
    for label in A.full_checks(dest, pairs):
        fails.append("full check: " + label)
    post = sum(os.path.getsize(os.path.join(Q.P, f)) for f in A.POST_GATE if os.path.exists(os.path.join(Q.P, f)))
    total = sum(os.path.getsize(os.path.join(dest, f)) for f in packet)
    check("the packet with the post-gate records stays at most 32 MiB (the gate's own console counted at 1 MiB)",
          total + post + (1 << 20) <= Q.LIMIT, f"{total} + {post} + 1048576 bytes")

    print("[confinement and repository]")
    status = Q.git("status", "--porcelain", "-uall", repo=Q.WORKTREE).decode().splitlines()
    if rehearse:
        check("REHEARSAL: the packet is in scratch; the real worktree holds nothing yet", status == [], status[:3])
    else:
        check("packet confinement: the worktree holds only untracked files under the packet path, exactly the packet",
              sorted(l[3:] for l in status) == sorted(f"{Q.PACKET}/{f}" for f in packet)
              and all(l.startswith("?? ") for l in status), f"{len(status)} entries")
    check("the worktree is at the parent, on the publication branch, with nothing staged", (
        Q.git("rev-parse", "HEAD", repo=Q.WORKTREE).decode().strip(),
        Q.git("symbolic-ref", "HEAD", repo=Q.WORKTREE).decode().strip(),
        Q.git("diff", "--cached", "--name-only", repo=Q.WORKTREE).decode()) == (Q.PARENT, "refs/heads/" + Q.LOCAL_BRANCH,
                                                                               ""))
    ids = fz["identities"]
    check("Candidate C, the catalog and the previous commits unchanged (trees, catalog blob)", (
        Q.git("rev-parse", Q.C + "^{tree}").decode().strip(),
        Q.git("rev-parse", f"{Q.R2}:{Q.CATALOG_PATH}").decode().strip(),
        Q.git("rev-parse", Q.EXEC01 + "^{tree}").decode().strip(),
        Q.git("rev-parse", Q.RESUME01 + "^{tree}").decode().strip()) == (
        Q.C_TREE, Q.CATALOG_BLOB, Q.EXEC01_TREE, Q.RESUME01_TREE) == (
        ids["C_tree"], ids["catalog_blob"], ids["EXEC01_tree"], ids["RESUME01_tree"]))
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in Q.git("ls-remote", Q.REMOTE).decode().splitlines()}
    check("the R2, EXEC-01 and RESUME-01 review refs unchanged on the remote",
          (rm.get(Q.R2_REF), rm.get(Q.EXEC01_REF), rm.get(Q.RESUME01_REF)) == (Q.R2, Q.EXEC01, Q.RESUME01))
    check("the destination ref is absent on the remote and locally", Q.DEST not in rm and subprocess.run(
        ["git", "-C", Q.REPO, "show-ref", "--verify", "-q", Q.DEST]).returncode == 1)

    print("[whitespace (git's default rules): this mission's files clean; every retained raw finding listed]")
    dirty, retained = [], []
    for f in allf:
        for n, kind in whitespace(open(os.path.join(dest, f), "rb").read()):
            (dirty if authored(f) else retained).append((f, n, kind))
    check("every file this mission authored is whitespace-clean (records, maps, index, tools, controls, manifests, "
          "publication receipts)", not dirty, dirty[:10])
    by = {}
    for f, n, kind in retained:
        by.setdefault(f, []).append((n, kind))
    print(f"     retained raw or preserved findings: {len(retained)} in {len(by)} files (byte-exact; never edited)")
    for f in sorted(by):
        origin = ("this mission's raw tool output, diff or development failure" if f in RAW_P4
                  else "PUBLISH-03 record" if f.startswith(A.P3_DIR + "/") else "CONTINUE-02 evidence")
        print(f"     {f} ({origin}): " + "; ".join(f"line {n} {k}" for n, k in by[f]))
    print("FINAL GATE: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[2]) if sys.argv[1:2] == ["--rehearse"] else None))
