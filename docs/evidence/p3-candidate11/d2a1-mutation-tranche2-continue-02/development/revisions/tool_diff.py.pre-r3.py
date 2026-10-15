#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by -CONTINUE-02 (section 7). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 6). Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02
(section 16): old = the Tranche-2 execution tooling (original-tools/: the Tranche-2 packet's tools/ at 2e06860f and the
tools it carried byte for byte from ebe42009); new = this mission's tools/; the authorization map is mission section
16's. Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

  D1  original-tools/ is exactly the Tranche-2 execution tooling (names and sha256: the Tranche-2 packet's SHA256SUMS
      for its tools/, the RESUME-01 packet's SHA256SUMS at ebe42009 for the tools it carried)
  D2  every tool is UNCHANGED, CHANGED, ADDED or NOT CARRIED exactly as the authorization map declares
  D3  byte-identical: classify.py (the general build classifier classify_build_success and the run classifier
      classify_a: no warning exception of any kind), supervise.py (resource and process checks, supervision),
      worktrees.py, verify_mutant.py, verify_tree.py, scope-wrap.sh, report.py, configs.py and push.py
  D4  core semantics: control.py's git, apply_patch, restored_check, restore, link and finish AST-identical to Tranche
      2's; main is Tranche 2's statement for statement plus one dispatch (cid == 'C5' -> main_c5) after the leading
      refusal; main_c5 is the only added function; the only tool reading warning diagnostics besides the unchanged
      classify.py is c5.py, whose admit refuses every control but C5 and is called only by main_c5, which main calls
      only for C5; expectations.py's helpers AST-identical; authenticate.py's helpers AST-identical
  D5  common.py: every name the reused tools import keeps its accepted value (identities, build and run policy,
      limits) and its helper functions but control_wt are AST-identical; changed and added names are only the declared
      mission identity, namespace, order, accepted sets, Tranche-2 and C5 identities, records and publication names
  D6  no changed or added tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build is
      called only by control.main and supervise.invoke only by control.main and control.main_c5
"""
import ast
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
ORD = "item: order/gate changes beginning at C5"
ADM = "item: dedicated C5 build-admission checker"
PIN = "item: dedicated C5 existing-executable pin"
MAP = "item: restored-side mappings"
REN = "item: rendering"
PUB = "item: packet/publication paths"
FRZ = "item: freeze and gate bindings (section 16: freeze the previous packet, the C5 evidence, executable and warnings)"
REC = "item: Architect ruling record"
CHANGED = {
    "common.py": [f"{ORD}, {REC}, {PUB}: mission identity, evidence root, worktree namespace (mt2c; C5's the preserved "
                  "mt2x one), order C5 A3 A6 C8 C15, the 17 accepted controls, the five-control matrix, the Tranche-2 "
                  "identities and accepted evidence, the C5 build and executable identities, the ruling and campaign "
                  "records, publication names and parent"],
    "control.py": [f"{ORD}: gate() over this order (C5's prior steps: the frozen ruling records, the campaign record and "
                   "the pre-execution gate; the preserved C5 worktree must exist); main()'s refusal text and the C5 "
                   "dispatch",
                   f"{ADM}, {PIN}: main_c5(): admission (c5.admit), the preserved worktree and executable (c5.worktree_"
                   "check, c5.pin), one run (supervise.invoke, unchanged), classify_a (unchanged; source EXACTLY_C), the "
                   "reauthentication, then link() (unchanged)"],
    "expectations.py": [f"{ORD}: the five controls in this order (receipt directories 61-65); {ADM}, {PIN}: C5's "
                        "preserved build and executable identities"],
    "authenticate.py": [f"{FRZ}: every input of this mission, the Tranche-2 stopped evidence and C5's preserved receipts "
                        "(section [8])"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before C5"],
    "final_gate.py": [f"{FRZ}: the final gate (C5's own receipt chain and preserved artifacts)"],
    "render.py": [f"{REN}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md and the derived status (C5's rows)"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent; no C5 build records; the preserved C5 executable "
                        "among the pinned"],
    "publish_checks.py": [f"{PUB}: parent, the Tranche-2 packet unchanged, refs, C5's key receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{REN}, {PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission"],
}
ADDED = {
    "c5.py": [f"{ADM} (section 6: exactly the five permitted warning groups, C5 only), {PIN} (section 7) and the "
              "preserved worktree's read-only authentication (sections 7 and 15)"],
    "c2_controls.py": ["section 16 (tooling controls on synthetic or historical evidence only): classifier, link, "
                       "expectation, C5 admission and pin controls, including every required negative control"],
}
NOT_CARRIED = {"t2_controls.py": "replaced by c2_controls.py (this mission's controls)"}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "link", "finish"],
              "expectations.py": ["fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section5"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_KEPT = ["REPO", "GIT_COMMON_DIR", "NEXUS", "C", "C_TREE", "C_PARENT", "CRATE", "CRATE_SRC", "PROTOCOL", "MODEL",
               "UNIT", "BLOBS", "R2", "R2_TREE", "R2_PARENT", "R2_REF", "R2_DIR", "CATALOG_PATH", "CATALOG_BLOB",
               "CATALOG_SHA", "CATALOG_VERSION", "DEFS_PATH", "DEFS_BLOB", "DEFS_SHA", "CONTRACT_COMMIT", "CONTRACT_PATH",
               "CONTRACT_SHA", "AD1R4", "AD1R4_SHA", "R5_COMMIT", "AD1R5", "AD1R5_SHA", "STAGE1", "STAGE1_PACKET",
               "STAGE2", "STAGE2_PACKET", "BASE_WT", "EXE", "EXE_SHA256", "EXE_SIZE", "BUILD_ARGV", "BUILD_LIMIT",
               "RUN_ENV", "MEMORY_MAX", "NEED_KIB", "LISTED", "REMOTE", "SLUG", "TEST_PREFIX", "UNIT_PREFIX_TEST",
               "NOT_AUTHORIZED", "KILL_REQUIRED", "A6_KEY_TEXT", "T1", "T1_TREE", "T1_PARENT", "T1_REF", "T1_PACKETS",
               "T1_RECEIPTS", "PACKET_LIMIT"]
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "ACCEPTED", "MATRIX", "UNIT_PREFIX", "RULING", "RULING_FIRST",
                  "CAMPAIGN", "CAMPAIGN_FIRST", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"C5_WT", "ACCEPTED_T1", "ACCEPTED_T2", "T2", "T2_TREE", "T2_PARENT", "T2_REF", "T2_PACKET",
                "T2_RECEIPTS", "T2_ACCEPTED_EVIDENCE", "C5_BUILD_DIR", "C5_PATCH_SHA256", "C5_MUTANT_BLOB",
                "C5_MUTANT_SHA256", "C5_EXE", "C5_EXE_SHA256", "C5_EXE_SIZE", "C5_STDERR_SHA256", "C5_LOCK_REASON",
                "C5_RULING", "C5_RULING_FIRST", "PARENT", "PARENT_TREE"}
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
    t2sums = {}
    for line in K.git("show", f"{K.T2}:{K.T2_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t2sums[rel[6:]] = h
    r1sums = {}
    for line in K.git("show", f"{K.T1}:{K.RESUME01_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            r1sums[rel[6:]] = h
    t2cf = K.git("show", f"{K.T2}:{K.T2_PACKET}/CARRIED-FORWARD.md").decode().split("## The old versions")[0]
    carried = {t: r1sums[t] for t in r1sums if f"| `{t}` | `{K.RESUME01_PACKET}/tools/{t}` |" in t2cf}
    base = dict(carried, **t2sums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the Tranche-2 execution tooling: its packet's tools/ "
          f"({len(t2sums)}; SHA256SUMS at {K.T2[:8]}) and the tools it carried byte for byte ({len(carried)}; RESUME-01 "
          f"SHA256SUMS at {K.T1[:8]})", old == base, sorted(set(old) ^ set(base)))
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
    # D4: control.py.
    to, tn = tree(os.path.join(OLD, "control.py")), tree(os.path.join(NEW, "control.py"))
    fo, fnw = funcs(to), funcs(tn)
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main_c5, main",
          [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main_c5", "main"])
    mo, mn = fn(to, "main"), fn(tn, "main")
    check("D4 control.main: Tranche 2's, statement for statement, plus exactly one statement after the leading refusal: "
          "the C5 dispatch",
          [ast.dump(x) for x in mo.body[1:]] == [ast.dump(x) for x in mn.body[2:]]
          and ast.unparse(mn.body[1]) == "if cid == 'C5':\n    return main_c5()"
          and "K.ACCEPTED" in ast.unparse(mn.body[0]) and "K.ORDER" in ast.unparse(mn.body[0])
          and ast.dump(mo.body[0]) == ast.dump(mn.body[0]))
    mod_o = [ast.dump(n) for n in to.body if not isinstance(n, ast.FunctionDef)]
    mod_n = [ast.dump(n) for n in tn.body if not isinstance(n, ast.FunctionDef)]
    added_mod = [ast.unparse(n) for n in tn.body if not isinstance(n, ast.FunctionDef) and ast.dump(n) not in mod_o]
    check("D4 control.py module level: Tranche 2's statements kept, plus only the docstring revision, `import c5` and "
          "the two new result strings", all(x in mod_n for x in mod_o[1:]) and len(added_mod) == 4
          and "import c5" in added_mod and any(a.startswith("RESULT_ARTIFACT_BLOCKED = ") for a in added_mod)
          and any(a.startswith("REAUTH_OK = ") for a in added_mod), added_mod[1:])
    c5t = tree(os.path.join(NEW, "c5.py"))
    admit = fn(c5t, "admit")
    first_ck = min((n for n in ast.walk(admit) if isinstance(n, ast.Call) and ast.unparse(n.func) == "ck"),
                   key=lambda n: (n.lineno, n.col_offset))  # the first check in source order
    callers = [f for f in new if f.endswith(".py") and f not in ("c5.py", "c2_controls.py")
               and any(c == "c5.admit" for c, _ in calls(tree(os.path.join(NEW, f))))]
    in_main_c5 = [c for c, _ in calls(fn(tn, "main_c5"))]
    warning_readers = [f for f in sorted(CHANGED) + sorted(ADDED) if f.endswith(".py")
                       and "warning" in open(os.path.join(NEW, f)).read() and f not in ("c5.py", "c2_controls.py")]
    check("D4 the only warning-admitting checker is c5.admit: its first check requires the control to be C5 (with the "
          "ruling's patch and mutant), it is called only by control.main_c5 (and the pre-freeze controls), and main "
          "calls main_c5 only for C5",
          "ctl.get('id') == 'C5'" in ast.unparse(first_ck) and callers == ["control.py"] and "c5.admit" in in_main_c5
          and not [c for c, _ in calls(fn(tn, "main")) if c == "c5.admit"],
          f"first check {ast.unparse(first_ck)[:80]}; callers {callers}")
    check("D4 no other changed or added tool handles warning diagnostics (the word appears only in their text and "
          "records, never in a classifier)", all(
              not [n for n in ast.walk(tree(os.path.join(NEW, f))) if isinstance(n, ast.FunctionDef)
                   and "warning" in n.name.lower()] for f in warning_readers), warning_readers)
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
    check("D5 common.py: names added only from the declared set (Tranche-2, C5, records, parent)",
          set(added) <= COMMON_ADDED, sorted(set(added) - COMMON_ADDED))
    removed = sorted(k for k in co if k not in cn)
    check("D5 common.py: no name removed", not removed, removed)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mt2c); C5's worktree is the "
          "preserved Tranche-2 one", cn["UNIT_PREFIX"] == "p3c11-mt2c" and K.control_wt("A3").endswith(
              "/nexus-os-p3-c11-mt2c-A3") and K.control_wt("C5") == K.NEXUS + "/nexus-os-p3-c11-mt2x-C5")
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
            ok_calls = (all(c in [x for x, _ in calls(fn(t, "main"))] for c in builds if c == "supervise.build")
                        and [c for c, _ in calls(fn(t, "main_c5")) if c == "supervise.build"] == []
                        and builds.count("supervise.invoke") == 2)
        else:
            ok_calls = not builds
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build only in main, supervise.invoke in main and main_c5" if f == "control.py"
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
            label = f"TRANCHE2-EXEC-01/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"TRANCHE2-CONTINUE-02/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the Tranche-2 execution tooling (`original-tools/`: `{K.T2_PACKET}/tools/` at `{K.T2}` and the tools it "
          f"carried byte for byte, `{K.RESUME01_PACKET}/tools/` at `{K.T1}`). New: this mission's `tools/`. The complete "
          "old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. Permitted differences "
          "(mission section 16): the Architect ruling record; the dedicated C5 build-admission checker; the dedicated C5 "
          "existing-executable pin; order/gate changes beginning at C5; the A6 corrected KEY_FIELDS assertion; "
          "restored-side mappings; rendering/packet/publication paths.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte"]))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Byte-identical: `classify.py` (the general build classifier, still `BUILD: NOT OK` on the historical C5 "
           "build, and the run classifier, including the A6 KEY_FIELDS assertion Tranche 2 already carried: the printed "
           "key fields must be the mutant's KEY_FIELDS, which name recovery_required), `supervise.py`, `worktrees.py`, "
           "`verify_mutant.py`, `verify_tree.py`, `scope-wrap.sh`, `report.py`, `configs.py`, `push.py`. No generic "
           "warning exception exists: the only warning-admitting check is `c5.admit`, C5 only. A3, A6, C8 and C15 run "
           "Tranche 2's apply, build (zero warnings, zero errors), run, restoration and link steps unchanged.", ""]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
