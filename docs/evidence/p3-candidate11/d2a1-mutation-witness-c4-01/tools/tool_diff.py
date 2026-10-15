#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Revised by P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01 (section 5, steps 6-8): old
= the C1/C25 mission's execution tooling (original-tools/: its packet's tools/ at 74b3f485 and the tools it carried byte
for byte); new = this mission's tools/; the authorization map below is the minimal adaptation for AD-1-R4 G.5/G.6/F.1/F.2
(C4): one added module (g5.py: the G.5(A) primary classifier, the type-(c) mutant-witness classifier with the G.6
transcript parser and checker, the type-(e3) healthy link with G.6 on it and against the mutant, the G.5 obligations),
g3.py not carried (C1 and C25 are accepted), control.main's G.5 flow (g5 in place of g3 at each step; the
post-restoration authentication, restore_auth, added), the ordering, freeze, gate and publication bindings and the
records; classify.py, t3.py, supervise.py and every other carried tool byte-identical; control.link and its helpers
AST-identical. The checks D1-D6 below are this mission's (the history after them is unchanged text). Revised by
P3-C11-DR01-D2A1-MUTATION-WITNESS-C1-C25-01 (section 3, steps 4-5;
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

  D1  original-tools/ is exactly the C1/C25 mission's execution tooling (names and sha256: its packet's SHA256SUMS for
      its tools/, and the commits and paths its CARRIED-FORWARD.md names for the tools it carried byte for byte)
  D2  every tool is UNCHANGED, CHANGED, ADDED or NOT CARRIED exactly as the authorization map declares
  D3  byte-identical: classify.py (the general build classifier classify_build_success, which rejects every warning, and
      the run classifier classify_a), t3.py (the original-explorer, permitted-property and L1 complete-search rules),
      supervise.py, worktrees.py, verify_mutant.py, verify_tree.py, scope-wrap.sh, report.py, configs.py and push.py; no
      warning-admission code anywhere (no stderr_admission, no c24 or c3w module imported or called by any tool)
  D4  control.py defines exactly git, gate, apply_patch, restored_check, restore, link, restore_auth, finish and main;
      git, apply_patch, restored_check, restore, link and finish AST-identical to the C1/C25 mission's; gate() and
      main() differ from it by exactly the authorized hunks listed below (nothing else); restore_auth is added and only
      reads; main()'s first statement refuses C7 by name; the helpers of expectations.py, authenticate.py,
      build_packet.py, t3_controls.py, rehearsals.py, final_gate.py and pre_exec_gate.py named in SAME_FUNCS
      AST-identical to the C1/C25 mission's
  D5  common.py: every name the reused tools import keeps its accepted value and its helper functions but control_wt are
      AST-identical; changed and added names are only the declared ones; nothing is removed; the C1/C25 mission's records
      keep their texts under RULING_W1 and CAMPAIGN_W1; the order is the C1/C25 mission's reserved control, C4; the 33
      accepted are the 31 with C1 C25; C7 alone is blocked; nothing is reserved
  D6  no changed tool builds or runs anything itself: no literal cargo or rustc argument list outside
      common.BUILD_ARGV (unchanged), every process launch is git, gh, ps or this Python interpreter; supervise.build and
      supervise.invoke are called once and twice, by control.main (the build; the primary and the mutant witness)
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
ORD = "item: ordering (C4 alone)"
C7X = "item: C7 excluded (MUTATION_CONTROL_BLOCKED) and every accepted control refused"
RES = "item: section-15 result labels"
REC = "item: Architect acceptance/status records"
PUB = "item: publication bindings"
FRZ = "item: freeze/gate bindings"
G5X = ("item: AD-1-R4 G.5/G.6/F.1/F.2 (the primary, the mutant witness and G.6, the healthy witness, the obligations, "
       "the post-restoration authentication)")
