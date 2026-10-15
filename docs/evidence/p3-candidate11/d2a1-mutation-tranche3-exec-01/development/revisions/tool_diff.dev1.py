#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by -CONTINUE-02, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 and
-TRANCHE2-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (section 11): old = the accepted
continuation's execution tooling (original-tools/: the continuation packet's tools/ at b97205b2 and the tools it carried
byte for byte); new = this mission's tools/; the authorization map is mission section 11's permitted differences
(mission/control order; expectations for these ten controls; C3b's alternative-property rule; C7's informational F-3
execution; L1 complete-search validation; restored-side mappings; rendering/evidence/publication paths; freeze/final-gate
bindings). Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

  D1  original-tools/ is exactly the continuation's execution tooling (names and sha256: its packet's SHA256SUMS for its
      tools/, and the commits and paths its CARRIED-FORWARD.md names for the tools it carried byte for byte)
  D2  every tool is UNCHANGED, CHANGED, ADDED or NOT CARRIED exactly as the authorization map declares; no tool imports a
      tool not carried (c5.py, c2_controls.py)
  D3  byte-identical: classify.py (the general build classifier classify_build_success and the run classifier
      classify_a: no warning exception of any kind; no generic classifier weakening), supervise.py, worktrees.py,
      verify_mutant.py, verify_tree.py, scope-wrap.sh, report.py, configs.py and push.py
  D4  core semantics: control.py's git, apply_patch, restored_check, restore and finish AST-identical to the
      continuation's; main is the continuation's with the C5 dispatch removed and lines only inserted (the t3.rules step
      after classify_a, which can only withdraw candidacy, and C7's informational run, which can only stop); link() is
      the continuation's with lines only inserted (the restoration-receipt check) except the one composed-quotient
      check, whose F-B text names the control's configuration; one more check, none removed; t3.rules never sets a
      candidate (its result is and-ed); t3.classify_info never writes a kill label; the helpers of expectations.py,
      authenticate.py and build_packet.py AST-identical
  D5  common.py: every name the reused tools import keeps its accepted value and its helper functions but control_wt are
      AST-identical; changed and added names are only the declared mission identity, namespace, order, accepted and
      reserved sets, matrix, A4/L1/C7 rules, continuation identities, records and publication names (the 64 MiB limit)
  D6  no changed or added tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build is
      called only by control.main and supervise.invoke only by control.main (the kill run and C7's informational run)
"""
import ast
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
ORD = "item: mission/control order"
EXP = "item: expectations for these ten controls"
ALT = "item: C3b alternative-property rule"
INF = "item: C7 informational F-3 execution"
L1V = "item: L1 complete-search validation"
MAP = "item: restored-side mappings"
REN = "item: rendering/evidence paths"
PUB = "item: publication paths"
FRZ = "item: freeze/final-gate bindings"
CHANGED = {
    "common.py": [f"{ORD}, {PUB}, {FRZ}: mission identity, evidence root, worktree namespace (mt3x), order C2 C24 C3b C3c "
                  "C6 C7 A1 A2 C20 A5 (receipt directories 66-75), the 22 accepted and the 3 reserved controls, the ten-"
                  "control matrix, the A4-ON and L1 control sets, C7's informational run, the continuation's identities, "
                  "the ruling and campaign records, publication names, parent and the 64 MiB limit"],
    "control.py": [f"{ORD}: gate() over this order (C2's prior steps: the frozen ruling and campaign records and the "
                   "pre-execution gate); main()'s C5 dispatch and main_c5 removed (no C5 here)",
                   f"{ALT}, {L1V}: main() inserts the tranche-3 rules (t3.rules) after classify_a: they can only withdraw "
                   "candidacy (and-ed)",
                   f"{INF}: main() inserts, for C7 only, after an admissible F-PT/P6 candidate and before restoration, one "
                   "informational run without A4 on F-3 (supervise.invoke, unchanged) classified by t3.classify_info; it "
                   "can only stop the mission, never make a candidate",
                   f"{MAP}: link() refuses to link before this control's own restoration receipt records RESTORE: "
                   "EXACTLY_C (one check inserted), and its composed-quotient check names the control's configuration "
                   "(F-PT for C6/C7) instead of F-B (the same check otherwise)"],
    "expectations.py": [f"{EXP}: the ten controls in this order; C6/C7 on full_fpt (original_cfg); C7's informational "
                        f"full_f3; the L1 anchors and rule; {ALT}: C3b's two properties; {MAP}: mission section 9"],
    "authenticate.py": [f"{FRZ}: every input of this mission; section [9]: the accepted continuation evidence and its five "
                        "accepted controls; the base tooling check moved to [9]"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before C2 (E5-E11 for this mission)"],
    "final_gate.py": [f"{FRZ}: the final gate (this mission's receipt chain, rules, C7's informational run, worktrees)"],
    "render.py": [f"{REN}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md and the derived status"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent; every control's build; C7's informational sampler "
                        "record; the 64 MiB limit"],
    "publish_checks.py": [f"{PUB}: parent, every previous packet unchanged, refs, the key receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent, the 64 MiB limit and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{REN}, {PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission"],
}
ADDED = {
    "t3.py": [f"{ALT}, {L1V}, {INF}: the tranche-3 rules (adding requirements to classify_a only) and C7's "
              "informational classifier (never a kill label)"],
    "t3_controls.py": ["section 11 (tooling controls on synthetic or historical evidence only): every required negative "
                       "control"],
}
NOT_CARRIED = {"c5.py": "C5-only (the continuation's narrow C5 build ruling); no C5 step in this mission",
               "c2_controls.py": "replaced by t3_controls.py (this mission's controls)"}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "finish"],
              "expectations.py": ["fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section5", "section7"],
              "build_packet.py": ["files_under", "whitespace", "authored", "prior_blobs", "selftest_block", "dup_policy"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_KEPT = ["REPO", "GIT_COMMON_DIR", "NEXUS", "C", "C_TREE", "C_PARENT", "CRATE", "CRATE_SRC", "PROTOCOL", "MODEL",
               "UNIT", "BLOBS", "R2", "R2_TREE", "R2_PARENT", "R2_REF", "R2_DIR", "CATALOG_PATH", "CATALOG_BLOB",
               "CATALOG_SHA", "CATALOG_VERSION", "DEFS_PATH", "DEFS_BLOB", "DEFS_SHA", "CONTRACT_COMMIT", "CONTRACT_PATH",
               "CONTRACT_SHA", "AD1R4", "AD1R4_SHA", "R5_COMMIT", "AD1R5", "AD1R5_SHA", "STAGE1", "STAGE1_PACKET",
               "STAGE2", "STAGE2_PACKET", "BASE_WT", "EXE", "EXE_SHA256", "EXE_SIZE", "BUILD_ARGV", "BUILD_LIMIT",
               "RUN_ENV", "MEMORY_MAX", "NEED_KIB", "LISTED", "REMOTE", "SLUG", "TEST_PREFIX", "UNIT_PREFIX_TEST",
               "KILL_REQUIRED", "A6_KEY_TEXT", "T1", "T1_TREE", "T1_PARENT", "T1_REF", "T1_PACKETS", "T1_RECEIPTS",
               "ACCEPTED_T1", "ACCEPTED_T2", "T2", "T2_TREE", "T2_PARENT", "T2_REF", "T2_PACKET", "T2_RECEIPTS",
               "T2_ACCEPTED_EVIDENCE", "C5_WT", "C5_BUILD_DIR", "C5_PATCH_SHA256", "C5_MUTANT_BLOB", "C5_MUTANT_SHA256",
               "C5_EXE", "C5_EXE_SHA256", "C5_EXE_SIZE", "C5_STDERR_SHA256", "C5_LOCK_REASON", "C5_RULING",
               "C5_RULING_FIRST"]
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "ACCEPTED", "NOT_AUTHORIZED", "MATRIX", "UNIT_PREFIX",
                  "RULING", "RULING_FIRST", "CAMPAIGN", "CAMPAIGN_FIRST", "PARENT", "PARENT_TREE", "PACKET", "BRANCH",
                  "DEST", "LOCAL_BRANCH", "PACKET_LIMIT"}
COMMON_ADDED = {"FIRST_DIR", "ACCEPTED_T2C", "A4_ON", "L1_CONTROLS", "C7_INFO", "T2C", "T2C_TREE", "T2C_PARENT",
                "T2C_REF", "T2C_PACKET", "T2C_RECEIPTS", "T2C_ACCEPTED_EVIDENCE", "ACCEPTED_WT", "RULING_T2C"}
results = []


def check(label, ok, detail=""):
    results.append((label, bool(ok), str(detail)))


def tree(path):
    return ast.parse(open(path).read())


def funcs(t):
    return {n.name: ast.dump(n) for n in t.body if isinstance(n, ast.FunctionDef)}


def fn(t, name):
    return next(n for n in t.body if isinstance(n, ast.FunctionDef) and n.name == name)


def namespace(path):
    ns = {}
    exec(compile(open(path).read(), path, "exec"), ns)
    return {k: v for k, v in ns.items() if not k.startswith("__") and not callable(v) and k not in ("hashlib", "os",
                                                                                                    "subprocess")}


def calls(node):
    return [(ast.unparse(n.func), n) for n in ast.walk(node) if isinstance(n, ast.Call)]


def launches(path):
    """Every process launch, as the literal argv[0], 'python' (sys.executable) or UNKNOWN; an argument list held in a
    name is resolved through that name's list assignments in the module (all of them must agree)."""
    t = tree(path)
    out = []
    lists = {}
    for n in ast.walk(t):
        if isinstance(n, ast.Assign) and isinstance(n.value, ast.List):
            for tg in n.targets:
                if isinstance(tg, ast.Name):
                    lists.setdefault(tg.id, []).append(n.value)

    def head_of(a):
        if isinstance(a, ast.Name) and a.id in lists:
            hs = {head_of(v) for v in lists[a.id]}
            return hs.pop() if len(hs) == 1 else "UNKNOWN"
        if isinstance(a, ast.List) and a.elts:
            e = a.elts[0]
            if isinstance(e, ast.Constant) and isinstance(e.value, str):
                return e.value
            if ast.unparse(e) == "sys.executable":
                return "python"
        return "UNKNOWN"
    for n in ast.walk(t):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name):
            mod, f = n.func.value.id, n.func.attr
            if (mod == "subprocess" and f in ("run", "Popen", "call", "check_call", "check_output")) or (
                    mod == "os" and (f in ("system", "popen") or f.startswith(("exec", "spawn")))):
                out.append((n.lineno, head_of(n.args[0]) if n.args else "UNKNOWN"))
    return out


def run():
    t2csums = {}
    for line in K.git("show", f"{K.T2C}:{K.T2C_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t2csums[rel[6:]] = h
    carried = {}
    for line in K.git("show", f"{K.T2C}:{K.T2C_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions")[0].splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = K.sha256_bytes(K.git("show", f"{cells[2]}:{cells[1]}"))
            check(f"D1 carried tool {cells[0]}: its sha256 at {cells[2][:8]}:{cells[1]} is the one the continuation's "
                  "CARRIED-FORWARD.md records", carried[cells[0]] == cells[4], cells[4][:16])
    base = dict(carried, **t2csums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the continuation's execution tooling: its packet's tools/ "
          f"({len(t2csums)}; SHA256SUMS at {K.T2C[:8]}) and the tools it carried byte for byte ({len(carried)})",
          old == base, sorted(set(old) ^ set(base)))
    status = {f: ("UNCHANGED" if f in new and new[f] == old[f] else "CHANGED" if f in new else "NOT CARRIED")
              for f in old}
    status.update({f: "ADDED" for f in new if f not in old})
    for kind, decl in (("CHANGED", CHANGED), ("ADDED", ADDED), ("NOT CARRIED", NOT_CARRIED)):
        got = sorted(f for f, s in status.items() if s == kind)
        check(f"D2 the {kind.lower()} tools are exactly the declared ones", got == sorted(decl), got)
    check("D2 the unchanged tools are exactly the byte-for-byte carried ones",
          sorted(f for f, s in status.items() if s == "UNCHANGED") == sorted(CARRIED_BYTE_FOR_BYTE),
          sorted(f for f, s in status.items() if s == "UNCHANGED"))
    imports = [(f, a.name) for f in new if f.endswith(".py") for m in ast.walk(tree(os.path.join(NEW, f)))
               if isinstance(m, (ast.Import, ast.ImportFrom)) for a in m.names if a.name + ".py" in NOT_CARRIED]
    check("D2 no tool imports a tool not carried", not imports, imports)
    for f in CARRIED_BYTE_FOR_BYTE:
        check(f"D3 {f} byte-identical (sha256 {old.get(f, '')[:16]}…)", status.get(f) == "UNCHANGED", status.get(f))
    check("D3 scope-wrap.sh keeps its executable mode", os.access(os.path.join(NEW, "scope-wrap.sh"), os.X_OK))
    check("D3 the general build classifier (classify.classify_build_success) is byte-identical: its check 'no error or "
          "warning diagnostic' still rejects every warning (no generic warning exception)",
          status.get("classify.py") == "UNCHANGED" and "ck(\"no error or warning diagnostic\", not errs, errs[:3])" in
          open(os.path.join(NEW, "classify.py")).read())
    check("D3 no changed or added tool imports c5 or defines a warning-handling function (no warning exception)",
          not [f for f in sorted(CHANGED) + sorted(ADDED) if f.endswith(".py") and (
              any(isinstance(m, (ast.Import, ast.ImportFrom)) and any(a.name == "c5" for a in m.names)
                  for m in ast.walk(tree(os.path.join(NEW, f))))
              or any(isinstance(n, ast.FunctionDef) and "warning" in n.name.lower()
                     for n in ast.walk(tree(os.path.join(NEW, f)))))])
    # D4: control.py.
    to, tn = tree(os.path.join(OLD, "control.py")), tree(os.path.join(NEW, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main",
          [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main"])

    def src_lines(path, node):
        return open(path).read().splitlines()[node.lineno - 1:node.end_lineno]

    def opcodes(a, b):
        return [op for op in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes() if op[0] != "equal"]
    mo, mn = fn(to, "main"), fn(tn, "main")
    a, b = src_lines(os.path.join(OLD, "control.py"), mo), src_lines(os.path.join(NEW, "control.py"), mn)
    ops = opcodes(a, b)
    deleted = [a[op[1]:op[2]] for op in ops if op[0] == "delete"]
    others = [op for op in ops if op[0] not in ("insert", "delete")]
    inserted = "\n".join(l for op in ops if op[0] == "insert" for l in b[op[3]:op[4]])
    check("D4 control.main: the continuation's, with only the C5 dispatch deleted and lines only inserted (the "
          "tranche-3 rules after classify_a; C7's informational run before restoration)",
          deleted == [['    if cid == "C5":', "        return main_c5()"]] and not others
          and "rules_ok = t3.rules(" in inserted and "candidate = candidate and rules_ok" in inserted
          and "t3.classify_info(" in inserted and "if candidate and ctl.get(\"informational\"):" in inserted
          and "candidate = True" not in inserted and "candidate = " + "rules_ok" not in inserted,
          f"{len(ops)} hunks; deleted {deleted}; other {others}")
    check("D4 the informational run can only stop: within it, no assignment to candidate; its classifier never writes "
          "the kill label", "candidate =" not in inserted.split("if candidate and ctl.get(\"informational\"):")[1]
          and "CANDIDATE" not in open(os.path.join(NEW, "t3.py")).read().split("def classify_info")[1].replace(
              "classify.CANDIDATE not in", ""), "")
    t3t = tree(os.path.join(NEW, "t3.py"))
    check("D4 t3.rules returns its own verdict and writes only its own records (RULES.txt, RULES.json); it never calls "
          "classify_a or writes CLASSIFICATION.txt", "classify_a" not in ast.unparse(fn(t3t, "rules"))
          and "CLASSIFICATION.txt\"), \"w\"" not in ast.unparse(fn(t3t, "rules")))
    lo, ln = fn(to, "link"), fn(tn, "link")
    a, b = src_lines(os.path.join(OLD, "control.py"), lo), src_lines(os.path.join(NEW, "control.py"), ln)
    ops = opcodes(a, b)
    ins = "\n".join(l for op in ops if op[0] == "insert" for l in b[op[3]:op[4]])
    repl = [(a[op[1]:op[2]], b[op[3]:op[4]]) for op in ops if op[0] != "insert"]
    old_q = "\n".join(x for o, _ in repl for x in o)
    new_q = "\n".join(x for _, n in repl for x in n)
    nchk = lambda f: len([n for n in ast.walk(f) if isinstance(n, ast.Call) and ast.unparse(n.func) == "chk"])
    check("D4 control.link: the continuation's, with the restoration-receipt check inserted and only the composed-quotient "
          "check's F-B text generalized to the control's configuration; one check more, none removed",
          nchk(ln) == nchk(lo) + 1 and "05-restore/RESTORE.txt" in ins and "RESTORE_OK" in ins
          and "F-B" in old_q and "F-B" not in new_q and "{cfg}" in new_q
          and all(k in new_q for k in ("composed-quotient test with the same flags", "bounds == [ctl['bounds']]",
                                       "gating-caller symmetry quotient composed with the admission"))
          and not [o for o, n in repl if not any("F-B" in x or "full_fb" in x or x.strip().startswith(("chk(", "elif L["))
                                                 or "EXPLORATION" in x or "quotient" in x or "bounds" in x for x in o)],
          f"{len(ops)} hunks; replaced {len(repl)}")
    for f, names in SAME_FUNCS.items():
        fa, fb = funcs(tree(os.path.join(OLD, f))), funcs(tree(os.path.join(NEW, f)))
        diff = [n for n in names if fa.get(n) is None or fa.get(n) != fb.get(n)]
        check(f"D4 {f}: functions {names} AST-identical to Tranche 2's", not diff, diff)
    # D5: common.py.
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    kept = [k for k in COMMON_KEPT if co.get(k, "<absent>") != cn.get(k, "<absent>")]
    check(f"D5 common.py: the {len(COMMON_KEPT)} names the reused tools and this mission read with their accepted values "
          "keep them (identities, catalog, contract, Stage-1/2, baseline executable, build and run policy, limits, "
          "Tranche-1 identities)", not kept, kept)
    changed = sorted(k for k in co if k in cn and co[k] != cn[k])
    check("D5 common.py: values changed only for the declared names", set(changed) <= COMMON_CHANGED, changed)
    added = sorted(k for k in cn if k not in co)
    check("D5 common.py: names added only from the declared set (order, rules, continuation, records)",
          set(added) <= COMMON_ADDED, sorted(set(added) - COMMON_ADDED))
    removed = sorted(k for k in co if k not in cn)
    check("D5 common.py: no name removed", not removed, removed)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mt3x)",
          cn["UNIT_PREFIX"] == "p3c11-mt3x" and all(K.control_wt(c) == K.NEXUS + "/nexus-os-p3-c11-mt3x-" + c
                                                    for c in K.ORDER))
    # D6: no changed or added tool builds or runs anything itself.
    allowed = {"git", "gh", "ps", "python"}
    for f in sorted(CHANGED) + sorted(ADDED):
        if not f.endswith(".py"):
            continue
        t = tree(os.path.join(NEW, f))
        bad = [x for x in launches(os.path.join(NEW, f)) if x[1] not in allowed]
        lists = [e.value for n in ast.walk(t) if isinstance(n, ast.List) for e in n.elts
                 if isinstance(e, ast.Constant) and e.value in ("cargo", "rustc")]
        if f == "common.py":
            lists = [x for x in lists if x != "cargo"] if ast.unparse(next(n.value for n in t.body if isinstance(
                n, ast.Assign) and ast.unparse(n.targets[0]) == "BUILD_ARGV")).count("'cargo'") == 1 else lists
        builds = [c for c, _ in calls(t) if c in ("supervise.build", "supervise.invoke")]
        if f == "control.py":
            in_main = [x for x, _ in calls(fn(t, "main"))]
            ok_calls = (builds.count("supervise.build") == 1 == in_main.count("supervise.build")
                        and builds.count("supervise.invoke") == 2 == in_main.count("supervise.invoke"))
        else:
            ok_calls = not builds
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build once and supervise.invoke twice (kill run, C7 informational run), all in main"
                 if f == "control.py"
                 else "; no supervise.build/invoke"), not bad and not lists and ok_calls, f"{bad} {lists} {builds}")
    return old, new, status


def main(argv):
    old, new, status = run()
    bad = [r for r in results if not r[1]]
    lines = [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + d) if d else ''}" for l, o, d in results]
    lines.append("TOOL DIFF: " + ("ALL CHECKS HOLD" if not bad else "FAILED"))
    if argv[0] == "--check":
        print("\n".join(lines))
        return 0 if not bad else 1
    out = os.path.abspath(argv[0])
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(out)
    diff = []
    for f in sorted(status):
        if status[f] in ("CHANGED", "ADDED"):
            a = open(os.path.join(OLD, f)).read().splitlines(keepends=True) if status[f] == "CHANGED" else []
            label = f"TRANCHE2-CONTINUE-02/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"TRANCHE3-EXEC-01/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the accepted continuation's execution tooling (`original-tools/`: `{K.T2C_PACKET}/tools/` at `{K.T2C}` "
          "and the tools it carried byte for byte, at the commits its CARRIED-FORWARD.md names). New: this mission's "
          "`tools/`. The complete old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. "
          "Permitted differences (mission section 11): mission/control order; expectations for these ten controls; C3b "
          "alternative-property rule; C7 informational F-3 execution; L1 complete-search validation; restored-side "
          "mappings; rendering/evidence/publication paths; freeze/final-gate bindings.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte"]))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Byte-identical: `classify.py` (the general build classifier `classify_build_success`, which rejects every "
           "warning, and the run classifier `classify_a`), `supervise.py`, `worktrees.py`, `verify_mutant.py`, "
           "`verify_tree.py`, `scope-wrap.sh`, `report.py`, `configs.py`, `push.py`. No warning exception exists in this "
           "mission (`c5.py` is not carried). The tranche-3 rules (`t3.rules`) only add requirements to `classify_a`; "
           "C7's informational classifier (`t3.classify_info`) never writes a kill label and can only stop the mission.",
           ""]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
