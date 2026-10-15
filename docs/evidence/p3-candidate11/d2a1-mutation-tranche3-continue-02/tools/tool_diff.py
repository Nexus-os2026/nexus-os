#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by -CONTINUE-02, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01,
-TRANCHE2-CONTINUE-02 and -TRANCHE3-EXEC-01. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 16): old
= the Tranche-3 execution tooling (original-tools/: the Tranche-3 packet's tools/ at 774f457a and the tools it carried
byte for byte); new = this mission's tools/; the authorization map is mission section 16's permitted differences (retain
the general build classifier byte-identical, the original-explorer classifier, the path-aware source/restoration checks,
L1 complete-search validation, the C7 informational logic and the restoration-bound link format; add only the C24
existing-build admission/pin flow, the C3b and C3c warning admission (as amended by AMENDMENT-01), the Architect
acceptance/status records and the continuation ordering/publication bindings). Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/C5-TO-C24.diff, OUTDIR/AUTHORIZATION.md and
                             OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

  D1  original-tools/ is exactly the Tranche-3 execution tooling (names and sha256: its packet's SHA256SUMS for its
      tools/, and the commits and paths its CARRIED-FORWARD.md names for the tools it carried byte for byte)
  D2  every tool is UNCHANGED, CHANGED or ADDED exactly as the authorization map declares; nothing is not carried
  D3  byte-identical: classify.py (the general build classifier classify_build_success, which rejects every warning, and
      the run classifier classify_a), t3.py (the original-explorer, permitted-property, A4, L1 complete-search and C7
      informational rules), supervise.py, worktrees.py, verify_mutant.py, verify_tree.py, scope-wrap.sh, report.py,
      configs.py and push.py; the only warning-admission code is c24.py (C24) and c3w.py (C3b, C3c), each refusing every
      other control as its first check, and control.py calls c24.admit only in main_c24 and c3w.admit only under
      `if cid in c3w.CONTROLS:` with CONTROLS == ["C3b", "C3c"]
  D4  core semantics: control.py's git, apply_patch, restored_check, restore and finish AST-identical to Tranche 3's;
      gate(), main() and link() differ from Tranche 3's by exactly the authorized hunks listed below (nothing else);
      main_c24 is the accepted continuation's main_c5 (b97205b2) with C5 named C24 and Tranche 2 named Tranche 3, its
      docstring, the continuation indentation of four calls and the inserted tranche-3 rules step (and-ed with
      classify_a) the only other differences; c24.py's parse_group, text_lines, parse_ts, mutant_sources and write, and its
      parser regular expressions, equal c5.py's with C5 named C24; the helpers of expectations.py, authenticate.py and
      build_packet.py AST-identical to Tranche 3's
  D5  common.py: every name the reused tools import keeps its accepted value and its helper functions but control_wt are
      AST-identical; changed and added names are only the declared ones; the Tranche-3 records keep their texts under
      RULING_T3X and CAMPAIGN_T3X
  D6  no changed or added tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build is
      called only by control.main (once) and supervise.invoke only by control.main (the kill run and C7's informational
      run) and control.main_c24 (the kill run)
