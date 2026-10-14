#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Read-only but for OUTDIR.
Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (section 7): old = the RESUME-01 frozen tools, new = this
mission's tools; the authorization map is this mission's.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-DIFF.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

Old: the tools of the RESUME-01 evidence commit (original-tools/, each byte-checked against that packet's SHA256SUMS).
New: tools/. Checks:
  D1  original-tools/ is exactly the RESUME-01 packet's tools/ (names and sha256)
  D2  every tool is UNCHANGED, CHANGED or ADDED exactly as the authorization map below declares; none removed
  D3  the tools the mission forbids changing are byte-identical: classify.py (test classifiers, named-property and
      cardinality logic, the general build classifier), supervise.py (resource supervision), verify_mutant.py and
      verify_tree.py (source-verification semantics), worktrees.py, expectations.py (catalog expectations), configs.py,
      normalize.py, report.py, scope-wrap.sh, healthy.py, rehearsal.py, tool_controls.py, verifier_controls.py
  D4  inside changed files, every function the change does not need is unchanged (AST identity); control.main is
      unchanged but for its refusal and the C28 dispatch; authenticate.main sections [1]-[5] are unchanged
  D5  common.py: every name keeps its value except the declared mission/path/publication/carried names; only declared
      names are added; commands, environments, limits, identities and the previous orders unchanged
  D6  the carried-forward expectations equal the EXEC-01 packet's
  D7  control.main_c28 builds nothing: no classify_build_success, classify_d, classify_a, supervise.build,
      worktrees.create or apply_patch; it calls the unchanged classify_b once, with source_verdict EXACTLY_C
  D8  the dedicated C28 tools run no process but git (no cargo, no rustc), and c28_admit.py writes only into its OUTDIR
"""
import ast
import difflib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
I1 = "item 1 (a dedicated read-only C28 existing-build admission checker)"
I2 = "item 2 (a dedicated C28 pin step)"
I3 = ("item 3 (gate/order logic: C22 C14 C19 C21 C27 carried as Architect-accepted; start with C28; continue from C28 "
      "to C12)")
I4 = "item 4 (packet, rendering, freeze/final-gate and publication-path changes required for this mission)"
CHANGED = {
    "common.py": [I3 + ": CONTINUE_ACCEPTED, CONTINUE_ORDER, RULING, RULING_FIRST, C28_RULING, C28_RULING_FIRST",
                  I4 + ": MISSION, EV, EVIDENCE_WT, PACKET, BRANCH, LOCAL_BRANCH (DEST follows BRANCH), CARRIED (only "
                  "what this mission links or gates on), RESUME_* (the previous evidence of section 3)"],
    "control.py": [I3 + ": gate() over CONTINUE_ORDER (C28's prior steps: both ruling records, the carried-forward "
                   "baselines, the pre-execution gate; the preserved C28 worktree must exist; C12's prior step: C28's "
                   "candidate RESULT); main() refuses the five accepted controls and dispatches C28 to main_c28(); "
                   "main_c28() and reauthenticate_restoration(): C28 from its existing build (calls the item-1/2 tool, "
                   "the unchanged supervise.invoke, classify_b and link); docstring"],
    "authenticate.py": [I4 + " (inputs of the pre-execution and final gates; section 3): section [6] states this "
                        "mission's paths and both previous review refs; section [8] authenticates the complete RESUME-01 "
                        "packet, every C14/C19/C21/C27 result, the C28 history, the previous tool-repair and freeze "
                        "receipts and the original tools"],
    "record_freeze.py": [I4 + ": freeze logic (section 14): the ruling records, this mission's pre-freeze receipts, "
                         "the identities recorded, the policy text"],
    "pre_exec_gate.py": [I4 + ": the static pre-execution gate of section 14 (E1-E10)"],
    "final_gate.py": [I4 + ": final-gate logic: continue order, C28's sub-receipts, the accepted controls untouched"],
    "render.py": [I4 + ": packet/index rendering: combined Tranche-1 status, CARRIED-FORWARD.md, INDEX"],
    "render_rehearsal.py": [I4 + ": the rendering's tool control, for the revised render.py"],
    "build_packet.py": [I4 + ": packet: both ruling records staged; only changed/added tools staged; no duplication of "
                        "either previous packet"],
    "publish_checks.py": [I4 + ": publication paths: parent 3695a817, this packet and ref, both previous packets "
                          "unchanged, key receipts"],
    "tool_diff.py": [I4 + ": this diff and its check (section 7: a complete previous-to-current tool diff)"],
}
ADDED = {
    "c28_admit.py": [I1 + " and " + I2 + ": read-only; reads the RESUME-01 C28 build evidence from git objects; "
                     "hashes/stat-s the preserved executable; no Cargo, no rustc"],
    "c28_controls.py": [I1 + " and " + I2 + ": the tool controls required by section 14 (synthetic evidence and "
                        "synthetic files; the real executable only hashed)"],
}
FORBIDDEN = ["classify.py", "supervise.py", "verify_mutant.py", "verify_tree.py", "worktrees.py", "expectations.py",
             "configs.py", "normalize.py", "report.py", "scope-wrap.sh", "healthy.py", "rehearsal.py", "tool_controls.py",
             "verifier_controls.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "restored_check", "restore", "link", "finish"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result", "previous"],
              "render.py": ["lines", "jload", "outer", "res", "prev_ref", "control_row"],
              "build_packet.py": ["files_under", "carried"],
              "final_gate.py": ["check"],
              "pre_exec_gate.py": ["check", "last"],
              "publish_checks.py": ["git", "check", "tree_map", "glob_re", "push_matches", "gh", "mutant_blobs",
                                    "remote_blob", "actions"],
              "common.py": ["control_wt", "build_env", "git", "blob", "source", "sha256_bytes", "sha256_file",
                            "git_blob_id", "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH", "CARRIED"}
COMMON_ADDED = {"CONTINUE_ACCEPTED", "CONTINUE_ORDER", "RULING", "RULING_FIRST", "C28_RULING", "C28_RULING_FIRST",
                "RESUME_MISSION", "RESUME", "RESUME_TREE", "RESUME_PARENT", "RESUME_REF", "RESUME_PACKET",
                "RESUME_EVIDENCE_WT"}
results = []


def check(label, ok, detail=""):
    results.append((label, bool(ok), str(detail)))


def funcs(path):
    tree = ast.parse(open(path).read())
    return {n.name: ast.dump(n) for n in tree.body if isinstance(n, ast.FunctionDef)}, tree


def fn(tree, name):
    return next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name)


def namespace(path):
    ns = {}
    exec(compile(open(path).read(), path, "exec"), ns)
    return {k: v for k, v in ns.items() if not k.startswith("__") and not callable(v) and k not in ("hashlib", "os",
                                                                                                    "subprocess")}


def calls(node):
    """(dotted callee, call node) for every call under node."""
    out = []
    for n in ast.walk(node):
        if isinstance(n, ast.Call):
            out.append((ast.unparse(n.func), n))
    return out


def run():
    prev = {}
    for line in K.git("show", f"{K.RESUME}:{K.RESUME_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            prev[rel[6:]] = h
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check("D1 original-tools/ is exactly the RESUME-01 packet's tools/ (names and sha256 from its SHA256SUMS at "
          f"{K.RESUME[:8]})", old == prev, f"{len(old)} files")
    status = {f: ("UNCHANGED" if f in new and new[f] == old[f] else "CHANGED" if f in new else "REMOVED") for f in old}
    status.update({f: "ADDED" for f in new if f not in old})
    check("D2 no tool removed", not [f for f, s in status.items() if s == "REMOVED"])
    check("D2 the changed tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "CHANGED")
          == sorted(CHANGED), sorted(f for f, s in status.items() if s == "CHANGED"))
    check("D2 the added tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "ADDED")
          == sorted(ADDED), sorted(f for f, s in status.items() if s == "ADDED"))
    for f in FORBIDDEN:
        check(f"D3 {f} byte-identical (sha256 {old.get(f, '')[:16]}…)", status.get(f) == "UNCHANGED", status.get(f))
    for f, names in SAME_FUNCS.items():
        a, ta = funcs(os.path.join(OLD, f))
        b, tb = funcs(os.path.join(NEW, f))
        names = [n for n in names if n in a]
        diff = [n for n in names if a.get(n) != b.get(n)]
        check(f"D4 {f}: unchanged functions {names} (AST identity)", not diff, diff)
    mo, mn = (fn(ast.parse(open(os.path.join(d, "control.py")).read()), "main") for d in (OLD, NEW))
    check("D4 control.main: unchanged but for the leading refusal (the five accepted controls; CONTINUE_ORDER) and the "
          "C28 dispatch to main_c28()", [ast.dump(s) for s in mo.body[1:]] == [ast.dump(s) for s in mn.body[2:]]
          and "CONTINUE_ORDER" in ast.unparse(mn.body[0]) and "CONTINUE_ACCEPTED" in ast.unparse(mn.body[0])
          and ast.unparse(mn.body[1]) == "if cid == 'C28':\n    return main_c28()", ast.unparse(mn.body[1]))
    ao, an = (fn(ast.parse(open(os.path.join(d, "authenticate.py")).read()), "main") for d in (OLD, NEW))
    cut = next(i for i, s in enumerate(ao.body) if "[6] refs" in ast.unparse(s))
    check("D4 authenticate.main: sections [1]-[5] unchanged (statement by statement, up to the [6] heading)",
          [ast.dump(s) for s in ao.body[:cut]] == [ast.dump(s) for s in an.body[:cut]], f"{cut} statements")
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    changed = {k for k in co if k in cn and co[k] != cn[k]}
    check("D5 common.py: no name removed", not [k for k in co if k not in cn], [k for k in co if k not in cn])
    check("D5 common.py: changed values only for the declared mission/path/publication/carried names",
          changed <= COMMON_CHANGED, sorted(changed))
    check("D5 common.py: only declared names added", set(cn) - set(co) <= COMMON_ADDED, sorted(set(cn) - set(co)))
    check("D5 common.py: commands, environments, limits, identities and the previous orders unchanged",
          all(co[k] == cn[k] for k in ("C", "C_TREE", "BLOBS", "CATALOG_BLOB", "CATALOG_SHA", "BUILD_ARGV",
                                        "BUILD_LIMIT", "RUN_ENV", "MEMORY_MAX", "NEED_KIB", "ORDER", "NOT_AUTHORIZED",
                                        "NAMED", "EXPLORE_CFG", "EXE", "EXE_SHA256", "EXE_SIZE", "LISTED",
                                        "UNIT_PREFIX", "HEALTHY_WT", "RESUME_ORDER", "ACCEPTED", "PREV", "PREV_PACKET",
                                        "CARRIED_DOCS", "DEV_WT")))
    check("D5 common.py: CARRIED is a subset of the previous carried set", set(cn["CARRIED"]) <= set(co["CARRIED"]),
          cn["CARRIED"])
    e = os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")
    want = [l.split("  ")[0] for l in K.git("show", f"{K.PREV}:{K.PREV_PACKET}/SHA256SUMS").decode().splitlines()
            if l.endswith("  receipts/02-EXPECTATIONS.json")]
    check("D6 the carried-forward expectations equal the EXEC-01 packet's", [K.sha256_file(e)] == want, want)
    tn = ast.parse(open(os.path.join(NEW, "control.py")).read())
    c28 = [c for c, _ in calls(fn(tn, "main_c28"))] + [c for c, _ in calls(fn(tn, "reauthenticate_restoration"))]
    banned = ["classify.classify_build_success", "classify.classify_d", "classify.classify_a", "supervise.build",
              "worktrees.create", "apply_patch", "restore", "git"]
    check("D7 control.main_c28/reauthenticate_restoration build, apply, create or reverse nothing (no "
          + ", ".join(banned) + ")", not [c for c in c28 if c in banned], [c for c in c28 if c in banned])
    cb = [n for c, n in calls(fn(tn, "main_c28")) if c == "classify.classify_b"]
    check("D7 control.main_c28 calls the unchanged classify_b exactly once, with source_verdict='EXACTLY_C'",
          len(cb) == 1 and any(k.arg == "source_verdict" and ast.unparse(k.value) == "'EXACTLY_C'" for k in cb[0].keywords),
          [ast.unparse(n) for n in cb])
    for f in ADDED:
        t = ast.parse(open(os.path.join(NEW, f)).read())
        proc = [n for c, n in calls(t) if c in ("subprocess.run", "subprocess.Popen", "subprocess.check_output",
                                                  "subprocess.call", "os.system", "os.popen")]
        bad = [ast.unparse(n)[:80] for n in proc if not (isinstance(n.args[0], ast.List) and n.args[0].elts
                                                          and ast.unparse(n.args[0].elts[0]) == "'git'")]
        consts = [n.value for n in ast.walk(t) if isinstance(n, ast.Constant) and n.value in ("cargo", "rustc")]
        check(f"D8 {f}: every process it starts is git (no cargo, no rustc)", not bad and not consts, bad + consts)
    t = ast.parse(open(os.path.join(NEW, "c28_admit.py")).read())
    writes = [n for c, n in calls(t) if c == "open" and len(n.args) > 1 and ast.unparse(n.args[1]) in ("'w'", "'wb'",
                                                                                                       "'a'")]
    check("D8 c28_admit.py writes only into its OUTDIR (every write is open(os.path.join(outdir, ...), 'w'))",
          writes and all(ast.unparse(n.args[0]).startswith("os.path.join(outdir,") for n in writes),
          [ast.unparse(n.args[0]) for n in writes])
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
            b = open(os.path.join(NEW, f)).read().splitlines(keepends=True)
            diff += difflib.unified_diff(a, b, f"RESUME-01/tools/{f}" if a else "/dev/null", f"CONTINUE-02/tools/{f}")
    open(os.path.join(out, "TOOL-DIFF.diff"), "w").write("".join(diff))
    md = [f"# Tool authorization map — {K.MISSION}", "",
          f"Old: the tools of `{K.RESUME_PACKET}/tools/` at `{K.RESUME}` (RESUME-01's frozen tools). New: this "
          "mission's `tools/`. The complete previous-to-current diff is `TOOL-DIFF.diff`; the mechanical checks are "
          "`TOOL-DIFF-CHECK.txt`.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f, ADDED.get(f, ["byte-identical (no change)"])))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Not changed (mission section 7): catalog expectations, source-verification semantics (verify_mutant.py, "
           "verify_tree.py), test classifiers, named-property logic and cardinality logic (classify.py), resource "
           "supervision (supervise.py, scope-wrap.sh), and the general no-warning build policy for every control other "
           "than C28 (classify.classify_build_success, unchanged and still the classifier of C12, C9, C11, C17, C18a and "
           "C18b)."]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
