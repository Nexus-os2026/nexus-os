#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by -CONTINUE-02 (section 7). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (section 6): old = the accepted Tranche-1 execution tooling (original-tools/:
CONTINUE-02's tools/ at ebe42009 and the RESUME-01 tools it ran unchanged) and, for the publication tools, the
PUBLISH-04 publication tooling (original-publication-tools/); new = this mission's tools/; the authorization map is
mission section 6's. Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

  D1  original-tools/ and original-publication-tools/ are exactly the accepted Tranche-1 execution tools and the
      PUBLISH-04 publication tools (names and sha256, from the Tranche-1 evidence commit's packet SHA256SUMS)
  D2  every tool is UNCHANGED, CHANGED, ADDED or NOT CARRIED exactly as the authorization map declares
  D3  the tools the mission carries byte for byte are byte-identical: supervise.py (resource and process checks,
      supervision), worktrees.py, verify_mutant.py (path-aware target handling), verify_tree.py, scope-wrap.sh,
      report.py (branch/report parsing), configs.py
  D4  core semantics: classify.py differs from Tranche 1's only inside classify_a (every other function and every
      module-level statement AST-identical), and only by the catalog-required permitted-property alternatives and the
      A6 KEY_FIELDS check (each gated on an expectation field present only for C3a/A6 or A6), whose inertness for every
      other control is shown byte for byte by receipts/03-t2-controls; control.py's git, apply_patch, restored_check,
      restore and finish are AST-identical and main is Tranche 1's but for its leading refusal (and the C28 dispatch,
      removed); expectations.py's fn_span, attrs_above and verdict_assertion, authenticate.py's helpers AST-identical
  D5  common.py: every name the reused tools import keeps its accepted value (identities, build and run policy,
      limits) and its helper functions are AST-identical; the changed names are only the declared mission identity,
      namespace, order, mapping and publication names
  D6  no changed or added tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build and
      supervise.invoke are called only by control.main (unchanged)