"""
import ast
import difflib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
ORD = "item: continuation ordering"
C24 = "item: C24 existing-build admission/pin flow"
C3W = "item: C3b/C3c warning admission (AMENDMENT-01)"
REC = "item: Architect acceptance/status records"
PUB = "item: publication bindings"
FRZ = "item: freeze/final-gate bindings"
CHANGED = {
    "common.py": [f"{ORD}, {PUB}, {FRZ}: mission identity, evidence root, worktree namespace (mt3c; C24's preserved "
                  "mt3x worktree), order C24 C3b C3c C6 C7 A1 A2 C20 A5 (receipt directories 81-89), the 23 accepted "
                  "controls (C2 added), the nine-control matrix, the Tranche-3 identities and C2's accepted evidence, "
                  f"publication names and parent; {C24}: the preserved C24 build, executable and worktree identities; "
                  f"{C3W}: the C3b/C3c permitted warning identities; {REC}: the four records"],
    "control.py": [f"{ORD}: gate() over this order (C24's prior steps: the four frozen records and the pre-execution "
                   "gate; C24's preserved worktree must be present)",
                   f"{C24}: main_c24 (the continuation's main_c5 for C24, with the tranche-3 rules after classify_a); "
                   "main() dispatches C24 to it; link() binds C24's link to its own reauthentication receipt",
                   f"{C3W}: main(), for C3b and C3c only, after the unchanged general classifier: c3w.admit decides "
                   "admission and names the pin record and the run's prior step; every other control unchanged"],
    "expectations.py": [f"{ORD}: the nine controls in this order; {C24}: C24's preserved-build identities; {C3W}: each "
                        "control's build rule"],
    "authenticate.py": [f"{FRZ}: every input of this mission; section [10]: the Tranche-3 evidence, C2 (accepted), C24's "
                        "preserved receipts (copied under inputs/c24/) and the base tooling check (moved from [9])"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records (the four records; C24's preserved identities; the "
                         "build rules)"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before C24 (E1-E11 for this mission)"],
    "final_gate.py": [f"{FRZ}: the final gate (C24's own receipt chain and preserved artifacts; C3b/C3c admission; this "
                      "mission's worktrees)"],
    "render.py": [f"{PUB}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md and the derived status (C24's admission, "
                  "pin and reauthentication; C3b/C3c admission)"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent; C24's preserved executable among the pinned ones"],
    "publish_checks.py": [f"{PUB}: parent, every previous packet unchanged, refs, the key receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission"],
    "t3_controls.py": [f"{C24}, {C3W}: the required negative controls (mission section 16) on synthetic and historical "
                       "evidence: sections A-F retargeted, G (C24) and H (C3b/C3c) added"],
}
ADDED = {
    "c24.py": [f"{C24}: the dedicated C24 build-admission checker, the preserved-executable pin and the preserved "
               "worktree check (the accepted continuation's c5.py for C24's one warning group)"],
    "c3w.py": [f"{C3W}: the dedicated C3b/C3c build-admission checker: a clean build or exactly the one predicted warning "
               "group; refuses every other control"],
}
NOT_CARRIED = {}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py", "t3.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "finish"],
              "expectations.py": ["fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb", "original_cfg", "fn_text"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section4", "section5", "section7", "section8"],
              "build_packet.py": ["allowlists", "files_under", "whitespace", "authored", "prior_blobs", "selftest_block",
                                  "dup_policy"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "FIRST_DIR", "ACCEPTED", "MATRIX", "UNIT_PREFIX",
                  "ACCEPTED_WT", "RULING", "RULING_FIRST", "CAMPAIGN", "CAMPAIGN_FIRST", "PARENT", "PARENT_TREE",
                  "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"C24_WT", "ACCEPTED_T3", "RULING_T3X", "RULING_T3X_FIRST", "CAMPAIGN_T3X", "CAMPAIGN_T3X_FIRST",
                "W_RULING", "W_RULING_FIRST", "AMENDMENT", "AMENDMENT_FIRST", "RECORDS", "RECORD_FIRST", "T3X",
                "T3X_TREE", "T3X_PARENT", "T3X_REF", "T3X_PACKET", "T3X_RECEIPTS", "T3X_ACCEPTED_EVIDENCE",
                "C24_BUILD_DIR", "C24_APPLY_DIR", "C24_PATCH_SHA256", "C24_MUTANT_BLOB", "C24_MUTANT_SHA256", "C24_EXE",
                "C24_EXE_SHA256", "C24_EXE_SIZE", "C24_STDERR_SHA256", "C24_LOCK_REASON", "C3W", "C3W_ENUM"}
# The authorized hunks of control.py (mission section 16), exactly: (function, old lines, new lines).
AUTH_HUNKS = [
    ("gate", ['        return [f"{cid} is not in this mission\'s order (the 22 accepted controls are never executed again; '
              'C1, C4 and "'],
     ['        return [f"{cid} is not in this mission\'s order (the 23 accepted controls are never executed again; '
      'C1, C4 and "']),
    ("gate", ['        need += [(K.RULING, K.RULING_FIRST), (K.CAMPAIGN, K.CAMPAIGN_FIRST)]'],
     ['        need += [(r, K.RECORD_FIRST[r]) for r in K.RECORDS]']),
    ("gate", ['        frozen = [r for r in [K.RULING, K.CAMPAIGN, "receipts/02-EXPECTATIONS.json"] if r not in fr["files"]]',
              '        lines.append(f"the Architect ruling record, the campaign status record and the expectations are in '
              'the freeze: "',
              '                     f"{\'yes\' if not frozen else \'NO: \' + str(frozen)}")'],
     ['        frozen = [r for r in K.RECORDS + ["receipts/02-EXPECTATIONS.json"] if r not in fr["files"]]',
      '        lines.append(f"the Architect ruling, build-ruling and amendment records, the campaign status record and '
      'the "',
      '                     f"expectations are in the freeze: {\'yes\' if not frozen else \'NO: \' + str(frozen)}")']),
    ("gate", ['    if os.path.lexists(exp["controls"][cid]["worktree"]):'],
     ['    if cid == "C24":', '        if not os.path.isdir(exp["controls"][cid]["worktree"]):',
      '            problems.append("the preserved C24 worktree is absent")',
      '    elif os.path.lexists(exp["controls"][cid]["worktree"]):']),
    ("main", [], ['    if cid == "C24":', '        return main_c24()']),
    ("main", ['            if first != "BUILD: PINNED":',
              '                stop = "the mutant build did not succeed (kills nothing; no rebuild)"'],
     ['            # TRANCHE3-CONTINUE-02 (mission sections 3-5, AMENDMENT-01): C3b and C3c only: the dedicated admission',
      '            # checker decides (a clean build, or exactly the one predicted warning group) and names the pin record.',
      '            admitted, pin_file = first == "BUILD: PINNED", os.path.join(bdir, "PIN.json")',
      '            bprior = [(ctl["dir"] + "/03-build/CLASSIFICATION.txt", "BUILD: PINNED")]',
      '            if cid in c3w.CONTROLS:',
      '                admitted, pin_file, afirst = c3w.admit(bdir, ctl, os.path.join(outdir, "03a-admission"))',
      '                facts.append(f"build admission (C3b/C3c rule, AMENDMENT-01): {afirst}")',
      '                bprior = [(ctl["dir"] + "/03a-admission/ADMISSION.txt", afirst)]',
      '            if not admitted:',
      '                stop = ("the mutant build is not admissible under its build rule (kills nothing; no rebuild)"',
      '                        if cid in c3w.CONTROLS else "the mutant build did not succeed (kills nothing; no rebuild)")']),
    ("main", ['                pin = json.load(open(os.path.join(bdir, "PIN.json")))'],
     ['                pin = json.load(open(pin_file))']),
    ("main", ['                                 "mutant-" + ctl["kind"], [(ctl["dir"] + "/03-build/CLASSIFICATION.txt", '
              '"BUILD: PINNED")],'],
     ['                                 "mutant-" + ctl["kind"], bprior,']),
    ("link", ['    own = os.path.join(os.path.dirname(outdir), "05-restore/RESTORE.txt")',
              '    chk("this control\'s own restoration receipt exists and records RESTORE: EXACTLY_C (no link before '
              'restoration)",',
              '        supervise.first_line(own) == RESTORE_OK, f"{os.path.relpath(own, K.EV)}: '
              '{supervise.first_line(own)!r}")'],
     ['    # TRANCHE3-CONTINUE-02 (mission sections 9 and 17): C24 (no patch applied here) links only after its own',
      '    # reauthentication receipt records REAUTH_OK.',
      '    own = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else '
      '"05-restore/RESTORE.txt")',
      '    want_own = REAUTH_OK if ctl["id"] == "C24" else RESTORE_OK',
      '    chk(f"this control\'s own {\'reauthentication\' if ctl[\'id\'] == \'C24\' else \'restoration\'} receipt '
      'exists and records "',
      '        f"{want_own} (no link before {\'reauthentication\' if ctl[\'id\'] == \'C24\' else \'restoration\'})",',
      '        supervise.first_line(own) == want_own, f"{os.path.relpath(own, K.EV)}: {supervise.first_line(own)!r}")']),
    ("link", ['    rest = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C5" else '
              '"05-restore/RESTORE.txt")',
              '    rest_line = (f"after this control\'s own {\'reauthentication\' if ctl[\'id\'] == \'C5\' else '
              '\'restoration\'} receipt "'],
     ['    rest = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else '
      '"05-restore/RESTORE.txt")',
      '    rest_line = (f"after this control\'s own {\'reauthentication\' if ctl[\'id\'] == \'C24\' else '
      '\'restoration\'} receipt "']),
]
# main_c24 = the continuation's main_c5 with these substitutions (and the differences D4 names).
C5_SUB = [("main_c5", "main_c24"), ("C5", "C24"), ("c5.", "c24."), ("Tranche-2", "Tranche-3"), ("Tranche 2", "Tranche 3"),
          ("K.T2}", "K.T3X}"), ("K.T2,", "K.T3X,")]
T2C_TOOLS = "docs/evidence/p3-candidate11/d2a1-mutation-tranche2-continue-02/tools/"
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


def src_lines(path, node):
    return open(path).read().splitlines()[node.lineno - 1:node.end_lineno]


def opcodes(a, b):
    return [op for op in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes() if op[0] != "equal"]


def subst(lines):
    out = []
    for line in lines:
        for a, b in C5_SUB:
            line = line.replace(a, b)
        out.append(line)
    return out


def run():
    t3xsums = {}
    for line in K.git("show", f"{K.T3X}:{K.T3X_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t3xsums[rel[6:]] = h
    carried = {}
    for line in K.git("show", f"{K.T3X}:{K.T3X_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions")[0].splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = K.sha256_bytes(K.git("show", f"{cells[2]}:{cells[1]}"))
            check(f"D1 carried tool {cells[0]}: its sha256 at {cells[2][:8]}:{cells[1]} is the one the Tranche-3 "
                  "CARRIED-FORWARD.md records", carried[cells[0]] == cells[4], cells[4][:16])
    base = dict(carried, **t3xsums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the Tranche-3 execution tooling: its packet's tools/ "
          f"({len(t3xsums)}; SHA256SUMS at {K.T3X[:8]}) and the tools it carried byte for byte ({len(carried)})",
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
    for f in CARRIED_BYTE_FOR_BYTE:
        check(f"D3 {f} byte-identical (sha256 {old.get(f, '')[:16]}…)", status.get(f) == "UNCHANGED", status.get(f))
    check("D3 scope-wrap.sh keeps its executable mode", os.access(os.path.join(NEW, "scope-wrap.sh"), os.X_OK))
    check("D3 the general build classifier (classify.classify_build_success) is byte-identical: its check 'no error or "
          "warning diagnostic' still rejects every warning (no generic warning exception)",
          status.get("classify.py") == "UNCHANGED" and "ck(\"no error or warning diagnostic\", not errs, errs[:3])" in
          open(os.path.join(NEW, "classify.py")).read())
    # D3: the only warning-admission code, and where it is called.
    adm_defs = sorted(f for f in new if f.endswith(".py") and f not in CARRIED_BYTE_FOR_BYTE and any(
        isinstance(n, ast.FunctionDef) and n.name == "stderr_admission" for n in ast.walk(tree(os.path.join(NEW, f)))))
    check("D3 the warning-group admission (stderr_admission) is defined only in c24.py", adm_defs == ["c24.py"], adm_defs)
    tc24, tc3w = tree(os.path.join(NEW, "c24.py")), tree(os.path.join(NEW, "c3w.py"))
    first_c24 = [n for n in fn(tc24, "admit").body if isinstance(n, ast.If)][0]
    check("D3 c24.admit's first check refuses every control but C24 (the C24 ruling's patch and mutant identities)",
          "ctl.get('id') == 'C24'" in ast.unparse(first_c24.test) and "K.C24_PATCH_SHA256" in ast.unparse(first_c24.test)
          and "return (rec, False)" in ast.unparse(first_c24.body), ast.unparse(first_c24.test)[:120])
    first_c3w = [n for n in fn(tc3w, "admit").body if isinstance(n, ast.If)][0]
    controls = [n for n in tc3w.body if isinstance(n, ast.Assign) and ast.unparse(n.targets[0]) == "CONTROLS"]
    check("D3 c3w.CONTROLS is exactly ['C3b', 'C3c'] and c3w.admit's first check refuses every other control",
          len(controls) == 1 and ast.literal_eval(controls[0].value) == ["C3b", "C3c"]
          and "cid in CONTROLS" in ast.unparse(first_c3w.test) and "return (False, None," in ast.unparse(first_c3w.body),
          ast.unparse(first_c3w.test)[:120])
    tn = tree(os.path.join(NEW, "control.py"))
    where = {}
    for f_ in [n for n in tn.body if isinstance(n, ast.FunctionDef)]:
        for c, node in calls(f_):
            if c in ("c24.admit", "c3w.admit"):
                where.setdefault(c, []).append(f_.name)
    c3w_if = [n for n in ast.walk(fn(tn, "main")) if isinstance(n, ast.If) and ast.unparse(n.test) == "cid in c3w.CONTROLS"]
    check("D3 control.py calls c24.admit only in main_c24 and c3w.admit only in main, inside `if cid in c3w.CONTROLS:`",
          where == {"c24.admit": ["main_c24"], "c3w.admit": ["main"]} and len(c3w_if) == 1
          and any(c == "c3w.admit" for c, _ in calls(c3w_if[0])), where)
    other_mods = [f for f in new if f.endswith(".py") and f not in ("control.py", "t3_controls.py", "c3w.py", "final_gate.py",
                                                                    "pre_exec_gate.py")
                  and any(c in ("c24.admit", "c3w.admit") for c, _ in calls(tree(os.path.join(NEW, f))))]
    check("D3 no other tool admits a build (only control.py; the controls and gates read admission records)", not other_mods,
          other_mods)
    # D4: control.py.
    to = tree(os.path.join(OLD, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main_c24, main",
          [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main_c24", "main"])
    for name in ("gate", "main", "link"):
        a = src_lines(os.path.join(OLD, "control.py"), fn(to, name))
        b = src_lines(os.path.join(NEW, "control.py"), fn(tn, name))
        got = [(name, a[op[1]:op[2]], b[op[3]:op[4]]) for op in opcodes(a, b)]
        want = [h for h in AUTH_HUNKS if h[0] == name]
        check(f"D4 control.{name}: Tranche 3's with exactly the {len(want)} authorized hunk(s) (old and new lines exact; "
              "nothing else differs)", got == want, f"{len(got)} hunks")
    t2c_control = K.git("show", f"{K.T2C}:{T2C_TOOLS}control.py").decode()
    a = subst(t2c_control.splitlines()[fn(ast.parse(t2c_control), "main_c5").lineno - 1:
                                       fn(ast.parse(t2c_control), "main_c5").end_lineno])
    b = src_lines(os.path.join(NEW, "control.py"), fn(tn, "main_c24"))
    ops = opcodes(a, b)
    norm = lambda ls: [re.sub(r"\s+", " ", x).strip() for x in ls]  # noqa: E731
    rest = [(a[op[1]:op[2]], b[op[3]:op[4]]) for op in ops if norm(a[op[1]:op[2]]) != norm(b[op[3]:op[4]])]
    doc = [r for r in rest if r[0] and r[0][0].lstrip().startswith('"""')]
    rules = [r for r in rest if r not in doc]
    check("D4 control.main_c24: the continuation's main_c5 (b97205b2) with C5 named C24 and Tranche 2 named Tranche 3; "
          "beyond its docstring and continuation indentation, the only difference is the inserted tranche-3 rules step "
          "after classify_a (and-ed; it can only withdraw candidacy)",
          len(doc) == 1 and len(rules) == 1 and rules[0][0] == [
              '    stop = None if candidate else "the mutant run is not admissible kill evidence of the named property"']
          and rules[0][1] == ['    rules_ok = t3.rules(os.path.join(outdir, "04a-rules"), rdir, ctl)',
                              '    facts.append(f"tranche-3 rules: {supervise.first_line(os.path.join(outdir, '
                              '\'04a-rules/RULES.txt\'))}")',
                              '    candidate = candidate and rules_ok',
                              '    stop = None if candidate else "the mutant run is not admissible kill evidence of the '
                              'named property or test"'], f"{len(ops)} hunks; {len(rest)} beyond whitespace")
    t2c_c5 = K.git("show", f"{K.T2C}:{T2C_TOOLS}c5.py").decode()
    c5t = ast.parse(t2c_c5)
    same = []
    for name in ("parse_group", "text_lines", "parse_ts", "mutant_sources", "write"):
        a = subst(t2c_c5.splitlines()[fn(c5t, name).lineno - 1:fn(c5t, name).end_lineno])
        b = src_lines(os.path.join(NEW, "c24.py"), fn(tc24, name))
        same.append((name, a == b))
    regs = ["HEAD_RE", "LOC_RE", "SRC_RE", "GUTTER_RE", "MARK_RE", "NOTE_RE", "PROGRESS_RE", "FINISHED_RE", "CRASH_RE"]
    val = lambda t, n: next(ast.unparse(x.value) for x in t.body if isinstance(x, ast.Assign)  # noqa: E731
                            and ast.unparse(x.targets[0]) == n)
    same += [(r, val(c5t, r) == val(tc24, r)) for r in regs]
    check("D4 c24.py: parse_group, text_lines, parse_ts, mutant_sources and write, and the parser's regular expressions, "
          "equal the accepted c5.py's (b97205b2) with C5 named C24",
          all(o for _, o in same), [n for n, o in same if not o])
    for f, names in SAME_FUNCS.items():
        fa, fb = funcs(tree(os.path.join(OLD, f))), funcs(tree(os.path.join(NEW, f)))
        diff = [n for n in names if fa.get(n) is None or fa.get(n) != fb.get(n)]
        check(f"D4 {f}: functions {names} AST-identical to Tranche 3's", not diff, diff)
    # D5: common.py.
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    changed = sorted(k for k in co if k in cn and co[k] != cn[k])
    check("D5 common.py: values changed only for the declared names", set(changed) == COMMON_CHANGED,
          sorted(set(changed) ^ COMMON_CHANGED))
    added = sorted(k for k in cn if k not in co)
    check("D5 common.py: names added exactly the declared set (C24, C3b/C3c, Tranche-3 identities, records)",
          set(added) == COMMON_ADDED, sorted(set(added) ^ COMMON_ADDED))
    removed = sorted(k for k in co if k not in cn)
    check("D5 common.py: no name removed", not removed, removed)
    check("D5 common.py: the Tranche-3 records keep their exact texts under RULING_T3X and CAMPAIGN_T3X",
          (cn["RULING_T3X"], cn["RULING_T3X_FIRST"], cn["CAMPAIGN_T3X"], cn["CAMPAIGN_T3X_FIRST"])
          == (co["RULING"], co["RULING_FIRST"], co["CAMPAIGN"], co["CAMPAIGN_FIRST"]))
    check("D5 common.py: the 23 accepted are the 22 with C2 appended; the order is Tranche 3's without C2",
          cn["ACCEPTED"] == co["ACCEPTED"] + ["C2"] and cn["ORDER"] == [c for c in co["ORDER"] if c != "C2"]
          and {c: v for c, v in cn["MATRIX"].items()} == {c: v for c, v in co["MATRIX"].items() if c != "C2"})
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mt3c); C24's worktree is its "
          "preserved Tranche-3 one", cn["UNIT_PREFIX"] == "p3c11-mt3c" and K.control_wt("C24") == K.C24_WT ==
          K.NEXUS + "/nexus-os-p3-c11-mt3x-C24" and all(K.control_wt(c) == K.NEXUS + "/nexus-os-p3-c11-mt3c-" + c
                                                       for c in K.ORDER if c != "C24"))
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
            in_c24 = [x for x, _ in calls(fn(t, "main_c24"))]
            ok_calls = (builds.count("supervise.build") == 1 == in_main.count("supervise.build")
                        and in_c24.count("supervise.build") == 0 and in_main.count("supervise.invoke") == 2
                        and in_c24.count("supervise.invoke") == 1 and builds.count("supervise.invoke") == 3)
        else:
            ok_calls = not builds
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build once (main); supervise.invoke twice in main (kill run, C7 informational run) and once "
                 "in main_c24 (kill run)" if f == "control.py" else "; no supervise.build/invoke"),
              not bad and not lists and ok_calls, f"{bad} {lists} {builds}")
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
            label = f"TRANCHE3-EXEC-01/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"TRANCHE3-CONTINUE-02/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    c5 = K.git("show", f"{K.T2C}:{T2C_TOOLS}c5.py").decode().splitlines(keepends=True)
    open(os.path.join(out, "C5-TO-C24.diff"), "w").write("".join(difflib.unified_diff(
        c5, open(os.path.join(NEW, "c24.py")).read().splitlines(keepends=True), f"TRANCHE2-CONTINUE-02/tools/c5.py "
        f"(b97205b2)", "TRANCHE3-CONTINUE-02/tools/c24.py")))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the Tranche-3 execution tooling (`original-tools/`: `{K.T3X_PACKET}/tools/` at `{K.T3X}` and the tools it "
          "carried byte for byte, at the commits its CARRIED-FORWARD.md names). New: this mission's `tools/`. The "
          "complete old-to-new diff is `TOOL-DIFF.diff`; `c24.py`'s derivation from the accepted continuation's `c5.py` "
          "is `C5-TO-C24.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. Permitted differences (mission section "
          "16): the C24 existing-build admission/pin flow; the C3b and C3c warning admission (as amended by "
          "AMENDMENT-01); the Architect acceptance/status records; the continuation ordering/publication bindings.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte"]))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Byte-identical: `classify.py` (the general build classifier `classify_build_success`, which rejects every "
           "warning, and the run classifier `classify_a`), `t3.py` (the original-explorer rules, L1 complete-search "
           "validation and C7's informational classifier), `supervise.py`, `worktrees.py`, `verify_mutant.py`, "
           "`verify_tree.py`, `scope-wrap.sh`, `report.py`, `configs.py`, `push.py`. The only warning admission is "
           "control-specific: `c24.py` (C24 only, its one preserved warning group) and `c3w.py` (C3b and C3c only: a "
           "clean build or exactly the one predicted warning group); each refuses every other control as its first "
           "check, and `control.py` calls each only for its own controls.", ""]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