CHANGED = {
    "common.py": [f"{ORD}, {PUB}, {FRZ}: mission identity, evidence root, worktree namespace (mwc4), order C4 (receipt "
                  "directory 81), the 33 accepted controls (C1 C25 added), nothing reserved, the one-control matrix, "
                  f"the C1/C25 evidence identities; {G5X}: G5 (the mission's statement of C4's G.5/G.6 bindings); "
                  f"{REC}: the two records (the C1/C25 mission's own two kept under RULING_W1 and CAMPAIGN_W1)"],
    "control.py": [f"{ORD}: gate() over this order; {C7X}: main() refuses C7 by name first; {G5X}: main() classifies "
                   "the primary by g5.classify_primary, runs the mutant witness on the same pinned executable (one more "
                   "supervise.invoke) classified by g5.classify_witness, authenticates the restoration (restore_auth, "
                   "added: read-only), links the healthy (e3) and the unchanged-core (e1) receipts only after "
                   f"restoration, writes the G.5 record; {RES}: INCOMPLETE when the primary or the witness is "
                   "INCOMPLETE; link() unchanged"],
    "expectations.py": [f"{ORD}: C4 alone; {C7X}: no expectation for C7 or an accepted control; {G5X}: g5_block and its "
                        "parsers (from the catalog, AD-1-R4 and C's source; cross-checked with common.G5); C4's two "
                        "catalog fields read under its special rule"],
    "authenticate.py": [f"{FRZ}: every input of this mission; [3] C4's G.5 catalog fields; [6] the mwc4 namespace and "
                        "the C1/C25 ref; [12] without its parent and tooling checks; [13] replaced by the C1/C25 evidence "
                        "(its two accepted controls, the base tooling check, the parent); [14] the C4 witness source at "
                        "A and C, AD-1-R4's clauses, the historical prototype receipts' identities, the healthy receipt"],
    "record_freeze.py": [f"{FRZ}: what the freeze covers and records (the two records; the C1/C25 identities; the G.5 "
                         "block and the healthy receipt; this mission's policy)"],
    "pre_exec_gate.py": [f"{FRZ}: the static pre-execution gate before C4 (E1-E11 for this mission; C7, C1 and C25 "
                         "refused; the G.5 block)"],
    "final_gate.py": [f"{FRZ}: the final gate (C4's G.5 receipt chain, its restore-auth record; the section-15 labels; "
                      "the witness on the primary's executable; this mission's worktree; no C7 or accepted-control "
                      "receipt)"],
    "render.py": [f"{PUB}: INDEX.md, TRANCHE-SUMMARY.md, CARRIED-FORWARD.md (the primary, the witness and G.6, the "
                  "restore-auth, the healthy and unchanged-core links, the G.5 record; the C1/C25 evidence; both witness "
                  "sources) and the derived status"],
    "build_packet.py": [f"{PUB}: the previous packets at this parent; C4's mutant witness's sampler record enumerated and "
                        "bound like the primary's; the rendering directory's name"],
    "publish_checks.py": [f"{PUB}: parent, every previous packet unchanged (the C1/C25 packet among them), the healthy and "
                          "F-8 receipts unchanged, refs, C4's G.5 receipts read back"],
    "worktree.py": [f"{PUB}: the evidence worktree at this mission's parent"],
    "finalize.py": [f"{PUB}: the parent and the commit message"],
    "tool_diff.py": [f"{FRZ}: this diff and its check"],
    "rehearsals.py": [f"{PUB}: render.py and build_packet.py rehearsed on synthetic roots of this mission; control.main "
                      "end to end with fakes (the stopped-control paths) and the final-gate rehearsal"],
    "t3_controls.py": [f"{ORD}, {C7X}, {G5X}: the required positive and negative controls (mission section 11) on "
                       "synthetic, historical and authenticated evidence: sections A, W, X, H, B, R, G, C, D, F"],
}
ADDED = {"g5.py": [f"{G5X}: classify_primary, transcript, g6_check, classify_witness, link_healthy, obligations; builds "
                   "and runs nothing"]}
