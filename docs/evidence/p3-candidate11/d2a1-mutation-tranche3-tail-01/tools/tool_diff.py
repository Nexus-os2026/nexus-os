#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by -CONTINUE-02, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01,
-TRANCHE2-CONTINUE-02, -TRANCHE3-EXEC-01 and -TRANCHE3-CONTINUE-02. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01
(section 5, steps 5-7): old = the Tranche-3 continuation's execution tooling (original-tools/: its packet's tools/ at
c10681b3 and the tools it carried byte for byte); new = this mission's tools/; the authorization map is the minimal
retarget to the four controls A1 A2 C20 A5 (mission section 5, step 6): retain the general build classifier, the
original-explorer classifier, the tranche-3 rules (L1 complete-search validation among them), the supervision, the
path-aware source and restoration checks and the restoration-bound link format byte-identical or AST-identical; remove
only what the four controls do not use (the C24 preserved-build flow and c24.py, the C3b/C3c warning admission and
c3w.py, C7's informational step); add only C7's explicit refusal, the mission's section-12 result labels, the
acceptance/status records and the ordering/publication bindings. Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

  D1  original-tools/ is exactly the continuation's execution tooling (names and sha256: its packet's SHA256SUMS for its
      tools/, and the commits and paths its CARRIED-FORWARD.md names for the tools it carried byte for byte)
  D2  every tool is UNCHANGED, CHANGED or NOT CARRIED exactly as the authorization map declares; nothing is added
  D3  byte-identical: classify.py (the general build classifier classify_build_success, which rejects every warning, and
      the run classifier classify_a), t3.py (the original-explorer, permitted-property and L1 complete-search rules),
      supervise.py, worktrees.py, verify_mutant.py, verify_tree.py, scope-wrap.sh, report.py, configs.py and push.py; no
      warning-admission code anywhere (no stderr_admission, no c24 or c3w module imported or called by any tool)
  D4  control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish and main; git,
      apply_patch, restored_check, restore and finish AST-identical to the continuation's; gate(), main() and link()
      differ from the continuation's by exactly the authorized hunks listed below (nothing else); main()'s first
      statement refuses C7 by name; the helpers of expectations.py, authenticate.py, build_packet.py, t3_controls.py,
      rehearsals.py, final_gate.py and pre_exec_gate.py named in SAME_FUNCS AST-identical to the continuation's
  D5  common.py: every name the reused tools import keeps its accepted value and its helper functions but control_wt are
      AST-identical; changed and added names are only the declared ones; nothing is removed; the continuation's records
      keep their texts under RULING_T3C and CAMPAIGN_T3C; the order is the continuation's last four controls and the
      matrix their rows; the 27 accepted are the 23 with C24 C3b C3c C6; C7 alone is blocked
  D6  no changed tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build and
      supervise.invoke are each called exactly once, by control.main (the build and the kill run)
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
ORD = "item: ordering (A1 A2 C20 A5)"
C7X = "item: C7 excluded (MUTATION_CONTROL_BLOCKED)"
RES = "item: section-12 result labels"
REC = "item: Architect acceptance/status records"
PUB = "item: publication bindings"
FRZ = "item: freeze/gate bindings"
RMV = "item: removal of what the four controls do not use"
CHANGED = {
    "common.py": [f"{ORD}, {PUB}, {FRZ}: mission identity, evidence root, worktree namespace (mt3t; every control a fresh "
                  "worktree), order A1 A2 C20 A5 (receipt directories 81-84), the 27 accepted controls (C24 C3b C3c C6 "
                  "added), the four-control matrix, the continuation's identities with the accepted controls' and C7's "
                  f"evidence, publication names and parent; {C7X}: BLOCKED = ['C7'] and its worktree; {REC}: the two "
                  "records (the continuation's own two kept under RULING_T3C and CAMPAIGN_T3C)"],
    "control.py": [f"{ORD}: gate() over this order (A1's prior steps: the two frozen records and the pre-execution gate); "
                   f"{C7X}: main() refuses C7 by name first; {RES}: STOPPED BEFORE EXECUTION (gate refused), INCOMPLETE — "
                   "ARCHITECT REVIEW REQUIRED (classify_a's run INCOMPLETE), MUTATION_CONTROL_BLOCKED otherwise; "
                   f"{RMV}: main_c24, C24's link binding, the C3b/C3c admission branch and C7's informational step"],
    "expectations.py": [f"{ORD}: the four controls in this order; {C7X}: the catalog partition counts C7 as blocked and "
                        "no expectation exists for it"],
    "authenticate.py": [f"{FRZ}: every input of this mission; section [10] without C24's input copies; the new section "
                        "[11]: the continuation evidence, its four accepted controls, C7's blocked receipts and the base "
                        "tooling check"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records (the two records; the continuation's identities; this "
                         "mission's policy)"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before A1 (E1-E11 for this mission; C7 refused)"],
    "final_gate.py": [f"{FRZ}: the final gate (one receipt chain per control; the section-12 labels; this mission's "
                      "worktrees; the accepted controls' and C7's worktrees; no C7 receipt)"],
    "render.py": [f"{PUB}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md and the derived status (the section-12 "
                  "labels)"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent (docstring; the rendering directory's name)"],
    "publish_checks.py": [f"{PUB}: parent, every previous packet unchanged (the continuation's among them), refs, the key "
                          "receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission; control.main "
                      "end to end with fakes (the stopped-first-control path) and the final-gate rehearsal"],
    "t3_controls.py": [f"{ORD}, {C7X}: the required controls (mission section 5, steps 8-12) on synthetic and historical "
                       "evidence: sections A-D and F retargeted; E, G and H removed"],
}
ADDED = {}
NOT_CARRIED = {
    "c24.py": "C24 is Architect-accepted; no preserved-build admission exists in this mission",
    "c3w.py": "C3b and C3c are Architect-accepted; no build admission exception exists in this mission",
}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py", "t3.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "finish"],
              "expectations.py": ["need", "fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb", "original_cfg", "fn_text"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section4", "section5", "section7", "section8"],
              "build_packet.py": ["allowlists", "files_under", "whitespace", "authored", "prior_blobs", "selftest_block",
                                  "dup_policy"],
              "t3_controls.py": ["record", "base_raw", "transform", "first_two", "l1_line", "pass_report",
                                 "incomplete_lines", "compose", "report_props"],
              "rehearsals.py": ["record", "run_tool"],
              "final_gate.py": ["check"],
              "pre_exec_gate.py": ["check", "last"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "ACCEPTED", "MATRIX", "UNIT_PREFIX", "ACCEPTED_WT", "RULING",
                  "RULING_FIRST", "CAMPAIGN", "CAMPAIGN_FIRST", "RECORDS", "RECORD_FIRST", "PARENT", "PARENT_TREE",
                  "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"ACCEPTED_T3C", "BLOCKED", "BLOCKED_WT", "RULING_T3C", "RULING_T3C_FIRST", "CAMPAIGN_T3C",
                "CAMPAIGN_T3C_FIRST", "T3C", "T3C_TREE", "T3C_PARENT", "T3C_REF", "T3C_PACKET", "T3C_RECEIPTS",
                "T3C_ACCEPTED_EVIDENCE", "T3C_BLOCKED_EVIDENCE"}
# The authorized hunks of control.py, exactly: (function, old lines, new lines).
AUTH_HUNKS = [
    ('gate',
     ['        return [f"{cid} is not in this mission\'s order (the 23 accepted controls are never executed again; C1, C4 and "',
      '                f"C25 are reserved for the final witness-bound tranche)"], []'],
     ['        return [f"{cid} is not in this mission\'s order (the 27 accepted controls are never executed again; C7 is "',
      '                f"MUTATION_CONTROL_BLOCKED and excluded; C1, C4 and C25 are reserved for the final witness-bound "',
      '                f"tranche)"], []']),
    ('gate',
     ['        lines.append(f"the Architect ruling, build-ruling and amendment records, the campaign status record and the "',
      '                     f"expectations are in the freeze: {\'yes\' if not frozen else \'NO: \' + str(frozen)}")'],
     ['        lines.append(f"the Architect ruling record, the campaign status record and the expectations are in the "',
      '                     f"freeze: {\'yes\' if not frozen else \'NO: \' + str(frozen)}")']),
    ('gate',
     ['    if cid == "C24":',
      '        if not os.path.isdir(exp["controls"][cid]["worktree"]):',
      '            problems.append("the preserved C24 worktree is absent")',
      '    elif os.path.lexists(exp["controls"][cid]["worktree"]):'],
     ['    if os.path.lexists(exp["controls"][cid]["worktree"]):']),
    ('main',
     [],
     ['    if cid in K.BLOCKED:',
      '        raise SystemExit(f"REFUSED: {cid} is MUTATION_CONTROL_BLOCKED by the Architect and expressly excluded from "',
      '                         f"{K.MISSION}: no rerun, rebuild, patch alteration, property substitution or comparison")']),
    ('main',
     ['    if cid == "C24":',
      '        return main_c24()'],
     []),
    ('main',
     ['        return finish(outdir, ctl, RESULT_BLOCKED, [f"not started: {\'; \'.join(problems)}"])'],
     ['        return finish(outdir, ctl, RESULT_STOPPED, [f"not started: {\'; \'.join(problems)}"])']),
    ('main',
     ['    candidate, stop = False, None'],
     ['    candidate, stop, incomplete = False, None, False']),
    ('main',
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
      '                        if cid in c3w.CONTROLS else "the mutant build did not succeed (kills nothing; no rebuild)")'],
     ['            # TRANCHE3-TAIL-01 (mission section 7): BUILD: PINNED under the unchanged general classifier is the only',
      '            # admission (zero warnings, zero errors; no exception).',
      '            if first != "BUILD: PINNED":',
      '                stop = "the mutant build did not succeed (kills nothing; no rebuild)"']),
    ('main',
     ['                pin = json.load(open(pin_file))'],
     ['                pin = json.load(open(os.path.join(bdir, "PIN.json")))']),
    ('main',
     ['                                 "mutant-" + ctl["kind"], bprior,'],
     ['                                 "mutant-" + ctl["kind"], [(ctl["dir"] + "/03-build/CLASSIFICATION.txt", "BUILD: PINNED")],']),
    ('main',
     [],
     ['                incomplete = not candidate and cls[1:2] != [] and cls[1].startswith(INCOMPLETE_OUTER)']),
    ('main',
     ['                # TRANCHE3-EXEC-01 (mission section 5): C7 only, after an admissible F-PT/P6 candidate and before',
      '                # restoration: the informational run without A4 on F-3, on the same pinned executable; never a kill.',
      '                if candidate and ctl.get("informational"):',
      '                    info = ctl["informational"]',
      '                    idir = os.path.join(outdir, info["dir"])',
      '                    supervise.invoke(idir, f"SUPERVISED MUTANT RUN — control {cid}: {info[\'test\']} on the same pinned "',
      '                                           "mutant executable, WITHOUT A4 on F-3 (informational, mission section 5; not "',
      '                                           "the kill receipt)", pin["path"], pin["sha256"], pin["size"],',
      '                                     os.path.join(wt, K.CRATE), wt, info["test"], info["ignored"], info["argv_tail"],',
      '                                     info["limit_seconds"], source_ok, "mutant-informational",',
      '                                     [(ctl["dir"] + "/04a-rules/RULES.txt", t3.RULES_OK)],',
      '                                     {"control": cid, "configuration": info["configuration"],',
      '                                      "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"],',
      '                                      "informational": True, "required_to_kill": False})',
      '                    iok, ifirst = t3.classify_info(idir, info, exe, "EXACTLY_C_PLUS_PATCH", ctl["mutant_blob"])',
      '                    facts.append(f"informational run without A4 on F-3 (not the kill receipt): {ifirst}")',
      '                    if not iok:',
      '                        stop = ("the C7 informational run is not admissible (infrastructure, identity, OOM, signal, "',
      '                                "overshoot or malformed receipt): the mission stops")'],
     []),
    ('main',
     ['    return finish(outdir, ctl, RESULT_OK if not stop and candidate and rok else RESULT_BLOCKED, facts)'],
     ['    # TRANCHE3-TAIL-01 (mission section 12): an INCOMPLETE run (classify_a, unchanged) kills nothing and is reported as',
      '    # INCOMPLETE — ARCHITECT REVIEW REQUIRED; every other unsuccessful control as MUTATION_CONTROL_BLOCKED.',
      '    status = (RESULT_OK if not stop and candidate and rok else',
      '              RESULT_INCOMPLETE if incomplete and rok else RESULT_BLOCKED)',
      '    return finish(outdir, ctl, status, facts)']),
    ('link',
     ['    # TRANCHE3-CONTINUE-02 (mission sections 9 and 17): C24 (no patch applied here) links only after its own',
      '    # reauthentication receipt records REAUTH_OK.',
      '    own = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else "05-restore/RESTORE.txt")',
      '    want_own = REAUTH_OK if ctl["id"] == "C24" else RESTORE_OK',
      '    chk(f"this control\'s own {\'reauthentication\' if ctl[\'id\'] == \'C24\' else \'restoration\'} receipt exists and records "',
      '        f"{want_own} (no link before {\'reauthentication\' if ctl[\'id\'] == \'C24\' else \'restoration\'})",',
      '        supervise.first_line(own) == want_own, f"{os.path.relpath(own, K.EV)}: {supervise.first_line(own)!r}")'],
     ['    # TRANCHE3-TAIL-01 (mission section 6): every control here applied its patch, so it links only after its own',
      "    # restoration receipt records RESTORE: EXACTLY_C (C24's reauthentication binding removed: C24 is accepted).",
      '    own = os.path.join(os.path.dirname(outdir), "05-restore/RESTORE.txt")',
      '    chk("this control\'s own restoration receipt exists and records RESTORE: EXACTLY_C (no link before restoration)",',
      '        supervise.first_line(own) == RESTORE_OK, f"{os.path.relpath(own, K.EV)}: {supervise.first_line(own)!r}")']),
    ('link',
     ['    rest = os.path.join(os.path.dirname(outdir), "05-reauth/REAUTH.txt" if ctl["id"] == "C24" else "05-restore/RESTORE.txt")',
      '    rest_line = (f"after this control\'s own {\'reauthentication\' if ctl[\'id\'] == \'C24\' else \'restoration\'} receipt "'],
     ['    rest = os.path.join(os.path.dirname(outdir), "05-restore/RESTORE.txt")',
      '    rest_line = (f"after this control\'s own restoration receipt "']),
]
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


def run():
    t3csums = {}
    for line in K.git("show", f"{K.T3C}:{K.T3C_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t3csums[rel[6:]] = h
    carried = {}
    for line in K.git("show", f"{K.T3C}:{K.T3C_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions")[0].splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = K.sha256_bytes(K.git("show", f"{cells[2]}:{cells[1]}"))
            check(f"D1 carried tool {cells[0]}: its sha256 at {cells[2][:8]}:{cells[1]} is the one the continuation's "
                  "CARRIED-FORWARD.md records", carried[cells[0]] == cells[4], cells[4][:16])
    base = dict(carried, **t3csums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the continuation's execution tooling: its packet's tools/ "
          f"({len(t3csums)}; SHA256SUMS at {K.T3C[:8]}) and the tools it carried byte for byte ({len(carried)})",
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
          "warning diagnostic' still rejects every warning (no warning exception)",
          status.get("classify.py") == "UNCHANGED" and "ck(\"no error or warning diagnostic\", not errs, errs[:3])" in
          open(os.path.join(NEW, "classify.py")).read())
    t3text = open(os.path.join(NEW, "t3.py")).read()
    check("D3 t3.py is byte-identical and holds the L1 complete-search rule (expanded = discovered; no INCOMPLETE line; "
          "the explorer's own L1 detail; within the ceiling and the time limit)",
          status.get("t3.py") == "UNCHANGED" and all(x in t3text for x in (
              'rep["expanded"] == rep["states"]', 'rep["incomplete"] is None', "L1_DETAIL.match", 'rep["elapsed_s"] < limit')))
    pyf = [f for f in new if f.endswith(".py")]
    adm = sorted(f for f in pyf if any(isinstance(n, ast.FunctionDef) and n.name == "stderr_admission"
                                       for n in ast.walk(tree(os.path.join(NEW, f)))))
    check("D3 no warning-group admission (stderr_admission) is defined in any tool", not adm, adm)
    imp = sorted(f for f in pyf for n in ast.walk(tree(os.path.join(NEW, f))) if isinstance(n, (ast.Import, ast.ImportFrom))
                 and any(a.name in ("c24", "c3w", "c5") for a in n.names))
    use = sorted(f for f in pyf if any(c.startswith(("c24.", "c3w.", "c5.")) for c, _ in calls(tree(os.path.join(NEW, f)))))
    check("D3 no tool imports or calls c24, c3w or c5 (no build admission exception in this mission)", not imp and not use,
          (imp, use))
    # D4: control.py.
    tn = tree(os.path.join(NEW, "control.py"))
    to = tree(os.path.join(OLD, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main",
          [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main"])
    for name in ("gate", "main", "link"):
        a = src_lines(os.path.join(OLD, "control.py"), fn(to, name))
        b = src_lines(os.path.join(NEW, "control.py"), fn(tn, name))
        got = [(name, a[op[1]:op[2]], b[op[3]:op[4]]) for op in opcodes(a, b)]
        want = [h for h in AUTH_HUNKS if h[0] == name]
        check(f"D4 control.{name}: the continuation's with exactly the {len(want)} authorized hunk(s) (old and new lines "
              "exact; nothing else differs)", got == want, f"{len(got)} hunks")
    first = fn(tn, "main").body[0]
    check("D4 control.main's first statement refuses every blocked control (C7) by name, before reading or writing "
          "anything", isinstance(first, ast.If) and ast.unparse(first.test) == "cid in K.BLOCKED"
          and isinstance(first.body[0], ast.Raise) and "SystemExit" in ast.unparse(first.body[0]),
          ast.unparse(first.test))
    for f, names in SAME_FUNCS.items():
        fa, fb = funcs(tree(os.path.join(OLD, f))), funcs(tree(os.path.join(NEW, f)))
        diff = [n for n in names if fa.get(n) is None or fa.get(n) != fb.get(n)]
        check(f"D4 {f}: functions {names} AST-identical to the continuation's", not diff, diff)
    # D5: common.py.
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    changed = sorted(k for k in co if k in cn and co[k] != cn[k])
    check("D5 common.py: values changed only for the declared names", set(changed) == COMMON_CHANGED,
          sorted(set(changed) ^ COMMON_CHANGED))
    added = sorted(k for k in cn if k not in co)
    check("D5 common.py: names added exactly the declared set (the continuation's identities, C7 blocked, records)",
          set(added) == COMMON_ADDED, sorted(set(added) ^ COMMON_ADDED))
    removed = sorted(k for k in co if k not in cn)
    check("D5 common.py: no name removed", not removed, removed)
    check("D5 common.py: the continuation's records keep their exact texts under RULING_T3C and CAMPAIGN_T3C",
          (cn["RULING_T3C"], cn["RULING_T3C_FIRST"], cn["CAMPAIGN_T3C"], cn["CAMPAIGN_T3C_FIRST"])
          == (co["RULING"], co["RULING_FIRST"], co["CAMPAIGN"], co["CAMPAIGN_FIRST"]))
    check("D5 common.py: the 27 accepted are the 23 with C24 C3b C3c C6 appended; C7 alone is blocked; the order is the "
          "continuation's last four controls and the matrix their unchanged rows",
          cn["ACCEPTED"] == co["ACCEPTED"] + ["C24", "C3b", "C3c", "C6"] and cn["BLOCKED"] == ["C7"]
          and cn["ORDER"] == co["ORDER"][5:] == ["A1", "A2", "C20", "A5"]
          and cn["MATRIX"] == {c: co["MATRIX"][c] for c in cn["ORDER"]}
          and len(set(cn["ORDER"] + cn["ACCEPTED"] + cn["BLOCKED"] + cn["NOT_AUTHORIZED"])) == 35)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mt3t); no preserved worktree",
          cn["UNIT_PREFIX"] == "p3c11-mt3t" and all(K.control_wt(c) == K.NEXUS + "/nexus-os-p3-c11-mt3t-" + c
                                                    for c in K.ORDER + K.BLOCKED + ["C24"]))
    # D6: no changed tool builds or runs anything itself.
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
                        and builds.count("supervise.invoke") == 1 == in_main.count("supervise.invoke"))
        else:
            ok_calls = not builds
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build and supervise.invoke once each, in main" if f == "control.py" else
                 "; no supervise.build/invoke"), not bad and not lists and ok_calls, f"{bad} {lists} {builds}")
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
            label = f"TRANCHE3-CONTINUE-02/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"TRANCHE3-TAIL-01/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the Tranche-3 continuation's execution tooling (`original-tools/`: `{K.T3C_PACKET}/tools/` at `{K.T3C}` and "
          "the tools it carried byte for byte, at the commits its CARRIED-FORWARD.md names). New: this mission's "
          "`tools/`. The complete old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. "
          "The retarget is the minimal one for the four controls A1 A2 C20 A5 (mission section 5, step 6): ordering and "
          "publication bindings, C7's explicit refusal, the section-12 result labels and the records; what the four "
          "controls do not use is removed (C24's preserved-build flow, the C3b/C3c admission, C7's informational step).",
          "", "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte"]))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Byte-identical: `classify.py` (the general build classifier `classify_build_success`, which rejects every "
           "warning, and the run classifier `classify_a`), `t3.py` (the original-explorer and permitted-property rules "
           "and the L1 complete-search validation), `supervise.py`, `worktrees.py`, `verify_mutant.py`, `verify_tree.py`, "
           "`scope-wrap.sh`, `report.py`, `configs.py`, `push.py`. No warning admission exists in this mission: `c24.py` "
           "and `c3w.py` are not carried and nothing imports or calls them.", ""]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