"""
import ast
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
OLDP = os.path.join(K.EV, "original-publication-tools")
CAT = "item: catalog-derived expectations"
ORD = "item: Tranche-2 control order (gate/order logic)"
MAP = "item: restored-evidence mappings"
REN = "item: ruling and summary rendering"
PUB = "item: packet/publication paths"
FRZ = "item: freeze and final-gate bindings"
REQ = "explicitly required by the frozen catalog"
CHANGED = {
    "classify.py": [f"{REQ}: classify_a only — C3a and A6 permit either named property (catalog named_property lists, "
                    "special_attribution_rules 'AD-1-R4 F.1(a): either property the row names'); A6's printed key fields "
                    "must be the mutant's KEY_FIELDS naming recovery_required (catalog R2 receipt_metadata_item); both "
                    "gated on expectation fields absent for every other control"],
    "common.py": [f"{ORD}, {MAP}, {PUB}: mission identity, evidence root, worktree namespace (mt2x), order, accepted and "
                  "not-authorized sets, the control matrix, the linked Stage-1/Stage-2/R5 identities, the ruling and "
                  "campaign records, publication paths and the 48 MiB packet limit"],
    "control.py": [f"{ORD}: gate() over the Tranche-2 order (C23's prior steps: the frozen ruling and campaign records "
                   "and the pre-execution gate); main()'s leading refusal; the C28-only steps removed",
                   f"{MAP}: link() links the catalog's restored side (Stage-1/Stage-2 e1 receipt; C8's composed-quotient "
                   "F-B PASS; C15's AD-1-R5 F discharge record)"],
    "expectations.py": [f"{CAT}: the ten controls from the catalog, cross-checked against the mission's matrix; the "
                        "mutants' bytes; A3/A6 verdict-assertion location from the mutant; A6 key fields; C8's "
                        "original-explorer F-B; the restored sides"],
    "authenticate.py": [f"{FRZ}: the authentication of every input of this mission (the inputs of the freeze and of "
                        "both gates)"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate of section 7"],
    "final_gate.py": [f"{FRZ}: the final gate"],
    "render.py": [f"{REN}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md and the derived status"],
    "build_packet.py": [f"{PUB}: this packet's contents, references and duplicate/zero-length policy"],
    "publish_checks.py": [f"{PUB}: publication checks for this packet, parent and ref (from PUBLISH-04's)"],
    "tool_diff.py": [f"{FRZ}: this diff and its check (section 6)"],
}
ADDED = {
    "t2_controls.py": ["section 7 (test tooling only on synthetic or historical evidence): classifier, link, expectation "
                       "and apply/restore controls"],
    "rehearsals.py": [f"{REN}, {PUB}: render.py and build_packet.py rehearsed on synthetic evidence roots"],
    "worktree.py": [f"{PUB}: the evidence worktree at the required parent (from PUBLISH-04's worktree.py)"],
    "finalize.py": [f"{PUB}: stage and commit the packet (from PUBLISH-04's finalize.py)"],
    "push.py": [f"{PUB}: the one explicit-refspec push (from PUBLISH-04's push.py)"],
}
NOT_CARRIED = {
    "c28_admit.py": "C28 only (Tranche 1)", "c28_controls.py": "C28 only (Tranche 1)",
    "healthy.py": "Tranche 1's healthy build (C22's e4); not rerun", "rehearsal.py": "Tranche 1's synthetic rehearsal",
    "tool_controls.py": "Tranche 1's synthetic workspace controls", "verifier_controls.py": "RESUME-01's verifier "
    "controls (the verifier is unchanged)", "normalize.py": "e1 baseline branch accounting; no baseline runs here",
    "render_rehearsal.py": "replaced by rehearsals.py",
}
CARRIED_BYTE_FOR_BYTE = ["supervise.py", "worktrees.py", "verify_mutant.py", "verify_tree.py", "scope-wrap.sh",
                         "report.py", "configs.py"]
PUBLICATION_BASE = {"publish_checks.py": "publish_checks.py", "worktree.py": "worktree.py", "finalize.py": "finalize.py",
                    "push.py": "push.py"}
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "finish"],
              "expectations.py": ["fn_span", "attrs_above", "verdict_assertion"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_KEPT = ["REPO", "GIT_COMMON_DIR", "NEXUS", "C", "C_TREE", "C_PARENT", "CRATE", "CRATE_SRC", "PROTOCOL", "MODEL",
               "UNIT", "BLOBS", "R2", "R2_TREE", "R2_PARENT", "R2_REF", "R2_DIR", "CATALOG_PATH", "CATALOG_BLOB",
               "CATALOG_SHA", "CATALOG_VERSION", "DEFS_PATH", "DEFS_BLOB", "DEFS_SHA", "CONTRACT_COMMIT", "CONTRACT_PATH",
               "CONTRACT_SHA", "AD1R4", "AD1R4_SHA", "R5_COMMIT", "AD1R5", "AD1R5_SHA", "STAGE1", "STAGE1_PACKET",
               "STAGE2", "STAGE2_PACKET", "BASE_WT", "EXE", "EXE_SHA256", "EXE_SIZE", "BUILD_ARGV", "BUILD_LIMIT",
               "RUN_ENV", "MEMORY_MAX", "NEED_KIB", "LISTED", "REMOTE", "SLUG", "TEST_PREFIX", "UNIT_PREFIX_TEST"]
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "NOT_AUTHORIZED", "ACCEPTED", "UNIT_PREFIX", "RULING",
                  "RULING_FIRST", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH", "PACKET_LIMIT"}
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
    """Every process launch, as the literal argv[0], 'python' (sys.executable) or UNKNOWN."""
    t = tree(path)
    out = []
    for n in ast.walk(t):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name):
            mod, f = n.func.value.id, n.func.attr
            if (mod == "subprocess" and f in ("run", "Popen", "call", "check_call", "check_output")) or (
                    mod == "os" and (f in ("system", "popen") or f.startswith(("exec", "spawn")))):
                a = n.args[0] if n.args else None
                head = "UNKNOWN"
                if isinstance(a, ast.List) and a.elts:
                    e = a.elts[0]
                    if isinstance(e, ast.Constant) and isinstance(e.value, str):
                        head = e.value
                    elif ast.unparse(e) == "sys.executable":
                        head = "python"
                out.append((n.lineno, head))
    return out


def run():
    sums = {}
    for name, packet in K.T1_PACKETS:
        for line in K.git("show", f"{K.T1}:{packet}/SHA256SUMS").decode().splitlines():
            h, rel = line.split("  ", 1)
            sums.setdefault(name, {})[rel] = h
    c2 = {r[6:]: h for r, h in sums["CONTINUE-02"].items() if r.startswith("tools/")}
    r1 = {r[6:]: h for r, h in sums["RESUME-01"].items() if r.startswith("tools/")}
    accepted = dict({t: h for t, h in r1.items() if t not in c2}, **c2)
    p4 = "publication-recovery/publish-04/tools/"
    pub = {r[len(p4):]: h for r, h in sums["CONTINUE-02"].items() if r.startswith(p4)}
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    oldp = {f: K.sha256_file(os.path.join(OLDP, f)) for f in sorted(os.listdir(OLDP))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the accepted Tranche-1 execution tooling (CONTINUE-02's tools/ and "
          f"the RESUME-01 tools it ran unchanged; sha256 from the packet SHA256SUMS at {K.T1[:8]})", old == accepted,
          sorted(set(old) ^ set(accepted)))
    check(f"D1 original-publication-tools/ ({len(oldp)}) is exactly the PUBLISH-04 publication tooling", oldp == pub,
          sorted(set(oldp) ^ set(pub)))
    status = {f: ("UNCHANGED" if f in new and new[f] == old[f] else "CHANGED" if f in new else "NOT CARRIED")
              for f in old}
    status.update({f: "ADDED" for f in new if f not in old})
    check("D2 the changed tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "CHANGED")
          == sorted(CHANGED), sorted(f for f, s in status.items() if s == "CHANGED"))
    check("D2 the added tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "ADDED")
          == sorted(ADDED), sorted(f for f, s in status.items() if s == "ADDED"))
    check("D2 the tools not carried are exactly the declared ones (none of them is imported or run by a carried tool)",
          sorted(f for f, s in status.items() if s == "NOT CARRIED") == sorted(NOT_CARRIED)
          and not [f for f in new if f.endswith(".py") and any(
              isinstance(n, (ast.Import, ast.ImportFrom)) and any(a.name.split(".")[0] + ".py" in NOT_CARRIED
                                                                  for a in n.names)
              for n in ast.walk(tree(os.path.join(NEW, f))))], sorted(f for f, s in status.items() if s == "NOT CARRIED"))
    check("D2 the unchanged tools are exactly the byte-for-byte carried ones",
          sorted(f for f, s in status.items() if s == "UNCHANGED") == sorted(CARRIED_BYTE_FOR_BYTE),
          sorted(f for f, s in status.items() if s == "UNCHANGED"))
    for f in CARRIED_BYTE_FOR_BYTE:
        check(f"D3 {f} byte-identical (sha256 {old.get(f, '')[:16]}…)", status.get(f) == "UNCHANGED", status.get(f))
    check("D3 scope-wrap.sh keeps its executable mode", os.access(os.path.join(NEW, "scope-wrap.sh"), os.X_OK))
    # D4: classify.py.
    to, tn = tree(os.path.join(OLD, "classify.py")), tree(os.path.join(NEW, "classify.py"))
    fo, fnw = funcs(to), funcs(tn)
    other = [n for n in fo if n != "classify_a" and fo[n] != fnw.get(n)]
    check("D4 classify.py: every function but classify_a is AST-identical to Tranche 1's", not other and set(fo) == set(fnw),
          other)
    mod_o = [ast.dump(n) for n in to.body if not isinstance(n, ast.FunctionDef)]
    mod_n = [ast.dump(n) for n in tn.body if not isinstance(n, ast.FunctionDef)]
    check("D4 classify.py: every module-level statement (constants, imports, the command-line block) unchanged",
          mod_o == mod_n)
    a, b = open(os.path.join(OLD, "classify.py")).read().splitlines(), open(os.path.join(NEW, "classify.py")).read().splitlines()
    ca_o, ca_n = fn(to, "classify_a"), fn(tn, "classify_a")
    hunks = [op for op in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes() if op[0] != "equal"]
    inside = all(ca_o.lineno <= i1 + 1 and i2 <= ca_o.end_lineno and ca_n.lineno <= j1 + 1 and j2 <= ca_n.end_lineno
                 for _, i1, i2, j1, j2 in hunks)
    added = "\n".join(l for _, i1, i2, j1, j2 in hunks for l in b[j1:j2])
    check("D4 classify.py: every changed line lies inside classify_a, and the change names only the catalog-required "
          "fields (permitted_properties, key_fields)", inside and "permitted_properties" in added and "key_fields" in added
          and not [w for w in ("ceiling", "states", "limit_seconds", "check_supervision", "check_identity",
                               "check_selection") if w in added], f"{len(hunks)} hunks")
    ctl_path = os.path.join(K.EV, "receipts/03-t2-controls/CONTROLS.json")
    if os.path.exists(ctl_path):
        import json
        ctl = json.load(open(ctl_path))
        same = [c for c in ctl if "Tranche-1 classify.py on an identical copy" in c["case"]]
        check(f"D4 classify_a's change is inert for every single-property control: Tranche 1's classifier writes "
              f"byte-identical outputs on {len(same)} synthetic receipts (receipts/03-t2-controls)",
              len(same) >= 18 and all(c["as_expected"] for c in same), len(same))
    else:
        check("D4 classify_a's inertness record (receipts/03-t2-controls/CONTROLS.json) exists", False, ctl_path)
    for f, names in SAME_FUNCS.items():
        fa, fb = funcs(tree(os.path.join(OLD, f))), funcs(tree(os.path.join(NEW, f)))
        diff = [n for n in names if fa.get(n) is None or fa.get(n) != fb.get(n)]
        check(f"D4 {f}: functions {names} AST-identical to Tranche 1's", not diff, diff)
    mo, mn = fn(tree(os.path.join(OLD, "control.py")), "main"), fn(tree(os.path.join(NEW, "control.py")), "main")
    check("D4 control.main: Tranche 1's, statement for statement, but for the leading refusal (the twelve accepted "
          "controls; the Tranche-2 order) and the removed C28 dispatch",
          [ast.dump(s) for s in mo.body[2:]] == [ast.dump(s) for s in mn.body[1:]]
          and ast.unparse(mo.body[1]) == "if cid == 'C28':\n    return main_c28()"
          and "K.ACCEPTED" in ast.unparse(mn.body[0]) and "K.ORDER" in ast.unparse(mn.body[0]))
    tc = tree(os.path.join(NEW, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main",
          [n.name for n in tc.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main"])
    # D5: common.py.
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    kept = [k for k in COMMON_KEPT if co.get(k, "<absent>") != cn.get(k, "<absent>")]
    check(f"D5 common.py: the {len(COMMON_KEPT)} names the reused tools and this mission read with their accepted values "
          "keep them (identities, catalog, contract, Stage-1/2, baseline executable, build and run policy, limits)",
          not kept, kept)
    changed = sorted(k for k in co if k in cn and co[k] != cn[k])
    check("D5 common.py: values changed only for the declared mission identity, namespace, order and publication names",
          set(changed) <= COMMON_CHANGED, changed)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mt2x), never Tranche 1's",
          cn["UNIT_PREFIX"] == "p3c11-mt2x" and K.control_wt("C23").endswith("/nexus-os-p3-c11-mt2x-C23"))
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
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build/invoke only in control.main" if f == "control.py" else "; no supervise.build/invoke"),
              not bad and not lists and (not builds if f != "control.py" else
                                         all(c in [x for x, _ in calls(fn(t, "main"))] for c in builds)),
              f"{bad} {lists} {builds}")
    return old, oldp, new, status


def main(argv):
    old, oldp, new, status = run()
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
            base = OLD if status[f] == "CHANGED" else (OLDP if f in PUBLICATION_BASE else None)
            src = os.path.join(base, f if base == OLD else PUBLICATION_BASE.get(f, f)) if base else None
            if status[f] == "CHANGED" and f in PUBLICATION_BASE:
                diff += difflib.unified_diff(open(os.path.join(OLDP, f)).read().splitlines(keepends=True),
                                             open(os.path.join(NEW, f)).read().splitlines(keepends=True),
                                             f"PUBLISH-04/tools/{f}", f"TRANCHE2/tools/{f} (against the publication base)")
            a = open(src).read().splitlines(keepends=True) if src else []
            label = ("CONTINUE-02-or-RESUME-01/tools/" if base == OLD else "PUBLISH-04/tools/") + f if src else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"TRANCHE2/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the accepted Tranche-1 execution tooling (`original-tools/`: `{K.CONTINUE02_PACKET}/tools/` at `{K.T1}` "
          f"and the RESUME-01 tools it ran unchanged, `{K.RESUME01_PACKET}/tools/`), and for publication the PUBLISH-04 "
          f"tooling (`{K.CONTINUE02_PACKET}/publication-recovery/publish-04/tools/`). New: this mission's `tools/`. The "
          "complete old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte (mission section 6)"]))
        o = old.get(f) or (oldp.get(PUBLICATION_BASE[f]) if f in PUBLICATION_BASE else None) or "—"
        md.append(f"| `{f}` | {status[f]} | `{o}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Unchanged in substance (mission section 6): the apply, build, run, restoration and classification steps "
           "(control.py's apply_patch, restore, restored_check and main; supervise.py; verify_mutant.py; verify_tree.py; "
           "worktrees.py; classify.py but for the catalog-required classify_a alternatives and A6 key fields); the "
           "zero-error, zero-warning build policy for every control (classify_build_success, unchanged; no C28-style "
           "exception exists here)."]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