NOT_CARRIED = {"g3.py": "AD-1-R4 G.3 applies to C1 and C25 only; both are Architect-accepted (mission section 2)"}
CARRIED_BYTE_FOR_BYTE = ["classify.py", "configs.py", "push.py", "report.py", "scope-wrap.sh", "supervise.py", "t3.py",
                         "verify_mutant.py", "verify_tree.py", "worktrees.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "link", "finish"],
              "expectations.py": ["need", "fn_span", "attrs_above", "verdict_assertion", "mutant_bytes", "key_fields",
                                  "model_constants", "config_block", "original_fb", "original_cfg", "fn_text",
                                  "witness_const", "g3_block"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "packet_files",
                                  "stage_receipt", "cat_src", "section4", "section5", "section7", "section8", "section9",
                                  "section10", "section11"],
              "build_packet.py": ["files_under", "whitespace", "authored", "prior_blobs", "selftest_block", "dup_policy"],
              "t3_controls.py": ["record", "exe_for", "base_raw", "transform", "synth", "first_two", "l1_line",
                                 "pass_report", "incomplete_lines", "compose", "result_label", "sub_all"],
              "rehearsals.py": ["record", "pick_dir", "run_tool", "synthetic_freeze", "supervise_first"],
              "record_freeze.py": ["main"],
              "final_gate.py": ["check"],
              "pre_exec_gate.py": ["check", "last"],
              "common.py": ["build_env", "git", "blob", "source", "sha256_bytes", "sha256_file", "git_blob_id",
                            "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "ORDER", "ACCEPTED", "NOT_AUTHORIZED", "MATRIX", "UNIT_PREFIX",
                  "ACCEPTED_WT", "RULING", "RULING_FIRST", "CAMPAIGN", "CAMPAIGN_FIRST", "RECORDS", "RECORD_FIRST",
                  "PARENT", "PARENT_TREE", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"ACCEPTED_W1", "G5", "RULING_W1", "RULING_W1_FIRST", "CAMPAIGN_W1", "CAMPAIGN_W1_FIRST", "W1", "W1_TREE",
                "W1_PARENT", "W1_REF", "W1_PACKET", "W1_RECEIPTS", "W1_ACCEPTED_EVIDENCE"}
# The authorized hunks of control.py, exactly: (function, old lines, new lines). WITNESS-C4-01: extracted from the old
# and new control.gate and control.main, each reviewed against the map above, and pinned here literally.
AUTH_HUNKS = [
    ('gate',
     ['        return [f"{cid} is not in this mission\'s order (the 31 accepted controls are never executed again; C7 is "',
      '                f"MUTATION_CONTROL_BLOCKED and excluded; C4 is reserved)"], []'],
     ['        return [f"{cid} is not in this mission\'s order (the 33 accepted controls are never executed again; C7 is "',
      '                f"MUTATION_CONTROL_BLOCKED and excluded)"], []']),
    ('main',
     ['                         "(type (a), the AD-1-R4 G.3(a) primary)")'],
     ['                         "(type (a), the AD-1-R4 G.5(A) primary)")']),
    ('main',
     ['                # WITNESS-C1-C25-01 (mission sections 4, 5; AD-1-R4 G.3(a)): the primary, recorded as printed.',
      '                g3.classify_primary(rdir, ctl, exe)'],
     ['                # WITNESS-C4-01 (mission section 6; AD-1-R4 G.5(A)): the primary, recorded as printed.',
      '                g5.classify_primary(rdir, ctl, exe)']),
    ('main',
     ['                primary_ok = cls[0] == g3.PRIMARY_OK'],
     ['                primary_ok = cls[0] == g5.PRIMARY_OK']),
    ('main',
     ['                facts.append(f"primary (type (a), G.3(a)): {cls[0]}; {cls[1]}")'],
     ['                facts.append(f"primary (type (a), G.5(A)): {cls[0]}; {cls[1]}")']),
    ('main',
     ['                    stop = ("the primary is not an admissible type-(a) FAIL of the predicted first failure "',
      '                            f"{ctl[\'g3\'][\'predicted\']} (another first property requires an Architect ruling)")'],
     ['                    stop = ("the primary is not an admissible type-(a) FAIL whose first printed property is P4 or P8 "',
      '                            "(any other first property stops for an Architect ruling; PASS, INCOMPLETE or a FAIL beyond "',
      '                            "the ceiling kills nothing)")']),
    ('main',
     ['                    # AD-1-R4 G.3(b), F.2: the mutant witness on the SAME pinned mutant executable.',
      '                    W = ctl["g3"]["witness"]'],
     ['                    # AD-1-R4 G.5(B), G.6, F.2 (mission section 7): the mutant witness on the SAME pinned executable.',
      '                    W = ctl["g5"]["witness"]']),
    ('main',
     ['                                           "mutant executable (type (c), the AD-1-R4 G.3(b) mutant witness)",'],
     ['                                           "mutant executable (type (c), the AD-1-R4 G.5(B) mutant witness)",']),
    ('main',
     ['                                     [(ctl["dir"] + "/04-run/CLASSIFICATION.txt", g3.PRIMARY_OK)],',
      '                                     {"control": cid, "configuration": "F-1 (the witness\'s fixed schedule)",'],
     ['                                     [(ctl["dir"] + "/04-run/CLASSIFICATION.txt", g5.PRIMARY_OK)],',
      '                                     {"control": cid, "configuration": "F-8 (the witness\'s two fixed scenarios from "',
      '                                                                       "initial(F8))",']),
    ('main',
     ['                    g3.classify_witness(wdir, ctl, exe, rdir)'],
     ['                    g5.classify_witness(wdir, ctl, exe, rdir)']),
    ('main',
     ['                    candidate = wcls[0] == g3.WITNESS_OK'],
     ['                    candidate = wcls[0] == g5.WITNESS_OK']),
    ('main',
     ['                    facts.append(f"mutant witness (type (c), G.3(b)): {wcls[0]}; {wcls[1]}")'],
     ['                    facts.append(f"mutant witness (type (c), G.5(B), G.6): {wcls[0]}; {wcls[1]}")']),
    ('main',
     ['                        stop = "the mutant witness is not an admissible type-(c) receipt of the named property"'],
     ['                        stop = ("the mutant witness is not an admissible type-(c) receipt of BOTH P4 and P8 at their "',
      '                                "frozen loci under G.6")']),
    ('main',
     [],
     ['    elif applied and ctl["kind"] != "build" and os.path.exists(os.path.join(outdir, "03-build/PIN.json")):',
      '        # WITNESS-C4-01 (mission section 12): the post-restoration authentication.',
      '        aok = restore_auth(wt, ctl, os.path.join(outdir, "05b-restore-auth"),',
      '                           json.load(open(os.path.join(outdir, "03-build/PIN.json"))))',
      '        facts.append(f"post-restoration authentication: "',
      '                     f"{supervise.first_line(os.path.join(outdir, \'05b-restore-auth/RESTORE-AUTH.txt\'))}")',
      '        if not aok:',
      '            rok = False',
      '            stop = (stop + "; " if stop else "") + "the post-restoration authentication failed"']),
    ('main',
     ['        # AD-1-R4 G.3(d) and I (mission sections 4, 5, 8): only after RESTORE: EXACTLY_C, the accepted healthy',
      '        # type-(e3) witness, then the accepted F-1 unchanged-core type-(e1) receipt; both linked, never rerun.',
      '        hok, href = g3.link_healthy(ctl, os.path.join(outdir, "06a-healthy-witness"))',
      '        facts.append(f"linked healthy witness (type (e3), G.3(d)): "'],
     ['        # AD-1-R4 G.5(C), (D) and I (mission sections 9 and 12): only after RESTORE: EXACTLY_C, the accepted healthy',
      '        # type-(e3) witness, then the accepted F-8 unchanged-core type-(e1) receipt; both linked, never rerun.',
      '        hok, href = g5.link_healthy(ctl, os.path.join(outdir, "06a-healthy-witness"))',
      '        facts.append(f"linked healthy witness (type (e3), G.5(C), G.6): "']),
    ('main',
     ['        facts.append(f"linked restored-side receipt (type (e1)): "'],
     ['        facts.append(f"linked restored-side receipt (type (e1), G.5(D)): "']),
    ('main',
     ['        gok = g3.obligations(ctl, os.path.join(outdir, "07-g3"))',
      '        facts.append(f"G.3: {supervise.first_line(os.path.join(outdir, \'07-g3/G3.txt\'))}")'],
     ['        gok = g5.obligations(ctl, os.path.join(outdir, "07-g5"))',
      '        facts.append(f"G.5: {supervise.first_line(os.path.join(outdir, \'07-g5/G5.txt\'))}")']),
    ('main',
     ['            facts.append(f"G.3(e) recording, proposed for Architect review (not an acceptance): "',
      '                         f"\'{ctl[\'g3\'][\'recording\']}\'")'],
     ['            printed = json.load(open(os.path.join(outdir, "07-g5/G5.json")))["recording_proposed_for_architect_review"]',
      '            facts.append(f"G.5(F) recording, proposed for Architect review (not an acceptance): \'{printed}\'")',
      '            facts.append("G.5(E): the Sole Architect\'s independent review is PENDING (never claimed by the executor)")']),
    ('main',
     ['            stop = "the healthy witness or the restored-side receipt could not be linked, or G.3 does not hold"'],
     ['            stop = "the healthy witness or the restored-side receipt could not be linked, or G.5 does not hold"']),
    ('main',
     [],
     ['    # WITNESS-C4-01 (mission section 15): the same labels.']),
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
    for line in K.git("show", f"{K.W1}:{K.W1_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            t3csums[rel[6:]] = h
    carried = {}
    for line in K.git("show", f"{K.W1}:{K.W1_PACKET}/CARRIED-FORWARD.md").decode().split(
            "## The old versions")[0].splitlines():
        cells = [c.strip().strip("`") for c in line.split("|")[1:-1]]
        if len(cells) == 5 and cells[0] not in ("Tool", "---"):
            carried[cells[0]] = K.sha256_bytes(K.git("show", f"{cells[2]}:{cells[1]}"))
            check(f"D1 carried tool {cells[0]}: its sha256 at {cells[2][:8]}:{cells[1]} is the one the C1/C25 "
                  "mission's CARRIED-FORWARD.md records", carried[cells[0]] == cells[4], cells[4][:16])
    base = dict(carried, **t3csums)
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check(f"D1 original-tools/ ({len(old)}) is exactly the C1/C25 mission's execution tooling: its packet's tools/ "
          f"({len(t3csums)}; SHA256SUMS at {K.W1[:8]}) and the tools it carried byte for byte ({len(carried)})",
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
    check("D3 classify.classify_a and t3.rules are byte-identical and no changed or added tool calls either on C4's "
          "run (the G.5(A) attribution is g5.classify_primary's)",
          status.get("classify.py") == status.get("t3.py") == "UNCHANGED"
          and not [f for f in pyf if f != "t3_controls.py" and any(c in ("classify.classify_a", "t3.rules")
                                                                  for c, _ in calls(tree(os.path.join(NEW, f))))])
    # D4: control.py.
    tn = tree(os.path.join(NEW, "control.py"))
    to = tree(os.path.join(OLD, "control.py"))
    check("D4 control.py defines exactly git, gate, apply_patch, restored_check, restore, link, restore_auth, finish, "
          "main", [n.name for n in tn.body if isinstance(n, ast.FunctionDef)] == [
              "git", "gate", "apply_patch", "restored_check", "restore", "link", "restore_auth", "finish", "main"])
    ra = fn(tn, "restore_auth")
    ra_calls = sorted({c for c, _ in calls(ra)})
    check("D4 control.restore_auth (added) only reads: no process launch of its own (worktrees.worktree_entry's "
          "read-only `git worktree list` aside), no git write, no write but its own two RESTORE-AUTH records "
          "(RESTORE-AUTH.txt and RESTORE-AUTH.json, each opened with 'w' once), no supervise.build/invoke",
          not [c for c in ra_calls if c in ("git", "control.git") or c.startswith((
              "subprocess.", "os.system", "os.remove", "os.unlink", "os.rename", "shutil.", "supervise.build",
              "supervise.invoke", "worktrees.create", "K.git"))]
          and ast.unparse(ra).count("'w')") == 2
          and ast.unparse(ra).count("os.path.join(outdir, 'RESTORE-AUTH.txt'), 'w')") == 1
          and ast.unparse(ra).count("os.path.join(outdir, 'RESTORE-AUTH.json'), 'w')") == 1, ra_calls)
    for name in ("gate", "main"):
        a = src_lines(os.path.join(OLD, "control.py"), fn(to, name))
        b = src_lines(os.path.join(NEW, "control.py"), fn(tn, name))
        got = [(name, a[op[1]:op[2]], b[op[3]:op[4]]) for op in opcodes(a, b)]
        want = [h for h in AUTH_HUNKS if h[0] == name]
        check(f"D4 control.{name}: the C1/C25 mission's with exactly the {len(want)} authorized hunk(s) (old and new "
              "lines exact; nothing else differs)", got == [tuple(h) for h in want], f"{len(got)} hunks")
    first = fn(tn, "main").body[0]
    check("D4 control.main's first statement refuses every blocked control (C7) by name, before reading or writing "
          "anything", isinstance(first, ast.If) and ast.unparse(first.test) == "cid in K.BLOCKED"
          and isinstance(first.body[0], ast.Raise) and "SystemExit" in ast.unparse(first.body[0]),
          ast.unparse(first.test))
    second = fn(tn, "main").body[1]
    check("D4 control.main's second statement still refuses every reserved control by name (none remains: "
          "NOT_AUTHORIZED is empty)", K.NOT_AUTHORIZED == [] and isinstance(second, ast.If)
          and ast.unparse(second.test) == "cid in K.NOT_AUTHORIZED"
          and isinstance(second.body[0], ast.Raise) and "SystemExit" in ast.unparse(second.body[0]),
          ast.unparse(second.test))
    for f, names in SAME_FUNCS.items():
        fa, fb = funcs(tree(os.path.join(OLD, f))), funcs(tree(os.path.join(NEW, f)))
        diff = [n for n in names if fa.get(n) is None or fa.get(n) != fb.get(n)]
        check(f"D4 {f}: functions {names} AST-identical to the C1/C25 mission's", not diff, diff)
    # D5: common.py.
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    changed = sorted(k for k in co if k in cn and co[k] != cn[k])
    check("D5 common.py: values changed only for the declared names", set(changed) == COMMON_CHANGED,
          sorted(set(changed) ^ COMMON_CHANGED))
    added = sorted(k for k in cn if k not in co)
    check("D5 common.py: names added exactly the declared set (the C1/C25 identities, G5, records)",
          set(added) == COMMON_ADDED, sorted(set(added) ^ COMMON_ADDED))
    removed = sorted(k for k in co if k not in cn)
    check("D5 common.py: no name removed", not removed, removed)
    check("D5 common.py: the C1/C25 mission's records keep their exact texts under RULING_W1 and CAMPAIGN_W1",
          (cn["RULING_W1"], cn["RULING_W1_FIRST"], cn["CAMPAIGN_W1"], cn["CAMPAIGN_W1_FIRST"])
          == (co["RULING"], co["RULING_FIRST"], co["CAMPAIGN"], co["CAMPAIGN_FIRST"]))
    check("D5 common.py: the 33 accepted are the 31 with C1 C25 (the C1/C25 mission's order) appended; C7 alone is "
          "blocked; nothing is reserved; the order is the C1/C25 mission's one reserved control, C4; G3 unchanged",
          cn["ACCEPTED"] == co["ACCEPTED"] + co["ORDER"] == co["ACCEPTED"] + ["C1", "C25"]
          and cn["BLOCKED"] == co["BLOCKED"] == ["C7"] and cn["NOT_AUTHORIZED"] == []
          and cn["ORDER"] == co["NOT_AUTHORIZED"] == ["C4"] and sorted(cn["MATRIX"]) == ["C4"]
          and sorted(cn["G5"]) == ["C4"] and cn["G3"] == co["G3"]
          and len(set(cn["ORDER"] + cn["ACCEPTED"] + cn["BLOCKED"] + cn["NOT_AUTHORIZED"])) == 35)
    check("D5 common.py: the worktree namespace and scope prefix are this mission's own (mwc4); no preserved worktree",
          cn["UNIT_PREFIX"] == "p3c11-mwc4" and all(K.control_wt(c) == K.NEXUS + "/nexus-os-p3-c11-mwc4-" + c
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
            label = f"WITNESS-C1-C25-01/tools/{f}" if status[f] == "CHANGED" else "/dev/null"
            diff += difflib.unified_diff(a, open(os.path.join(NEW, f)).read().splitlines(keepends=True), label,
                                         f"WITNESS-C4-01/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the C1/C25 mission's execution tooling (`original-tools/`: `{K.W1_PACKET}/tools/` at `{K.W1}` and "
          "the tools it carried byte for byte, at the commits its CARRIED-FORWARD.md names). New: this mission's "
          "`tools/`. The complete old-to-new diff is `TOOL-DIFF.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`. "
          "The adaptation is the minimal one for AD-1-R4 G.5/G.6/F.1/F.2 (C4; mission section 5, step 7): one added "
          "module (`g5.py`), `g3.py` not carried, control.main's G.5 flow and its post-restoration authentication, the "
          "ordering, freeze, gate and publication bindings and the records.",
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
