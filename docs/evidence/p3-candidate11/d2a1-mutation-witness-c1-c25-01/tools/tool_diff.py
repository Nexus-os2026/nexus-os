#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (section 3, steps 4-5;
section 7): old = the Tranche-3 tail's execution tooling (original-tools/: its packet's tools/ at e41c84b1 and the tools
it carried byte for byte); new = this mission's tools/; the authorization map below is the minimal adaptation for
AD-1-R4 G.3/F.1/F.2 (C1, C25): one added module (g3.py: the G.3(a) primary classifier, the type-(c) mutant-witness
classifier, the type-(e3) healthy link, the G.3 obligations), control.main's G.3 flow (the primary classified by g3, the
mutant witness on the same pinned executable, the two links after restoration, the G.3 record), C4's refusal by name,
and the ordering, freeze, gate and publication bindings; classify.py, t3.py, supervise.py and every other carried tool
byte-identical; control.link and its helpers AST-identical. The checks D1-D6 below are this mission's (the history
above them is unchanged text). Revised by -CONTINUE-02, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01,
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
ORD = "item: ordering (C1 then C25)"
C7X = "item: C4 and C7 excluded (reserved; MUTATION_CONTROL_BLOCKED)"
RES = "item: section-10 result labels"
REC = "item: Architect acceptance/status records"
PUB = "item: publication bindings"
FRZ = "item: freeze/gate bindings"
G3X = "item: AD-1-R4 G.3/F.1/F.2 (the primary, the mutant witness, the healthy witness, the obligations)"
CHANGED = {
    "common.py": [f"{ORD}, {PUB}, {FRZ}: mission identity, evidence root, worktree namespace (mtw), order C1 C25 "
                  "(receipt directories 81-82), the 31 accepted controls (A1 A2 C20 A5 added), C4 the one reserved "
                  f"control, the two-control matrix, the tail's identities with its accepted controls' evidence; {G3X}: "
                  f"G3 (the mission's statement of each control's G.3 bindings) and WITNESS_LIMIT; {REC}: the two "
                  "records (the tail's own two kept under RULING_T3T and CAMPAIGN_T3T)"],
    "control.py": [f"{ORD}: gate() over this order; {C7X}: main() refuses C7 and C4 by name first; {G3X}: main() "
                   "classifies the primary by g3.classify_primary, runs the mutant witness on the same pinned executable "
                   "(one more supervise.invoke) classified by g3.classify_witness, links the healthy (e3) and the "
                   "unchanged-core (e1) receipts only after restoration, writes the G.3 record; {RES}: INCOMPLETE when "
                   "the primary or the witness is INCOMPLETE; link() unchanged"],
    "expectations.py": [f"{ORD}: the two controls in this order; {C7X}: no expectation for C4 or C7; {G3X}: g3_block and "
                        "witness_const (from the catalog, AD-1-R4 and C's source; cross-checked with common.G3)"],
    "authenticate.py": [f"{FRZ}: every input of this mission; section [11] without its tooling check; the new sections "
                        "[12] (the tail evidence, its four accepted controls and the base tooling check) and [13] (the "
                        "witness tests and their reviewed source, AD-1-R4's clauses, the healthy receipts)"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records (the two records; the tail's identities; the G.3 "
                         "blocks and the healthy receipts; this mission's policy)"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before C1 (E1-E11 for this mission; C4 and C7 refused; "
                         "the G.3 blocks)"],
    "final_gate.py": [f"{FRZ}: the final gate (one G.3 receipt chain per control; the section-10 labels; the witness on "
                      "the primary's executable; this mission's worktrees; no C4 or C7 receipt)"],
    "render.py": [f"{PUB}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md (the primary, the witness, the healthy and "
                  "unchanged-core links, the G.3 record; the tail's evidence) and the derived status"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent; the mutant witness's sampler record enumerated and "
                        "bound like the primary's; the rendering directory's name"],
    "publish_checks.py": [f"{PUB}: parent, every previous packet unchanged (the tail's among them), the healthy and F-1 "
                          "receipts unchanged, refs, the G.3 receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission; control.main "
                      "end to end with fakes (the stopped-first-control path) and the final-gate rehearsal"],
    "t3_controls.py": [f"{ORD}, {C7X}, {G3X}: the required positive and negative controls (mission section 7) on "
                       "synthetic, historical and authenticated evidence: sections A, W, H, B, G, C, D, F"],
}
ADDED = {"g3.py": [f"{G3X}: classify_primary, classify_witness, link_healthy, obligations; builds and runs nothing"]}
NOT_CARRIED = {}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py", "t3.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "link", "finish"],
              "expectations.py": ["need", "fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb", "original_cfg", "fn_text"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section4", "section5", "section7", "section8", "section9",
                                  "section10"],
              "build_packet.py": ["files_under", "whitespace", "authored", "prior_blobs", "selftest_block", "dup_policy"],
              "t3_controls.py": ["record", "base_raw", "transform", "synth", "first_two", "l1_line", "pass_report",
                                 "incomplete_lines", "compose", "sub_all"],
              "rehearsals.py": ["record", "run_tool", "synthetic_freeze"],
              "record_freeze.py": ["main"],
              "final_gate.py": ["check"],
              "pre_exec_gate.py": ["check", "last"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "ACCEPTED", "NOT_AUTHORIZED", "MATRIX", "UNIT_PREFIX",
                  "ACCEPTED_WT", "RULING", "RULING_FIRST", "CAMPAIGN", "CAMPAIGN_FIRST", "RECORDS", "RECORD_FIRST",
                  "PARENT", "PARENT_TREE", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"ACCEPTED_T3T", "G3", "WITNESS_LIMIT", "RULING_T3T", "RULING_T3T_FIRST", "CAMPAIGN_T3T",
                "CAMPAIGN_T3T_FIRST", "T3T", "T3T_TREE", "T3T_PARENT", "T3T_REF", "T3T_PACKET", "T3T_RECEIPTS",
                "T3T_ACCEPTED_EVIDENCE"}
# The authorized hunks of control.py, exactly: (function, old lines, new lines).
AUTH_HUNKS = [
    ('gate',
     ['        return [f"{cid} is not in this mission\'s order (the 27 accepted controls are never executed again; C7 is "',
      '                f"MUTATION_CONTROL_BLOCKED and excluded; C1, C4 and C25 are reserved for the final witness-bound "',
      '                f"tranche)"], []'],
     ['        return [f"{cid} is not in this mission\'s order (the 31 accepted controls are never executed again; C7 is "',
      '                f"MUTATION_CONTROL_BLOCKED and excluded; C4 is reserved)"], []']),
    ('main',
     [],
     ['    if cid in K.NOT_AUTHORIZED:',
      '        raise SystemExit(f"REFUSED: {cid} is reserved and not authorized by {K.MISSION}: never executed, built or "',
      '                         "started here")']),
    ('main',
     ['                         f"(type ({\'a\' if ctl[\'kind\'] == \'explore\' else \'b\'}))")'],
     ['                         "(type (a), the AD-1-R4 G.3(a) primary)")']),
    ('main',
     ['                (classify.classify_a if ctl["kind"] == "explore" else classify.classify_b)(rdir, ctl, exe)'],
     ['                # WITNESS-C1-C25-01 (mission sections 4, 5; AD-1-R4 G.3(a)): the primary, recorded as printed.',
      '                g3.classify_primary(rdir, ctl, exe)']),
    ('main',
     ['                candidate = cls[0] == classify.CANDIDATE',
      '                incomplete = not candidate and cls[1:2] != [] and cls[1].startswith(INCOMPLETE_OUTER)',
      '                facts.append(f"run (type ({\'a\' if ctl[\'kind\'] == \'explore\' else \'b\'})): {cls[0]}; {cls[1]}")',
      '                # TRANCHE3-EXEC-01: the tranche-3 rules (t3.rules) must hold too; they only add requirements.',
      '                rules_ok = t3.rules(os.path.join(outdir, "04a-rules"), rdir, ctl)',
      '                facts.append(f"tranche-3 rules: {supervise.first_line(os.path.join(outdir, \'04a-rules/RULES.txt\'))}")',
      '                candidate = candidate and rules_ok',
      '                if not candidate:',
      '                    stop = "the mutant run is not admissible kill evidence of the named property or test"'],
     ['                primary_ok = cls[0] == g3.PRIMARY_OK',
      '                incomplete = not primary_ok and cls[1:2] != [] and cls[1].startswith(INCOMPLETE_OUTER)',
      '                facts.append(f"primary (type (a), G.3(a)): {cls[0]}; {cls[1]}")',
      '                if not primary_ok:',
      '                    stop = ("the primary is not an admissible type-(a) FAIL of the predicted first failure "',
      '                            f"{ctl[\'g3\'][\'predicted\']} (another first property requires an Architect ruling)")',
      '                else:',
      '                    # AD-1-R4 G.3(b), F.2: the mutant witness on the SAME pinned mutant executable.',
      '                    W = ctl["g3"]["witness"]',
      '                    wdir = os.path.join(outdir, "04c-witness")',
      '                    supervise.invoke(wdir, f"SUPERVISED MUTANT RUN — control {cid}: {W[\'test\']} on the same pinned "',
      '                                           "mutant executable (type (c), the AD-1-R4 G.3(b) mutant witness)",',
      '                                     pin["path"], pin["sha256"], pin["size"], os.path.join(wt, K.CRATE), wt, W["test"],',
      '                                     W["ignored"], W["argv_tail"], W["limit_seconds"], source_ok, "mutant-witness",',
      '                                     [(ctl["dir"] + "/04-run/CLASSIFICATION.txt", g3.PRIMARY_OK)],',
      '                                     {"control": cid, "configuration": "F-1 (the witness\'s fixed schedule)",',
      '                                      "role": "mutant witness (type (c))", "mutant_blob": ctl["mutant_blob"],',
      '                                      "patch_sha256": ctl["patch"]["sha256"], "primary_receipt": ctl["dir"] + "/04-run"})',
      '                    g3.classify_witness(wdir, ctl, exe, rdir)',
      '                    wcls = open(os.path.join(wdir, "CLASSIFICATION.txt")).read().splitlines()',
      '                    candidate = wcls[0] == g3.WITNESS_OK',
      '                    incomplete = not candidate and wcls[1:2] != [] and wcls[1].startswith(INCOMPLETE_OUTER)',
      '                    facts.append(f"mutant witness (type (c), G.3(b)): {wcls[0]}; {wcls[1]}")',
      '                    if not candidate:',
      '                        stop = "the mutant witness is not an admissible type-(c) receipt of the named property"']),
    ('main',
     [],
     ['        # AD-1-R4 G.3(d) and I (mission sections 4, 5, 8): only after RESTORE: EXACTLY_C, the accepted healthy',
      '        # type-(e3) witness, then the accepted F-1 unchanged-core type-(e1) receipt; both linked, never rerun.',
      '        hok, href = g3.link_healthy(ctl, os.path.join(outdir, "06a-healthy-witness"))',
      '        facts.append(f"linked healthy witness (type (e3), G.3(d)): "',
      '                     f"{supervise.first_line(os.path.join(outdir, \'06a-healthy-witness/HEALTHY.txt\'))}; {href}")']),
    ('main',
     ['        lok, ref = link(ctl, exp, os.path.join(outdir, "06-link"), bdesc)',
      '        facts.append(f"linked restored-side receipt: {supervise.first_line(os.path.join(outdir, \'06-link/LINK.txt\'))}; "',
      '                     f"{ref}")',
      '        if not lok:',
      '            stop = "the restored-side receipt could not be linked"'],
     ['        lok, ref = link(ctl, exp, os.path.join(outdir, "06b-link"), bdesc)',
      '        facts.append(f"linked restored-side receipt (type (e1)): "',
      '                     f"{supervise.first_line(os.path.join(outdir, \'06b-link/LINK.txt\'))}; {ref}")',
      '        gok = g3.obligations(ctl, os.path.join(outdir, "07-g3"))',
      '        facts.append(f"G.3: {supervise.first_line(os.path.join(outdir, \'07-g3/G3.txt\'))}")',
      '        if gok:',
      '            facts.append(f"G.3(e) recording, proposed for Architect review (not an acceptance): "',
      '                         f"\'{ctl[\'g3\'][\'recording\']}\'")',
      '        if not (hok and lok and gok):',
      '            stop = "the healthy witness or the restored-side receipt could not be linked, or G.3 does not hold"']),
    ('main',
     ['    # TRANCHE3-TAIL-01 (mission section 12): an INCOMPLETE run (classify_a, unchanged) kills nothing and is reported as'],
     ['    # WITNESS-C1-C25-01 (mission section 10): an INCOMPLETE primary or witness kills nothing and is reported as']),
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
    for line in K.git("show", f"{K.T3T}:{K.T3T_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t3csums[rel[6:]] = h
    carried = {}
    for line in K.git("show", f"{K.T3T}:{K.T3T_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions")[0].splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = K.sha256_bytes(K.git("show", f"{cells[2]}:{cells[1]}"))
            check(f"D1 carried tool {cells[0]}: its sha256 at {cells[2][:8]}:{cells[1]} is the one the tail's "
                  "CARRIED-FORWARD.md records", carried[cells[0]] == cells[4], cells[4][:16])
    base = dict(carried, **t3csums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the tail's execution tooling: its packet's tools/ "
          f"({len(t3csums)}; SHA256SUMS at {K.T3T[:8]}) and the tools it carried byte for byte ({len(carried)})",
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
    check("D3 classify.classify_a and t3.rules are byte-identical and no changed or added tool calls either on a G.3 "
          "control's run (the G.3(a) attribution is g3.classify_primary's; classify_a is called only by t3_controls.py "
          "beside it, as a negative control)",
          status.get("classify.py") == status.get("t3.py") == "UNCHANGED"
          and not [f for f in pyf if f != "t3_controls.py" and any(c in ("classify.classify_a", "t3.rules")
                                                                  for c, _ in calls(tree(os.path.join(NEW, f))))])
    # D4: control.py.
    tn = tree(os.path.join(NEW, "control.py"))
    to = tree(os.path.join(OLD, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, finish, main",
          [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == ["git", "gate", "apply_patch", "restored_check",
                                                                         "restore", "link", "finish", "main"])
    for name in ("gate", "main"):
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
    second = fn(tn, "main").body[1]
    check("D4 control.main's second statement refuses every reserved control (C4) by name, before reading or writing "
          "anything", isinstance(second, ast.If) and ast.unparse(second.test) == "cid in K.NOT_AUTHORIZED"
          and isinstance(second.body[0], ast.Raise) and "SystemExit" in ast.unparse(second.body[0]),
          ast.unparse(second.test))
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
    check("D5 common.py: the tail's records keep their exact texts under RULING_T3T and CAMPAIGN_T3T",
          (cn["RULING_T3T"], cn["RULING_T3T_FIRST"], cn["CAMPAIGN_T3T"], cn["CAMPAIGN_T3T_FIRST"])
          == (co["RULING"], co["RULING_FIRST"], co["CAMPAIGN"], co["CAMPAIGN_FIRST"]))
    check("D5 common.py: the 31 accepted are the 27 with A1 A2 C20 A5 (the tail's order) appended; C7 alone is blocked; "
          "C4 alone is reserved; the order is the tail's other reserved controls, C1 then C25",
          cn["ACCEPTED"] == co["ACCEPTED"] + co["ORDER"] == co["ACCEPTED"] + ["A1", "A2", "C20", "A5"]
          and cn["BLOCKED"] == co["BLOCKED"] == ["C7"] and cn["NOT_AUTHORIZED"] == ["C4"]
          and cn["ORDER"] == [c for c in co["NOT_AUTHORIZED"] if c != "C4"] == ["C1", "C25"]
          and sorted(cn["MATRIX"]) == ["C1", "C25"] and sorted(cn["G3"]) == ["C1", "C25"]
          and len(set(cn["ORDER"] + cn["ACCEPTED"] + cn["BLOCKED"] + cn["NOT_AUTHORIZED"])) == 35)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mtw); no preserved worktree",
          cn["UNIT_PREFIX"] == "p3c11-mtw" and all(K.control_wt(c) == K.NEXUS + "/nexus-os-p3-c11-mtw-" + c
                                                   for c in K.ORDER + K.BLOCKED + K.NOT_AUTHORIZED))
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
                        and builds.count("supervise.invoke") == 2 == in_main.count("supervise.invoke"))
        else:
            ok_calls = not builds
        check(f"D6 {f}: launches only git/gh/ps/python; no cargo or rustc argument list"
              + ("; supervise.build once and supervise.invoke twice (the primary and the mutant witness), in main"
                 if f == "control.py" else "; no supervise.build/invoke"), not bad and not lists and ok_calls,
              f"{bad} {lists} {builds}")
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
            label = f"TRANCHE3-TAIL-01/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"WITNESS-C1-C25-01/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the Tranche-3 tail's execution tooling (`original-tools/`: `{K.T3T_PACKET}/tools/` at `{K.T3T}` and "
          "the tools it carried byte for byte, at the commits its CARRIED-FORWARD.md names). New: this mission's "
          "`tools/`. The complete old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. "
          "The adaptation is the minimal one for AD-1-R4 G.3/F.1/F.2 (C1, C25; mission section 3, step 5): one added "
          "module (`g3.py`), control.main's G.3 flow, C4's refusal by name, the ordering, freeze, gate and publication "
          "bindings and the records.",
          "", "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f) or ADDED.get(f) or ([f"not carried: {NOT_CARRIED[f]}"] if f in NOT_CARRIED else
                                                          ["carried byte for byte"]))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Byte-identical: `classify.py` (the general build classifier `classify_build_success`, which rejects every "
           "warning, and the run classifier `classify_a`), `t3.py`, `supervise.py`, `worktrees.py`, `verify_mutant.py`, "
           "`verify_tree.py`, `scope-wrap.sh`, `report.py`, `configs.py`, `push.py`. AST-identical: `control.link` and the "
           "helpers named in SAME_FUNCS. No warning admission exists in this mission.", ""]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
